//! The four things that can travel down a wire.
//!
//! The type set is deliberately tiny. Every extra port type is a rule a
//! scientist has to hold in their head before they can connect two boxes, and
//! the evacuation model only ever needs a magnitude, a condition, the
//! household's standing plan, and a proposed action.

use serde::{Deserialize, Serialize};

#[cfg(feature = "reflect")]
use bevy_reflect::Reflect;

/// What a port carries. Two ports connect only if their types are equal —
/// there is no coercion, deliberately: a silent bool-to-number would let a
/// graph that reads wrong evaluate fine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    /// A continuous magnitude. Most are 0–1 normalised; the ones that are not
    /// (metres, minutes) say so in the port doc.
    Number,
    /// A condition.
    Bool,
    /// The household's pre-fire plan — what they say they will do.
    Intent,
    /// A proposed action, carrying its own priority. Only [`crate::nodes`]
    /// action nodes produce these, and only `out.decision` consumes them.
    Action,
}

impl ValueType {
    pub fn label(self) -> &'static str {
        match self {
            ValueType::Number => "number",
            ValueType::Bool => "bool",
            ValueType::Intent => "intent",
            ValueType::Action => "action",
        }
    }

    /// Editor pin colour, as linear-ish sRGB bytes. Kept here rather than in
    /// the editor so a headless report can colour-code the same way.
    pub fn colour(self) -> [u8; 3] {
        match self {
            ValueType::Number => [0x6f, 0xb1, 0xe8],
            ValueType::Bool => [0xd8, 0xa6, 0x4b],
            ValueType::Intent => [0x9a, 0x8c, 0xe0],
            ValueType::Action => [0xe0, 0x6c, 0x5f],
        }
    }
}

/// The household's standing plan, as recorded in the population bake.
///
/// Mirrors `scenario::population::Intent`. It is restated here rather than
/// imported so this crate stays a leaf: the graph format and the evaluator
/// have no reason to know a scenario exists, which is what lets the editor and
/// the unit tests run without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(rename_all = "snake_case")]
pub enum IntentValue {
    LeaveEarly,
    WaitAndSee,
    StayDefend,
}

impl IntentValue {
    pub const ALL: [IntentValue; 3] =
        [IntentValue::LeaveEarly, IntentValue::WaitAndSee, IntentValue::StayDefend];

    pub fn key(self) -> &'static str {
        match self {
            IntentValue::LeaveEarly => "leave_early",
            IntentValue::WaitAndSee => "wait_and_see",
            IntentValue::StayDefend => "stay_defend",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            IntentValue::LeaveEarly => "Leave early",
            IntentValue::WaitAndSee => "Wait and see",
            IntentValue::StayDefend => "Stay and defend",
        }
    }

    pub fn from_key(k: &str) -> Option<IntentValue> {
        IntentValue::ALL.into_iter().find(|i| i.key() == k)
    }
}

/// What the graph tells the household to do this decision tick.
///
/// These are the four states the movement layer in `abm` already understands.
/// A graph cannot invent a fifth — the point of the composer is to change
/// *when* people do these things, not to add new physics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Carry on as normal. What a graph produces when nothing fires.
    Stay,
    /// Start milling: gather people and belongings, then leave.
    Prepare,
    /// Leave now, skipping whatever preparation is left.
    EvacuateNow,
    /// Stay at the property and fight for it.
    Defend,
    /// Too late to move — take shelter where they are.
    Shelter,
}

impl ActionKind {
    pub const ALL: [ActionKind; 5] = [
        ActionKind::Stay,
        ActionKind::Prepare,
        ActionKind::EvacuateNow,
        ActionKind::Defend,
        ActionKind::Shelter,
    ];

    pub fn key(self) -> &'static str {
        match self {
            ActionKind::Stay => "stay",
            ActionKind::Prepare => "prepare",
            ActionKind::EvacuateNow => "evacuate_now",
            ActionKind::Defend => "defend",
            ActionKind::Shelter => "shelter",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ActionKind::Stay => "Stay put",
            ActionKind::Prepare => "Prepare to leave",
            ActionKind::EvacuateNow => "Evacuate now",
            ActionKind::Defend => "Defend property",
            ActionKind::Shelter => "Shelter in place",
        }
    }

    pub fn from_key(k: &str) -> Option<ActionKind> {
        ActionKind::ALL.into_iter().find(|a| a.key() == k)
    }
}

/// A proposed action and the strength of the proposal.
///
/// Priority is what resolves the common case of two branches firing at once —
/// "the order arrived" and "the fire is at the fence" are both true, and the
/// second has to win.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
pub struct ActionProposal {
    pub kind: ActionKind,
    pub priority: f32,
    /// Whether the proposal is live at all. An action node whose trigger is
    /// false still emits a value, so the trace can show it was considered and
    /// declined rather than showing nothing.
    pub fired: bool,
}

/// A value on a wire.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(tag = "t", content = "v", rename_all = "snake_case")]
pub enum Value {
    Number(f32),
    Bool(bool),
    Intent(IntentValue),
    Action(ActionProposal),
}

impl Value {
    pub fn ty(&self) -> ValueType {
        match self {
            Value::Number(_) => ValueType::Number,
            Value::Bool(_) => ValueType::Bool,
            Value::Intent(_) => ValueType::Intent,
            Value::Action(_) => ValueType::Action,
        }
    }

    pub fn as_number(&self) -> f32 {
        match self {
            Value::Number(n) => *n,
            // Reachable only through a graph the validator rejected; a number
            // is still the least surprising thing to hand the caller.
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n > 0.5,
            _ => false,
        }
    }

    pub fn as_intent(&self) -> IntentValue {
        match self {
            Value::Intent(i) => *i,
            _ => IntentValue::WaitAndSee,
        }
    }

    pub fn as_action(&self) -> ActionProposal {
        match self {
            Value::Action(a) => *a,
            _ => ActionProposal { kind: ActionKind::Stay, priority: 0.0, fired: false },
        }
    }

    /// One-line rendering for the trace panel.
    pub fn display(&self) -> String {
        match self {
            Value::Number(n) => format!("{n:.3}"),
            Value::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
            Value::Intent(i) => i.label().to_string(),
            Value::Action(a) => {
                if a.fired {
                    format!("{} @ {:.2}", a.kind.label(), a.priority)
                } else {
                    format!("({} withheld)", a.kind.label())
                }
            }
        }
    }
}
