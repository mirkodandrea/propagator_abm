//! Compound blocks for suppression units.
//!
//! Two of them carry the whole hand-written policy that used to live in
//! `abm::suppression::step_ground` as a pair of `if`s: the safety margin a unit
//! disengages at, and the point at which it breaks off for water. Those two
//! numbers — 0.35 of threat, and a dry tank — were constants in Rust, which
//! meant "what if crews accepted more risk?" was a question you answered by
//! editing the model and rebuilding.
//!
//! The third is the one thing the hand-written layer had no answer for at all:
//! a unit that has been sent somewhere it cannot usefully work.

use crate::behavior_node;
use crate::value::{UnitKindKey, Value};

behavior_node! {
    id: "block.unit_safety",
    name: "Safety margin",
    category: Block,
    domain: SuppressionUnit,
    doc: "The point at which this unit stops working and pulls back.\n\n\
          Firefighters are not civilians who happen to be braver; they are \
          people working to a stated margin, and they disengage while they still \
          can. The shipped limit is 0.35 of threat against the civilians' 0.55, \
          and there is a second trigger on accumulated heat so a unit that has \
          been taking punishment leaves before it has taken all of it.\n\n\
          Per-kind limits are here because an aircraft overflying a front is not \
          in the same danger as a crew standing in it. Wire \"withdraw\" \
          straight to the Withdraw action and give it the highest priority in \
          the graph — safety overrides orders, and that is the one rule the \
          model will not let a player break.",
    keywords: ["safety", "withdraw", "threat", "limit", "burnover", "disengage", "pull back"],
    inputs: [],
    outputs: [
        (bool "withdraw", "This unit should pull back now"),
        (number "margin", "Threat minus the limit: how far past it they are"),
        (number "limit", "The limit in force for this unit, for a readout")
    ],
    params: [
        (number "hand_crew_limit", "Hand crew limit", "Threat above which a squadra on foot disengages.", 0.35, 0.0, 1.0, ""),
        (number "engine_limit", "Engine limit", "Threat above which an engine disengages. The crew is with the vehicle, so this is not much higher.", 0.35, 0.0, 1.0, ""),
        (number "air_limit", "Air tanker limit", "Threat above which an aircraft breaks off. Effectively never, since it is not on the ground.", 1.0, 0.0, 1.0, ""),
        (number "heat_limit", "Accumulated heat limit", "Fraction of survivable exposure at which the unit leaves regardless of the current threat. 1.0 disables it, which means leaving only when it is already too late.", 0.5, 0.0, 1.0, "")
    ],
    eval: |ctx, p, _i, out| {
        let u = ctx.unit();
        let limit = match u.kind {
            UnitKindKey::HandCrew => p.num(0),
            UnitKindKey::Engine => p.num(1),
            UnitKindKey::AirTanker => p.num(2),
        };
        let hot = u.heat_fraction >= p.num(3);
        out.push(Value::Bool(u.threat_here >= limit || hot));
        out.push(Value::Number(u.threat_here - limit));
        out.push(Value::Number(limit));
    },
}

behavior_node! {
    id: "block.unit_resupply",
    name: "Break off for water",
    category: Block,
    domain: SuppressionUnit,
    doc: "When a unit that carries water goes for more.\n\n\
          The shipped model pumps the tank dry and only then drives to a \
          hydrant, which costs the round trip at the worst possible moment. \
          Raise \"Refill below\" and the engine leaves earlier with water still \
          in the tank, which is what a real crew boss does and is a genuinely \
          open question in this model: 2,500 L is six minutes of pumping and the \
          hydrant round trip is longer than that.\n\n\
          Never fires for a hand crew, which carries no water. A dry unit still \
          cannot pump whatever this says — that is physics, and it is enforced \
          below the behaviour layer.",
    keywords: ["water", "tank", "hydrant", "refill", "resupply", "empty", "shuttle"],
    inputs: [],
    outputs: [
        (bool "refill", "Break off and go for water"),
        (number "remaining", "Tank remaining, 0-1, for a readout")
    ],
    params: [
        (number "refill_below", "Refill at or below", "Tank fraction at which to break off. Zero reproduces the shipped model: pump dry, then go.", 0.0, 0.0, 1.0, ""),
        (bool "only_when_working", "Only once on scene", "Whether to wait until the unit has actually reached its task before breaking off. On keeps a unit from turning round mid-response.", true)
    ],
    eval: |ctx, p, _i, out| {
        let u = ctx.unit();
        let on_scene = u.is_working || !p.boolean(1);
        // Inclusive, so that the shipped setting of zero means "when it is dry"
        // rather than "never" — an always-negative that would make the whole
        // block look like it was working while doing nothing at all.
        out.push(Value::Bool(u.carries_water && on_scene && u.water_fraction <= p.num(0)));
        out.push(Value::Number(u.water_fraction));
    },
}

behavior_node! {
    id: "block.unit_futile",
    name: "Nothing useful here",
    category: Block,
    domain: SuppressionUnit,
    doc: "A unit that has been sent somewhere it cannot do any good.\n\n\
          Three ways that happens, and all three are ordinary rather than \
          exceptional: the order points somewhere the road network does not \
          reach, so an engine is parked short of it; there is no unburnt \
          burnable fuel left within reach, so there is nothing to wet or cut; or \
          the unit has been sitting on the same order long enough that it was \
          plainly the wrong one.\n\n\
          The shipped model's answer to all of these was a note in the panel and \
          a unit that kept standing there. Wire this to Hold position or Return \
          to staging and the unit says so by moving.",
    keywords: ["stranded", "useless", "stuck", "unreachable", "idle", "waste", "give up"],
    inputs: [],
    outputs: [
        (bool "futile", "There is nothing worth doing here"),
        (bool "stranded", "The ordered point is not reachable at all")
    ],
    params: [
        (bool "on_unreachable", "When the order is out of reach", "Count an ordered point the network cannot reach as futile. Off leaves an engine working the roadside where it stopped, which is what it does today.", false),
        (bool "on_no_targets", "When there is nothing to work", "Count having no unburnt burnable fuel within reach as futile.", true),
        (number "give_up_after_min", "Give up after", "Minutes on one order after which it counts as futile regardless. Zero disables it.", 0.0, 0.0, 120.0, " min")
    ],
    eval: |ctx, p, _i, out| {
        let u = ctx.unit();
        let stranded = u.has_task && !u.task_reachable;
        let stale = p.num(2) > 0.0 && u.minutes_on_task >= p.num(2);
        let futile = (p.boolean(0) && stranded)
            || (p.boolean(1) && u.is_working && !u.work_available)
            || stale;
        out.push(Value::Bool(futile));
        out.push(Value::Bool(stranded));
    },
}
