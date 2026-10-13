//! Playtest 7 found that «evacuate everyone at T+0» costs nothing. The traffic
//! queue exists (`abm::traffic`) and slows the engines (`emergency_factor`):
//! does it ever form here, where, and does it touch the units or the families?
//! Same fire and seed per row group; only the civil plan changes.
//!
//!   cargo run --release -p rocca --example intasamento [caso...] [--semi 1,2,3]
use abm::suppression::UnitState;
use abm::Mode;
use rocca::game::Arrival;
use rocca::{Civil, Game, Plan};
use scenario::population::Status;

fn best_priority(case: &str) -> &'static [&'static str] {
    match case {
        "Rocco3" => &["San Rocco"],
        "Forna3_gira" => &["Fornaci"],
        _ => &["Le Ghiande", "Le Terrazze"],
    }
}

#[derive(Clone, Copy)]
enum Civ {
    None,
    EvacAll(i64),
    PreAll,
    PreThenEvac(i64),
}

impl Civ {
    fn label(self) -> String {
        match self {
            Civ::None => "nessun ordine".into(),
            Civ::EvacAll(m) => format!("evacua tutti T+{m}"),
            Civ::PreAll => "preallerta tutti".into(),
            Civ::PreThenEvac(m) => format!("preallerta, evacua T+{m}"),
        }
    }
}

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
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
        cases = vec!["Rocco3".into(), "Forna3_gira".into(), "Coste2_gira".into()];
    }
    let plans = [Civ::None, Civ::EvacAll(0), Civ::EvacAll(30), Civ::PreAll, Civ::PreThenEvac(30)];

    println!("| caso | seme | piano | punti | auto in strada (max) | in coda (max) | link più pieno | partenze 90 % (min) | ultimo arrivo (min) | mezzi in moto (min·mezzo) | 1° mezzo sul borgo | auto scaldate dal fuoco | pericolo max su un'auto | auto·min in allarme | dove la coda (al picco) | colte in casa | colte fuori | case |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for case in &cases {
        for &seed in &seeds {
            let without = Game::without_orders(data, case, seed)?;
            for civ in plans {
                let mut g = Game::new(data, case, seed)?;
                // the rules before 2026-10-13: any evacuation at any time (`examples/sindaca.rs` plays the mayor's)
                g.mayor = None;
                let n = g.districts.len();
                // PRIORITA="San Rocco,Castelvento" overrides the playtest-7 best.
                let prio: Vec<usize> = match std::env::var("PRIORITA") {
                    Ok(p) => p.split(',').map(|s| g.district_index(s.trim()).unwrap()).collect(),
                    Err(_) => best_priority(case).iter().map(|s| g.district_index(s).unwrap()).collect(),
                };
                let set_all = |p: Plan, c: Civil| (0..n).fold(p, |p, d| p.with_civil(d, c));
                let mut plan = Plan::new(n).with_priorities(&prio);
                let mut later: Option<(i64, Civil)> = None;
                match civ {
                    Civ::None => {}
                    Civ::EvacAll(0) => plan = set_all(plan, Civil::Evacua),
                    Civ::EvacAll(m) => later = Some((m * 60, Civil::Evacua)),
                    Civ::PreAll => plan = set_all(plan, Civil::Preallerta),
                    Civ::PreThenEvac(m) => {
                        plan = set_all(plan, Civil::Preallerta);
                        later = Some((m * 60, Civil::Evacua));
                    }
                }
                g.commit(plan.clone())?;

                let hh = g.agents.households.len();
                let mut left: Vec<Option<i64>> = vec![None; hh];
                let mut arrived_last = 0i64;
                let (mut cars_max, mut queued_max, mut busiest) = (0usize, 0usize, (0u16, 0.0f32));
                let mut moving_s = 0i64;
                let mut first_post: Option<i64> = None;
                let mut heated = std::collections::HashSet::new();
                let (mut danger_max, mut alarm_s) = (0.0f32, 0i64);
                let mut last_pos: Vec<scenario::Pos> = vec![];
                let (mut stopped_max, mut where_max) = (0usize, String::new());
                let end = g.case.duration_s();
                while g.time_s() < end {
                    if let Some((t, c)) = later {
                        if g.time_s() >= t {
                            plan = set_all(g.active.clone(), c);
                            g.commit(plan.clone())?;
                            later = None;
                        }
                    }
                    g.step()?;
                    let now = g.time_s();
                    for (k, h) in g.agents.households.iter().enumerate() {
                        if left[k].is_none() && matches!(h.status, Status::Evacuating | Status::Evacuated) {
                            left[k] = Some(now);
                        }
                    }
                    if g.agents.households.iter().any(|h| h.status == Status::Evacuating) {
                        arrived_last = now;
                    }
                    for (ti, t) in g.agents.travellers.iter().enumerate() {
                        if t.mode == Mode::Car && t.heat_s > 0.0 {
                            heated.insert(ti);
                        }
                    }
                    let mut stopped: Vec<usize> = vec![0; n];
                    let mut n_stopped = 0;
                    for (ti, t) in g.agents.travellers.iter().enumerate() {
                        if t.mode != Mode::Car || !matches!(t.state, abm::TravelState::OnNetwork | abm::TravelState::Approaching) {
                            continue;
                        }
                        let d = g.fire.threat().at(t.pos);
                        danger_max = danger_max.max(d);
                        if d >= fire::threat::ALARMING {
                            alarm_s += rocca::STEP_S;
                        }
                        if last_pos.get(ti).is_some_and(|p| rocca::district::dist(*p, t.pos) < 1.0) {
                            let near = (0..n).min_by(|&a, &b| rocca::district::dist(g.districts[a].centre, t.pos).total_cmp(&rocca::district::dist(g.districts[b].centre, t.pos))).unwrap();
                            stopped[near] += 1;
                            n_stopped += 1;
                        }
                    }
                    if n_stopped > stopped_max {
                        stopped_max = n_stopped;
                        let mut v: Vec<(usize, usize)> = stopped.iter().copied().enumerate().filter(|x| x.1 > 0).collect();
                        v.sort_by_key(|x| std::cmp::Reverse(x.1));
                        where_max = v.iter().take(3).map(|(d, c)| format!("{} {c}", g.districts[*d].name)).collect::<Vec<_>>().join(", ") + &format!(" @T+{}", now / 60);
                    }
                    last_pos = g.agents.travellers.iter().map(|t| t.pos).collect();
                    let cars = g.agents.travellers.iter().filter(|t| t.mode == Mode::Car).count();
                    cars_max = cars_max.max(cars);
                    queued_max = queued_max.max(g.agents.traffic.queued_vehicles());
                    if let Some((_, c, s)) = g.agents.traffic.busiest() {
                        if c > busiest.0 {
                            busiest = (c, s);
                        }
                    }
                    moving_s += g.crews.units.iter().filter(|u| u.state == UnitState::Moving).count() as i64 * rocca::STEP_S;
                    if first_post.is_none() && g.arrivals(prio[0]).iter().any(|(_, a)| matches!(a, Arrival::OnPost)) {
                        first_post = Some(now);
                    }
                }
                let o = g.outcome();
                let sc = rocca::score::score(&o, &without);
                let mut times: Vec<i64> = left.iter().flatten().copied().collect();
                times.sort();
                let p90 = if times.is_empty() { "–".into() } else { format!("{}", times[(times.len() * 9 / 10).min(times.len() - 1)] / 60) };
                let caught_out = (0..hh)
                    .filter(|&k| matches!(g.agents.households[k].status, Status::Trapped | Status::Casualty) && g.caught_at(k).is_none())
                    .count();
                println!(
                    "| {case} | {seed} | {} | {} | {cars_max} | {queued_max} | {} ({:.0} %) | {p90} | {} | {} | {} | {} | {} | {:.2} | {} | {} | {caught_out} | {} |",
                    civ.label(),
                    sc.total,
                    busiest.0,
                    busiest.1 * 100.0,
                    if times.is_empty() { "–".into() } else { format!("{}", arrived_last / 60) },
                    moving_s / 60,
                    first_post.map_or("–".into(), |t| format!("T+{}", t / 60)),
                    heated.len(),
                    danger_max,
                    alarm_s / 60,
                    where_max,
                    o.caught(),
                    o.homes_hit(),
                );
            }
        }
    }
    Ok(())
}
