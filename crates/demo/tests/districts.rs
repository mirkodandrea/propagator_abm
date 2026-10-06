//! The district game (`docs/demo-spec-gameplay.md` §2): each town is three
//! districts at three bearings from the fire, and the lesson is *who* to warn
//! as much as *when*. Asserted as "fires", on drawn sessions, so the pins hold
//! for what a visitor actually plays. Tables: `tests/district_sweep.rs`.

use demo::cost::Weights;
use demo::policy::Policy;
use demo::sweep::{run_grid, Record};
use demo::{Run, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

#[test]
fn every_town_has_three_districts_and_an_assembly_area() {
    for id in ALL {
        let d = demo::draw(id, 1).unwrap();
        let run = Run::new(&data_dir(), d.spec, 1).unwrap();
        let ds = &run.referee.districts;
        assert_eq!(ds.len(), 3, "{id}: {:?}", ds.iter().map(|d| &d.name).collect::<Vec<_>>());
        assert!(ds.iter().all(|d| d.households.len() >= 30), "{id}: a district too small to matter");
        assert_eq!(ds.iter().map(|d| d.households.len()).sum::<usize>(), run.agents.households.len(), "{id}: a household in no district");
        // An *area di attesa* inside the window, not only the map-edge exits:
        // without it Porto's one refuge was the end of the road through the fire.
        assert!(run.agents.refuges.iter().any(|r| !r.is_exit), "{id}: no assembly area");
    }
}

/// The wind decides who: with no order, district 0 is caught when the wind holds
/// and district 1 when it shifts, and district 2 (upwind) hardly ever.
#[test]
fn the_wind_decides_which_district_is_in_danger() {
    let rs = run_grid(&data_dir(), &["demo_borgo", "demo_valle"], &[Policy::none()], &[Default::default()], 1..=16);
    for id in ["demo_borgo", "demo_valle"] {
        let mean = |shift: bool, k: usize| {
            let v: Vec<f32> = rs
                .iter()
                .filter(|r| r.town == id && demo::draw(id, r.seed).unwrap().spec.shift.is_some() == shift)
                .map(|r| r.districts[k].caught as f32)
                .collect();
            v.iter().sum::<f32>() / v.len().max(1) as f32
        };
        let (held0, held1, shift0, shift1) = (mean(false, 0), mean(false, 1), mean(true, 0), mean(true, 1));
        let upwind = (mean(false, 2) + mean(true, 2)) / 2.0;
        println!("{id}: wind holds -> {held0:.1} / {held1:.1}; wind shifts -> {shift0:.1} / {shift1:.1}; upwind {upwind:.1}");
        assert!(held0 > held1 + 4.0, "{id}: the opening wind should drive the fire at district 0");
        assert!(shift1 > held1 + 4.0, "{id}: a shift should put district 1 in danger");
        assert!(upwind < 1.5, "{id}: the upwind district is caught ({upwind:.1}) -- it is meant to be the false alarm");
    }
}

/// Early and targeted beats both waiting and warning everyone: warning the two
/// districts at risk at T+0 halves the families caught; waiting until the fire
/// is close saves almost nobody; warning at T+20 is clearly worse than T+0; and
/// warning everybody saves no more but raises false alarms and costs more.
#[test]
fn warn_the_districts_at_risk_early() {
    let policies = vec![
        Policy::none(),
        Policy::warn_at_risk(0),
        Policy::warn_at_risk(20),
        Policy::named("warn when threatened").with(demo::policy::Rule::WarnWhenThreatened),
        Policy::evacuate(0),
    ];
    let rs = run_grid(&data_dir(), &ALL, &policies, &[Default::default()], 1..=12);
    let w = Weights::ANALYSIS;
    for id in ALL {
        let of = |p: usize| rs.iter().filter(move |r: &&Record| r.town == id && r.policy == p);
        let n = of(0).count() as f32;
        let caught = |p: usize| of(p).map(|r| r.out.caught as f32).sum::<f32>() / n;
        let alarms = |p: usize| of(p).map(|r| r.false_alarms as f32).sum::<f32>() / n;
        let loss = |p: usize| of(p).map(|r| w.loss(r.eur, r.out.caught, 0) / 1000.0).sum::<f32>() / n;
        let (none, early, late, wait, all) = (caught(0), caught(1), caught(2), caught(3), caught(4));
        println!(
            "{id}: caught none {none:.1}, at-risk T+0 {early:.1}, T+20 {late:.1}, when threatened {wait:.1}, everyone {all:.1}; \
             false alarms at-risk {:.2} everyone {:.2}; loss k {:.0} / {:.0} / {:.0}",
            alarms(1), alarms(4), loss(0), loss(1), loss(4)
        );
        assert!(early * 2.0 < none, "{id}: an early warning to the right districts should halve the families caught");
        assert!(late > early + 2.0, "{id}: T+20 should be clearly worse than T+0 ({late:.1} vs {early:.1})");
        assert!(wait > 0.7 * none, "{id}: waiting to see the fire should not work ({wait:.1} vs {none:.1})");
        assert!(alarms(4) > alarms(1) + 0.3, "{id}: warning everyone should raise false alarms");
        assert!(loss(1) < loss(0), "{id}: the targeted warning should beat doing nothing on loss");
    }
}
