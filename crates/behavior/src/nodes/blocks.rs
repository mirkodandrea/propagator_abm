//! Compound blocks: one behavioural assumption per box.
//!
//! The primitives in [`logic`](super::logic) can express the shipped evacuation
//! model, and did — in thirty-one nodes, of which nine were observations wired
//! into six arithmetic nodes to build one threshold. That is a graph you can
//! *read* only if you already know what it says, which is the wrong way round:
//! the composer exists so a scientist can change an assumption, and an
//! assumption spread over six boxes is not an assumption, it is an
//! implementation.
//!
//! A block is the same arrangement with the seams closed. It reads what it
//! needs straight off the [`Observation`](crate::Observation) and exposes the
//! numbers the assumption actually turns on as parameters — which is also
//! exactly what a subtype overrides, so a profile stops being a list of node
//! ids and becomes a list of quantities with names.
//!
//! ### Why blocks have almost no input ports
//!
//! A block reading the observation itself rather than through wired
//! observation nodes is what removes most of the boxes. The cost is that its
//! *structure* is fixed: you can change what "alarmed" means numerically, but
//! not what it is computed from. That is the right trade for the level this is
//! pitched at, and the escape hatch is complete — every primitive is still in
//! the palette, and rebuilding a block out of them is supported and documented.
//! Where a block genuinely needs to be told something the observation cannot
//! say, it takes an input — `block.stand_ground` needs the departure decision,
//! which is downstream of it.

use crate::behavior_node;
use crate::value::{IntentValue, Value};

// ---------------------------------------------------------------------------
// Households
// ---------------------------------------------------------------------------

behavior_node! {
    id: "block.alarm",
    name: "Alarm threshold",
    category: Block,
    domain: Household,
    doc: "How much alarm this household needs before it acts, and whether it \
          has that much yet.\n\n\
          The threshold starts at a level set by the plan they came in with, \
          comes down for a household that takes wildfire seriously, and is \
          spread across the population by their individual variation so they do \
          not all leave on the same tick. Their accumulated alarm is then \
          compared against it.\n\n\
          This is the single most consequential number in the evacuation model \
          and the three per-plan levels are the first thing worth varying \
          between profiles.",
    keywords: ["threshold", "depart", "leave", "alarm", "cue", "trigger", "wake"],
    inputs: [],
    outputs: [
        (bool "alarmed", "Their alarm has passed the threshold"),
        (number "threshold", "The level they needed, for a readout"),
        (number "margin", "Alarm minus threshold: how far past it they are")
    ],
    params: [
        (number "leave_early", "Planned to leave", "Alarm needed by a household that always meant to go. Near zero: they go on the first signal.", 0.02, 0.0, 1.0, ""),
        (number "wait_and_see", "Wait and see", "Alarm needed by the undecided majority, which is most of the population and most of the risk.", 0.22, 0.0, 1.0, ""),
        (number "stay_defend", "Planned to stay", "Alarm needed by a household that meant to defend. High, which is why they leave late or not at all.", 0.55, 0.0, 1.0, ""),
        (number "risk_relief", "Risk awareness relief", "How much a fully risk-aware household lowers its own threshold. Applied as relief x (1 - risk perception).", 0.15, 0.0, 1.0, ""),
        (number "spread", "Spread across households", "Width of the individual variation, centred on zero. Zero makes the whole town act on one tick, which is visibly wrong.", 0.10, 0.0, 1.0, "")
    ],
    eval: |ctx, p, _i, out| {
        let h = ctx.household();
        let base = match h.intent {
            IntentValue::LeaveEarly => p.num(0),
            IntentValue::WaitAndSee => p.num(1),
            IntentValue::StayDefend => p.num(2),
        };
        let threshold =
            base + p.num(3) * (1.0 - h.risk_perception) + p.num(4) * (h.jitter - 0.5);
        out.push(Value::Bool(h.cue > threshold));
        out.push(Value::Number(threshold));
        out.push(Value::Number(h.cue - threshold));
    },
}

behavior_node! {
    id: "block.order_response",
    name: "Response to the order",
    category: Block,
    domain: Household,
    doc: "Whether an evacuation order actually moves this household.\n\n\
          Three things have to be true: the order has reached them over their \
          own channel (90 s on a mobile alert, twenty minutes for a household \
          with none), they give official instructions enough weight to act on \
          them, and they were not already committed to staying.\n\n\
          That last clause is the finding the whole evacuation literature turns \
          on, and it is why \"we told them to go\" is not the same as \"they \
          went\". Turn it off with \"Defenders comply\" to model a population \
          that does as it is told, and watch what it does to the casualty count.",
    keywords: ["order", "warning", "trust", "comply", "evacuate", "authority", "alert"],
    inputs: [],
    outputs: [
        (bool "comply", "They will move because of the order"),
        (bool "heard", "The order reached them, believed or not")
    ],
    params: [
        (number "trust_threshold", "Trust needed", "How much weight in official instructions it takes before an order alone moves them.", 0.35, 0.0, 1.0, ""),
        (bool "defenders_comply", "Defenders comply", "Whether a household that planned to defend leaves on an order. False is what the evidence says.", false)
    ],
    eval: |ctx, p, _i, out| {
        let h = ctx.household();
        let believed = h.warning_received && h.trust_authority > p.num(0);
        let exempt = h.intent == IntentValue::StayDefend && !p.boolean(1);
        out.push(Value::Bool(believed && !exempt));
        out.push(Value::Bool(h.warning_received));
    },
}

behavior_node! {
    id: "block.fire_at_the_door",
    name: "Fire on the property",
    category: Block,
    domain: Household,
    doc: "The moment the fire stops being something to think about.\n\n\
          \"Overrun\" is the threat outside passing the level at which standing \
          there is survivable, or the house itself alight — the cue that reaches \
          even a household that had decided to stay. \"Trapped\" is the same \
          moment with no route to a refuge left, which is the case where \
          driving out is the thing that kills them and sheltering is the answer.\n\n\
          Wire \"overrun\" to \"Evacuate now\" and \"trapped\" to \"Shelter in \
          place\", and give shelter the higher priority.",
    keywords: ["overrun", "trapped", "cut off", "threat", "alight", "survivable", "late"],
    inputs: [],
    outputs: [
        (bool "overrun", "Not survivable outside, or the house is alight"),
        (bool "trapped", "The same, with every route to a refuge gone"),
        (number "threat", "The raw threat at the property, for a readout")
    ],
    params: [
        (number "threat_limit", "Not survivable above", "Threat at the property beyond which standing outside is not survivable. The civilians' number is 0.55; 0.35 is the margin firefighters work to.", 0.35, 0.0, 1.0, "")
    ],
    eval: |ctx, p, _i, out| {
        let h = ctx.household();
        let overrun = h.threat > p.num(0) || h.structure_alight;
        out.push(Value::Bool(overrun));
        out.push(Value::Bool(overrun && h.route_blocked));
        out.push(Value::Number(h.threat));
    },
}

behavior_node! {
    id: "block.stand_ground",
    name: "Stand and defend",
    category: Block,
    domain: Household,
    doc: "Whether this household commits to fighting for the property.\n\n\
          Takes the departure decision as an input, because that is the one \
          thing it cannot read for itself: a household defends when it planned \
          to and has not decided to go. Optionally it also needs enough cleared \
          ground around the house for defending to be survivable rather than \
          fatal — which is off by default, because people defend houses they \
          should not.",
    keywords: ["defend", "stay", "property", "fight", "hoses"],
    inputs: [(bool "departing", "Whether they have decided to leave", false)],
    outputs: [(bool "defending", "They commit to defending")],
    params: [
        (number "min_defensible_space", "Cleared ground needed", "Defensible space below which they do not commit. Zero lets anyone defend, which is the realistic setting.", 0.0, 0.0, 1.0, ""),
        (bool "only_if_planned", "Only if they planned to", "Whether defending requires it to have been their stated plan. Off lets any household that has not left end up defending by default.", true)
    ],
    eval: |ctx, p, i, out| {
        let h = ctx.household();
        let planned = h.intent == IntentValue::StayDefend || !p.boolean(1);
        let capable = h.defensible_space >= p.num(0);
        out.push(Value::Bool(planned && capable && !i.boolean(0)));
    },
}

behavior_node! {
    id: "block.preparation",
    name: "Preparation time",
    category: Block,
    domain: Household,
    doc: "How long this household takes to get out of the door, as a multiplier \
          on the milling time the population bake drew for it.\n\n\
          Set per plan, because it is the plan that decides how much there is \
          left to do: a household that meant to leave has packed, one that meant \
          to defend is already outside with the hoses out, and the undecided \
          majority starts from scratch. Wire it into \"Preparation multiplier\".\n\n\
          Milling time is the largest single lever in the whole evacuation \
          model — larger than the warning, larger than the road network.",
    keywords: ["milling", "delay", "prep", "pack", "door", "leave"],
    inputs: [],
    outputs: [(number "multiplier", "Multiplier on their baked milling time")],
    params: [
        (number "leave_early", "Planned to leave", "They are packed. Below 1 shortens it.", 0.8, 0.05, 5.0, "x"),
        (number "wait_and_see", "Wait and see", "The baseline the bake was drawn for.", 1.0, 0.05, 5.0, "x"),
        (number "stay_defend", "Planned to stay", "Already outside, so less to gather — but they left it far too late for that to help.", 0.5, 0.05, 5.0, "x")
    ],
    eval: |ctx, p, _i, out| {
        let m = match ctx.household().intent {
            IntentValue::LeaveEarly => p.num(0),
            IntentValue::WaitAndSee => p.num(1),
            IntentValue::StayDefend => p.num(2),
        };
        out.push(Value::Number(m));
    },
}
