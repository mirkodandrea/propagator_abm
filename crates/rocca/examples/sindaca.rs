//! Evacuation authorised by the mayor (01-SPEC-GIOCO §3, 2026-10-13): does
//! «evacua appena autorizzato» still dominate, when does the authorisation
//! come, what stars do the plans earn and how long would a kiosk game last?
//!
//! Per case and seed: the districts the same fire with no orders reaches at
//! all («minacciati», hindsight, as in `strategie_civili`) are the priorities
//! of every plan. Two plans under the old rules (any evacuation at any time),
//! then, for each lead time of the mayor (SINDACA_MIN, minutes, default 20),
//! the plans under the new rule. Messenger teams from `game.json`.
//!
//! «preallerta i vicini» needs no hindsight: at the opening and then every
//! 15 simulated minutes (the gap between two «Decidi»), it pre-alerts the
//! districts whose homes are within R of the burning front, the distance
//! shortened downwind as the coordinator ranks homes (`district::reach_m`).
//! «priorità sbagliate» ranks the districts the fire misses first.
//!
//! GREZZI=<file.csv> also writes each game's raw shares behind the stars.
//!
//! «anticipo reale» is, over the authorised districts the fire reached at
//! the doors, the minutes from the authorisation to the fire at the doors
//! (negative: authorised late); «mai raggiunti» counts the authorised
//! districts it never reached. «minuti reali» is the kiosk's pace: 40 s for
//! the opening decision, ×80 (×240 while `Game::is_quiet`, 10 min), 45 s at
//! ×1 for each authorisation. «evacua in ritardo» waits RITARDO_MIN
//! (default 10) after each authorisation.
//!
//!   cargo run --release -p rocca --example sindaca [caso...] [--semi 1,2,3]
use std::collections::BTreeMap;

use rocca::score::{self, Stars};
use rocca::{Civil, Game, Mayor, Plan, STEP_S};
use scenario::population::Status;

const OPENING_S: f64 = 40.0;
const AUTH_S: f64 = 45.0;
const PACE: (f64, f64) = (80.0, 240.0);
const QUIET_S: i64 = 10 * 60;

const OLD: [&str; 2] = ["nessun ordine", "evacua tutti subito"];
const NEW: [&str; 8] = [
    "evacua appena autorizzato",
    "evacua in ritardo",
    "solo preallerta",
    "preallerta tutti, evacua appena autorizzato",
    "preallerta ed evacua solo i minacciati",
    "preallerta i vicini (1,5 km), evacua appena autorizzato",
    "preallerta i vicini (3 km), evacua appena autorizzato",
    "preallerta tutti, evacua appena autorizzato, priorità sbagliate",
];
/// How often «preallerta i vicini» looks again: the gap between two «Decidi».
const DECIDE_S: i64 = 15 * 60;
/// A «Decidi» that changes something, at ×1.
const DECIDE_REAL_S: f64 = 30.0;

/// The radius of a «preallerta i vicini» plan, metres.
fn near_m(plan: &str) -> Option<f32> {
    match plan {
        "preallerta i vicini (1,5 km), evacua appena autorizzato" => Some(1500.0),
        "preallerta i vicini (3 km), evacua appena autorizzato" => Some(3000.0),
        _ => None,
    }
}

struct Run {
    points: i64,
    stars: Stars,
    caught: usize,
    out: usize,
    auths: usize,
    lead_min: Vec<i64>,
    never: usize,
    minutes: f64,
    /// Shares behind the stars: families and homes spared, timing, measure.
    raw: (Option<f64>, Option<f64>, Option<f64>, f64),
    needless: usize,
}

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let leads: Vec<i64> = std::env::var("SINDACA_MIN")
        .unwrap_or_else(|_| "20".into())
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    // how long «evacua in ritardo» waits after an authorisation
    let late_s: i64 = std::env::var("RITARDO_MIN").map_or(10, |v| v.parse().unwrap()) * 60;
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
        cases = rocca::Territory::load(data)?.cases.iter().map(|c| c.name.clone()).collect();
    }
    let pairs: Vec<(String, u64)> = cases.iter().flat_map(|c| seeds.iter().map(move |&s| (c.clone(), s))).collect();
    let withouts = par_map(&pairs, |(case, seed)| Game::without_orders(data, case, *seed));
    // (fire, rules: None = old or Some(lead minutes), plan)
    let mut jobs: Vec<(usize, Option<i64>, &str, Vec<usize>)> = vec![];
    for (k, without) in withouts.iter().enumerate() {
        let without = without.as_ref().map_err(|e| anyhow::anyhow!("{e:#}"))?;
        let mut hit: Vec<usize> = (0..without.districts.len())
            .filter(|&d| without.districts[d].caught > 0 || without.districts[d].homes_hit > 0)
            .collect();
        hit.sort_by_key(|&d| std::cmp::Reverse(without.districts[d].caught));
        if hit.is_empty() {
            continue;
        }
        jobs.extend(OLD.iter().map(|&p| (k, None, p, hit.clone())));
        for &l in &leads {
            jobs.extend(NEW.iter().map(|&p| (k, Some(l), p, hit.clone())));
        }
    }
    let runs = par_map(&jobs, |(k, lead, plan, hit)| -> anyhow::Result<Run> {
        let ((case, seed), without) = (&pairs[*k], withouts[*k].as_ref().unwrap());
        let mut g = Game::new(data, case, *seed)?;
        g.mayor = lead.map(|l| Mayor { lead_s: l * 60 });
        let n = g.districts.len();
        let all: Vec<usize> = (0..n).collect();
        let wrong: Vec<usize> = (0..n).filter(|d| !hit.contains(d)).chain(hit.iter().rev().copied()).collect();
        let order = if plan.contains("priorità sbagliate") { &wrong } else { hit };
        let mut p = Plan::new(n).with_priorities(order);
        let near = near_m(plan);
        let pre: &[usize] = match *plan {
            "solo preallerta" | "preallerta tutti, evacua appena autorizzato" | "preallerta tutti, evacua appena autorizzato, priorità sbagliate" => &all,
            "preallerta ed evacua solo i minacciati" => hit,
            _ => &[],
        };
        for &d in pre {
            p = p.with_civil(d, Civil::Preallerta);
        }
        if *plan == "evacua tutti subito" {
            for &d in &all {
                p = p.with_civil(d, Civil::Evacua);
            }
        }
        g.commit(p)?;
        let mut real = OPENING_S;
        let (mut auths, mut due) = (0usize, vec![]);
        let mut next_look = 0;
        while g.time_s() < g.case.duration_s() {
            if let Some(r) = near.filter(|_| g.time_s() >= next_look) {
                next_look = g.time_s() + DECIDE_S;
                let mut plan = g.active.clone();
                for d in 0..n {
                    let e = rocca::district::exposure(&g.districts[d], &g.agents, &g.fire, &g.scn);
                    if plan.civil[d] == Civil::Nessuno && e.is_some_and(|e| rocca::district::reach_m(e.distance_m, e.downwind) <= r) {
                        plan = plan.with_civil(d, Civil::Preallerta);
                    }
                }
                if plan != g.active {
                    real += if g.time_s() > STEP_S { DECIDE_REAL_S } else { 0.0 };
                    g.commit(plan)?;
                }
            }
            g.step()?;
            real += STEP_S as f64 / if g.is_quiet(QUIET_S) { PACE.1 } else { PACE.0 };
            let now = g.time_s();
            for d in g.take_authorisations() {
                auths += 1;
                real += AUTH_S;
                let wants = match *plan {
                    p if p.contains("evacua appena autorizzato") => true,
                    "evacua in ritardo" => {
                        due.push((now + late_s, d));
                        false
                    }
                    "preallerta ed evacua solo i minacciati" => hit.contains(&d),
                    _ => false,
                };
                if wants {
                    let plan = g.active.clone().with_civil(d, Civil::Evacua);
                    g.commit(plan)?;
                }
            }
            // the late plan: the player's next decision moment
            let ready: Vec<usize> = due.iter().filter(|(t, _)| *t <= now).map(|(_, d)| *d).collect();
            if !ready.is_empty() {
                due.retain(|(t, _)| *t > now);
                real += AUTH_S;
                let mut plan = g.active.clone();
                for d in ready {
                    plan = plan.with_civil(d, Civil::Evacua);
                }
                g.commit(plan)?;
            }
        }
        let o = g.outcome();
        let sc = score::score(&o, without);
        let out = (0..g.agents.households.len())
            .filter(|&k| matches!(g.agents.households[k].status, Status::Trapped | Status::Casualty) && g.caught_at(k).is_none())
            .count();
        let (mut lead_min, mut never) = (vec![], 0);
        for d in 0..n {
            match (g.authorised_at(d), g.at_doors_at(d)) {
                (Some(a), Some(t)) => lead_min.push((t - a) / 60),
                (Some(_), None) => never += 1,
                _ => {}
            }
        }
        Ok(Run {
            points: sc.total,
            stars: score::stars(&sc, g.timing(), g.measure()),
            caught: o.caught(),
            out,
            auths,
            lead_min,
            never,
            minutes: real / 60.0,
            raw: (
                (sc.families_at_risk > 0).then(|| sc.families as f64 / sc.families_at_risk as f64),
                (sc.homes_at_risk > 0).then(|| sc.homes as f64 / sc.homes_at_risk as f64),
                g.timing(),
                g.measure(),
            ),
            needless: g.needless_evacuations(),
        })
    });
    let star = |s: Option<u8>| s.map_or("—".to_string(), |s| s.to_string());
    let rules = |l: &Option<i64>| l.map_or("vecchie".to_string(), |l| format!("sindaca {l} min"));
    println!("| caso | seme | regole | piano | punti | P / C / T / M | colte in casa | colte fuori | autorizzazioni | anticipo reale (min) | mai raggiunti | minuti reali |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut done = vec![];
    for (job, run) in jobs.iter().zip(runs) {
        let r = run?;
        let (case, seed) = &pairs[job.0];
        let s = r.stars;
        println!(
            "| {case} | {seed} | {} | {} | {} | {} / {} / {} / {} | {} | {} | {} | {} | {} | {:.1} |",
            rules(&job.1),
            job.2,
            r.points,
            star(s.persone),
            star(s.case),
            star(s.tempismo),
            s.misura,
            r.caught,
            r.out,
            r.auths,
            r.lead_min.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(", "),
            r.never,
            r.minutes
        );
        done.push((job.0, job.1, job.2, r));
    }
    if let Ok(path) = std::env::var("GREZZI") {
        let opt = |x: Option<f64>| x.map_or(String::new(), |x| format!("{x:.4}"));
        let mut csv = String::from("caso,seme,regole,piano,punti,famiglie,case,tempismo,misura,evacuate_inutili\n");
        for (k, lead, plan, r) in &done {
            let (case, seed) = &pairs[*k];
            csv += &format!("{case},{seed},{},\"{plan}\",{},{},{},{},{:.4},{}\n", rules(lead), r.points, opt(r.raw.0), opt(r.raw.1), opt(r.raw.2), r.raw.3, r.needless);
        }
        std::fs::write(path, csv)?;
    }

    // Averages per rules and plan.
    println!();
    println!("| regole | piano | punti | ★ persone | ★ case | ★ tempismo | ★ misura | colte in casa | colte fuori | autorizzazioni | minuti reali |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    let mut groups: BTreeMap<(Option<i64>, usize), Vec<&Run>> = BTreeMap::new();
    for (_, lead, plan, r) in &done {
        let order = OLD.iter().chain(NEW.iter()).position(|p| p == plan).unwrap();
        groups.entry((*lead, order)).or_default().push(r);
    }
    let mean = |v: &[f64]| if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 };
    for ((lead, order), rs) in &groups {
        let f = |g: &dyn Fn(&Run) -> Option<f64>| mean(&rs.iter().filter_map(|r| g(r)).collect::<Vec<_>>());
        println!(
            "| {} | {} | {:.0} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} |",
            rules(lead),
            OLD.iter().chain(NEW.iter()).nth(*order).unwrap(),
            f(&|r| Some(r.points as f64)),
            f(&|r| r.stars.persone.map(f64::from)),
            f(&|r| r.stars.case.map(f64::from)),
            f(&|r| r.stars.tempismo.map(f64::from)),
            f(&|r| Some(r.stars.misura as f64)),
            f(&|r| Some(r.caught as f64)),
            f(&|r| Some(r.out as f64)),
            f(&|r| Some(r.auths as f64)),
            f(&|r| Some(r.minutes)),
        );
    }

    // Does «evacua appena autorizzato» dominate? Per fire and lead: the
    // fires where it loses more than 20 points to another plan.
    println!();
    println!("| regole | incendi | «evacua appena autorizzato» perde > 20 punti contro: in ritardo / solo preallerta / preallerta + evacua / solo i minacciati / vecchie «evacua tutti subito» | anticipo reale mediano (min) | autorizzate tardi (dopo il fuoco alle case) | autorizzate mai raggiunte |");
    println!("|---|---|---|---|---|---|");
    for &l in &leads {
        let fires: Vec<usize> = {
            let mut f: Vec<usize> = done.iter().filter(|x| x.1 == Some(l)).map(|x| x.0).collect();
            f.dedup();
            f
        };
        let get = |k: usize, lead: Option<i64>, plan: &str| done.iter().find(|x| x.0 == k && x.1 == lead && x.2 == plan).map(|x| x.3.points);
        let loses = |other: &str, lead: Option<i64>| {
            fires.iter().filter(|&&k| match (get(k, Some(l), NEW[0]), get(k, lead, other)) {
                (Some(a), Some(b)) => b - a > 20,
                _ => false,
            }).count()
        };
        let mut leads_all: Vec<i64> = done.iter().filter(|x| x.1 == Some(l) && x.2 == NEW[0]).flat_map(|x| x.3.lead_min.clone()).collect();
        leads_all.sort_unstable();
        let late = leads_all.iter().filter(|&&m| m < 0).count();
        let never: usize = done.iter().filter(|x| x.1 == Some(l) && x.2 == NEW[0]).map(|x| x.3.never).sum();
        println!(
            "| sindaca {l} min | {} | {} / {} / {} / {} / {} | {} | {} su {} | {} |",
            fires.len(),
            loses(NEW[1], Some(l)),
            loses(NEW[2], Some(l)),
            loses(NEW[3], Some(l)),
            loses(NEW[4], Some(l)),
            loses(OLD[1], None),
            leads_all.get(leads_all.len() / 2).map_or("—".into(), |m| m.to_string()),
            late,
            leads_all.len(),
            never
        );
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
