//! Compare the live and proposed scales on the current playable fire.
//! cargo run --offline --release -p rocca --example score_review
use anyhow::Result;
use rocca::{district, score, Game, Outcome, Territory};
use std::path::Path;

fn play(seed: u64, policy: &str) -> Result<Outcome> {
    let data = Path::new("data");
    let territory = Territory::load(data)?;
    let mut game = Game::new(data, &territory.game, seed)?;
    let mut evacuated = vec![false; game.districts.len()];
    let mut first = true;
    while !game.is_over() {
        if policy != "nessuno" && game.can_pause() {
            game.pause().map_err(anyhow::Error::msg)?;
            let mut targets: Vec<_> = game.districts.iter().enumerate().filter_map(|(i, d)| {
                district::exposure(d, &game.fire, &game.scn)
                    .map(|e| (i, district::reach_m(e.distance_m, e.downwind)))
            }).collect();
            targets.sort_by(|a, b| a.1.total_cmp(&b.1));
            if policy == "innesco" && first {
                if let Some(hex) = game.hexes.nearest(game.case.ignition()) {
                    let at = hex.at;
                    for k in 0..game.teams.len() { let _ = game.move_team(k, at); }
                }
            }
            if policy == "combinata" {
                for (k, &(d, _)) in targets.iter().cycle().take(game.teams.len()).enumerate() {
                    if let Some(hex) = game.hexes.nearest(game.districts[d].centre) {
                        let at = hex.at;
                        if game.teams[k].hex != Some(at) { let _ = game.move_team(k, at); }
                    }
                }
            }
            if (policy == "evacuazione" || policy == "combinata") && game.evac.active().is_none() {
                if let Some(&(d, _)) = targets.iter().find(|&&(d, _)| !evacuated[d] && game.plan_evacuation(d).is_ok()) {
                    if game.evacuate(d).is_ok() { evacuated[d] = true; }
                }
            }
            first = false;
            game.resume();
        }
        game.step()?;
    }
    Ok(game.outcome())
}
fn marks(st: score::Stars) -> String {
    st.all().map(|s| s.map_or("–".into(), |s| s.to_string())).join(" / ")
}
fn main() -> Result<()> {
    println!("| Seme | Strategia | Persone colpite | Case colpite | ha | Stelle attuali P/C/E | Proposta P/C/E |");
    println!("|---|---|---:|---:|---:|---|---|");
    for seed in 1..=3 {
        let base = play(seed, "nessuno")?;
        for policy in ["nessuno", "innesco", "evacuazione", "combinata"] {
            let mine = if policy == "nessuno" { base.clone() } else { play(seed, policy)? };
            println!("| {seed} | {policy} | {} | {} | {:.0} | {} | {} |", mine.caught(), mine.homes_hit(), mine.hectares, marks(score::stars(&mine, &base)), marks(score::proposed_stars(&mine, &base)));
        }
    }
    Ok(())
}
