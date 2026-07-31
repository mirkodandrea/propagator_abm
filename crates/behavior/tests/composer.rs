//! What has to hold for an authored behaviour to be trustworthy.
//!
//! The interesting cases here are the negative ones. A composer that lets a
//! scientist build a graph which type-checks, runs, and quietly means
//! something other than what it looks like is worse than no composer, so most
//! of this file is about the editor *refusing* things.

use std::collections::BTreeMap;

use behavior::defaults::{default_graph, default_subtypes, DEFAULT_GRAPH_ID};
use behavior::eval::Overrides;
use behavior::graph::Wire;
use behavior::testbench::{self, SweepField};
use behavior::{
    registry, ActionKind, BehaviorGraph, Category, CompiledGraph, IntentValue, Observation,
    ParamValue, Severity,
};

// --- the registry ----------------------------------------------------------

#[test]
fn nodes_register_themselves() {
    let reg = registry();
    assert!(reg.len() > 30, "only {} nodes registered", reg.len());
    // Every category has something in it; an empty one means a whole section
    // of the palette silently vanished.
    for c in Category::ALL {
        assert!(reg.in_category(c).next().is_some(), "{c:?} is empty");
    }
    assert!(reg.get("out.decision").is_some());
}

#[test]
fn every_node_declares_itself_coherently() {
    for spec in registry().all() {
        assert!(!spec.name.is_empty(), "{} has no name", spec.id);
        assert!(!spec.doc.is_empty(), "{} has no doc", spec.id);
        assert!(spec.id.contains('.'), "{} is not namespaced", spec.id);
        match spec.category {
            // An observation with an input could read something other than the
            // observation, which is the one thing the sandbox rests on.
            Category::Observation | Category::Parameter => {
                assert!(spec.inputs.is_empty(), "{} is a source with inputs", spec.id);
                assert!(!spec.outputs.is_empty(), "{} produces nothing", spec.id);
            }
            _ => assert!(!spec.inputs.is_empty(), "{} consumes nothing", spec.id),
        }
        // Only the decision sink takes several wires; anywhere else it would
        // silently drop all but the first.
        for p in spec.inputs {
            assert!(
                !p.multi || spec.id == "out.decision",
                "{}.{} is multi-connection",
                spec.id,
                p.name
            );
        }
    }
}

#[test]
fn palette_search_finds_nodes_by_the_words_people_use() {
    let hits = |q: &str| registry().all().filter(|s| s.matches(q)).count();
    assert!(hits("smoke") > 0, "\"smoke\" finds nothing");
    assert!(hits("evacuate") > 0);
    assert!(hits("threshold") > 0);
    assert!(hits("zzzz") == 0);
}

// --- the shipped graph -----------------------------------------------------

#[test]
fn the_default_graph_validates() {
    let g = default_graph();
    let r = behavior::validate(&g);
    for i in &r.issues {
        assert_ne!(i.severity, Severity::Error, "{}", i.message);
    }
    assert_eq!(r.order.len(), g.nodes.len());
}

#[test]
fn the_default_graph_round_trips_through_json() {
    let g = default_graph();
    let back = BehaviorGraph::from_json(&g.to_json().unwrap()).unwrap();
    assert_eq!(g, back);
}

/// The behaviour has to produce the right answer in each of the situations the
/// bench ships, or the bench is decoration.
#[test]
fn the_default_behaviour_answers_the_shipped_situations() {
    let g = CompiledGraph::compile(&default_graph(), &Overrides::new()).unwrap();
    let expect = |name: &str, want: ActionKind| {
        let s = testbench::situations()
            .into_iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("no situation {name:?}"));
        let got = g.eval(&s.obs);
        assert_eq!(got.action, want, "{name}: got {:?}", got.action);
    };

    expect("Quiet", ActionKind::Stay);
    expect("Smoke on the ridge", ActionKind::Stay);
    // The order arrives and is believed.
    expect("Order given, fire distant", ActionKind::Prepare);
    // The same order, a household that does not act on official instructions.
    expect("Order given, low trust", ActionKind::Stay);
    expect("Fire at the fence", ActionKind::EvacuateNow);
    // Sheltering has to outbid running once the road is gone: the road is
    // where the casualties are.
    expect("Cut off", ActionKind::Shelter);
}

#[test]
fn a_household_that_planned_to_defend_does_not_leave_on_an_order() {
    let g = CompiledGraph::compile(&default_graph(), &Overrides::new()).unwrap();
    let ordered = testbench::situations()
        .into_iter()
        .find(|s| s.name == "Order given, fire distant")
        .unwrap()
        .obs;

    assert_eq!(g.eval(&ordered).action, ActionKind::Prepare);
    let defending = Observation { intent: IntentValue::StayDefend, ..ordered };
    assert_eq!(g.eval(&defending).action, ActionKind::Defend);
}

#[test]
fn evaluation_is_pure() {
    let g = CompiledGraph::compile(&default_graph(), &Overrides::new()).unwrap();
    let obs = testbench::situations()[4].obs;
    let first = g.eval(&obs);
    for _ in 0..50 {
        assert_eq!(g.eval(&obs), first);
    }
}

// --- subtypes --------------------------------------------------------------

#[test]
fn subtypes_share_one_graph_and_differ_only_in_numbers() {
    let lib = behavior::defaults::default_library();
    assert!(lib.subtypes.len() >= 4);
    for s in lib.subtypes.values() {
        assert_eq!(s.graph, DEFAULT_GRAPH_ID, "{} has its own graph", s.id);
        lib.compile(&s.id).unwrap_or_else(|e| panic!("{}: {e}", s.id));
    }
}

#[test]
fn an_override_actually_changes_the_answer() {
    let g = default_graph();
    // The alarm threshold a wait-and-see household has to pass.
    let node = g.nodes.iter().find(|n| n.type_id == "intent.weight").unwrap();
    let key = BehaviorGraph::override_key(node.id, "wait_and_see");

    let smoke = testbench::situations()
        .into_iter()
        .find(|s| s.name == "Smoke on the ridge")
        .unwrap()
        .obs;

    let stock = CompiledGraph::compile(&g, &Overrides::new()).unwrap();
    assert_eq!(stock.eval(&smoke).action, ActionKind::Stay);

    // Drop the threshold under the alarm this household already feels.
    let mut ov: Overrides = BTreeMap::new();
    ov.insert(key, ParamValue::Number(0.05));
    let jumpy = CompiledGraph::compile(&g, &ov).unwrap();
    assert_eq!(jumpy.eval(&smoke).action, ActionKind::Prepare);
}

#[test]
fn comparing_two_subtypes_names_what_differs() {
    let subs = default_subtypes();
    let g = default_graph();
    let a = subs.iter().find(|s| s.id == "prepared-resident").unwrap();
    let b = subs.iter().find(|s| s.id == "committed-defender").unwrap();
    let diff = behavior::subtype::compare(a, b, Some(&g));
    assert!(!diff.is_empty());
    // Override keys are rendered as something a person wrote, not "17.value".
    assert!(
        diff.iter().any(|d| d.what.contains("Defensible space") || d.what.contains("\"")),
        "{diff:#?}"
    );
    // A subtype compared with itself has nothing to say.
    assert!(behavior::subtype::compare(a, a, Some(&g)).is_empty());
}

#[test]
fn pruning_drops_overrides_whose_node_is_gone() {
    let mut g = default_graph();
    let mut s = default_subtypes().into_iter().find(|s| s.id == "committed-defender").unwrap();
    let before = s.overrides.len();
    assert!(before > 0);

    let victim: behavior::NodeId = behavior::subtype::split_key(s.overrides.keys().next().unwrap())
        .unwrap()
        .0;
    g.remove(victim);

    let dead = s.prune(&g);
    assert_eq!(dead.len(), before - s.overrides.len());
    assert!(!dead.is_empty());
}

// --- what the validator has to refuse --------------------------------------

fn first_error(g: &BehaviorGraph) -> String {
    behavior::validate(g)
        .errors()
        .next()
        .map(|i| i.message.clone())
        .unwrap_or_else(|| "no error reported".into())
}

#[test]
fn a_graph_with_no_decision_output_is_refused() {
    let mut g = BehaviorGraph::new("empty", "Empty");
    g.add("obs.cue", [0.0, 0.0]);
    assert!(first_error(&g).contains("Decision"));
}

#[test]
fn two_decision_outputs_are_refused() {
    let mut g = default_graph();
    g.add("out.decision", [0.0, 0.0]);
    assert!(first_error(&g).contains("exactly one"));
}

#[test]
fn a_type_mismatch_cannot_be_connected() {
    let mut g = BehaviorGraph::new("mismatch", "Mismatch");
    let cue = g.add("obs.cue", [0.0, 0.0]).unwrap();
    let and = g.add("logic.and", [1.0, 0.0]).unwrap();
    // A number into a bool port: refused at the point of connection, so the
    // editor never draws a wire it will later report as broken.
    assert!(!g.connect(Wire { from_node: cue, from_port: 0, to_node: and, to_port: 0 }));
    assert!(g.wires.is_empty());
}

#[test]
fn a_type_mismatch_smuggled_past_connect_is_still_reported() {
    let mut g = default_graph();
    let cue = g.nodes.iter().find(|n| n.type_id == "obs.cue").unwrap().id;
    let and = g.nodes.iter().find(|n| n.type_id == "logic.and").unwrap().id;
    // What a hand-edited file looks like.
    g.wires.push(Wire { from_node: cue, from_port: 0, to_node: and, to_port: 0 });
    assert!(first_error(&g).contains("takes a bool"));
}

#[test]
fn a_cycle_names_the_nodes_in_it() {
    let mut g = default_graph();
    let a = g.add("math.add", [0.0, 0.0]).unwrap();
    let b = g.add("math.add", [0.0, 0.0]).unwrap();
    g.wires.push(Wire { from_node: a, from_port: 0, to_node: b, to_port: 0 });
    g.wires.push(Wire { from_node: b, from_port: 0, to_node: a, to_port: 0 });

    let r = behavior::validate(&g);
    assert!(!r.ok());
    assert!(r.issues.iter().any(|i| i.message.contains("feeds back")));
    assert!(r.for_node(a).next().is_some(), "the cycle is not attributed to a node");
    assert!(r.order.is_empty());
}

#[test]
fn an_unknown_node_type_loads_and_then_fails_validation() {
    let json = r#"{
        "id": "future", "name": "From a later build",
        "nodes": [{"id": 0, "type": "obs.wind_at_house"}],
        "wires": []
    }"#;
    // Loading has to succeed, or a scientist who opens a colleague's file gets
    // a parse error instead of a list of what their build is missing.
    let g = BehaviorGraph::from_json(json).unwrap();
    assert!(first_error(&g).contains("obs.wind_at_house"));
}

#[test]
fn a_required_input_left_dangling_is_reported() {
    let mut g = default_graph();
    // "Intent is" has no default for its intent input: there is no sensible
    // plan to invent for a household.
    let n = g.add("intent.is", [0.0, 0.0]).unwrap();
    let r = behavior::validate(&g);
    assert!(r.for_node(n).any(|i| i.message.contains("needs a connection")), "{r:#?}");
}

#[test]
fn a_second_wire_into_a_single_input_is_reported() {
    let mut g = default_graph();
    let cue = g.nodes.iter().find(|n| n.type_id == "obs.cue").unwrap().id;
    let above = g.add("cmp.above", [0.0, 0.0]).unwrap();
    g.wires.push(Wire { from_node: cue, from_port: 0, to_node: above, to_port: 0 });
    let threat = g.nodes.iter().find(|n| n.type_id == "obs.threat").unwrap().id;
    g.wires.push(Wire { from_node: threat, from_port: 0, to_node: above, to_port: 0 });
    assert!(first_error(&g).contains("which takes one"));
}

#[test]
fn a_choice_parameter_cannot_be_set_to_something_that_does_not_exist() {
    let mut g = default_graph();
    let n = g.nodes.iter().find(|n| n.type_id == "intent.is").unwrap().id;
    g.node_mut(n)
        .unwrap()
        .params
        .insert("plan".into(), ParamValue::Choice("flee_to_the_sea".into()));
    assert!(first_error(&g).contains("flee_to_the_sea"));
}

#[test]
fn an_orphan_node_is_a_warning_not_an_error() {
    let mut g = default_graph();
    g.add("obs.household_size", [0.0, 0.0]);
    let r = behavior::validate(&g);
    assert!(r.ok(), "an unwired node should not stop the graph running");
    assert!(r.warnings().any(|w| w.message.contains("not wired")));
}

// --- the bench -------------------------------------------------------------

#[test]
fn a_trace_records_every_node_and_ranks_the_proposals() {
    let g = CompiledGraph::compile(&default_graph(), &Overrides::new()).unwrap();
    let obs = testbench::situations().into_iter().find(|s| s.name == "Cut off").unwrap().obs;
    let (d, trace) = g.eval_traced(&obs);

    assert_eq!(trace.nodes.len(), g.node_count());
    assert_eq!(d.action, ActionKind::Shelter);
    // Several branches fire at once here; the strongest is the answer.
    assert!(trace.proposals.len() > 1, "{:?}", trace.proposals);
    assert_eq!(trace.proposals[0].1, ActionKind::Shelter);
    assert!(trace.proposals.windows(2).all(|w| w[0].2 >= w[1].2));
}

#[test]
fn a_sweep_finds_the_threshold_it_was_pointed_at() {
    let g = CompiledGraph::compile(&default_graph(), &Overrides::new()).unwrap();
    let base = Observation { warning_received: false, order_issued: false, ..Observation::default() };
    let pts = testbench::sweep(&g, &base, SweepField::Cue, 101);
    let changes = testbench::transitions(&pts);
    assert!(!changes.is_empty(), "alarm never changes the answer");
    let (at, from, to) = changes[0];
    assert_eq!(from, ActionKind::Stay);
    assert_eq!(to, ActionKind::Prepare);
    // The default wait-and-see threshold, with a mid jitter and mid risk.
    assert!((0.1..0.35).contains(&at), "departure at alarm {at}");
}

#[test]
fn subtypes_can_be_compared_on_one_situation() {
    let lib = behavior::defaults::default_library();
    let compiled: Vec<_> =
        lib.subtypes.keys().map(|id| (id.clone(), lib.compile(id).unwrap())).collect();
    let refs: Vec<(String, &CompiledGraph)> =
        compiled.iter().map(|(id, g)| (id.clone(), g)).collect();

    let obs = testbench::situations()
        .into_iter()
        .find(|s| s.name == "Smoke on the ridge")
        .unwrap()
        .obs;
    let answers = testbench::compare_subtypes(&refs, &obs);
    assert_eq!(answers.len(), refs.len());
    // The point of having profiles at all: on the same cue they do not all
    // agree. If they did, the subtypes would be decoration.
    let distinct: std::collections::BTreeSet<_> =
        answers.iter().map(|a| a.decision.action).collect();
    assert!(distinct.len() > 1, "every subtype answered the same: {answers:#?}");
}

// --- the library -----------------------------------------------------------

#[test]
fn the_library_round_trips_through_a_directory() {
    let dir = std::env::temp_dir().join(format!("behaviour-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let lib = behavior::defaults::default_library();
    lib.save_dir(&dir).unwrap();

    let back = behavior::Library::load_dir(&dir).unwrap();
    assert_eq!(back.graphs, lib.graphs);
    assert_eq!(back.subtypes, lib.subtypes);

    // Shares are normalised, not taken literally: a scientist typing 3 and 1
    // means three to one, not 400% of the population.
    let a = back.assignment();
    let total: f32 = a.iter().map(|(_, s)| s).sum();
    assert!((total - 1.0).abs() < 1e-5, "shares sum to {total}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_missing_directory_falls_back_to_the_shipped_library() {
    let lib = behavior::Library::load_or_default(std::path::Path::new("/nonexistent/behaviours"));
    assert!(lib.graphs.contains_key(DEFAULT_GRAPH_ID));
}
/// Regenerate `data/behaviours/` from the built-in defaults.
///
/// Ignored, because it writes to the repository. Run it after changing
/// `defaults.rs`:
///
/// ```text
/// cargo test -p behavior --release -- --ignored write_shipped_library
/// ```
#[test]
#[ignore]
fn write_shipped_library() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/behaviours");
    behavior::defaults::default_library().save_dir(&root).unwrap();
    println!("wrote {}", root.display());
}
