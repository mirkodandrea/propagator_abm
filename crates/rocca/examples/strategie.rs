//! Balance table: simple strategies × cases × seeds 1–3 (docs/03-VERIFICHE.md).
//! target/release/examples/strategie [caso ...]   (all cases of ROCCA_SCENARIO if none)
use anyhow::Result;
use rocca::{district, score, Game, Outcome, Territory};
use std::path::Path;

const STRATEGIES: [&str; 6] = ["nessuno", "innesco", "evacua-tutto", "squadre-borghi", "piano", "piano-tardi"];

fn play(case: &str, seed: u64, policy: &str) -> Result<(Outcome, u32)> {
    let data = Path::new("data");
    let territory = Territory::load(data)?;
    let c = territory.case(case).expect("caso").clone();
    let mut game = Game::with_case(data, territory, c, seed)?;
    let mut evacuated = vec![false; game.districts.len()];
    let mut first = true;
    let mut orders = 0;
    while !game.is_over() {
        if policy != "nessuno" && game.can_pause() && (policy != "piano-tardi" || game.time_s() >= (game.case.start_min + 30) * 60) {
            game.pause().map_err(anyhow::Error::msg)?;
            let mut targets: Vec<_> = game
                .districts
                .iter()
                .enumerate()
                .filter_map(|(i, d)| district::exposure(d, &game.fire, &game.scn).map(|e| (i, district::reach_m(e.distance_m, e.downwind))))
                .collect();
            targets.sort_by(|a, b| a.1.total_cmp(&b.1));
            let to_ignition = matches!(policy, "innesco") && first;
            if to_ignition {
                if let Some(h) = game.hexes.nearest(game.case.ignition()) {
                    let at = h.at;
                    for k in 0..game.teams.len() {
                        orders += game.move_team(k, at).is_ok() as u32;
                    }
                }
            }
            if matches!(policy, "squadre-borghi" | "piano" | "piano-tardi") {
                for (k, &(d, _)) in targets.iter().cycle().take(game.teams.len()).enumerate() {
                    if let Some(h) = game.hexes.nearest(game.districts[d].centre) {
                        let at = h.at;
                        if game.teams[k].hex != Some(at) {
                            orders += game.move_team(k, at).is_ok() as u32;
                        }
                    }
                }
            }
            if matches!(policy, "evacua-tutto" | "piano" | "piano-tardi") && game.evac.active().is_none() {
                if let Some(&(d, _)) = targets.iter().find(|&&(d, _)| !evacuated[d] && game.plan_evacuation(d).is_ok()) {
                    if game.evacuate(d).is_ok() {
                        evacuated[d] = true;
                        orders += 1;
                    }
                }
            }
            first = false;
            game.resume();
        }
        game.step()?;
    }
    Ok((game.outcome(), orders))
}

fn main() -> Result<()> {
    let data = Path::new("data");
    let t = Territory::load(data)?;
    let mut cases: Vec<String> = std::env::args().skip(1).collect();
    if cases.is_empty() {
        cases = t.cases.iter().map(|c| c.name.clone()).collect();
    }
    let jobs: Vec<(String, u64, &str)> = cases
        .iter()
        .flat_map(|c| (1..=3u64).flat_map(move |s| STRATEGIES.iter().map(move |p| (c.clone(), s, *p))))
        .collect();
    let n = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let results = std::sync::Mutex::new(vec![]);
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|sc| {
        for _ in 0..n {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some((c, s, p)) = jobs.get(i) else { break };
                let r = play(c, *s, p);
                results.lock().unwrap().push((i, r));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|r| r.0);
    println!("| Caso | Seme | Strategia | Ordini | Persone | Case | ha | Perse | Stelle P/C/E |");
    println!("|---|---|---|---:|---:|---:|---:|---:|---|");
    let mut base: Option<Outcome> = None;
    for (i, r) in results {
        let (c, s, p) = &jobs[i];
        match r {
            Ok((o, ord)) => {
                if *p == "nessuno" {
                    base = Some(o.clone());
                }
                let st = score::stars(&o, base.as_ref().unwrap()).all().map(|x| x.map_or("–".into(), |v| v.to_string())).join("/");
                println!("| {c} | {s} | {p} | {ord} | {} | {} | {:.0} | {} | {st} |", o.caught(), o.homes_hit(), o.hectares, o.units_lost);
            }
            Err(e) => println!("| {c} | {s} | {p} | errore: {e} |"),
        }
    }
    Ok(())
}
