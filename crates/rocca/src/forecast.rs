//! «Dove andrà il fuoco»: a stochastic forecast of the fire, issued when the
//! weather service announces a change of wind and again when it happens
//! (user, 2026-10-13).
//!
//! What it knows is what the player could know: the fire **now** (the core's
//! own state, [`fire::ensemble::Snapshot`]), the wind now, and the change
//! the forecast has announced ([`crate::crisis::FORECAST_LEAD_S`] ahead).
//! Nothing else the simulation will do is read.
//!
//! [`MEMBERS`] runs of the same propagator go forward [`HORIZONS_S`], each
//! with its own seed and a wind perturbed around the forecast: the wind now
//! by ±[`NOW_DIR_DEG`] and ±[`NOW_SPEED`], the announced wind by
//! ±[`LATER_DIR_DEG`] and ±[`LATER_SPEED`], its time by ±[`LATER_AT_S`].
//! **Senza interventi**: the members carry the lines already cut and the
//! water already dropped, but no crew works in them from now on, so the
//! forecast is pessimistic where the units are working. Per cell, the share
//! of members in which it has burnt by +1 h and by +3 h; the 50 % contour
//! is the isochrone the map draws.
//!
//! Deterministic: the members' seeds and winds come from the game's seed and
//! the time of issue only.

use anyhow::Result;
use fire::ensemble::{Member, Snapshot};
use fire::Weather;

use crate::crisis::FORECAST_LEAD_S;
use crate::Game;

/// Members of the ensemble (constant: the forecast's resolution is 1/16).
pub const MEMBERS: usize = 16;
/// The forecast's horizons from the time of issue, simulated seconds.
pub const HORIZONS_S: [i64; 2] = [3600, 3 * 3600];
/// The share of members a cell must have burnt in to be inside the isochrone.
pub const LIKELY: f32 = 0.5;
/// Spread of the wind now (observed: small), degrees and fraction of speed.
pub const NOW_DIR_DEG: f64 = 10.0;
pub const NOW_SPEED: f64 = 0.10;
/// Spread of the announced wind: direction, speed, and when it comes.
pub const LATER_DIR_DEG: f64 = 20.0;
pub const LATER_SPEED: f64 = 0.20;
pub const LATER_AT_S: i64 = 10 * 60;
/// How long after the wind has turned the map keeps the forecast.
pub const SHOWN_AFTER_SHIFT_S: i64 = 30 * 60;

/// Why a forecast is due: the boundary conditions the player knows changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Epoch {
    /// The weather service has announced the change of wind.
    Announced,
    /// The wind has turned.
    Shifted,
}

/// The forecast: per fire cell, the share of members burnt by each horizon.
#[derive(Debug, Clone)]
pub struct Forecast {
    pub epoch: Epoch,
    /// Simulated second of issue; the horizons count from here.
    pub issued_s: i64,
    pub members: usize,
    pub rows: usize,
    pub cols: usize,
    /// `[+1 h, +3 h]`, row-major on the fire grid (row 0 north), 0-1.
    pub prob: [Vec<f32>; 2],
}

impl Forecast {
    /// Whether cell `i` is inside horizon `h`'s isochrone.
    pub fn likely(&self, h: usize, i: usize) -> bool {
        self.prob[h][i] >= LIKELY
    }

    /// Cells inside horizon `h`'s isochrone.
    pub fn likely_cells(&self, h: usize) -> usize {
        self.prob[h].iter().filter(|&&p| p >= LIKELY).count()
    }
}

/// The members still to run, and what the finished ones burnt. Owns its
/// data: run it here ([`Job::run`]), on another thread, or a member per
/// frame ([`Job::step`]) where there are no threads.
pub struct Job {
    pub epoch: Epoch,
    snapshot: Snapshot,
    members: Vec<Member>,
    done: usize,
    counts: [Vec<u16>; 2],
}

/// splitmix64: a few good bits from a seed, no dependency.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform in [-1, 1).
fn signed(z: u64) -> f64 {
    (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// Stratified offsets in [-1, 1): member `k` of `n` draws within its own
/// slice, shuffled per quantity, so 16 members cover the spread evenly.
fn stratified(k: usize, n: usize, key: u64) -> f64 {
    // a permutation of the slices by sorting on hashes
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&j| mix(key ^ (j as u64).wrapping_mul(0x2545_F491_4F6C_DD1D)));
    let slice = order[k] as f64;
    let jitter = (signed(mix(key ^ 0xA5A5 ^ k as u64)) + 1.0) / 2.0;
    (slice + jitter) / n as f64 * 2.0 - 1.0
}

fn turn(w: Weather, dir_deg: f64, speed: f64) -> Weather {
    Weather { wind_dir_deg: (w.wind_dir_deg + dir_deg).rem_euclid(360.0), wind_speed_kmh: (w.wind_speed_kmh * (1.0 + speed)).max(0.0), ..w }
}

impl Game {
    /// The forecast the player should see now, if any: from the announcement
    /// of the case's change of wind ([`FORECAST_LEAD_S`] ahead) until
    /// [`SHOWN_AFTER_SHIFT_S`] after it. A new value is a new forecast due;
    /// `None` is none to show.
    pub fn forecast_epoch(&self) -> Option<Epoch> {
        let (at, _) = self.territory.shifted(&self.case)?;
        let now = self.time_s();
        if now < at - FORECAST_LEAD_S || now >= at + SHOWN_AFTER_SHIFT_S {
            None
        } else if now < at {
            Some(Epoch::Announced)
        } else {
            Some(Epoch::Shifted)
        }
    }

    /// The ensemble for the forecast due now ([`Game::forecast_epoch`]), or
    /// for the wind as it is when none is due. Reads the state, changes
    /// nothing.
    pub fn forecast_job(&self) -> Result<Job> {
        let epoch = self.forecast_epoch().unwrap_or(Epoch::Shifted);
        let now = self.time_s();
        let snapshot = self.fire.snapshot(&self.scn)?;
        // the wind as it is known now: the game turns the fire's wind at the
        // start of the step after the shift's time, the forecast at the time
        let weather = match self.territory.shifted(&self.case) {
            Some((at, w)) if now >= at => w,
            _ => self.fire.weather(),
        };
        // the change still to come, as announced; none once it has happened
        let later = self.territory.shifted(&self.case).filter(|&(at, _)| epoch == Epoch::Announced && at > now);
        let key = mix(self.seed ^ mix(now as u64 ^ 0xF0EC_A57));
        let q = |k: usize, what: u64| stratified(k, MEMBERS, mix(key ^ what));
        let members = (0..MEMBERS)
            .map(|k| Member {
                seed: mix(key ^ (k as u64 + 1)),
                now: turn(weather, q(k, 1) * NOW_DIR_DEG, q(k, 2) * NOW_SPEED),
                later: later.map(|(at, w)| {
                    let at = (at + (q(k, 5) * LATER_AT_S as f64).round() as i64).max(now);
                    (at, turn(w, q(k, 3) * LATER_DIR_DEG, q(k, 4) * LATER_SPEED))
                }),
            })
            .collect();
        let n = snapshot.rows * snapshot.cols;
        Ok(Job { epoch, snapshot, members, done: 0, counts: [vec![0; n], vec![0; n]] })
    }

    /// The forecast due now, computed here (headless; the kiosk runs
    /// [`Job`] off the frame).
    pub fn forecast(&self) -> Result<Forecast> {
        self.forecast_job()?.run()
    }
}

impl Job {
    pub fn issued_s(&self) -> i64 {
        self.snapshot.time_s
    }

    /// The same ensemble as if no change of wind had been announced: the
    /// baseline that shows what the announcement adds (`examples/previsione.rs`).
    pub fn without_change(mut self) -> Job {
        for m in &mut self.members {
            m.later = None;
        }
        self
    }

    pub fn is_done(&self) -> bool {
        self.done == self.members.len()
    }

    fn add(&mut self, arrival: &[i32]) {
        let t0 = self.snapshot.time_s;
        for (h, &dt) in HORIZONS_S.iter().enumerate() {
            let until = (t0 + dt) as i32;
            for (c, &t) in self.counts[h].iter_mut().zip(arrival) {
                *c += (t != i32::MIN && t <= until) as u16;
            }
        }
    }

    /// Run the next member. Returns whether all are done.
    pub fn step(&mut self) -> Result<bool> {
        if let Some(m) = self.members.get(self.done).copied() {
            let a = self.snapshot.run(&m, HORIZONS_S[1])?;
            self.add(&a);
            self.done += 1;
        }
        Ok(self.is_done())
    }

    /// Run every member left, on all cores where there are threads; the
    /// result does not depend on how many.
    pub fn run(mut self) -> Result<Forecast> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let left: Vec<Member> = self.members[self.done..].to_vec();
            let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, left.len().max(1));
            let chunk = left.len().div_ceil(threads).max(1);
            let snap = &self.snapshot;
            let runs: Vec<Result<Vec<Vec<i32>>>> = std::thread::scope(|s| {
                let hs: Vec<_> = left.chunks(chunk).map(|ms| s.spawn(move || ms.iter().map(|m| snap.run(m, HORIZONS_S[1])).collect())).collect();
                hs.into_iter().map(|h| h.join().expect("forecast member panicked")).collect()
            });
            for r in runs {
                for a in r? {
                    self.add(&a);
                    self.done += 1;
                }
            }
        }
        while !self.step()? {}
        Ok(self.finish())
    }

    /// The forecast from the members run so far.
    pub fn finish(self) -> Forecast {
        let n = self.done.max(1) as f32;
        let [a, b] = self.counts;
        let p = |c: Vec<u16>| c.into_iter().map(|c| c as f32 / n).collect();
        Forecast {
            epoch: self.epoch,
            issued_s: self.snapshot.time_s,
            members: self.done,
            rows: self.snapshot.rows,
            cols: self.snapshot.cols,
            prob: [p(a), p(b)],
        }
    }
}
