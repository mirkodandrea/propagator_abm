//! The spread-probability overlay is a port of formulas that live in
//! `propagator-core`'s private `models` module (see `fire::hazard`). These
//! tests pin the properties that would break silently if either copy drifted:
//! the field must lean downwind, lean upslope, die in wet fuel, and light up
//! only the unburnt fringe of the front.

use fire::hazard::probability_to_neighbour;
use fire::{CellFire, FireSim, Weather};
use scenario::{Cell, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data dir")
}

/// Propagation bearings in the kernel's convention: angle 0 is toward south.
const TOWARD_SOUTH: f64 = 0.0;
const TOWARD_NORTH: f64 = std::f64::consts::PI;

#[test]
fn wind_favours_the_downwind_neighbour() {
    // wind_dir 0 = blowing *from* the north, so the fire runs south.
    let north_wind = 0.0;
    let downwind = probability_to_neighbour(TOWARD_SOUTH, 20.0, north_wind, 30.0, 0.06, 0.0, 0.5);
    let upwind = probability_to_neighbour(TOWARD_NORTH, 20.0, north_wind, 30.0, 0.06, 0.0, 0.5);
    assert!(
        downwind > upwind,
        "downwind {downwind:.3} should beat upwind {upwind:.3}"
    );
}

#[test]
fn slope_favours_uphill() {
    let calm = 0.0;
    let uphill = probability_to_neighbour(TOWARD_SOUTH, 20.0, 0.0, calm, 0.06, 12.0, 0.5);
    let flat = probability_to_neighbour(TOWARD_SOUTH, 20.0, 0.0, calm, 0.06, 0.0, 0.5);
    let downhill = probability_to_neighbour(TOWARD_SOUTH, 20.0, 0.0, calm, 0.06, -12.0, 0.5);
    assert!(uphill > flat && flat > downhill, "{uphill} {flat} {downhill}");
}

#[test]
fn moisture_extinguishes_spread() {
    let dry = probability_to_neighbour(TOWARD_SOUTH, 20.0, 0.0, 20.0, 0.03, 0.0, 0.5);
    let damp = probability_to_neighbour(TOWARD_SOUTH, 20.0, 0.0, 20.0, 0.20, 0.0, 0.5);
    let soaked = probability_to_neighbour(TOWARD_SOUTH, 20.0, 0.0, 20.0, 0.30, 0.0, 0.5);
    assert!(dry > damp && damp > soaked, "{dry} {damp} {soaked}");
    assert!(soaked < 0.01, "at extinction moisture spread should stop: {soaked}");
    assert!((0.0..=1.0).contains(&dry));
}

#[test]
fn hazard_field_covers_the_unburnt_fringe() {
    let scn = Scenario::load(data_dir()).expect("load scenario");
    let mut sim = FireSim::new(&scn, Weather::default(), 42).expect("build core");
    let plan = fire::plan_ignition(&scn, Weather::default().wind_dir_deg, 200.0);
    sim.ignite_patch(plan.centre, plan.radius_m, &scn).expect("ignite");
    for _ in 0..30 {
        sim.advance(20).expect("advance");
    }

    let cols = scn.world.fire_cols;
    let hazard = sim.hazard();
    let mut fringe = 0;
    for (i, &p) in hazard.as_slice().iter().enumerate() {
        if p <= 0.0 {
            continue;
        }
        fringe += 1;
        assert!(p <= 1.0, "probability out of range: {p}");
        let cell = Cell { row: i / cols, col: i % cols };
        // Only ground the fire has yet to take, and only burnable ground.
        assert_eq!(sim.cell_state(cell), CellFire::Unburnt);
        assert!(scn.is_burnable(cell));
    }
    assert!(fringe > 0, "a going fire must have a fringe with non-zero spread probability");
    assert!(hazard.peak() > 0.0);
}
