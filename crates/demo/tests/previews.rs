//! The screen must say what the model will do (gameplay §4): every preview
//! checked against what the order then does, on seeds 1-10, every turn,
//! every target an engine can be given. Only ground units can be *Lontano*
//! or *Ritirata*; engines and the crew are both checked.
//!
//! * *Lontano* ⇔ the unit never works there: a district's engine defends none
//!   of its homes; elsewhere it never pumps within hose reach of the point.
//! * *Difende N case* = the homes the verdict credits the moment it starts
//!   work (`Tally::defended_now`, with no other unit about).
//! * *Ritirata* ⇒ it pulls back within the turn.

use abm::suppression::{UnitState, ENGINE_REACH_M};
use demo::turn_policy::grid;
use demo::*;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

#[derive(Debug)]
#[allow(dead_code)] // seed and turn are read through Debug in failure messages
struct Case {
    seed: u64,
    turn: u8,
    token: TokenId,
    target: TargetKind,
    effect: Effect,
    /// Homes credited at the first step anything was defended (districts).
    first_defended: Option<u32>,
    /// Pumped within hose reach of the point (non-districts).
    pumped_there: bool,
    withdrew_in_turn: bool,
    withdrew_or_lost: bool,
}

fn cases() -> Vec<Case> {
    let dir = data_dir();
    let seeds: Vec<u64> = (1..=10).collect();
    // job = (turn - 1) * 2 + (0: engine E1, 1: crew S); targets inside.
    grid(&seeds, 10, |seed, job| {
        let turn = (job / 2) as u8 + 1;
        let token = if job % 2 == 0 { TokenId::E1 } else { TokenId::S };
        let mut out = vec![];
        let probe = {
            let mut s = Session::new(&dir, seed).unwrap();
            while s.turn().index < turn {
                s.end_turn().unwrap();
            }
            s.valid_targets(token).into_iter().map(|t| (t.kind, s.preview(token, t.id).unwrap().effect)).collect::<Vec<_>>()
        };
        for (kind, effect) in probe {
            let mut s = Session::new(&dir, seed).unwrap();
            while s.turn().index < turn {
                s.end_turn().unwrap();
            }
            let tg = *s.target_of(kind).unwrap();
            assert_eq!(s.preview(token, tg.id).unwrap().effect, effect, "replay is deterministic");
            s.assign(token, tg.id).unwrap();
            let k = token.unit().unwrap();
            let district = if let TargetKind::District(d) = kind { Some(s.run.referee.districts[d].households.clone()) } else { None };
            let (mut first_defended, mut pumped_there, mut withdrew_in_turn, mut withdrew_or_lost) = (None, false, false, false);
            let mut used = 0.0f32;
            for t in 0..2 {
                if s.finished() {
                    break;
                }
                s.end_turn_observed(|x| {
                    let now = x.time_s();
                    let u = &x.run.crews.units[k];
                    if matches!(u.state, UnitState::Withdrawing | UnitState::Lost) {
                        withdrew_or_lost = true;
                        withdrew_in_turn |= t == 0;
                    }
                    if let Some(hs) = &district {
                        if first_defended.is_none() && hs.iter().any(|&i| x.run.referee.tally.defended(i, now)) {
                            first_defended = Some(x.run.referee.tally.defended_now(now) as u32);
                        }
                    } else if u.water_used_l + u.line_cut_m > used {
                        let p = tg.pos;
                        if ((u.pos.x - p.x).powi(2) + (u.pos.y - p.y).powi(2)).sqrt() <= ENGINE_REACH_M + 1.0 {
                            pumped_there = true;
                        }
                    }
                    used = u.water_used_l + u.line_cut_m;
                })
                .unwrap();
            }
            out.push(Case { seed, turn, token, target: kind, effect, first_defended, pumped_there, withdrew_in_turn, withdrew_or_lost });
        }
        out
    })
    .into_iter()
    .flat_map(|(_, _, v)| v)
    .collect()
}

#[test]
fn previews_say_what_the_unit_will_do() {
    let cs = cases();
    let mut bad = vec![];
    for c in &cs {
        let works = match c.target {
            TargetKind::District(_) => c.first_defended.is_some(),
            _ => c.pumped_there,
        };
        let engine = c.token.kind() == TokenKind::Autobotte;
        match c.effect {
            Effect::Lontano if works => bad.push(format!("{c:?}: Lontano, but it worked there")),
            Effect::Difende { homes } if c.first_defended != Some(homes) && !c.withdrew_or_lost => {
                bad.push(format!("{c:?}: promised {homes} homes, credited {:?}", c.first_defended))
            }
            Effect::Ritirata if !c.withdrew_in_turn => bad.push(format!("{c:?}: Ritirata, but it did not pull back within the turn")),
            // A district engine that is not Lontano and not pointless must defend.
            Effect::Difende { .. } if !works && !c.withdrew_or_lost => bad.push(format!("{c:?}: promised to defend, defended nothing")),
            _ => {}
        }
        if !engine {
            assert!(!matches!(c.effect, Effect::Difende { .. }), "{c:?}");
        }
    }
    let n = cs.len();
    let lontano = cs.iter().filter(|c| c.effect == Effect::Lontano).count();
    let ritirata = cs.iter().filter(|c| c.effect == Effect::Ritirata).count();
    let difende = cs.iter().filter(|c| matches!(c.effect, Effect::Difende { .. })).count();
    println!("{n} orders checked: {difende} Difende, {lontano} Lontano, {ritirata} Ritirata");
    assert!(bad.is_empty(), "{} of {n} previews disagree with the model:\n{}", bad.len(), bad.join("\n"));
    assert!(difende > 0 && n > 100, "the check covered too little: {n} orders, {difende} Difende");
}

/// Il Borgo, the order that saves most homes, is never shown as out of reach
/// on the seeds a playtester will meet first.
#[test]
fn il_borgo_is_reachable_by_engine_at_turn_one() {
    for seed in 1..=10 {
        let s = Session::new(&data_dir(), seed).unwrap();
        let b = s.target_of(TargetKind::District(0)).unwrap().id;
        let e = s.preview(TokenId::E1, b).unwrap().effect;
        assert!(matches!(e, Effect::Difende { homes } if homes > 0), "seed {seed}: {e:?}");
    }
}

/// The head never promises a withdrawal it will not deliver: an engine or
/// crew sent there at turn 2 is shown as working and saving no homes (or as
/// out of reach), the way milestone 0 measured it (§7.6).
#[test]
fn the_head_says_it_saves_no_homes() {
    for seed in 1..=10 {
        let mut s = Session::new(&data_dir(), seed).unwrap();
        s.end_turn().unwrap();
        let h = s.target_of(TargetKind::Head).unwrap().id;
        for t in [TokenId::E1, TokenId::S] {
            let e = s.preview(t, h).unwrap().effect;
            assert!(matches!(e, Effect::NienteCase | Effect::Lontano), "seed {seed} {t:?}: {e:?}");
        }
    }
}
