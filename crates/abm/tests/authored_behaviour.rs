//! Does an authored behaviour actually drive the model, and does it keep the
//! guarantees the hand-written one has?
//!
//! The composer is only worth having if a graph a scientist edits changes the
//! outcome of a real incident — and only worth *trusting* if it cannot break
//! step-size invariance or reproducibility on the way. Both are checked here
//! against the shipped scenario rather than against a toy.

use std::collections::BTreeMap;

use abm::{Abm, BehaviorRuntime};
use behavior::{ActionKind, BehaviorGraph, Library, ParamValue};
use fire::{FireSim, Weather};
use scenario::population::Status;
use scenario::Scenario;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .unwrap()
}

fn fire_for(scn: &Scenario) -> FireSim {
    let weather = Weather::default();
    let plan = fire::plan_ignition(scn, weather.wind_dir_deg, 250.0);
    let mut fire = FireSim::new(scn, weather, 42).unwrap();
    fire.ignite_patch(plan.centre, plan.radius_m, scn).unwrap();
    fire
}

fn run(scn: &Scenario, fire: &mut FireSim, agents: &mut Abm, minutes: i64, dt: i64) {
    for _ in 0..(minutes * 60 / dt) {
        fire.advance(dt).unwrap();
        agents.step(dt as f32, fire, scn);
    }
}

/// A library with one subtype on the shipped graph, so a test can change one
/// number and see what it does.
fn one_subtype(overrides: BTreeMap<String, ParamValue>) -> Library {
    let mut lib = Library::default();
    let g = behavior::defaults::default_graph();
    let mut s = behavior::AgentSubtype::new("test", "Test profile", &g.id);
    s.overrides = overrides;
    s.share = 1.0;
    lib.graphs.insert(g.id.clone(), g);
    lib.subtypes.insert(s.id.clone(), s);
    lib
}

fn runtime(lib: &Library) -> BehaviorRuntime {
    BehaviorRuntime::build(lib).unwrap().expect("a subtype with a share")
}

#[test]
fn the_shipped_library_runs_a_real_incident() {
    let scn = Scenario::load(data_dir()).unwrap();
    let lib = behavior::defaults::default_library();
    let mut fire = fire_for(&scn);
    let mut agents = Abm::with_behavior(&scn, 42, Some(runtime(&lib))).unwrap();

    agents.order_evacuation_all();
    run(&scn, &mut fire, &mut agents, 90, 10);

    let s = agents.stats();
    assert!(s.safe > 100, "authored behaviour evacuated only {}", s.safe);
    // Every household is accounted for in one of the states the movement
    // layer knows: an authored graph must not be able to strand people in a
    // state nothing advances.
    let stuck = agents
        .households
        .iter()
        .filter(|h| h.status == Status::Normal && h.cue > 0.5)
        .count();
    assert_eq!(stuck, 0, "{stuck} alarmed households never reacted at all");
}

/// The property that everything else in the model rests on, checked through
/// the composer: the same incident stepped at 2 s and 60 s must evacuate the
/// same households.
#[test]
fn authored_behaviour_is_step_size_invariant() {
    let scn = Scenario::load(data_dir()).unwrap();
    let lib = behavior::defaults::default_library();

    let departed = |dt: i64| {
        let mut fire = fire_for(&scn);
        let mut agents = Abm::with_behavior(&scn, 42, Some(runtime(&lib))).unwrap();
        agents.order_evacuation_all();
        run(&scn, &mut fire, &mut agents, 60, dt);
        agents
            .households
            .iter()
            .filter(|h| !matches!(h.status, Status::Normal | Status::Warned))
            .count()
    };

    let fine = departed(2);
    let coarse = departed(60);
    // Not bit-identical: the fire itself is advanced on a different schedule,
    // so the two runs see slightly different fronts. The decision layer must
    // not add to that.
    let drift = (fine as f32 - coarse as f32).abs() / fine.max(1) as f32;
    assert!(drift < 0.06, "{fine} departed at 2 s, {coarse} at 60 s");
}

#[test]
fn subtype_assignment_survives_a_rebuild() {
    let scn = Scenario::load(data_dir()).unwrap();
    let lib = behavior::defaults::default_library();

    let profile_of = || {
        let agents = Abm::with_behavior(&scn, 42, Some(runtime(&lib))).unwrap();
        (0..agents.households.len())
            .map(|i| agents.behaviour_of(i).map(|(id, _, _)| id.to_string()))
            .collect::<Vec<_>>()
    };
    assert_eq!(profile_of(), profile_of());

    // And it is a real spread, not everyone in one bucket.
    let counts: BTreeMap<_, usize> = profile_of().into_iter().flatten().fold(
        BTreeMap::new(),
        |mut m, id| {
            *m.entry(id).or_default() += 1;
            m
        },
    );
    assert_eq!(counts.len(), lib.subtypes.len(), "{counts:?}");
}

/// The point of the whole exercise: change one number in the editor and the
/// incident comes out differently.
#[test]
fn lowering_the_alarm_threshold_gets_more_people_out_sooner() {
    let scn = Scenario::load(data_dir()).unwrap();
    let g = behavior::defaults::default_graph();
    let node = g.nodes.iter().find(|n| n.type_id == "intent.weight").unwrap();

    let safe_after = |threshold: f32| {
        let mut ov = BTreeMap::new();
        for plan in ["leave_early", "wait_and_see", "stay_defend"] {
            ov.insert(BehaviorGraph::override_key(node.id, plan), ParamValue::Number(threshold));
        }
        let lib = one_subtype(ov);
        let mut fire = fire_for(&scn);
        let mut agents = Abm::with_behavior(&scn, 42, Some(runtime(&lib))).unwrap();
        // No order at all: this isolates the household's own reading of the
        // fire, which is what the threshold governs.
        run(&scn, &mut fire, &mut agents, 90, 10);
        agents.stats().safe
    };

    let jumpy = safe_after(0.05);
    let stoic = safe_after(0.95);
    assert!(
        jumpy > stoic,
        "threshold has no effect: {jumpy} safe at 0.05, {stoic} at 0.95"
    );
}

#[test]
fn a_graph_that_never_fires_leaves_everyone_at_home() {
    let scn = Scenario::load(data_dir()).unwrap();
    // A behaviour whose only sink is the decision node, with nothing wired
    // into it. It validates — an empty decision is a legitimate thing to
    // author, temporarily — and it must simply do nothing rather than
    // misbehave.
    let mut g = BehaviorGraph::new("inert", "Inert");
    g.add("out.decision", [0.0, 0.0]);
    assert!(behavior::validate(&g).ok());

    let mut lib = Library::default();
    let mut s = behavior::AgentSubtype::new("inert", "Inert", "inert");
    s.share = 1.0;
    lib.graphs.insert("inert".into(), g);
    lib.subtypes.insert("inert".into(), s);

    let mut fire = fire_for(&scn);
    let mut agents = Abm::with_behavior(&scn, 42, Some(runtime(&lib))).unwrap();
    agents.order_evacuation_all();
    run(&scn, &mut fire, &mut agents, 60, 20);

    // Only the people who were already out when it started are moving.
    let departed = agents
        .households
        .iter()
        .filter(|h| matches!(h.status, Status::Preparing | Status::Evacuating | Status::Evacuated))
        .count();
    assert_eq!(departed, 0, "{departed} households left on an empty behaviour");
}

#[test]
fn the_inspector_can_explain_one_household() {
    let scn = Scenario::load(data_dir()).unwrap();
    let lib = behavior::defaults::default_library();
    let mut fire = fire_for(&scn);
    let mut agents = Abm::with_behavior(&scn, 42, Some(runtime(&lib))).unwrap();
    agents.order_evacuation_all();
    run(&scn, &mut fire, &mut agents, 45, 20);

    let (id, name, decision) = agents.behaviour_of(0).expect("a subtype");
    assert!(!id.is_empty() && !name.is_empty());

    let (again, trace) = agents.explain(0, &fire).expect("a trace");
    assert_eq!(again.action, decision.action, "the trace disagrees with the run");
    assert!(!trace.nodes.is_empty());
    // Every node in the graph reports a value, which is what makes the panel
    // an explanation rather than a summary.
    assert!(trace.nodes.iter().all(|n| !n.outputs.is_empty()));
}

#[test]
fn no_behaviour_loaded_leaves_the_shipped_model_alone() {
    let scn = Scenario::load(data_dir()).unwrap();

    let safe = |behaviour: bool| {
        let rt = behaviour.then(|| runtime(&behavior::defaults::default_library()));
        let mut fire = fire_for(&scn);
        let mut agents = Abm::with_behavior(&scn, 42, rt).unwrap();
        agents.order_evacuation_all();
        run(&scn, &mut fire, &mut agents, 60, 20);
        agents.stats().safe
    };

    // Not asserting they match — they are different models, and that is the
    // point. Asserting the hand-written path is still the one that runs when
    // nothing is authored.
    let plain = safe(false);
    assert!(plain > 100, "the shipped model stopped working: {plain} safe");
}

#[test]
fn a_subtype_trait_reaches_the_households() {
    let scn = Scenario::load(data_dir()).unwrap();
    let mut lib = one_subtype(BTreeMap::new());
    lib.subtypes
        .get_mut("test")
        .unwrap()
        .traits
        .insert(behavior::TraitKey::PrepTimeMin, 3.0);

    let agents = Abm::with_behavior(&scn, 42, Some(runtime(&lib))).unwrap();
    assert!(
        agents.households.iter().all(|h| (h.prep_time_min - 3.0).abs() < 1e-6),
        "the subtype's preparation time did not reach the households"
    );
}

#[test]
fn a_capability_reaches_the_households() {
    let scn = Scenario::load(data_dir()).unwrap();
    let mut lib = one_subtype(BTreeMap::new());
    lib.subtypes
        .get_mut("test")
        .unwrap()
        .capabilities
        .insert(behavior::Capability::Vehicle, false);

    let agents = Abm::with_behavior(&scn, 42, Some(runtime(&lib))).unwrap();
    assert!(agents.households.iter().all(|h| h.vehicles == 0));
}

/// A behaviour that only ever shelters must not evacuate anyone, however loud
/// the fire gets. This is the check that the action mapping in
/// `abm::behaviour` is wired to the movement layer the way it claims.
#[test]
fn shelter_is_not_a_departure() {
    let scn = Scenario::load(data_dir()).unwrap();
    let mut g = BehaviorGraph::new("shelter-always", "Always shelter");
    let on = g.add("param.bool", [0.0, 0.0]).unwrap();
    let act = g.add("action.shelter", [200.0, 0.0]).unwrap();
    let out = g.add("out.decision", [400.0, 0.0]).unwrap();
    assert!(g.connect(behavior::Wire { from_node: on, from_port: 0, to_node: act, to_port: 0 }));
    assert!(g.connect(behavior::Wire { from_node: act, from_port: 0, to_node: out, to_port: 0 }));

    let mut lib = Library::default();
    let mut s = behavior::AgentSubtype::new("shelter", "Shelter", "shelter-always");
    s.share = 1.0;
    lib.graphs.insert("shelter-always".into(), g);
    lib.subtypes.insert("shelter".into(), s);

    let mut fire = fire_for(&scn);
    let mut agents = Abm::with_behavior(&scn, 42, Some(runtime(&lib))).unwrap();
    agents.order_evacuation_all();
    run(&scn, &mut fire, &mut agents, 60, 20);

    assert_eq!(
        agents.behaviour_of(0).unwrap().2.action,
        ActionKind::Shelter,
        "the graph is not producing the action it was built to"
    );
    let left = agents.households.iter().filter(|h| h.traveller.is_some()).count();
    assert_eq!(left, 0, "{left} households evacuated on a shelter-only behaviour");
}
