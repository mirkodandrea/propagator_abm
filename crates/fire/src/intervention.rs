//! What crews, engines and aircraft actually do to the fire.
//!
//! Both routes into the core already exist as boundary-condition fields, so
//! suppression is expressed in the model's own terms rather than bolted on:
//! `vegetation_changes` removes fuel, `additional_moisture` wets it.
//!
//! **Suppression acts on the fuel ahead of the front, never on the flames.**
//! A cell the core has already lit stays lit — burn-out is this crate's own
//! ageing layer (`BURNOUT_S`), not something the kernel models — so wetting or
//! clearing a burning cell changes nothing at all. What both interventions do
//! is make the *next* cell harder to reach. That is also how real initial
//! attack works, and it is why the unit model aims at unburnt burnable fuel:
//! see [`Intervention::useful_cells`].
//!
//! ### The two magic numbers, and where they come from
//!
//! [`MOISTURE_POINTS_PER_LITRE`] is the one that decides whether aircraft
//! matter. The core takes `additional_moisture` in percentage points of fuel
//! moisture, accumulates it, decays it at 1%/minute, and stops spread once
//! effective moisture passes its 30% moisture of extinction. So "does a
//! 6,000 L drop stop this fire" is entirely a question of how many points a
//! litre buys. Fine fuel load in Mediterranean shrub is order 1 kg/m²,
//! moisture content is water mass over dry fuel mass, and perhaps a third of a
//! drop reaches the fine fuel that carries fire rather than the canopy, the
//! ground or the air — so 1 L/m² is ~30 points, not the ~1 point the first
//! version of this file assumed. At 30:
//!
//! | Action | Coverage | Moisture added | Effect |
//! |---|---|---|---|
//! | Canadair, 6,137 L over 60 × 220 m | 0.46 L/m² | +14 pts | 6% → 20%: slowed, holds nothing alone |
//! | Two overlapping drops | 0.93 L/m² | +28 pts | 6% → 34%: past extinction, briefly |
//! | Engine emptying its tank over one cell | 6 L/m² | saturated | that cell is out |
//!
//! Which is the operationally honest answer: drops buy time for the ground
//! crews, and nothing an engine can carry holds a wind-driven front.
//!
//! [`CLEARED_FUEL_CODE`] is duller but was the source of a real bug. The
//! core's `vegetation_changes` grid is *sparse by NaN*: every non-NaN cell
//! **sets** that cell's fuel class. Filling the grid with zeros and writing
//! the line into it therefore reclassified the whole 512 × 512 window as
//! non-vegetated — a "fireline" that stopped the fire everywhere at once, and
//! looked plausible in a test that only asserted the fire got smaller.

use scenario::{Cell, Pos, World};

/// Percentage points of fuel moisture bought by one litre per square metre.
/// See the module note for the derivation; the core's moisture of extinction
/// is 30 points, which is the number to compare against.
pub const MOISTURE_POINTS_PER_LITRE: f32 = 30.0;

/// `eu_fuel12` class written into a cleared cell. Both -1 and 0 are
/// `burn = false` in the shipped table; 0 is what the raster already uses for
/// rock, road and roof, so a cut line reads as the same thing as a car park.
pub const CLEARED_FUEL_CODE: f64 = 0.0;

/// Cap on accumulated added moisture, percentage points. The core clamps
/// effective moisture to 100% itself; this keeps the accounting bounded while
/// it is still in the units the UI reports.
pub const MAX_ADDED_MOISTURE_PTS: f32 = 100.0;

#[derive(Debug, Clone, Copy)]
pub enum InterventionKind {
    /// Fuel cleared to bare ground -- hand line, dozer line, or an existing
    /// break being widened. Permanent for the rest of the run.
    Fireline,
    /// Water or retardant applied to the cell. The core decays added moisture
    /// at 1% per minute, so this is temporary, with a ~69 min half life: long
    /// enough to matter for an initial attack, short enough that a line held
    /// only by water does not hold all afternoon.
    Water { litres_per_m2: f64 },
}

#[derive(Debug, Clone)]
pub struct Intervention {
    pub kind: InterventionKind,
    pub cells: Vec<Cell>,
}

impl Intervention {
    pub fn fireline(cells: Vec<Cell>) -> Intervention {
        Intervention { kind: InterventionKind::Fireline, cells }
    }

    /// Water applied at a given depth. An engine working off its tank lays
    /// several litres per square metre over the couple of cells its hose
    /// reaches; an air drop spreads a much larger load much thinner.
    pub fn water(cells: Vec<Cell>, litres_per_m2: f64) -> Intervention {
        Intervention { kind: InterventionKind::Water { litres_per_m2 }, cells }
    }

    /// Total litres this action represents, for the debrief and the UI.
    pub fn litres(&self, world: &World) -> f64 {
        match self.kind {
            InterventionKind::Fireline => 0.0,
            InterventionKind::Water { litres_per_m2 } => {
                let cell_m2 = (world.cellsize * world.cellsize) as f64;
                litres_per_m2 * cell_m2 * self.cells.len() as f64
            }
        }
    }

    /// Keep only the cells where an action would do anything: burnable, and
    /// not already alight or burnt.
    ///
    /// Not an optimisation. Applying water to the flaming front is the single
    /// most intuitive wrong thing to do here — it reads as fighting the fire
    /// and changes nothing, because the kernel never un-lights a cell. Keeping
    /// the filter in one place means the unit model, the tests and any future
    /// scripted intervention agree on what "useful" means.
    pub fn useful_cells(
        cells: impl IntoIterator<Item = Cell>,
        burnable: impl Fn(Cell) -> bool,
        unburnt: impl Fn(Cell) -> bool,
    ) -> Vec<Cell> {
        cells
            .into_iter()
            .filter(|c| burnable(*c) && unburnt(*c))
            .collect()
    }
}

/// Cells whose centre lies within `radius_m` of `centre`, clipped to the grid.
pub fn cells_in_radius(world: &World, centre: Pos, radius_m: f32) -> Vec<Cell> {
    let c = world.cell_of(centre);
    let span = (radius_m / world.cellsize).ceil() as i64 + 1;
    let mut out = Vec::new();
    for dr in -span..=span {
        for dc in -span..=span {
            let (row, col) = (c.row as i64 + dr, c.col as i64 + dc);
            if row < 0
                || col < 0
                || row >= world.fire_rows as i64
                || col >= world.fire_cols as i64
            {
                continue;
            }
            let cell = Cell { row: row as usize, col: col as usize };
            let p = world.centre_of(cell);
            if (p.x - centre.x).powi(2) + (p.y - centre.y).powi(2) <= radius_m * radius_m {
                out.push(cell);
            }
        }
    }
    out
}

/// Cells within `half_width_m` of the segment `from` -> `to`.
///
/// This is the shape of every deliberate suppression action that is not a
/// point: a hand line, a dozer line, a retardant drop laid along a flank.
/// Working in metres and converting once keeps the 20 m grid out of the
/// caller, which is the rule the whole project runs on.
pub fn cells_along(world: &World, from: Pos, to: Pos, half_width_m: f32) -> Vec<Cell> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-3 {
        return cells_in_radius(world, from, half_width_m);
    }
    // Walk the segment at half a cell so none is stepped over. Deduping
    // through a linear scan rather than a set: a 500 m line touches a few
    // dozen cells, and the caller's fixed grid order matters more than the
    // asymptotics.
    let step = world.cellsize * 0.5;
    let n = (len / step).ceil().max(1.0) as usize;
    let mut out: Vec<Cell> = Vec::new();
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let p = Pos { x: from.x + dx * t, y: from.y + dy * t };
        for c in cells_in_radius(world, p, half_width_m) {
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}
