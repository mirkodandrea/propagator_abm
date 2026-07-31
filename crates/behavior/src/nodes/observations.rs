//! One node per field of [`Observation`](crate::Observation).
//!
//! Exposing a new agent capability to the editor is a one-line addition here:
//! add the field to `Observation`, add the line, and the palette has it. The
//! docs are the ones a scientist reads on hover, so they say what the quantity
//! *is* rather than restating the field name.

use crate::behavior_node;
use crate::value::Value;

/// A number read straight off the observation.
macro_rules! obs_number {
    ($id:literal, $name:literal, $doc:literal, [$($kw:literal),* $(,)?], $field:ident) => {
        behavior_node! {
            id: $id,
            name: $name,
            category: Observation,
            doc: $doc,
            keywords: [$($kw),*],
            inputs: [],
            outputs: [(number "value", $doc)],
            params: [],
            eval: |ctx, _p, _i, out| out.push(Value::Number(ctx.obs.$field)),
        }
    };
}

/// A condition read straight off the observation.
macro_rules! obs_bool {
    ($id:literal, $name:literal, $doc:literal, [$($kw:literal),* $(,)?], $field:ident) => {
        behavior_node! {
            id: $id,
            name: $name,
            category: Observation,
            doc: $doc,
            keywords: [$($kw),*],
            inputs: [],
            outputs: [(bool "value", $doc)],
            params: [],
            eval: |ctx, _p, _i, out| out.push(Value::Bool(ctx.obs.$field)),
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

// --- the household ---------------------------------------------------------

behavior_node! {
    id: "obs.intent",
    name: "Stated intent",
    category: Observation,
    doc: "The household's pre-fire plan: leave early, wait and see, or stay \
          and defend. Compare it with \"Intent is\" or weight on it with \
          \"Intent weight\".",
    keywords: ["plan", "leave", "defend", "wait"],
    inputs: [],
    outputs: [(intent "value", "The household's stated plan")],
    params: [],
    eval: |ctx, _p, _i, out| out.push(Value::Intent(ctx.obs.intent)),
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
