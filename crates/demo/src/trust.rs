//! Cry-wolf (`docs/demo-spec-gameplay.md` 5.2): an order the fire never justified
//! costs the commander the town's willingness to obey the *next* one.
//!
//! Mechanism, deliberately small. When households are ordered out, the order is
//! remembered. [`JUDGE_AFTER_S`] later it is judged against what the fire actually
//! did: if most of the households it moved never had fire within
//! [`NEEDLESS_RADIUS_M`] of their home in that time, the order was **needless**,
//! and every household not yet ordered loses `delta x needless_share` of
//! `trust_authority` (the town hears that the last one was a false alarm). The
//! shipped compliance gate (`block.order_response`, trust > 0.35) then does the
//! rest: fewer households comply with the next order, which is the model's own
//! mechanism and not a new one.
//!
//! Needless is judged on **hindsight after a delay**, from the fire state, not from
//! a twin and not from a forecast: the commander is judged by what the next half
//! hour showed, the way a town would. (Whether a forward estimate would disagree is
//! an open question, spec 8.)
//!
//! **Provably inert** (finding 34): a [`CryWolf`] only exists when a variant turns
//! it on; with it off nothing here runs and no household trait is touched, and
//! with it on but no order given there is nothing to judge, so no trait moves
//! (`tests/trust.rs`).

use abm::Abm;
use fire::FireSim;

/// How long after an order it is judged.
pub const JUDGE_AFTER_S: i64 = 30 * 60;
/// Fire this close to a home, at any point before the judgment, justifies having
/// moved that household.
pub const NEEDLESS_RADIUS_M: f32 = 800.0;
/// An order is needless when more than this share of the households it moved were
/// never threatened.
pub const NEEDLESS_SHARE: f32 = 0.5;
/// A household has lost faith when its trust has fallen by at least this much.
pub const ANGRY_DROP: f32 = 0.10;

/// What the HUD shows (contract, spec 1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Trust {
    /// Mean `trust_authority` over the town, 0-1.
    pub mean: f32,
    /// Households whose trust has fallen by [`ANGRY_DROP`] or more since the start.
    pub angry_households: usize,
}

struct Pending {
    at_s: i64,
    ordered: Vec<usize>,
    /// Whether any fire has come within the radius of each ordered household.
    threatened: Vec<bool>,
    judged: bool,
}

pub struct CryWolf {
    delta: f32,
    base: Vec<f32>,
    pending: Vec<Pending>,
    /// How many orders have been judged needless (for the end card / a test).
    pub needless_orders: usize,
}

impl CryWolf {
    /// `delta`: the most trust a fully needless order costs every household still
    /// to be ordered (0-1; the trust bake has median 0.70 and the gate sits at 0.35).
    pub fn new(agents: &Abm, delta: f32) -> CryWolf {
        CryWolf { delta, base: agents.households.iter().map(|h| h.trust_authority).collect(), pending: vec![], needless_orders: 0 }
    }

    /// Remember an order that has just moved these households.
    pub fn note_order(&mut self, at_s: i64, newly_ordered: Vec<usize>) {
        if !newly_ordered.is_empty() {
            let n = newly_ordered.len();
            self.pending.push(Pending { at_s, ordered: newly_ordered, threatened: vec![false; n], judged: false });
        }
    }

    /// Call once after each step.
    pub fn step(&mut self, agents: &mut Abm, fire: &FireSim, world: &scenario::World) {
        use fire::CellFire;
        let now = fire.time_s();
        let state = fire.state();
        let mut penalty = 0.0f32;
        for p in self.pending.iter_mut().filter(|p| !p.judged) {
            for (k, &i) in p.ordered.iter().enumerate() {
                if !p.threatened[k] {
                    let home = agents.households[i].home;
                    p.threatened[k] = fire::cells_in_radius(world, home, NEEDLESS_RADIUS_M)
                        .iter()
                        .any(|c| state[c.row * world.fire_cols + c.col] != CellFire::Unburnt);
                }
            }
            if now >= p.at_s + JUDGE_AFTER_S {
                p.judged = true;
                let needless = p.threatened.iter().filter(|t| !**t).count() as f32 / p.threatened.len() as f32;
                if needless > NEEDLESS_SHARE {
                    self.needless_orders += 1;
                    penalty += self.delta * needless;
                }
            }
        }
        if penalty > 0.0 {
            for h in agents.households.iter_mut().filter(|h| !h.ordered) {
                h.trust_authority = (h.trust_authority - penalty).max(0.0);
            }
        }
    }

    pub fn trust(&self, agents: &Abm) -> Trust {
        let n = agents.households.len().max(1) as f32;
        Trust {
            mean: agents.households.iter().map(|h| h.trust_authority).sum::<f32>() / n,
            angry_households: agents
                .households
                .iter()
                .zip(&self.base)
                .filter(|(h, b)| **b - h.trust_authority >= ANGRY_DROP)
                .count(),
        }
    }
}
