//! A session's weather and the forecast the player sees of it (spec 6.1).
//!
//! The draw is seeded, so the COMPARE twin, a retry and the tests reproduce it.
//! The forecast is deliberately *calibrated rather than clever*: it reports the
//! chance the draw rolled against, so it is right on average and wrong exactly
//! when the dice go the other way -- which is the lesson, acting under
//! uncertainty. A second issue, later, is better informed and can still
//! contradict the first.

use fire::Weather;

use crate::mission::{spec, Spec, WindShift};

/// Splitmix64: a few lines, no dependency, good enough for a draw.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

/// One session's realised weather, with the hidden quantities the forecast is
/// made from.
#[derive(Debug, Clone, Copy)]
pub struct Draw {
    /// The town's spec with this session's opening weather and shift (or none).
    pub spec: Spec,
    /// The chance of a shift this session was rolled against.
    pub shift_p: f32,
    pub seed: u64,
}

/// What the commander is shown. Never the truth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Forecast {
    /// 1 at the briefing, 2 about fifteen minutes in.
    pub issue: u8,
    pub wind_from_deg: f32,
    pub wind_kmh: f32,
    /// Half-width of the bearing uncertainty, degrees. Narrower on issue 2.
    pub cone_deg: f32,
    /// Chance of a change of wind, in 5% steps.
    pub shift_p: f32,
    /// Where the wind would come from after a shift.
    pub shift_to_deg: f32,
    /// Earliest and latest minute the change is expected.
    pub shift_eta_min: (u32, u32),
}

/// Simulated second at which forecast issue 2 is shown.
pub const ISSUE_2_AT_S: i64 = 15 * 60;

pub fn draw(id: &str, seed: u64) -> Option<Draw> {
    let base = spec(id)?;
    let mut r = Rng(seed ^ 0xA076_1D64_78BD_642F);
    let c = base.climate;
    let mut s = base;
    s.weather = Weather {
        wind_dir_deg: (base.weather.wind_dir_deg + r.range(-15.0, 15.0) as f64).rem_euclid(360.0),
        wind_speed_kmh: (base.weather.wind_speed_kmh + r.range(-5.0, 5.0) as f64).max(8.0),
        moisture_pct: (base.weather.moisture_pct + r.range(-1.5, 1.5) as f64).max(3.0),
    };
    let p = r.range(c.shift_p.0, c.shift_p.1);
    let shifts = r.unit() < p;
    let at_s = r.range(c.shift_window_s.0 as f32, c.shift_window_s.1 as f32) as i64 / 60 * 60;
    s.shift = shifts.then_some(WindShift { at_s, weather: c.shift_to });
    Some(Draw { spec: s, shift_p: p, seed })
}

impl Draw {
    /// The forecast for this session. Issue 1 states the odds the draw used;
    /// issue 2 leans toward what will happen but is still noisy.
    pub fn forecast(&self, issue: u8) -> Forecast {
        let mut r = Rng(self.seed ^ 0xE703_7ED1_A0B4_28DB ^ ((issue as u64) << 56));
        let c = self.spec.climate;
        let w = self.spec.weather;
        let truth = if self.spec.shift.is_some() { 1.0 } else { 0.0 };
        let p = match issue {
            1 => self.shift_p,
            _ => (0.55 * truth + 0.45 * self.shift_p + r.range(-0.15, 0.15)).clamp(0.02, 0.98),
        };
        let cone = if issue == 1 { 22.0 } else { 12.0 };
        let (lo, hi) = (c.shift_window_s.0 / 60, c.shift_window_s.1 / 60);
        // Issue 2 knows roughly when, if it is coming.
        let (a, b) = match (issue, self.spec.shift) {
            (2, Some(s)) => ((s.at_s / 60 - 6).max(lo / 2), s.at_s / 60 + 6),
            _ => (lo, hi),
        };
        Forecast {
            issue,
            wind_from_deg: (w.wind_dir_deg as f32 + r.range(-0.5, 0.5) * cone).rem_euclid(360.0),
            wind_kmh: (w.wind_speed_kmh as f32 + r.range(-3.0, 3.0)).max(5.0),
            cone_deg: cone,
            shift_p: (p * 20.0).round() / 20.0,
            shift_to_deg: c.shift_to.wind_dir_deg as f32,
            shift_eta_min: (a as u32, b as u32),
        }
    }
}
