//! The sinks: what the model reads back out.
//!
//! One decision sink per [`Domain`](crate::Domain), plus two optional readouts.
//! Keeping the model's read surface this narrow is what makes an authored
//! behaviour safe to run in a scenario: whatever the graph does internally, all
//! it can hand back is an action, a milling multiplier and a number for the HUD.
//!
//! The two decision sinks are identical machinery and differ in exactly one
//! thing — what a graph with nothing firing means. A household that has been
//! told nothing stays put; a unit that has been told nothing has still been
//! given an order by the commander and gets on with it.

use crate::behavior_node;
use crate::node::withheld;
use crate::value::{ActionKind, Value};

/// Pick the strongest live proposal on a decision sink's multi-input.
macro_rules! strongest {
    ($i:expr, $fallback:expr) => {{
        let mut best = withheld($fallback).as_action();
        for v in $i.all(0) {
            let a = v.as_action();
            if a.fired && (!best.fired || a.priority > best.priority) {
                best = a;
            }
        }
        Value::Action(best)
    }};
}

behavior_node! {
    id: "out.decision",
    name: "Decision",
    category: Output,
    domain: Household,
    doc: "The behaviour's answer. Wire every action proposal into this — it \
          takes as many as you like and picks the strongest. With nothing \
          wired in, or nothing firing, the household stays put.",
    keywords: ["output", "result", "sink", "answer"],
    inputs: [(actions "proposals", "Every action proposal, in any order")],
    outputs: [(action "chosen", "The winning proposal")],
    params: [],
    eval: |_c, _p, i, out| out.push(strongest!(i, ActionKind::Stay)),
}

behavior_node! {
    id: "out.unit_decision",
    name: "Unit decision",
    category: Output,
    domain: SuppressionUnit,
    doc: "The policy's answer. Wire every action proposal into this — it takes \
          as many as you like and picks the strongest. With nothing wired in, or \
          nothing firing, the unit carries on with the order it was given, which \
          is the common case and is why this graph can be very small.",
    keywords: ["output", "result", "sink", "answer", "policy"],
    inputs: [(actions "proposals", "Every action proposal, in any order")],
    outputs: [(action "chosen", "The winning proposal")],
    params: [],
    eval: |_c, _p, i, out| out.push(strongest!(i, ActionKind::Continue)),
}

behavior_node! {
    id: "out.prep_scale",
    name: "Preparation multiplier",
    category: Output,
    domain: Household,
    doc: "Scales the household's baked milling time. 1.0 leaves it alone; 0.5 \
          is a household that drops what it is doing and goes. Optional — \
          without it, milling time is whatever the population bake says.",
    keywords: ["output", "milling", "delay", "prep", "sink"],
    inputs: [(number "scale", "Multiplier on preparation time", 1.0)],
    outputs: [(number "value", "The multiplier, clamped to a sane range")],
    params: [],
    eval: |_c, _p, i, out| {
        // Unclamped this is a way to author a household that leaves before it
        // is warned, or one that never leaves at all.
        out.push(Value::Number(i.num(0).clamp(0.05, 10.0)));
    },
}

behavior_node! {
    id: "out.urgency",
    name: "Urgency readout",
    category: Output,
    doc: "A 0-1 number surfaced in the household inspector. It has no \
          mechanical effect at all — it is here so an author can expose the \
          intermediate quantity their behaviour actually turns on, and watch \
          it during a run.",
    keywords: ["output", "debug", "readout", "inspect", "sink"],
    inputs: [(number "value", "Whatever you want to watch", 0.0)],
    outputs: [(number "value", "The same number, clamped to 0-1")],
    params: [],
    eval: |_c, _p, i, out| out.push(Value::Number(i.num(0).clamp(0.0, 1.0))),
}
