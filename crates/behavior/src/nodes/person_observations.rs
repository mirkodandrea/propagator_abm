//! One node per field of [`PersonObs`](crate::PersonObs).
//!
//! The same one-line-per-field shape as the household and unit observations,
//! and for the same reason: exposing a new thing a separated person can know is
//! a field on the struct plus a line here, and the palette has it.
//!
//! Every node here declares `domain: Person`, which is what keeps it out of a
//! household's palette — and, more importantly, keeps "Distance home" out of a
//! graph where there is no single person to be away from anywhere.

use crate::behavior_node;
use crate::value::Value;

/// A number read straight off the person observation.
macro_rules! person_number {
    ($id:literal, $name:literal, $doc:literal, [$($kw:literal),* $(,)?], $field:ident) => {
        behavior_node! {
            id: $id,
            name: $name,
            category: Observation,
            domain: Person,
            doc: $doc,
            keywords: [$($kw),*],
            inputs: [],
            outputs: [(number "value", $doc)],
            params: [],
            eval: |ctx, _p, _i, out| out.push(Value::Number(ctx.person().$field)),
        }
    };
}

/// A condition read straight off the person observation.
macro_rules! person_bool {
    ($id:literal, $name:literal, $doc:literal, [$($kw:literal),* $(,)?], $field:ident) => {
        behavior_node! {
            id: $id,
            name: $name,
            category: Observation,
            domain: Person,
            doc: $doc,
            keywords: [$($kw),*],
            inputs: [],
            outputs: [(bool "value", $doc)],
            params: [],
            eval: |ctx, _p, _i, out| out.push(Value::Bool(ctx.person().$field)),
        }
    };
}

// --- the incident -----------------------------------------------------------

person_number!(
    "person.time_min",
    "Time",
    "Simulated minutes since the incident started.",
    ["clock", "elapsed", "minutes"],
    time_min
);

person_number!(
    "person.threat",
    "Threat here",
    "Survivability where this person is standing, 0-1. At 0.55 it is not \
     survivable to be out in it. This is the pedestrian-scale field, which is \
     the right one: this agent is a pedestrian.",
    ["danger", "heat", "survivable", "smoke"],
    threat
);

person_number!(
    "person.heat_fraction",
    "Accumulated heat",
    "Flame exposure so far as a fraction of what a person survives. Reaching \
     1.0 is fatal, and no branch of a graph can prevent it — but a branch that \
     reads it can stop someone walking into more.",
    ["exposure", "burn", "dose", "lethal"],
    heat_fraction
);

person_number!(
    "person.fire_distance_m",
    "Distance to fire",
    "Metres to the nearest burning cell, as coarsely as someone standing here \
     could judge it. Saturates at 2500 m.",
    ["smoke", "see", "metres", "near", "far"],
    fire_distance_m
);

person_number!(
    "person.cue",
    "Perceived alarm",
    "This person's own accumulated alarm, 0-1. Rises fast and decays slowly. \
     Driven by what is around them, not around the house — someone out in the \
     smoke is alarmed long before the family at home is.",
    ["worry", "alarm", "perception", "aware"],
    cue
);

// --- the commander ----------------------------------------------------------

person_bool!(
    "person.order_issued",
    "Order issued",
    "A public evacuation order is out. There is no per-channel delay here, \
     unlike a household's: sirens and street broadcasts reach whoever is in \
     the street.",
    ["evacuation", "commander", "warning", "siren"],
    order_issued
);

person_number!(
    "person.minutes_since_order",
    "Minutes since order",
    "Minutes since the order was issued, or a very large number if none has \
     been.",
    ["delay", "elapsed", "warning"],
    minutes_since_order
);

// --- the person -------------------------------------------------------------

person_number!(
    "person.age",
    "Age",
    "Years. In this model age drives walking speed, so it is the honest way to \
     ask \"is this person fast enough to get out of here\".",
    ["old", "young", "elderly", "child", "trait"],
    age
);

person_number!(
    "person.walk_speed",
    "Walking speed",
    "Metres per second on the flat, before slope and smoke. Around 1.3 for an \
     adult; well under 1.0 for someone who needs help.",
    ["pace", "speed", "foot", "trait"],
    walk_speed
);

person_bool!(
    "person.needs_assistance",
    "Needs assistance",
    "This person cannot move unaided. On their own and away from home, this is \
     the worst combination the model can produce.",
    ["mobility", "elderly", "help", "capability"],
    needs_assistance
);

// --- the family they are away from -----------------------------------------

person_number!(
    "person.home_distance_m",
    "Distance home",
    "Straight-line metres back to their household's home. Compare it with \
     \"Distance to refuge\", which is measured the same way — going home is \
     only ever the shorter trip for someone who was nearly there.",
    ["family", "house", "back", "reunification", "distance"],
    home_distance_m
);

person_bool!(
    "person.household_at_home",
    "Family still at home",
    "Someone in the household is still at the house — the thing a person \
     heading home would be going back *for*. Once the family has left, going \
     home is dangerous and pointless rather than merely dangerous.",
    ["family", "reunification", "waiting", "house"],
    household_at_home
);

person_bool!(
    "person.household_safe",
    "Family already safe",
    "The household has reached a refuge or left the map. Nothing to go back for.",
    ["family", "evacuated", "safe", "reunification"],
    household_safe
);

person_bool!(
    "person.home_alight",
    "Home is alight",
    "Their house has ignited. There is nothing to go back to, and someone who \
     does not know that is exactly who this branch is about.",
    ["burning", "house", "structure", "lost"],
    home_alight
);

// --- their way out ----------------------------------------------------------

person_number!(
    "person.refuge_distance_m",
    "Distance to refuge",
    "Straight-line metres to the nearest refuge, on the same scale as \
     \"Distance home\" so the two can be compared. Whether the route there \
     survives is a separate question — see \"Route blocked\".",
    ["safety", "assembly", "out", "distance"],
    refuge_distance_m
);

person_bool!(
    "person.route_blocked",
    "Route blocked",
    "No route to any refuge survives the fire. This is the moment where \
     continuing to walk out is worse than stopping.",
    ["cut", "trapped", "road"],
    route_blocked
);

person_bool!(
    "person.is_moving",
    "Already walking",
    "This person is already on their way somewhere.",
    ["state", "travelling", "walking"],
    is_moving
);

person_bool!(
    "person.is_heading_home",
    "Already heading home",
    "They have already turned back for the house. Read it to stop a branch \
     re-deciding something it decided a tick ago.",
    ["state", "reunification", "back"],
    is_heading_home
);

person_bool!(
    "person.is_sheltering",
    "Already sheltering",
    "They have already stopped and taken shelter where they are.",
    ["state", "stopped", "trapped"],
    is_sheltering
);

person_number!(
    "person.jitter",
    "Individual variation",
    "A stable per-person draw in 0-1. Use it to spread a threshold across the \
     population instead of having everyone act on the same tick. It is an \
     observation rather than a random node on purpose: it is hashed from the \
     person id, so it cannot make a run irreproducible.",
    ["random", "noise", "spread", "variation"],
    jitter
);
