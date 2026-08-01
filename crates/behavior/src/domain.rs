//! Which kind of agent a behaviour is about.
//!
//! The composer started as one editor for one agent, and every part of it —
//! the observation, the action set, the palette, the test bench — silently
//! assumed a household. Suppression units are agents in exactly the same sense
//! and want exactly the same treatment, so the assumption is now a value.
//!
//! ### One domain at a time, deliberately
//!
//! A [`BehaviorGraph`](crate::BehaviorGraph) belongs to exactly one domain, and
//! a node declares which domain it belongs to or that it belongs to any. The
//! editor shows one domain's palette at a time and the validator rejects a
//! foreign node, so there is no such thing as a graph that half-runs. The
//! alternative — one palette with everything in it and a runtime check — was
//! rejected on the same grounds the port types were: a scientist should not be
//! able to build the wrong thing and find out later.
//!
//! ### What the domains do *not* share
//!
//! Nothing but the arithmetic. The three observations have almost no field in
//! common beyond the clock and the jitter, the action sets are disjoint, and
//! each has its own decision sink. What they share is the machinery: one
//! registry, one validator, one evaluator, one editor.
//!
//! ### Why a person is not a household
//!
//! [`Domain::Person`] exists for the one case a household cannot answer: a
//! person who is not with their household. Everyone else evacuates as a family,
//! at the pace of the slowest member, and giving each of them a graph would be
//! a per-person assumption written as a per-family one — the exact confusion
//! [`Domain::agent_label`] is worded to avoid.

use serde::{Deserialize, Serialize};

#[cfg(feature = "reflect")]
use bevy_reflect::Reflect;

use crate::value::ActionKind;

/// The kinds of agent a behaviour can be authored for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    /// Civilians, as households. The domain the composer shipped with, and the
    /// default for a graph file that does not say — every graph written before
    /// domains existed is one of these.
    Household,
    /// Hand crews, engines and air tankers: the commander's suppression
    /// resources.
    SuppressionUnit,
    /// One person who is not with their household — someone who was out when it
    /// started, or who has become separated from the family since.
    ///
    /// Deliberately narrow. A person who is at home is *part of* a household and
    /// has no decisions of their own to make in this model: the family leaves
    /// together, at the pace of its slowest member. What a separated person
    /// decides is the one thing the household cannot decide for them — whether
    /// to keep walking out, stop where they are, or go back for the others.
    Person,
}

impl Default for Domain {
    fn default() -> Self {
        Domain::Household
    }
}

impl Domain {
    pub const ALL: [Domain; 3] = [Domain::Household, Domain::Person, Domain::SuppressionUnit];

    /// Stable key, used in the saved graph and in a subtype's file.
    pub fn key(self) -> &'static str {
        match self {
            Domain::Household => "household",
            Domain::SuppressionUnit => "suppression_unit",
            Domain::Person => "person",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Domain::Household => "Households",
            Domain::SuppressionUnit => "Suppression units",
            Domain::Person => "Separated people",
        }
    }

    /// What one agent of this domain *is*, for the editor's header. Worth
    /// spelling out: "civilians" is a population and "a household" is the thing
    /// the graph actually runs on, and confusing the two is how a per-person
    /// assumption ends up written as a per-family one.
    pub fn agent_label(self) -> &'static str {
        match self {
            Domain::Household => "one household",
            Domain::SuppressionUnit => "one unit",
            Domain::Person => "one separated person",
        }
    }

    pub fn doc(self) -> &'static str {
        match self {
            Domain::Household => {
                "A household deciding whether to prepare, leave, defend or shelter. \
                 The graph replaces the departure decision only: perception, milling, \
                 movement and lethality are the model's."
            }
            Domain::SuppressionUnit => {
                "A crew, engine or aircraft deciding whether to carry on with the \
                 order it was given, break off for water, pull back, hold, or return \
                 to staging. Where it is sent stays the commander's decision — a graph \
                 cannot produce a map position."
            }
            Domain::Person => {
                "One person who is away from their household, deciding whether to keep \
                 walking out, stop and shelter, or go back for the family. People at \
                 home do not run this — they leave with the household, which is the \
                 unit the evacuation model is built on."
            }
        }
    }

    /// The one sink the model reads this domain's answer from.
    pub fn decision_output(self) -> &'static str {
        match self {
            Domain::Household => crate::nodes::DECISION_OUTPUT,
            Domain::SuppressionUnit => crate::nodes::UNIT_DECISION_OUTPUT,
            Domain::Person => crate::nodes::PERSON_DECISION_OUTPUT,
        }
    }

    /// What the agent does when the graph proposes nothing.
    ///
    /// Not the same shape of answer in the two domains, and the difference
    /// matters: a household with nothing firing has not been told to do
    /// anything and stays put, while a unit with nothing firing has already been
    /// told what to do by the commander and gets on with it.
    pub fn default_action(self) -> ActionKind {
        match self {
            Domain::Household => ActionKind::Stay,
            Domain::SuppressionUnit => ActionKind::Continue,
            // Same shape as a unit's: someone already walking out with nothing
            // firing keeps walking. Stopping them would need a branch to say so.
            Domain::Person => ActionKind::Remain,
        }
    }

    pub fn from_key(k: &str) -> Option<Domain> {
        Domain::ALL.into_iter().find(|d| d.key() == k)
    }
}
