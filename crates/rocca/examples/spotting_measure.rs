//! No-order baseline, using the actual game loop (6 s steps).
//! Run: cargo run --release -p rocca --example spotting_measure > results/spotting.csv
//! `ember_targets` includes pending landings and is NOT ignition provenance.
//! `detached_fires` uses the resident detector: new blobs >120 m from prior fire.
use anyhow::Result;
use fire::FireSim;
use rocca::{Game, Territory};
use std::path::Path;

fn main() -> Result<()> {
    let data = Path::new("data");
    let territory = Territory::load(data)?;
    let original = territory.case(&territory.game).unwrap().clone();
    eprintln!("scenario={} case={} opening={}min end={}min wind={}km/h moisture={}% shift={:?}",
        territory.scenario, original.name, original.start_min, original.minutes,
        original.wind_kmh, territory.weather(&original).moisture_pct, original.shift);
    println!("seed,spotting,minute,hectares,burning_cells,ember_targets,detached_fires,homes_hit,litres_applied,line_cells");
    for seed in [2, 1, 3, 4, 42] {
        for spotting in [true, false] {
            let mut case = original.clone();
            case.start_min = 0;
            let mut g = Game::with_case(data, territory.clone(), case, seed)?;
            if !spotting {
                for fuel in &mut g.scn.fuel_defs {
                    fuel.spotting = false;
                }
                g.fire = FireSim::new(&g.scn, territory.weather(&g.case), seed)?;
                g.fire.ignite_patch(g.scn.world.cell_of(g.case.ignition()), g.case.radius_m, &g.scn)?;
                g.step()?;
            }
            let mut detector = abm::spot::SpotFires::new(&g.scn);
            detector.update(&g.fire, &g.scn, g.time_s() as f32);
            let mut detached = 0;
            for minute in [10, 20, 30, 45, 60, 90, 120, 180, 240, 300] {
                if minute > original.minutes { continue; }
                while g.time_s() < minute * 60 {
                    g.step()?;
                    detached += detector.update(&g.fire, &g.scn, g.time_s() as f32);
                }
                let o = g.outcome();
                println!("{seed},{spotting},{minute},{:.2},{},{},{detached},{},{:.0},{}",
                    o.hectares, g.fire.active_cells().len(), g.fire.ember_ignited_cells(),
                    o.homes_hit(), g.fire.litres_applied, g.fire.line_cells);
                anyhow::ensure!(g.fire.litres_applied == 0.0 && g.fire.line_cells == 0,
                    "unexpected suppression in no-order baseline");
            }
            eprintln!("finished seed={seed} spotting={spotting}");
        }
    }
    Ok(())
}
