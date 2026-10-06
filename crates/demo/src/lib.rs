//! The kiosk demo (`docs/demo-spec.md`): three fictional towns, each built to
//! teach one thing, played for three minutes by someone who has never seen the
//! game.
//!
//! This crate is a leaf above the model. It changes no number in `scenario`,
//! `fire`, `abm` or `behavior` (spec §9.1): the towns are data, and everything
//! that makes them a *mission* -- where the fire starts, what the wind does,
//! what the outcome card counts -- lives here, where the headless tests in
//! `tests/` can measure it with no window.

pub mod mission;
pub mod run;

pub use mission::{spec, Spec, WindShift, ALL};
pub use run::{Order, Outcome, Run, Tally, STEP_S};
