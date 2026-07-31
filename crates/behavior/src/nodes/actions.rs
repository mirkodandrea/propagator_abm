//! Proposing an action.
//!
//! Every action node is the same shape: a condition in, a proposal out,
//! carrying a priority. Nothing here decides anything — the proposals all
//! arrive at the single `Decision` output, which picks the strongest. That
//! indirection is what lets a scientist add a branch without having to think
//! about how it interacts with the branches already there: a new proposal
//! either outbids the others or it does not.
//!
//! A node whose condition is false still emits its proposal, marked
//! `fired: false`, so the test bench can show that a branch was *considered*
//! and lost rather than showing an empty slot.

use crate::behavior_node;
use crate::node::withheld;
use crate::value::{ActionKind, ActionProposal, Value};

/// A preset action node: fixed kind, one priority parameter.
macro_rules! action_node {
    ($id:literal, $name:literal, $doc:literal, [$($kw:literal),* $(,)?], $kind:ident, $prio:expr) => {
        behavior_node! {
            id: $id,
            name: $name,
            category: Action,
            doc: $doc,
            keywords: [$($kw),*],
            inputs: [(bool "when", "Propose this action while the condition holds", false)],
            outputs: [(action "proposal", "The proposal, for the Decision output")],
            params: [
                (number "priority", "Priority",
                    "Strongest proposal wins. Ties go to the lower node id, which \
                     is stable but arbitrary — separate them rather than relying on it.",
                    $prio, 0.0, 10.0, "")
            ],
            eval: |_c, p, i, out| {
                out.push(if i.boolean(0) {
                    Value::Action(ActionProposal {
                        kind: ActionKind::$kind,
                        priority: p.num(0),
                        fired: true,
                    })
                } else {
                    withheld(ActionKind::$kind)
                });
            },
        }
    };
}

action_node!(
    "action.prepare",
    "Prepare to leave",
    "Start milling: gather people and belongings, then go. The household's \
     own preparation time still applies — this is the decision, not the \
     departure.",
    ["evacuate", "leave", "mill", "decide"],
    Prepare,
    1.0
);

action_node!(
    "action.evacuate_now",
    "Evacuate now",
    "Leave immediately, abandoning whatever preparation is left. Give this a \
     higher priority than \"Prepare to leave\" or it will never win.",
    ["flee", "go", "immediate", "urgent"],
    EvacuateNow,
    3.0
);

action_node!(
    "action.defend",
    "Defend property",
    "Stay and fight for the house. Survivable in proportion to defensible \
     space, and fatal without it.",
    ["stay", "fight", "shelter", "property"],
    Defend,
    1.5
);

action_node!(
    "action.shelter",
    "Shelter in place",
    "Too late to move: take shelter where they are. A house buys around ten \
     times as long as standing in the open, which is why this is a real \
     option and not a euphemism.",
    ["refuge", "inside", "trapped", "last resort"],
    Shelter,
    4.0
);

behavior_node! {
    id: "action.propose",
    name: "Propose action",
    category: Action,
    doc: "Any action, with the kind chosen as a parameter. Use this when a \
          subtype needs to change *which* action a branch proposes rather \
          than only how strongly.",
    keywords: ["custom", "generic", "any", "decide"],
    inputs: [(bool "when", "Propose while the condition holds", false)],
    outputs: [(action "proposal", "The proposal, for the Decision output")],
    params: [
        (choice "action", "Action", "What to propose.", "prepare",
            ["stay", "prepare", "evacuate_now", "defend", "shelter"]),
        (number "priority", "Priority", "Strongest proposal wins.", 1.0, 0.0, 10.0, "")
    ],
    eval: |_c, p, i, out| {
        let kind = p.action(0);
        out.push(if i.boolean(0) {
            Value::Action(ActionProposal { kind, priority: p.num(1), fired: true })
        } else {
            withheld(kind)
        });
    },
}
