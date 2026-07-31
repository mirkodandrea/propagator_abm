//! One node per field of [`UnitObs`](crate::UnitObs).
//!
//! The suppression half of the observation surface. Same shape as the household
//! half, same one-line cost to add to, and the same closed contract: a unit's
//! graph can read what the unit can see from where it is standing and nothing
//! else.
//!
//! Note what is deliberately *absent*. There is no node for where the fire's
//! head is, no node for what the other units are doing, and no node for a good
//! place to cut. Those would make a graph into a planner, and the planner in
//! this game is the player.

use crate::behavior_node;
use crate::value::{UnitKindKey, Value};

macro_rules! unit_number {
    ($id:literal, $name:literal, $doc:literal, [$($kw:literal),* $(,)?], $field:ident) => {
        behavior_node! {
            id: $id,
            name: $name,
            category: Observation,
            domain: SuppressionUnit,
            doc: $doc,
            keywords: [$($kw),*],
            inputs: [],
            outputs: [(number "value", $doc)],
            params: [],
            eval: |ctx, _p, _i, out| out.push(Value::Number(ctx.unit().$field)),
        }
    };
}

macro_rules! unit_bool {
    ($id:literal, $name:literal, $doc:literal, [$($kw:literal),* $(,)?], $field:ident) => {
        behavior_node! {
            id: $id,
            name: $name,
            category: Observation,
            domain: SuppressionUnit,
            doc: $doc,
            keywords: [$($kw),*],
            inputs: [],
            outputs: [(bool "value", $doc)],
            params: [],
            eval: |ctx, _p, _i, out| out.push(Value::Bool(ctx.unit().$field)),
        }
    };
}

// --- the incident -----------------------------------------------------------

unit_number!(
    "unit.time_min",
    "Time",
    "Simulated minutes since the incident started.",
    ["clock", "elapsed", "minutes"],
    time_min
);

// --- the fire, from where the unit is standing ------------------------------

unit_number!(
    "unit.threat_here",
    "Threat at the unit",
    "Survivability where this unit is standing, 0-1. The same field the \
     civilians read: 0.35 is the stated working limit for firefighters and 0.55 \
     is impassable. Below the civilians' number on purpose — crews disengage \
     while they still can.",
    ["danger", "safety", "heat", "survivable", "withdraw"],
    threat_here
);

unit_number!(
    "unit.heat_fraction",
    "Accumulated heat",
    "Flame exposure so far as a fraction of what this unit survives, 0-1. \
     Reaching 1.0 is being burnt over, and no graph can author that away — this \
     is here so a behaviour can pull back well before it.",
    ["burnover", "exposure", "cumulative", "danger"],
    heat_fraction
);

unit_number!(
    "unit.distance_to_fire_m",
    "Distance to fire",
    "Metres to the nearest burning cell. Very large when there is no fire.",
    ["near", "front", "metres", "edge"],
    distance_to_fire_m
);

// --- what it is carrying ----------------------------------------------------

unit_number!(
    "unit.water_fraction",
    "Water remaining",
    "Tank remaining, 0-1. Always zero for a hand crew, so pair it with \
     \"Carries water\" rather than testing it on its own.",
    ["tank", "supply", "litres", "refill"],
    water_fraction
);

unit_bool!(
    "unit.carries_water",
    "Carries water",
    "This unit has a tank at all. False for a hand crew, whose consumable is \
     daylight rather than water.",
    ["tank", "engine", "aircraft", "capability"],
    carries_water
);

// --- the order --------------------------------------------------------------

unit_bool!(
    "unit.has_task",
    "Has an order",
    "The commander has given this unit something to do beyond standing by.",
    ["tasked", "order", "assigned"],
    has_task
);

unit_number!(
    "unit.distance_to_task_m",
    "Distance to the order",
    "Metres to where the order points. Very large when there is no order.",
    ["target", "travel", "metres", "eta"],
    distance_to_task_m
);

unit_bool!(
    "unit.task_reachable",
    "Order is reachable",
    "The ordered point is on a stretch of network this unit can actually get \
     to. False is the engine that ran out of tarmac 600 m short — which is a \
     situation to author a response to, not an error.",
    ["road", "reach", "stranded", "network"],
    task_reachable
);

unit_number!(
    "unit.line_progress",
    "Line cut so far",
    "Fraction of an ordered fuel break already cut, 0-1. Zero for anything not \
     cutting line.",
    ["fireline", "crew", "progress", "break"],
    line_progress
);

unit_number!(
    "unit.minutes_on_task",
    "Minutes on this order",
    "How long this unit has been working its current order. The honest proxy \
     for fatigue until fatigue is modelled.",
    ["elapsed", "shift", "tired", "duration"],
    minutes_on_task
);

unit_bool!(
    "unit.work_available",
    "Something to work on",
    "There is unburnt burnable fuel within reach for an engine or an aircraft, \
     or unfinished line for a crew. False is the unit standing in the black \
     with nothing useful left to do where it is.",
    ["targets", "fuel", "useful", "suppressible"],
    work_available
);

// --- where it is in its own cycle -------------------------------------------

unit_bool!(
    "unit.is_working",
    "Already working",
    "Actually cutting, pumping or dropping, as opposed to travelling to the job.",
    ["state", "on task", "engaged"],
    is_working
);

unit_bool!(
    "unit.is_moving",
    "Already travelling",
    "On its way somewhere.",
    ["state", "responding", "driving"],
    is_moving
);

unit_number!(
    "unit.distance_to_base_m",
    "Distance to staging",
    "Metres back to where this unit staged. What a withdrawal actually costs.",
    ["home", "return", "retreat", "metres"],
    distance_to_base_m
);

unit_number!(
    "unit.jitter",
    "Individual variation",
    "A stable per-unit draw in 0-1, hashed from the unit id. An observation \
     rather than a random node for the same reason the households' is: it \
     cannot make a run irreproducible.",
    ["random", "noise", "spread", "variation"],
    jitter
);

// --- what kind of resource this is ------------------------------------------

behavior_node! {
    id: "unit.is_kind",
    name: "Unit is a",
    category: Observation,
    domain: SuppressionUnit,
    doc: "True when this unit is the kind selected here. The way one policy \
          serves all three kinds — which is what you want, because a safety \
          rule that differs between a crew and an engine is a safety rule \
          someone has to remember to keep in step.",
    keywords: ["crew", "engine", "aircraft", "tanker", "type", "kind"],
    inputs: [],
    outputs: [(bool "value", "Whether this unit is that kind")],
    params: [
        (choice "kind", "Kind", "The resource type to test for.", "engine",
            ["hand_crew", "engine", "air_tanker"])
    ],
    eval: |ctx, p, _i, out| out.push(Value::Bool(ctx.unit().kind == p.unit_kind(0))),
}

behavior_node! {
    id: "unit.kind_weight",
    name: "Weight by unit kind",
    category: Observation,
    domain: SuppressionUnit,
    doc: "A number chosen by what kind of resource this is. The honest way to \
          say \"an aircraft accepts more heat than a crew on the ground\" \
          without building three separate policies.",
    keywords: ["crew", "engine", "aircraft", "table", "per-kind", "weight"],
    inputs: [],
    outputs: [(number "value", "The weight for this unit's kind")],
    params: [
        (number "hand_crew", "Hand crew", "Weight for a squadra on foot.", 1.0, -100.0, 100.0, ""),
        (number "engine", "Engine", "Weight for an autobotte.", 1.0, -100.0, 100.0, ""),
        (number "air_tanker", "Air tanker", "Weight for a Canadair.", 1.0, -100.0, 100.0, "")
    ],
    eval: |ctx, p, _i, out| {
        let w = match ctx.unit().kind {
            UnitKindKey::HandCrew => p.num(0),
            UnitKindKey::Engine => p.num(1),
            UnitKindKey::AirTanker => p.num(2),
        };
        out.push(Value::Number(w));
    },
}
