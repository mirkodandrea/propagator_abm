//! The screen must say what the model will do (gameplay §4, "Previews must
//! be true"): every preview checked against what the order then does, on
//! seeds 1-10, every turn, every token and every target it can be given.
//!
//! * *Lontano* ⇔ the unit never works there: a district's engine defends none
//!   of its homes; elsewhere it never works within hose reach of the point.
//! * *Difende N* / *Fascia N* / *Bagna N* = the homes the verdict credits the
//!   moment the unit starts work there (`Tally::defended_now`, nothing else
//!   about).
//! * *Avvisa N* = the families the patrol warns when it arrives.
//! * *Ritirata* ⇒ it pulls back within the turn.

use abm::suppression::{UnitState, ENGINE_REACH_M};
use demo::turn_policy::grid;
use demo::*;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

const TOKENS: [TokenId; 6] = TokenId::ALL;

#[derive(Debug)]
#[allow(dead_code)] // seed and turn are read through Debug in failure messages
struct Case {
    seed: u64,
    turn: u8,
    token: TokenId,
    target: TargetKind,
    effect: Effect,
    /// Homes credited at the first step anything was defended.
    first_defended: Option<u32>,
    /// Worked within hose reach of the point (head and flanks).
    worked_there: bool,
    /// Families the patrol warned on arrival.
    warned: Option<u32>,
    withdrew_in_turn: bool,
    withdrew_or_lost: bool,
}

/// The session at the opening of `turn`, with no orders except the Canadair
/// call at turn 1 (so the plane is on station for turns 4 and 5).
fn at_turn(seed: u64, turn: u8) -> Session {
    let mut s = Session::new(&data_dir(), seed).unwrap();
    let sky = s.target_of(TargetKind::Sky).unwrap().id;
    s.assign(TokenId::K, sky).unwrap();
    while s.turn().index < turn {
        s.end_turn().unwrap();
    }
    s
}

fn cases() -> Vec<Case> {
    let seeds: Vec<u64> = (1..=10).collect();
    grid(&seeds, 5 * TOKENS.len(), |seed, job| {
        let turn = (job / TOKENS.len()) as u8 + 1;
        let token = TOKENS[job % TOKENS.len()];
        let mut out = vec![];
        let probe: Vec<(TargetKind, Effect)> = {
            let s = at_turn(seed, turn);
            if !s.in_tray(token) || !s.token(token).orderable {
                return out;
            }
            s.valid_targets(token).into_iter().filter(|t| t.kind != TargetKind::Sky).map(|t| (t.kind, s.preview(token, t.id).unwrap().effect)).collect()
        };
        for (kind, effect) in probe {
            let mut s = at_turn(seed, turn);
            let tg = *s.target_of(kind).unwrap();
            assert_eq!(s.preview(token, tg.id).unwrap().effect, effect, "replay is deterministic");
            s.assign(token, tg.id).unwrap();
            let k = token.unit();
            let homes: Vec<usize> = match kind {
                TargetKind::District(d) => s.run.referee.districts[d].households.clone(),
                _ => vec![],
            };
            let mut c = Case { seed, turn, token, target: kind, effect, first_defended: None, worked_there: false, warned: None, withdrew_in_turn: false, withdrew_or_lost: false };
            let mut used = 0.0f32;
            // To the end: a crew walking to Le Coste takes over two turns.
            for t in 0..8 {
                if s.finished() {
                    break;
                }
                let before = s.run.agents.households.iter().filter(|h| h.ordered).count();
                s.end_turn_observed(|x| {
                    let now = x.time_s();
                    if c.first_defended.is_none() && homes.iter().any(|&i| x.run.referee.tally.defended(i, now)) {
                        c.first_defended = Some(x.run.referee.tally.defended_now(now) as u32);
                    }
                    let Some(k) = k else { return };
                    let u = &x.run.crews.units[k];
                    let broke_off = token == TokenId::K && u.state == UnitState::Staged && u.note.contains("broke off");
                    if matches!(u.state, UnitState::Withdrawing | UnitState::Lost) || broke_off {
                        c.withdrew_or_lost = true;
                        c.withdrew_in_turn |= t == 0;
                    }
                    if u.water_used_l + u.line_cut_m > used {
                        let p = tg.pos;
                        if ((u.pos.x - p.x).powi(2) + (u.pos.y - p.y).powi(2)).sqrt() <= ENGINE_REACH_M + 1.0 {
                            c.worked_there = true;
                        }
                    }
                    used = u.water_used_l + u.line_cut_m;
                })
                .unwrap();
                if token == TokenId::P && t == 0 {
                    c.warned = Some((s.run.agents.households.iter().filter(|h| h.ordered).count() - before) as u32);
                }
            }
            out.push(c);
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
            _ => c.worked_there,
        };
        let excused = c.withdrew_or_lost;
        match c.effect {
            Effect::Lontano if works => bad.push(format!("{c:?}: Lontano, but it worked there")),
            Effect::Difende { homes } | Effect::Fascia { homes } | Effect::Bagna { homes } if homes > 0 && c.first_defended != Some(homes) && !excused => {
                bad.push(format!("{c:?}: promised {homes} homes, credited {:?}", c.first_defended))
            }
            Effect::Avvisa { families } if c.warned != Some(families) => bad.push(format!("{c:?}: promised to warn {families}, warned {:?}", c.warned)),
            Effect::Ritirata if !c.withdrew_in_turn => bad.push(format!("{c:?}: Ritirata, but it did not pull back within the turn")),
            _ => {}
        }
    }
    let n = cs.len();
    let count = |f: &dyn Fn(&Effect) -> bool| cs.iter().filter(|c| f(&c.effect)).count();
    println!(
        "{n} orders checked: {} Avvisa, {} Difende, {} Fascia, {} Bagna, {} NonSalvaCase, {} Lontano, {} Ritirata, {} Inutile",
        count(&|e| matches!(e, Effect::Avvisa { .. })),
        count(&|e| matches!(e, Effect::Difende { .. })),
        count(&|e| matches!(e, Effect::Fascia { .. })),
        count(&|e| matches!(e, Effect::Bagna { .. })),
        count(&|e| *e == Effect::NonSalvaCase),
        count(&|e| *e == Effect::Lontano),
        count(&|e| *e == Effect::Ritirata),
        count(&|e| *e == Effect::Inutile),
    );
    assert!(bad.is_empty(), "{} of {n} previews disagree with the model:\n{}", bad.len(), bad.join("\n"));
    for kind in ["Avvisa", "Difende", "Fascia", "Bagna"] {
        assert!(cs.iter().any(|c| format!("{:?}", c.effect).starts_with(kind)), "no {kind} preview was checked");
    }
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

/// The head never promises a withdrawal it will not deliver: ground units
/// and the plane sent there at turn 2 are shown as saving no homes (or as out
/// of reach), the way milestone 0 measured them (§7.6).
#[test]
fn the_head_says_it_saves_no_homes() {
    for seed in 1..=10 {
        let mut s = Session::new(&data_dir(), seed).unwrap();
        s.end_turn().unwrap();
        let h = s.target_of(TargetKind::Head).unwrap().id;
        for t in [TokenId::E1, TokenId::S] {
            let e = s.preview(t, h).unwrap().effect;
            assert!(matches!(e, Effect::NonSalvaCase | Effect::Lontano), "seed {seed} {t:?}: {e:?}");
        }
    }
}
