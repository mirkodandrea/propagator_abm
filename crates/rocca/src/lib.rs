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
pub mod district;
pub mod evac;
pub mod forecast;
pub mod game;
pub mod hex;
pub mod score;
pub mod tutorial;
pub mod words;

pub use case::{Case, Territory};
pub use evac::{EvacPlan, State};
pub use game::{DistrictOutcome, Game, LogEntry, MovePlan, Outcome, Team, PAUSE_COOLDOWN_S, PAUSE_S, PAUSE_X, RUN_X, STEP_S};
pub use hex::{Axial, Hex, HexGrid, HEX_M};
