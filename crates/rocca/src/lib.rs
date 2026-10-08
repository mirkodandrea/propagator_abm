//! Rocca Ventosa: the one simulation and gameplay authority.
//!
//! The player ranks places and gives civil orders ([`plan`]); the
//! [`coordinator`] turns the ranking into posts; the simulation (fire, civilians,
//! units) decides what actually happens ([`game`]). Territory and fires come
//! from the scenario's `game.json` ([`case`]), written by the Scenario Factory.

pub mod case;
pub mod coordinator;
pub mod crisis;
pub mod district;
pub mod game;
pub mod plan;
pub mod words;

pub use case::{Case, Territory};
pub use coordinator::{Post, Proposal};
pub use crisis::Crisis;
pub use game::{DistrictOutcome, Game, Outcome, STEP_S};
pub use plan::{Civil, Plan};
