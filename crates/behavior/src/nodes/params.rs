//! Constants.
//!
//! A parameter node exists so a number can be *named* on the canvas and
//! overridden by a subtype from one place. Wiring the same constant into six
//! comparisons through one of these is the difference between a subtype
//! override that reads as an assumption and six that read as a patch.

use crate::behavior_node;
use crate::value::Value;

behavior_node! {
    id: "param.number",
    name: "Number",
    category: Parameter,
    doc: "A named constant. Give it a label so the subtype override list reads \
          as something a person wrote.",
    keywords: ["constant", "value", "literal", "tunable"],
    inputs: [],
    outputs: [(number "out", "The constant")],
    params: [
        (text "label", "Label", "What this number means.", "value"),
        (number "value", "Value", "The constant itself.", 0.5, -100000.0, 100000.0, "")
    ],
    eval: |_c, p, _i, out| out.push(Value::Number(p.num(1))),
}

behavior_node! {
    id: "param.bool",
    name: "Switch",
    category: Parameter,
    doc: "A named on/off constant. Useful for turning a whole branch of a \
          behaviour off in one subtype without deleting it.",
    keywords: ["constant", "toggle", "flag", "enable"],
    inputs: [],
    outputs: [(bool "out", "The switch position")],
    params: [
        (text "label", "Label", "What this switch controls.", "enabled"),
        (bool "value", "On", "The switch position.", true)
    ],
    eval: |_c, p, _i, out| out.push(Value::Bool(p.boolean(1))),
}

behavior_node! {
    id: "param.intent",
    name: "Fixed intent",
    category: Parameter,
    domain: Household,
    doc: "A constant plan, for testing a graph against one population without \
          editing the observation.",
    keywords: ["constant", "plan", "override"],
    inputs: [],
    outputs: [(intent "out", "The chosen plan")],
    params: [
        (choice "plan", "Plan", "The plan to emit.", "wait_and_see",
            ["leave_early", "wait_and_see", "stay_defend"])
    ],
    eval: |_c, p, _i, out| out.push(Value::Intent(p.intent(0))),
}
