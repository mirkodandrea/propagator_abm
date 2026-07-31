//! The behaviour the game ships with, built in code.
//!
//! This is a transcription of the hand-written decision layer in `abm` — the
//! same thresholds, the same intent-dependent departure rule, the same
//! trust-weighted response to an order. It exists for two reasons. It gives a
//! scientist something to *start from* rather than an empty canvas, and it is
//! the honest demonstration that the node set is expressive enough for the
//! model that already exists: if the composer could not reproduce the shipped
//! behaviour, it could not replace it.
//!
//! It is built here rather than shipped as a JSON blob so it cannot drift out
//! of sync with the node registry — a renamed port breaks the build, not a
//! file someone loads in six months.

use crate::graph::{BehaviorGraph, NodeId, Wire};
use crate::node::ParamValue;
use crate::subtype::{AgentSubtype, Capability, TraitKey};

pub const DEFAULT_GRAPH_ID: &str = "default-evacuation";

/// Small builder so the graph below reads as the model it is, rather than as
/// forty lines of index arithmetic.
struct Build {
    g: BehaviorGraph,
    col: f32,
}

impl Build {
    fn add(&mut self, ty: &str, x: f32, y: f32) -> NodeId {
        self.col = self.col.max(x);
        self.g.add(ty, [x, y]).unwrap_or_else(|| panic!("unregistered node type {ty}"))
    }

    fn set(&mut self, node: NodeId, param: &str, v: ParamValue) {
        if let Some(n) = self.g.node_mut(node) {
            n.params.insert(param.to_string(), v);
        }
    }

    fn num(&mut self, node: NodeId, param: &str, v: f32) {
        self.set(node, param, ParamValue::Number(v));
    }

    fn choice(&mut self, node: NodeId, param: &str, v: &str) {
        self.set(node, param, ParamValue::Choice(v.to_string()));
    }

    fn text(&mut self, node: NodeId, param: &str, v: &str) {
        self.set(node, param, ParamValue::Text(v.to_string()));
    }

    fn wire(&mut self, from: NodeId, from_port: u16, to: NodeId, to_port: u16) {
        let w = Wire { from_node: from, from_port, to_node: to, to_port };
        assert!(self.g.connect(w), "default graph wire rejected: {w:?}");
    }
}

/// The shipped evacuation behaviour.
pub fn default_graph() -> BehaviorGraph {
    let mut b = Build {
        g: BehaviorGraph {
            id: DEFAULT_GRAPH_ID.to_string(),
            name: "Four-stage evacuation".to_string(),
            description:
                "The behaviour the game ships with. Awareness rises from the fire the \
                 household can perceive; departure needs either that alarm to pass a \
                 threshold set by their stated plan, or an order they trust. Households \
                 that planned to defend hold until the fire is at the property; anyone \
                 whose route is cut shelters where they stand."
                    .to_string(),
            nodes: Vec::new(),
            wires: Vec::new(),
        },
        col: 0.0,
    };

    // --- column 1: what the household perceives ---------------------------
    let cue = b.add("obs.cue", 0.0, 0.0);
    let risk = b.add("obs.risk_perception", 0.0, 90.0);
    let jitter = b.add("obs.jitter", 0.0, 180.0);
    let intent = b.add("obs.intent", 0.0, 270.0);
    let warned = b.add("obs.warning_received", 0.0, 360.0);
    let trust = b.add("obs.trust_authority", 0.0, 450.0);
    let threat = b.add("obs.threat", 0.0, 540.0);
    let blocked = b.add("obs.route_blocked", 0.0, 630.0);
    let alight = b.add("obs.structure_alight", 0.0, 720.0);

    // --- column 2: the departure threshold ---------------------------------
    // Base level of alarm this household needs before it moves, set by the
    // plan they came in with. These are the numbers from `abm::decide`.
    let by_intent = b.add("intent.weight", 300.0, 180.0);
    b.num(by_intent, "leave_early", 0.02);
    b.num(by_intent, "wait_and_see", 0.22);
    b.num(by_intent, "stay_defend", 0.55);
    b.wire(intent, 0, by_intent, 0);

    // A risk-aware household needs less alarm to act on it.
    let risk_relief = b.add("math.scale", 300.0, 90.0);
    b.num(risk_relief, "gain", -0.15);
    b.num(risk_relief, "offset", 0.15);
    b.wire(risk, 0, risk_relief, 0);

    // Individual spread, so the population does not depart on one tick.
    let spread = b.add("math.scale", 300.0, 270.0);
    b.num(spread, "gain", 0.10);
    b.num(spread, "offset", -0.05);
    b.wire(jitter, 0, spread, 0);

    let thr_partial = b.add("math.add", 560.0, 135.0);
    b.wire(by_intent, 0, thr_partial, 0);
    b.wire(risk_relief, 0, thr_partial, 1);

    let threshold = b.add("math.add", 560.0, 230.0);
    b.wire(thr_partial, 0, threshold, 0);
    b.wire(spread, 0, threshold, 1);

    let alarmed = b.add("cmp.greater", 800.0, 60.0);
    b.wire(cue, 0, alarmed, 0);
    b.wire(threshold, 0, alarmed, 1);

    // --- column 2b: the order, and whether they believe it ------------------
    let trusting = b.add("cmp.above", 300.0, 450.0);
    b.num(trusting, "threshold", 0.35);
    b.wire(trust, 0, trusting, 0);

    let credible = b.add("logic.and", 560.0, 400.0);
    b.wire(warned, 0, credible, 0);
    b.wire(trusting, 0, credible, 1);

    // Households that planned to defend do not leave on an order alone. This
    // is the finding the whole evacuation literature turns on, and it is why
    // "we told them to go" is not the same as "they went".
    let plans_to_stay = b.add("intent.is", 300.0, 360.0);
    b.choice(plans_to_stay, "plan", "stay_defend");
    b.wire(intent, 0, plans_to_stay, 0);

    let will_move_on_order = b.add("logic.not", 560.0, 330.0);
    b.wire(plans_to_stay, 0, will_move_on_order, 0);

    let ordered_out = b.add("logic.and", 800.0, 360.0);
    b.wire(credible, 0, ordered_out, 0);
    b.wire(will_move_on_order, 0, ordered_out, 1);

    let depart = b.add("logic.or", 1040.0, 180.0);
    b.wire(alarmed, 0, depart, 0);
    b.wire(ordered_out, 0, depart, 1);

    // --- column 3: the fire arriving --------------------------------------
    let severe = b.add("param.number", 300.0, 540.0);
    b.text(severe, "label", "not survivable outside");
    b.num(severe, "value", 0.35);

    let unsurvivable = b.add("cmp.greater", 560.0, 540.0);
    b.wire(threat, 0, unsurvivable, 0);
    b.wire(severe, 0, unsurvivable, 1);

    // Losing the house is the cue that reaches even a household that has
    // decided to stay.
    let overrun = b.add("logic.or", 800.0, 600.0);
    b.wire(unsurvivable, 0, overrun, 0);
    b.wire(alight, 0, overrun, 1);

    let trapped = b.add("logic.and", 800.0, 700.0);
    b.wire(blocked, 0, trapped, 0);
    b.wire(overrun, 0, trapped, 1);

    // --- column 4: the proposals -------------------------------------------
    let prepare = b.add("action.prepare", 1300.0, 120.0);
    b.num(prepare, "priority", 1.0);
    b.wire(depart, 0, prepare, 0);

    let run = b.add("action.evacuate_now", 1300.0, 260.0);
    b.num(run, "priority", 3.0);
    b.wire(overrun, 0, run, 0);

    let defend = b.add("action.defend", 1300.0, 400.0);
    b.num(defend, "priority", 0.5);
    let defending = b.add("logic.and", 1040.0, 400.0);
    let not_departing = b.add("logic.not", 1040.0, 500.0);
    b.wire(depart, 0, not_departing, 0);
    b.wire(plans_to_stay, 0, defending, 0);
    b.wire(not_departing, 0, defending, 1);
    b.wire(defending, 0, defend, 0);

    // Sheltering outbids running: with the route cut, the road is where the
    // casualties are.
    let shelter = b.add("action.shelter", 1300.0, 560.0);
    b.num(shelter, "priority", 4.0);
    b.wire(trapped, 0, shelter, 0);

    // --- column 5: the sinks ------------------------------------------------
    let decision = b.add("out.decision", 1600.0, 260.0);
    b.wire(prepare, 0, decision, 0);
    b.wire(run, 0, decision, 0);
    b.wire(defend, 0, decision, 0);
    b.wire(shelter, 0, decision, 0);

    // Households that had already decided to go have less to pack; ones that
    // meant to stay are outside with the hoses out and leave in a hurry.
    let prep_by_intent = b.add("intent.weight", 1300.0, 700.0);
    b.num(prep_by_intent, "leave_early", 0.8);
    b.num(prep_by_intent, "wait_and_see", 1.0);
    b.num(prep_by_intent, "stay_defend", 0.5);
    b.wire(intent, 0, prep_by_intent, 0);

    let prep_out = b.add("out.prep_scale", 1600.0, 700.0);
    b.wire(prep_by_intent, 0, prep_out, 0);

    let urgency = b.add("math.max", 1300.0, 820.0);
    b.wire(cue, 0, urgency, 0);
    b.wire(threat, 0, urgency, 1);
    let urgency_out = b.add("out.urgency", 1600.0, 820.0);
    b.wire(urgency, 0, urgency_out, 0);

    b.g
}

/// Override keys into the default graph, resolved by walking the built graph
/// rather than hard-coded, so a reordering of `default_graph` cannot silently
/// point a subtype at the wrong node.
fn key(g: &BehaviorGraph, type_id: &str, nth: usize, param: &str) -> String {
    let node = g
        .nodes
        .iter()
        .filter(|n| n.type_id == type_id)
        .nth(nth)
        .unwrap_or_else(|| panic!("default graph has no {type_id}#{nth}"));
    BehaviorGraph::override_key(node.id, param)
}

/// Four profiles over the one graph, which is the pattern the composer is for:
/// the structure is shared, and only the numbers differ.
pub fn default_subtypes() -> Vec<AgentSubtype> {
    let g = default_graph();
    let alarm = |param: &str| key(&g, "intent.weight", 0, param);
    let trust_threshold = key(&g, "cmp.above", 0, "threshold");
    let severe = key(&g, "param.number", 0, "value");
    let prepare_priority = key(&g, "action.prepare", 0, "priority");

    let base = |id: &str, name: &str, desc: &str| {
        let mut s = AgentSubtype::new(id, name, DEFAULT_GRAPH_ID);
        s.description = desc.to_string();
        s.author = "shipped".to_string();
        s
    };

    let mut prepared = base(
        "prepared-resident",
        "Prepared resident",
        "Has a plan and has rehearsed it. Goes on the first credible signal, and the \
         packing is already done.",
    );
    prepared.tags = vec!["baseline".into(), "prepared".into()];
    prepared.share = 0.25;
    prepared.overrides.insert(alarm("leave_early"), ParamValue::Number(0.02));
    prepared.overrides.insert(alarm("wait_and_see"), ParamValue::Number(0.10));
    prepared.traits.insert(TraitKey::RiskPerception, 0.8);
    prepared.traits.insert(TraitKey::PrepTimeMin, 8.0);
    prepared.traits.insert(TraitKey::DefensibleSpace, 0.7);

    let mut waiting = base(
        "wait-and-see",
        "Wait and see",
        "The dangerous majority. Needs a direct cue or an order they trust, and then \
         takes a long time to actually get out of the door.",
    );
    waiting.tags = vec!["baseline".into(), "majority".into()];
    waiting.share = 0.45;
    waiting.traits.insert(TraitKey::PrepTimeMin, 25.0);

    let mut defender = base(
        "committed-defender",
        "Committed defender",
        "Intends to stay with the property and will not move on an order. Leaves only \
         when the fire is on the house, which is far too late to drive out.",
    );
    defender.tags = vec!["defend".into()];
    defender.share = 0.2;
    defender.overrides.insert(alarm("stay_defend"), ParamValue::Number(0.75));
    defender.overrides.insert(severe, ParamValue::Number(0.55));
    defender.traits.insert(TraitKey::DefensibleSpace, 0.85);
    defender.traits.insert(TraitKey::TrustAuthority, 0.2);

    let mut assisted = base(
        "needs-assistance",
        "Needs assistance",
        "Cannot leave unaided and has no vehicle. Reacts early, because reacting late \
         is not survivable at walking pace — and this is the profile that shows whether \
         a warning timing works for the people it has to work for.",
    );
    assisted.tags = vec!["vulnerable".into()];
    assisted.share = 0.1;
    assisted.overrides.insert(alarm("wait_and_see"), ParamValue::Number(0.08));
    assisted.overrides.insert(trust_threshold, ParamValue::Number(0.2));
    assisted.overrides.insert(prepare_priority, ParamValue::Number(1.0));
    assisted.traits.insert(TraitKey::PrepTimeMin, 35.0);
    assisted.capabilities.insert(Capability::Vehicle, false);
    assisted.capabilities.insert(Capability::NeedsAssistance, true);

    vec![prepared, waiting, defender, assisted]
}

/// The shipped graph and subtypes as a ready-to-use library.
pub fn default_library() -> crate::library::Library {
    let mut lib = crate::library::Library::default();
    lib.graphs.insert(DEFAULT_GRAPH_ID.to_string(), default_graph());
    for s in default_subtypes() {
        lib.subtypes.insert(s.id.clone(), s);
    }
    lib
}
