//! Arithmetic, comparison, boolean algebra and the two shaping curves that
//! turn a physical quantity into something a threshold can be set on.
//!
//! Kept small on purpose. A palette with thirty maths nodes in it is a
//! programming language with a worse editor; the useful ones here are the
//! ramp and the intent weight, because they are what a behavioural assumption
//! actually looks like.

use crate::behavior_node;
use crate::value::{IntentValue, Value};

behavior_node! {
    id: "math.add",
    name: "Add",
    category: Logic,
    doc: "a + b.",
    keywords: ["sum", "plus", "+"],
    inputs: [(number "a", "First term", 0.0), (number "b", "Second term", 0.0)],
    outputs: [(number "out", "a + b")],
    params: [],
    eval: |_c, _p, i, out| out.push(Value::Number(i.num(0) + i.num(1))),
}

behavior_node! {
    id: "math.subtract",
    name: "Subtract",
    category: Logic,
    doc: "a - b.",
    keywords: ["minus", "difference", "-"],
    inputs: [(number "a", "Left term", 0.0), (number "b", "Right term", 0.0)],
    outputs: [(number "out", "a - b")],
    params: [],
    eval: |_c, _p, i, out| out.push(Value::Number(i.num(0) - i.num(1))),
}

behavior_node! {
    id: "math.multiply",
    name: "Multiply",
    category: Logic,
    doc: "a x b.",
    keywords: ["product", "times", "*"],
    inputs: [(number "a", "First factor", 1.0), (number "b", "Second factor", 1.0)],
    outputs: [(number "out", "a x b")],
    params: [],
    eval: |_c, _p, i, out| out.push(Value::Number(i.num(0) * i.num(1))),
}

behavior_node! {
    id: "math.scale",
    name: "Scale and offset",
    category: Logic,
    doc: "value x gain + offset. The one-node way to turn a 0-1 quantity into \
          a weighted contribution.",
    keywords: ["gain", "weight", "linear", "affine"],
    inputs: [(number "value", "Input", 0.0)],
    outputs: [(number "out", "value x gain + offset")],
    params: [
        (number "gain", "Gain", "Multiplier applied first.", 1.0, -100.0, 100.0, ""),
        (number "offset", "Offset", "Added after the gain.", 0.0, -100.0, 100.0, "")
    ],
    eval: |_c, p, i, out| out.push(Value::Number(i.num(0) * p.num(0) + p.num(1))),
}

behavior_node! {
    id: "math.min",
    name: "Smaller of",
    category: Logic,
    doc: "The smaller of two numbers.",
    keywords: ["minimum", "lower"],
    inputs: [(number "a", "First", 0.0), (number "b", "Second", 0.0)],
    outputs: [(number "out", "min(a, b)")],
    params: [],
    eval: |_c, _p, i, out| out.push(Value::Number(i.num(0).min(i.num(1)))),
}

behavior_node! {
    id: "math.max",
    name: "Larger of",
    category: Logic,
    doc: "The larger of two numbers. The usual way to combine several cues \
          into one: whichever is loudest wins.",
    keywords: ["maximum", "higher", "combine", "strongest"],
    inputs: [(number "a", "First", 0.0), (number "b", "Second", 0.0)],
    outputs: [(number "out", "max(a, b)")],
    params: [],
    eval: |_c, _p, i, out| out.push(Value::Number(i.num(0).max(i.num(1)))),
}

behavior_node! {
    id: "math.clamp",
    name: "Clamp",
    category: Logic,
    doc: "Hold a number inside a range.",
    keywords: ["limit", "bound", "saturate"],
    inputs: [(number "value", "Input", 0.0)],
    outputs: [(number "out", "The clamped value")],
    params: [
        (number "low", "Lowest", "Values below this come out as this.", 0.0, -1000.0, 1000.0, ""),
        (number "high", "Highest", "Values above this come out as this.", 1.0, -1000.0, 1000.0, "")
    ],
    eval: |_c, p, i, out| {
        let (lo, hi) = (p.num(0), p.num(1));
        out.push(Value::Number(i.num(0).clamp(lo.min(hi), hi.max(lo))));
    },
}

behavior_node! {
    id: "math.ramp",
    name: "Ramp",
    category: Logic,
    doc: "Map a quantity onto 0-1 across a band, flat outside it. Set \"at 0\" \
          above \"at 1\" for a falling ramp — which is what distance wants: \
          near is alarming, far is not.",
    keywords: ["curve", "interpolate", "falloff", "response", "normalise"],
    inputs: [(number "value", "The quantity to map", 0.0)],
    outputs: [(number "out", "0 to 1")],
    params: [
        (number "at_zero", "Value at 0", "Input value that maps to 0.", 0.0, -100000.0, 100000.0, ""),
        (number "at_one", "Value at 1", "Input value that maps to 1.", 1.0, -100000.0, 100000.0, "")
    ],
    eval: |_c, p, i, out| {
        let (a, b) = (p.num(0), p.num(1));
        let span = b - a;
        // A zero-width band is a step, not a division by zero.
        let t = if span.abs() < 1.0e-9 {
            if i.num(0) >= b { 1.0 } else { 0.0 }
        } else {
            (i.num(0) - a) / span
        };
        out.push(Value::Number(t.clamp(0.0, 1.0)));
    },
}

behavior_node! {
    id: "math.blend",
    name: "Blend",
    category: Logic,
    doc: "Mix between a and b by t, with t held to 0-1.",
    keywords: ["lerp", "mix", "interpolate"],
    inputs: [
        (number "a", "Value when t is 0", 0.0),
        (number "b", "Value when t is 1", 1.0),
        (number "t", "Mix", 0.0)
    ],
    outputs: [(number "out", "The blended value")],
    params: [],
    eval: |_c, _p, i, out| {
        let t = i.num(2).clamp(0.0, 1.0);
        out.push(Value::Number(i.num(0) * (1.0 - t) + i.num(1) * t));
    },
}

// --- comparison ------------------------------------------------------------

behavior_node! {
    id: "cmp.above",
    name: "Above threshold",
    category: Logic,
    doc: "True when the value is above the threshold. The threshold is a \
          parameter, which makes it the thing a subtype overrides.",
    keywords: ["greater", "exceeds", "threshold", ">"],
    inputs: [(number "value", "The quantity to test", 0.0)],
    outputs: [(bool "out", "value > threshold")],
    params: [
        (number "threshold", "Threshold", "The level the value has to pass.", 0.5, -100000.0, 100000.0, "")
    ],
    eval: |_c, p, i, out| out.push(Value::Bool(i.num(0) > p.num(0))),
}

behavior_node! {
    id: "cmp.below",
    name: "Below threshold",
    category: Logic,
    doc: "True when the value is below the threshold.",
    keywords: ["less", "under", "threshold", "<"],
    inputs: [(number "value", "The quantity to test", 0.0)],
    outputs: [(bool "out", "value < threshold")],
    params: [
        (number "threshold", "Threshold", "The level the value has to stay under.", 0.5, -100000.0, 100000.0, "")
    ],
    eval: |_c, p, i, out| out.push(Value::Bool(i.num(0) < p.num(0))),
}

behavior_node! {
    id: "cmp.greater",
    name: "a greater than b",
    category: Logic,
    doc: "Compare two wired quantities.",
    keywords: ["greater", "compare", ">"],
    inputs: [(number "a", "Left", 0.0), (number "b", "Right", 0.0)],
    outputs: [(bool "out", "a > b")],
    params: [],
    eval: |_c, _p, i, out| out.push(Value::Bool(i.num(0) > i.num(1))),
}

// --- boolean ---------------------------------------------------------------

behavior_node! {
    id: "logic.and",
    name: "And",
    category: Logic,
    doc: "True when both are true.",
    keywords: ["both", "&&"],
    inputs: [(bool "a", "First condition", false), (bool "b", "Second condition", false)],
    outputs: [(bool "out", "a and b")],
    params: [],
    eval: |_c, _p, i, out| out.push(Value::Bool(i.boolean(0) && i.boolean(1))),
}

behavior_node! {
    id: "logic.or",
    name: "Or",
    category: Logic,
    doc: "True when either is true.",
    keywords: ["either", "any", "||"],
    inputs: [(bool "a", "First condition", false), (bool "b", "Second condition", false)],
    outputs: [(bool "out", "a or b")],
    params: [],
    eval: |_c, _p, i, out| out.push(Value::Bool(i.boolean(0) || i.boolean(1))),
}

behavior_node! {
    id: "logic.not",
    name: "Not",
    category: Logic,
    doc: "Inverts a condition.",
    keywords: ["invert", "negate", "!"],
    inputs: [(bool "a", "Condition", false)],
    outputs: [(bool "out", "not a")],
    params: [],
    eval: |_c, _p, i, out| out.push(Value::Bool(!i.boolean(0))),
}

behavior_node! {
    id: "logic.any",
    name: "Any of",
    category: Logic,
    doc: "True when any of the conditions wired in is true. Takes as many as \
          you like, which is the difference between one box and a chain of \
          three \"Or\"s that has to be re-wired every time a branch is added.",
    keywords: ["or", "either", "some", "chain", "combine"],
    inputs: [(bools "conditions", "Any number of conditions")],
    outputs: [(bool "out", "True when at least one holds")],
    params: [],
    eval: |_c, _p, i, out| {
        out.push(Value::Bool(i.all(0).iter().any(|v| v.as_bool())));
    },
}

behavior_node! {
    id: "logic.all",
    name: "All of",
    category: Logic,
    doc: "True when every condition wired in is true. With nothing wired in it \
          is true, which is the useful convention: an unconstrained branch fires.",
    keywords: ["and", "every", "both", "chain", "combine"],
    inputs: [(bools "conditions", "Any number of conditions")],
    outputs: [(bool "out", "True when they all hold")],
    params: [],
    eval: |_c, _p, i, out| {
        out.push(Value::Bool(i.all(0).iter().all(|v| v.as_bool())));
    },
}

behavior_node! {
    id: "flow.pick_number",
    name: "Pick number",
    category: Logic,
    doc: "One number when the condition holds, another when it does not.",
    keywords: ["if", "select", "switch", "choose", "ternary"],
    inputs: [
        (bool "when", "The condition", false),
        (number "then", "Used when the condition holds", 1.0),
        (number "otherwise", "Used when it does not", 0.0)
    ],
    outputs: [(number "out", "The chosen number")],
    params: [],
    eval: |_c, _p, i, out| {
        out.push(Value::Number(if i.boolean(0) { i.num(1) } else { i.num(2) }));
    },
}

// --- intent ----------------------------------------------------------------

behavior_node! {
    id: "intent.is",
    name: "Intent is",
    category: Logic,
    domain: Household,
    doc: "True when the household's stated plan is the one selected here.",
    keywords: ["plan", "equals", "leave", "defend", "wait"],
    inputs: [(intent "intent", "The household's stated plan")],
    outputs: [(bool "out", "Whether it matches")],
    params: [
        (choice "plan", "Plan", "The plan to test for.", "wait_and_see",
            ["leave_early", "wait_and_see", "stay_defend"])
    ],
    eval: |_c, p, i, out| out.push(Value::Bool(i.intent(0) == p.intent(0))),
}

behavior_node! {
    id: "intent.weight",
    name: "Intent weight",
    category: Logic,
    domain: Household,
    doc: "A number chosen by the household's stated plan. This is the honest \
          way to say \"people who planned to leave react sooner\" without \
          building three separate graphs.",
    keywords: ["plan", "weight", "per-intent", "table"],
    inputs: [(intent "intent", "The household's stated plan")],
    outputs: [(number "out", "The weight for that plan")],
    params: [
        (number "leave_early", "Leave early", "Weight for households that planned to go.", 1.0, -100.0, 100.0, ""),
        (number "wait_and_see", "Wait and see", "Weight for the undecided majority.", 1.0, -100.0, 100.0, ""),
        (number "stay_defend", "Stay and defend", "Weight for households that planned to stay.", 1.0, -100.0, 100.0, "")
    ],
    eval: |_c, p, i, out| {
        let w = match i.intent(0) {
            IntentValue::LeaveEarly => p.num(0),
            IntentValue::WaitAndSee => p.num(1),
            IntentValue::StayDefend => p.num(2),
        };
        out.push(Value::Number(w));
    },
}
