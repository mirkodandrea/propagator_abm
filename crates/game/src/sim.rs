//! Driving the fire model from the Bevy loop.
//!
//! The fire runs *in-process* on the PROPAGATOR Rust core — there is no
//! external process and no Python at runtime. Simulated time is decoupled from
//! frame time by a fixed accumulator: at 1x, one wall-clock second is one
//! simulated second; the speed control multiplies that. Stepping is capped per
//! frame so a slow step cannot spiral into a death loop.

use bevy::prelude::*;
use fire::{FireSim, IgnitionPlan, Weather};
use scenario::Scenario;

/// Radius of the fire the scenario opens with: already a going fire at the
/// WUI edge, which is the situation an incident commander is called to.
///
/// Sized empirically (`crates/fire/tests/sizing.rs`). A fire only travels
/// ~500-800 m in a two-hour initial-attack window, so a small ignition simply
/// cannot reach the coastal settlement from anywhere it can sustain itself.
/// Measured over a 2 h run: 150 m radius threatens 37-80 households, 250 m
/// threatens 137 while burning half as much ground as 500 m. 250 m it is.
pub const START_RADIUS_M: f32 = 250.0;

/// Never advance more than this much simulated time in a single frame, however
/// far behind the accumulator has fallen.
const MAX_STEP_PER_FRAME_S: f32 = 30.0;
/// Granularity handed to the core. Small enough that the fire front updates
/// smoothly, large enough that the event heap isn't churned pointlessly.
const STEP_QUANTUM_S: i64 = 2;

#[derive(Resource)]
pub struct Sim {
    pub fire: FireSim,
    pub scenario: Scenario,
    pub playing: bool,
    /// Simulated seconds per wall-clock second.
    pub speed: f32,
    accumulator: f32,
    /// Bumped whenever the fire state changes, so views rebuild only then.
    pub generation: u64,
    pub ignition: IgnitionPlan,
}

impl Sim {
    pub fn new(scenario: Scenario, weather: Weather, seed: u64) -> anyhow::Result<Sim> {
        let mut fire = FireSim::new(&scenario, weather, seed)?;
        // A going fire at the WUI edge, not a single cell: see
        // FireSim::ignite_patch and fire::ignition for why both the size and
        // the placement matter.
        let ignition = fire::plan_ignition(&scenario, weather.wind_dir_deg, START_RADIUS_M);
        // println, not info!: Sim::new runs before Bevy installs its logger.
        println!(
            "ignition ({}, {}) r={:.0} m: {} households downwind, corridor {:.0}% burnable",
            ignition.centre.row,
            ignition.centre.col,
            ignition.radius_m,
            ignition.households_downwind,
            ignition.corridor_fuel * 100.0
        );
        fire.ignite_patch(ignition.centre, ignition.radius_m, &scenario)?;
        Ok(Sim {
            fire,
            scenario,
            // SPOTORNO_AUTOPLAY=1 starts running immediately, for screenshots
            // and for headless timing runs.
            playing: std::env::var("SPOTORNO_AUTOPLAY").is_ok(),
            speed: 8.0,
            accumulator: 0.0,
            generation: 0,
            ignition,
        })
    }

    pub fn time_s(&self) -> i64 {
        self.fire.time_s()
    }

    /// `HH:MM:SS` since ignition, for the HUD.
    pub fn clock(&self) -> String {
        let t = self.fire.time_s();
        format!("{:02}:{:02}:{:02}", t / 3600, (t / 60) % 60, t % 60)
    }
}

pub fn step_fire(mut sim: ResMut<Sim>, time: Res<Time>) {
    if !sim.playing {
        return;
    }
    let dt = time.delta_seconds().min(0.25) * sim.speed;
    sim.accumulator = (sim.accumulator + dt).min(MAX_STEP_PER_FRAME_S);

    let whole = sim.accumulator as i64;
    if whole < STEP_QUANTUM_S {
        return;
    }
    let advance = (whole / STEP_QUANTUM_S) * STEP_QUANTUM_S;
    sim.accumulator -= advance as f32;

    match sim.fire.advance(advance) {
        Ok(_) => sim.generation += 1,
        Err(e) => {
            error!("fire core failed: {e:#}");
            sim.playing = false;
        }
    }
}
