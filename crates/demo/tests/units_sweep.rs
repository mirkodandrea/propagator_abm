//! Spec §4: do units matter? `cargo test -p demo --release -- --ignored units_option_a --nocapture`

use abm::suppression::UnitEffect;
use demo::policy::Policy;
use demo::sweep::{paired, regret, run_grid, summarise};
use demo::{Variant, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

/// Where do units have to be sent for them to do anything at all? (they refuse
/// to work in lethal heat, so "at the head" may be the wrong answer).
#[test]
#[ignore = "report"]
fn units_where() {
    let policies = vec![
        Policy::none(),
        Policy::units_ahead(3, 0.0),
        Policy::units_ahead(3, 150.0),
        Policy::units_ahead(3, 300.0),
        Policy::units_ahead(3, 500.0),
        Policy::protect(3),
        Policy::units_ahead(30, 150.0),
    ];
    table(&policies, &[1.0, 8.0, 32.0]);
}

/// Option C: a slower opening fire, so units at T+3 face a front of a few cells.
/// Paired against `none` on the *same* slowed fire.
#[test]
#[ignore = "report"]
fn units_option_c() {
    use demo::weather::draw;
    use demo::Run;
    for id in ALL {
        for (label, wind_x, moist_add) in [("as shipped", 1.0f64, 0.0f64), ("wind x0.6, +3% moisture", 0.6, 3.0), ("wind x0.4, +5% moisture", 0.4, 5.0)] {
            let mut rows: Vec<(f32, f32, f32, f32, f32, f32)> = vec![];
            for seed in 1..=16u64 {
                let d = draw(id, seed).unwrap();
                let mut s = d.spec;
                s.weather.wind_speed_kmh = (s.weather.wind_speed_kmh * wind_x).max(6.0);
                s.weather.moisture_pct += moist_add;
                if let Some(sh) = s.shift.as_mut() {
                    sh.weather.wind_speed_kmh = (sh.weather.wind_speed_kmh * wind_x).max(6.0);
                    sh.weather.moisture_pct += moist_add;
                }
                let d2 = demo::Draw { spec: s, ..d };
                let go = |p: &Policy| {
                    let mut r = Run::new(&data_dir(), s, seed).unwrap();
                    p.play(&mut r, &d2).unwrap()
                };
                let a = go(&Policy::none());
                let b = go(&Policy::units_ahead(3, 150.0));
                rows.push((a.hectares, b.hectares, a.homes_lost as f32, b.homes_lost as f32, a.caught as f32, b.caught as f32));
            }
            let m = |f: fn(&(f32, f32, f32, f32, f32, f32)) -> f32| rows.iter().map(f).sum::<f32>() / rows.len() as f32;
            let se = |f: fn(&(f32, f32, f32, f32, f32, f32)) -> f32| {
                let xs: Vec<f32> = rows.iter().map(f).collect();
                let mu = xs.iter().sum::<f32>() / xs.len() as f32;
                (xs.iter().map(|x| (x - mu).powi(2)).sum::<f32>() / xs.len() as f32).sqrt() / (xs.len() as f32).sqrt()
            };
            println!(
                "{id:<11} {label:<26} none: {:>5.1} ha {:>5.1} homes | units T+3: {:>+5.1}+-{:.1} ha  {:>+5.1}+-{:.1} homes",
                m(|r| r.0), m(|r| r.2),
                m(|r| r.1 - r.0), se(|r| r.1 - r.0), m(|r| r.3 - r.2), se(|r| r.3 - r.2)
            );
        }
    }
}

/// Option B: units protect homes instead of stopping the front.
#[test]
#[ignore = "report"]
fn units_option_b() {
    let policies = vec![
        Policy::none(),
        Policy::units_ahead(3, 300.0),
        Policy::protect(3),
        Policy::protect(10),
        Policy::protect(30),
        Policy::protect_when_near(1000.0),
        Policy::protect_when_near(600.0),
        Policy::protect_when_near(300.0),
        Policy::evacuate(0),
        Policy::evacuate_and_units(0, 3),
    ];
    let variants = vec![Variant::default(), Variant { defend_homes: true, ..Variant::default() }];
    let rs = run_grid(&data_dir(), &ALL, &policies, &variants, 1..=16);
    for town in ALL {
        println!("\n== {town}   (mean over 16 seeds; defend_homes off | on)");
        for (pi, p) in policies.iter().enumerate() {
            let a = summarise(&rs, town, pi, 0);
            let b = summarise(&rs, town, pi, 1);
            let (h, hs) = paired(&rs, town, (pi, 1), (0, 0), |o| o.homes_lost as f32);
            let (hn, hns) = paired(&rs, town, (pi, 1), (0, 1), |o| o.homes_lost as f32);
            let (ha, has) = paired(&rs, town, (pi, 1), (0, 1), |o| o.hectares);
            println!(
                "  {:<26} | homes {:>5.1} | {:>5.1}   | vs none(on): homes {:>+5.1} +-{:>3.1}  ha {:>+5.1} +-{:>3.1}  caught {:>5.1}",
                p.name, a.homes_lost.mean, b.homes_lost.mean, hn, hns, ha, has, b.caught.mean
            );
            let _ = (h, hs);
        }
    }
}

/// The upper bound: units that cut ×8 line, wet ×8, reach four times as far and
/// keep working in heat. If even this moves nothing, no multiplier is the answer.
#[test]
#[ignore = "report"]
fn units_god_mode() {
    let policies = vec![Policy::none(), Policy::units_ahead(3, 0.0), Policy::units_ahead(3, 300.0), Policy::units_ahead(10, 150.0)];
    let variants = vec![
        Variant::default(),
        Variant { unit_effect: UnitEffect { line_x: 8.0, water_x: 8.0, reach_x: 4.0, nerve_x: 0.25 }, ..Variant::default() },
        Variant { unit_effect: UnitEffect { line_x: 8.0, water_x: 8.0, reach_x: 8.0, nerve_x: 0.0 }, ..Variant::default() },
    ];
    let rs = run_grid(&data_dir(), &ALL, &policies, &variants, 1..=16);
    for town in ALL {
        println!("\n== {town}");
        for (pi, p) in policies.iter().enumerate().skip(1) {
            for (vi, name) in ["published", "x8 reach4 nerve.25", "x8 reach8 nerve0"].iter().enumerate() {
                let (h, hs) = paired(&rs, town, (pi, vi), (0, 0), |o| o.homes_lost as f32);
                let (a, as_) = paired(&rs, town, (pi, vi), (0, 0), |o| o.hectares);
                let (c, cs) = paired(&rs, town, (pi, vi), (0, 0), |o| o.caught as f32);
                println!("  {:<22} {:<20} | homes {:>+6.1} +-{:>4.1}  ha {:>+6.1} +-{:>4.1}  caught {:>+5.1} +-{:>4.1}", p.name, name, h, hs, a, as_, c, cs);
            }
        }
    }
}

#[test]
#[ignore = "report"]
fn units_option_a() {
    let policies = vec![
        Policy::none(),
        Policy::units(3),
        Policy::units(30),
        Policy::evacuate(0),
        Policy::evacuate_and_units(0, 3),
    ];
    table(&policies, &[1.0, 2.0, 4.0, 8.0]);
}

fn table(policies: &[Policy], xs: &[f32]) {
    let policies = policies.to_vec();
    let variants: Vec<Variant> = xs.iter().map(|&x| Variant { unit_effect: UnitEffect::all(x), ..Variant::default() }).collect();
    let rs = run_grid(&data_dir(), &ALL, &policies, &variants, 1..=16);
    for town in ALL {
        println!("\n== {town}   (mean over 16 drawn seeds; sd in brackets)");
        println!("{:<26} {:>3} | {:>14} {:>14} {:>14} {:>14}", "policy", "x", "homes hit", "hectares", "caught", "secure");
        for (pi, p) in policies.iter().enumerate() {
            for (vi, x) in xs.iter().enumerate() {
                let s = summarise(&rs, town, pi, vi);
                println!(
                    "{:<26} {:>3} | {:>6.1} ({:>4.1}) {:>6.1} ({:>4.1}) {:>6.1} ({:>4.1}) {:>6.1} ({:>4.1})",
                    p.name, x, s.homes_lost.mean, s.homes_lost.sd, s.hectares.mean, s.hectares.sd,
                    s.caught.mean, s.caught.sd, s.secure.mean, s.secure.sd
                );
            }
        }
        println!("  paired vs `{}` (x{}): mean difference +- s.e. over seeds", policies[0].name, xs[0]);
        for (pi, p) in policies.iter().enumerate().skip(1) {
            for (vi, x) in xs.iter().enumerate() {
                let (h, hs) = paired(&rs, town, (pi, vi), (0, 0), |o| o.homes_lost as f32);
                let (a, as_) = paired(&rs, town, (pi, vi), (0, 0), |o| o.hectares);
                let (c, cs) = paired(&rs, town, (pi, vi), (0, 0), |o| o.caught as f32);
                println!("  {:<26} {:>3} | homes {:>+6.1} +-{:>4.1}  ha {:>+6.1} +-{:>4.1}  caught {:>+5.1} +-{:>4.1}", p.name, x, h, hs, a, as_, c, cs);
            }
        }
        let _ = regret(&rs, town, 0, 0);
    }
}
