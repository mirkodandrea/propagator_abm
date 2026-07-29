//! Where the fire is *likely* to go next.
//!
//! The game runs a single realization (see the module docs on [`crate`]), so
//! there is no ensemble to average and therefore no burn-probability map of
//! the usual PROPAGATOR kind. What still exists, and is arguably more useful
//! to a commander watching a going fire, is the **one-step spread
//! probability**: for every unburnt cell touching the active front, the
//! probability that the front carries into it on the next transition.
//!
//! That is exactly the quantity the kernel draws its Bernoulli trial against,
//! so this is not an invented heuristic -- it is the model's own probability,
//! evaluated over the whole front instead of sampled once. Combining the eight
//! neighbours as independent trials gives
//!
//! ```text
//! P(cell) = 1 - prod_over_burning_neighbours(1 - p_ij)
//! ```
//!
//! The formulas below are a port of `propagator-core`'s `models.rs`
//! (`w_h_effect`, `w_h_effect_on_probability`, `p_moisture`,
//! `probability_to_neighbour`), which are private to that crate. They are pure
//! functions of published constants; `hazard_matches_kernel_shape` in
//! `tests/hazard.rs` pins the properties that matter (downwind > upwind,
//! upslope > downslope, monotone in moisture) so a drift in either copy shows
//! up as a test failure rather than as a quietly wrong overlay.

use propagator_core::FuelSystem;
use scenario::{Cell, World};

use crate::Weather;

/// The 8 neighbours, in the kernel's order and geometry.
const NEIGHBOURS: [(i64, i64); 8] = [
    (-1, -1), (-1, 0), (-1, 1),
    (0, -1),           (0, 1),
    (1, -1),  (1, 0),  (1, 1),
];

// Empirical wind/slope constants of the core's `Standard` effect model.
const D1: f64 = 0.5;
const D2: f64 = 1.4;
const D3: f64 = 8.2;
const D4: f64 = 2.0;
const D5: f64 = 50.0;

/// Fuel moisture fraction at which spread ceases.
const MOISTURE_OF_EXTINCTION: f64 = 0.3;

#[inline]
fn clip(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

#[inline]
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Combined wind+slope scale factor. `angle` and `w_dir` in radians, with the
/// kernel's meteorological convention: angle 0 is propagation toward south,
/// `w_dir` is the bearing the wind blows *from*.
fn w_h_effect(angle: f64, w_speed: f64, w_dir: f64, dh: f64, dist: f64) -> f64 {
    let module = (1.0 - D1 * D2 * (-D4).tanh())
        + D1 * (D2 * (w_speed / D3 - D4).tanh())
        + w_speed / D5;
    let a = (module - 1.0) / 4.0;
    let w_effect_on_direction =
        (a + 1.0) * (1.0 - a * a) / (1.0 - a * (w_dir - angle).cos());
    let slope = dh / dist;
    let h_effect = 2f64.powf(((slope * 3.0).powi(2) * sign(slope)).tanh());
    h_effect * w_effect_on_direction
}

fn w_h_effect_on_probability(
    angle: f64,
    w_speed: f64,
    w_dir: f64,
    dh: f64,
    dist: f64,
) -> f64 {
    let mut wh = w_h_effect(angle, clip(w_speed, 0.0, 60.0), w_dir, dh, dist) - 1.0;
    if wh > 0.0 {
        wh /= 2.13;
    } else if wh < 0.0 {
        wh /= 1.12;
    }
    wh + 1.0
}

/// Trucchia moisture correction; `moist` is a fraction.
fn p_moisture(moist: f64) -> f64 {
    let x = moist / MOISTURE_OF_EXTINCTION;
    let p = -11.507 * x.powi(5) + 22.963 * x.powi(4) - 17.331 * x.powi(3)
        + 6.598 * x.powi(2)
        - 1.7211 * x
        + 1.0003;
    clip(p, 0.0, 1.0)
}

/// Probability that a burning cell carries into one neighbour.
pub fn probability_to_neighbour(
    angle: f64,
    dist: f64,
    w_dir: f64,
    w_speed: f64,
    moist: f64,
    dh: f64,
    transition_probability: f64,
) -> f64 {
    let alpha = w_h_effect_on_probability(angle, w_speed, w_dir, dh, dist).max(0.0);
    let p = 1.0 - (1.0 - transition_probability).powf(alpha);
    clip(p * p_moisture(moist), 0.0, 1.0)
}

/// One-step spread probability per cell, in `[0, 1]`.
///
/// Zero everywhere except in the unburnt fringe around the active front, which
/// is what makes it cheap enough to recompute on every advance: the work is
/// proportional to the perimeter, not to the grid.
pub struct HazardField {
    p: Vec<f32>,
    /// Indices written last update, so the next one clears only those.
    touched: Vec<u32>,
    /// Per-cell fuel table index, precomputed once.
    fuel_idx: Vec<i32>,
    /// Cells the kernel will even consider: non-vegetated codes are mapped to
    /// a fallback fuel index rather than to -1, so the index grid alone cannot
    /// tell burnable ground from a car park.
    burnable: Vec<bool>,
    dem: Vec<f64>,
    fuels: FuelSystem,
    world: World,
}

impl HazardField {
    pub fn new(scn: &scenario::Scenario, fuels: FuelSystem) -> anyhow::Result<HazardField> {
        let world = scn.world;
        let (rows, cols) = (world.fire_rows, world.fire_cols);
        let veg = propagator_core::Grid2::from_vec(rows, cols, scn.fuel.clone());
        let fuel_idx = fuels
            .build_fuel_index_grid(&veg)
            .map_err(|e| anyhow::anyhow!("{e:?}"))?
            .as_slice()
            .to_vec();
        let burnable = (0..rows * cols)
            .map(|i| scn.is_burnable(Cell { row: i / cols, col: i % cols }))
            .collect();

        Ok(HazardField {
            p: vec![0.0; rows * cols],
            touched: Vec::new(),
            fuel_idx,
            burnable,
            dem: scn.dem.clone(),
            fuels,
            world,
        })
    }

    pub fn get(&self, c: Cell) -> f32 {
        self.p[c.row * self.world.fire_cols + c.col]
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.p
    }

    /// Highest one-step probability anywhere on the fringe, for the HUD.
    pub fn peak(&self) -> f32 {
        self.touched
            .iter()
            .map(|&i| self.p[i as usize])
            .fold(0.0, f32::max)
    }

    /// Recompute the fringe from the current front.
    ///
    /// `burnt` is the flat fire state: any cell already alight is skipped, so
    /// the field shows only ground the fire has yet to take.
    pub fn update(&mut self, active: &[Cell], state: &[crate::CellFire], weather: Weather) {
        for &i in &self.touched {
            self.p[i as usize] = 0.0;
        }
        self.touched.clear();

        let (rows, cols) = (self.world.fire_rows, self.world.fire_cols);
        let cellsize = self.world.cellsize as f64;
        let w_dir = weather.wind_dir_deg.to_radians();
        let w_speed = weather.wind_speed_kmh;
        let moist = weather.moisture_pct / 100.0;

        for src in active {
            let from = self.fuel_idx[src.row * cols + src.col];
            if from < 0 || !self.burnable[src.row * cols + src.col] {
                continue;
            }
            let h_from = self.dem[src.row * cols + src.col];

            for &(dr, dc) in &NEIGHBOURS {
                let (r, c) = (src.row as i64 + dr, src.col as i64 + dc);
                if r < 0 || c < 0 || r >= rows as i64 || c >= cols as i64 {
                    continue;
                }
                let i = r as usize * cols + c as usize;
                if state[i] != crate::CellFire::Unburnt {
                    continue;
                }
                let to = self.fuel_idx[i];
                if to < 0 || !self.burnable[i] {
                    continue;
                }

                // Same geometry as the kernel: angle 0 is propagation south.
                let angle = ((dc as f64).atan2(-(dr as f64)) + std::f64::consts::PI)
                    .rem_euclid(std::f64::consts::TAU);
                let dist = ((dr * dr + dc * dc) as f64).sqrt() * cellsize;
                let dh = self.dem[i] - h_from;
                let p0 = self.fuels.transition_probability(from as usize, to as usize);
                if p0 <= 0.0 {
                    continue;
                }

                let p =
                    probability_to_neighbour(angle, dist, w_dir, w_speed, moist, dh, p0) as f32;
                if p <= 0.0 {
                    continue;
                }
                if self.p[i] == 0.0 {
                    self.touched.push(i as u32);
                }
                // Independent trials from each burning neighbour.
                self.p[i] = 1.0 - (1.0 - self.p[i]) * (1.0 - p);
            }
        }
    }
}
