//! What each real scenario should open with, and why.
//!
//! `sim::START_RADIUS_M` and `Weather::default()` (tramontana, 35 km/h, 6%
//! moisture) were tuned once against Spotorno alone (`sizing.rs`), and every
//! other real scenario -- `mati`, `pedrogao`, `rhodes` -- inherited them
//! unmeasured: a north wind blowing toward the sea has no reason to be the
//! right start for a hillside above the Attica coast. This sweeps a fine
//! radius grid at a wind bearing grounded in the place's own historical fire
//! -- not whichever direction happens to maximise threatened households --
//! and reports what an established, meaningfully-threatening two-hour fire
//! looks like there. The chosen values are pinned in
//! `crates/game/src/sim.rs::opening_conditions`; a first, coarser sweep over
//! 8 compass bearings x 3 radii is what narrowed the search to these
//! bearings in the first place and is not kept, since this file supersedes
//! it.
//!
//! Bearings picked from the historical event each scenario is modelled on,
//! not from whatever maximises threatened households:
//!   - `mati`: WNW ~293deg, 32-56 km/h sustained (BAMS 2019 analysis of the
//!     2018 Attica fire).
//!   - `pedrogao`: the fire's own ~90deg rotation put it on a wind from
//!     roughly NW, ~315deg, driven by convective outflow ahead of the storm
//!     that overran the N236-1.
//!   - `rhodes`: meltemi is northwesterly-to-westerly in the southeastern
//!     Aegean (as opposed to the northerly form further north), ~315deg.

use fire::{plan_with_standoff, CellFire, FireSim, Weather};
use scenario::Scenario;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

const STANDOFF_M: f32 = 350.0;
const RADII_M: [f32; 6] = [150.0, 175.0, 200.0, 225.0, 250.0, 300.0];

fn sweep(id: &str, wind_dir: f64, wind_speed: f64, moisture: f64) {
    let scn = Scenario::load_by_id(data_dir(), id).unwrap();
    println!(
        "\n=== {id}: wind {wind_dir:.0}deg, {wind_speed:.0} km/h, {moisture:.0}% moisture ({} households) ===",
        scn.population.households.len()
    );
    println!("  radius m  downwind hh  corridor%   start ha   2h ha   peak threatened   alight");
    for radius in RADII_M {
        let plan = plan_with_standoff(&scn, wind_dir, radius, STANDOFF_M);
        if plan.households_downwind == 0 {
            println!("  {radius:8.0}  (no candidate cell found)");
            continue;
        }
        let weather = Weather { wind_dir_deg: wind_dir, wind_speed_kmh: wind_speed, moisture_pct: moisture };
        let mut sim = FireSim::new(&scn, weather, 42).unwrap();
        if sim.ignite_patch(plan.centre, plan.radius_m, &scn).is_err() {
            println!("  {radius:8.0}  (ignition failed to establish)");
            continue;
        }
        let start_cells = sim.state().iter().filter(|s| **s != CellFire::Unburnt).count();
        let mut peak_threatened = 0usize;
        for _ in 0..24 {
            sim.advance(300).unwrap();
            peak_threatened = peak_threatened.max(sim.exposure().threatened(0.05).count());
        }
        let ha = sim.state().iter().filter(|s| **s != CellFire::Unburnt).count() as f32 * 0.04;
        let alight = sim.exposure().fields().iter().filter(|f| f.alight).count();
        println!(
            "  {radius:8.0}  {:12}  {:9.0}   {:8.1}   {ha:5.1}   {peak_threatened:15}   {alight:6}",
            plan.households_downwind,
            plan.corridor_fuel * 100.0,
            start_cells as f32 * 0.04,
        );
    }
}

/// Slow (4 scenarios x 6 radii x 2 h simulations). Run explicitly when
/// retuning a scenario's opening conditions:
/// `cargo test -p fire --release -- --ignored sweep_real_scenario_openings --nocapture`
#[test]
#[ignore]
fn sweep_real_scenario_openings() {
    sweep("mati", 293.0, 45.0, 5.0);
    sweep("pedrogao", 315.0, 45.0, 5.0);
    sweep("rhodes", 315.0, 30.0, 6.0);
    // Spotorno's tramontana, for a same-units baseline next to the others.
    sweep("spotorno", 0.0, 35.0, 6.0);
}
