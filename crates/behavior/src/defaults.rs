//! The behaviours the game ships with, built in code.
//!
//! Two graphs, one per [`Domain`]. Both are transcriptions of hand-written
//! model code — the departure rules in `abm::decide`, and the safety and
//! resupply policy in `abm::suppression` — with the same thresholds and the
//! same answers. They exist for two reasons: they give a scientist something to
//! *start from* rather than an empty canvas, and they are the honest
//! demonstration that the node set is expressive enough for the model that
//! already exists. If the composer could not reproduce the shipped behaviour,
//! it could not replace it.
//!
//! They are built here rather than shipped as JSON blobs so they cannot drift
//! out of sync with the node registry — a renamed port breaks the build, not a
//! file someone loads in six months.
//!
//! ### Written in blocks, on purpose
//!
//! The civilian graph used to be thirty-one nodes: nine observations feeding
//! six arithmetic nodes to build one threshold, and so on. It said exactly what
//! it says now and you had to already know the model to read it. Rebuilt on
//! [`Category::Block`](crate::Category::Block) nodes it is thirteen, each one a
//! named assumption with its numbers on the front — which is the level a
//! subtype override wants to be at anyway. The arithmetic version is still
//! buildable and still supported; it is just not what a scientist should have
//! to open first.

use crate::domain::Domain;
use crate::graph::{BehaviorGraph, NodeId, Wire};
use crate::node::ParamValue;
use crate::subtype::{AgentSubtype, Capability, TraitKey};
use crate::value::UnitKindKey;

pub const DEFAULT_GRAPH_ID: &str = "default-evacuation";
pub const DEFAULT_UNIT_GRAPH_ID: &str = "default-unit-policy";

/// Small builder so the graphs below read as the models they are, rather than
/// as forty lines of index arithmetic.
struct Build {
    g: BehaviorGraph,
}

impl Build {
    fn add(&mut self, ty: &str, x: f32, y: f32) -> NodeId {
        self.g
            .add(ty, [x, y])
            .unwrap_or_else(|| panic!("node type {ty} is not registered, or not in this domain"))
    }

    fn set(&mut self, node: NodeId, param: &str, v: ParamValue) {
        if let Some(n) = self.g.node_mut(node) {
            n.params.insert(param.to_string(), v);
        }
    }

    fn num(&mut self, node: NodeId, param: &str, v: f32) {
        self.set(node, param, ParamValue::Number(v));
    }

    fn boolean(&mut self, node: NodeId, param: &str, v: bool) {
        self.set(node, param, ParamValue::Bool(v));
    }

    fn wire(&mut self, from: NodeId, from_port: u16, to: NodeId, to_port: u16) {
        let w = Wire { from_node: from, from_port, to_node: to, to_port };
        assert!(self.g.connect(w), "default graph wire rejected: {w:?}");
    }
}

// ---------------------------------------------------------------------------
// Civilians
// ---------------------------------------------------------------------------

/// The shipped evacuation behaviour.
pub fn default_graph() -> BehaviorGraph {
    let mut b = Build {
        g: BehaviorGraph {
            id: DEFAULT_GRAPH_ID.to_string(),
            name: "Four-stage evacuation".to_string(),
            domain: Domain::Household,
            description:
                "The behaviour the game ships with. Departure needs either the household's \
                 own alarm to pass a threshold set by the plan they came in with, or an \
                 order they trust — and households that planned to defend do not move on \
                 an order at all. Anyone the fire reaches leaves immediately; anyone whose \
                 route is cut shelters where they stand."
                    .to_string(),
            nodes: Vec::new(),
            wires: Vec::new(),
        },
    };

    // --- column 1: the three assumptions ----------------------------------
    // Each of these was six to ten nodes before. The numbers on them are the
    // ones from `abm::decide`, unchanged.
    let alarm = b.add("block.alarm", 0.0, 0.0);
    b.num(alarm, "leave_early", 0.02);
    b.num(alarm, "wait_and_see", 0.22);
    b.num(alarm, "stay_defend", 0.55);
    b.num(alarm, "risk_relief", 0.15);
    b.num(alarm, "spread", 0.10);

    let order = b.add("block.order_response", 0.0, 260.0);
    b.num(order, "trust_threshold", 0.35);
    b.boolean(order, "defenders_comply", false);

    let arrival = b.add("block.fire_at_the_door", 0.0, 470.0);
    b.num(arrival, "threat_limit", 0.35);

    // --- column 2: departure, and its consequence -------------------------
    let depart = b.add("logic.any", 320.0, 80.0);
    b.wire(alarm, 0, depart, 0);
    b.wire(order, 0, depart, 0);

    let defending = b.add("block.stand_ground", 320.0, 300.0);
    b.num(defending, "min_defensible_space", 0.0);
    b.boolean(defending, "only_if_planned", true);
    b.wire(depart, 0, defending, 0);

    // --- column 3: the proposals -------------------------------------------
    let prepare = b.add("action.prepare", 600.0, 40.0);
    b.num(prepare, "priority", 1.0);
    b.wire(depart, 0, prepare, 0);

    let run = b.add("action.evacuate_now", 600.0, 180.0);
    b.num(run, "priority", 3.0);
    b.wire(arrival, 0, run, 0);

    let defend = b.add("action.defend", 600.0, 320.0);
    b.num(defend, "priority", 0.5);
    b.wire(defending, 0, defend, 0);

    // Sheltering outbids running: with the route cut, the road is where the
    // casualties are.
    let shelter = b.add("action.shelter", 600.0, 460.0);
    b.num(shelter, "priority", 4.0);
    b.wire(arrival, 1, shelter, 0);

    // --- column 4: the sinks ------------------------------------------------
    let decision = b.add("out.decision", 860.0, 200.0);
    b.wire(prepare, 0, decision, 0);
    b.wire(run, 0, decision, 0);
    b.wire(defend, 0, decision, 0);
    b.wire(shelter, 0, decision, 0);

    let prep = b.add("block.preparation", 600.0, 620.0);
    b.num(prep, "leave_early", 0.8);
    b.num(prep, "wait_and_see", 1.0);
    b.num(prep, "stay_defend", 0.5);
    let prep_out = b.add("out.prep_scale", 860.0, 620.0);
    b.wire(prep, 0, prep_out, 0);

    // The margin past their own threshold is a better readout than the raw
    // alarm: it says how far past the point of acting they are, which is the
    // quantity the behaviour actually turns on.
    let urgency_out = b.add("out.urgency", 860.0, 740.0);
    b.wire(alarm, 2, urgency_out, 0);

    b.g
}

// ---------------------------------------------------------------------------
// Suppression units
// ---------------------------------------------------------------------------

/// The shipped unit policy: exactly what `abm::suppression` did in Rust.
///
/// Six nodes, and it is not small because it is a toy — it is small because
/// almost everything a unit does is the commander's decision, and the only
/// thing the unit decides for itself is when to stop. That was two `if`s and
/// two constants; it is now two blocks with their numbers on the front.
pub fn default_unit_graph() -> BehaviorGraph {
    let mut b = Build {
        g: BehaviorGraph {
            id: DEFAULT_UNIT_GRAPH_ID.to_string(),
            name: "Standing orders".to_string(),
            domain: Domain::SuppressionUnit,
            description:
                "What a crew, engine or aircraft does about its own safety and its own \
                 supplies, which is everything a unit decides that the commander does \
                 not. Reproduces the hand-written policy exactly: pull back above 0.35 \
                 of threat, and go for water when the tank is dry. Anything not covered \
                 here is the order the unit was given, which it gets on with."
                    .to_string(),
            nodes: Vec::new(),
            wires: Vec::new(),
        },
    };

    let safety = b.add("block.unit_safety", 0.0, 0.0);
    b.num(safety, "hand_crew_limit", 0.35);
    b.num(safety, "engine_limit", 0.35);
    // An aircraft overflying the front is not standing in it. Effectively never.
    b.num(safety, "air_limit", 1.0);
    // 1.0 disables the accumulated-heat trigger, which is what the shipped
    // model did: it only ever looked at the instantaneous threat. Bringing this
    // down is the first interesting thing to try in this graph.
    b.num(safety, "heat_limit", 1.0);

    let water = b.add("block.unit_resupply", 0.0, 300.0);
    // Zero reproduces "pump dry, then go". Raising it is the open question:
    // 2,500 L is six minutes of pumping and the hydrant round trip is longer.
    b.num(water, "refill_below", 0.0);
    b.boolean(water, "only_when_working", true);

    let withdraw = b.add("action.unit_withdraw", 320.0, 20.0);
    b.num(withdraw, "priority", 5.0);
    b.wire(safety, 0, withdraw, 0);

    let refill = b.add("action.unit_refill", 320.0, 300.0);
    b.num(refill, "priority", 3.0);
    b.wire(water, 0, refill, 0);

    let decision = b.add("out.unit_decision", 620.0, 150.0);
    b.wire(withdraw, 0, decision, 0);
    b.wire(refill, 0, decision, 0);

    // How far past its own safety limit a unit is, in the HUD. Nothing else in
    // the interface says this, and it is the number a player wants when a crew
    // pulls out of somewhere that looked fine.
    let urgency = b.add("out.urgency", 620.0, 400.0);
    b.wire(safety, 1, urgency, 0);

    b.g
}

/// Override keys into a built graph, resolved by walking it rather than
/// hard-coded, so a reordering of the builders cannot silently point a subtype
/// at the wrong node.
fn key(g: &BehaviorGraph, type_id: &str, nth: usize, param: &str) -> String {
    let node = g
        .nodes
        .iter()
        .filter(|n| n.type_id == type_id)
        .nth(nth)
        .unwrap_or_else(|| panic!("default graph has no {type_id}#{nth}"));
    BehaviorGraph::override_key(node.id, param)
}

/// Four civilian profiles over the one graph, which is the pattern the composer
/// is for: the structure is shared, and only the numbers differ.
pub fn default_subtypes() -> Vec<AgentSubtype> {
    let g = default_graph();
    let alarm = |param: &str| key(&g, "block.alarm", 0, param);
    let trust_threshold = key(&g, "block.order_response", 0, "trust_threshold");
    let severe = key(&g, "block.fire_at_the_door", 0, "threat_limit");
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

/// Suppression profiles.
///
/// Two, and the second is off by default: one policy for everything, which is
/// the shipped model, and one that shows what the per-kind parameters are for.
/// Deliberately not four — a profile per unit kind would suggest the kinds want
/// different *policies*, and they do not. They want the same policy with
/// different numbers, which is one graph.
pub fn default_unit_subtypes() -> Vec<AgentSubtype> {
    let g = default_unit_graph();
    let safety = |param: &str| key(&g, "block.unit_safety", 0, param);
    let water = |param: &str| key(&g, "block.unit_resupply", 0, param);

    let mut standing = AgentSubtype::new(
        "standing-orders",
        "Standing orders",
        DEFAULT_UNIT_GRAPH_ID,
    );
    standing.description =
        "The shipped policy, applied to every unit: disengage above 0.35 of threat, \
         and pump the tank dry before going for water. This is what the model does \
         with no authored behaviour at all, written down."
            .to_string();
    standing.author = "shipped".to_string();
    standing.tags = vec!["baseline".into()];
    standing.share = 0.0;
    standing.enabled = true;

    let mut cautious = AgentSubtype::new(
        "cautious-engines",
        "Cautious engines",
        DEFAULT_UNIT_GRAPH_ID,
    );
    cautious.description =
        "Engines only. Pulls back earlier, breaks off for water with a third of the \
         tank left rather than running it dry, and leaves on accumulated heat as well \
         as on the threat in front of it. The comparison worth running: it delivers \
         less water per unit time and loses fewer engines, and which of those matters \
         is the question."
            .to_string();
    cautious.author = "shipped".to_string();
    cautious.tags = vec!["cautious".into(), "engines".into()];
    cautious.unit_kinds = vec![UnitKindKey::Engine];
    cautious.share = 0.0;
    // Off by default, like every authored behaviour: the shipped measurements
    // describe the model they were taken on.
    cautious.enabled = false;
    cautious.overrides.insert(safety("engine_limit"), ParamValue::Number(0.25));
    cautious.overrides.insert(safety("heat_limit"), ParamValue::Number(0.4));
    cautious.overrides.insert(water("refill_below"), ParamValue::Number(0.33));

    vec![standing, cautious]
}

/// Everything shipped, as a ready-to-use library.
pub fn default_library() -> crate::library::Library {
    let mut lib = crate::library::Library::default();
    lib.graphs.insert(DEFAULT_GRAPH_ID.to_string(), default_graph());
    lib.graphs.insert(DEFAULT_UNIT_GRAPH_ID.to_string(), default_unit_graph());
    for s in default_subtypes().into_iter().chain(default_unit_subtypes()) {
        lib.subtypes.insert(s.id.clone(), s);
    }
    lib
}
