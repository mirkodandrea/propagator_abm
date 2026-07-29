//! What the shipped scenario actually does, start to finish.

use fire::{CellFire, FireSim, Weather};
use scenario::Scenario;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

#[test]
fn report() {
    let scn = Scenario::load(data_dir()).unwrap();
    let weather = Weather::default();
    let plan = fire::plan_ignition(&scn, weather.wind_dir_deg, 250.0);
    let mut sim = FireSim::new(&scn, weather, 42).unwrap();
    sim.ignite_patch(plan.centre, plan.radius_m, &scn).unwrap();

    println!("Spotorno initial attack | tramontana {} km/h from {}deg, {}% moisture",
        weather.wind_speed_kmh, weather.wind_dir_deg, weather.moisture_pct);
    println!("ignition ({}, {}) r={:.0} m, {} households in the downwind corridor\n",
        plan.centre.row, plan.centre.col, plan.radius_m, plan.households_downwind);
    println!("  time    burnt ha   front   peak FLI   flame m   ember reach   threatened   lost");
    for m in 1..=24 {
        sim.advance(300).unwrap();
        let burnt = sim.state().iter().filter(|s| **s != CellFire::Unburnt).count();
        let front = sim.active_cells().len();
        let fli = sim.active_cells().iter().map(|c| sim.cell_intensity(*c)).fold(0.0f32, f32::max);
        let thr = sim.exposure().threatened(0.05).count();
        let lost = sim.exposure().fields().iter().filter(|f| f.alight).count();
        if m % 3 == 0 {
            println!("  {:3} min {:9.1}   {front:5}   {fli:8.0}   {:7.1}   {:11.0}   {thr:10}   {lost:4}",
                m * 5, burnt as f32 * 0.04,
                fire::exposure::flame_length_m(fli),
                fire::exposure::ember_range_m(fli, weather.wind_speed_kmh as f32));
        }
    }
    let dmg = sim.exposure().fields().iter().filter(|f| f.damage > 0.01).count();
    println!("\nhouseholds with any accumulated damage: {dmg} of {}", scn.population.households.len());
}
