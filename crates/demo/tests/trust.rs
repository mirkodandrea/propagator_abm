//! Spec 5.2: an order the fire never justified costs the next one its compliance,
//! and none of it exists until used (finding 34).

use demo::{spec, Order, Run, Variant, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

fn wolf(delta: f32) -> Variant {
    Variant { cry_wolf: Some(delta), ..Variant::default() }
}

#[test]
fn nothing_moves_until_an_order_is_judged_needless() {
    for id in ALL {
        let s = spec(id).unwrap();
        // Off, on-with-no-orders and on-with-a-justified-order leave every trait alone.
        let mut off = Run::new(&data_dir(), s, 3).unwrap();
        let mut on = Run::with_variant(&data_dir(), s, 3, wolf(0.25)).unwrap();
        let start = on.trust();
        for _ in 0..(40 * 60 / demo::STEP_S) {
            off.step().unwrap();
            on.step().unwrap();
        }
        assert_eq!(on.trust(), start, "{id}: trust moved with no order given");
        assert_eq!(off.outcome(), on.outcome(), "{id}: switching cry-wolf on changed a run with no orders");
        assert_eq!(off.trust(), start, "{id}");
    }
}

/// The branch fires: warning the upwind district (Fondovalle) at T+0 is judged
/// a false alarm thirty minutes on, and the households not yet warned trust the
/// next order less. (Whether that costs families depends on a later order still
/// being useful, which on the district towns it rarely is -- why the kiosk
/// judges false alarms on the end card instead; `trust::JUDGE_AFTER_S`.)
#[test]
fn a_needless_order_costs_the_town_its_trust() {
    let s = spec("demo_valle").unwrap();
    let (mut trust_drop, mut angry) = (0.0f32, 0.0f32);
    let seeds = 1..=6u64;
    let n = seeds.clone().count() as f32;
    for seed in seeds {
        let go = |v: Variant| {
            let mut r = Run::with_variant(&data_dir(), s, seed, v).unwrap();
            r.order(Order::EvacuateDistrict(2));
            while r.time_s() < 32 * 60 {
                r.step().unwrap();
            }
            r.trust()
        };
        let (t_off, t_on) = (go(Variant::default()), go(wolf(0.35)));
        trust_drop += (t_off.mean - t_on.mean) / n;
        angry += t_on.angry_households as f32 / n;
        assert_eq!(t_off.angry_households, 0, "cry-wolf is off and somebody is angry");
    }
    println!("valle: mean trust drop {trust_drop:.3}; angry {angry:.0}");
    assert!(trust_drop > 0.05, "a needless order should have lowered the town's trust ({trust_drop:.3})");
    assert!(angry >= 20.0, "nobody is angry after a needless order ({angry})");
}
