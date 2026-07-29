//! The scenario has to threaten people, and the interaction radius has to
//! track fire intensity rather than sitting at a constant.

use fire::exposure::{ember_range_m, flame_length_m, radiant_range_m};
use fire::{CellFire, FireSim, Weather};
use scenario::Scenario;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

#[test]
fn interaction_area_scales_with_intensity() {
    println!("  FLI kW/m   flame m   radiant m   ember m (35 km/h)");
    for fli in [100.0, 500.0, 2_000.0, 5_000.0, 15_000.0, 40_000.0f32] {
        println!(
            "  {fli:>9.0}   {:>7.2}   {:>9.1}   {:>7.1}",
            flame_length_m(fli),
            radiant_range_m(fli),
            ember_range_m(fli, 35.0)
        );
    }
    // A crown-fire cell must reach substantially further than a moderate one.
    // (Compared against 500 kW/m, not 100: below ~150 kW/m the range sits on
    // its 10 m floor, so ratios against it measure the clamp, not the physics.)
    assert!(radiant_range_m(15_000.0) > 2.5 * radiant_range_m(500.0));
    // Cohen's structure-ignition work puts radiant ignition inside ~40 m even
    // for crown fire; the model must not reach absurdly further.
    assert!(radiant_range_m(40_000.0) < 60.0);
    assert!(ember_range_m(15_000.0, 35.0) > 5.0 * ember_range_m(100.0, 35.0));
    // Wind matters for embers, not for radiation.
    assert!(ember_range_m(5_000.0, 60.0) > ember_range_m(5_000.0, 10.0));
}

#[test]
fn wui_scenario_threatens_households() {
    let scn = Scenario::load(data_dir()).unwrap();
    let weather = Weather::default();
    let plan = fire::plan_ignition(&scn, weather.wind_dir_deg, 250.0);
    println!(
        "ignition {:?} fuel {}  |  {} households downwind, corridor {:.0}% burnable",
        plan.centre,
        scn.fuel_at(plan.centre),
        plan.households_downwind,
        plan.corridor_fuel * 100.0
    );
    assert!(plan.households_downwind > 30, "scenario puts too few people at risk");

    let mut sim = FireSim::new(&scn, weather, 42).unwrap();
    sim.ignite_patch(plan.centre, plan.radius_m, &scn).unwrap();

    println!("   t     burnt ha   peak FLI   threatened   alight");
    let mut peak_threatened = 0usize;
    for m in 1..=120 {
        sim.advance(60).unwrap();
        peak_threatened = peak_threatened.max(sim.exposure().threatened(0.05).count());
        if m % 20 == 0 {
            let burnt = sim.state().iter().filter(|s| **s != CellFire::Unburnt).count();
            let peak = sim.intensity().iter().cloned().fold(0.0f32, f32::max);
            let threatened = sim.exposure().threatened(0.05).count();
            let alight = sim.exposure().fields().iter().filter(|f| f.alight).count();
            println!(
                "  {m:3} min  {:8.1}   {peak:8.0}   {threatened:10}   {alight:6}",
                burnt as f32 * 0.04
            );
        }
    }
    println!("peak households threatened over the run: {peak_threatened}");
    assert!(
        peak_threatened > 60,
        "scenario must put real numbers of people at risk, peaked at {peak_threatened}"
    );
}

/// Structure damage must integrate over simulated time, not per update call --
/// otherwise the caller's step size silently changes how many houses burn.
#[test]
fn damage_is_independent_of_step_size() {
    let scn = Scenario::load(data_dir()).unwrap();
    let weather = Weather::default();
    let plan = fire::plan_ignition(&scn, weather.wind_dir_deg, 250.0);

    let total_damage = |step: i64| -> f32 {
        let mut sim = FireSim::new(&scn, weather, 42).unwrap();
        sim.ignite_patch(plan.centre, plan.radius_m, &scn).unwrap();
        let steps = 3600 / step;
        for _ in 0..steps {
            sim.advance(step).unwrap();
        }
        sim.exposure().fields().iter().map(|f| f.damage).sum()
    };

    let coarse = total_damage(300);
    let fine = total_damage(20);
    println!("total damage: 300 s steps {coarse:.2}, 20 s steps {fine:.2}");
    let rel = (coarse - fine).abs() / coarse.max(fine).max(1e-6);
    assert!(rel < 0.25, "damage depends on step size: {coarse:.2} vs {fine:.2}");
}
