//! The kiosk demo (`docs/demo-spec.md`): three fictional towns, each built to
//! teach one thing, played for three minutes by someone who has never seen the
//! game.
//!
//! This crate is a leaf above the model. It changes no number in `scenario`,
//! `fire`, `abm` or `behavior` (spec §9.1): the towns are data, and everything
//! that makes them a *mission* -- where the fire starts, what the wind does,
//! what the outcome card counts -- lives here, where the headless tests in
//! `tests/` can measure it with no window.

pub mod cost;
pub mod mission;
pub mod policy;
pub mod sweep;
pub mod run;
pub mod weather;

pub use mission::{spec, Climate, Spec, WindShift, ALL};
pub use weather::{draw, draw_with, Draw, Forecast, ISSUE_2_AT_S};
pub use run::{Order, Outcome, Run, Tally, Variant, STEP_S};
