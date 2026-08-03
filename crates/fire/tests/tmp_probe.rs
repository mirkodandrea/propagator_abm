//! TEMPORARY: what shrub spotting did to the line, and where a line still holds.

use fire::{cells_along, CellFire, FireSim, Intervention, Weather};
use scenario::{Cell, Pos, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .unwrap()
}

const START_RADIUS_M: f32 = 250.0;
const RUN_S: i64 = 120 * 60;
const HALF_LEN_M: f32 = 700.0;
const STEP_S: i64 = 60;

fn band_ahead(scn: &Scenario, centre: Cell, ahead_m: f32, half_len_m: f32) -> Vec<Cell> {
    let c = scn.world.centre_of(centre);
    let a = Pos { x: c.x - half_len_m, y: c.y - ahead_m };
    let b = Pos { x: c.x + half_len_m, y: c.y - ahead_m };
    cells_along(&scn.world, a, b, 30.0).into_iter().filter(|c| scn.is_burnable(*c)).collect()
}

fn burnt(sim: &FireSim) -> usize {
    sim.state().iter().filter(|s| **s != CellFire::Unburnt).count()
}

fn run(scn: &Scenario, ign: Cell, w: Weather, seed: u64, line: Option<Vec<Cell>>) -> usize {
    run_act(scn, ign, w, seed, |sim| {
        if let Some(l) = line {
            sim.queue(Intervention::fireline(l));
        }
    })
}

fn run_act(
    scn: &Scenario,
    ign: Cell,
    w: Weather,
    seed: u64,
    act: impl FnOnce(&mut FireSim),
) -> usize {
    let mut sim = FireSim::new(scn, w, seed).unwrap();
    sim.ignite_patch(ign, START_RADIUS_M, scn).unwrap();
    act(&mut sim);
    while sim.time_s() < RUN_S {
        sim.advance(STEP_S).unwrap();
    }
    burnt(&sim)
}

#[test]
#[ignore]
fn water_sweep() {
    for shrubs in [false, true] {
        let mut scn = Scenario::load(&data_dir()).unwrap();
        if !shrubs {
            for d in scn.fuel_defs.iter_mut() {
                if (7..=9).contains(&d.id) {
                    d.spotting = false;
                    d.prob_ign_by_embers = 0.0;
                }
            }
        }
        let w = Weather::default();
        let plan = fire::plan_ignition(&scn, w.wind_dir_deg, START_RADIUS_M);
        let seeds = [42u64, 1, 2, 3, 4];
        let swath = band_ahead(&scn, plan.centre, 300.0, HALF_LEN_M);
        let cell_m2 = (scn.world.cellsize * scn.world.cellsize) as f64;
        let loads = |n: f64| n * 6137.0 / (swath.len() as f64 * cell_m2);

        let mean = |v: &[usize]| v.iter().sum::<usize>() as f32 / v.len() as f32;
        let free: Vec<usize> = seeds.iter().map(|s| run(&scn, plan.centre, w, *s, None)).collect();
        println!("shrub_spotting={shrubs}  free mean {:.0}  {free:?}", mean(&free));
        for n in [8.0f64, 20.0] {
            let l = loads(n);
            let got: Vec<usize> = seeds
                .iter()
                .map(|s| {
                    run_act(&scn, plan.centre, w, *s, |sim| {
                        sim.queue(Intervention::water(swath.clone(), l));
                    })
                })
                .collect();
            println!(
                "   {n:4.0} loads ({l:.2} L/m², +{:.0} pts): mean {:6.0} = {:3.0}% of free  {got:?}",
                l as f32 * fire::intervention::MOISTURE_POINTS_PER_LITRE,
                mean(&got),
                mean(&got) / mean(&free) * 100.0
            );
        }
    }
}

#[test]
#[ignore]
fn line_offset_sweep() {
    for shrubs in [false, true] {
        let mut scn = Scenario::load(&data_dir()).unwrap();
        if !shrubs {
            for d in scn.fuel_defs.iter_mut() {
                if (7..=9).contains(&d.id) {
                    d.spotting = false;
                    d.prob_ign_by_embers = 0.0;
                }
            }
        }
        let w = Weather::default();
        let plan = fire::plan_ignition(&scn, w.wind_dir_deg, START_RADIUS_M);
        let seeds = [42u64, 1, 2, 3, 4];

        let free: Vec<usize> = seeds.iter().map(|s| run(&scn, plan.centre, w, *s, None)).collect();
        let free_mean = free.iter().sum::<usize>() as f32 / seeds.len() as f32;
        println!(
            "shrub_spotting={shrubs}  free mean {free_mean:.0} cells  ({:?})",
            free
        );

        for ahead in [300.0f32, 500.0, 800.0] {
            for half in [700.0f32, 1400.0] {
                let line = band_ahead(&scn, plan.centre, ahead, half);
                let held: Vec<usize> = seeds
                    .iter()
                    .map(|s| run(&scn, plan.centre, w, *s, Some(line.clone())))
                    .collect();
                let mean = held.iter().sum::<usize>() as f32 / seeds.len() as f32;
                println!(
                    "   line {ahead:4.0} m ahead, {:4.0} m half-length ({:3} cells): \
                     mean {mean:6.0}  = {:3.0}% of free   {held:?}",
                    half,
                    line.len(),
                    mean / free_mean * 100.0
                );
            }
        }
    }
}
