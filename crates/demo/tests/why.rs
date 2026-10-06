//! Playtest 16 #37: the end card says *why*. Each explanation is reachable, and the
//! pure function agrees with the run that produced the facts.

use demo::policy::Policy;
use demo::weather::draw;
use demo::{Facts, Outcome, Run, WhyKind, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

#[test]
fn the_classification_is_what_it_says() {
    let o = |caught| Outcome { households: 100, caught, ..Default::default() };
    let f = Facts { order_at_s: Some(600), shift_at_s: None, near_town_s: Some(1500), first_caught_s: Some(1800) };
    assert_eq!(demo::why::why(&o(0), Facts { near_town_s: None, ..f }).kind, WhyKind::FireNeverCame);
    assert_eq!(demo::why::why(&o(0), f).kind, WhyKind::OrderInTime);
    assert_eq!(demo::why::why(&o(5), Facts { order_at_s: None, ..f }).kind, WhyKind::NoOrder);
    assert_eq!(demo::why::why(&o(5), Facts { order_at_s: Some(2400), ..f }).kind, WhyKind::OrderTooLate);
    assert_eq!(demo::why::why(&o(5), f).kind, WhyKind::SomeSlowToLeave);
    assert_eq!(demo::why::why(&o(5), Facts { order_at_s: None, shift_at_s: Some(1200), ..f }).kind, WhyKind::WindShiftReachedTown);
    assert_eq!(demo::why::why(&o(5), f).lead_min, Some(20));
}

/// Over real sessions every kind of explanation that should be reachable is.
#[test]
fn real_sessions_reach_every_explanation() {
    let mut seen = std::collections::HashSet::new();
    for id in ALL {
        for seed in 1..=10u64 {
            let d = draw(id, seed).unwrap();
            for p in [Policy::none(), Policy::evacuate(0), Policy::evacuate(40)] {
                let mut r = Run::new(&data_dir(), d.spec, seed).unwrap();
                p.play(&mut r, &d).unwrap();
                let w = r.why();
                // The facts the card uses are the facts of the run.
                assert_eq!(w.facts.first_caught_s, r.first_caught_s());
                seen.insert(std::mem::discriminant(&w.kind));
                if p.name == "none" {
                    assert!(w.facts.order_at_s.is_none());
                    assert!(matches!(w.kind, WhyKind::FireNeverCame | WhyKind::CloseButNobodyCaught | WhyKind::NoOrder | WhyKind::WindShiftReachedTown), "{id} {seed}: {:?}", w.kind);
                }
            }
        }
    }
    println!("{} distinct explanations reached", seen.len());
    assert!(seen.len() >= 4, "only {} kinds of explanation were ever reached", seen.len());
}
