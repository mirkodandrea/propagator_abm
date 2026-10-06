//! Spec 5.4 / 5.6: the model says when something needs a decision.

use demo::policy::Policy;
use demo::weather::draw;
use demo::{EventKind, Run, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

fn count(run: &Run, f: impl Fn(&EventKind) -> bool) -> usize {
    run.referee.events.iter().filter(|e| f(&e.kind)).count()
}

/// 5.4: a typical session produces spot fires, as events with a time and a place.
/// Borgo and porto throw embers in every session; valle only once the west wind
/// of its shift has risen (the 25 km/h valley-floor east wind never does, measured
/// at 25-35 km/h and 4-6 % moisture), so valle is asserted on its scripted shift.
#[test]
fn a_typical_session_has_spot_fires_and_says_so() {
    for id in ["demo_borgo", "demo_porto"] {
        let mut with = 0;
        for seed in 1..=8u64 {
            let d = draw(id, seed).unwrap();
            let mut r = Run::new(&data_dir(), d.spec, seed).unwrap();
            Policy::none().play(&mut r, &d).unwrap();
            let spots: Vec<_> = r.referee.events.iter().filter(|e| e.kind == EventKind::SpotFire).collect();
            with += (!spots.is_empty()) as usize;
            for e in spots {
                let p = e.pos.expect("a spot fire has a place");
                assert!(e.at_s > 0 && p.x >= 0.0 && p.y >= 0.0 && p.x <= r.scn.world.width_m && p.y <= r.scn.world.height_m, "{id}: {e:?}");
            }
        }
        assert!(with >= 7, "{id}: only {with} of 8 sessions had a spot fire");
    }
    let spec = demo::spec("demo_valle").unwrap();
    for seed in 1..=4u64 {
        let mut r = Run::new(&data_dir(), spec, seed).unwrap();
        r.play(&[]).unwrap();
        assert!(count(&r, |k| *k == EventKind::SpotFire) > 0, "valle seed {seed}: the shift did not throw an ember");
        assert!(r.referee.events.iter().any(|e| matches!(e.kind, EventKind::WindShifted { .. })), "valle seed {seed}: the shift was not reported");
    }
}

/// The events of a run are a function of the run: same seed, same events.
#[test]
fn events_are_deterministic() {
    let d = draw("demo_porto", 5).unwrap();
    let go = || {
        let mut r = Run::new(&data_dir(), d.spec, 5).unwrap();
        Policy::units(3).play(&mut r, &d).unwrap();
        r.referee.events
    };
    assert_eq!(go(), go());
}

/// Valle opens in a 25 km/h east wind and almost never throws an ember (3 of 16
/// sessions). What wind / moisture makes it spot, and what does that do to the beat?
#[test]
#[ignore = "report"]
fn valle_spotting_sweep() {
    use demo::{Order, WindShift};
    for (kmh, moist) in [(25.0f64, 6.0f64), (30.0, 6.0), (35.0, 6.0), (30.0, 4.0), (35.0, 4.5)] {
        let (mut sp, mut b_none, mut b_early, mut ha) = (0, 0.0f32, 0.0f32, 0.0f32);
        for seed in 1..=10u64 {
            let mut s = demo::spec("demo_valle").unwrap();
            s.weather.wind_speed_kmh = kmh;
            s.weather.moisture_pct = moist;
            s.shift = if std::env::var("NOSHIFT").is_ok() { None } else { Some(WindShift { at_s: 1800, weather: s.shift.unwrap().weather }) };
            let mut r = Run::new(&data_dir(), s, seed).unwrap();
            r.play(&[]).unwrap();
            sp += (count(&r, |k| *k == EventKind::SpotFire) > 0) as usize;
            b_none += r.caught_where(|p| p.x > 2400.0) as f32 / 10.0;
            ha += r.outcome().hectares / 10.0;
            let mut r2 = Run::new(&data_dir(), s, seed).unwrap();
            r2.play(&[(0, Order::EvacuateAll)]).unwrap();
            b_early += r2.caught_where(|p| p.x > 2400.0) as f32 / 10.0;
        }
        println!("valle {kmh} km/h {moist}%: {sp}/10 sessions spot; hamlet B caught none {b_none:.1} early {b_early:.1}; {ha:.0} ha");
    }
}

#[test]
#[ignore = "report"]
fn event_report() {
    for id in ALL {
        let (mut spots, mut with_spot, mut shifts, mut near, mut first_spot) = (0, 0, 0, 0, vec![]);
        for seed in 1..=16u64 {
            let d = draw(id, seed).unwrap();
            let mut r = Run::new(&data_dir(), d.spec, seed).unwrap();
            Policy::none().play(&mut r, &d).unwrap();
            let s = count(&r, |k| *k == EventKind::SpotFire);
            spots += s;
            with_spot += (s > 0) as usize;
            shifts += count(&r, |k| matches!(k, EventKind::WindShifted { .. }));
            near += count(&r, |k| *k == EventKind::FireNearTown);
            if let Some(e) = r.referee.events.iter().find(|e| e.kind == EventKind::SpotFire) {
                first_spot.push(e.at_s / 60);
            }
        }
        println!("{id}: spot fires {spots} over 16 sessions ({with_spot} sessions have one), first at T+{first_spot:?} min; wind shifts {shifts}; fire near town {near}");
    }
}
