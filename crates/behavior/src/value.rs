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

/// What the graph tells an agent to do this decision tick.
///
/// One flat set covering both domains, tagged by [`ActionKind::domain`]. Flat
/// rather than nested so a proposal, a wire and a decision are the same shape
/// whichever kind of agent produced them; the domains stay apart because a
/// graph can only contain its own domain's action nodes, which the validator
/// enforces.
///
/// Neither half is extensible from a graph. The point of the composer is to
/// change *when* an agent does these things, not to add new physics — adding an
/// action means teaching the movement or suppression layer what to do about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    // --- households --------------------------------------------------------
    /// Carry on as normal. What a household's graph produces when nothing fires.
    Stay,
    /// Start milling: gather people and belongings, then leave.
    Prepare,
    /// Leave now, skipping whatever preparation is left.
    EvacuateNow,
    /// Stay at the property and fight for it.
    Defend,
    /// Too late to move — take shelter where they are.
    Shelter,
    /// Leave the house on foot for the nearest survivable open ground: a car
    /// park, a cleared field, the beach. Not a refuge and not organised — the
    /// shelter of last resort a household takes when the house is not
    /// defensible and the road is not usable.
    ShelterNearby,
    /// Leave on foot for the water's edge. Only reachable in a coastal window,
    /// and only a way *out* when a boat lift is coming — otherwise it is the
    /// place people stand in the water and wait, which is what Mati was.
    MakeForShore,

    // --- suppression units -------------------------------------------------
    /// Get on with the order. What a unit's graph produces when nothing fires,
    /// which is the common case: a unit already has instructions.
    Continue,
    /// Break off and pull back to staging. The safety override, and the one
    /// thing a unit does against its orders.
    Withdraw,
    /// Break off for water and resume afterwards. Meaningless for a hand crew.
    Refill,
    /// Stop where you are and stand by. Unlike `Withdraw` this does not travel:
    /// it is a unit that has decided the job in front of it is not worth doing.
    HoldPosition,
    /// Go back to staging and await orders. Unlike `Withdraw` this is not a
    /// safety action and does not carry the withdrawal note.
    ReturnToBase,

    // --- separated people ---------------------------------------------------
    /// Carry on doing whatever they were doing. What a separated person's graph
    /// produces when nothing fires, and — since the model already has them
    /// walking out — usually means "keep walking".
    Remain,
    /// Make for the nearest refuge on foot. The guidance answer, and what the
    /// model has always done for people who were out when it started.
    WalkOut,
    /// Stop and shelter where they are. For someone whose way out is worse than
    /// standing still, which on a cut road it often is.
    TakeShelter,
    /// Walk back to the household's home. The behaviour every evacuation study
    /// finds and no evacuation plan wants, and the reason this domain exists.
    HeadHome,
    /// Make for the nearest survivable open ground rather than for a refuge.
    /// What someone does when the way out is worse than the nearest clearing.
    WalkToOpenGround,
    /// Make for the water's edge. The Rhodes behaviour when a lift is coming,
    /// and the Mati one when it is not.
    WalkToShore,
}

impl ActionKind {
    pub const ALL: [ActionKind; 18] = [
        ActionKind::Stay,
        ActionKind::Prepare,
        ActionKind::EvacuateNow,
        ActionKind::Defend,
        ActionKind::Shelter,
        ActionKind::ShelterNearby,
        ActionKind::MakeForShore,
        ActionKind::Continue,
        ActionKind::Withdraw,
        ActionKind::Refill,
        ActionKind::HoldPosition,
        ActionKind::ReturnToBase,
        ActionKind::Remain,
        ActionKind::WalkOut,
        ActionKind::TakeShelter,
        ActionKind::HeadHome,
        ActionKind::WalkToOpenGround,
        ActionKind::WalkToShore,
    ];

    pub fn key(self) -> &'static str {
        match self {
            ActionKind::Stay => "stay",
            ActionKind::Prepare => "prepare",
            ActionKind::EvacuateNow => "evacuate_now",
            ActionKind::Defend => "defend",
            ActionKind::Shelter => "shelter",
            ActionKind::ShelterNearby => "shelter_nearby",
            ActionKind::MakeForShore => "make_for_shore",
            ActionKind::Continue => "continue",
            ActionKind::Withdraw => "withdraw",
            ActionKind::Refill => "refill",
            ActionKind::HoldPosition => "hold_position",
            ActionKind::ReturnToBase => "return_to_base",
            ActionKind::Remain => "remain",
            ActionKind::WalkOut => "walk_out",
            ActionKind::TakeShelter => "take_shelter",
            ActionKind::HeadHome => "head_home",
            ActionKind::WalkToOpenGround => "walk_to_open_ground",
            ActionKind::WalkToShore => "walk_to_shore",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ActionKind::Stay => "Stay put",
            ActionKind::Prepare => "Prepare to leave",
            ActionKind::EvacuateNow => "Evacuate now",
            ActionKind::Defend => "Defend property",
            ActionKind::Shelter => "Shelter in place",
            ActionKind::ShelterNearby => "Make for open ground",
            ActionKind::MakeForShore => "Make for the shore",
            ActionKind::Continue => "Carry on",
            ActionKind::Withdraw => "Withdraw",
            ActionKind::Refill => "Break off for water",
            ActionKind::HoldPosition => "Hold position",
            ActionKind::ReturnToBase => "Return to staging",
            ActionKind::Remain => "Carry on",
            ActionKind::WalkOut => "Walk out",
            ActionKind::TakeShelter => "Shelter where they are",
            ActionKind::HeadHome => "Head home",
            ActionKind::WalkToOpenGround => "Make for open ground",
            ActionKind::WalkToShore => "Make for the shore",
        }
    }

    pub fn domain(self) -> crate::domain::Domain {
        use crate::domain::Domain;
        match self {
            ActionKind::Stay
            | ActionKind::Prepare
            | ActionKind::EvacuateNow
            | ActionKind::Defend
            | ActionKind::Shelter
            | ActionKind::ShelterNearby
            | ActionKind::MakeForShore => Domain::Household,
            ActionKind::Remain
            | ActionKind::WalkOut
            | ActionKind::TakeShelter
            | ActionKind::HeadHome
            | ActionKind::WalkToOpenGround
            | ActionKind::WalkToShore => Domain::Person,
            _ => Domain::SuppressionUnit,
        }
    }

    /// This domain's actions, in the order they are offered.
    pub fn for_domain(d: crate::domain::Domain) -> impl Iterator<Item = ActionKind> {
        ActionKind::ALL.into_iter().filter(move |a| a.domain() == d)
    }

    pub fn from_key(k: &str) -> Option<ActionKind> {
        ActionKind::ALL.into_iter().find(|a| a.key() == k)
    }
}

/// Which suppression resource a unit is.
///
/// Restated here rather than imported from `abm::suppression` for the same
/// reason [`IntentValue`] is restated: this crate is a leaf, and the editor and
/// its tests run with no scenario, no fire model and no road network loaded.
/// `abm` owns the one conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(rename_all = "snake_case")]
pub enum UnitKindKey {
    HandCrew,
    Engine,
    AirTanker,
}

impl UnitKindKey {
    pub const ALL: [UnitKindKey; 3] =
        [UnitKindKey::HandCrew, UnitKindKey::Engine, UnitKindKey::AirTanker];

    pub fn key(self) -> &'static str {
        match self {
            UnitKindKey::HandCrew => "hand_crew",
            UnitKindKey::Engine => "engine",
            UnitKindKey::AirTanker => "air_tanker",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            UnitKindKey::HandCrew => "Hand crew",
            UnitKindKey::Engine => "Engine",
            UnitKindKey::AirTanker => "Air tanker",
        }
    }

    pub fn from_key(k: &str) -> Option<UnitKindKey> {
        UnitKindKey::ALL.into_iter().find(|u| u.key() == k)
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
