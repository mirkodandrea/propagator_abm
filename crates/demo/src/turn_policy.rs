//! Scripted commanders for the turn game (`docs/demo-spec-gameplay.md` §0,
//! §8) and the grid that plays them on drawn seeds on all cores.
//!
//! A [`TurnPolicy`] is token → target assignments per turn, chosen by looking
//! only at what a player sees at the turn's opening (the tray, the targets,
//! the wind, the forecast, the districts). It plays through the same
//! [`Session`] methods the `play` binary and the kiosk call.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;

use crate::district::Level;
use crate::session::Session;
use crate::turn::*;

/// A target chosen by meaning, resolved against the turn's targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sel {
    District(usize),
    Head,
    Flank(Side),
    /// The oldest spot fire still burning.
    FirstSpot,
    Sky,
    Town,
}

pub type Plan = Vec<(TokenId, Sel)>;

#[derive(Clone)]
pub struct TurnPolicy {
    pub name: String,
    plan: Arc<dyn Fn(&Session) -> Plan + Send + Sync>,
}

impl TurnPolicy {
    pub fn new(name: impl Into<String>, plan: impl Fn(&Session) -> Plan + Send + Sync + 'static) -> TurnPolicy {
        TurnPolicy { name: name.into(), plan: Arc::new(plan) }
    }

    /// Orders for the turn that is open now.
    pub fn plan(&self, s: &Session) -> Plan {
        (self.plan)(s)
    }

    /// Give this turn's orders (those that resolve and are accepted).
    pub fn order(&self, s: &mut Session) {
        for (t, sel) in self.plan(s) {
            if let Some(id) = resolve(s, sel) {
                let _ = s.assign(t, id);
            }
        }
    }

    /// Play a whole session: orders each turn, then *Avanti*, to the end.
    pub fn play(&self, s: &mut Session) -> Result<()> {
        while !s.finished() {
            if !s.turn().finale() {
                self.order(s);
            }
            s.end_turn()?;
        }
        Ok(())
    }

    /// Play turns until `turn` is open (orders given on each turn before it).
    pub fn play_until(&self, s: &mut Session, turn: u8) -> Result<()> {
        while !s.finished() && s.turn().index < turn {
            self.order(s);
            s.end_turn()?;
        }
        Ok(())
    }
}

/// The target `sel` means this turn, if there is one.
pub fn resolve(s: &Session, sel: Sel) -> Option<TargetId> {
    let kind = match sel {
        Sel::District(d) => TargetKind::District(d),
        Sel::Head => TargetKind::Head,
        Sel::Flank(side) => TargetKind::Flank(side),
        Sel::Sky => TargetKind::Sky,
        Sel::Town => TargetKind::Town,
        Sel::FirstSpot => {
            return s.targets().iter().filter(|t| matches!(t.kind, TargetKind::SpotFire(_))).min_by_key(|t| t.id).map(|t| t.id)
        }
    };
    s.target_of(kind).map(|t| t.id)
}

fn on(turn: u8, s: &Session) -> bool {
    s.turn().index == turn
}

fn free(s: &Session, t: TokenId) -> bool {
    s.in_tray(t) && s.token(t).state == TokenState::Libero
}

/// Il Borgo, Le Coste, Il Mulino (the scenario's order).
pub const BORGO: usize = 0;
pub const COSTE: usize = 1;
pub const MULINO: usize = 2;

/// The §8 policies, by name.
pub fn none() -> TurnPolicy {
    TurnPolicy::new("none", |_| vec![])
}

pub fn patrol_borgo_t1() -> TurnPolicy {
    TurnPolicy::new("patrol-borgo-t1", |s| if on(1, s) { vec![(TokenId::P, Sel::District(BORGO))] } else { vec![] })
}

pub fn it_alert_t1() -> TurnPolicy {
    TurnPolicy::new("it-alert-t1", |s| if on(1, s) { vec![(TokenId::I, Sel::Town)] } else { vec![] })
}

pub fn patrol_borgo_coste() -> TurnPolicy {
    TurnPolicy::new("patrol-borgo-t1 + patrol-coste-t2", |s| match s.turn().index {
        1 => vec![(TokenId::P, Sel::District(BORGO))],
        2 => vec![(TokenId::P, Sel::District(COSTE))],
        _ => vec![],
    })
}

/// Every engine to Il Borgo's edge as soon as it is in the tray.
pub fn engines_borgo_t1() -> TurnPolicy {
    TurnPolicy::new("engines-borgo-t1", |s| {
        let mut p = vec![];
        if on(1, s) {
            p.push((TokenId::E1, Sel::District(BORGO)));
            p.push((TokenId::E2, Sel::District(BORGO)));
        }
        if on(REINFORCEMENT_TURN, s) {
            p.push((TokenId::E3, Sel::District(BORGO)));
        }
        p
    })
}

/// Engines at the head whenever they are free (the wrong use).
pub fn engines_head() -> TurnPolicy {
    TurnPolicy::new("engines-head", |s| {
        [TokenId::E1, TokenId::E2, TokenId::E3].into_iter().filter(|t| free(s, *t)).map(|t| (t, Sel::Head)).collect()
    })
}

pub fn all_in() -> TurnPolicy {
    TurnPolicy::new("all-in", |s| {
        let mut p = vec![];
        if on(1, s) {
            p.extend([(TokenId::I, Sel::Town), (TokenId::E1, Sel::District(BORGO)), (TokenId::E2, Sel::District(BORGO)), (TokenId::K, Sel::Sky)]);
        }
        if on(REINFORCEMENT_TURN, s) {
            p.push((TokenId::E3, Sel::District(BORGO)));
        }
        p
    })
}

/// Reads the wind and the forecast (gameplay §8): patrol to Il Borgo and the
/// Canadair called at turn 1, engines on Il Borgo, then on Le Coste once the
/// wind has turned; the patrol to Le Coste at turn 2 if issue 2 says likely.
/// The plane drops on whichever district the wind now drives the fire at.
pub fn forecast_player() -> TurnPolicy {
    TurnPolicy::new("forecast-player", |s| {
        let mut p = vec![];
        let at = if s.wind_turned() { COSTE } else { BORGO };
        if on(1, s) {
            p.extend([(TokenId::P, Sel::District(BORGO)), (TokenId::K, Sel::Sky), (TokenId::E1, Sel::District(BORGO)), (TokenId::E2, Sel::District(BORGO))]);
        }
        if on(2, s) && s.forecast().shift_p >= LIKELY_P {
            p.push((TokenId::P, Sel::District(COSTE)));
        }
        if s.wind_turned() && free(s, TokenId::P) && !s.districts()[COSTE].warned {
            p.push((TokenId::P, Sel::District(COSTE)));
        }
        for e in [TokenId::E1, TokenId::E2, TokenId::E3] {
            let t = s.token(e);
            if !t.orderable {
                continue;
            }
            if t.doing != Some(TargetKind::District(at)) {
                p.push((e, Sel::District(at)));
            }
        }
        let k = s.token(TokenId::K);
        if k.orderable && k.state != TokenState::NonChiamato && k.doing != Some(TargetKind::District(at)) {
            p.push((TokenId::K, Sel::District(at)));
        }
        p
    })
}

/// Waits to see: warns a district once the fire threatens it, sends the
/// engines there, calls the plane when the first district is threatened.
pub fn wait_and_see() -> TurnPolicy {
    TurnPolicy::new("wait-and-see", |s| {
        let mut p = vec![];
        let ds = s.districts();
        let hot: Vec<usize> = ds.iter().filter(|d| d.level >= Level::Threatened).map(|d| d.district).collect();
        if let Some(&d) = hot.iter().find(|&&d| !ds[d].warned) {
            if free(s, TokenId::P) {
                p.push((TokenId::P, Sel::District(d)));
            }
        }
        if let Some(&d) = hot.first() {
            for e in [TokenId::E1, TokenId::E2, TokenId::E3] {
                let t = s.token(e);
                if t.orderable && t.doing.is_none() {
                    p.push((e, Sel::District(d)));
                }
            }
            let k = s.token(TokenId::K);
            if k.state == TokenState::NonChiamato {
                p.push((TokenId::K, Sel::Sky));
            } else if k.orderable && k.doing.is_none() {
                p.push((TokenId::K, Sel::District(d)));
            }
        }
        p
    })
}

/// The balance set (gameplay §8), in a fixed order.
pub fn balance_set() -> Vec<TurnPolicy> {
    vec![none(), patrol_borgo_t1(), it_alert_t1(), patrol_borgo_coste(), engines_borgo_t1(), engines_head(), all_in(), forecast_player(), wait_and_see()]
}

/// Run `f(seed, job)` for every seed × job on all cores. Results in input order.
pub fn grid<R: Send>(seeds: &[u64], jobs: usize, f: impl Fn(u64, usize) -> R + Sync) -> Vec<(u64, usize, R)> {
    let all: Vec<(u64, usize)> = seeds.iter().flat_map(|&s| (0..jobs).map(move |j| (s, j))).collect();
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(all.len().max(1));
    let mut out: Vec<Option<(u64, usize, R)>> = (0..all.len()).map(|_| None).collect();
    std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|_| {
                sc.spawn(|| {
                    let mut mine = vec![];
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= all.len() {
                            break;
                        }
                        let (seed, j) = all[i];
                        mine.push((i, (seed, j, f(seed, j))));
                    }
                    mine
                })
            })
            .collect();
        for h in hs {
            for (i, r) in h.join().expect("grid worker") {
                out[i] = Some(r);
            }
        }
    });
    out.into_iter().map(|r| r.expect("every job ran")).collect()
}

/// One counterfactual per seed, in parallel.
pub fn counterfactuals(data_dir: &Path, seeds: &[u64]) -> std::collections::HashMap<u64, Counterfactual> {
    grid(seeds, 1, |seed, _| {
        let d = crate::weather::draw(crate::session::TOWN, seed).expect("demo town");
        Session::counterfactual_of(data_dir, d).expect("counterfactual")
    })
    .into_iter()
    .map(|(s, _, c)| (s, c))
    .collect()
}

/// Mean and standard error.
pub fn mean_se(xs: &[f32]) -> (f32, f32) {
    if xs.is_empty() {
        return (0.0, 0.0);
    }
    let n = xs.len() as f32;
    let m = xs.iter().sum::<f32>() / n;
    let var = if xs.len() > 1 { xs.iter().map(|x| (x - m).powi(2)).sum::<f32>() / (n - 1.0) } else { 0.0 };
    (m, (var / n).sqrt())
}
