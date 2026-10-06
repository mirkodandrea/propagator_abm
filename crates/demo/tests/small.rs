//! Exploratory (spec §6.4): how small can the opening fire be and still establish?
//! `cargo test -p demo --release -- --ignored small --nocapture`
use demo::{spec, Run, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

#[test]
#[ignore]
fn small() {
    for id in ALL {
        for r in [60.0f32, 80.0, 120.0] {
            let mut row = vec![];
            for seed in 1..=8u64 {
                let mut s = spec(id).unwrap();
                s.radius_m = r;
                let mut run = Run::new(&data_dir(), s, seed).unwrap();
                let mut h = vec![];
                while run.time_s() < s.duration_s {
                    run.step().unwrap();
                    if [180, 600, 1800].contains(&run.time_s()) { h.push(run.outcome().hectares); }
                }
                h.push(run.outcome().hectares);
                row.push(format!("{:.0}/{:.0}/{:.0}/{:.0}", h[0], h[1], h[2], h[3]));
            }
            println!("{id} r={r:3.0}  ha@3/10/30/end by seed: {}", row.join("  "));
        }
    }
}

fn mean_caught(s: demo::Spec, at: Option<i64>, only: impl Fn(scenario::Pos) -> bool + Copy) -> f32 {
    let orders: Vec<_> = at.into_iter().map(|t| (t, demo::Order::EvacuateAll)).collect();
    (1..=5u64).map(|seed| {
        let mut r = Run::new(&data_dir(), s, seed).unwrap();
        r.play(&orders).unwrap();
        r.caught_where(only) as f32
    }).sum::<f32>() / 5.0
}

#[test]
#[ignore]
fn small_position() {
    for y in [1400.0f32, 1550.0, 1700.0, 1850.0] {
        let mut s = spec("demo_porto").unwrap();
        s.ignition.y = y;
        let v: Vec<_> = [None, Some(0), Some(600), Some(1200)].iter().map(|&o| mean_caught(s, o, |_| true)).collect();
        println!("porto y={y}: none/T0/T10/T20 {v:?}");
    }
    for (x, y) in [(2000.0f32, 2400.0f32), (1900.0, 2300.0), (1800.0, 2300.0), (1700.0, 2400.0)] {
        let mut s = spec("demo_valle").unwrap();
        s.ignition = scenario::Pos { x, y };
        let v: Vec<_> = [None, Some(0), Some(600), Some(1200)].iter().map(|&o| (mean_caught(s, o, |p| p.x <= 2400.0), mean_caught(s, o, |p| p.x > 2400.0))).collect();
        println!("valle ({x},{y}) A/B none/T0/T10/T20 {v:?}");
    }
}

#[test]
#[ignore]
fn small_late() {
    for (id, y) in [("demo_borgo", 1300.0f32), ("demo_porto", 1850.0), ("demo_porto", 2000.0)] {
        let mut s = spec(id).unwrap();
        s.ignition.y = y;
        let v: Vec<_> = [None, Some(0), Some(900), Some(1800), Some(2400), Some(3000)].iter().map(|&o| mean_caught(s, o, |_| true)).collect();
        println!("{id} y={y}: none/T0/T15/T30/T40/T50 {v:?}");
    }
    use fire::Weather;
    let w = |dir, kmh| Weather { wind_dir_deg: dir, wind_speed_kmh: kmh, moisture_pct: 6.0 };
    for (x, at, kmh) in [(2200.0f32, 1200i64, 40.0f64), (2200.0, 1800, 45.0), (2000.0, 1800, 45.0), (2300.0, 1500, 45.0)] {
        let mut s = spec("demo_valle").unwrap();
        s.ignition.x = x;
        s.shift = Some(demo::WindShift { at_s: at, weather: w(270.0, kmh) });
        let v: Vec<_> = [None, Some(0), Some(600), Some(1200)].iter().map(|&o| (mean_caught(s, o, |p| p.x <= 2400.0), mean_caught(s, o, |p| p.x > 2400.0))).collect();
        println!("valle x={x} shift@{at} {kmh}: A/B none/T0/T10/T20 {v:?}");
    }
}



#[test]
#[ignore]
fn early_action_all() {
    use abm::suppression::UnitKind;
    for id in ALL {
        let s = spec(id).unwrap();
        let go = |orders: &[(i64, demo::Order)]| -> (f32, f32) {
            let (mut h, mut l) = (0.0, 0.0);
            for seed in 1..=5u64 {
                let mut r = Run::new(&data_dir(), s, seed).unwrap();
                let o = r.play(orders).unwrap();
                h += o.hectares / 5.0; l += o.homes_lost as f32 / 5.0;
            }
            (h, l)
        };
        let at = s.ignition;
        let all = |t: i64| { let mut v = vec![]; for _ in 0..3 { v.push((t, demo::Order::Attack { kind: UnitKind::HandCrew, at })); v.push((t, demo::Order::Attack { kind: UnitKind::Engine, at })); } v };
        println!("{id}: none {:?}  ALL T3 {:?}  T10 {:?}  T20 {:?}", go(&[]), go(&all(180)), go(&all(600)), go(&all(1200)));
    }
}



