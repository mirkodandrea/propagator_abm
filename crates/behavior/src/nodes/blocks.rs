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
    id: "block.order_confirmation",
    name: "Do they check first?",
    category: Block,
    domain: Household,
    doc: "What a household does between hearing an order and acting on it.\n\n\
          The shipped model has them act on the first word they hear, and that \
          is the one part of the warning sequence every study of it says does \
          not happen. People who are told to go look out of the window, ring \
          somebody, walk to the end of the road, wait for it to be said again — \
          and *then* go. The order does not start the evacuation, it starts the \
          checking.\n\n\
          Two things end the checking. Their own senses confirm it — alarm at \
          or past \"Alarm that confirms it\", or the fire close enough to see — \
          and they go at once. Or nothing confirms it and they go anyway once \
          they have spent \"Time spent checking\" on it, which is the household \
          that asked around and found nobody who knew any better.\n\n\
          This does not make anyone refuse: trust in \"Response to the order\" \
          is what decides *whether*, and this decides *when*. Turning it on \
          moves the departure curve to the right and stretches it out, which is \
          the shape every real evacuation has and the shipped one does not.",
    keywords: ["confirm", "milling", "check", "delay", "believe", "order", "warning", "second source"],
    inputs: [(bool "told", "Whether the order alone would move them", false)],
    outputs: [
        (bool "acts", "They act on the order now"),
        (bool "still checking", "They believe it and have not gone yet"),
        (number "wait_min", "Minutes of checking this household will do, for a readout")
    ],
    params: [
        (bool "enabled", "Enabled", "Whether checking happens at all. Off passes the order straight through, which is the shipped model and every figure measured on it.", false),
        (number "milling_min", "Time spent checking", "Minutes between being told and acting, for a household nothing else confirms it to.", 12.0, 0.0, 120.0, "min"),
        (number "confirm_alarm", "Alarm that confirms it", "Their own alarm at or above which the order needs no further confirmation and they go at once.", 0.25, 0.0, 1.0, ""),
        (number "confirm_within_m", "Fire near enough to confirm it", "Seeing fire this close is confirmation on its own, whatever their alarm says.", 1000.0, 0.0, 2500.0, "m"),
        (number "spread", "Spread across households", "Width of the individual variation on the checking time, centred on zero. Zero makes every household that was told at the same moment leave at the same moment.", 6.0, 0.0, 60.0, "min")
    ],
    eval: |ctx, p, i, out| {
        let h = ctx.household();
        let told = i.boolean(0);
        // Inclusive on both, because a profile that wants "any alarm at all
        // confirms it" sets the threshold to zero and a strict test there is a
        // branch that never fires (finding 26).
        let corroborated = h.cue >= p.num(2) || h.fire_distance_m <= p.num(3);
        // The clock runs from when they were *told*, not from when the order
        // was issued: a household with no channel hears twenty minutes late and
        // its checking starts then, which is the whole reason
        // `minutes_since_warning` exists.
        let wait = (p.num(1) + p.num(4) * (h.jitter - 0.5)).max(0.0);
        let waited = h.minutes_since_warning >= wait;
        let done = !p.boolean(0) || corroborated || waited;
        out.push(Value::Bool(told && done));
        out.push(Value::Bool(told && p.boolean(0) && !done));
        out.push(Value::Number(if p.boolean(0) { wait } else { 0.0 }));
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

// ---------------------------------------------------------------------------
// What the incident itself can break
// ---------------------------------------------------------------------------
//
// Five blocks, and none of them existed until three scenarios were built against
// real disasters and the model turned out to have nothing to say about what
// actually killed people there. Each one names its incident in its doc, because
// a threshold with a source is a different object from a threshold someone
// picked: see `docs/behavior-gaps.md`.
//
// Three of the five carry an `enabled` switch and ship on the default canvas
// with it off, for the reason `block.person_reunite` does: turning one on
// changes the casualty figures, and every measurement in `crates/fire/tests` was
// taken with them off. The two without a switch are not on the shipped canvas at
// all, so placing one is already the decision — a block that does nothing until
// you find a second switch is the trap finding 26 is about.

behavior_node! {
    id: "block.spot_fire",
    name: "A fire behind you",
    category: Block,
    domain: Household,
    doc: "Whether a fire has started somewhere it was not, near enough and \
          recently enough to change what this household does.\n\n\
          The core genuinely models spotting: embers land ahead of the front and \
          ignite cells that are not contiguous with it. Until this block, \
          nothing a household could read said so — their distance-to-fire is a \
          field over the *whole* burning mask, so a new fire 400 m behind them \
          reads exactly like the front creeping 400 m closer, and the two are \
          not the same event at all. The Pedrogao Grande and Mati accounts are \
          both descriptions of the second one: fire behind you that was not \
          there when you left, on the road you were relying on.\n\n\
          Age matters as much as distance. A spot fire twenty minutes old is \
          part of the landscape and already in everyone's threat field; one from \
          two minutes ago is the reason to go now.",
    keywords: ["ember", "spot", "spotting", "firebrand", "behind", "cut off", "jump"],
    inputs: [],
    outputs: [
        (bool "spotted", "A new fire, near enough and recent enough to act on"),
        (number "distance", "Metres to it, for a readout"),
        (number "age", "Minutes since it started, for a readout")
    ],
    params: [
        (bool "enabled", "Enabled", "Whether this changes the decision at all. Off is the shipped model, and every figure in the fire tests was measured with it off.", false),
        (number "radius_m", "Near enough", "Metres within which a new fire is this household's problem rather than the incident's.", 800.0, 0.0, 2500.0, "m"),
        (number "recent_min", "Recent enough", "Minutes after which it stops being news. Past this it is just part of the fire.", 20.0, 0.0, 180.0, "min")
    ],
    eval: |ctx, p, _i, out| {
        let h = ctx.household();
        // Inclusive on both, because a scenario with the radius set to the
        // saturation distance or the window to zero is a legitimate setting and
        // a strict comparison there is a branch that never fires.
        let spotted =
            p.boolean(0) && h.spot_fire_distance_m <= p.num(1) && h.spot_fire_age_min <= p.num(2);
        out.push(Value::Bool(spotted));
        out.push(Value::Number(h.spot_fire_distance_m));
        out.push(Value::Number(h.spot_fire_age_min.min(9999.0)));
    },
}

behavior_node! {
    id: "block.no_signal",
    name: "No warning is coming",
    category: Block,
    domain: Household,
    doc: "What a household does when the network that was going to warn them is \
          down.\n\n\
          Every household draws its own warning channel at generation time and \
          its own delay follows from that, so in the shipped model everybody's \
          warning is late for a private reason. Real failures are not private: \
          reporting on Pedrogao Grande cites the fire knocking out \
          communications as a contributing cause of the death toll — one mast, \
          everyone under it, at the moment it mattered most.\n\n\
          The interesting half is not the outage, it is who it hurts. A \
          household that trusts official instructions is *waiting* for a message \
          that will never arrive, and the more they trust it the longer they \
          wait. \"Giving up on it\" is the moment they stop waiting and act on \
          what they can see.",
    keywords: ["network", "mast", "phone", "outage", "alert", "correlated", "signal"],
    inputs: [],
    outputs: [
        (bool "no signal", "The network covering this house is down"),
        (bool "giving up on it", "They stop waiting for the message and act")
    ],
    params: [
        (bool "enabled", "Enabled", "Whether \"giving up on it\" ever fires. Off is the shipped model. \"No signal\" is reported either way.", false),
        (number "patience_min", "How long they wait", "Minutes after the order was issued before a household that expected to be told acts without having been.", 10.0, 0.0, 120.0, "min"),
        (number "min_trust", "Trust that makes them wait", "Trust in authority above which a household is waiting for an instruction rather than making its own mind up. Below this they were never waiting, so there is nothing to give up on.", 0.35, 0.0, 1.0, "")
    ],
    eval: |ctx, p, _i, out| {
        let h = ctx.household();
        let waiting = h.comms_down
            && h.order_issued
            && !h.warning_received
            && h.trust_authority >= p.num(2)
            && h.minutes_since_order >= p.num(1);
        out.push(Value::Bool(h.comms_down));
        out.push(Value::Bool(p.boolean(0) && waiting));
    },
}

behavior_node! {
    id: "block.road_closed",
    name: "The road out is closed",
    category: Block,
    domain: Household,
    doc: "What a household does when the road it would have driven out on has \
          been closed to traffic by order.\n\n\
          Investigators found the police failed to close the N236 in time at \
          Pedrogao Grande, and gave it as a specific reason the toll there was \
          as high as it was. The closure is the commander's lever and it is not \
          free: it takes a road away from everyone on it, and a household with a \
          car and no road is a household on foot.\n\n\
          \"Will not walk it\" is the other half, and it is the cost of the \
          lever: past a distance people do not set off on foot at all, they stay \
          in the house and wait for the road to reopen. Wire it somewhere \
          visible.",
    keywords: ["closure", "police", "barricade", "traffic", "roadblock", "walk"],
    inputs: [],
    outputs: [
        (bool "closed", "Their way out is closed to traffic"),
        (bool "on foot instead", "They will walk it"),
        (bool "will not walk it", "Too far to walk: they stay put")
    ],
    params: [
        (number "will_walk_within", "Furthest they will walk", "Refuge distance, on the routing field's own scale, beyond which a household with no usable road stays in the house instead of setting off.", 3000.0, 0.0, 20000.0, "")
    ],
    eval: |ctx, p, _i, out| {
        let h = ctx.household();
        let stranded = h.road_closed && h.has_vehicle && !h.is_moving;
        let within = h.refuge_distance_m <= p.num(0);
        out.push(Value::Bool(h.road_closed));
        out.push(Value::Bool(stranded && within));
        out.push(Value::Bool(stranded && !within));
    },
}

behavior_node! {
    id: "block.visitors",
    name: "Visitors, not residents",
    category: Block,
    domain: Household,
    doc: "What a party staying in a hotel, a let or a campsite does, as opposed \
          to a family in its own house.\n\n\
          Rhodes was substantially a crisis about people who did not live there: \
          no car of their own, no idea which road goes where, and a warning that \
          arrives through whoever is running the place rather than over a \
          resident's alert. The model represents them as households with the \
          visitor capability set, so a profile assigns them the same way it \
          assigns anything else — see the `holiday-let` profile, which ships at \
          zero share.\n\n\
          The number that matters is \"Acts on their own\": a resident reads a \
          column of smoke over that ridge and knows what it means for this town, \
          and a visitor does not, so their threshold for moving without being \
          told is higher. That is the whole difference, and it is why they died \
          in the places they did.",
    keywords: ["tourist", "hotel", "transient", "guest", "holiday", "campsite"],
    inputs: [],
    outputs: [
        (bool "visitor", "Nobody here is at home"),
        (bool "acts on their own", "Alarmed enough to move without being told"),
        (bool "on foot", "A visitor with no vehicle")
    ],
    params: [
        (number "own_judgement_alarm", "Acts on their own above", "Alarm a visitor needs before moving without being told. Higher than a resident's threshold, because they cannot read what they are looking at.", 0.60, 0.0, 1.0, "")
    ],
    eval: |ctx, p, _i, out| {
        let h = ctx.household();
        out.push(Value::Bool(h.is_visitor));
        out.push(Value::Bool(h.is_visitor && h.cue >= p.num(0)));
        out.push(Value::Bool(h.is_visitor && !h.has_vehicle));
    },
}

behavior_node! {
    id: "block.last_resort",
    name: "Nowhere left to drive",
    category: Block,
    domain: Household,
    doc: "Where a household goes when the fire is on the property and there is \
          no route out: their own house, or open ground.\n\n\
          The shipped answer is the house, and it is a real answer — walls buy \
          about ten times as long as standing outside, which is why sheltering \
          in place is a policy and not a euphemism. It is not the only answer \
          people actually take. Mati's fatalities include people who died in \
          lanes trying to reach the shoreline, and the ones who reached open \
          water lived; Rhodes moved thousands off beaches. Until this block the \
          model had nowhere to send them: a household whose evacuation failed \
          could shelter at home or die on the road, and the car park two streets \
          away did not exist.\n\n\
          \"The shore\" is deliberately gated on a lift being on the way. \
          Standing in the sea is survivable and is not an evacuation, and a \
          model that treats reaching the water as reaching safety says Mati was \
          a success.\n\n\
          This block decides **where**, not whether, which is why it takes the \
          moment as an input rather than testing a threshold of its own. Wire \
          \"Fire on the property\"'s *overrun* into it and the two branches \
          cannot disagree about when the fire has arrived. A second threshold \
          here would have been the more obvious design and it would have been \
          wrong twice over: it duplicates a number that is already a parameter \
          somewhere else, and — measured rather than guessed — the threat at a \
          house on this calibration barely reaches 0.3 in a two-hour incident, \
          so a fresh 0.35 threshold is a branch that never fires. Houses stand \
          on non-vegetated ground, which is exactly why they never burn in the \
          CA either.",
    keywords: ["last resort", "open ground", "beach", "shore", "clearing", "trapped", "sea"],
    inputs: [(bool "the fire is here", "Wire \"Fire on the property\"'s overrun output in", false)],
    outputs: [
        (bool "open ground", "Make for the nearest survivable clearing"),
        (bool "the shore", "Make for the water's edge instead"),
        (number "distance", "Metres to whichever it chose, for a readout")
    ],
    params: [
        (bool "enabled", "Enabled", "Whether this fires at all. Off is the shipped model: a household with the fire on it drives out if it can and shelters in the house if it cannot.", false),
        (number "max_walk_m", "Furthest they will walk", "Straight-line metres to open ground beyond which they do not attempt it and stay in the house.", 600.0, 0.0, 5000.0, "m"),
        (bool "only_when_cut_off", "Only when the road is cut", "Whether this also needs every route to a refuge to be gone. Off, because on this calibration that condition essentially never occurs and requiring it is a branch that never fires.", false),
        (bool "only_with_lift", "Shore only with a lift", "Whether the water's edge counts only when a boat pickup is coming. On is the honest setting: without one, reaching the sea is surviving, not evacuating.", true),
        (number "lift_within_min", "Lift close enough", "Minutes until the pickup, within which making for the shore is worth it.", 30.0, 0.0, 180.0, "min")
    ],
    eval: |ctx, p, i, out| {
        let h = ctx.household();
        let trigger = p.boolean(0)
            && i.boolean(0)
            // Somebody already on the road has made their choice; this is about
            // the household the fire reached at home.
            && !h.is_moving
            && (!p.boolean(2) || h.route_blocked);
        let lift = !p.boolean(3) || h.boat_lift_min <= p.num(4);
        let shore = trigger && h.shore_distance_m <= p.num(1) && lift;
        // The shore wins when both are reachable: it is the one with a way out
        // of the incident at the end of it.
        let ground = trigger && h.open_ground_distance_m <= p.num(1) && !shore;
        out.push(Value::Bool(ground));
        out.push(Value::Bool(shore));
        let d = if shore { h.shore_distance_m } else { h.open_ground_distance_m };
        out.push(Value::Number(if d.is_finite() { d } else { 9999.0 }));
    },
}
