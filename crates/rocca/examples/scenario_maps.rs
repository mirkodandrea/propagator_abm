//! Export actual no-order fire arrivals for static scenario maps; no window.
use anyhow::{Context, Result};
use rocca::{Case, Game, Territory};
use serde_json::json;
use std::{io::Write, path::PathBuf};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let input = PathBuf::from(
        args.next()
            .context("scenario_maps <cases.json> <output> [seed]")?,
    );
    let output = PathBuf::from(args.next().context("missing output directory")?);
    let seed: u64 = args.next().unwrap_or_else(|| "2".into()).parse()?;
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(input)?)?;
    let cases: Vec<Case> = serde_json::from_value(manifest["cases"].clone())?;
    let data = PathBuf::from("data");
    let territory = Territory::load(&data)?;
    std::fs::create_dir_all(&output)?;
    for case in cases {
        let mut game = Game::with_case(&data, territory.clone(), case.clone(), seed)?;
        while !game.is_over() {
            game.step()?;
        }
        let outcome = game.outcome();
        let mut raster = std::fs::File::create(output.join(format!("{}.i32", case.name)))?;
        for &time in game.fire.arrival_times() {
            raster.write_all(&time.to_le_bytes())?;
        }
        let districts: Vec<_> = game.districts.iter().zip(&outcome.districts).map(|(d,o)| json!({
            "name":d.name,"centre":[d.centre.x,d.centre.y],"caught":o.caught,"homes":o.homes_hit,
            "people":o.people,"households":o.homes
        })).collect();
        let exposure = game.fire.exposure().fields();
        let houses: Vec<_> = game.scn.population.households.iter().enumerate().map(|(i,h)| json!({
            "pos":h.pos,"alight":exposure[i].alight,"caught":game.evac.families[i].state==rocca::State::Caught
        })).collect();
        std::fs::write(
            output.join(format!("{}.json", case.name)),
            serde_json::to_vec_pretty(&json!({
                "case":case.name,"seed":seed,"rows":game.scn.world.fire_rows,"cols":game.scn.world.fire_cols,
                "end_min":outcome.at_s as f64/60.0,"ha":outcome.hectares,"caught":outcome.caught(),
                "homes":outcome.homes_hit(),"districts":districts,"houses":houses
            }))?,
        )?;
        eprintln!(
            "{}: seed {}, {:.1} ha, {} people, {} houses",
            case.name,
            seed,
            outcome.hectares,
            outcome.caught(),
            outcome.homes_hit()
        );
    }
    Ok(())
}
