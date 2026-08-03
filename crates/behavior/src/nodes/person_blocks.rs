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

// ---------------------------------------------------------------------------
// Somewhere to go that is not a refuge
// ---------------------------------------------------------------------------
//
// A separated person's whole action set used to be "the refuge, the house, or
// stand still". Both loss-of-life scenarios in `docs/behavior-gaps.md` turn on
// the fourth thing: somewhere near that is not on the evacuation plan. The
// people who reached open water at Mati lived and the ones who did not were the
// fatalities, and neither outcome was expressible here.

behavior_node! {
    id: "block.person_spot_fire",
    name: "A fire between them and where they are going",
    category: Block,
    domain: Person,
    doc: "Whether a fire has started somewhere it was not, near enough and \
          recently enough for someone already walking to change their mind.\n\n\
          The same block the households get, and for someone out on the road it \
          is the worse of the two: the front is where they last saw it, and an \
          ember jump lands between them and the refuge they are walking to. \
          Their route field will catch up within the minute — that is finding 8 \
          working — but a person watching it start does not wait for the routing \
          refresh, and until now the model had them do exactly that.",
    keywords: ["ember", "spot", "spotting", "firebrand", "cut off", "ahead", "jump"],
    inputs: [],
    outputs: [
        (bool "spotted", "A new fire, near enough and recent enough to act on"),
        (number "distance", "Metres to it, for a readout")
    ],
    params: [
        (number "radius_m", "Near enough", "Metres within which a new fire is this person's problem.", 600.0, 0.0, 2500.0, "m"),
        (number "recent_min", "Recent enough", "Minutes after which it stops being news.", 15.0, 0.0, 180.0, "min")
    ],
    eval: |ctx, p, _i, out| {
        let s = ctx.person();
        let spotted =
            s.spot_fire_distance_m <= p.num(0) && s.spot_fire_age_min <= p.num(1);
        out.push(Value::Bool(spotted));
        out.push(Value::Number(s.spot_fire_distance_m));
    },
}

behavior_node! {
    id: "block.person_last_resort",
    name: "Somewhere nearer than the refuge",
    category: Block,
    domain: Person,
    doc: "Whether this person gives up on the refuge and makes for the nearest \
          open ground instead.\n\n\
          A refuge is somewhere an evacuation is organised; open ground is \
          somewhere the fire cannot reach you. They are usually not the same \
          place and the second is usually much nearer, and the difference \
          between them is the gap people on foot die in — a lane that ends at a \
          beach three hundred metres away, walked past on the way to an assembly \
          point two kilometres off.\n\n\
          Ships off. Turning it on is the comparison worth running, because it \
          is the one thing in this model that reduces casualties without \
          anybody arriving to help.\n\n\
          Takes the moment as an input for the same reason its household \
          counterpart does: \"Caught in the open\" already owns the threshold \
          for when being out in it stops working, and a second copy of that \
          number here would be one to keep in step and one to get wrong.",
    keywords: ["clearing", "open ground", "beach", "shore", "last resort", "nearer"],
    inputs: [(bool "caught out", "Wire \"Caught in the open\"'s overrun output in", false)],
    outputs: [
        (bool "open ground", "Make for the nearest survivable clearing"),
        (bool "the shore", "Make for the water's edge instead"),
        (number "distance", "Metres to whichever it chose, for a readout")
    ],
    params: [
        (bool "enabled", "Enabled", "Whether this fires at all. Off is the shipped model: someone with no route left stops where they stand.", false),
        (bool "also_when_cut_off", "Also when the route is cut", "Whether a blocked route counts on its own, without the threat being bad yet. On: a road that ends in the fire is a reason to turn aside before it is a reason to stop.", true),
        (number "max_walk_m", "Furthest they will walk", "Straight-line metres to open ground beyond which it is not worth turning aside for.", 500.0, 0.0, 5000.0, "m"),
        (bool "only_with_lift", "Shore only with a lift", "Whether the water's edge counts only when a boat pickup is coming. On is the honest setting: standing in the sea is surviving, not evacuating. Note the household block's equivalent needs the same answer, and for the same reason.", true),
        (number "lift_within_min", "Lift close enough", "Minutes until the pickup, within which making for the shore is worth it.", 30.0, 0.0, 180.0, "min")
    ],
    eval: |ctx, p, i, out| {
        let s = ctx.person();
        let trigger = p.boolean(0) && (i.boolean(0) || (p.boolean(1) && s.route_blocked));
        let lift = !p.boolean(3) || s.boat_lift_min <= p.num(4);
        let shore = trigger && s.shore_distance_m <= p.num(2) && lift;
        let ground = trigger && s.open_ground_distance_m <= p.num(2) && !shore;
        out.push(Value::Bool(ground));
        out.push(Value::Bool(shore));
        let d = if shore { s.shore_distance_m } else { s.open_ground_distance_m };
        out.push(Value::Number(if d.is_finite() { d } else { 9999.0 }));
    },
}

behavior_node! {
    id: "block.person_boat_pickup",
    name: "Boats are coming",
    category: Block,
    domain: Person,
    doc: "Whether this person makes for the shore because there is a lift at \
          the other end of it.\n\n\
          The Rhodes case, and the reason it is the success story of the three: \
          when the road network could not clear the area in time, the evacuation \
          used a capacity that was not the road network. A person walks to the \
          water because someone told them boats would be there, which is a \
          different decision from walking to the water because there is nowhere \
          else left — that one is \"Somewhere nearer than the refuge\".\n\n\
          Unlike that block this one does not need things to be going badly. It \
          fires on the announcement, which is what an organised maritime \
          evacuation actually is.",
    keywords: ["boat", "sea", "coastguard", "lift", "pickup", "ferry", "rhodes"],
    inputs: [],
    outputs: [
        (bool "makes for the shore", "They walk to the pickup"),
        (number "minutes", "Minutes until it is on station, for a readout")
    ],
    params: [
        (bool "enabled", "Enabled", "Whether this fires at all. Off is the shipped model, which has no maritime evacuation in it.", false),
        (number "within_min", "Announced within", "Minutes until the pickup, within which it is worth walking to the shore for. Larger than the crossing time, because people gather before the boat arrives.", 25.0, 0.0, 180.0, "min"),
        (number "max_shore_distance_m", "Furthest they will walk", "Straight-line metres to the water beyond which they take the road instead.", 1500.0, 0.0, 10000.0, "m"),
        (bool "even_if_refuge_nearer", "Even when the refuge is nearer", "Whether the pickup wins over a nearer land refuge. On is what an announced evacuation produces; off models people who only take the boat when it is the closest thing.", true)
    ],
    eval: |ctx, p, _i, out| {
        let s = ctx.person();
        let nearer = p.boolean(3) || s.shore_distance_m <= s.refuge_distance_m;
        let go = p.boolean(0)
            && s.boat_lift_min <= p.num(1)
            && s.shore_distance_m <= p.num(2)
            && nearer;
        out.push(Value::Bool(go));
        out.push(Value::Number(s.boat_lift_min.min(9999.0)));
    },
}
