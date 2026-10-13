//! Running the same fire forward many times from where it is now: the
//! building block of a stochastic forecast.
//!
//! A [`Snapshot`] is the core's own checkpoint of the game's fire (front
//! heaps, burnt cells, cut lines, water on the ground, pending boundary
//! conditions), so every member continues *this* fire rather than a fire
//! re-lit from its perimeter. Each member runs on its own core with its own
//! seed and weather. Nothing here touches the game's [`FireSim`]: reading a
//! snapshot changes nothing in the simulation.
//!
//! The snapshot owns its data and is `Send`, so a caller may run members on
//! another thread, all at once ([`Snapshot::run`]) or one at a time.

use anyhow::{Context, Result};
use propagator_core::{
    BoundaryConditions, FieldInput, FuelSystem, OobMode, Propagator, PropagatorCheckpoint,
    ResumeOptions,
};
use scenario::Scenario;

use crate::{FireSim, Weather};

/// One run of the ensemble: its seed and the weather it is driven by.
#[derive(Debug, Clone, Copy)]
pub struct Member {
    pub seed: u64,
    /// The weather from the snapshot's time on.
    pub now: Weather,
    /// A change of weather at an absolute simulated second, if one is due.
    pub later: Option<(i64, Weather)>,
}

/// The game's fire at one instant, ready to be run forward.
#[derive(Clone)]
pub struct Snapshot {
    checkpoint: PropagatorCheckpoint,
    fuels: FuelSystem,
    /// Simulated second of the snapshot.
    pub time_s: i64,
    pub rows: usize,
    pub cols: usize,
}

impl FireSim {
    /// The fire now, as the core holds it. Reads only.
    pub fn snapshot(&self, scn: &Scenario) -> Result<Snapshot> {
        Ok(Snapshot {
            checkpoint: self.sim.checkpoint(),
            fuels: crate::fuels::eu12_fuel_system(&scn.fuel_defs)?,
            time_s: self.time_s,
            rows: self.world.fire_rows,
            cols: self.world.fire_cols,
        })
    }
}

fn weather_bc(time: i64, w: Weather) -> BoundaryConditions {
    BoundaryConditions {
        time,
        wind_dir: Some(FieldInput::Scalar(w.wind_dir_deg)),
        wind_speed: Some(FieldInput::Scalar(w.wind_speed_kmh)),
        moisture: Some(FieldInput::Scalar(w.moisture_pct)),
        ..Default::default()
    }
}

impl Snapshot {
    /// Run `member` for `horizon_s` simulated seconds from the snapshot and
    /// return, per fire cell, the absolute simulated second it was first lit
    /// (before or during the run), `i32::MIN` where it never burnt.
    /// Deterministic for the same snapshot and member.
    pub fn run(&self, member: &Member, horizon_s: i64) -> Result<Vec<i32>> {
        let mut sim = Propagator::from_checkpoint(
            &self.checkpoint,
            ResumeOptions {
                fuels: Some(self.fuels.clone()),
                oob_mode: OobMode::Ignore,
                seed: Some(member.seed),
                // the caller decides how members share the machine
                n_threads: Some(1),
                ..Default::default()
            },
        )
        .context("resuming the fire from its snapshot")?;
        sim.set_boundary_conditions(weather_bc(self.time_s, member.now)).context("member weather")?;
        if let Some((at, w)) = member.later {
            if at >= self.time_s && at < self.time_s + horizon_s {
                sim.set_boundary_conditions(weather_bc(at, w)).context("member weather change")?;
            }
        }
        sim.step_window(horizon_s).context("running a member")?;
        let fire = sim.get_fire().context("member fire")?;
        let arrival = sim.get_arrival_time().context("member arrival")?;
        Ok(fire[0]
            .as_slice()
            .iter()
            .zip(arrival[0].as_slice())
            .map(|(&lit, &t)| if lit != 0 { t } else { i32::MIN })
            .collect())
    }
}
