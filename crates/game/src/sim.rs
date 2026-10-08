//! The kiosk's handle on the one simulation and gameplay authority,
//! [`rocca::Game`]. Nothing here steps a model or decides anything: it only
//! turns frame time into game steps at the chosen speed, and counts
//! generations so views rebuild when the state changed.

use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use rocca::{Game, STEP_S};

#[derive(Resource)]
pub struct Sim {
    pub game: Game,
    pub data: PathBuf,
    /// Simulated seconds per real second. 0 = paused (planning, ×0).
    pub speed: f32,
    accumulator: f32,
    /// Bumped whenever the game state changes, so views rebuild only then.
    /// Monotonic across restarts: a staleness token, not a step count.
    pub generation: u64,
}

/// Raised on the frame a new game started, so views holding state the game no
/// longer explains (a burning roof, a vehicle entity) can drop it.
#[derive(Event)]
pub struct SimRestarted;

impl Deref for Sim {
    type Target = Game;
    fn deref(&self) -> &Game {
        &self.game
    }
}

impl DerefMut for Sim {
    fn deref_mut(&mut self) -> &mut Game {
        &mut self.game
    }
}

/// Most game steps a frame may run, so a slow step cannot spiral.
const MAX_STEPS_PER_FRAME: usize = 40;

impl Sim {
    pub fn new(data: &Path, case: &str, seed: u64) -> anyhow::Result<Sim> {
        Ok(Sim { game: Game::new(data, case, seed)?, data: data.to_path_buf(), speed: 0.0, accumulator: 0.0, generation: 0 })
    }

    /// A new game on the same territory: `case` and `seed` from scratch. The
    /// operator's manual restart; nothing else calls it.
    pub fn restart(&mut self, case: &str, seed: u64) -> anyhow::Result<()> {
        self.game = Game::new(&self.data, case, seed)?;
        self.speed = 0.0;
        self.accumulator = 0.0;
        self.generation += 1;
        Ok(())
    }

    /// Advance by `real_s` of wall time at the current speed. Effects depend on
    /// simulated time only: steps are always [`STEP_S`] long.
    pub fn tick(&mut self, real_s: f32) -> anyhow::Result<bool> {
        if self.speed <= 0.0 {
            return Ok(false);
        }
        self.accumulator += real_s * self.speed;
        let mut stepped = false;
        for _ in 0..MAX_STEPS_PER_FRAME {
            if self.accumulator < STEP_S as f32 {
                break;
            }
            self.game.step()?;
            self.accumulator -= STEP_S as f32;
            stepped = true;
        }
        // Behind by more than a frame's cap: drop the backlog, keep real time.
        self.accumulator = self.accumulator.min(STEP_S as f32);
        if stepped {
            self.generation += 1;
        }
        Ok(stepped)
    }
}
