//! How fast households leave when ordered out (2026-10-13, for the organised
//! exit's capacity, `abm::Abm::set_exit_capacity`): per level and seed, the
//! busiest minute and the busiest 10 minutes of departures, with everyone
//! ordered out at once (old rules) and with «preallerta tutti, evacua appena
//! autorizzato» (the mayor's rule). USCITA=<famiglie/min> sets the capacity.
//!
//!   cargo run --release -p rocca --example partenze [caso...] [--semi 1,2,3]
use rocca::{Civil, Game, Plan};
use scenario::population::Status;

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let cap: Option<f32> = std::env::var("USCITA").ok().map(|v| v.parse().unwrap());
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut seeds = vec![1u64, 2, 3];
    let mut cases = vec![];
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--semi" {
            seeds = args[i + 1].split(',').map(|s| s.parse().unwrap()).collect();
            i += 2;
        } else {
            cases.push(args[i].clone());
            i += 1;
        }
    }
    if cases.is_empty() {
        cases = vec!["easy".into(), "medium".into(), "expert".into()];
    }
    println!("| caso | seme | piano | famiglie | partite | max in 1 min | max in 10 min | in attesa (max) |");
    println!("|---|---|---|---|---|---|---|---|");
    for case in &cases {
        for &seed in &seeds {
            for mayor in [false, true] {
                let mut g = Game::new(data, case, seed)?;
                if !mayor {
                    g.mayor = None;
                }
                if let Some(c) = cap {
                    g.agents.set_exit_capacity(Some(c));
                }
                let n = g.districts.len();
                let mut plan = Plan::new(n);
                for d in 0..n {
                    plan = plan.with_civil(d, if mayor { Civil::Preallerta } else { Civil::Evacua });
                }
                g.commit(plan)?;
                let mut per_min: Vec<usize> = vec![];
                let mut was = vec![false; g.agents.households.len()];
                let mut waiting = 0;
                while g.time_s() < g.case.duration_s() {
                    g.step()?;
                    if mayor {
                        for d in g.take_authorisations() {
                            let p = g.active.clone().with_civil(d, Civil::Evacua);
                            g.commit(p)?;
                        }
                    }
                    let m = (g.time_s() / 60) as usize;
                    per_min.resize(m + 1, 0);
                    for (k, h) in g.agents.households.iter().enumerate() {
                        let gone = h.traveller.is_some() || matches!(h.status, Status::Evacuating | Status::Evacuated);
                        if gone && !was[k] {
                            was[k] = true;
                            per_min[m] += 1;
                        }
                    }
                    waiting = waiting.max(g.agents.waiting_to_leave().len());
                }
                let max1 = per_min.iter().max().copied().unwrap_or(0);
                let max10 = per_min.windows(10).map(|w| w.iter().sum::<usize>()).max().unwrap_or(0);
                let plan = if mayor { "preallerta tutti, evacua appena autorizzato" } else { "evacua tutti subito" };
                println!("| {case} | {seed} | {plan} | {} | {} | {max1} | {max10} | {waiting} |", was.len(), was.iter().filter(|&&w| w).count());
            }
        }
    }
    Ok(())
}
