//! Who carries the order to the door (headless trial, 2026-10-11).
//!
//! Off by default ([`Game::messengers`](crate::Game) is `None`): every order
//! then travels over each household's own channel, 90 s to 20 minutes, however
//! many districts are ordered at once. A wildfire evacuation in Italy is
//! carried door to door by the municipal police, the carabinieri and the
//! volunteers, with loudspeakers: a few teams, and a district ordered for
//! nothing takes them away from the one in the fire's path. That trade-off is
//! what this module puts in the model, to be measured before the game uses it.
//!
//! A district ordered (or pre-alerted) joins a round: its households, nearest
//! its centre first, each told in person when a team reaches it
//! ([`abm::Abm::tell_in_person`]). Households already pre-alerted are
//! listening and hear the order over their channel as before; with
//! `broadcast`, so do those on a mobile alert.

use std::collections::VecDeque;

/// The teams carrying orders.
#[derive(Debug, Clone, Copy)]
pub struct Messengers {
    pub teams: f32,
    /// Households one team tells per minute, travel included.
    pub per_min: f32,
    /// Mobile alerts still reach their households over the network.
    pub broadcast: bool,
    /// All teams on the highest-priority district with households left to
    /// tell; otherwise they are shared equally among the open rounds.
    pub by_priority: bool,
}

/// One district's households still to be told.
#[derive(Debug, Clone)]
pub struct Round {
    pub district: usize,
    pub evacuate: bool,
    pub queue: VecDeque<usize>,
    pub progress: f32,
}
