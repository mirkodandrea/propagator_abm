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
    ($id:literal, $name:literal, $dom:ident, $doc:literal, [$($kw:literal),* $(,)?], $kind:ident, $prio:expr) => {
        behavior_node! {
            id: $id,
            name: $name,
            category: Action,
            domain: $dom,
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

// --- households -------------------------------------------------------------

action_node!(
    "action.prepare",
    "Prepare to leave",
    Household,
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
    Household,
    "Leave immediately, abandoning whatever preparation is left. Give this a \
     higher priority than \"Prepare to leave\" or it will never win.",
    ["flee", "go", "immediate", "urgent"],
    EvacuateNow,
    3.0
);

action_node!(
    "action.defend",
    "Defend property",
    Household,
    "Stay and fight for the house. Survivable in proportion to defensible \
     space, and fatal without it.",
    ["stay", "fight", "shelter", "property"],
    Defend,
    1.5
);

action_node!(
    "action.shelter",
    "Shelter in place",
    Household,
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
    domain: Household,
    doc: "Any household action, with the kind chosen as a parameter. Use this \
          when a subtype needs to change *which* action a branch proposes \
          rather than only how strongly.",
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

// --- suppression units ------------------------------------------------------
//
// The default priorities encode the one ordering the model insists on: safety
// outbids everything, and carrying on is what happens when nothing else does.
// They are parameters, so an author can invert them — and find out why they
// were that way round.

action_node!(
    "action.unit_withdraw",
    "Withdraw",
    SuppressionUnit,
    "Break off and pull back to staging, whatever the order was. The safety \
     action, and the only thing a unit does against its orders — give it the \
     highest priority in the graph. A unit that has withdrawn awaits new \
     orders rather than resuming on its own.",
    ["safety", "retreat", "pull back", "disengage", "abandon"],
    Withdraw,
    5.0
);

action_node!(
    "action.unit_refill",
    "Break off for water",
    SuppressionUnit,
    "Go to the nearest hydrant, fill up, and come back to the same order. For \
     an aircraft this is the scoop leg instead. Nothing at all for a hand crew, \
     which carries no water.",
    ["water", "hydrant", "tank", "resupply", "scoop"],
    Refill,
    3.0
);

action_node!(
    "action.unit_hold",
    "Hold position",
    SuppressionUnit,
    "Stop working and stand by where you are. Not a retreat — the unit stays \
     put and stays available. What a unit does when the job in front of it is \
     not worth doing but the ground is still safe.",
    ["stand by", "wait", "stop", "idle"],
    HoldPosition,
    2.0
);

action_node!(
    "action.unit_return",
    "Return to staging",
    SuppressionUnit,
    "Drive back to where this unit staged and await orders. Unlike Withdraw \
     this is not a safety action and carries no warning to the player, so use \
     it for a unit that has finished rather than one that is in danger.",
    ["base", "rtb", "go back", "staging", "finished"],
    ReturnToBase,
    1.0
);

action_node!(
    "action.unit_continue",
    "Carry on",
    SuppressionUnit,
    "Get on with the order. This is already what happens when nothing fires, so \
     a node for it is only worth placing when you want to *outbid* something \
     else — a unit that carries on through a threat another branch would have \
     pulled it out of.",
    ["comply", "obey", "proceed", "work"],
    Continue,
    0.0
);

behavior_node! {
    id: "action.unit_propose",
    name: "Propose unit action",
    category: Action,
    domain: SuppressionUnit,
    doc: "Any unit action, with the kind chosen as a parameter. Use this when a \
          profile needs to change *which* action a branch proposes rather than \
          only how strongly — \"engines return to staging when dry, crews hold\" \
          is one graph and two overrides rather than two graphs.",
    keywords: ["custom", "generic", "any", "decide"],
    inputs: [(bool "when", "Propose while the condition holds", false)],
    outputs: [(action "proposal", "The proposal, for the Unit decision output")],
    params: [
        (choice "action", "Action", "What to propose.", "withdraw",
            ["continue", "withdraw", "refill", "hold_position", "return_to_base"]),
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
