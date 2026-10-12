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

use abm::drive::{Drive, Vehicle};
use abm::suppression::ENGINE_SPEED;
use scenario::Pos;

/// The teams carrying orders.
#[derive(Debug, Clone, Copy)]
pub struct Messengers {
    /// How many teams there are, staged at the stations in turn.
    pub teams: usize,
    /// Households one team tells per minute once at the door; the way there
    /// is driven on the open roads and costs what it costs.
    pub per_min: f32,
    /// Mobile alerts still reach their households over the network.
    pub broadcast: bool,
    /// A team goes to the highest-priority district with households left to
    /// tell, and is diverted on the road when a higher one opens; otherwise
    /// to the open district with the fewest teams. Either way a district
    /// already has enough teams once it has one per household left.
    pub by_priority: bool,
    /// Trial only: the teams read the priorities backwards, the unranked
    /// districts first, so a wrong order of the teams alone can be measured
    /// with the units' priorities right.
    pub reversed: bool,
}

/// One district's households still to be told.
#[derive(Debug, Clone)]
pub struct Round {
    pub district: usize,
    pub evacuate: bool,
    pub queue: VecDeque<usize>,
}

/// How a team drives: a police or carabinieri car, on carriageways.
pub const VEHICLE: Vehicle = Vehicle { drivable_only: true, road_speed: ENGINE_SPEED, path_speed: ENGINE_SPEED };

/// What a team is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Task {
    /// Waiting for a district to go to; looks again at `retry_s`. Drives back
    /// to its station meanwhile.
    Idle { retry_s: i64 },
    /// On the road to `district`.
    Driving { district: usize },
    /// Telling the households of `district`; `carry` is the fraction of the
    /// next household already spent.
    Door { district: usize, carry: f32 },
    /// Caught by the fire ([`abm::suppression::BURNOVER_S`]).
    Lost,
}

#[derive(Debug, Clone)]
pub struct Team {
    pub pos: Pos,
    /// Bearing of travel, radians, for rendering.
    pub heading: f32,
    /// The station it started from, and pulls back to.
    pub base: Pos,
    /// Its way on the roads, driven as the engines drive ([`abm::drive`]).
    pub drive: Drive,
    pub task: Task,
    pub told: usize,
    /// Flame exposure, seconds, in the units' currency
    /// ([`abm::suppression::BURNOVER_S`]).
    pub heat_s: f32,
    /// Times the team was sent elsewhere while on the road.
    pub diverted: u32,
    /// Times it left a district the fire came too close to
    /// ([`abm::suppression::WORK_LIMIT`]).
    pub left: u32,
    /// Districts no open road led to or the fire drove it out of, each
    /// skipped until the given time.
    pub unreachable: Vec<(usize, i64)>,
}

impl Team {
    pub fn new(pos: Pos) -> Team {
        Team {
            pos,
            heading: 0.0,
            base: pos,
            drive: Drive::default(),
            task: Task::Idle { retry_s: 0 },
            told: 0,
            heat_s: 0.0,
            diverted: 0,
            left: 0,
            unreachable: vec![],
        }
    }

    /// Whether `d` is skipped at `now`.
    pub fn cut_off(&self, d: usize, now: i64) -> bool {
        self.unreachable.iter().any(|&(u, until)| u == d && now < until)
    }

    /// The district the team is going to or working in.
    pub fn district(&self) -> Option<usize> {
        match self.task {
            Task::Driving { district } | Task::Door { district, .. } => Some(district),
            Task::Idle { .. } | Task::Lost => None,
        }
    }
}
