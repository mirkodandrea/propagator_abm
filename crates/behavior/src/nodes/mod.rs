//! The node library that ships with the game.
//!
//! Nothing here is special: every node is registered through the same
//! [`behavior_node!`](crate::behavior_node) macro a downstream crate would
//! use, and the editor cannot tell a built-in from one added later. The
//! grouping into files is for readers, not for the registry.

mod actions;
mod blocks;
mod logic;
mod observations;
mod outputs;
mod params;
mod person_blocks;
mod person_observations;
mod unit_blocks;
mod unit_observations;

/// The sink the model reads a household's decision from. Named here because the
/// validator and the compiler both need to find it, and a string literal
/// repeated in three places is a rename waiting to go wrong.
///
/// Reach these through [`Domain::decision_output`](crate::Domain::decision_output)
/// rather than by name wherever the domain is a variable.
pub const DECISION_OUTPUT: &str = "out.decision";
/// The same, for a suppression unit.
pub const UNIT_DECISION_OUTPUT: &str = "out.unit_decision";
/// The same, for one person who is away from their household.
pub const PERSON_DECISION_OUTPUT: &str = "out.person_decision";
pub const PREP_SCALE_OUTPUT: &str = "out.prep_scale";
pub const URGENCY_OUTPUT: &str = "out.urgency";
