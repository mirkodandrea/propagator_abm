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

action_node!(
    "action.shelter_nearby",
    "Make for open ground",
    Household,
    "Leave the house on foot for the nearest survivable open ground — a car \
     park, a cleared field, a beach. Not a refuge and not organised: it is the \
     shelter of last resort for a household whose house is not defensible and \
     whose road is not usable, and it is the one thing this model could not \
     express until scenarios built against Mati and Rhodes asked for it.\n\n\
     Outbids \"Shelter in place\" only if you say so. Which of the two is right \
     is the whole question: a house buys ten times as long as standing outside, \
     and open ground buys indefinitely as long as it is genuinely open.",
    ["last resort", "clearing", "beach", "car park", "outside", "open"],
    ShelterNearby,
    3.5
);

action_node!(
    "action.make_for_shore",
    "Make for the shore",
    Household,
    "Leave on foot for the water's edge. In the water a person is out of the \
     fire's reach for as long as they can stand it, and a boat lift takes them \
     off — which is the Rhodes outcome. With no lift coming it is the Mati one: \
     alive at the shoreline, still in the incident.\n\n\
     Only reachable in a coastal window. Gate it on \"Distance to the shore\" \
     being finite, or it proposes something nobody can do.",
    ["sea", "water", "beach", "boat", "swim", "coast"],
    MakeForShore,
    3.5
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
            ["stay", "prepare", "evacuate_now", "defend", "shelter", "shelter_nearby",
             "make_for_shore"]),
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

// --- separated people --------------------------------------------------------
//
// Four actions, and the priorities encode the ordering the model insists on:
// sheltering outbids walking, because on a cut road the walking is what kills
// them, and going home outbids walking out only because a person who has decided
// to go back has decided against the refuge.

action_node!(
    "action.person_walk_out",
    "Walk out",
    Person,
    "Make for the nearest refuge on foot. What the model has always done with \
     people who were out when it started, and what the guidance says. A person \
     already walking out carries on: this proposal is idempotent.",
    ["refuge", "leave", "evacuate", "foot", "safety"],
    WalkOut,
    1.0
);

action_node!(
    "action.person_shelter",
    "Shelter where they are",
    Person,
    "Stop walking and take what cover there is. The right call when there is no \
     route left, because a road that ends in the fire is worse than standing \
     still — give this the highest priority in the graph.",
    ["stop", "trapped", "cover", "cut off", "last resort"],
    TakeShelter,
    4.0
);

action_node!(
    "action.person_head_home",
    "Head home",
    Person,
    "Turn round and walk back to the household's house. Wire this from \"Going \
     back for the family\" rather than from a bare condition — the block is \
     where the assumption lives, and it ships switched off.",
    ["family", "reunification", "back", "return", "house"],
    HeadHome,
    2.0
);

action_node!(
    "action.person_remain",
    "Carry on",
    Person,
    "Keep doing whatever they were doing. This is already what happens when \
     nothing fires, so a node for it is only worth placing when you want to \
     *outbid* something else.",
    ["nothing", "stay", "continue", "hold"],
    Remain,
    0.0
);

action_node!(
    "action.person_open_ground",
    "Make for open ground",
    Person,
    "Head for the nearest survivable open ground instead of for a refuge. \
     Usually much nearer than the refuge is, and the difference between the two \
     is exactly the gap that kills people on foot: a refuge is somewhere the \
     evacuation is organised, and open ground is somewhere the fire cannot \
     reach you.\n\n\
     Give it a higher priority than \"Walk out\" and a lower one than \"Shelter \
     where they are\", which is for the case where there is not even that.",
    ["clearing", "last resort", "car park", "beach", "safety zone"],
    WalkToOpenGround,
    2.5
);

action_node!(
    "action.person_shore",
    "Make for the shore",
    Person,
    "Head for the water's edge. The behaviour the Mati accounts describe — \
     people who reached open water lived, and people who did not reach it were \
     the fatalities — and the one Rhodes turned into an organised evacuation by \
     putting boats at the other end of it.\n\n\
     Gate it on \"Distance to the shore\" being finite: inland it proposes \
     something nobody can do.",
    ["sea", "water", "beach", "boat", "swim", "coast"],
    WalkToShore,
    2.5
);

behavior_node! {
    id: "action.person_propose",
    name: "Propose person action",
    category: Action,
    domain: Person,
    doc: "Any action a separated person can take, with the kind chosen as a \
          parameter. Use this when a profile needs to change *which* action a \
          branch proposes rather than only how strongly.",
    keywords: ["custom", "generic", "any", "decide"],
    inputs: [(bool "when", "Propose while the condition holds", false)],
    outputs: [(action "proposal", "The proposal, for the Person decision output")],
    params: [
        (choice "action", "Action", "What to propose.", "walk_out",
            ["remain", "walk_out", "take_shelter", "head_home", "walk_to_open_ground",
             "walk_to_shore"]),
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
