//! The A/B harness (`docs/demo-spec-gameplay.md` §0): policies x variants x
//! towns x seeds, run on all cores, summarised as mean, spread and regret.
//!
//! Every session is a *drawn* one (`weather::draw`), because that is what a
//! visitor plays, and the draw is shared by every policy and variant on a given
//! (town, seed) -- so any difference in a row is the policy's or the variant's.

use std::path::Path;

use crate::policy::Policy;
use crate::run::{Outcome, Run, Variant};
use crate::weather::draw_with;

#[derive(Debug, Clone)]
pub struct Record {
    pub town: &'static str,
    pub policy: usize,
    pub variant: usize,
    pub seed: u64,
    pub out: Outcome,
    /// Money spent by the end of the mission (cost::Log::price).
    pub eur: f32,
}

/// Run every combination. Order of the result is unspecified; use the indices.
pub fn run_grid(
    data_dir: &Path,
    towns: &[&'static str],
    policies: &[Policy],
    variants: &[Variant],
    seeds: std::ops::RangeInclusive<u64>,
) -> Vec<Record> {
    let mut jobs = vec![];
    for &town in towns {
        for seed in seeds.clone() {
            for pi in 0..policies.len() {
                for vi in 0..variants.len() {
                    jobs.push((town, seed, pi, vi));
                }
            }
        }
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(12);
    let chunk = jobs.len().div_ceil(threads).max(1);
    let mut all = vec![];
    std::thread::scope(|s| {
        let hs: Vec<_> = jobs
            .chunks(chunk)
            .map(|c| {
                s.spawn(move || {
                    c.iter()
                        .map(|&(town, seed, pi, vi)| {
                            let d = draw_with(town, seed, variants[vi].shift_p).expect("demo town");
                            let mut run = Run::with_variant(data_dir, d.spec, seed, variants[vi]).expect("run");
                            let out = policies[pi].play(&mut run, &d).expect("play");
                            Record { town, policy: pi, variant: vi, seed, out, eur: run.ledger().total_eur() }
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in hs {
            all.extend(h.join().expect("sweep worker"));
        }
    });
    all
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Stat {
    pub mean: f32,
    pub sd: f32,
}

pub fn stat(xs: impl IntoIterator<Item = f32>) -> Stat {
    let v: Vec<f32> = xs.into_iter().collect();
    if v.is_empty() {
        return Stat::default();
    }
    let m = v.iter().sum::<f32>() / v.len() as f32;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f32>() / v.len() as f32;
    Stat { mean: m, sd: var.sqrt() }
}

/// What a row of a table says about one (town, policy, variant).
#[derive(Debug, Clone, Copy)]
pub struct Summary {
    pub secure: Stat,
    pub caught: Stat,
    pub homes_lost: Stat,
    pub hectares: Stat,
    pub n: usize,
}

pub fn summarise(rs: &[Record], town: &str, policy: usize, variant: usize) -> Summary {
    let sel: Vec<&Record> = rs.iter().filter(|r| r.town == town && r.policy == policy && r.variant == variant).collect();
    Summary {
        secure: stat(sel.iter().map(|r| r.out.secure() as f32)),
        caught: stat(sel.iter().map(|r| r.out.caught as f32)),
        homes_lost: stat(sel.iter().map(|r| r.out.homes_lost as f32)),
        hectares: stat(sel.iter().map(|r| r.out.hectares)),
        n: sel.len(),
    }
}

/// Paired difference `(policy,variant) - (base_policy,base_variant)` on one
/// metric, per seed, as mean and standard error. The draw and the seed are shared,
/// so this is the effect of the policy/variant itself, without the seed-to-seed
/// spread (sd ~ 20 families) that swamps an unpaired comparison.
pub fn paired(
    rs: &[Record],
    town: &str,
    (policy, variant): (usize, usize),
    (bp, bv): (usize, usize),
    metric: impl Fn(&Outcome) -> f32,
) -> (f32, f32) {
    let mut d = vec![];
    for r in rs.iter().filter(|r| r.town == town && r.policy == policy && r.variant == variant) {
        if let Some(b) = rs.iter().find(|o| o.town == town && o.seed == r.seed && o.policy == bp && o.variant == bv) {
            d.push(metric(&r.out) - metric(&b.out));
        }
    }
    let s = stat(d.iter().copied());
    (s.mean, s.sd / (d.len().max(1) as f32).sqrt())
}

/// Regret on an arbitrary per-record loss (lower is better): on each (town, seed,
/// variant), this policy's loss minus the best any policy in the grid achieved,
/// averaged over seeds; and the share of seeds on which it was (jointly) best.
pub fn regret_by(
    rs: &[Record],
    town: &str,
    policy: usize,
    variant: usize,
    loss: impl Fn(&Record) -> f32,
) -> (f32, f32) {
    let (mut sum, mut n, mut wins) = (0.0, 0usize, 0usize);
    for r in rs.iter().filter(|r| r.town == town && r.policy == policy && r.variant == variant) {
        let best = rs
            .iter()
            .filter(|o| o.town == town && o.seed == r.seed && o.variant == variant)
            .map(&loss)
            .fold(f32::INFINITY, f32::min);
        sum += loss(r) - best;
        n += 1;
        if loss(r) <= best + 1e-3 {
            wins += 1;
        }
    }
    let n = n.max(1) as f32;
    (sum / n, wins as f32 / n)
}

/// Regret on `secure` families: on each (town, seed, variant), the best any
/// policy in the grid achieved minus this one's, averaged over seeds. Also the
/// share of seeds on which this policy was (jointly) the best.
pub fn regret(rs: &[Record], town: &str, policy: usize, variant: usize) -> (f32, f32) {
    let mut reg = vec![];
    let mut wins = 0usize;
    for r in rs.iter().filter(|r| r.town == town && r.policy == policy && r.variant == variant) {
        let best = rs
            .iter()
            .filter(|o| o.town == town && o.seed == r.seed && o.variant == variant)
            .map(|o| o.out.secure())
            .max()
            .unwrap_or(0);
        reg.push((best - r.out.secure().min(best)) as f32);
        if r.out.secure() >= best {
            wins += 1;
        }
    }
    let n = reg.len().max(1) as f32;
    (reg.iter().sum::<f32>() / n, wins as f32 / n)
}
