//! Danger anywhere on the map, not just at a house.
//!
//! [`StructureExposure`](crate::exposure) answers "is this building being
//! destroyed", which is a slow, integrated question asked at ~750 fixed
//! points. Agents need a different one: "is it survivable *here*, right now",
//! asked at arbitrary moving positions, several hundred times a step, and
//! about people rather than walls.
//!
//! So this is a coarse scalar field on the fire grid, splatted from the active
//! front once per fire update and then sampled for free. It reuses the same
//! intensity-driven reach as the exposure model -- flame length from Byram,
//! radiant reach at four flame lengths -- so the two layers cannot disagree
//! about how far a given fire can hurt you.
//!
//! Two things differ from structure exposure, both because the receiver is a
//! person and not a building:
//!
//! - **the radius is smaller.** Long-range embers destroy houses over hours;
//!   they do not kill someone walking down a road. Ember reach is therefore
//!   capped hard here ([`EMBER_CAP_M`]) while the exposure model lets it run
//!   to 2.5 km.
//! - **it is not integrated.** A person's exposure history lives on the
//!   person, because whether they can leave depends on where they have been.

use scenario::{Cell, Pos, World};

use crate::exposure::{flame_length_m, radiant_range_m};
use crate::{CellFire, Weather};

/// Ember reach for *people*, metres. Firebrands land far downwind and start
/// house fires hours later, but the lethal-to-a-pedestrian zone is much
/// tighter than the structure-ignition one, so this is capped well below the
/// exposure model's 2.5 km.
pub const EMBER_CAP_M: f32 = 400.0;

/// Danger at which an agent in the open is considered to be taking direct
/// flame contact and can no longer move through.
pub const IMPASSABLE: f32 = 0.55;

/// Danger above which an agent should be trying to leave, whatever else it
/// intended to do.
pub const ALARMING: f32 = 0.12;

/// Per-cell danger to a person in the open, 0-1.
///
/// 1.0 is inside the flaming front. The scale is deliberately not linear in
/// intensity: what matters to an evacuation is the shape of the survivable
/// region, and that is set by distance to flame rather than by kW/m.
pub struct ThreatField {
    danger: Vec<f32>,
    /// Fireline intensity of the hottest cell reaching each cell, kW/m.
    /// Carried through so the UI can say *why* somewhere is lethal.
    peak_fli: Vec<f32>,
    world: World,
    /// Cells touched by the last update, so clearing costs the front rather
    /// than the grid.
    dirty: Vec<u32>,
    /// Cells that have left `Unburnt` (reported by the owner through
    /// [`ThreatField::mark_burnt`]); they hold the burnt floor.
    burnt: Vec<u32>,
    /// Distance in metres by offset, `(2*tab_span+1)^2`, row-major in `dr`.
    dist_tab: Vec<f32>,
    /// Cosine to the wind by offset, valid for `dot_wind`.
    dot_tab: Vec<f32>,
    dot_wind: [u32; 2],
    tab_span: i64,
}

impl ThreatField {
    pub fn new(world: World) -> ThreatField {
        let n = world.fire_rows * world.fire_cols;
        ThreatField {
            danger: vec![0.0; n],
            peak_fli: vec![0.0; n],
            world,
            dirty: Vec::new(),
            burnt: Vec::new(),
            dist_tab: Vec::new(),
            dot_tab: Vec::new(),
            dot_wind: [0, 0],
            tab_span: -1,
        }
    }

    /// Record that a cell is no longer `Unburnt`; it keeps the burnt floor
    /// from the next update on. Call once per cell.
    pub fn mark_burnt(&mut self, i: usize) {
        self.burnt.push(i as u32);
    }

    /// Make the offset tables cover `span` cells, same arithmetic as the
    /// direct computation.
    fn ensure_tables(&mut self, span: i64, wind: [f32; 2]) {
        let cellsize = self.world.cellsize;
        let wbits = [wind[0].to_bits(), wind[1].to_bits()];
        let rebuild = span > self.tab_span;
        if rebuild {
            self.tab_span = span;
            let w = (2 * span + 1) as usize;
            self.dist_tab = vec![0.0; w * w];
            for dr in -span..=span {
                for dc in -span..=span {
                    let ex = dc as f32 * cellsize;
                    let ny = -dr as f32 * cellsize;
                    self.dist_tab[(dr + span) as usize * w + (dc + span) as usize] =
                        (ex * ex + ny * ny).sqrt();
                }
            }
        }
        if rebuild || wbits != self.dot_wind || self.dot_tab.is_empty() {
            self.dot_wind = wbits;
            let span = self.tab_span;
            let w = (2 * span + 1) as usize;
            self.dot_tab = vec![0.0; w * w];
            for dr in -span..=span {
                for dc in -span..=span {
                    let ex = dc as f32 * cellsize;
                    let ny = -dr as f32 * cellsize;
                    let k = (dr + span) as usize * w + (dc + span) as usize;
                    let d = self.dist_tab[k];
                    self.dot_tab[k] = (ex * wind[0] + ny * wind[1]) / d;
                }
            }
        }
    }

    pub fn danger(&self) -> &[f32] {
        &self.danger
    }

    /// Danger at a world position, 0-1. Outside the window is safe by
    /// definition -- leaving the map *is* the evacuation.
    pub fn at(&self, p: Pos) -> f32 {
        if !self.world.contains(p) {
            return 0.0;
        }
        let c = self.world.cell_of(p);
        self.danger[c.row * self.world.fire_cols + c.col]
    }

    pub fn at_cell(&self, c: Cell) -> f32 {
        self.danger[c.row * self.world.fire_cols + c.col]
    }

    /// Fireline intensity behind the danger at a position, kW/m.
    pub fn fli_at(&self, p: Pos) -> f32 {
        if !self.world.contains(p) {
            return 0.0;
        }
        let c = self.world.cell_of(p);
        self.peak_fli[c.row * self.world.fire_cols + c.col]
    }

    /// Set the danger at a position by hand, for tests: the next
    /// [`ThreatField::update`] clears it like any other.
    pub fn paint(&mut self, p: Pos, danger: f32) {
        if !self.world.contains(p) {
            return;
        }
        let c = self.world.cell_of(p);
        let i = c.row * self.world.fire_cols + c.col;
        self.danger[i] = danger;
        self.dirty.push(i as u32);
    }

    /// True where a person on foot or in a vehicle cannot pass.
    pub fn blocked(&self, p: Pos) -> bool {
        self.at(p) >= IMPASSABLE
    }

    /// Rebuild from the current front.
    pub fn update(
        &mut self,
        _state: &[CellFire],
        active: &[Cell],
        intensity: &[f32],
        weather: Weather,
    ) {
        for &i in &self.dirty {
            self.danger[i as usize] = 0.0;
            self.peak_fli[i as usize] = 0.0;
        }
        self.dirty.clear();
        if active.is_empty() {
            return;
        }

        let (rows, cols) = (self.world.fire_rows, self.world.fire_cols);
        let cellsize = self.world.cellsize;
        let toward = (weather.wind_dir_deg + 180.0).to_radians() as f32;
        // World frame: +x east, +y north. Row index grows southward, so the
        // northward component flips sign when stepping in rows.
        let wind = [toward.sin(), toward.cos()];
        let wind_kmh = weather.wind_speed_kmh as f32;

        // Largest reach any cell can have, to size the tables once.
        let mut max_span = 0i64;
        for cell in active {
            let fli = intensity[cell.row * cols + cell.col];
            if fli <= 0.0 {
                continue;
            }
            let r_rad = radiant_range_m(fli);
            let r_emb = ((flame_length_m(fli) * 12.0) * (1.0 + wind_kmh / 30.0))
                .min(EMBER_CAP_M);
            max_span = max_span.max((r_rad.max(r_emb) / cellsize).ceil() as i64);
        }
        self.ensure_tables(max_span, wind);
        let tspan = self.tab_span;
        let tw = (2 * tspan + 1) as usize;

        for cell in active {
            let idx = cell.row * cols + cell.col;
            let fli = intensity[idx];
            if fli <= 0.0 {
                continue;
            }
            let r_rad = radiant_range_m(fli);
            // Ember danger to a person tracks plume strength and wind like the
            // structure model, but is capped much harder: see EMBER_CAP_M.
            let r_emb = ((flame_length_m(fli) * 12.0) * (1.0 + wind_kmh / 30.0))
                .min(EMBER_CAP_M);
            let reach = r_rad.max(r_emb);
            let span = (reach / cellsize).ceil() as i64;

            let (cr, cc) = (cell.row as i64, cell.col as i64);
            let dr_lo = (-span).max(-cr);
            let dr_hi = span.min(rows as i64 - 1 - cr);
            let dc_lo = (-span).max(-cc);
            let dc_hi = span.min(cols as i64 - 1 - cc);
            for dr in dr_lo..=dr_hi {
                let trow = (dr + tspan) as usize * tw;
                let grow = ((cr + dr) * cols as i64) as usize;
                for dc in dc_lo..=dc_hi {
                    let t = trow + (dc + tspan) as usize;
                    let d = self.dist_tab[t];
                    if d > reach {
                        continue;
                    }
                    let i = grow + (cc + dc) as usize;

                    let mut v = 0.0f32;
                    if d <= r_rad {
                        // Squared falloff to the radiant safe-separation
                        // distance: at four flame lengths this is 0, inside
                        // the flame it saturates.
                        let near = 1.0 - d / r_rad;
                        v = v.max(near * near * 1.4);
                    }
                    if d <= r_emb && d > 1.0 {
                        let dot = self.dot_tab[t];
                        if dot > 0.0 {
                            v = v.max(dot.powi(2) * (1.0 - d / r_emb) * 0.5);
                        }
                    }
                    if v <= 0.0 {
                        continue;
                    }
                    if self.danger[i] == 0.0 {
                        self.dirty.push(i as u32);
                    }
                    if v > self.danger[i] {
                        self.danger[i] = v.min(1.0);
                    }
                    if fli > self.peak_fli[i] {
                        self.peak_fli[i] = fli;
                    }
                }
            }
        }

        // A cell that is itself alight is maximally dangerous whatever the
        // splat says, and a burnt-out cell is still no place to walk.
        for cell in active {
            let i = cell.row * cols + cell.col;
            if self.danger[i] == 0.0 {
                self.dirty.push(i as u32);
            }
            self.danger[i] = 1.0;
        }
        for &i in &self.burnt {
            let i = i as usize;
            if self.danger[i] < 0.25 {
                if self.danger[i] == 0.0 {
                    self.dirty.push(i as u32);
                }
                self.danger[i] = 0.25;
            }
        }
    }
}
