//! Does the suppression model do anything, and does it refuse the right things?
//!
//! Same shape as `evacuation.rs`, and for the same reason: the units are model,
//! not rendering, so the whole loop — dispatch, travel on the real network,
//! work, refill, withdraw — runs headlessly against the shipped scenario in
//! well under a second.
//!
//! The interesting assertions here are the *negative* ones. A suppression model
//! that quietly does nothing looks identical to one that works, because the
//! fire spreads either way; so most of these pin that units actually arrived,
//! actually spent water, and actually refused orders that cannot be carried out.

use abm::suppression::{Suppression, Task, UnitKind, UnitState};
use abm::Abm;
use fire::{FireSim, Weather};
use scenario::{Pos, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .unwrap()
}

struct World {
    scn: Scenario,
    fire: FireSim,
    agents: Abm,
    crews: Suppression,
    ignition: scenario::Cell,
}

fn setup() -> World {
    let scn = Scenario::load(data_dir()).unwrap();
    let weather = Weather::default();
    let plan = fire::plan_ignition(&scn, weather.wind_dir_deg, 250.0);
    let mut fire = FireSim::new(&scn, weather, 42).unwrap();
    fire.ignite_patch(plan.centre, plan.radius_m, &scn).unwrap();
    let agents = Abm::new(&scn, 42).unwrap();
    let bases: Vec<Pos> = agents.refuges.iter().map(|r| r.pos).collect();
    let crews = Suppression::new(&scn, &bases).unwrap();
    World { scn, fire, agents, crews, ignition: plan.centre }
}

impl World {
    fn run(&mut self, minutes: i64, dt: i64) {
        for _ in 0..(minutes * 60 / dt) {
            self.fire.advance(dt).unwrap();
            let actions =
                self.crews
                    .step(dt as f32, &self.agents.network, &self.agents.traffic, &self.fire, &self.scn);
            for a in actions {
                self.fire.queue(a);
            }
        }
    }

    /// A point `m` metres downwind of the ignition: where the fire is going,
    /// which is the only place suppression is worth putting. Wind is from the
    /// north here, so downwind is south (-y).
    fn downwind(&self, m: f32) -> Pos {
        let c = self.scn.world.centre_of(self.ignition);
        Pos { x: c.x, y: c.y - m }
    }

    /// `ahead_m` past the head of the fire as it is *now*: the furthest
    /// downwind burning cell. What a commander re-reads off the map every few
    /// minutes, and the difference between water that lands in front of the
    /// front and water that lands in the black behind it.
    fn head(&self, ahead_m: f32) -> Option<Pos> {
        self.fire
            .active_cells()
            .iter()
            .map(|c| self.scn.world.centre_of(*c))
            .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap())
            .map(|p| Pos { x: p.x, y: p.y - ahead_m })
    }
}

#[test]
fn roster_stages_on_measured_ground() {
    let w = setup();
    let engines = w.crews.units.iter().filter(|u| u.kind == UnitKind::Engine).count();
    let crews = w.crews.units.iter().filter(|u| u.kind == UnitKind::HandCrew).count();
    let air = w.crews.units.iter().filter(|u| u.kind == UnitKind::AirTanker).count();
    assert!(engines >= 2 && crews >= 2 && air >= 1);

    for u in &w.crews.units {
        if u.kind.is_air() {
            // Air support is not on the incident until it is asked for.
            assert_eq!(u.state, UnitState::Unavailable, "{} started available", u.callsign);
            assert!(!u.assignable());
            continue;
        }
        assert_eq!(u.state, UnitState::Staged);
        // Staging areas are the measured refuges, so they are out of the fuel.
        let frac = abm::refuge::burnable_fraction(&w.scn, u.pos, 300.0);
        assert!(
            frac < 0.2,
            "{} stages in {:.0}% burnable fuel",
            u.callsign,
            frac * 100.0
        );
    }
}

#[test]
fn an_engine_drives_to_the_fire_and_spends_its_tank() {
    let mut w = setup();
    let id = w.crews.nearest_available(w.downwind(300.0), UnitKind::Engine).unwrap();
    let start = w.crews.units[id].pos;
    w.crews.assign(id, Task::Attack { at: w.downwind(300.0) }).unwrap();

    w.run(40, 10);

    let u = &w.crews.units[id];
    println!(
        "{}: {} -> {:.0} m from staging, {} — {:.0} L used, tank {:.0}%, note {:?}",
        u.callsign,
        u.state.label(),
        ((u.pos.x - start.x).powi(2) + (u.pos.y - start.y).powi(2)).sqrt(),
        u.state.label(),
        u.water_used_l,
        u.water_frac() * 100.0,
        u.note,
    );
    assert_ne!(u.state, UnitState::Lost, "the engine was burnt over: {}", u.note);
    assert!(
        (u.pos.x - start.x).abs() + (u.pos.y - start.y).abs() > 100.0,
        "the engine never left staging"
    );
    // Either it is pumping, or it has been round the refill loop; both mean it
    // reached work. What must not happen is arriving and doing nothing.
    assert!(
        u.water_used_l > 0.0,
        "engine reached the fire but never applied water: {}",
        u.note
    );
}

#[test]
fn an_engine_refills_rather_than_running_dry_forever() {
    let mut w = setup();
    let id = w.crews.nearest_available(w.downwind(300.0), UnitKind::Engine).unwrap();
    w.crews.assign(id, Task::Attack { at: w.downwind(300.0) }).unwrap();
    w.run(90, 10);

    let u = &w.crews.units[id];
    println!(
        "{} after 90 min: {} · {:.0} L used ({:.1} tanks) · tank {:.0}%",
        u.callsign,
        u.state.label(),
        u.water_used_l,
        u.water_used_l / u.tank_l,
        u.water_frac() * 100.0
    );
    // A 2,500 L tank at 400 L/min is six minutes of pumping, so ninety minutes
    // of work is only possible if the refill loop actually closes.
    assert!(
        u.water_used_l > u.tank_l,
        "engine used {:.0} L, less than one tank, in 90 minutes",
        u.water_used_l
    );
}

#[test]
fn a_hand_crew_cuts_line_at_its_published_rate() {
    let mut w = setup();
    // A line across the fire's line of advance, 400 m ahead of it.
    let c = w.downwind(400.0);
    let from = Pos { x: c.x - 150.0, y: c.y };
    let to = Pos { x: c.x + 150.0, y: c.y };
    let id = w.crews.nearest_available(from, UnitKind::HandCrew).unwrap();
    w.crews.assign(id, Task::Line { from, to }).unwrap();

    w.run(120, 10);

    let u = &w.crews.units[id];
    let cut = u.line_cut_m;
    println!(
        "{}: {} · {:.0} m of line cut in 2 h · note {:?}",
        u.callsign,
        u.state.label(),
        cut,
        u.note
    );
    assert!(cut > 20.0, "the crew cut {cut:.0} m: it never started work");
    // Two hours at 120 m/h is 240 m, minus however long it took to walk in.
    // The ceiling is the honest half of this test: a hand crew cannot cut a
    // kilometre, and a model that lets it would make the aircraft pointless.
    assert!(
        cut <= 245.0,
        "the crew cut {cut:.0} m in two hours, above its {} m/h rate",
        abm::suppression::LINE_M_PER_H
    );
    assert!(
        w.fire.cleared().iter().filter(|c| **c).count() > 0,
        "line was cut but no cell lost its fuel"
    );
}

#[test]
fn air_support_has_to_be_requested_and_takes_time_to_arrive() {
    let mut w = setup();
    let air: Vec<usize> = w
        .crews
        .units
        .iter()
        .filter(|u| u.kind.is_air())
        .map(|u| u.id)
        .collect();

    // Cannot be tasked before it is called for.
    let err = w.crews.assign(air[0], Task::Drop { at: w.downwind(300.0) });
    assert!(err.is_err(), "an unrequested aircraft accepted an order");

    let n = w.crews.request_air();
    assert_eq!(n, air.len());
    let eta = w.crews.air_eta_s().expect("an eta once requested");
    println!("air support requested: {n} aircraft, first overhead in {:.0} min", eta / 60.0);
    assert!((eta - abm::suppression::AIR_RESPONSE_S).abs() < 1.0);

    // Ten minutes in, still inbound and still not on scene -- but briefable,
    // which is what a real incident does with an aircraft on its way.
    w.run(10, 10);
    assert_eq!(w.crews.units[air[0]].state, UnitState::Inbound);
    assert!(!w.crews.units[air[0]].on_scene());
    assert!(
        w.crews.assign(air[0], Task::Drop { at: w.downwind(300.0) }).is_ok(),
        "an inbound aircraft could not be briefed"
    );
    assert_eq!(
        w.crews.units[air[0]].state,
        UnitState::Inbound,
        "briefing an inbound aircraft teleported it onto the incident"
    );

    // Past the response time it is here, and already working its briefing
    // rather than waiting to be noticed.
    w.run(20, 10);
    assert!(w.crews.units[air[0]].on_scene(), "aircraft never arrived");
    assert_ne!(
        w.crews.units[air[0]].state,
        UnitState::Staged,
        "a briefed aircraft arrived and stood idle"
    );

    // The untasked one is simply on station.
    assert_eq!(w.crews.units[air[1]].state, UnitState::Staged);
}

#[test]
fn a_tanker_cycles_between_the_fire_and_the_water() {
    let mut w = setup();
    w.crews.request_air();
    // Wait it in.
    w.run(26, 10);
    let id = w.crews.units.iter().find(|u| u.kind.is_air()).unwrap().id;
    w.crews.assign(id, Task::Drop { at: w.downwind(300.0) }).unwrap();

    let litres_before = w.fire.litres_applied;
    w.run(45, 10);

    let u = &w.crews.units[id];
    println!(
        "{}: {} drops, {:.0} L delivered; fire received {:.0} L total",
        u.callsign,
        u.drops,
        u.water_used_l,
        w.fire.litres_applied - litres_before
    );
    assert!(u.drops >= 2, "only {} drop(s) in 45 minutes of tasking", u.drops);
    assert!(w.fire.litres_applied > litres_before, "no water reached the fire");
}

#[test]
fn units_refuse_orders_their_kind_cannot_carry_out() {
    let mut w = setup();
    let engine = w.crews.units.iter().find(|u| u.kind == UnitKind::Engine).unwrap().id;
    let crew = w.crews.units.iter().find(|u| u.kind == UnitKind::HandCrew).unwrap().id;
    let here = w.downwind(300.0);

    assert!(
        w.crews.assign(engine, Task::Line { from: here, to: here }).is_err(),
        "an engine accepted a hand-line order"
    );
    assert!(
        w.crews.assign(engine, Task::Drop { at: here }).is_err(),
        "an engine accepted a drop"
    );
    assert!(
        w.crews.assign(crew, Task::Drop { at: here }).is_err(),
        "a hand crew accepted a drop"
    );
    // And the orders each kind *can* take are accepted.
    assert!(w.crews.assign(engine, Task::Attack { at: here }).is_ok());
    assert!(w.crews.assign(crew, Task::Attack { at: here }).is_ok());
}

#[test]
fn a_unit_sent_into_the_fire_withdraws_instead_of_dying() {
    let mut w = setup();
    // Let the fire establish, then order a crew straight into the middle of it.
    w.run(20, 20);
    let centre = w.scn.world.centre_of(w.ignition);
    let id = w.crews.nearest_available(centre, UnitKind::HandCrew).unwrap();
    w.crews.assign(id, Task::Attack { at: centre }).unwrap();
    w.run(60, 10);

    let u = &w.crews.units[id];
    println!(
        "{} ordered into the burn: {} — note {:?}, heat {:.0} s",
        u.callsign,
        u.state.label(),
        u.note,
        u.heat_s
    );
    // It may still be walking in, it may have turned round, it may be back at
    // staging. What it must not be is dead, and it must not be pretending to
    // work inside the front.
    assert_ne!(u.state, UnitState::Lost, "obeying an order killed the unit");
    if u.state == UnitState::Working {
        assert!(
            fire::ThreatField::at(w.fire.threat(), u.pos) < abm::suppression::WORK_LIMIT,
            "unit is working where the threat field says it cannot"
        );
    }
}

#[test]
fn water_is_independent_of_step_size() {
    // The same trap as `fire::exposure` and the civilian model: work has to be
    // integrated over simulated time, not accrued per call. The game steps at
    // 2 s and these tests at 60 s, a 30x difference.
    let spent = |dt: i64| -> (f32, f32) {
        let mut w = setup();
        let at = w.downwind(300.0);
        let e = w.crews.nearest_available(at, UnitKind::Engine).unwrap();
        let c = w.crews.nearest_available(at, UnitKind::HandCrew).unwrap();
        w.crews.assign(e, Task::Attack { at }).unwrap();
        let line_to = Pos { x: at.x + 200.0, y: at.y };
        w.crews.assign(c, Task::Line { from: at, to: line_to }).unwrap();
        w.run(60, dt);
        (w.crews.units[e].water_used_l, w.crews.units[c].line_cut_m)
    };

    let (fine_l, fine_m) = spent(2);
    let (coarse_l, coarse_m) = spent(60);
    println!(
        "water {fine_l:.0} L at 2 s vs {coarse_l:.0} L at 60 s; \
         line {fine_m:.0} m vs {coarse_m:.0} m"
    );
    assert!(fine_l > 0.0 && fine_m > 0.0, "nothing happened at the fine step");
    // Not exact: travel is integrated in whole steps, so a coarse run reaches
    // work a few seconds earlier or later. Within 15% is the standard the
    // civilian model is held to as well.
    let rel = |a: f32, b: f32| (a - b).abs() / a.max(b).max(1.0);
    assert!(
        rel(fine_l, coarse_l) < 0.15,
        "water use depends on step size: {fine_l:.0} vs {coarse_l:.0}"
    );
    assert!(
        rel(fine_m, coarse_m) < 0.15,
        "line production depends on step size: {fine_m:.0} vs {coarse_m:.0}"
    );
}

/// How the incident was fought, for [`suppression_changes_the_outcome`].
enum Plan {
    /// Nobody is dispatched at all.
    None,
    /// Everything is sent to one point ahead of the fire at T+0 and left there.
    /// The intuitive plan, and the one that wastes most of the water: the front
    /// passes the drop point and every load after that lands in the black.
    Static,
    /// The aircraft are re-tasked onto the head of the fire every five minutes,
    /// which is what a commander with a map actually does.
    Retasked,
}

#[test]
#[ignore = "measures the whole point of the feature; run with --ignored"]
fn suppression_changes_the_outcome() {
    let fought = |plan: Plan| -> (f32, f32, f64) {
        let mut w = setup();
        let ground: Vec<usize> = w
            .crews
            .units
            .iter()
            .filter(|u| !u.kind.is_air())
            .map(|u| u.id)
            .collect();
        let air: Vec<usize> =
            w.crews.units.iter().filter(|u| u.kind.is_air()).map(|u| u.id).collect();

        match plan {
            Plan::None => w.run(120, 10),
            Plan::Static | Plan::Retasked => {
                let head = w.downwind(350.0);
                w.crews.request_air();
                for id in &ground {
                    let _ = w.crews.assign(*id, Task::Attack { at: head });
                }
                // Aircraft cannot be tasked until they are on station.
                w.run(26, 10);
                for id in &air {
                    let _ = w.crews.assign(*id, Task::Drop { at: head });
                }
                if matches!(plan, Plan::Static) {
                    w.run(94, 10);
                } else {
                    for _ in 0..19 {
                        w.run(5, 10);
                        if let Some(at) = w.head(80.0) {
                            for id in &air {
                                let _ = w.crews.assign(*id, Task::Drop { at });
                            }
                        }
                    }
                }
            }
        }

        let cells = w
            .fire
            .state()
            .iter()
            .filter(|s| **s != fire::CellFire::Unburnt)
            .count();
        let cell_ha = w.scn.world.cellsize * w.scn.world.cellsize / 10_000.0;
        let stats = w.crews.stats();
        (cells as f32 * cell_ha, stats.line_m, stats.water_l)
    };

    let (free, _, _) = fought(Plan::None);
    let (static_ha, static_m, static_l) = fought(Plan::Static);
    let (retask_ha, retask_m, retask_l) = fought(Plan::Retasked);
    println!(
        "2 h at seed 42, tramontana 35 km/h, 6% moisture:\n\
         \x20 no suppression            {free:5.1} ha\n\
         \x20 committed to one point    {static_ha:5.1} ha  \
         ({static_m:3.0} m line, {:.0} kL)\n\
         \x20 aircraft re-tasked /5 min {retask_ha:5.1} ha  \
         ({retask_m:3.0} m line, {:.0} kL)",
        static_l / 1000.0,
        retask_l / 1000.0,
    );
    assert!(static_ha < free, "dispatching everything did not help at all");
    assert!(
        retask_ha <= static_ha,
        "following the head of the fire was worse than ignoring it \
         ({retask_ha:.1} vs {static_ha:.1} ha)"
    );
}
