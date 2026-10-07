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
    Sky,
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

pub fn patrol_coste_borgo() -> TurnPolicy {
    TurnPolicy::new("patrol-coste-t1 + patrol-borgo-t2", |s| match s.turn().index {
        1 => vec![(TokenId::P, Sel::District(COSTE))],
        2 => vec![(TokenId::P, Sel::District(BORGO))],
        _ => vec![],
    })
}

/// Il Borgo, Le Coste, Il Mulino in turns 1-3: warns everyone, one stop a turn.
pub fn patrol_everyone() -> TurnPolicy {
    TurnPolicy::new("patrol-everyone", |s| match s.turn().index {
        1 => vec![(TokenId::P, Sel::District(BORGO))],
        2 => vec![(TokenId::P, Sel::District(COSTE))],
        3 => vec![(TokenId::P, Sel::District(MULINO))],
        _ => vec![],
    })
}

/// The district the opening forecast makes most likely to be hit first: Le
/// Coste when a shift is *probabile* (the wind would turn the fire onto it
/// before it reaches Il Borgo), else Il Borgo.
pub fn first_at_risk(s: &Session) -> usize {
    if s.draw.forecast(1).shift_p >= LIKELY_P {
        COSTE
    } else {
        BORGO
    }
}

fn other(d: usize) -> usize {
    if d == BORGO {
        COSTE
    } else {
        BORGO
    }
}

/// The forecast player's patrol, from turn `start`: the first district at
/// risk, then the other.
fn forecast_patrol(s: &Session, start: u8) -> Vec<(TokenId, Sel)> {
    let first = first_at_risk(s);
    match s.turn().index {
        t if t == start => vec![(TokenId::P, Sel::District(first))],
        t if t == start + 1 => vec![(TokenId::P, Sel::District(other(first)))],
        _ => vec![],
    }
}

/// The forecast player's patrol plan alone (lesson 3's baseline).
pub fn forecast_patrol_only() -> TurnPolicy {
    TurnPolicy::new("forecast-patrol", |s| forecast_patrol(s, 1))
}

/// A stop at Il Mulino first (nothing ever points the fire there), then the
/// forecast player's patrol plan one turn late (lesson 3).
pub fn patrol_mulino_first() -> TurnPolicy {
    TurnPolicy::new("patrol-mulino-first", |s| {
        if s.turn().index == 1 {
            vec![(TokenId::P, Sel::District(MULINO))]
        } else {
            forecast_patrol(s, 2)
        }
    })
}

pub fn crew_borgo_t1() -> TurnPolicy {
    TurnPolicy::new("crew-borgo-t1", |s| if on(1, s) { vec![(TokenId::S, Sel::District(BORGO))] } else { vec![] })
}

pub fn crew_mulino_t1() -> TurnPolicy {
    TurnPolicy::new("crew-mulino-t1", |s| if on(1, s) { vec![(TokenId::S, Sel::District(MULINO))] } else { vec![] })
}

/// Reads the wind and the forecast (gameplay §8): the patrol to the district
/// the forecast makes most likely to be hit first, then the other; the
/// Canadair called at turn 1 and dropping on the district the wind drives the
/// fire at; engines and crew on Il Borgo at turn 1, engines to Le Coste once
/// the wind has turned.
pub fn forecast_player() -> TurnPolicy {
    TurnPolicy::new("forecast-player", |s| {
        let mut p = forecast_patrol(s, 1);
        let at = if s.wind_turned() { COSTE } else { BORGO };
        if on(1, s) {
            p.extend([(TokenId::K, Sel::Sky), (TokenId::E1, Sel::District(BORGO)), (TokenId::E2, Sel::District(BORGO)), (TokenId::S, Sel::District(BORGO))]);
        }
        for e in [TokenId::E1, TokenId::E2, TokenId::E3] {
            let t = s.token(e);
            if t.orderable && t.doing != Some(TargetKind::District(at)) && !(on(1, s) && e != TokenId::E3) {
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
            let c = s.token(TokenId::S);
            if c.orderable && c.doing.is_none() {
                p.push((TokenId::S, Sel::District(d)));
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
    vec![
        none(),
        patrol_borgo_t1(),
        patrol_borgo_coste(),
        patrol_coste_borgo(),
        patrol_everyone(),
        patrol_mulino_first(),
        engines_borgo_t1(),
        engines_head(),
        crew_borgo_t1(),
        crew_mulino_t1(),
        forecast_player(),
        wait_and_see(),
    ]
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
