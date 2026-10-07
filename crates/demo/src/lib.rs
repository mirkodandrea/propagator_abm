//! The kiosk demo (`docs/demo-spec.md`): Rocca Ventosa, five turns, a handful
//! of scarce resources, a verdict against the same fire with no orders.
//!
//! This crate is a leaf above the model. The turn game is [`Session`] (the
//! engine, `session.rs`) speaking the vocabulary in [`turn`]; the headless
//! twin underneath it is [`Run`], whose [`Referee`] keeps the one set of
//! books. Everything that makes the town a *mission* -- where the fire starts,
//! what the wind does, what the verdict counts -- lives here, where the
//! headless tests in `tests/` can measure it with no window.
//!
//! The older modules (`cost`, `trust`, `why`, `refusal`, the instant-order
//! `policy`) belong to the scrapped real-time design; the kiosk still builds
//! against them until it is rebuilt on [`Session`] (milestone 3).

pub mod cost;
pub mod district;
pub mod event;
pub mod mission;
pub mod policy;
pub mod refusal;
pub mod run;
pub mod session;
pub mod sweep;
pub mod trust;
pub mod turn;
pub mod turn_policy;
pub mod weather;
pub mod why;

pub use district::{District, Level};
pub use event::{Event, EventKind};
pub use mission::{spec, Climate, Spec, WindShift, ALL};
pub use run::{Order, Outcome, Parts, Referee, Run, Tally, Variant, STEP_S};
pub use session::{Session, TOWN};
pub use turn::*;
pub use weather::{draw, draw_with, Draw, Forecast, ISSUE_2_AT_S};
pub use why::{Facts, Why, WhyKind};
