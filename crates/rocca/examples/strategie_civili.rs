//! Does evacuating everyone ever cost more than evacuating only the places in
//! the fire's path? Per case and seed: the districts the same fire with no
//! orders reaches («minacciati»), then five civil plans with those districts
//! as priorities. «colte fuori» are families killed or trapped after leaving
//! home, which the score does not count (`rocca::score` weighs caught at home).
//!
//!   cargo run --release -p rocca --example strategie_civili [caso...] [--semi 1,2]
use abm::Mode;
use rocca::{Civil, Game, Plan};
use scenario::population::Status;

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut seeds = vec![1u64];
    let mut cases: Vec<String> = vec![];
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
        let g = Game::new(data, "Rocco3", 1)?;
        cases = g.territory.cases.iter().map(|c| c.name.clone()).collect();
    }
    println!("| caso | seme | minacciati | piano | punti | colte in casa | colte fuori | calore max su un'auto (s) | auto·min in allarme | in coda (max) |");
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for case in &cases {
        for &seed in &seeds {
            let without = Game::without_orders(data, case, seed)?;
            let mut hit: Vec<usize> = (0..without.districts.len())
                .filter(|&d| without.districts[d].caught > 0 || without.districts[d].homes_hit > 0)
                .collect();
            hit.sort_by_key(|&d| std::cmp::Reverse(without.districts[d].caught));
            if hit.is_empty() {
                continue;
            }
            for plan in ["nessun ordine", "evacua tutti", "evacua i minacciati", "preallerta tutti", "preallerta tutti, evacua i minacciati"] {
                let mut g = Game::new(data, case, seed)?;
                let n = g.districts.len();
                let names: Vec<String> = hit.iter().map(|&d| g.districts[d].name.clone()).collect();
                let mut p = Plan::new(n).with_priorities(&hit);
                let all: Vec<usize> = (0..n).collect();
                let (pre, evac): (&[usize], &[usize]) = match plan {
                    "nessun ordine" => (&[], &[]),
                    "evacua tutti" => (&[], &all),
                    "evacua i minacciati" => (&[], &hit),
                    "preallerta tutti" => (&all, &[]),
                    _ => (&all, &hit),
                };
                for &d in pre {
                    p = p.with_civil(d, Civil::Preallerta);
                }
                for &d in evac {
                    p = p.with_civil(d, Civil::Evacua);
                }
                g.commit(p)?;
                let (mut heat, mut alarm_s, mut queued) = (0.0f32, 0i64, 0usize);
                while g.time_s() < g.case.duration_s() {
                    g.step()?;
                    for t in &g.agents.travellers {
                        if t.mode != Mode::Car || !matches!(t.state, abm::TravelState::OnNetwork | abm::TravelState::Approaching) {
                            continue;
                        }
                        heat = heat.max(t.heat_s);
                        if g.fire.threat().at(t.pos) >= fire::threat::ALARMING {
                            alarm_s += rocca::STEP_S;
                        }
                    }
                    queued = queued.max(g.agents.traffic.queued_vehicles());
                }
                let o = g.outcome();
                let sc = rocca::score::score(&o, &without);
                let out = (0..g.agents.households.len())
                    .filter(|&k| matches!(g.agents.households[k].status, Status::Trapped | Status::Casualty) && g.caught_at(k).is_none())
                    .count();
                println!(
                    "| {case} | {seed} | {} | {plan} | {} | {} | {out} | {heat:.0} | {} | {queued} |",
                    names.join(", "),
                    sc.total,
                    o.caught(),
                    alarm_s / 60
                );
            }
        }
    }
    Ok(())
}
