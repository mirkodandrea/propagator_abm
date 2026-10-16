//! The suppression laboratory: headless analysis of how the teams put the
//! fire out, alone and together, on simplified synthetic territories.
//!
//! - [`synth`]: the territories (flat or sloped, uniform or patchy fuel, one
//!   road or a lattice, one or two villages).
//! - [`strategy`]: the coordination policies standing in for the player.
//! - [`telemetry`]: what is measured, every step of every run.
//! - [`run`]: one job, and a batch on all cores.
//! - [`report`]: CSV and Markdown.
//!
//! Everything goes through [`crate::Game`], the one authority; the laboratory
//! only reads it and gives the orders the player could give.

pub mod report;
pub mod run;
pub mod strategy;
pub mod synth;
pub mod telemetry;
