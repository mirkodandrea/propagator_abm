//! Spec 5.2: an order the fire never justified costs the next one its compliance,
//! and none of it exists until used (finding 34).

use demo::{spec, Order, Run, Variant, ALL};
use scenario::Pos;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

fn wolf(delta: f32) -> Variant {
    Variant { cry_wolf: Some(delta), ..Variant::default() }
}

/// The home furthest from the opening ignition: where a needless zone order goes.
fn far_corner(run: &Run) -> Pos {
    let o = run.spec.ignition;
    run.agents
        .households
        .iter()
        .map(|h| h.home)
        .max_by(|a, b| {
            let d = |p: &Pos| (p.x - o.x).powi(2) + (p.y - o.y).powi(2);
            d(a).partial_cmp(&d(b)).unwrap()
        })
        .unwrap()
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

/// The branch fires: a zone order on homes the fire never gets near is judged
/// needless 30 minutes on, and the households still to be ordered trust the next
/// order less -- so more of them are still at home when it comes.
#[test]
fn a_needless_order_costs_the_next_one_its_compliance() {
    let s = spec("demo_valle").unwrap();
    let (mut caught_on, mut caught_off, mut trust_drop, mut angry) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let seeds = 1..=8u64;
    let n = seeds.clone().count() as f32;
    for seed in seeds {
        let go = |v: Variant| {
            let mut r = Run::with_variant(&data_dir(), s, seed, v).unwrap();
            let far = far_corner(&r);
            // Needless: the far corner, now.
            r.order(Order::EvacuateZone { centre: far, radius_m: 350.0 });
            while r.time_s() < 31 * 60 {
                r.step().unwrap();
            }
            let mid = r.trust();
            // The real thing, thirty-one minutes in, just after the wind has turned.
            r.order(Order::EvacuateAll);
            while r.time_s() < s.duration_s {
                r.step().unwrap();
            }
            (r.outcome().caught as f32, mid)
        };
        let (c_off, t_off) = go(Variant::default());
        let (c_on, t_on) = go(wolf(0.35));
        caught_off += c_off / n;
        caught_on += c_on / n;
        trust_drop += (t_off.mean - t_on.mean) / n;
        angry += t_on.angry_households as f32 / n;
        assert_eq!(t_off.angry_households, 0, "cry-wolf is off and somebody is angry");
    }
    println!("valle: caught {caught_off:.1} (off) vs {caught_on:.1} (on); mean trust drop {trust_drop:.3}; angry {angry:.0}");
    assert!(trust_drop > 0.05, "a needless order should have lowered the town's trust ({trust_drop:.3})");
    assert!(angry >= 20.0, "nobody is angry after a needless order ({angry})");
    assert!(caught_on > caught_off + 0.5, "the next order should find fewer complying ({caught_on:.1} vs {caught_off:.1})");
}
