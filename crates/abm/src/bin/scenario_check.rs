//! Headless validator for a Scenario Factory output directory.
//!
//! `scenario_check <data_dir> <scenario_id>` prints one JSON report on stdout
//! and exits 0 when every hard check passes, 1 otherwise.
//!
//! The point of this tool is that it asks the *model* what it makes of a
//! scenario. Refuges, havens, the road network, its components, civilian route
//! fields, unit rosters and engine routing all come from the same `abm` code the
//! game runs; nothing here re-implements a decision. What this file adds is
//! bookkeeping: join the results to the population and summarise per locality.
//! A scenario the factory emits is only trustworthy if this binary and the game
//! agree about it, which is why there is no second implementation.

use std::collections::{BTreeMap, HashMap, HashSet};

use abm::network::{self, NodeId, RoadNetwork};
use abm::suppression::{Suppression, UnitKind, ENGINE_SPEED};
use abm::Abm;
use scenario::{Pos, Scenario};
use serde_json::{json, Value};

/// A household must sit this close to the centroid of the building it claims.
const HOUSE_TO_BUILDING_M: f32 = 50.0;
/// Share of a locality's households that must have a car route to a refuge.
const MIN_CAR_ROUTE_SHARE: f32 = 0.90;
/// How many offending ids a violation list shows; counts are always exact.
const EXAMPLES: usize = 5;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: scenario_check <data_dir> <scenario_id>");
        std::process::exit(2);
    }
    let report = check(&args[1], &args[2]);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    let ok = report["ok"].as_bool().unwrap_or(false);
    std::process::exit(if ok { 0 } else { 1 });
}

/// Accumulates the hard-check list so each check is one line at its call site.
#[derive(Default)]
struct Checks(Vec<Value>);

impl Checks {
    fn add(&mut self, name: &str, ok: bool, detail: impl Into<String>) {
        self.0.push(json!({ "name": name, "ok": ok, "detail": detail.into() }));
    }
    fn all_ok(&self) -> bool {
        self.0.iter().all(|c| c["ok"].as_bool().unwrap_or(false))
    }
}

fn check(data_dir: &str, id: &str) -> Value {
    let mut checks = Checks::default();
    // A load failure is itself a hard failure, and there is nothing else to
    // report without a scenario, so the report degenerates to that one check.
    let scn = match Scenario::load_by_id(data_dir, id) {
        Ok(s) => {
            checks.add("load", true, "Scenario::load_by_id succeeded");
            s
        }
        Err(e) => {
            checks.add("load", false, format!("{e:#}"));
            return json!({ "scenario": id, "ok": false, "hard_checks": checks.0 });
        }
    };

    let world = scn.world;
    let n_fire = world.fire_rows * world.fire_cols;

    // ---- grids -----------------------------------------------------------
    // The render terrain is a separate raster; if its extent differs from the
    // fire grid's, every vector is draped on the wrong ground.
    let t = &scn.terrain;
    let tol = t.posting + 1.0;
    let render_w = t.cols as f32 * t.posting;
    let render_h = t.rows as f32 * t.posting;
    let extent_ok = (t.width_m - world.width_m).abs() <= tol
        && (t.height_m - world.height_m).abs() <= tol
        && (render_w - world.width_m).abs() <= tol
        && (render_h - world.height_m).abs() <= tol;
    let fire_extent_ok = ((world.fire_cols as f32 * world.cellsize) - world.width_m).abs() <= world.cellsize
        && ((world.fire_rows as f32 * world.cellsize) - world.height_m).abs() <= world.cellsize;
    let lens_ok = scn.fuel.len() == n_fire && scn.dem.len() == n_fire && t.elev.len() == t.rows * t.cols;
    let meta_ok = scn.metadata.fire_grid_size == [world.fire_rows, world.fire_cols];
    let grid_ok = extent_ok && fire_extent_ok && lens_ok && meta_ok;
    checks.add(
        "grid_match",
        grid_ok,
        format!(
            "render {}x{} @ {} m = {:.0}x{:.0} m; fire {}x{} @ {} m; world {:.0}x{:.0} m; \
             render_extent_ok={extent_ok} fire_extent_ok={fire_extent_ok} lens_ok={lens_ok} metadata_grid_ok={meta_ok}",
            t.cols, t.rows, t.posting, render_w, render_h,
            world.fire_cols, world.fire_rows, world.cellsize, world.width_m, world.height_m
        ),
    );

    // ---- counts ----------------------------------------------------------
    let v = &scn.vectors;
    let pop = &scn.population;
    let drivable = v.roads.iter().filter(|r| r.drivable).count();
    let track = v.roads.iter().filter(|r| r.track && !r.drivable).count();
    let other = v.roads.len() - drivable - track;
    let mut water: BTreeMap<&str, usize> = BTreeMap::new();
    for w in &v.water {
        *water.entry(w.kind.as_str()).or_default() += 1;
    }
    let mut loc_hh: BTreeMap<String, usize> = BTreeMap::new();
    let mut loc_people: BTreeMap<String, usize> = BTreeMap::new();
    let loc_of = |h: &scenario::Household| h.locality.clone().unwrap_or_else(|| "(none)".into());
    for h in &pop.households {
        *loc_hh.entry(loc_of(h)).or_default() += 1;
        *loc_people.entry(loc_of(h)).or_default() += h.members.len().max(h.size as usize);
    }

    // ---- households and buildings ---------------------------------------
    let buildings: HashMap<i64, &scenario::Building> = v.buildings.iter().map(|b| (b.id, b)).collect();
    let (mut missing_b, mut far_b, mut off_world, mut on_fuel_h, mut orphan_p) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut n_missing_b, mut n_far_b, mut n_off, mut n_fuel_h, mut n_orphan) = (0, 0, 0, 0, 0);
    let mut cell_mismatch = 0usize;
    let hh_ids: HashSet<usize> = pop.households.iter().map(|h| h.id).collect();
    for h in &pop.households {
        let p = Pos { x: h.pos[0], y: h.pos[1] };
        let push = |n: &mut usize, list: &mut Vec<usize>| {
            *n += 1;
            if list.len() < EXAMPLES {
                list.push(h.id);
            }
        };
        match buildings.get(&h.building) {
            None => push(&mut n_missing_b, &mut missing_b),
            Some(b) => {
                let d = ((b.centroid[0] - p.x).powi(2) + (b.centroid[1] - p.y).powi(2)).sqrt();
                if d > HOUSE_TO_BUILDING_M {
                    push(&mut n_far_b, &mut far_b);
                }
            }
        }
        if !world.contains(p) {
            push(&mut n_off, &mut off_world);
        }
        let cell = world.cell_of(p);
        // A house must stand on ground the fire model cannot burn.
        if scn.is_burnable(cell) {
            push(&mut n_fuel_h, &mut on_fuel_h);
        }
        if h.cell != [cell.row, cell.col] {
            cell_mismatch += 1;
        }
    }
    for p in &pop.people {
        if !hh_ids.contains(&p.household) {
            n_orphan += 1;
            if orphan_p.len() < EXAMPLES {
                orphan_p.push(p.id);
            }
        }
    }
    let house_link_ok = n_missing_b == 0 && n_far_b == 0 && n_off == 0 && n_orphan == 0;
    checks.add(
        "household_building_link",
        house_link_ok,
        format!(
            "missing_building={n_missing_b} farther_than_{HOUSE_TO_BUILDING_M}m={n_far_b} \
             outside_world={n_off} people_without_household={n_orphan}"
        ),
    );
    checks.add("households_on_non_burnable", n_fuel_h == 0, format!("{n_fuel_h} households on burnable cells"));

    let buildings_on_fuel: Vec<i64> = v
        .buildings
        .iter()
        .filter(|b| scn.is_burnable(world.cell_of(Pos { x: b.centroid[0], y: b.centroid[1] })))
        .map(|b| b.id)
        .collect();
    checks.add(
        "buildings_on_non_burnable",
        buildings_on_fuel.is_empty(),
        format!("{} building centroids on burnable cells", buildings_on_fuel.len()),
    );

    // ---- model: network, refuges, units ----------------------------------
    // `Abm::new` is the authority on refuges and havens; building it here (not
    // calling refuge::choose ourselves) guarantees we report what the game uses.
    let agents = match Abm::new(&scn, 42) {
        Ok(a) => a,
        Err(e) => {
            checks.add("abm_builds", false, format!("{e:#}"));
            return json!({ "scenario": id, "ok": false, "hard_checks": checks.0 });
        }
    };
    checks.add("abm_builds", true, "Abm::new succeeded");
    let net: &RoadNetwork = &agents.network;

    // Components are over drivable nodes only: a footpath node is its own
    // label in the drivable labelling and would swamp the statistics.
    let mut comp_sizes: HashMap<u32, usize> = HashMap::new();
    let mut drivable_nodes = 0usize;
    for n in 0..net.len() as NodeId {
        if net.is_drivable_node(n) {
            drivable_nodes += 1;
            *comp_sizes.entry(net.component(n, true)).or_default() += 1;
        }
    }
    let mut sizes: Vec<(u32, usize)> = comp_sizes.into_iter().collect();
    sizes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let main_comp = sizes.first().map(|s| s.0);
    let on_main = |n: NodeId| Some(net.component(n, true)) == main_comp;
    let outside_main = drivable_nodes - sizes.first().map(|s| s.1).unwrap_or(0);

    // Which drivable component each household's car entry node is in. More than
    // one means some households can never drive to the main network.
    let entry_car: Vec<Option<NodeId>> = pop
        .households
        .iter()
        .map(|h| net.nearest(Pos { x: h.pos[0], y: h.pos[1] }, true))
        .collect();
    let entry_foot: Vec<Option<NodeId>> = pop
        .households
        .iter()
        .map(|h| net.nearest(Pos { x: h.pos[0], y: h.pos[1] }, false))
        .collect();
    let mut hh_comps: HashMap<u32, usize> = HashMap::new();
    let mut hh_no_node = 0usize;
    for e in &entry_car {
        match e {
            Some(n) => *hh_comps.entry(net.component(*n, true)).or_default() += 1,
            None => hh_no_node += 1,
        }
    }
    let hh_off_main = pop
        .households
        .iter()
        .zip(&entry_car)
        .filter(|(_, e)| e.map_or(true, |n| !on_main(n)))
        .count();
    checks.add(
        "households_on_main_drivable_component",
        hh_comps.len() <= 1 && hh_no_node == 0,
        format!(
            "{} drivable components contain households; {hh_off_main} households' nearest drivable node is off the largest; {hh_no_node} have no drivable node",
            hh_comps.len()
        ),
    );

    // Refuges and havens, as the model chose them.
    let n_exit = agents.refuges.iter().filter(|r| r.is_exit).count();
    let n_inner = agents.refuges.len() - n_exit;
    checks.add("non_exit_refuge_exists", n_inner > 0, format!("{n_inner} in-town refuges, {n_exit} map-edge exits"));
    let refuge_json: Vec<Value> = agents
        .refuges
        .iter()
        .map(|r| {
            json!({
                "x": r.pos.x, "y": r.pos.y, "node": r.node,
                "kind": if r.is_exit { "exit" } else { "clearing" },
                "burnable_frac": r.burnable_frac,
                "on_main_drivable": on_main(r.node),
            })
        })
        .collect();
    let havens_water = agents.havens.iter().filter(|h| h.is_water()).count();
    let haven_json: Vec<Value> = agents
        .havens
        .iter()
        .map(|h| {
            json!({ "x": h.pos.x, "y": h.pos.y,
                    "kind": if h.is_water() { "water" } else { "open_ground" },
                    "burnable_frac": h.burnable_frac })
        })
        .collect();

    // Reachability at t=0: the same multi-source Dijkstra the game uses, with
    // an empty threat field. `ThreatField::new` is all-zero, i.e. no fire.
    let calm = fire::ThreatField::new(world);
    let refuge_nodes: Vec<NodeId> = agents.refuges.iter().map(|r| r.node).collect();
    let car = network::solve(net, &refuge_nodes, &calm, true);
    let foot = network::solve(net, &refuge_nodes, &calm, false);
    struct Loc {
        n: usize,
        car_ok: usize,
        foot_ok: usize,
        car_s: Vec<f32>,
        foot_s: Vec<f32>,
        entry_m: Vec<f32>,
        sum: (f32, f32),
    }
    let mut locs: BTreeMap<String, Loc> = BTreeMap::new();
    for (i, h) in pop.households.iter().enumerate() {
        let l = locs.entry(loc_of(h)).or_insert(Loc {
            n: 0, car_ok: 0, foot_ok: 0, car_s: vec![], foot_s: vec![], entry_m: vec![], sum: (0.0, 0.0),
        });
        l.n += 1;
        l.sum.0 += h.pos[0];
        l.sum.1 += h.pos[1];
        if let Some(n) = entry_car[i] {
            if car.reachable(n) {
                l.car_ok += 1;
                l.car_s.push(car.cost[n as usize]);
            }
            let p = net.pos(n);
            l.entry_m.push(((p.x - h.pos[0]).powi(2) + (p.y - h.pos[1]).powi(2)).sqrt());
        }
        if let Some(n) = entry_foot[i] {
            if foot.reachable(n) {
                l.foot_ok += 1;
                l.foot_s.push(foot.cost[n as usize]);
            }
        }
    }
    let share = |a: usize, n: usize| if n == 0 { 0.0 } else { a as f32 / n as f32 };
    let mut worst_locality = (String::new(), 2.0f32);
    let reach_json: Vec<Value> = locs
        .iter()
        .map(|(name, l)| {
            let cs = share(l.car_ok, l.n);
            if cs < worst_locality.1 {
                worst_locality = (name.clone(), cs);
            }
            json!({
                "locality": name, "households": l.n,
                "car_route_share": cs, "foot_route_share": share(l.foot_ok, l.n),
                "car_time_s_median": median(&l.car_s), "car_time_s_max": max(&l.car_s),
                "foot_time_s_median": median(&l.foot_s), "foot_time_s_max": max(&l.foot_s),
                "home_to_road_m_median": median(&l.entry_m), "home_to_road_m_max": max(&l.entry_m),
            })
        })
        .collect();
    let car_ok_all = locs.values().all(|l| share(l.car_ok, l.n) >= MIN_CAR_ROUTE_SHARE);
    checks.add(
        "locality_car_route_share",
        car_ok_all,
        format!(
            "worst locality {:?} at {:.0}% (need >= {:.0}%)",
            worst_locality.0, worst_locality.1 * 100.0, MIN_CAR_ROUTE_SHARE * 100.0
        ),
    );

    // Units: staged at the refuges, the measured staging areas.
    // Ignition is not known here, so refuge order stands in for "closest first".
    let bases: Vec<Pos> = agents.refuges.iter().map(|r| r.pos).collect();
    let (units_json, engines_bad, engine_eta, hydrants_json) = match Suppression::new(&scn, &bases) {
        Ok(crews) => {
            let mut bad = Vec::new();
            let mut units = Vec::new();
            let mut etas = Vec::new();
            for u in &crews.units {
                let ground = !u.kind.is_air();
                let node = ground.then(|| net.nearest(u.base, true)).flatten();
                // "Reachable" as `Suppression` itself asks: same component as
                // the node the unit stands at.
                let reach = node.and_then(|f| net.nearest_reachable(u.base, true, f));
                let main = node.map_or(false, on_main);
                if u.kind == UnitKind::Engine && !main {
                    bad.push(u.id);
                }
                units.push(json!({
                    "id": u.id, "kind": u.kind.label(), "callsign": u.callsign,
                    "base": { "x": u.base.x, "y": u.base.y },
                    "ground": ground, "base_node": node,
                    "base_on_main_drivable": if ground { json!(main) } else { Value::Null },
                    "nearest_reachable_node": reach,
                }));
                if u.kind == UnitKind::Engine {
                    for (name, l) in &locs {
                        let c = Pos { x: l.sum.0 / l.n as f32, y: l.sum.1 / l.n as f32 };
                        let eta = node
                            .and_then(|f| net.nearest_reachable(c, true, f).map(|t| (f, t)))
                            .and_then(|(f, t)| network::route(net, f, t, &calm, true).map(|p| (f, p)))
                            .map(|(f, path)| {
                                let mut at = net.pos(f);
                                let mut m = 0.0;
                                for n in path {
                                    let p = net.pos(n);
                                    m += ((p.x - at.x).powi(2) + (p.y - at.y).powi(2)).sqrt();
                                    at = p;
                                }
                                m
                            });
                        etas.push(json!({
                            "unit": u.id, "locality": name,
                            "road_m": eta, "eta_s": eta.map(|m| m / ENGINE_SPEED),
                        }));
                    }
                }
            }
            let hyd: Vec<Pos> = v.hydrants().map(|w| Pos { x: w.pos[0], y: w.pos[1] }).collect();
            let reachable = hyd
                .iter()
                .filter(|p| net.nearest(**p, true).map_or(false, on_main))
                .count();
            let hj = json!({ "total": hyd.len(), "on_main_drivable": reachable });
            (units, bad, etas, hj)
        }
        Err(e) => {
            checks.add("suppression_builds", false, format!("{e:#}"));
            (vec![], vec![], vec![], Value::Null)
        }
    };
    checks.add(
        "engine_bases_on_main_drivable",
        engines_bad.is_empty(),
        format!("engines off the main component: {engines_bad:?}"),
    );

    json!({
        "scenario": id,
        "ok": checks.all_ok(),
        "world": {
            "width_m": world.width_m, "height_m": world.height_m,
            "fire_rows": world.fire_rows, "fire_cols": world.fire_cols, "cellsize": world.cellsize,
            "render_rows": t.rows, "render_cols": t.cols, "render_posting_m": t.posting,
            "grid_ok": grid_ok,
        },
        "counts": {
            "buildings": v.buildings.len(),
            "roads": { "drivable": drivable, "track": track, "other": other },
            "water": water,
            "households": pop.households.len(), "people": pop.people.len(), "dwellings": pop.dwellings.len(),
            "per_locality": loc_hh.iter().map(|(k, h)| (k.clone(), json!({
                "households": h, "people": loc_people[k],
            }))).collect::<BTreeMap<_, _>>(),
        },
        "households_ok": {
            "ok": house_link_ok && n_fuel_h == 0,
            "missing_building": { "count": n_missing_b, "examples": missing_b },
            "farther_than_50m_from_building": { "count": n_far_b, "examples": far_b },
            "outside_world": { "count": n_off, "examples": off_world },
            "people_without_household": { "count": n_orphan, "examples": orphan_p },
            "on_burnable_cell": { "count": n_fuel_h, "examples": on_fuel_h },
            "stored_cell_differs_from_position": cell_mismatch,
        },
        "buildings_on_fuel": buildings_on_fuel.len(),
        "buildings_on_fuel_examples": buildings_on_fuel.iter().take(EXAMPLES).collect::<Vec<_>>(),
        "roads": {
            "network_nodes": net.len(), "network_edges": net.edge_count,
            "drivable_nodes": drivable_nodes,
            "drivable_components": sizes.len(),
            "largest_component": sizes.first().map(|s| s.1).unwrap_or(0),
            "largest_5_sizes": sizes.iter().take(5).map(|s| s.1).collect::<Vec<_>>(),
            "drivable_nodes_outside_largest": outside_main,
            "all_drivable_in_largest": outside_main == 0,
            "households_by_drivable_component": hh_comps.len(),
            "households_off_largest": hh_off_main,
            "exits": n_exit,
        },
        "refuges": { "count": agents.refuges.len(), "exit": n_exit, "clearing": n_inner, "list": refuge_json },
        "havens": { "count": agents.havens.len(), "water": havens_water, "list": haven_json },
        "reachability": reach_json,
        "units": { "roster": units_json, "engine_eta_to_locality_centroid": engine_eta, "hydrants": hydrants_json },
        "hard_checks": checks.0,
    })
}

fn median(v: &[f32]) -> Value {
    if v.is_empty() {
        return Value::Null;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    json!(s[s.len() / 2])
}

fn max(v: &[f32]) -> Value {
    v.iter().copied().fold(None, |m: Option<f32>, x| Some(m.map_or(x, |m| m.max(x)))).map_or(Value::Null, |m| json!(m))
}
