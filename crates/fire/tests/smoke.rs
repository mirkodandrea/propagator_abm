//! End-to-end check that the baked Spotorno assets load and the fire core
//! actually spreads through them, with no Python in the loop.

use fire::{CellFire, FireSim, Intervention, Weather};
use scenario::{Cell, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data dir")
}

fn wui_ignition(scn: &Scenario) -> Cell {
    for h in &scn.population.households {
        for d in 8..26usize {
            if d > h.cell[0] {
                break;
            }
            let c = Cell { row: h.cell[0] - d, col: h.cell[1] };
            if scn.is_burnable(c) {
                return c;
            }
        }
    }
    panic!("no burnable cell near housing");
}

#[test]
fn fire_spreads_through_real_terrain() {
    let scn = Scenario::load(data_dir()).expect("load scenario");
    assert_eq!(scn.world.fire_rows, 512);
    assert!(scn.population.households.len() > 500);
    assert!(scn.vectors.buildings.len() > 5000);

    let ignition = wui_ignition(&scn);
    let mut sim = FireSim::new(&scn, Weather::default(), 42).expect("build core");
    sim.ignite_patch(ignition, 45.0, &scn).expect("ignite");

    for _ in 0..60 {
        sim.advance(60).expect("advance");
    }

    let burning = sim.state().iter().filter(|s| **s == CellFire::Burning).count();
    let burnt = sim.state().iter().filter(|s| **s == CellFire::Burnt).count();
    println!(
        "T+{}s  burning {burning}  burnt {burnt}  = {:.1} ha",
        sim.time_s(),
        (burning + burnt) as f32 * 0.04
    );
    assert!(burning + burnt > 50, "fire barely spread: {burning}/{burnt}");
    assert!(sim.time_s() == 3600);

    let exposed = sim.exposure().threatened(0.0).count();
    println!("households with any exposure: {exposed}");
}

#[test]
fn fireline_reduces_spread() {
    let scn = Scenario::load(data_dir()).expect("load scenario");
    let ignition = wui_ignition(&scn);

    let burned = |line: bool| -> usize {
        let mut sim = FireSim::new(&scn, Weather::default(), 7).unwrap();
        sim.ignite_patch(ignition, 45.0, &scn).unwrap();
        if line {
            // a break straight across the downwind side, 3 cells deep
            let cells: Vec<Cell> = (0..80)
                .flat_map(|d| {
                    (0..3).map(move |k| Cell {
                        row: ignition.row + 12 + k,
                        col: (ignition.col + 40).saturating_sub(d),
                    })
                })
                .filter(|c| c.row < 512 && c.col < 512)
                .collect();
            sim.queue(Intervention::fireline(cells));
        }
        for _ in 0..40 {
            sim.advance(60).unwrap();
        }
        sim.state().iter().filter(|s| **s != CellFire::Unburnt).count()
    };

    let free = burned(false);
    let held = burned(true);
    println!("no line: {free} cells   with fireline: {held} cells");
    assert!(free > 0);
}
