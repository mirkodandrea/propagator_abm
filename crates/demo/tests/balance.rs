//! Balance targets (`docs/demo-spec-gameplay.md` §8): the scripted commanders
//! of §8 on Rocca Ventosa, drawn seeds, played through `Session`.
//!
//! One grid, computed once per run of this file; one test per target. A
//! target the model does not meet is kept with its assertion intact and
//! `#[ignore]`d with the measured numbers in the reason -- it is reported to
//! the lead, not tuned away (gameplay §9: published numbers are not changed
//! to make a table come out). `table` prints everything.

use std::sync::OnceLock;

use demo::turn_policy::{balance_set, grid, mean_se, MULINO};
use demo::*;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

const N: u64 = 40;

#[derive(Debug, Clone)]
struct Rec {
    caught: f32,
    homes: f32,
    stamps: Vec<Stamp>,
}

/// (seed, policy) → record, policies in `balance_set()` order.
fn results() -> &'static Vec<Vec<Rec>> {
    static R: OnceLock<Vec<Vec<Rec>>> = OnceLock::new();
    R.get_or_init(|| {
        let ps = balance_set();
        let dir = data_dir();
        let seeds: Vec<u64> = (1..=N).collect();
        let flat = grid(&seeds, ps.len(), |seed, j| {
            let mut s = Session::new(&dir, seed).unwrap();
            ps[j].play(&mut s).unwrap();
            let f = s.facts();
            let stamps = (0..3).map(|d| s.stamp(d)).collect();
            Rec { caught: f.families_caught as f32, homes: f.homes_hit as f32, stamps }
        });
        let mut out = vec![vec![]; seeds.len()];
        for (seed, _, r) in flat {
            out[(seed - 1) as usize].push(r);
        }
        out
    })
}

fn idx(name: &str) -> usize {
    balance_set().iter().position(|p| p.name == name).unwrap()
}

fn col(j: usize, f: impl Fn(&Rec) -> f32) -> (f32, f32) {
    mean_se(&results().iter().map(|r| f(&r[j])).collect::<Vec<_>>())
}

/// Paired difference j - k per seed.
fn diff(j: usize, k: usize, f: impl Fn(&Rec) -> f32) -> (f32, f32) {
    mean_se(&results().iter().map(|r| f(&r[j]) - f(&r[k])).collect::<Vec<_>>())
}

fn fp() -> usize {
    idx("forecast-player")
}

#[test]
#[ignore]
fn table() {
    let ps = balance_set();
    println!("\n§8 balance (N={N}, drawn seeds). families caught | homes hit | Δ caught vs forecast-player | Δ homes vs forecast-player | Mulino AllarmeInutile | beats forecast-player on both");
    for (j, p) in ps.iter().enumerate() {
        let (c, h) = (col(j, |r| r.caught), col(j, |r| r.homes));
        let (dc, dh) = (diff(j, fp(), |r| r.caught), diff(j, fp(), |r| r.homes));
        let wolf = results().iter().filter(|r| r[j].stamps[MULINO] == Stamp::AllarmeInutile).count();
        let both = results().iter().filter(|r| r[j].caught < r[fp()].caught && r[j].homes < r[fp()].homes).count();
        println!(
            "{:34} {:5.1} ± {:3.1} | {:5.1} ± {:3.1} | {:+5.1} ± {:3.1} | {:+5.1} ± {:3.1} | {wolf:2}/{N} | {both:2}/{N}",
            p.name, c.0, c.1, h.0, h.1, dc.0, dc.1, dh.0, dh.1
        );
    }
}

/// `forecast-player` is best or tied on families: no policy's mean is below
/// it by more than two standard errors of the paired difference.
#[test]
#[ignore = "fails on Rocca Ventosa: it-alert-t1 and all-in catch 4.6 ± 0.4 families against forecast-player's 7.8 ± 0.6 (paired -3.2 ± 0.4); gameplay spec §8 table"]
fn forecast_player_is_best_or_tied_on_families() {
    for j in 0..balance_set().len() {
        let (d, se) = diff(j, fp(), |r| r.caught);
        assert!(d >= -2.0 * se.max(0.25), "{} catches {d:.1} ± {se:.1} fewer families than forecast-player", balance_set()[j].name);
    }
}

#[test]
fn forecast_player_is_best_or_tied_on_homes() {
    for j in 0..balance_set().len() {
        let (d, se) = diff(j, fp(), |r| r.homes);
        assert!(d >= -2.0 * se.max(0.5), "{} hits {d:.1} ± {se:.1} fewer homes than forecast-player", balance_set()[j].name);
    }
}

/// `all-in` (IT-alert) stamps Il Mulino *Allarme inutile* -- in every
/// session the fire does not come within the threat distance of it (all but
/// seed 15, where the warning was justified and stamps *In tempo*).
#[test]
fn all_in_loses_a_stamp() {
    let j = idx("all-in");
    let mut wolf = 0;
    for (k, r) in results().iter().enumerate() {
        let s = r[j].stamps[MULINO];
        assert!(matches!(s, Stamp::AllarmeInutile | Stamp::InTempo), "seed {}: {s:?}", k + 1);
        wolf += (s == Stamp::AllarmeInutile) as u64;
    }
    assert!(wolf * 10 >= N * 9, "all-in lost the stamp in only {wolf}/{N} sessions");
}

/// ... and ties `forecast-player` on families.
#[test]
#[ignore = "fails: all-in beats forecast-player on families (4.6 vs 7.8, paired -3.2 ± 0.4); the IT-alert costs nothing in the model but the stamp"]
fn all_in_ties_on_families() {
    let (d, se) = diff(idx("all-in"), fp(), |r| r.caught);
    assert!(d.abs() <= 2.0 * se.max(0.25), "all-in - forecast-player = {d:.1} ± {se:.1} families");
}

/// Engines at the head are worth what no engines are.
#[test]
fn engines_at_the_head_equal_no_orders_on_homes() {
    let (d, se) = diff(idx("engines-head"), idx("none"), |r| r.homes);
    assert!(d.abs() <= 2.0 * se.max(0.5), "engines-head - none = {d:.1} ± {se:.1} homes");
}

/// Waiting to see costs families.
#[test]
fn wait_and_see_loses_on_families() {
    let (d, se) = diff(idx("wait-and-see"), fp(), |r| r.caught);
    assert!(d > 2.0 * se, "wait-and-see - forecast-player = {d:.1} ± {se:.1} families caught");
}

/// No single policy beats `forecast-player` on both counts in more than a
/// third of seeds.
#[test]
fn nothing_dominates_the_forecast_player() {
    for (j, p) in balance_set().iter().enumerate() {
        let both = results().iter().filter(|r| r[j].caught < r[fp()].caught && r[j].homes < r[fp()].homes).count();
        assert!(both as u64 * 3 <= N, "{} beats forecast-player on both in {both}/{N} seeds", p.name);
    }
}
