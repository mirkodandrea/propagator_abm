//! Isola di Roccavento: the one simulation and gameplay authority.
//!
//! The player pauses ([`Game::pause`]) to see the expert's forecast
//! ([`forecast`]) and give orders: teams to hexes ([`hex`]), one district at a
//! time to evacuate ([`evac`]). The simulation (fire, teams, evacuation)
//! decides what happens ([`game`]); the stars weigh it against the same fire
//! with no orders ([`score`]). The tutorial is a script on the same engine
//! ([`tutorial`]). Territory and fires come from the scenario's `game.json`
//! ([`case`]).

pub mod case;
pub mod chat;
pub mod district;
pub mod evac;
pub mod forecast;
pub mod game;
pub mod hex;
// The laboratory writes files and links directories: native only, not in the browser build.
#[cfg(not(target_arch = "wasm32"))]
pub mod lab;
pub mod score;
pub mod tutorial;
pub mod words;

pub use case::{Case, Territory};
pub use chat::{ChatMsg, Kind, Voice};
pub use evac::{EvacPlan, State};
pub use game::{ClosedStretch, DistrictOutcome, Game, LogEntry, MovePlan, Outcome, Team, Tuning, TEAM_DANGER_M, TEAM_FIRE_CRIT_M, REFILL_S, RUN_X, STEP_S};
pub use hex::{Axial, Hex, HexGrid, HEX_M, TEAM_REACH_HEXES};
