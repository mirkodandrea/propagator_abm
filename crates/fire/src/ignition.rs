//! Choosing where a scenario starts.
//!
//! This is scenario design expressed as code, and it took three attempts to
//! get right. Each failure mode is worth keeping written down, because each
//! one produces a fire that runs perfectly well and still makes a useless
//! scenario:
//!
//! 1. **Ridge top.** Under a tramontana the wind pushes the fire downslope
//!    while the slope resists it. The fire crawls — 7 ha in two hours — and
//!    never reaches anyone.
//! 2. **Nearest fuel to any house.** Puts the fire on the seaward edge of the
//!    settlement, where it runs into open ground. 117 ha, and two households
//!    exposed.
//! 3. **Most downwind housing, ignoring fuel.** Picks an isolated grass pocket
//!    surrounded by streets and gardens. The fire suffocates at 3.5 ha.
//!
//! So a usable ignition needs all three at once: enough fuel to support the
//! starting patch, a continuous fuel corridor running downwind, and housing at
//! the far end of that corridor.

use scenario::{Cell, Scenario};

/// Where the fire starts and how big it already is.
#[derive(Debug, Clone, Copy)]
pub struct IgnitionPlan {
    pub centre: Cell,
    pub radius_m: f32,
    /// Households sitting in the downwind corridor — the population the
    /// scenario actually puts at risk.
    pub households_downwind: usize,
    /// Fraction of the downwind corridor that is burnable.
    pub corridor_fuel: f32,
}

/// How far downwind the scenario is expected to matter.
const CORRIDOR_M: f32 = 2200.0;
/// Housing closer than this is *worse*, not better. A fire lit right on the
/// wildland-urban edge sits in fuel already broken up by streets and gardens,
/// so it never builds a front: measured, such a start burned 11 ha in two
/// hours and peaked at 32 households threatened. Starting ~600 m back in
/// continuous fuel and letting the fire arrive with momentum gave 59 ha and
/// 170 households on the same map.
const STANDOFF_M: f32 = 350.0;
/// Half-width of that corridor.
const CORRIDOR_HALF_W: f32 = 500.0;

/// Pick an ignition for the given wind, maximising threatened housing subject
/// to the fire actually being able to get there.
pub fn plan(scn: &Scenario, wind_from_deg: f64, radius_m: f32) -> IgnitionPlan {
    plan_with_standoff(scn, wind_from_deg, radius_m, STANDOFF_M)
}

/// As [`plan`], with the minimum distance to housing given explicitly.
pub fn plan_with_standoff(
    scn: &Scenario,
    wind_from_deg: f64,
    radius_m: f32,
    standoff_m: f32,
) -> IgnitionPlan {
    let w = scn.world;
    // Wind blows *from* `wind_from_deg`, so the fire runs toward the
    // reciprocal bearing. World frame is +x east, +y north.
    let toward = ((wind_from_deg + 180.0).to_radians()) as f32;
    let dir = [toward.sin(), toward.cos()];

    let patch_cells = (radius_m / w.cellsize).ceil() as usize;
    let margin = patch_cells + 2;

    let mut best = IgnitionPlan {
        centre: Cell { row: w.fire_rows / 2, col: w.fire_cols / 2 },
        radius_m,
        households_downwind: 0,
        corridor_fuel: 0.0,
    };
    let mut best_score = f32::NEG_INFINITY;

    for row in (margin..w.fire_rows - margin).step_by(3) {
        for col in (margin..w.fire_cols - margin).step_by(3) {
            let c = Cell { row, col };
            if !scn.is_burnable(c) {
                continue;
            }

            // (a) enough fuel under the starting patch itself
            let patch_fuel = burnable_fraction_disc(scn, c, patch_cells);
            if patch_fuel < 0.7 {
                continue;
            }

            // (b) a continuous fuel corridor running downwind
            let p = w.centre_of(c);
            let corridor_fuel = corridor_burnable_fraction(scn, p, dir);
            if corridor_fuel < 0.45 {
                continue;
            }

            // (c) housing at the far end of it
            let mut reachable = 0usize;
            let mut weight = 0.0f32;
            for h in &scn.population.households {
                let vx = h.pos[0] - p.x;
                let vy = h.pos[1] - p.y;
                let along = vx * dir[0] + vy * dir[1];
                if along <= standoff_m || along > CORRIDOR_M {
                    continue;
                }
                let across = (vx * -dir[1] + vy * dir[0]).abs();
                if across > CORRIDOR_HALF_W {
                    continue;
                }
                reachable += 1;
                weight += 1.0;
            }
            if reachable == 0 {
                continue;
            }

            // Corridor continuity is squared: a gappy corridor does not just
            // slow the fire, it stops it, so it should dominate raw exposure
            // counts rather than trade off linearly against them.
            let score = weight * corridor_fuel * corridor_fuel * patch_fuel;
            if score > best_score {
                best_score = score;
                best = IgnitionPlan {
                    centre: c,
                    radius_m,
                    households_downwind: reachable,
                    corridor_fuel,
                };
            }
        }
    }
    best
}

fn burnable_fraction_disc(scn: &Scenario, centre: Cell, r: usize) -> f32 {
    let w = scn.world;
    let (mut hit, mut total) = (0u32, 0u32);
    let ri = r as isize;
    for dr in -ri..=ri {
        for dc in -ri..=ri {
            if dr * dr + dc * dc > ri * ri {
                continue;
            }
            let row = centre.row as isize + dr;
            let col = centre.col as isize + dc;
            if row < 0 || col < 0 || row as usize >= w.fire_rows || col as usize >= w.fire_cols {
                continue;
            }
            total += 1;
            if scn.is_burnable(Cell { row: row as usize, col: col as usize }) {
                hit += 1;
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        hit as f32 / total as f32
    }
}

/// Sample the downwind corridor for burnable fuel.
fn corridor_burnable_fraction(scn: &Scenario, from: scenario::Pos, dir: [f32; 2]) -> f32 {
    let w = scn.world;
    let (mut hit, mut total) = (0u32, 0u32);
    let mut along = 60.0f32;
    while along < CORRIDOR_M {
        let mut across = -CORRIDOR_HALF_W;
        while across <= CORRIDOR_HALF_W {
            let p = scenario::Pos {
                x: from.x + dir[0] * along - dir[1] * across,
                y: from.y + dir[1] * along + dir[0] * across,
            };
            if w.contains(p) {
                total += 1;
                if scn.is_burnable(w.cell_of(p)) {
                    hit += 1;
                }
            }
            across += 100.0;
        }
        along += 100.0;
    }
    if total == 0 {
        0.0
    } else {
        hit as f32 / total as f32
    }
}
