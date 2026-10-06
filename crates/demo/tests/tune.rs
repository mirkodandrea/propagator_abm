//! Exploratory: where should each fire start? `cargo test -p demo --release -- --ignored tune --nocapture`
use demo::{spec, Order, Run};
use fire::Weather;
use scenario::Pos;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

fn caught(s: demo::Spec, order_at: Option<i64>) -> (f32, f32, f32, f32) {
    let (mut c, mut l, mut h, mut t) = (0.0, 0.0, 0.0, 0.0);
    for seed in 1..=3u64 {
        let mut r = Run::new(&data_dir(), s, seed).unwrap();
        let orders: Vec<_> = order_at.into_iter().map(|t| (t, Order::EvacuateAll)).collect();
        let o = r.play(&orders).unwrap();
        c += o.caught as f32 / 3.0; l += o.homes_lost as f32 / 3.0; h += o.hectares / 3.0;
        t += r.first_caught_s().unwrap_or(s.duration_s) as f32 / 180.0;
    }
    (c, l, h, t)
}

fn sweep(id: &str, cands: &[(f32, f32)], radii: &[f32], winds: &[(f64, f64)], moist: f64) {
    println!("\n== {id}");
    println!("  {:>6} {:>6} {:>7} {:>5} | caught none/T0/T10/T20  lost(T0)  ha", "x", "y", "r", "kmh");
    for &(x, y) in cands { for &r in radii { for &(dir, kmh) in winds {
        let mut s = spec(id).unwrap();
        s.ignition = Pos { x, y }; s.radius_m = r;
        s.weather = Weather { wind_dir_deg: dir, wind_speed_kmh: kmh, moisture_pct: moist };
        let n = caught(s, None); let a = caught(s, Some(0)); let b = caught(s, Some(600)); let c = caught(s, Some(1200));
        println!("  {x:6.0} {y:6.0} {r:7.0} {kmh:5.0} | {:5.0} {:5.0} {:5.0} {:5.0}   {:5.0}   {:5.0}  firstcaught(none) {:3.0} min", n.0, a.0, b.0, c.0, a.1, a.2, n.3);
    }}}
}

#[test]
#[ignore]
fn tune() {
    println!("\n== valle: caught in hamlet B (x>2400), shift W at T+20; none/T0/T10/T20");
    for (y, r, k1, k2) in [(2400.0, 120.0, 30.0, 35.0), (2400.0, 200.0, 30.0, 35.0), (2200.0, 150.0, 30.0, 35.0), (2400.0, 150.0, 25.0, 40.0)] {
        let mut s = spec("demo_valle").unwrap();
        s.ignition = Pos { x: 2000.0, y };
        s.radius_m = r;
        s.weather = Weather { wind_dir_deg: 90.0, wind_speed_kmh: k1, moisture_pct: 6.0 };
        s.shift = Some(demo::WindShift { at_s: 1200, weather: Weather { wind_dir_deg: 270.0, wind_speed_kmh: k2, moisture_pct: 6.0 } });
        let mut row = vec![];
        for o in [None, Some(0), Some(600), Some(1200)] {
            let (mut a, mut b) = (0.0, 0.0);
            for seed in 1..=3u64 {
                let mut run = Run::new(&data_dir(), s, seed).unwrap();
                let orders: Vec<_> = o.into_iter().map(|t| (t, Order::EvacuateAll)).collect();
                run.play(&orders).unwrap();
                a += run.caught_where(|p| p.x <= 2400.0) as f32 / 3.0;
                b += run.caught_where(|p| p.x > 2400.0) as f32 / 3.0;
            }
            row.push(format!("{a:.0}/{b:.0}"));
        }
        println!("  y={y} r={r} {k1}->{k2} km/h  A/B caught: {}", row.join("  "));
    }
}
