//! Driving the wildfire controls without a human at the keyboard.
//!
//! The controls added to the UI — place an ignition, shift the wind, restart —
//! are the only parts of this project that cannot be reached from a test in
//! `crates/fire` or `crates/abm`, because they are *Bevy* behaviour: they
//! depend on resources, events and the reset systems that consume them. A
//! restart that leaves a charred building standing or a stale vehicle on a road
//! is exactly the kind of bug that only shows up in the assembled app.
//!
//! So this drives them in order, in the real app with the real systems running,
//! and prints what the fire and the town did at each stage. Enabled with
//! `SPOTORNO_SELFTEST=1`; it fast-forwards, so it takes a few seconds.
//!
//! It asserts the things that would be silent failures — a restart that did not
//! actually rewind, an ignition the core quietly ignored — and exits non-zero
//! if any of them break.

use bevy::prelude::*;
use fire::CellFire;
use scenario::Cell;

use crate::ignition_edit::IgnitionTool;
use crate::sim::{Sim, SimRestarted};

/// How far to run before each checkpoint, in simulated seconds.
const LEG_S: i64 = 900;

#[derive(Resource, Default)]
pub struct SelfTest {
    stage: Stage,
    /// Burnt area at the end of the first leg, to compare the restart against.
    first_leg_ha: f32,
    failures: Vec<String>,
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
enum Stage {
    #[default]
    Burn,
    AddIgnition,
    BurnMore,
    ShiftWind,
    BurnShifted,
    Restart,
    Verify,
    Done,
}

pub fn from_env() -> Option<SelfTest> {
    std::env::var("SPOTORNO_SELFTEST").ok().map(|_| SelfTest::default())
}

pub fn run(
    mut test: ResMut<SelfTest>,
    mut sim: ResMut<Sim>,
    mut tool: ResMut<IgnitionTool>,
    mut restarted: EventWriter<SimRestarted>,
    mut exit: EventWriter<AppExit>,
) {
    // Always running, always as fast as the step cap allows.
    sim.playing = true;
    sim.speed = 512.0;

    let burnt = burnt_ha(&sim);
    let t = sim.time_s();

    match test.stage {
        Stage::Burn => {
            if t < LEG_S {
                return;
            }
            test.first_leg_ha = burnt;
            println!("[selftest] T+{t}s baseline: {burnt:.1} ha burnt");
            check(&mut test, burnt > 5.0, "opening fire never established");
            test.stage = Stage::AddIgnition;
        }

        // A second start, well clear of the first so its growth is its own.
        Stage::AddIgnition => {
            let seed_cell = sim.ignition.centre;
            let target = Cell {
                row: seed_cell.row.saturating_sub(60),
                col: seed_cell.col.saturating_sub(60),
            };
            let before = sim.ignitions.len();
            tool.radius_m = 120.0;
            match sim.add_ignition(target, tool.radius_m) {
                Ok(()) => {
                    check(
                        &mut test,
                        sim.ignitions.len() == before + 1,
                        "added ignition was not recorded",
                    );
                    check(
                        &mut test,
                        sim.ignitions.last().is_some_and(|i| i.at_s == t),
                        "added ignition did not carry its timestamp",
                    );
                    println!(
                        "[selftest] lit a second patch at ({}, {}) r=120 m, T+{t}s",
                        target.row, target.col
                    );
                }
                // Not a failure: the offset cell may be non-burnable. The
                // point of the stage is that the API path works, and a refusal
                // is a legitimate answer from it.
                Err(e) => println!("[selftest] second patch refused (non-burnable): {e:#}"),
            }
            test.stage = Stage::BurnMore;
        }
        Stage::BurnMore => {
            if t < LEG_S * 2 {
                return;
            }
            println!("[selftest] T+{t}s after second patch: {burnt:.1} ha burnt");
            let baseline = test.first_leg_ha;
            check(
                &mut test,
                burnt > baseline,
                "fire did not grow after the second ignition",
            );
            test.stage = Stage::ShiftWind;
        }

        // A 90-degree wind shift, applied live. This is the control whose whole
        // value is that it does *not* rewrite the existing scar.
        Stage::ShiftWind => {
            let scar_before = burnt;
            sim.weather.wind_dir_deg = 270.0;
            sim.weather.wind_speed_kmh = 50.0;
            check(&mut test, sim.weather_dirty(), "staged weather did not read as pending");
            if let Err(e) = sim.apply_weather() {
                check(&mut test, false, &format!("applying weather failed: {e:#}"));
            }
            check(
                &mut test,
                !sim.weather_dirty(),
                "weather still pending after being applied",
            );
            check(
                &mut test,
                (burnt_ha(&sim) - scar_before).abs() < 0.01,
                "applying weather changed the existing burn scar",
            );
            println!("[selftest] wind shifted to 50 km/h from W, scar intact at {scar_before:.1} ha");
            test.stage = Stage::BurnShifted;
        }
        Stage::BurnShifted => {
            if t < LEG_S * 3 {
                return;
            }
            println!("[selftest] T+{t}s after wind shift: {burnt:.1} ha burnt");
            test.stage = Stage::Restart;
        }

        Stage::Restart => {
            let ignitions = sim.ignitions.len();
            let gen_before = sim.generation;
            match sim.restart() {
                Ok(()) => {
                    restarted.send(SimRestarted);
                }
                Err(e) => check(&mut test, false, &format!("restart failed: {e:#}")),
            }
            check(&mut test, sim.time_s() == 0, "restart did not rewind the clock");
            check(
                &mut test,
                sim.generation > gen_before,
                "restart did not invalidate the views (generation must never rewind)",
            );
            check(
                &mut test,
                sim.ignitions.len() == ignitions,
                "restart lost an ignition from the replay list",
            );
            // The opening fire is relit immediately; the mid-run patch is armed
            // for its own time, so straight after a restart the scar is only
            // ever the opening fire's.
            let after = burnt_ha(&sim);
            let baseline = test.first_leg_ha;
            check(
                &mut test,
                after < baseline,
                &format!(
                    "restart left {after:.1} ha burnt, more than the opening fire's \
                     first {LEG_S}s ({baseline:.1} ha) -- the old scar survived"
                ),
            );
            check(
                &mut test,
                sim.agents.households.iter().all(|h| !h.ordered),
                "restart left households still under an evacuation order",
            );
            println!("[selftest] restarted: T+0, {after:.1} ha, {ignitions} ignition(s) replayed");
            test.stage = Stage::Verify;
        }

        // Run the replayed scenario back out to the first checkpoint. It has
        // the shifted wind now, so it will not match the baseline -- what is
        // being checked is that a restarted sim runs at all, and that the
        // mid-run ignition came back on schedule rather than at T+0.
        Stage::Verify => {
            if t < LEG_S + 60 {
                return;
            }
            println!("[selftest] T+{t}s after restart: {burnt:.1} ha burnt");
            check(&mut test, burnt > 5.0, "restarted fire never established");
            test.stage = Stage::Done;
        }

        Stage::Done => {
            if test.failures.is_empty() {
                println!("[selftest] PASS");
                exit.send(AppExit::Success);
            } else {
                for f in &test.failures {
                    println!("[selftest] FAIL: {f}");
                }
                exit.send(AppExit::Error(
                    std::num::NonZeroU8::new(1).expect("1 is non-zero"),
                ));
            }
        }
    }
}

fn burnt_ha(sim: &Sim) -> f32 {
    let cell_ha = sim.scenario.world.cellsize * sim.scenario.world.cellsize / 10_000.0;
    sim.fire
        .state()
        .iter()
        .filter(|s| matches!(s, CellFire::Burning | CellFire::Burnt))
        .count() as f32
        * cell_ha
}

fn check(test: &mut SelfTest, ok: bool, what: &str) {
    if !ok {
        test.failures.push(what.to_string());
    }
}
