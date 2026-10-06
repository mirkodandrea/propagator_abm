//! Unit effectiveness (spec §4): what is pinned, and why option A is not the answer.
//! The sweeps behind these assertions are `tests/units_sweep.rs`; their tables are
//! recorded in `docs/demo-spec-gameplay.md` §4.

use abm::suppression::UnitEffect;
use demo::policy::Policy;
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

// Option A (a multiplier on what a unit does) was pinned as *not* the lever on
// the one-village towns; on the district towns a god-mode multiplier saves
// ~8 homes on Rocca Ventosa, but option B below is the shipped rule and the
// kiosk never sets a multiplier. The sweep stays in `tests/units_sweep.rs`.

/// Option B fires, and the lesson is *where*: three engines defending the
/// district the wind is driving at (district 0 while the wind holds) at least
/// halve the homes the fire reaches there, and the same engines sent to the
/// upwind district (2) save nothing. Defence on, drawn sessions where the wind
/// holds.
#[test]
fn defending_the_right_district_saves_homes() {
    use demo::policy::Act;
    let v = Variant { defend_homes: true, ..Variant::default() };
    for id in ["demo_borgo", "demo_valle"] {
        let (mut none, mut right, mut wrong, mut n) = (0.0, 0.0, 0.0, 0.0);
        for seed in 1..=16u64 {
            let d = draw(id, seed).unwrap();
            if d.spec.shift.is_some() {
                continue;
            }
            let go = |p: Policy| {
                let mut r = Run::with_variant(&data_dir(), d.spec, seed, v).unwrap();
                p.play(&mut r, &d).unwrap().homes_lost as f32
            };
            none += go(Policy::none());
            right += go(Policy::named("d0").at(180, Act::DefendDistrict(0, 3)));
            wrong += go(Policy::named("d2").at(180, Act::DefendDistrict(2, 3)));
            n += 1.0;
        }
        let (none, right, wrong) = (none / n, right / n, wrong / n);
        println!("{id}: homes hit none {none:.1}, defend district 0 {right:.1}, defend upwind {wrong:.1} ({n} sessions)");
        assert!(right < 0.6 * none, "{id}: engines on the right district should save homes");
        assert!((wrong - none).abs() < 2.0, "{id}: engines on the upwind district should change nothing");
    }
}
