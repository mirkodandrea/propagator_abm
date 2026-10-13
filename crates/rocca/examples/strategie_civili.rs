//! Does evacuating everyone ever cost more than evacuating only the places in
//! the fire's path? Per case and seed: the districts the same fire with no
//! orders reaches («minacciati»), then five civil plans with those districts
//! as priorities. «colte fuori» are families killed or trapped after leaving
//! home, which the score does not count (`rocca::score` weighs caught at home).
//!
//! MESSAGGERI=squadre,famiglie_al_minuto_alla_porta,broadcast(0|1),per_priorita(0|1)[,rovesciate(0|1)]
//! carries the orders door to door (`rocca::messengers`) with these teams
//! instead of the ones in `game.json`; MESSAGGERI=0 sends every order over
//! each household's channel.
//!
//! PRIORITA=sbagliate ranks the districts the fire misses first and the ones it
//! reaches last, whatever the plan.
//!
//! Every game runs on its own thread, as fast as the machine goes; the rows
//! come out in the usual order. «squadre» are, over all teams: households told
//! in person, km driven, re-plans because the fire cut the road ahead,
//! diversions to another district on the road, districts left because the fire
//! came too close, and teams caught by the fire.
//!
//!   cargo run --release -p rocca --example strategie_civili [caso...] [--semi 1,2]
use abm::Mode;
use rocca::{Civil, Game, Plan};
use scenario::population::Status;

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let messengers = std::env::var("MESSAGGERI").ok().map(|v| {
        let f: Vec<f32> = v.split(',').map(|x| x.trim().parse().unwrap()).collect();
        (f[0] > 0.0).then(|| rocca::Messengers {
            teams: f[0] as usize,
            per_min: f[1],
            broadcast: f[2] > 0.0,
            by_priority: f[3] > 0.0,
            reversed: f.get(4).is_some_and(|&x| x > 0.0),
        })
    });
    let wrong = std::env::var("PRIORITA").is_ok_and(|v| v == "sbagliate");
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
    println!("| caso | seme | minacciati | piano | punti | colte in casa | colte fuori | calore max su un'auto (s) | auto·min in allarme | in coda (max) | squadre: avvisate / km / tagli / dirottate / lasciati / perse |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    let pairs: Vec<(String, u64)> = cases.iter().flat_map(|c| seeds.iter().map(move |&s| (c.clone(), s))).collect();
    let withouts = par_map(&pairs, |(case, seed)| Game::without_orders(data, case, *seed));
    let plans = ["nessun ordine", "evacua tutti", "evacua i minacciati", "preallerta tutti", "preallerta tutti, evacua i minacciati"];
    let mut jobs = vec![];
    for (k, without) in withouts.iter().enumerate() {
        let without = without.as_ref().map_err(|e| anyhow::anyhow!("{e:#}"))?;
        let mut hit: Vec<usize> = (0..without.districts.len())
            .filter(|&d| without.districts[d].caught > 0 || without.districts[d].homes_hit > 0)
            .collect();
        hit.sort_by_key(|&d| std::cmp::Reverse(without.districts[d].caught));
        if !hit.is_empty() {
            jobs.extend(plans.iter().map(|&plan| (k, hit.clone(), plan)));
        }
    }
    let rows = par_map(&jobs, |(k, hit, plan)| -> anyhow::Result<String> {
        let ((case, seed), without) = (&pairs[*k], withouts[*k].as_ref().unwrap());
        let mut g = Game::new(data, case, *seed)?;
        // the rules before 2026-10-13: any evacuation at any time (`examples/sindaca.rs` plays the mayor's)
        g.mayor = None;
        if let Some(m) = messengers {
            g.messengers = m;
        }
        let n = g.districts.len();
        let names: Vec<String> = hit.iter().map(|&d| g.districts[d].name.clone()).collect();
        let order: Vec<usize> = if wrong {
            (0..n).filter(|d| !hit.contains(d)).chain(hit.iter().rev().copied()).collect()
        } else {
            hit.clone()
        };
        let mut p = Plan::new(n).with_priorities(&order);
        let all: Vec<usize> = (0..n).collect();
        let (pre, evac): (&[usize], &[usize]) = match *plan {
            "nessun ordine" => (&[], &[]),
            "evacua tutti" => (&[], &all),
            "evacua i minacciati" => (&[], hit),
            "preallerta tutti" => (&all, &[]),
            _ => (&all, hit),
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
        let sc = rocca::score::score(&o, without);
        let out = (0..g.agents.households.len())
            .filter(|&k| matches!(g.agents.households[k].status, Status::Trapped | Status::Casualty) && g.caught_at(k).is_none())
            .count();
        let teams = if g.teams.is_empty() {
            "—".to_string()
        } else {
            let t = &g.teams;
            format!(
                "{} / {:.1} / {} / {} / {} / {}",
                t.iter().map(|t| t.told).sum::<usize>(),
                t.iter().map(|t| t.drive.driven_m).sum::<f32>() / 1000.0,
                t.iter().map(|t| t.drive.cut_ahead).sum::<u32>(),
                t.iter().map(|t| t.diverted).sum::<u32>(),
                t.iter().map(|t| t.left).sum::<u32>(),
                t.iter().filter(|t| t.task == rocca::messengers::Task::Lost).count()
            )
        };
        Ok(format!(
            "| {case} | {seed} | {} | {plan} | {} | {} | {out} | {heat:.0} | {} | {queued} | {teams} |",
            names.join(", "),
            sc.total,
            o.caught(),
            alarm_s / 60
        ))
    });
    for row in rows {
        println!("{}", row?);
    }
    Ok(())
}

/// `f` over `items` on every core, results in the items' order.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(items.len().max(1));
    let mut out: Vec<(usize, R)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = vec![];
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        mine.push((i, f(item)));
                    }
                    mine
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap()).collect()
    });
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, r)| r).collect()
}
