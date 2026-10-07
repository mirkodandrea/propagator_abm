//! Milestone 0 (`docs/demo-spec-gameplay.md` §7): the six measurements that
//! decide which tokens ship. Rocca Ventosa, drawn seeds, every policy played
//! through `Session` exactly as a player would, all cores, mean ± s.e.
//!
//! Measurements, so `#[ignore]`d: run with
//! `cargo test --release -p demo --test sweeps -- --ignored --nocapture --test-threads 1`.
//! The tables are recorded in the gameplay spec §7; the conclusions that
//! hold are pinned in `tests/lessons.rs` and `tests/balance.rs`.

use std::collections::HashMap;

use demo::district::Level;
use demo::turn_policy::{self as tp, grid, mean_se, Sel, TurnPolicy, BORGO, COSTE};
use demo::*;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

const N: u64 = 40;

fn seeds() -> Vec<u64> {
    (1..=N).collect()
}

fn shifts(seed: u64) -> bool {
    demo::draw(TOWN, seed).unwrap().spec.shift.is_some()
}

fn fmt((m, se): (f32, f32)) -> String {
    format!("{m:6.1} ± {se:4.1}")
}

/// Play each policy on each seed; `probe` reads what it needs off the finished session.
fn sweep<R: Send>(policies: &[TurnPolicy], probe: impl Fn(&mut Session) -> R + Sync) -> HashMap<(u64, usize), R> {
    let dir = data_dir();
    grid(&seeds(), policies.len(), |seed, j| {
        let mut s = Session::new(&dir, seed).unwrap();
        policies[j].play(&mut s).unwrap();
        probe(&mut s)
    })
    .into_iter()
    .map(|(s, j, r)| ((s, j), r))
    .collect()
}


/// Mean ± s.e. of `f` over seeds (optionally only shift / hold sessions) for policy `j`.
fn col<R>(rs: &HashMap<(u64, usize), R>, j: usize, only: Option<bool>, f: impl Fn(&R) -> f32) -> (f32, f32) {
    let v: Vec<f32> = seeds().into_iter().filter(|s| only.map_or(true, |w| shifts(*s) == w)).map(|s| f(&rs[&(s, j)])).collect();
    mean_se(&v)
}

/// Paired difference (j - base) per seed.
fn diff<R>(rs: &HashMap<(u64, usize), R>, j: usize, base: usize, only: Option<bool>, f: impl Fn(&R) -> f32) -> (f32, f32) {
    let v: Vec<f32> = seeds().into_iter().filter(|s| only.map_or(true, |w| shifts(*s) == w)).map(|s| f(&rs[&(s, j)]) - f(&rs[&(s, base)])).collect();
    mean_se(&v)
}

fn at(turn: u8, plan: Vec<(TokenId, Sel)>) -> impl Fn(&Session) -> Vec<(TokenId, Sel)> + Send + Sync {
    move |s: &Session| if s.turn().index == turn { plan.clone() } else { vec![] }
}

#[test]
#[ignore]
fn station_drive_times() {
    let s = Session::new(&data_dir(), 1).unwrap();
    println!("station at {:?}", s.station_pos());
    for t in s.targets() {
        let p: Vec<String> = [TokenId::P, TokenId::E1, TokenId::S].iter().filter_map(|k| s.preview(*k, t.id).map(|p| format!("{k:?} {:.1} min", p.eta_s as f32 / 60.0))).collect();
        println!("{:?}: {}", t.kind, p.join(", "));
    }
    let n = seeds().iter().filter(|s| shifts(**s)).count();
    println!("wind shifts in {n} of {N} drawn sessions");
}

/// §7.1 Hand crew on Il Borgo's edge at turn 1 vs none. Pass: ≥ 5 homes fewer.
/// Also, for the lead, what a `unit_effect` on line production would take
/// (flag: more effective than the published model).
#[test]
#[ignore]
fn s1_hand_crew() {
    let ps = vec![
        tp::none(),
        TurnPolicy::new("crew-borgo-t1", at(1, vec![(TokenId::S, Sel::District(BORGO))])),
        TurnPolicy::new("crew-flank-left-t1", at(1, vec![(TokenId::S, Sel::Flank(Side::Left))])),
        TurnPolicy::new("crew-flank-right-t1", at(1, vec![(TokenId::S, Sel::Flank(Side::Right))])),
    ];
    let rs = sweep(&ps, |s| {
        let f = s.facts();
        let line = s.run.crews.units[3].line_cut_m;
        (f.homes_hit as f32, f.districts[BORGO].1 as f32, line)
    });
    println!("\n§7.1 hand crew (N={N}): homes hit, all | wind holds | Borgo homes | line cut (m) | Δ homes vs none (paired)");
    for (j, p) in ps.iter().enumerate() {
        println!(
            "{:22} {} | {} | {} | {} | Δ {}",
            p.name,
            fmt(col(&rs, j, None, |r| r.0)),
            fmt(col(&rs, j, Some(false), |r| r.0)),
            fmt(col(&rs, j, None, |r| r.1)),
            fmt(col(&rs, j, None, |r| r.2)),
            fmt(diff(&rs, j, 0, None, |r| r.0)),
        );
    }
    // Variants: the same crew-borgo-t1 with faster line production.
    let dir = data_dir();
    for x in [4.0f32, 10.0] {
        let v = Variant { crews_defend: false, unit_effect: abm::suppression::UnitEffect { line_x: x, ..abm::suppression::UnitEffect::ONE }, ..demo::session::variant() };
        let r = grid(&seeds(), 2, |seed, j| {
            let d = demo::draw(TOWN, seed).unwrap();
            let mut s = Session::with_variant(&dir, d, v).unwrap();
            if j == 1 {
                TurnPolicy::new("", at(1, vec![(TokenId::S, Sel::District(BORGO))])).play(&mut s).unwrap();
            } else {
                s.finish().unwrap();
            }
            (s.facts().homes_hit as f32, s.run.crews.units[3].line_cut_m)
        });
        let d: Vec<f32> = seeds().iter().map(|sd| r.iter().find(|x| x.0 == *sd && x.1 == 1).unwrap().2 .0 - r.iter().find(|x| x.0 == *sd && x.1 == 0).unwrap().2 .0).collect();
        let line: Vec<f32> = r.iter().filter(|x| x.1 == 1).map(|x| x.2 .1).collect();
        println!("variant line_x={x}: crew-borgo-t1 Δ homes vs none {} | line {}", fmt(mean_se(&d)), fmt(mean_se(&line)));
    }
}

/// §7.2 Canadair called turn 1, then drops on Il Borgo's edge / a flank / a
/// spot fire vs never called. Pass: beats none by ≥ 5 homes, or puts out a
/// spot fire most of the time.
#[test]
#[ignore]
fn s2_canadair() {
    let drop_on = |sel: Sel| {
        move |s: &Session| {
            let k = s.token(TokenId::K);
            if s.turn().index == 1 {
                vec![(TokenId::K, Sel::Sky)]
            } else if k.orderable && k.state != TokenState::NonChiamato && k.doing.is_none() {
                vec![(TokenId::K, sel)]
            } else {
                vec![]
            }
        }
    };
    let ps = vec![
        tp::none(),
        TurnPolicy::new("K borgo edge", drop_on(Sel::District(BORGO))),
        TurnPolicy::new("K flank left", drop_on(Sel::Flank(Side::Left))),
        TurnPolicy::new("K flank right", drop_on(Sel::Flank(Side::Right))),
        TurnPolicy::new("K head", drop_on(Sel::Head)),
        TurnPolicy::new("K at-risk edge", |s: &Session| {
            let k = s.token(TokenId::K);
            let d = if s.wind_turned() { COSTE } else { BORGO };
            if s.turn().index == 1 {
                vec![(TokenId::K, Sel::Sky)]
            } else if k.orderable && k.state != TokenState::NonChiamato && k.doing != Some(TargetKind::District(d)) {
                vec![(TokenId::K, Sel::District(d))]
            } else {
                vec![]
            }
        }),
    ];
    let rs = sweep(&ps, |s| {
        let f = s.facts();
        let live = s.spot_fires().iter().filter(|x| x.1.is_none()).count() as f32;
        let out = s.spot_fires().iter().filter(|x| x.1 == Some(false)).count() as f32;
        (f.homes_hit as f32, live, out, s.drops().1 as f32, s.drops().0 as f32)
    });
    println!("\n§7.2 Canadair called t1 (N={N}): homes hit | Δ vs none (paired) | Δ wind holds | Δ shifts | spots live at T+60 | spots out | drops ≤T+40 | drops all");
    for (j, p) in ps.iter().enumerate() {
        println!(
            "{:16} {} | Δ {} | {} | {} | {} | {} | {} | {}",
            p.name,
            fmt(col(&rs, j, None, |r| r.0)),
            fmt(diff(&rs, j, 0, None, |r| r.0)),
            fmt(diff(&rs, j, 0, Some(false), |r| r.0)),
            fmt(diff(&rs, j, 0, Some(true), |r| r.0)),
            fmt(col(&rs, j, None, |r| r.1)),
            fmt(col(&rs, j, None, |r| r.2)),
            fmt(col(&rs, j, None, |r| r.3)),
            fmt(col(&rs, j, None, |r| r.4)),
        );
    }
}

// §7.3 (engines and crews on spot fires) was measured on 2026-10-07 and the
// spot-fire target cut; the table is in the gameplay spec.

/// §7.4 Patrol to Il Borgo at turn 1 vs an instant warning at T+0 vs none:
/// families caught. Pass: the patrol still ≤ half of none. And the Borgo →
/// Le Coste sequencing in shift sessions.
#[test]
#[ignore]
fn s4_patrol_delay() {
    let dir = data_dir();
    // Job 0..: none, patrol Borgo t1, instant Borgo T+0, patrol B t1 + C t2,
    // IT-alert t1, instant Borgo + Coste T+0, patrol Coste t1 + Borgo t2.
    let names = ["none", "patrol-borgo-t1", "instant-borgo-T0", "patrol-borgo-t1+coste-t2", "(IT-alert, cut)", "instant-borgo+coste-T0", "patrol-coste-t1+borgo-t2", "patrol-borgo-t3"];
    let rs: HashMap<(u64, usize), (f32, f32, f32, f32, f32)> = grid(&seeds(), names.len(), |seed, j| {
        let mut s = Session::new(&dir, seed).unwrap();
        let p: TurnPolicy = match j {
            1 => tp::patrol_borgo_t1(),
            3 => tp::patrol_borgo_coste(),
            4 => tp::none(),
            6 => TurnPolicy::new("", |s: &Session| match s.turn().index {
                1 => vec![(TokenId::P, Sel::District(COSTE))],
                2 => vec![(TokenId::P, Sel::District(BORGO))],
                _ => vec![],
            }),
            7 => TurnPolicy::new("", at(3, vec![(TokenId::P, Sel::District(BORGO))])),
            _ => tp::none(),
        };
        if j == 2 {
            s.run.order(Order::EvacuateDistrict(BORGO));
        }
        if j == 5 {
            s.run.order(Order::EvacuateDistrict(BORGO));
            s.run.order(Order::EvacuateDistrict(COSTE));
        }
        p.play(&mut s).unwrap();
        let f = s.facts();
        let warned_b = s.run.referee.reports[BORGO].warned_at_s.map_or(-1.0, |t| t as f32 / 60.0);
        let warned_c = s.run.referee.reports[COSTE].warned_at_s.map_or(-1.0, |t| t as f32 / 60.0);
        (f.families_caught as f32, f.districts[BORGO].0 as f32, f.districts[COSTE].0 as f32, warned_b, warned_c)
    })
    .into_iter()
    .map(|(s, j, r)| ((s, j), r))
    .collect();
    println!("\n§7.4 patrol (N={N}). families caught: all | wind holds (Borgo caught) | shifts (Coste caught) | warning landed Borgo / Coste (min)");
    for (j, n) in names.iter().enumerate() {
        println!(
            "{:26} {} | {} ({}) | {} ({}) | {} / {}",
            n,
            fmt(col(&rs, j, None, |r| r.0)),
            fmt(col(&rs, j, Some(false), |r| r.0)),
            fmt(col(&rs, j, Some(false), |r| r.1)),
            fmt(col(&rs, j, Some(true), |r| r.0)),
            fmt(col(&rs, j, Some(true), |r| r.2)),
            fmt(col(&rs, j, None, |r| r.3)),
            fmt(col(&rs, j, None, |r| r.4)),
        );
    }
}

/// §7.5 Engines posted on Il Borgo at turn 1 vs turn 2 vs turn 3: how many
/// are working (with water) vs refilling when the fire reaches Il Borgo, and
/// homes hit. Wind-holds sessions are where the front reaches Il Borgo.
#[test]
#[ignore]
fn s5_tank() {
    let dir = data_dir();
    let names = ["none", "engines-borgo-t1", "engines-borgo-t2", "engines-borgo-t3"];
    let rs: HashMap<(u64, usize), (f32, f32, f32, f32, f32)> = grid(&seeds(), names.len(), |seed, j| {
        let mut s = Session::new(&dir, seed).unwrap();
        let turn = j as u8;
        let p = TurnPolicy::new("", move |s: &Session| {
            let t = s.turn().index;
            let mut v = vec![];
            if turn > 0 && t == turn {
                v.extend([(TokenId::E1, Sel::District(BORGO)), (TokenId::E2, Sel::District(BORGO))]);
            }
            if turn > 0 && t == turn.max(REINFORCEMENT_TURN) {
                v.push((TokenId::E3, Sel::District(BORGO)));
            }
            v
        });
        // State of the engines the moment the fire first comes within the
        // threat distance of Il Borgo (or reaches it).
        let mut snap: Option<(f32, f32, f32)> = None;
        while !s.finished() {
            if !s.turn().finale() {
                p.order(&mut s);
            }
            s.end_turn_observed(|x| {
                if snap.is_none() {
                    let r = &x.run.referee.reports[BORGO];
                    if r.threatened_at_s.is_some() || r.reached_at_s.is_some() {
                        let mut working = 0.0;
                        let mut refilling = 0.0;
                        let mut water = 0.0;
                        for k in 0..3 {
                            let u = &x.run.crews.units[k];
                            if u.task == abm::suppression::Task::Hold {
                                continue;
                            }
                            water += u.water_frac();
                            match u.state {
                                abm::suppression::UnitState::Working if u.water_l > 0.0 => working += 1.0,
                                abm::suppression::UnitState::Refilling => refilling += 1.0,
                                _ => {}
                            }
                        }
                        snap = Some((working, refilling, water));
                    }
                }
            })
            .unwrap();
        }
        let f = s.facts();
        let (w, r, water) = snap.unwrap_or((-1.0, -1.0, -1.0));
        (f.homes_hit as f32, f.districts[BORGO].1 as f32, w, r, water)
    })
    .into_iter()
    .map(|(s, j, r)| ((s, j), r))
    .collect();
    println!("\n§7.5 tank, wind holds only (N={}). homes hit | Borgo homes | Δ Borgo vs none | Δ Borgo vs posted t1 | at threat: working with water | refilling | water (sum of tanks)", seeds().iter().filter(|s| !shifts(**s)).count());
    for (j, n) in names.iter().enumerate() {
        let snaps = |f: fn(&(f32, f32, f32, f32, f32)) -> f32| {
            let v: Vec<f32> = seeds().into_iter().filter(|s| !shifts(*s)).map(|s| rs[&(s, j)]).filter(|r| r.2 >= 0.0).map(|r| f(&r)).collect();
            mean_se(&v)
        };
        println!(
            "{:18} {} | {} | Δ {} | Δ {} | {} | {} | {}",
            n,
            fmt(col(&rs, j, Some(false), |r| r.0)),
            fmt(col(&rs, j, Some(false), |r| r.1)),
            fmt(diff(&rs, j, 0, Some(false), |r| r.1)),
            fmt(diff(&rs, j, 1, Some(false), |r| r.1)),
            fmt(snaps(|r| r.2)),
            fmt(snaps(|r| r.3)),
            fmt(snaps(|r| r.4)),
        );
    }
}

/// §7.6 Every ground unit sent to the head withdraws within one turn, in
/// every drawn session. Measured for both readings of "the head" as an order.
#[test]
#[ignore]
fn s6_head_attack() {
    let dir = data_dir();
    let cases = [(TokenId::E1, 1u8), (TokenId::E1, 2), (TokenId::E1, 3), (TokenId::S, 1), (TokenId::S, 2), (TokenId::S, 3)];
    for mode in [demo::session::HeadOrder::Fixed, demo::session::HeadOrder::Track] {
        let rs = grid(&seeds(), cases.len(), |seed, j| {
            let (tok, turn) = cases[j];
            let mut s = Session::new(&dir, seed).unwrap();
            s.head_order = mode;
            let p = TurnPolicy::new("", at(turn, vec![(tok, Sel::Head)]));
            p.play_until(&mut s, turn).unwrap();
            p.order(&mut s);
            let sent = s.pending().iter().any(|(t, _)| *t == tok);
            let k = tok.unit().unwrap();
            let mut pulled: Option<i64> = None;
            let t0 = s.time_s();
            for _ in 0..2 {
                s.end_turn_observed(|x| {
                    if pulled.is_none() && x.run.crews.units[k].state == abm::suppression::UnitState::Withdrawing {
                        pulled = Some(x.time_s() - t0);
                    }
                })
                .unwrap();
            }
            let worked = s.run.crews.units[k].water_used_l + s.run.crews.units[k].line_cut_m;
            (sent, pulled, worked)
        });
        println!("\n§7.6 head attack, {mode:?} (N={N}): sent | withdrew within 1 turn | within 2 turns | minutes to withdraw | work done (L or m of line)");
        for (j, (tok, turn)) in cases.iter().enumerate() {
            let mine: Vec<_> = rs.iter().filter(|r| r.1 == j && r.2 .0).collect();
            let one = mine.iter().filter(|r| r.2 .1.is_some_and(|t| t <= TURN_S)).count();
            let two = mine.iter().filter(|r| r.2 .1.is_some()).count();
            let mins: Vec<f32> = mine.iter().filter_map(|r| r.2 .1.map(|t| t as f32 / 60.0)).collect();
            let work: Vec<f32> = mine.iter().map(|r| r.2 .2).collect();
            println!("{tok:?} at turn {turn}: {} | {one} | {two} | {} | {}", mine.len(), fmt(mean_se(&mins)), fmt(mean_se(&work)));
        }
    }
    let _ = Level::Calm;
}

/// §7.7 The boosted crew: a *fascia* in the fuel outside the fire-facing
/// houses at increasing `line_x`, and the fallback (the crew defends homes
/// like an engine, no water, no road). Il Borgo with the wind holding, Le
/// Coste with it shifting, Il Mulino (never reached) as the control; crew sent
/// at turn 1 vs no orders.
#[test]
#[ignore]
fn s7_boosted_crew() {
    use demo::session::CrewMode;
    let dir = data_dir();
    let none: HashMap<u64, Counterfactual> = demo::turn_policy::counterfactuals(&dir, &seeds());
    let mut configs: Vec<(String, CrewMode, Variant)> = vec![];
    for x in [1.0f32, 4.0, 10.0, 20.0, 40.0] {
        let v = Variant { crews_defend: false, unit_effect: abm::suppression::UnitEffect { line_x: x, ..abm::suppression::UnitEffect::ONE }, ..demo::session::variant() };
        configs.push((format!("fascia line_x={x}"), CrewMode::Fascia, v));
    }
    configs.push(("defence (no water)".into(), CrewMode::Difesa, demo::session::variant()));
    println!("\n§7.7 boosted crew (N={N}), crew at turn 1. Δ homes in that district vs none (paired): Il Borgo, wind holds | Le Coste, shifts | Il Mulino, all | line cut (m) | minutes to start work");
    for (name, mode, v) in &configs {
        let r = grid(&seeds(), 3, |seed, d| {
            let draw = demo::draw(TOWN, seed).unwrap();
            let mut s = Session::with_variant(&dir, draw, *v).unwrap();
            s.crew_mode = *mode;
            let p = TurnPolicy::new("", at(1, vec![(TokenId::S, Sel::District(d))]));
            let mut started: Option<i64> = None;
            while !s.finished() {
                if !s.turn().finale() {
                    p.order(&mut s);
                }
                s.end_turn_observed(|x| {
                    if started.is_none() && x.run.crews.units[3].state == abm::suppression::UnitState::Working {
                        started = Some(x.time_s());
                    }
                })
                .unwrap();
            }
            let f = s.facts();
            (f.districts[d].1 as f32 - none[&seed].districts[d].1 as f32, s.run.crews.units[3].line_cut_m, started.map(|t| t as f32 / 60.0))
        });
        let pick = |d: usize, only: Option<bool>| {
            let v: Vec<f32> = r.iter().filter(|x| x.1 == d && only.map_or(true, |w| shifts(x.0) == w)).map(|x| x.2 .0).collect();
            mean_se(&v)
        };
        let line: Vec<f32> = r.iter().filter(|x| x.1 == BORGO).map(|x| x.2 .1).collect();
        let start: Vec<f32> = r.iter().filter(|x| x.1 == BORGO).filter_map(|x| x.2 .2).collect();
        println!(
            "{:22} {} | {} | {} | {} | {}",
            name,
            fmt(pick(BORGO, Some(false))),
            fmt(pick(COSTE, Some(true))),
            fmt(pick(demo::turn_policy::MULINO, None)),
            fmt(mean_se(&line)),
            fmt(mean_se(&start)),
        );
    }
}
