//! A scenario start must establish reliably. A single lit cell does not.

use fire::{CellFire, FireSim, Weather};
use scenario::{Cell, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

#[test]
fn patch_ignition_establishes_for_every_seed() {
    let scn = Scenario::load(data_dir()).unwrap();
    let centre = Cell { row: 391, col: 232 };

    let mut single_failures = 0;
    let mut patch_failures = 0;
    for seed in 1..=20u64 {
        let mut a = FireSim::new(&scn, Weather::default(), seed).unwrap();
        a.ignite(&[centre]).unwrap();
        a.advance(7200).unwrap();
        let na = a.state().iter().filter(|s| **s != CellFire::Unburnt).count();
        if na < 50 { single_failures += 1; }

        let mut b = FireSim::new(&scn, Weather::default(), seed).unwrap();
        b.ignite_patch(centre, 45.0, &scn).unwrap();
        b.advance(7200).unwrap();
        let nb = b.state().iter().filter(|s| **s != CellFire::Unburnt).count();
        if nb < 50 { patch_failures += 1; }
        println!("seed {seed:2}: single {na:5}   patch {nb:5}");
    }
    println!("\nfizzled (<50 cells in 2h): single-cell {single_failures}/20, patch {patch_failures}/20");
    assert_eq!(patch_failures, 0, "patch ignition must always establish");
}
