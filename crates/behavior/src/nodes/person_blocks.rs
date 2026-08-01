//! Compound blocks for a separated person.
//!
//! Three, because there are only three questions a person out on their own gets
//! to answer: is it still worth walking, is it still survivable to walk, and is
//! there anyone back at the house worth turning round for. Everything else about
//! them — where the refuge is, how fast they walk, whether the smoke kills them —
//! is the model's.
//!
//! The reunification block is the one that matters. Going back for family is the
//! behaviour that turns up in every post-fire interview and in no evacuation
//! plan, and until now this model could not express it at all: people who were
//! out walked to a refuge and that was the whole of it. It ships **off**, gated
//! by its own `enabled` parameter, because turning it on changes the casualty
//! count and every measurement in `crates/fire/tests` was taken with it off.

use crate::behavior_node;
use crate::value::Value;

behavior_node! {
    id: "block.person_exposure",
    name: "Caught in the open",
    category: Block,
    domain: Person,
    doc: "Whether it is still survivable to be walking, and whether there is \
          anywhere left to walk to.\n\n\
          \"Overrun\" is the threat where they stand passing the level a person \
          survives in the open, or their accumulated heat load getting close to \
          what kills them. \"No way out\" is the same moment with every route to \
          a refuge gone — which is the case where standing still and taking what \
          cover there is beats carrying on down a road that ends in the fire.\n\n\
          Wire \"no way out\" to \"Shelter where they are\" and give it the \
          highest priority in the graph.",
    keywords: ["overrun", "trapped", "cut off", "threat", "survivable", "heat", "open"],
    inputs: [],
    outputs: [
        (bool "overrun", "Not survivable to be out in this"),
        (bool "no way out", "The same, with every route to a refuge gone"),
        (number "margin", "Threat minus the limit: how far past it they are")
    ],
    params: [
        (number "threat_limit", "Not survivable above", "Threat in the open beyond which a person on foot is not surviving it. The civilians' number is 0.55; the firefighters work to 0.35 with equipment this person does not have.", 0.55, 0.0, 1.0, ""),
        (number "heat_limit", "Heat load limit", "Accumulated exposure, as a fraction of lethal, past which they stop walking whatever the threat in front of them reads. 1.0 disables it — which is what the model did before this block existed.", 0.6, 0.0, 1.0, "")
    ],
    eval: |ctx, p, _i, out| {
        let s = ctx.person();
        let overrun = s.threat > p.num(0) || s.heat_fraction >= p.num(1);
        out.push(Value::Bool(overrun));
        out.push(Value::Bool(overrun && s.route_blocked));
        out.push(Value::Number(s.threat - p.num(0)));
    },
}

behavior_node! {
    id: "block.person_walk_out",
    name: "Set off for the refuge",
    category: Block,
    domain: Person,
    doc: "Whether this person makes for the nearest refuge on foot.\n\n\
          The shipped answer is \"immediately, always\", which is what the model \
          has always done with people who were out when it started and is what \
          the guidance says they should do. The parameters are here so that \
          answer can be questioned: raise \"Alarm needed\" and they mill in the \
          street like everyone else, and turn off \"An order is enough\" to see \
          what a broadcast on its own is worth.\n\n\
          Leaving \"Alarm needed\" at zero makes this fire on every tick, which \
          is deliberate and is the shipped behaviour.",
    keywords: ["walk", "refuge", "leave", "out", "evacuate", "set off", "threshold"],
    inputs: [],
    outputs: [
        (bool "walking out", "They make for the refuge"),
        (number "threshold", "The alarm they needed, for a readout")
    ],
    params: [
        (number "alarm_needed", "Alarm needed", "Alarm this person needs before they set off at all. Zero is the shipped behaviour: they go straight away.", 0.0, 0.0, 1.0, ""),
        (bool "order_is_enough", "An order is enough", "Whether a public evacuation order sends them regardless of their own alarm.", true),
        (number "spread", "Spread across people", "Width of the individual variation on the threshold, centred on zero. Zero makes everyone act on the same tick.", 0.0, 0.0, 1.0, "")
    ],
    eval: |ctx, p, _i, out| {
        let s = ctx.person();
        let threshold = p.num(0) + p.num(2) * (s.jitter - 0.5);
        // Inclusive, because the shipped setting sits on the bottom of the
        // range: a strict comparison at 0.0 would be a branch that never fires.
        let alarmed = s.cue >= threshold;
        out.push(Value::Bool(alarmed || (p.boolean(1) && s.order_issued)));
        out.push(Value::Number(threshold));
    },
}

behavior_node! {
    id: "block.person_reunite",
    name: "Going back for the family",
    category: Block,
    domain: Person,
    doc: "Whether this person turns round and walks back to the house.\n\n\
          Off by default, and it is the single most consequential switch in this \
          domain. Every post-fire study finds people doing this and no \
          evacuation plan assumes it; the model could not express it at all \
          until now, so every casualty figure the game has ever printed was \
          taken with it off.\n\n\
          The conditions are the ones the interviews describe: someone is still \
          at the house, the house is still there, home is not much further than \
          safety is, and the way back is not already alight. Loosen any of them \
          and watch what it costs.",
    keywords: ["family", "reunification", "home", "back", "children", "return"],
    inputs: [],
    outputs: [
        (bool "heads home", "They turn round for the house"),
        (number "detour", "Home distance over refuge distance, for a readout")
    ],
    params: [
        (bool "enabled", "Enabled", "Whether this behaviour happens at all. Off is the shipped model, and the measurements in the fire tests were taken with it off.", false),
        (bool "only_if_family_home", "Only if someone is there", "Whether they need the family to still be at the house. Off models someone who does not know the family has already gone, which is the realistic and the lethal case.", true),
        (number "max_home_distance_m", "Furthest they will go", "Metres back to the house beyond which they do not attempt it.", 1200.0, 0.0, 5000.0, "m"),
        (number "max_detour", "Furthest out of their way", "Home distance as a multiple of refuge distance, beyond which safety wins. 1.0 means only when home is nearer than the refuge.", 1.5, 0.1, 10.0, "x"),
        (number "max_threat", "Will not walk into", "Threat where they stand above which they give up on it and make for safety instead.", 0.30, 0.0, 1.0, "")
    ],
    eval: |ctx, p, _i, out| {
        let s = ctx.person();
        let detour = if s.refuge_distance_m > 1.0 {
            s.home_distance_m / s.refuge_distance_m
        } else {
            f32::INFINITY
        };
        let someone_there = s.household_at_home || !p.boolean(1);
        let heads_home = p.boolean(0)
            && someone_there
            && !s.household_safe
            && !s.home_alight
            && s.home_distance_m.is_finite()
            && s.home_distance_m <= p.num(2)
            && detour <= p.num(3)
            && s.threat <= p.num(4);
        out.push(Value::Bool(heads_home));
        out.push(Value::Number(if detour.is_finite() { detour } else { 999.0 }));
    },
}
