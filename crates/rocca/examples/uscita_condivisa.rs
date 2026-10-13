//! The hamlets' cars reach the refuge through Castelvento's streets, so an
//! evacuation of Castelvento (which is non-burnable, and drives 300 m to its
//! own refuge) can hold them on their access road, in the fuel. Does it, in the
//! cases where the fire cuts that road early? Same fire and seed per group.
//!
//!   cargo run --release -p rocca --example uscita_condivisa -- [caso:borgo ...] [--semi 1,2,3]
use abm::Mode;
use rocca::{Civil, Game, Plan};
use scenario::population::Status;

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut seeds = vec![1u64];
    let mut cases: Vec<(String, String)> = vec![];
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--semi" {
            seeds = args[i + 1].split(',').map(|s| s.parse().unwrap()).collect();
            i += 2;
        } else {
            let (c, d) = args[i].split_once(':').unwrap();
            cases.push((c.into(), d.into()));
            i += 1;
        }
    }
    if cases.is_empty() {
        for (c, d) in [("Borgo2", "San Rocco"), ("Borgo2_gira", "San Rocco"), ("Terra3", "Fornaci"), ("Terra3_gira", "Fornaci"), ("Forna1_gira", "Fornaci"), ("Rocco2_gira", "Fornaci")] {
            cases.push((c.into(), d.into()));
        }
    }
    println!("| caso | seme | ordine a | piano | punti | colte in casa (borgo) | colte fuori | pericolo max su auto | auto·min in allarme | auto ferme vicino al borgo (max) | calore max su un'auto (letale = 90 s) |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for (case, hamlet) in &cases {
        for &seed in &seeds {
            let without = Game::without_orders(data, case, seed)?;
            for at in [0i64, 15, 30, 45] {
                for who in ["solo il borgo", "tutti tranne Castelvento", "tutti"] {
                    let mut g = Game::new(data, case, seed)?;
                    // the rules before 2026-10-13: any evacuation at any time (`examples/sindaca.rs` plays the mayor's)
                    g.mayor = None;
                    let n = g.districts.len();
                    let h = g.district_index(hamlet).unwrap();
                    let cv = g.district_index("Castelvento").unwrap();
                    let targets: Vec<usize> = match who {
                        "solo il borgo" => vec![h],
                        "tutti tranne Castelvento" => (0..n).filter(|&d| d != cv).collect(),
                        _ => (0..n).collect(),
                    };
                    g.commit(Plan::new(n).with_priorities(&[h]))?;
                    g.run_until(at * 60)?;
                    let p = targets.iter().fold(g.active.clone(), |p, &d| p.with_civil(d, Civil::Evacua));
                    g.commit(p)?;
                    let (mut danger, mut alarm_s, mut stopped_max, mut heat) = (0.0f32, 0i64, 0usize, 0.0f32);
                    let mut last: Vec<scenario::Pos> = vec![];
                    let centre = g.districts[h].centre;
                    while g.time_s() < g.case.duration_s() {
                        g.step()?;
                        let mut stopped = 0;
                        for (ti, t) in g.agents.travellers.iter().enumerate() {
                            if t.mode != Mode::Car || !matches!(t.state, abm::TravelState::OnNetwork | abm::TravelState::Approaching) {
                                continue;
                            }
                            heat = heat.max(t.heat_s);
                            let d = g.fire.threat().at(t.pos);
                            danger = danger.max(d);
                            if d >= fire::threat::ALARMING {
                                alarm_s += rocca::STEP_S;
                            }
                            if last.get(ti).is_some_and(|p| rocca::district::dist(*p, t.pos) < 1.0) && rocca::district::dist(centre, t.pos) < 600.0 {
                                stopped += 1;
                            }
                        }
                        stopped_max = stopped_max.max(stopped);
                        last = g.agents.travellers.iter().map(|t| t.pos).collect();
                    }
                    let o = g.outcome();
                    let sc = rocca::score::score(&o, &without);
                    let out = (0..g.agents.households.len())
                        .filter(|&k| matches!(g.agents.households[k].status, Status::Trapped | Status::Casualty) && g.caught_at(k).is_none())
                        .count();
                    println!(
                        "| {case} | {seed} | T+{at} | {who} | {} | {} | {out} | {danger:.2} | {} | {stopped_max} | {heat:.0} |",
                        sc.total,
                        o.districts[h].caught,
                        alarm_s / 60
                    );
                }
            }
        }
    }
    Ok(())
}
