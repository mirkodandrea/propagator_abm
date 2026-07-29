//! How big must the opening fire be, and how close to the WUI, for the
//! scenario to actually put people at risk within an initial-attack window?

use fire::{CellFire, FireSim, Weather};
use scenario::Scenario;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

/// Slow (12 x 2 h simulations). Run explicitly when retuning the scenario:
/// `cargo test -p fire --release -- --ignored --nocapture`
#[test]
#[ignore]
fn sweep_start_size_and_standoff() {
    let scn = Scenario::load(data_dir()).unwrap();
    let weather = Weather::default();
    println!("  radius m  standoff m   start ha   2h ha   peak threatened   alight");
    for radius in [150.0f32, 250.0, 350.0, 500.0] {
        for standoff in [150.0f32, 350.0, 700.0] {
            let plan = fire::plan_with_standoff(&scn, weather.wind_dir_deg, radius, standoff);
            let mut sim = FireSim::new(&scn, weather, 42).unwrap();
            if sim.ignite_patch(plan.centre, plan.radius_m, &scn).is_err() { continue; }
            let start = sim.state().iter().filter(|s| **s != CellFire::Unburnt).count();
            sim.advance(60).unwrap();
            let start = start.max(sim.state().iter().filter(|s| **s != CellFire::Unburnt).count());
            let mut peak = 0usize;
            for _ in 0..24 {
                sim.advance(300).unwrap();
                peak = peak.max(sim.exposure().threatened(0.05).count());
            }
            let ha = sim.state().iter().filter(|s| **s != CellFire::Unburnt).count() as f32 * 0.04;
            let alight = sim.exposure().fields().iter().filter(|f| f.alight).count();
            println!(
                "  {radius:8.0}  {standoff:10.0}   {:8.1}   {ha:5.1}   {peak:15}   {alight:6}",
                start as f32 * 0.04
            );
        }
    }
}
