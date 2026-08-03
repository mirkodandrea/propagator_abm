//! What has to hold for an authored behaviour to be trustworthy.
//!
//! The interesting cases here are the negative ones. A composer that lets a
//! scientist build a graph which type-checks, runs, and quietly means
//! something other than what it looks like is worse than no composer, so most
//! of this file is about the editor *refusing* things.

use std::collections::BTreeMap;

use behavior::defaults::{
    default_graph, default_person_graph, default_person_subtypes, default_subtypes,
    default_unit_graph, default_unit_subtypes, DEFAULT_GRAPH_ID, DEFAULT_PERSON_GRAPH_ID,
    DEFAULT_UNIT_GRAPH_ID,
};
use behavior::eval::Overrides;
use behavior::graph::Wire;
use behavior::testbench::{self, SweepField};
use behavior::{
    registry, ActionKind, BehaviorGraph, Category, CompiledGraph, Domain, HouseholdObs,
    IntentValue, Observation, ParamValue, PersonObs, Severity, UnitObs,
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
    assert!(reg.get("out.unit_decision").is_some());
}

#[test]
fn every_node_declares_itself_coherently() {
    for spec in registry().all() {
        assert!(!spec.name.is_empty(), "{} has no name", spec.id);
        assert!(!spec.doc.is_empty(), "{} has no doc", spec.id);
        assert!(spec.id.contains('.'), "{} is not namespaced", spec.id);
        match spec.category {
            // An observation with an input could read something other than the
            // observation, which is the one thing the sandbox rests on. A block
            // reads the observation too, and mostly has no inputs for the same
            // reason — but it is allowed them, because some genuinely need to
            // be told something downstream of themselves.
            Category::Observation | Category::Parameter => {
                assert!(spec.inputs.is_empty(), "{} is a source with inputs", spec.id);
                assert!(!spec.outputs.is_empty(), "{} produces nothing", spec.id);
            }
            Category::Block => {
                assert!(!spec.outputs.is_empty(), "{} produces nothing", spec.id);
                assert!(!spec.params.is_empty(), "{} is a block with no knobs", spec.id);
            }
            _ => assert!(!spec.inputs.is_empty(), "{} consumes nothing", spec.id),
        }
        // Multi-connection inputs are for combining an unknown number of
        // things. Anywhere else they would silently drop all but the first.
        for p in spec.inputs {
            let combiner = matches!(
                spec.id,
                "out.decision"
                    | "out.unit_decision"
                    | "out.person_decision"
                    | "logic.any"
                    | "logic.all"
            );
            assert!(!p.multi || combiner, "{}.{} is multi-connection", spec.id, p.name);
        }
    }
}

/// The invariant that makes the domain split load-bearing rather than
/// decorative: anything that reads the world has to say whose world it is.
///
/// A block or an observation without a domain would read a default observation
/// in whichever graph it was dropped into — every field zero, every condition
/// false — and behave exactly like a correctly-wired node whose branch never
/// fires. That is the worst failure mode this crate has, and this is the only
/// place it can be caught.
#[test]
fn anything_that_reads_the_world_declares_a_domain() {
    for spec in registry().all() {
        if matches!(spec.category, Category::Observation | Category::Block | Category::Action) {
            assert!(
                spec.domain.is_some(),
                "{} reads or writes an agent's world without declaring a domain",
                spec.id
            );
        }
    }
}

#[test]
fn every_domain_has_exactly_one_decision_sink_of_its_own() {
    for d in Domain::ALL {
        let spec = registry()
            .get(d.decision_output())
            .unwrap_or_else(|| panic!("{d:?} names a sink that is not registered"));
        assert_eq!(spec.domain, Some(d), "{}'s sink belongs to another domain", spec.id);
        assert_eq!(spec.category, Category::Output);
    }
    // And the two are not the same node.
    assert_ne!(
        Domain::Household.decision_output(),
        Domain::SuppressionUnit.decision_output()
    );
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
    let defending =
        Observation::from(HouseholdObs { intent: IntentValue::StayDefend, ..ordered.household() });
    assert_eq!(g.eval(&defending).action, ActionKind::Defend);
}

/// The graph is written in blocks, and that is the point of this round of work:
/// the same behaviour used to take thirty-one nodes.
///
/// Pinned rather than left as a comment because the whole complaint the blocks
/// answer was "the detail is too fine grained", and a graph that drifts back up
/// to thirty nodes has quietly undone it.
///
/// It is pinned as a *shape* rather than as a count, and that distinction had to
/// be made the first time the graph legitimately grew: adding the four blocks
/// the real-incident scenarios asked for took it from thirteen nodes to twenty,
/// and a bare cap would have read that as the regression it is the opposite of.
/// What matters is that every box is either a named assumption, a proposal, a
/// sink, or the one fan-in that collects the proposals — never a step of
/// arithmetic somebody has to reconstruct the meaning of.
#[test]
fn the_default_graph_is_written_in_blocks() {
    let g = default_graph();
    let blocks = g
        .nodes
        .iter()
        .filter(|n| n.spec().map(|s| s.category) == Some(Category::Block))
        .count();
    assert!(blocks >= 8, "only {blocks} of the graph is blocks");

    for n in &g.nodes {
        let spec = n.spec().expect("every shipped node is registered");
        let structural = matches!(
            spec.category,
            Category::Block | Category::Action | Category::Output
        ) || n.type_id == "logic.any";
        assert!(
            structural,
            "{} is neither an assumption, a proposal, a sink nor the fan-in: \
             the shipped behaviour is being written in pieces again",
            n.type_id
        );
    }
    // No bare arithmetic left: every number in the shipped behaviour is a
    // parameter on a named assumption, which is what a subtype overrides.
    assert!(
        !g.nodes.iter().any(|n| n.type_id.starts_with("math.")),
        "the shipped behaviour is doing arithmetic on the canvas again"
    );
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
        // One graph per domain, not one graph per profile. Variation without
        // duplication is the whole pattern.
        assert!(
            s.graph == DEFAULT_GRAPH_ID
                || s.graph == DEFAULT_UNIT_GRAPH_ID
                || s.graph == DEFAULT_PERSON_GRAPH_ID,
            "{} has its own graph",
            s.id
        );
        lib.compile(&s.id).unwrap_or_else(|e| panic!("{}: {e}", s.id));
    }
}

#[test]
fn an_override_actually_changes_the_answer() {
    let g = default_graph();
    // The alarm threshold a wait-and-see household has to pass.
    let node = g.nodes.iter().find(|n| n.type_id == "block.alarm").unwrap();
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
    let cue = g.add("obs.cue", [0.0, 0.0]).unwrap();
    let and = g.add("logic.and", [0.0, 0.0]).unwrap();
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
    let cue = g.add("obs.cue", [0.0, 0.0]).unwrap();
    let threat = g.add("obs.threat", [0.0, 0.0]).unwrap();
    let above = g.add("cmp.above", [0.0, 0.0]).unwrap();
    g.wires.push(Wire { from_node: cue, from_port: 0, to_node: above, to_port: 0 });
    g.wires.push(Wire { from_node: threat, from_port: 0, to_node: above, to_port: 0 });
    assert!(first_error(&g).contains("which takes one"));
}

/// The same rule, on the port that is *meant* to take several.
#[test]
fn a_combining_input_takes_as_many_wires_as_it_is_given() {
    let mut g = default_graph();
    let any = g.add("logic.any", [0.0, 0.0]).unwrap();
    let alight = g.add("obs.structure_alight", [0.0, 0.0]).unwrap();
    let blocked = g.add("obs.route_blocked", [0.0, 0.0]).unwrap();
    assert!(g.connect(Wire { from_node: alight, from_port: 0, to_node: any, to_port: 0 }));
    assert!(g.connect(Wire { from_node: blocked, from_port: 0, to_node: any, to_port: 0 }));
    assert!(behavior::validate(&g).ok());
}

#[test]
fn a_choice_parameter_cannot_be_set_to_something_that_does_not_exist() {
    let mut g = default_graph();
    let n = g.add("intent.is", [0.0, 0.0]).unwrap();
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
    let base = Observation::from(HouseholdObs {
        warning_received: false,
        order_issued: false,
        ..HouseholdObs::default()
    });
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
    let compiled: Vec<_> = lib
        .subtypes
        .values()
        .filter(|s| lib.domain_of(s) == Some(Domain::Household))
        .map(|s| (s.id.clone(), lib.compile(&s.id).unwrap()))
        .collect();
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

// --- domains ---------------------------------------------------------------

#[test]
fn a_graph_carries_its_domain_through_json() {
    let g = default_unit_graph();
    assert_eq!(g.domain, Domain::SuppressionUnit);
    let back = BehaviorGraph::from_json(&g.to_json().unwrap()).unwrap();
    assert_eq!(g, back);
}

/// Every graph written before domains existed is a household behaviour, and has
/// to keep loading as one.
#[test]
fn a_graph_file_without_a_domain_loads_as_a_household() {
    let json = r#"{
        "id": "old", "name": "From before domains",
        "nodes": [{"id": 0, "type": "out.decision"}],
        "wires": []
    }"#;
    let g = BehaviorGraph::from_json(json).unwrap();
    assert_eq!(g.domain, Domain::Household);
    assert!(behavior::validate(&g).ok());
}

#[test]
fn a_node_from_the_other_domain_cannot_be_placed() {
    let mut units = default_unit_graph();
    assert!(units.add("obs.cue", [0.0, 0.0]).is_none(), "a household cue went into a unit policy");
    let mut people = default_graph();
    assert!(people.add("unit.water_fraction", [0.0, 0.0]).is_none());
    // The domain-free nodes go in either.
    assert!(units.add("math.add", [0.0, 0.0]).is_some());
    assert!(people.add("math.add", [0.0, 0.0]).is_some());
}

/// The same rule for a file someone hand-edited, or a graph whose domain was
/// changed after it was built.
#[test]
fn a_node_from_the_other_domain_smuggled_into_a_file_is_reported() {
    let mut g = default_graph();
    g.domain = Domain::SuppressionUnit;
    let msg = first_error(&g);
    assert!(msg.contains("households") && msg.contains("suppression"), "{msg}");
}

/// The household sink is not a unit sink, so swapping the domain over cannot
/// leave a graph that quietly reads the wrong answer out.
#[test]
fn each_domain_wants_its_own_decision_sink() {
    let mut g = BehaviorGraph::new_in(Domain::SuppressionUnit, "u", "U");
    g.add("out.decision", [0.0, 0.0]);
    assert!(!behavior::validate(&g).ok());
    assert!(g.add("out.unit_decision", [0.0, 0.0]).is_some());
}

// --- the shipped unit policy -----------------------------------------------

#[test]
fn the_default_unit_graph_validates() {
    let g = default_unit_graph();
    let r = behavior::validate(&g);
    for i in &r.issues {
        assert_ne!(i.severity, Severity::Error, "{}", i.message);
    }
    assert_eq!(r.order.len(), g.nodes.len());
}

/// The shipped policy has to reproduce the hand-written one exactly, or turning
/// the composer on silently changes what every suppression measurement means.
#[test]
fn the_default_unit_policy_answers_the_shipped_situations() {
    let g = CompiledGraph::compile(&default_unit_graph(), &Overrides::new()).unwrap();
    let expect = |name: &str, want: ActionKind| {
        let s = testbench::unit_situations()
            .into_iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("no situation {name:?}"));
        let got = g.eval(&s.obs);
        assert_eq!(got.action, want, "{name}: got {:?}", got.action);
    };

    // A unit with nothing to say about its own situation gets on with the
    // order, which is the common case and is why this graph is five nodes.
    expect("Staged", ActionKind::Continue);
    expect("Responding", ActionKind::Continue);
    expect("Working the fuel", ActionKind::Continue);
    // The two things the hand-written policy did.
    expect("Tank dry", ActionKind::Refill);
    expect("Past the safety limit", ActionKind::Withdraw);
    // And the two it did not: the accumulated-heat trigger is off by default,
    // and an aircraft is not a ground unit.
    expect("Taking heat", ActionKind::Continue);
    expect("Tanker over the front", ActionKind::Continue);
}

/// Safety has to outbid resupply. A unit that is both dry and standing
/// somewhere lethal drives *out*, not to the hydrant.
#[test]
fn withdrawing_outbids_going_for_water() {
    let g = CompiledGraph::compile(&default_unit_graph(), &Overrides::new()).unwrap();
    let obs = Observation::from(UnitObs {
        kind: behavior::UnitKindKey::Engine,
        carries_water: true,
        water_fraction: 0.0,
        is_working: true,
        threat_here: 0.9,
        has_task: true,
        ..UnitObs::default()
    });
    assert_eq!(g.eval(&obs).action, ActionKind::Withdraw);
}

/// A hand crew carries no water, so the resupply branch must never fire for
/// one — the classic always-negative that would look like a working policy.
#[test]
fn a_hand_crew_never_breaks_off_for_water() {
    let mut ov = Overrides::new();
    let g = default_unit_graph();
    // Even with the trigger wound right up.
    let key = BehaviorGraph::override_key(
        g.nodes.iter().find(|n| n.type_id == "block.unit_resupply").unwrap().id,
        "refill_below",
    );
    ov.insert(key, ParamValue::Number(1.0));
    let compiled = CompiledGraph::compile(&g, &ov).unwrap();

    let crew = Observation::from(UnitObs {
        kind: behavior::UnitKindKey::HandCrew,
        carries_water: false,
        water_fraction: 0.0,
        is_working: true,
        has_task: true,
        ..UnitObs::default()
    });
    assert_ne!(compiled.eval(&crew).action, ActionKind::Refill);
}

#[test]
fn a_unit_profile_governs_the_kinds_it_names() {
    let cautious = default_unit_subtypes()
        .into_iter()
        .find(|s| s.id == "cautious-engines")
        .unwrap();
    // Off by default, like every authored behaviour.
    assert!(!cautious.enabled);
    let mut on = cautious.clone();
    on.enabled = true;
    assert!(on.governs(behavior::UnitKindKey::Engine));
    assert!(!on.governs(behavior::UnitKindKey::HandCrew));

    // An empty list means every kind.
    let standing = default_unit_subtypes()
        .into_iter()
        .find(|s| s.id == "standing-orders")
        .unwrap();
    assert!(behavior::UnitKindKey::ALL.into_iter().all(|k| standing.governs(k)));
}

/// The profile that exists to show the per-kind parameters are worth having has
/// to actually answer differently, or it is decoration.
#[test]
fn the_cautious_profile_pulls_engines_out_earlier() {
    let lib = behavior::defaults::default_library();
    let stock = lib.compile("standing-orders").unwrap();
    let cautious = lib.compile("cautious-engines").unwrap();

    let marginal = Observation::from(UnitObs {
        kind: behavior::UnitKindKey::Engine,
        carries_water: true,
        water_fraction: 0.5,
        is_working: true,
        has_task: true,
        threat_here: 0.30,
        ..UnitObs::default()
    });
    assert_eq!(stock.eval(&marginal).action, ActionKind::Continue);
    assert_eq!(cautious.eval(&marginal).action, ActionKind::Withdraw);
}

#[test]
fn a_unit_sweep_finds_the_safety_threshold() {
    let g = CompiledGraph::compile(&default_unit_graph(), &Overrides::new()).unwrap();
    let base = Observation::from(UnitObs {
        kind: behavior::UnitKindKey::Engine,
        carries_water: true,
        water_fraction: 1.0,
        is_working: true,
        has_task: true,
        ..UnitObs::default()
    });
    let pts = testbench::sweep(&g, &base, SweepField::UnitThreat, 101);
    let changes = testbench::transitions(&pts);
    assert_eq!(changes.len(), 1, "{changes:?}");
    let (at, from, to) = changes[0];
    assert_eq!(from, ActionKind::Continue);
    assert_eq!(to, ActionKind::Withdraw);
    assert!((0.34..0.37).contains(&at), "engines disengage at {at}");
}

/// A sweep only offers fields the graph's own agent has. Offering the others
/// would offer a sweep that provably varies nothing.
#[test]
fn sweep_fields_are_scoped_to_the_domain() {
    for d in Domain::ALL {
        let fields = SweepField::for_domain(d);
        assert!(!fields.is_empty());
        assert!(fields.iter().all(|f| f.domain() == d), "{d:?} offers a foreign sweep");
    }
    // And a foreign sweep is inert rather than corrupting: setting a household
    // field on a unit observation does nothing at all.
    let mut obs = Observation::empty(Domain::SuppressionUnit);
    SweepField::Cue.set(&mut obs, 0.9);
    assert_eq!(obs, Observation::empty(Domain::SuppressionUnit));
}

// --- separated people ------------------------------------------------------

#[test]
fn the_shipped_person_behaviour_validates_and_walks_them_out() {
    let g = default_person_graph();
    let r = behavior::validate(&g);
    assert!(r.ok(), "{:?}", r.errors().map(|e| &e.message).collect::<Vec<_>>());

    let c = CompiledGraph::compile(&g, &Overrides::new()).unwrap();
    // The first tick, for someone who was out when it started. This is the
    // behaviour the hand-written model had, and it has to survive being
    // written down.
    let first = testbench::person_situations()
        .into_iter()
        .find(|s| s.name == "Out and about")
        .unwrap();
    assert_eq!(c.eval(&first.obs).action, ActionKind::WalkOut);
}

/// Sheltering has to outbid walking when there is nowhere left to walk, or the
/// behaviour marches people into the fire — the same ordering the household
/// graph turns on, and the same reason.
#[test]
fn a_person_with_no_route_shelters_rather_than_walking() {
    let c = CompiledGraph::compile(&default_person_graph(), &Overrides::new()).unwrap();
    let cut = testbench::person_situations()
        .into_iter()
        .find(|s| s.name == "Cut off")
        .unwrap();
    assert_eq!(c.eval(&cut.obs).action, ActionKind::TakeShelter);
}

/// The whole point of the reunification block is that it does nothing until
/// someone turns it on. A branch that shipped silently live would have changed
/// every casualty figure the model has printed.
#[test]
fn going_home_is_off_in_the_shipped_behaviour_and_on_in_the_profile() {
    let g = default_person_graph();
    let shipped = CompiledGraph::compile(&g, &Overrides::new()).unwrap();
    let nearly_home = testbench::person_situations()
        .into_iter()
        .find(|s| s.name == "Nearly home")
        .unwrap();
    assert_eq!(shipped.eval(&nearly_home.obs).action, ActionKind::WalkOut);

    let family = default_person_subtypes()
        .into_iter()
        .find(|s| s.id == "family-first")
        .unwrap();
    let on = CompiledGraph::compile(&g, &family.overrides).unwrap();
    assert_eq!(on.eval(&nearly_home.obs).action, ActionKind::HeadHome);
}

/// Going home has to stop being an option when there is nothing to go back to,
/// even for the profile that is built to do it. This is the branch that decides
/// whether the model is describing a plausible mistake or an absurd one.
#[test]
fn nobody_walks_back_into_a_house_that_is_already_burning() {
    let family = default_person_subtypes()
        .into_iter()
        .find(|s| s.id == "family-first")
        .unwrap();
    let on = CompiledGraph::compile(&default_person_graph(), &family.overrides).unwrap();
    for name in ["House alight", "Cut off"] {
        let s = testbench::person_situations().into_iter().find(|s| s.name == name).unwrap();
        assert_ne!(on.eval(&s.obs).action, ActionKind::HeadHome, "{name}");
    }
}

/// A person node in a household graph, and the other way round. The same rule
/// the two original domains hold to, extended to the third — and worth pinning
/// separately, because the person observation is the one whose field names most
/// resemble the household's.
#[test]
fn the_person_domain_is_partitioned_like_the_others() {
    let mut people = default_person_graph();
    assert!(people.add("obs.cue", [0.0, 0.0]).is_none(), "a household cue went into a person");
    assert!(people.add("unit.water_fraction", [0.0, 0.0]).is_none());
    assert!(people.add("math.add", [0.0, 0.0]).is_some());

    let mut households = default_graph();
    assert!(households.add("person.home_distance_m", [0.0, 0.0]).is_none());

    // And the sinks are not interchangeable.
    let mut g = BehaviorGraph::new_in(Domain::Person, "p", "P");
    g.add("out.decision", [0.0, 0.0]);
    assert!(!behavior::validate(&g).ok());
}

#[test]
fn a_person_behaviour_round_trips_through_json() {
    let g = default_person_graph();
    let back = BehaviorGraph::from_json(&g.to_json().unwrap()).unwrap();
    assert_eq!(back, g);
    assert_eq!(back.domain, Domain::Person);
}

#[test]
fn person_profiles_are_assigned_by_share() {
    let lib = behavior::defaults::default_library();
    let a = lib.person_assignment();
    assert_eq!(a.len(), 1, "only the baseline ships with a share: {a:?}");
    assert_eq!(a[0].0, "walk-out");
    // Everything the model can run is assigned somewhere, which is what
    // `has_assignment` is for — checking only the households used to discard a
    // library whose one live profile was a unit policy.
    assert!(lib.has_assignment());
}

/// The person observation has to be inert in another domain's graph, the same
/// way the household and unit ones are.
#[test]
fn a_person_sweep_does_nothing_to_a_household() {
    let mut obs = Observation::empty(Domain::Household);
    SweepField::PersonHomeDistance.set(&mut obs, 50.0);
    assert_eq!(obs, Observation::empty(Domain::Household));
    assert_eq!(SweepField::PersonHomeDistance.domain(), Domain::Person);
    let base = PersonObs::default();
    assert!(base.home_distance_m > 0.0);
}

// --- the execution slice ---------------------------------------------------

/// What the live inspector draws bright. Every node in the slice contributed a
/// value to the decision that was taken, and every node outside it did not —
/// which is the only claim a highlight is allowed to make.
#[test]
fn the_active_slice_is_what_produced_the_decision() {
    let g = CompiledGraph::compile(&default_graph(), &Overrides::new()).unwrap();
    let cut = testbench::situations().into_iter().find(|s| s.name == "Cut off").unwrap();
    let (d, trace) = g.eval_traced(&cut.obs);
    assert_eq!(d.action, ActionKind::Shelter);

    let active = trace.active();
    let winner = trace.winner().expect("something fired");
    assert!(active.contains(&winner));
    assert!(active.contains(&trace.sink.unwrap()), "the sink is always on the path");

    // The winner here is fed by "Fire on the property", so that block is on the
    // path; "Response to the order" fed the branch that lost, and is not.
    let node_of = |ty: &str| trace.nodes.iter().find(|n| n.type_id == ty).unwrap().node;
    assert!(active.contains(&node_of("block.fire_at_the_door")));
    assert!(
        !active.contains(&node_of("block.order_response")),
        "a losing branch is on the path"
    );
    assert!(!active.contains(&node_of("logic.any")), "a losing branch is on the path");

    // Every proposal is wired into the decision sink, so a slice that expanded
    // it would reach every branch there is. That the losing ones are absent is
    // the check that it does not.
    assert!(!active.contains(&node_of("action.prepare")));

    // The alarm threshold *is* on the path, and not through the action it fed:
    // its third output drives the urgency readout, which the model reads back.
    // A sink is a sink whether or not it changes anything mechanically.
    assert!(active.contains(&node_of("block.alarm")));
    assert!(active.contains(&node_of("out.urgency")));
    assert!(active.contains(&node_of("block.preparation")), "the milling multiplier is a lever");

    // Every wire drawn bright joins two nodes that are themselves bright.
    for (from, _, to, _) in trace.active_wires() {
        assert!(active.contains(&from) && active.contains(&to));
    }
}

/// Nothing fired is a real answer, not a missing one: the agent is doing its
/// domain's default and no branch of the graph asked for it. The slice has to
/// say so rather than lighting up something arbitrary.
#[test]
fn a_graph_with_nothing_firing_has_no_action_on_the_path() {
    let g = CompiledGraph::compile(&default_person_graph(), &Overrides::new()).unwrap();
    let mut obs = PersonObs::default();
    // Nothing to react to, and the walk-out block held off.
    obs.cue = -1.0;
    obs.order_issued = false;
    let (d, trace) = g.eval_traced(&obs.into());
    assert_eq!(d.action, ActionKind::Remain);
    assert!(trace.winner().is_none());

    // No proposal is on the path, because none of them was taken. What remains
    // is the sinks and whatever feeds the readouts, which is honest: those
    // values *were* handed back to the model.
    let active = trace.active();
    assert!(active.contains(&trace.sink.unwrap()));
    for n in &trace.nodes {
        if n.category == Category::Action {
            assert!(!active.contains(&n.node), "{} is on the path", n.name);
        }
    }

    // But the branches that declined are named, which is the thing a reader
    // most wants told apart from a node nothing reaches.
    assert!(!trace.withheld().is_empty());
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

/// Print what the palette actually offers, per domain.
///
/// Ignored, because it is a report rather than a check: the numbers go in
/// `CLAUDE.md`, and a test that asserted them would fail every time somebody
/// added a node, which is the opposite of what should happen.
#[test]
#[ignore]
fn palette_report() {
    println!("{} nodes registered", registry().len());
    for d in Domain::ALL {
        println!("  {:20} {}", d.label(), registry().in_domain(d).count());
    }
    let shared = registry().all().filter(|s| s.domain.is_none()).count();
    println!("  {:20} {shared}", "domain-free");
}
