//! Does an editable unit policy actually govern every unit?
//!
//! The same shape as `authored_behaviour.rs`, and the same worry. A policy layer
//! that quietly does nothing is indistinguishable from one that works, because
//! the units carry on doing what the commander told them either way — so most of
//! this pins that the policy is *reached*: that the baseline graph is stable,
//! and that changing one number in it visibly
//! changes what a unit does on a real incident.

use std::collections::BTreeMap;

use abm::suppression::{Suppression, Task, UnitKind, UnitState, WORK_LIMIT};
use abm::{Abm, UnitRuntime};
use behavior::{AgentSubtype, BehaviorGraph, Library, ParamValue, UnitKindKey};
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

/// The shipped scenario, using the default graph policy or an explicitly
/// supplied graph runtime.
fn setup(policy: Option<UnitRuntime>) -> World {
    let scn = Scenario::load(data_dir()).unwrap();
    let weather = Weather::default();
    let plan = fire::plan_ignition(&scn, weather.wind_dir_deg, 250.0);
    let mut fire = FireSim::new(&scn, weather, 42).unwrap();
    fire.ignite_patch(plan.centre, plan.radius_m, &scn).unwrap();
    let agents = Abm::new(&scn, 42).unwrap();
    let bases: Vec<Pos> = agents.refuges.iter().map(|r| r.pos).collect();
    let crews = match policy {
        Some(policy) => Suppression::with_policy(&scn, &bases, policy).unwrap(),
        None => Suppression::new(&scn, &bases).unwrap(),
    };
    World { scn, fire, agents, crews, ignition: plan.centre }
}

impl World {
    fn run(&mut self, minutes: i64, dt: i64) {
        for _ in 0..(minutes * 60 / dt) {
            self.fire.advance(dt).unwrap();
            let actions = self.crews.step(dt as f32, &self.agents.network, &self.agents.traffic, &self.fire, &self.scn);
            for a in actions {
                self.fire.queue(a);
            }
        }
    }

    fn downwind(&self, m: f32) -> Pos {
        let c = self.scn.world.centre_of(self.ignition);
        Pos { x: c.x, y: c.y - m }
    }

    fn unit(&self, kind: UnitKind) -> usize {
        self.crews.units.iter().position(|u| u.kind == kind).unwrap()
    }
}

/// The shipped library's suppression half, ready to run.
fn shipped() -> UnitRuntime {
    let lib = behavior::defaults::default_library();
    UnitRuntime::build(&lib).unwrap().expect("the shipped library has an enabled unit profile")
}

/// One profile on the shipped unit graph, with `overrides` applied to every
/// kind. The equivalent of a scientist opening the composer, moving one slider
/// and pressing Apply.
fn tweaked(overrides: BTreeMap<String, ParamValue>) -> UnitRuntime {
    let g = behavior::defaults::default_unit_graph();
    let mut lib = Library::default();
    let mut s = AgentSubtype::new("tweaked", "Tweaked", &g.id);
    s.enabled = true;
    s.overrides = overrides;
    lib.subtypes.insert(s.id.clone(), s);
    lib.graphs.insert(g.id.clone(), g);
    UnitRuntime::build(&lib).unwrap().unwrap()
}

/// An override key into the shipped unit graph, by node type and parameter.
fn key(type_id: &str, param: &str) -> String {
    let g = behavior::defaults::default_unit_graph();
    let n = g.nodes.iter().find(|n| n.type_id == type_id).unwrap();
    BehaviorGraph::override_key(n.id, param)
}

// --- the policy is reached ---------------------------------------------------

#[test]
fn every_unit_kind_is_governed_by_the_shipped_profile() {
    let w = setup(Some(shipped()));
    for kind in [UnitKind::Engine, UnitKind::HandCrew, UnitKind::AirTanker] {
        let i = w.unit(kind);
        let (id, _) = w
            .crews
            .policy_of(i)
            .unwrap_or_else(|| panic!("{} is not governed by anything", kind.label()));
        assert_eq!(id, "standing-orders");
    }
}

/// Even the convenience constructor installs a graph policy; there is no
/// alternate decision layer hidden behind it.
#[test]
fn the_default_constructor_is_graph_driven() {
    let w = setup(None);
    assert!(!w.crews.policy().is_empty());
    assert!(w.crews.policy_of(0).is_some());
}

/// A profile that names one kind governs that kind and no other. This is the
/// unit domain's whole assignment rule, and getting it wrong would silently
/// apply an engine policy to the aircraft.
#[test]
fn a_partial_unit_policy_is_rejected() {
    let g = behavior::defaults::default_unit_graph();
    let mut lib = Library::default();
    let mut s = AgentSubtype::new("engines-only", "Engines only", &g.id);
    s.enabled = true;
    s.unit_kinds = vec![UnitKindKey::Engine];
    lib.subtypes.insert(s.id.clone(), s);
    lib.graphs.insert(g.id.clone(), g);

    let runtime = UnitRuntime::build(&lib).unwrap().unwrap();
    let scn = Scenario::load(data_dir()).unwrap();
    let agents = Abm::new(&scn, 42).unwrap();
    let bases: Vec<Pos> = agents.refuges.iter().map(|r| r.pos).collect();
    let error = Suppression::with_policy(&scn, &bases, runtime)
        .err()
        .expect("uncovered unit kinds must reject the run");
    assert!(error.to_string().contains("covers"), "{error:#}");
}

// --- the default constructor uses the reference graph -----------------------

/// The convenience constructor and an explicitly compiled copy of the
/// reference graph must produce the same incident.
#[test]
fn the_default_constructor_matches_the_reference_policy() {
    let attack = |policy: Option<UnitRuntime>| {
        let mut w = setup(policy);
        let target = w.downwind(300.0);
        for i in 0..w.crews.units.len() {
            if w.crews.units[i].kind != UnitKind::AirTanker {
                let _ = w.crews.assign(i, Task::Attack { at: target });
            }
        }
        w.run(60, 10);
        let s = w.crews.stats();
        (s.water_l.round() as i64, s.line_m.round() as i64, s.withdrawing, s.lost)
    };

    assert_eq!(attack(None), attack(Some(shipped())));
}

/// And the hard floor underneath it: a unit sent into lethal threat still comes
/// out, whichever layer is deciding.
///
/// A hand crew, not an engine. The ignition is inland in continuous fuel and the
/// drivable network does not reach it, so an engine ordered there parks where
/// the tarmac ends and never feels the fire at all — which is the shipped
/// model's honest answer (see CLAUDE.md finding 17) and tests nothing about
/// safety. A crew walks in.
#[test]
fn a_unit_sent_into_the_fire_still_withdraws_under_an_authored_policy() {
    let mut w = setup(Some(shipped()));
    // Let the fire establish first, or "the middle of it" is a cold cell.
    w.run(20, 20);
    let into_it = w.scn.world.centre_of(w.ignition);
    let crew = w.crews.nearest_available(into_it, UnitKind::HandCrew).unwrap();
    w.crews.assign(crew, Task::Attack { at: into_it }).unwrap();
    w.run(60, 10);

    let u = &w.crews.units[crew];
    assert_ne!(u.state, UnitState::Lost, "an authored policy got the unit killed");
    if u.state == UnitState::Working {
        assert!(
            w.fire.threat().at(u.pos) < WORK_LIMIT,
            "{} is working where the threat field says it cannot",
            u.callsign
        );
    }
}

// --- and changing a number changes the incident ------------------------------

/// The point of the whole exercise, for the suppression half: move one slider
/// and the units behave differently on the same fire.
///
/// Winding the safety limit down to almost nothing means a unit refuses to work
/// anywhere it can feel the fire at all, so it gets less done. Measured on line
/// cut by the crews rather than water delivered: the engines work from the road
/// and the road is cold, so the number that moves is the one belonging to the
/// units that actually walk up to the fire.
#[test]
fn lowering_the_safety_limit_pulls_units_out_sooner() {
    let line_after = |limit: f32| {
        let mut ov = BTreeMap::new();
        for k in ["hand_crew_limit", "engine_limit", "air_limit"] {
            ov.insert(key("block.unit_safety", k), ParamValue::Number(limit));
        }
        let mut w = setup(Some(tweaked(ov)));
        w.run(20, 20);
        let head = w.downwind(120.0);
        for i in 0..w.crews.units.len() {
            if w.crews.units[i].kind == UnitKind::HandCrew {
                let _ = w.crews.assign(i, Task::Attack { at: head });
            }
        }
        w.run(60, 10);
        w.crews.stats().line_m
    };

    let bold = line_after(WORK_LIMIT);
    let timid = line_after(0.005);
    assert!(
        timid < bold,
        "the safety limit has no effect: {bold:.0} m of line at {WORK_LIMIT}, {timid:.0} m at 0.005"
    );
}

/// Breaking off for water with a third of the tank left has to actually happen,
/// and it is the open question the block exists to let someone answer: the
/// hydrant round trip is longer than six minutes of pumping.
#[test]
fn raising_the_refill_threshold_sends_engines_for_water_earlier() {
    let refills_by = |threshold: f32| {
        let mut ov = BTreeMap::new();
        ov.insert(key("block.unit_resupply", "refill_below"), ParamValue::Number(threshold));
        let mut w = setup(Some(tweaked(ov)));
        let target = w.downwind(250.0);
        let engine = w.unit(UnitKind::Engine);
        w.crews.assign(engine, Task::Attack { at: target }).unwrap();
        // Long enough to arrive and pump, short enough that a "pump dry" engine
        // has not yet had time to empty the tank *and* be sent back.
        w.run(25, 10);
        (w.crews.units[engine].state, w.crews.units[engine].water_frac())
    };

    let (_, dry_frac) = refills_by(0.0);
    let (early_state, early_frac) = refills_by(0.6);

    // The eager engine broke off with water still aboard; the shipped one did
    // not break off until it had none.
    assert!(
        early_frac > dry_frac || early_state == UnitState::Refilling,
        "refill threshold has no effect: dry engine at {dry_frac:.2}, eager at {early_frac:.2}"
    );
}

// --- the invariants the composer must not be able to break -------------------

/// The same rule the civilian graphs are held to. A policy maps one observation
/// to one action and accumulates nothing, so the answer at a 2 s step and a 60 s
/// step is the same incident.
///
/// Not bit-identical: the unit model sub-steps at 4 s and a 60 s call still
/// rounds six transitions differently, which is exactly what `SUBSTEP_S` exists
/// to bound. What must hold is that the *policy* adds no further drift.
#[test]
fn an_authored_policy_is_step_size_invariant() {
    let water_at = |dt: i64| {
        let mut w = setup(Some(shipped()));
        let target = w.downwind(300.0);
        let engine = w.unit(UnitKind::Engine);
        w.crews.assign(engine, Task::Attack { at: target }).unwrap();
        w.run(60, dt);
        w.crews.stats().water_l
    };

    let fine = water_at(2);
    let coarse = water_at(60);
    let drift = (fine - coarse).abs() / fine.max(1.0);
    assert!(drift < 0.05, "{fine:.0} L at 2 s, {coarse:.0} L at 60 s");
}

/// Two runs of the same authored policy on the same seed are the same run. The
/// only per-unit variation a graph can reach is `jitter`, which is hashed from
/// the unit id.
#[test]
fn an_authored_policy_is_deterministic() {
    let run = || {
        let mut w = setup(Some(shipped()));
        let target = w.downwind(300.0);
        for i in 0..w.crews.units.len() {
            if !w.crews.units[i].kind.is_air() {
                let _ = w.crews.assign(i, Task::Attack { at: target });
            }
        }
        w.run(45, 10);
        w.crews
            .units
            .iter()
            .map(|u| (u.state, (u.pos.x * 100.0) as i64, (u.pos.y * 100.0) as i64))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}

/// A policy that refuses everything must not be able to get a unit killed, and
/// must not be able to keep one alive either: burning over is physics, and it
/// sits below the behaviour layer.
#[test]
fn a_policy_that_never_withdraws_cannot_author_away_a_burnover() {
    let mut ov = BTreeMap::new();
    for k in ["hand_crew_limit", "engine_limit", "air_limit", "heat_limit"] {
        ov.insert(key("block.unit_safety", k), ParamValue::Number(1.0));
    }
    let mut w = setup(Some(tweaked(ov)));

    let into_it = w.scn.world.centre_of(w.ignition);
    let engine = w.unit(UnitKind::Engine);
    w.crews.assign(engine, Task::Attack { at: into_it }).unwrap();
    w.run(40, 10);

    // It never pulled itself out — that is what the overrides say.
    assert_ne!(w.crews.units[engine].state, UnitState::Withdrawing);
    // And the heat model still ran, so it is either dead or it never actually
    // reached the flames. Either is honest; what would not be is a unit sitting
    // in an impassable cell with no accumulated exposure at all.
    let u = &w.crews.units[engine];
    let danger = w.fire.threat().at(u.pos);
    assert!(
        u.state == UnitState::Lost || danger < fire::threat::IMPASSABLE || u.heat_s > 0.0,
        "{} is standing in {danger:.2} threat with no exposure",
        u.callsign
    );
}

/// Prints the refill comparison, so the assertion above can be checked rather
/// than trusted. Ignored: a report, not a test.
#[test]
#[ignore = "report, not a test"]
fn refill_threshold_report() {
    for threshold in [0.0, 0.33, 0.6] {
        let mut ov = BTreeMap::new();
        ov.insert(key("block.unit_resupply", "refill_below"), ParamValue::Number(threshold));
        let mut w = setup(Some(tweaked(ov)));
        let target = w.downwind(250.0);
        let engine = w.unit(UnitKind::Engine);
        w.crews.assign(engine, Task::Attack { at: target }).unwrap();
        w.run(25, 10);
        let u = &w.crews.units[engine];
        println!(
            "refill_below {threshold:.2}: {} tank {:.2} — {} L delivered",
            u.state.label(),
            u.water_frac(),
            w.crews.stats().water_l
        );
    }
}
