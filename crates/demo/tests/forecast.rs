//! The weather draw and the forecast (spec 6.1): reproducible, calibrated,
//! wrong sometimes -- asserted as 'fires', not 'runs'.

use demo::{draw, Run, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

#[test]
fn a_draw_is_reproducible_and_varies() {
    for id in ALL {
        let a = draw(id, 7).unwrap();
        let b = draw(id, 7).unwrap();
        assert_eq!(a.spec.weather.wind_dir_deg, b.spec.weather.wind_dir_deg);
        assert_eq!(a.forecast(1), b.forecast(1), "{id}: a retry must see the same forecast");
        let winds: std::collections::HashSet<_> =
            (1..40).map(|s| draw(id, s).unwrap().spec.weather.wind_dir_deg.to_bits()).collect();
        assert!(winds.len() > 30, "{id}: the draw barely varies");
    }
}

/// The forecast is useful on average: sessions it gave a high chance to shift
/// far more often than those it gave a low one, and the overall rate matches.
#[test]
fn the_forecast_is_calibrated() {
    for id in ["demo_borgo", "demo_porto"] {
        let (mut hi, mut hi_n, mut lo, mut lo_n, mut mean_p, mut mean_s) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for seed in 1..=1500u64 {
            let d = draw(id, seed).unwrap();
            let f = d.forecast(1);
            let shifted = d.spec.shift.is_some() as u8 as f32;
            mean_p += f.shift_p; mean_s += shifted;
            if f.shift_p >= 0.6 { hi += shifted; hi_n += 1.0; }
            if f.shift_p <= 0.4 { lo += shifted; lo_n += 1.0; }
        }
        let (n, hi_r, lo_r) = (1500.0, hi / hi_n, lo / lo_n);
        println!("{id}: mean forecast {:.2} vs shifted {:.2}; high-p {hi_r:.2}, low-p {lo_r:.2}", mean_p / n, mean_s / n);
        assert!((mean_p - mean_s).abs() / n < 0.04, "{id}: forecast odds are off the real rate");
        assert!(hi_r > lo_r + 0.3, "{id}: the forecast carries no information");
    }
}

/// ...and wrong sometimes: it is odds, not a verdict.
#[test]
fn the_forecast_is_sometimes_wrong() {
    let (mut said_shift_none, mut said_none_shift) = (0, 0);
    for seed in 1..=500u64 {
        let d = draw("demo_borgo", seed).unwrap();
        let f = d.forecast(1);
        if f.shift_p >= 0.6 && d.spec.shift.is_none() { said_shift_none += 1; }
        if f.shift_p <= 0.35 && d.spec.shift.is_some() { said_none_shift += 1; }
    }
    assert!(said_shift_none >= 10 && said_none_shift >= 10, "{said_shift_none}/{said_none_shift}: too reliable to be a forecast");
}

/// The second issue is better informed than the first (lower Brier score) and
/// yet can still contradict it.
#[test]
fn the_second_issue_is_sharper_but_can_contradict() {
    let (mut b1, mut b2, mut flips) = (0.0, 0.0, 0);
    for seed in 1..=1000u64 {
        let d = draw("demo_porto", seed).unwrap();
        let y = d.spec.shift.is_some() as u8 as f32;
        let (f1, f2) = (d.forecast(1), d.forecast(2));
        b1 += (f1.shift_p - y).powi(2);
        b2 += (f2.shift_p - y).powi(2);
        if (f1.shift_p >= 0.5) != (f2.shift_p >= 0.5) { flips += 1; }
    }
    assert!(b2 < b1 * 0.8, "issue 2 is not better informed ({b2} vs {b1})");
    assert!(flips >= 30, "issue 2 never contradicts issue 1 ({flips})");
}

#[test]
fn a_drawn_session_plays_in_every_town() {
    for id in ALL {
        let d = draw(id, 3).unwrap();
        let o = Run::new(&data_dir(), d.spec, 3).unwrap().play(&[]).unwrap();
        assert!(o.hectares > 1.0, "{id}: the drawn fire did not establish");
    }
}

// "A shift changes who is at risk" is now per district:
// `districts::the_wind_decides_which_district_is_in_danger`.
