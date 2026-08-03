//! Reference behavior fixtures and the generator for the editable files.
//!
//! Three graphs, one per [`Domain`]. They transcribe the model's former
//! decision code — household departure, separated-person destination, and unit
//! safety/resupply — with the same thresholds and answers. They exist for two
//! reasons: they give a scientist something to
//! *start from* rather than an empty canvas, and they are the honest
//! demonstration that the node set is expressive enough for the model that
//! already exists. If the composer could not reproduce the shipped behaviour,
//! it could not replace it.
//!
//! The game itself loads `data/behaviours/`; it never falls back to this module.
//! Keeping a typed builder here gives tests a compact fixture and regenerates
//! the editable JSON without letting its ports drift from the node registry.
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
pub const DEFAULT_PERSON_GRAPH_ID: &str = "default-separated-person";

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

    let prep = b.add("block.preparation", 600.0, 900.0);
    b.num(prep, "leave_early", 0.8);
    b.num(prep, "wait_and_see", 1.0);
    b.num(prep, "stay_defend", 0.5);
    let prep_out = b.add("out.prep_scale", 860.0, 900.0);
    b.wire(prep, 0, prep_out, 0);

    // The margin past their own threshold is a better readout than the raw
    // alarm: it says how far past the point of acting they are, which is the
    // quantity the behaviour actually turns on.
    let urgency_out = b.add("out.urgency", 860.0, 1020.0);
    b.wire(alarm, 2, urgency_out, 0);

    // --- what the incident itself can do to them ---------------------------
    //
    // Six nodes that did not exist until three scenarios were built against real
    // disasters (`docs/behavior-gaps.md`). Two of the blocks ship switched off,
    // because turning one on changes the casualty figures every measurement in
    // `crates/fire/tests` was taken without. The other two need a lever nobody
    // has pulled — a road closure, a transient population — so they are live and
    // silent, which is not the same state as disabled and is the honest one for
    // a branch whose condition has simply not happened.
    //
    // **Added at the end, and that is not a style choice.** A node's id is its
    // position in this sequence, and a subtype's overrides are keyed on it, so
    // inserting these where they belong on the canvas renumbered every node
    // after them and silently pointed every override in every profile at the
    // wrong box. That is finding 24 one level out: the ids survive the editor
    // now, and what they do not survive is somebody rewriting the shipped graph
    // in the middle. Append.
    let spot = b.add("block.spot_fire", 0.0, 700.0);
    b.boolean(spot, "enabled", false);
    b.num(spot, "radius_m", 800.0);
    b.num(spot, "recent_min", 20.0);
    b.wire(spot, 0, depart, 0);

    let signal = b.add("block.no_signal", 0.0, 930.0);
    b.boolean(signal, "enabled", false);
    b.num(signal, "patience_min", 10.0);
    b.num(signal, "min_trust", 0.35);
    b.wire(signal, 1, depart, 0);

    let closed = b.add("block.road_closed", 0.0, 1140.0);
    b.num(closed, "will_walk_within", 3000.0);
    b.wire(closed, 1, depart, 0);

    let visitors = b.add("block.visitors", 0.0, 1350.0);
    b.num(visitors, "own_judgement_alarm", 0.60);
    b.wire(visitors, 1, depart, 0);

    // Somewhere to go that is not the house and not a refuge. Both proposals
    // outbid sheltering in place, and they have to: this block fires in exactly
    // the situation `block.fire_at_the_door` calls trapped, so at the shelter
    // branch's own priority it would be a box that never won anything — the
    // always-negative finding 26 is about.
    let last = b.add("block.last_resort", 320.0, 1560.0);
    b.boolean(last, "enabled", false);
    b.num(last, "max_walk_m", 600.0);
    b.boolean(last, "only_when_cut_off", false);
    b.boolean(last, "only_with_lift", true);
    b.num(last, "lift_within_min", 30.0);
    // The same moment `action.shelter` and `action.evacuate_now` turn on, so
    // the three branches cannot disagree about when the fire has arrived.
    b.wire(arrival, 0, last, 0);

    let open_ground = b.add("action.shelter_nearby", 600.0, 1500.0);
    b.num(open_ground, "priority", 4.5);
    b.wire(last, 0, open_ground, 0);
    b.wire(open_ground, 0, decision, 0);

    let to_shore = b.add("action.make_for_shore", 600.0, 1640.0);
    b.num(to_shore, "priority", 4.5);
    b.wire(last, 1, to_shore, 0);
    b.wire(to_shore, 0, decision, 0);

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

// ---------------------------------------------------------------------------
// Separated people
// ---------------------------------------------------------------------------

/// What one person away from their household does, as the model already did it.
///
/// Five nodes. The shipped answer has always been "walk to the nearest refuge
/// and keep walking", and that is what the top two branches say. The third —
/// going back for the family — is wired in but switched off at the block, which
/// is the honest way to ship a behaviour that would change every casualty figure
/// the game has printed: it is on the canvas where an author can see it, and it
/// does nothing until someone turns it on.
pub fn default_person_graph() -> BehaviorGraph {
    let mut b = Build {
        g: BehaviorGraph {
            id: DEFAULT_PERSON_GRAPH_ID.to_string(),
            name: "Walk out".to_string(),
            domain: Domain::Person,
            description:
                "What someone who was away from home when it started does. They make for \
                 the nearest refuge on foot straight away, and stop and shelter if the \
                 way out is cut. Going back for the family is on the canvas and switched \
                 off — turn it on in \"Going back for the family\" and watch the \
                 casualty count."
                    .to_string(),
            nodes: Vec::new(),
            wires: Vec::new(),
        },
    };

    // --- column 1: the three assumptions ----------------------------------
    let walk = b.add("block.person_walk_out", 0.0, 0.0);
    // Zero, inclusive, so this fires on the first tick: the shipped behaviour.
    b.num(walk, "alarm_needed", 0.0);
    b.boolean(walk, "order_is_enough", true);
    b.num(walk, "spread", 0.0);

    let exposure = b.add("block.person_exposure", 0.0, 250.0);
    b.num(exposure, "threat_limit", 0.55);
    // 1.0 disables the heat trigger, which is what the movement layer did on its
    // own: it only ever stopped someone who had nowhere left to walk.
    b.num(exposure, "heat_limit", 1.0);

    let reunite = b.add("block.person_reunite", 0.0, 520.0);
    b.boolean(reunite, "enabled", false);
    b.boolean(reunite, "only_if_family_home", true);
    b.num(reunite, "max_home_distance_m", 1200.0);
    b.num(reunite, "max_detour", 1.5);
    b.num(reunite, "max_threat", 0.30);

    // --- column 2: the proposals --------------------------------------------
    let out_now = b.add("action.person_walk_out", 340.0, 20.0);
    b.num(out_now, "priority", 1.0);
    b.wire(walk, 0, out_now, 0);

    // Sheltering outbids everything: with no route left, walking is what kills
    // them, and it is the same ordering the household graph uses.
    let shelter = b.add("action.person_shelter", 340.0, 280.0);
    b.num(shelter, "priority", 4.0);
    b.wire(exposure, 1, shelter, 0);

    let home = b.add("action.person_head_home", 340.0, 540.0);
    b.num(home, "priority", 2.0);
    b.wire(reunite, 0, home, 0);

    // --- column 3: the sink --------------------------------------------------
    let decision = b.add("out.person_decision", 640.0, 250.0);
    b.wire(out_now, 0, decision, 0);
    b.wire(shelter, 0, decision, 0);
    b.wire(home, 0, decision, 0);

    // How far past survivable it is where they are standing. Nothing else in the
    // interface says this for a person on foot.
    let urgency = b.add("out.urgency", 640.0, 460.0);
    b.wire(exposure, 2, urgency, 0);

    // --- somewhere nearer than the refuge -----------------------------------
    //
    // Appended rather than placed where they belong, for the reason the
    // household graph's late additions are: a node's id is its position in this
    // sequence and every profile's overrides are keyed on it.
    //
    // Both ship off. The first changes who dies and the second needs a lift
    // nobody has asked for, and either way every separated-person figure the
    // model has printed was taken without them.
    let last = b.add("block.person_last_resort", 0.0, 780.0);
    b.boolean(last, "enabled", false);
    b.boolean(last, "also_when_cut_off", true);
    b.num(last, "max_walk_m", 500.0);
    b.boolean(last, "only_with_lift", true);
    b.num(last, "lift_within_min", 30.0);
    b.wire(exposure, 0, last, 0);

    let pickup = b.add("block.person_boat_pickup", 0.0, 1030.0);
    b.boolean(pickup, "enabled", false);
    b.num(pickup, "within_min", 25.0);
    b.num(pickup, "max_shore_distance_m", 1500.0);
    b.boolean(pickup, "even_if_refuge_nearer", true);

    // Open ground outbids sheltering where they stand, and has to for the same
    // reason the household's does: it fires in the situation the shelter branch
    // already covers, so at the shelter branch's priority it would be a box that
    // never won.
    let ground = b.add("action.person_open_ground", 340.0, 790.0);
    b.num(ground, "priority", 4.5);
    b.wire(last, 0, ground, 0);
    b.wire(ground, 0, decision, 0);

    // Two ways to end up walking to the water and one action: being out of
    // options, and being told boats are coming. They are different decisions
    // and it matters that they are separate boxes — the second is an organised
    // evacuation and the first is Mati.
    let shore_when = b.add("logic.any", 340.0, 1030.0);
    b.wire(last, 1, shore_when, 0);
    b.wire(pickup, 0, shore_when, 0);

    let shore = b.add("action.person_shore", 600.0, 1030.0);
    b.num(shore, "priority", 4.5);
    b.wire(shore_when, 0, shore, 0);
    b.wire(shore, 0, decision, 0);

    b.g
}

/// Two person profiles, and the second is the point of the domain.
///
/// `walk-out` is the shipped model written down; `family-first` is the same
/// graph with reunification switched on. Ships at share 0 — turning it on is a
/// deliberate act, because it changes what the model says about who dies.
pub fn default_person_subtypes() -> Vec<AgentSubtype> {
    let g = default_person_graph();
    let reunite = |param: &str| key(&g, "block.person_reunite", 0, param);
    let walk = |param: &str| key(&g, "block.person_walk_out", 0, param);

    let mut out = AgentSubtype::new("walk-out", "Walks out", DEFAULT_PERSON_GRAPH_ID);
    out.description =
        "Makes for the nearest refuge and keeps going. What the model has always done \
         with people who were out when it started, and what the guidance tells them to \
         do."
            .to_string();
    out.author = "shipped".to_string();
    out.tags = vec!["baseline".into()];
    out.share = 1.0;

    let mut family = AgentSubtype::new("family-first", "Goes back for family", DEFAULT_PERSON_GRAPH_ID);
    family.description =
        "Turns round for the house while there is still someone in it and the way back \
         is not already burning. The behaviour every post-fire study finds and no \
         evacuation plan assumes. Ships at zero share: give it one and the casualty \
         figures stop being comparable with anything measured before, which is exactly \
         the comparison worth running."
            .to_string();
    family.author = "shipped".to_string();
    family.tags = vec!["reunification".into(), "family".into()];
    family.share = 0.0;
    family.overrides.insert(reunite("enabled"), ParamValue::Bool(true));
    // Someone who has not heard the family already left is the lethal case, and
    // it is the one the interviews describe.
    family.overrides.insert(reunite("only_if_family_home"), ParamValue::Bool(false));
    family.overrides.insert(reunite("max_home_distance_m"), ParamValue::Number(2000.0));
    family.overrides.insert(reunite("max_detour"), ParamValue::Number(3.0));
    family.overrides.insert(walk("spread"), ParamValue::Number(0.1));

    let last = |param: &str| key(&g, "block.person_last_resort", 0, param);
    let pickup = |param: &str| key(&g, "block.person_boat_pickup", 0, param);

    let mut water = AgentSubtype::new("to-the-water", "Makes for the water", DEFAULT_PERSON_GRAPH_ID);
    water.description =
        "Gives up on the refuge for whatever is nearest and survivable, and walks to the \
         shore when a boat lift is coming. The two halves of it are different decisions: \
         the first is what the Mati accounts describe — people who reached open water \
         lived and people who did not were the fatalities — and the second is what Rhodes \
         turned that into by putting boats at the other end. Ships at zero share, and it \
         is the only profile in the model that reduces casualties without anybody \
         arriving to help, which is exactly why it is worth measuring rather than \
         assuming."
            .to_string();
    water.author = "shipped".to_string();
    water.tags = vec!["maritime".into(), "last resort".into()];
    water.share = 0.0;
    water.overrides.insert(last("enabled"), ParamValue::Bool(true));
    water.overrides.insert(pickup("enabled"), ParamValue::Bool(true));

    vec![out, family, water]
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

    // --- two profiles that ship at zero share -------------------------------
    //
    // Both change the model rather than tune it, and both are here because a
    // scenario built against a real disaster asked for something the model
    // could not say. Giving either a share makes the run incomparable with
    // every figure measured before it, which is the comparison, not a caveat.

    let spot = |param: &str| key(&g, "block.spot_fire", 0, param);
    let signal = |param: &str| key(&g, "block.no_signal", 0, param);
    let last = |param: &str| key(&g, "block.last_resort", 0, param);
    let visitors = |param: &str| key(&g, "block.visitors", 0, param);

    let mut visiting = base(
        "holiday-let",
        "Visitors in a let",
        "Nobody here lives here. No car of their own, no idea which road goes where, \
         and a warning that arrives through whoever is running the place rather than \
         over a resident's alert — which makes them the one population that gets *more* \
         reliable when the fire takes the mast out. Rhodes was substantially a crisis \
         about these people, and until this profile the model had none of them: every \
         person in it was drawn from a dwelling.",
    );
    visiting.tags = vec!["transient".into(), "tourist".into()];
    visiting.share = 0.0;
    visiting.capabilities.insert(Capability::Transient, true);
    visiting.capabilities.insert(Capability::Vehicle, false);
    // They act on an instruction readily and on their own judgement late, which
    // is the whole difference: a resident reads a column of smoke over that
    // ridge and knows what it means for this town.
    visiting.overrides.insert(visitors("own_judgement_alarm"), ParamValue::Number(0.55));
    visiting.traits.insert(TraitKey::TrustAuthority, 0.75);
    visiting.traits.insert(TraitKey::RiskPerception, 0.35);
    visiting.traits.insert(TraitKey::PrepTimeMin, 15.0);
    visiting.traits.insert(TraitKey::DefensibleSpace, 0.2);

    let mut reactive = base(
        "reacts-to-events",
        "Reacts to what happens",
        "The same household as \"Wait and see\", with the three branches the real \
         incidents needed switched on: a fire that starts behind them moves them, a \
         warning that never arrives eventually stops being waited for, and a fire on \
         the property with no road left sends them to open ground instead of into the \
         house. None of that is a different kind of person — it is the same person in \
         an incident the model could not previously describe.",
    );
    reactive.tags = vec!["incident".into(), "spotting".into(), "last resort".into()];
    reactive.share = 0.0;
    reactive.overrides.insert(spot("enabled"), ParamValue::Bool(true));
    reactive.overrides.insert(signal("enabled"), ParamValue::Bool(true));
    reactive.overrides.insert(last("enabled"), ParamValue::Bool(true));

    vec![prepared, waiting, defender, assisted, visiting, reactive]
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
         and pump the tank dry before going for water. This baseline is an editable \
         graph like every other policy."
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
    // Off by default so the shipped measurements retain their baseline policy.
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
    lib.graphs.insert(DEFAULT_PERSON_GRAPH_ID.to_string(), default_person_graph());
    for s in default_subtypes()
        .into_iter()
        .chain(default_unit_subtypes())
        .chain(default_person_subtypes())
    {
        lib.subtypes.insert(s.id.clone(), s);
    }
    lib
}
