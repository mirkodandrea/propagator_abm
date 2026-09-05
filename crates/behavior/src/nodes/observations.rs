//! One node per field of [`HouseholdObs`](crate::HouseholdObs).
//!
//! Exposing a new civilian capability to the editor is a one-line addition
//! here: add the field to `HouseholdObs`, add the line, and the palette has it.
//! The docs are the ones a scientist reads on hover, so they say what the
//! quantity *is* rather than restating the field name.
//!
//! Every node here declares `domain: Household`, which is what keeps it out of
//! a suppression unit's palette and out of a suppression graph.

use crate::behavior_node;
use crate::value::Value;

/// A number read straight off the household observation.
macro_rules! obs_number {
    ($id:literal, $name:literal, $doc:literal, [$($kw:literal),* $(,)?], $field:ident) => {
        behavior_node! {
            id: $id,
            name: $name,
            category: Observation,
            domain: Household,
            doc: $doc,
            keywords: [$($kw),*],
            inputs: [],
            outputs: [(number "value", $doc)],
            params: [],
            eval: |ctx, _p, _i, out| out.push(Value::Number(ctx.household().$field)),
        }
    };
}

/// A condition read straight off the household observation.
macro_rules! obs_bool {
    ($id:literal, $name:literal, $doc:literal, [$($kw:literal),* $(,)?], $field:ident) => {
        behavior_node! {
            id: $id,
            name: $name,
            category: Observation,
            domain: Household,
            doc: $doc,
            keywords: [$($kw),*],
            inputs: [],
            outputs: [(bool "value", $doc)],
            params: [],
            eval: |ctx, _p, _i, out| out.push(Value::Bool(ctx.household().$field)),
        }
    };
}

// --- the incident ----------------------------------------------------------

obs_number!(
    "obs.time_min",
    "Time",
    "Simulated minutes since the incident started.",
    ["clock", "elapsed", "minutes"],
    time_min
);

obs_number!(
    "obs.threat",
    "Threat at home",
    "Survivability at the house, 0-1. At 1.0 it is not survivable to stand \
     outside. Instantaneous, not integrated: this is the pedestrian-scale \
     field, not the one that destroys structures.",
    ["danger", "heat", "survivable", "radiant"],
    threat
);

obs_number!(
    "obs.radiant",
    "Radiant exposure",
    "Radiant component of the structure's exposure, 0-1. Short-range, and \
     what a flame front does to a wall as it passes.",
    ["flame", "structure", "exposure"],
    radiant
);

obs_number!(
    "obs.ember",
    "Ember exposure",
    "Ember component of the structure's exposure, 0-1. Reaches kilometres \
     and is what burns houses down hours after the front has gone.",
    ["firebrand", "spotting", "structure"],
    ember
);

obs_bool!(
    "obs.structure_alight",
    "House is alight",
    "The household's own house has ignited.",
    ["burning", "fire", "structure"],
    structure_alight
);

obs_number!(
    "obs.fire_distance_m",
    "Distance to fire",
    "Metres to the nearest burning cell, as coarsely as someone standing at \
     the house could judge it. Saturates at 2500 m — beyond that the fire is \
     not a personal cue however large it is.",
    ["smoke", "see", "metres", "near", "far"],
    fire_distance_m
);

obs_number!(
    "obs.cue",
    "Perceived alarm",
    "The household's accumulated alarm, 0-1. Rises fast and decays slowly, \
     and is already scaled by their own risk perception — once alarmed, \
     people stay alarmed.",
    ["worry", "alarm", "perception", "aware"],
    cue
);

// --- the commander ---------------------------------------------------------

obs_bool!(
    "obs.order_issued",
    "Order issued",
    "An evacuation order covers this household. Note this is what the \
     commander did, not what the household knows.",
    ["evacuation", "commander", "warning"],
    order_issued
);

obs_bool!(
    "obs.warning_received",
    "Warning received",
    "The order has actually arrived over this household's own channel. The \
     gap behind \"Order issued\" runs from 90 s on a mobile alert to 20 \
     minutes for a household with no channel at all.",
    ["alert", "siren", "heard", "warning"],
    warning_received
);

obs_number!(
    "obs.minutes_since_order",
    "Minutes since order",
    "Minutes since the order was issued, or a very large number if none has \
     been.",
    ["delay", "elapsed", "warning"],
    minutes_since_order
);

obs_number!(
    "obs.minutes_since_warning",
    "Minutes since told",
    "Minutes since the order actually reached this household over its own \
     channel, or a very large number if it has not. Behind \"Minutes since \
     order\" by anything from 90 s to twenty minutes, and it is the clock a \
     household's own reaction runs on -- they cannot start reacting to a \
     message they have not had.",
    ["delay", "elapsed", "told", "heard", "milling", "confirm"],
    minutes_since_warning
);

// --- the household ---------------------------------------------------------

behavior_node! {
    id: "obs.intent",
    name: "Stated intent",
    category: Observation,
    domain: Household,
    doc: "The household's pre-fire plan: leave early, wait and see, or stay \
          and defend. Compare it with \"Intent is\" or weight on it with \
          \"Intent weight\".",
    keywords: ["plan", "leave", "defend", "wait"],
    inputs: [],
    outputs: [(intent "value", "The household's stated plan")],
    params: [],
    eval: |ctx, _p, _i, out| out.push(Value::Intent(ctx.household().intent)),
}

obs_number!(
    "obs.risk_perception",
    "Risk perception",
    "How seriously this household takes wildfire risk, 0-1.",
    ["attitude", "aware", "trait"],
    risk_perception
);

obs_number!(
    "obs.trust_authority",
    "Trust in authority",
    "How much weight they give an official instruction, 0-1. Below about \
     0.35 an order on its own does not move them.",
    ["trust", "official", "compliance", "trait"],
    trust_authority
);

obs_number!(
    "obs.prep_time_min",
    "Baked preparation time",
    "Minutes of milling this household does before it actually leaves. The \
     single biggest lever in the model.",
    ["milling", "delay", "trait"],
    prep_time_min
);

obs_number!(
    "obs.defensible_space",
    "Defensible space",
    "Cleared ground around the property, 0-1. What makes staying to defend \
     survivable rather than fatal.",
    ["clearance", "garden", "trait"],
    defensible_space
);

obs_number!(
    "obs.household_size",
    "Household size",
    "People in the household.",
    ["people", "family", "size"],
    household_size
);

obs_bool!(
    "obs.has_vehicle",
    "Has a vehicle",
    "A car is available. Households without one walk, which is slower but \
     immune to a road being cut.",
    ["car", "drive", "capability"],
    has_vehicle
);

obs_bool!(
    "obs.needs_assistance",
    "Needs assistance",
    "Someone in the household cannot move unaided.",
    ["mobility", "elderly", "capability"],
    needs_assistance
);

// --- where they are in the process ----------------------------------------

obs_bool!(
    "obs.is_preparing",
    "Already preparing",
    "The household has already decided to go and is milling.",
    ["state", "milling"],
    is_preparing
);

obs_bool!(
    "obs.is_moving",
    "Already moving",
    "The household is on the road.",
    ["state", "travelling", "evacuating"],
    is_moving
);

obs_bool!(
    "obs.is_defending",
    "Already defending",
    "The household has committed to defending the property.",
    ["state", "stay"],
    is_defending
);

obs_bool!(
    "obs.route_blocked",
    "Route blocked",
    "No route to any refuge survives the fire. A household that learns this \
     late is the classic fatality.",
    ["cut", "trapped", "road"],
    route_blocked
);

obs_number!(
    "obs.refuge_distance_m",
    "Distance to refuge",
    "Network metres to the nearest reachable refuge. Very large when the \
     route is blocked.",
    ["safety", "assembly", "distance"],
    refuge_distance_m
);

// --- what the incident itself can break ------------------------------------
//
// These are the fields a scenario built against a real disaster asked for. Each
// one is a thing the model could not previously say — see `docs/behavior-gaps.md`.

obs_number!(
    "obs.spot_fire_distance_m",
    "Distance to a spot fire",
    "Metres to the nearest fire that started somewhere not contiguous with the \
     mapped front: an ember jump, or a second ignition. Saturates at 2500 m.\n\n\
     Deliberately separate from \"Distance to fire\": the front is where you \
     last saw it, and a spot fire is the one behind you, on the road you were \
     relying on.",
    ["ember", "spot", "jump", "firebrand", "behind", "new"],
    spot_fire_distance_m
);

obs_number!(
    "obs.spot_fire_age_min",
    "Age of that spot fire",
    "Minutes since the nearest spot fire started, or a very large number if \
     there has been none. A spot fire an hour old is part of the landscape; \
     one from two minutes ago is the reason to go now.",
    ["ember", "spot", "new", "recent", "minutes"],
    spot_fire_age_min
);

obs_bool!(
    "obs.road_closed",
    "Road closed to traffic",
    "The road this household would drive out on has been closed by order. \
     Distinct from \"Route blocked\", which is the fire closing it: this one is \
     a decision somebody made, and it is the lever investigators found missing \
     at Pedrogao Grande.",
    ["closure", "police", "barricade", "traffic", "order"],
    road_closed
);

obs_bool!(
    "obs.comms_down",
    "No signal",
    "The warning network covering this house is down — the fire has taken out \
     the mast. Their channel is now whatever they can see and whoever knocks on \
     the door.\n\n\
     This is a *correlated* failure: every household under the same mast loses \
     it at once, which is what actually happens and is not what a per-household \
     channel draw can express.",
    ["network", "mast", "phone", "outage", "alert", "cell"],
    comms_down
);

obs_bool!(
    "obs.is_visitor",
    "Visitors, not residents",
    "Nobody here is at home: a hotel, a let, a campsite. No vehicle of their \
     own, no knowledge of which road goes where, and a warning that arrives \
     through whoever is running the place rather than over a resident's alert.",
    ["tourist", "hotel", "transient", "guest", "holiday"],
    is_visitor
);

obs_number!(
    "obs.open_ground_distance_m",
    "Distance to open ground",
    "Network metres on foot to the nearest survivable open ground: a car park, \
     a cleared field, a beach. Not a refuge — nobody is organising anything \
     there, and it is measured the same way refuges are, off the fuel around \
     it. Very large when there is none.",
    ["clearing", "safety zone", "last resort", "park", "beach"],
    open_ground_distance_m
);

obs_number!(
    "obs.shore_distance_m",
    "Distance to the shore",
    "Network metres on foot to the water's edge. Very large inland — two of the \
     four shipped real scenarios have no coast in their window at all, so a \
     behaviour that leans on this has to read sensibly when it is absent.",
    ["sea", "water", "beach", "coast", "shoreline"],
    shore_distance_m
);

obs_number!(
    "obs.boat_lift_min",
    "Minutes to a boat lift",
    "Minutes until a maritime pickup is on station at the shore, zero once it \
     is, and a very large number when none has been asked for. Rhodes moved \
     thousands of people this way when the roads could not clear the area in \
     time.",
    ["boat", "sea", "coastguard", "lift", "pickup", "ferry"],
    boat_lift_min
);

obs_number!(
    "obs.jitter",
    "Individual variation",
    "A stable per-household draw in 0-1. Use it to spread a threshold across \
     the population instead of having everyone act on the same tick. It is an \
     observation rather than a random node on purpose: it is hashed from the \
     household id, so it cannot make a run irreproducible.",
    ["random", "noise", "spread", "variation"],
    jitter
);
