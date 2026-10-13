//! Does the evacuation wave slow the engines when the player moves them
//! *during* it? Same fire and seed; at T+`al` the first priority becomes each
//! district in turn, under three civil plans. Minutes until the first unit is
//! on the new post.
//!
//!   cargo run --release -p rocca --example intasamento_mezzi [caso] [--al 34]
use rocca::game::Arrival;
use rocca::{Civil, Game, Plan};

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let case = args.first().cloned().unwrap_or_else(|| "Rocco3".into());
    let at_min: i64 = args.iter().position(|a| a == "--al").map_or(34, |i| args[i + 1].parse().unwrap());
    let probe = Game::new(data, &case, 1)?;
    let names: Vec<String> = probe.districts.iter().map(|d| d.name.clone()).collect();
    let n = names.len();
    println!("Caso {case}, seme 1: priorità cambiata a T+{at_min}. Minuti fino al primo mezzo in postazione (auto ferme in quel momento).\n");
    println!("| nuova priorità | nessun ordine | evacua tutti a T+0 | preallerta, evacua tutti a T+30 |");
    println!("|---|---|---|---|");
    for target in 0..n {
        let mut row = vec![names[target].clone()];
        for plan_kind in 0..3 {
            let mut g = Game::new(data, &case, 1)?;
            // the rules before 2026-10-13: any evacuation at any time (`examples/sindaca.rs` plays the mayor's)
            g.mayor = None;
            let all = |p: Plan, c: Civil| (0..n).fold(p, |p, d| p.with_civil(d, c));
            // Start on the district farthest from the target, so the move is real.
            let start = (0..n)
                .filter(|&d| d != target)
                .max_by(|&a, &b| rocca::district::dist(g.districts[a].centre, g.districts[target].centre).total_cmp(&rocca::district::dist(g.districts[b].centre, g.districts[target].centre)))
                .unwrap();
            let mut plan = Plan::new(n).with_priorities(&[start]);
            plan = match plan_kind {
                1 => all(plan, Civil::Evacua),
                2 => all(plan, Civil::Preallerta),
                _ => plan,
            };
            g.commit(plan.clone())?;
            if plan_kind == 2 {
                g.run_until(30 * 60)?;
                plan = all(g.active.clone(), Civil::Evacua);
                g.commit(plan.clone())?;
            }
            g.run_until(at_min * 60)?;
            let queued = g.agents.traffic.queued_vehicles();
            let mut p = g.active.clone();
            p.priorities = vec![target];
            g.commit(p)?;
            let t0 = g.time_s();
            let mut on: Option<i64> = None;
            while g.time_s() < t0 + 90 * 60 && g.time_s() < g.case.duration_s() {
                g.step()?;
                if g.arrivals(target).iter().any(|(_, a)| matches!(a, Arrival::OnPost)) {
                    on = Some(g.time_s() - t0);
                    break;
                }
            }
            row.push(format!("{} ({queued})", on.map_or(">90".into(), |s| format!("{:.0}", s as f64 / 60.0))));
        }
        println!("| {} |", row.join(" | "));
    }
    Ok(())
}
