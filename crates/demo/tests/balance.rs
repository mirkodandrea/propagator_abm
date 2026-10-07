//! Balance targets (`docs/demo-spec-gameplay.md` §8): the scripted commanders
//! of §8 on Rocca Ventosa, drawn seeds, played through `Session`.
//!
//! One grid, computed once per run of this file; one test per target. A
//! target the model does not meet keeps its assertion and is `#[ignore]`d
//! with the measured numbers -- reported to the lead, not tuned away
//! (gameplay §9). `table` prints everything, including whether Il Borgo or
//! Le Coste first is right depending on the forecast.

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

/// [seed-1][policy], policies in `balance_set()` order.
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
            let none = Session::counterfactual_of(&dir, s.draw).unwrap();
            let stamps = s.verdict_against(none).districts.iter().map(|d| d.people).collect();
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

/// A stamp that marks a mistake.
fn bad(s: Stamp) -> bool {
    !s.good()
}

#[test]
#[ignore]
fn table() {
    let ps = balance_set();
    println!("\n§8 balance (N={N}, drawn seeds). families caught | homes hit | Δ caught vs forecast-player | Δ homes | sessions with a bad stamp | Mulino AllarmeInutile | beats forecast-player on both");
    for (j, p) in ps.iter().enumerate() {
        let (c, h) = (col(j, |r| r.caught), col(j, |r| r.homes));
        let (dc, dh) = (diff(j, fp(), |r| r.caught), diff(j, fp(), |r| r.homes));
        let stamp = results().iter().filter(|r| r[j].stamps.iter().any(|s| bad(*s))).count();
        let wolf = results().iter().filter(|r| r[j].stamps[MULINO] == Stamp::AllarmeInutile).count();
        let both = results().iter().filter(|r| r[j].caught < r[fp()].caught && r[j].homes < r[fp()].homes).count();
        println!(
            "{:34} {:5.1} ± {:3.1} | {:5.1} ± {:3.1} | {:+5.1} ± {:3.1} | {:+5.1} ± {:3.1} | {stamp:2}/{N} | {wolf:2}/{N} | {both:2}/{N}",
            p.name, c.0, c.1, h.0, h.1, dc.0, dc.1, dh.0, dh.1
        );
    }
    // Does the right order of the patrol depend on the forecast? Il Borgo
    // first vs Le Coste first, by the opening forecast's chance of a shift.
    let (b, c) = (idx("patrol-borgo-t1 + patrol-coste-t2"), idx("patrol-coste-t1 + patrol-borgo-t2"));
    println!("\nIl Borgo first − Le Coste first, families caught, by issue-1 shift chance (negative: Borgo first is better)");
    for (lo, hi) in [(0.0, 0.3), (0.3, 0.5), (0.5, 0.6), (0.6, 1.0)] {
        let v: Vec<f32> = (1..=N)
            .filter(|s| {
                let p = demo::draw(TOWN, *s).unwrap().forecast(1).shift_p;
                p >= lo && p < hi
            })
            .map(|s| results()[(s - 1) as usize][b].caught - results()[(s - 1) as usize][c].caught)
            .collect();
        let shifted = (1..=N)
            .filter(|s| {
                let d = demo::draw(TOWN, *s).unwrap();
                let p = d.forecast(1).shift_p;
                p >= lo && p < hi && d.spec.shift.is_some()
            })
            .count();
        let (m, se) = mean_se(&v);
        println!("  p in [{lo:.2}, {hi:.2}): n={:2}, wind turned in {shifted:2}: {m:+5.1} ± {se:3.1}", v.len());
    }
}

/// `forecast-player` is best or tied on families: no policy's mean is below
/// it by more than two standard errors of the paired difference.
#[test]
fn forecast_player_is_best_or_tied_on_families() {
    for j in 0..balance_set().len() {
        let (d, se) = diff(j, fp(), |r| r.caught);
        assert!(d >= -2.0 * se.max(0.25), "{} catches {:.1} ± {se:.1} fewer families than forecast-player", balance_set()[j].name, -d);
    }
}

#[test]
fn forecast_player_is_best_or_tied_on_homes() {
    for j in 0..balance_set().len() {
        let (d, se) = diff(j, fp(), |r| r.homes);
        assert!(d >= -2.0 * se.max(0.5), "{} hits {:.1} ± {se:.1} fewer homes than forecast-player", balance_set()[j].name, -d);
    }
}

/// Warning everyone, or Il Mulino first, loses families and a stamp.
#[test]
fn needless_stops_lose_families_and_a_stamp() {
    for name in ["patrol-everyone", "patrol-mulino-first"] {
        let j = idx(name);
        let (d, se) = diff(j, fp(), |r| r.caught);
        assert!(d > 2.0 * se && d > 0.5, "{name}: {d:.1} ± {se:.1} more families caught than forecast-player");
        let wolf = results().iter().filter(|r| r[j].stamps[MULINO] == Stamp::AllarmeInutile).count() as u64;
        assert!(wolf * 10 >= N * 9, "{name}: Il Mulino a false alarm in only {wolf}/{N}");
    }
}

/// Engines at the head are worth what no engines are.
#[test]
fn engines_at_the_head_equal_no_orders_on_homes() {
    let (d, se) = diff(idx("engines-head"), idx("none"), |r| r.homes);
    assert!(d.abs() <= 2.0 * se.max(0.5), "engines-head - none = {d:.1} ± {se:.1} homes");
}

/// The crew on the fire's path beats the crew on Il Mulino by ≥ 5 homes.
#[test]
fn the_crew_matters_where_the_fire_goes() {
    let (d, se) = diff(idx("crew-borgo-t1"), idx("crew-mulino-t1"), |r| r.homes);
    assert!(d <= -5.0 && -d > 2.0 * se, "crew Borgo - crew Mulino = {d:.1} ± {se:.1} homes");
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
