//! Unit effectiveness (spec §4): what is pinned, and why option A is not the answer.
//! The sweeps behind these assertions are `tests/units_sweep.rs`; their tables are
//! recorded in `docs/demo-spec-gameplay.md` §4.

use abm::suppression::UnitEffect;
use demo::policy::Policy;
use demo::sweep::{paired, run_grid};
use demo::weather::draw;
use demo::{Run, Variant, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

/// Finding 34: a new mechanism is provably inert, not merely off. The default
/// variant is the published model, and so is every dial at its neutral setting.
#[test]
fn the_variants_are_inert_by_default() {
    assert_eq!(Variant::default().unit_effect, UnitEffect::ONE);
    assert!(!Variant::default().defend_homes);
    for id in ALL {
        let d = draw(id, 5).unwrap();
        let play = |v: Variant| {
            let mut r = Run::with_variant(&data_dir(), d.spec, 5, v).unwrap();
            Policy::units_ahead(3, 150.0).play(&mut r, &d).unwrap()
        };
        let published = {
            let mut r = Run::new(&data_dir(), d.spec, 5).unwrap();
            Policy::units_ahead(3, 150.0).play(&mut r, &d).unwrap()
        };
        assert_eq!(play(Variant::default()), published, "{id}: Run::new and the default variant differ");
        assert_eq!(play(Variant { unit_effect: UnitEffect::all(1.0), ..Variant::default() }), published, "{id}");
    }
}

/// With nobody defending anything, switching home defence on changes nothing:
/// the latch reproduces the end-state rule `homes_lost` always used.
#[test]
fn home_defence_changes_nothing_until_a_unit_is_posted() {
    for id in ALL {
        for seed in [2u64, 9] {
            let d = draw(id, seed).unwrap();
            let go = |defend| {
                let mut r = Run::with_variant(&data_dir(), d.spec, seed, Variant { defend_homes: defend, ..Variant::default() }).unwrap();
                Policy::evacuate(0).play(&mut r, &d).unwrap()
            };
            assert_eq!(go(false), go(true), "{id} seed {seed}");
        }
    }
}

/// Option A is *not* the lever (spec §4): even units that cut x8 line, wet x8,
/// reach 4x further and keep working in heat leave homes hit within noise on
/// every town. Asserted so nobody re-tunes the multiplier and calls it a fix.
#[test]
fn scaling_what_a_unit_does_does_not_save_homes() {
    let god = Variant { unit_effect: UnitEffect { line_x: 8.0, water_x: 8.0, reach_x: 4.0, nerve_x: 0.25 }, ..Variant::default() };
    let policies = vec![Policy::none(), Policy::units_ahead(3, 300.0)];
    let rs = run_grid(&data_dir(), &ALL, &policies, &[Variant::default(), god], 1..=12);
    for id in ALL {
        let (d, se) = paired(&rs, id, (1, 1), (0, 0), |o| o.homes_lost as f32);
        println!("{id}: god-mode units change homes hit by {d:+.1} +- {se:.1}");
        assert!(d > -6.0, "{id}: a multiplier now saves {d:.1} homes: option A has become viable, re-read spec §4");
    }
}

/// Option B fires: posting engines at the houses nearest the fire, with defence
/// on, saves homes on valle (a hamlet the fire reaches at ~T+30), and sending
/// them later saves fewer -- timing matters.
#[test]
fn protecting_homes_saves_homes_and_earlier_saves_more() {
    let on = Variant { defend_homes: true, ..Variant::default() };
    let policies = vec![Policy::none(), Policy::protect(3), Policy::protect(30)];
    let rs = run_grid(&data_dir(), &["demo_valle"], &policies, &[Variant::default(), on], 1..=12);
    let (early, e_se) = paired(&rs, "demo_valle", (1, 1), (0, 1), |o| o.homes_lost as f32);
    let (late, _) = paired(&rs, "demo_valle", (2, 1), (0, 1), |o| o.homes_lost as f32);
    println!("valle: protect T+3 {early:+.1} +- {e_se:.1} homes, T+30 {late:+.1}");
    assert!(early < -2.0, "protecting at T+3 saved only {early:.1} homes");
    assert!(early < late - 0.5, "an engine posted at T+3 should save more than one posted at T+30 ({early:.1} vs {late:.1})");
    // And with defence off the same orders are worth nothing.
    let (off, off_se) = paired(&rs, "demo_valle", (1, 0), (0, 0), |o| o.homes_lost as f32);
    assert!(off > -3.0 * off_se.max(1.0), "without defence the posting should not help ({off:+.1})");
}
