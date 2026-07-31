//! The node library that ships with the game.
//!
//! Nothing here is special: every node is registered through the same
//! [`behavior_node!`](crate::behavior_node) macro a downstream crate would
//! use, and the editor cannot tell a built-in from one added later. The
//! grouping into files is for readers, not for the registry.

mod actions;
mod logic;
mod observations;
mod outputs;
mod params;

/// The one sink the model reads a decision from. Named here because the
/// validator and the compiler both need to find it, and a string literal
/// repeated in three places is a rename waiting to go wrong.
pub const DECISION_OUTPUT: &str = "out.decision";
pub const PREP_SCALE_OUTPUT: &str = "out.prep_scale";
pub const URGENCY_OUTPUT: &str = "out.urgency";
