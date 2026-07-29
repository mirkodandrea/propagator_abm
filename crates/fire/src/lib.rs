//! The wildfire model, running in-process on the PROPAGATOR Rust core.
//!
//! The core is event-driven: it holds a per-realization min-heap of pending
//! ignitions and advances to the next scheduled event, so wall-clock cost
//! tracks the fire perimeter rather than the grid. [`FireSim::advance`] wraps
//! that in a fixed simulated-time budget, which is what a real-time game loop
//! actually wants.
//!
//! The game runs a **single realization**: a commander needs one concrete
//! fire to fight, not a probability cloud. The stochastic ensemble stays
//! useful for the debrief, where the same seed can be re-run many times.

use anyhow::{Context, Result};
use propagator_core::{
    BoundaryConditions, FieldInput, Grid2, Ignitions, OobMode, Propagator,
    PropagatorConfig,
};
use scenario::{Cell, Scenario, World};

pub mod exposure;
pub mod fuels;
pub mod hazard;
pub mod ignition;
pub mod intervention;

pub use exposure::{ExposureField, StructureExposure};
pub use hazard::HazardField;
pub use ignition::{plan as plan_ignition, plan_with_standoff, IgnitionPlan};
pub use intervention::{Intervention, InterventionKind};

/// Weather driving the fire. Applied as boundary conditions; changing any of
/// these mid-run schedules a new event.
#[derive(Debug, Clone, Copy)]
pub struct Weather {
    /// Degrees clockwise from north, the direction the wind blows *from*
    /// (meteorological convention -- the core's own units).
    pub wind_dir_deg: f64,
    pub wind_speed_kmh: f64,
    pub moisture_pct: f64,
}

impl Default for Weather {
    fn default() -> Self {
        // Tramontana: a north wind pushing fire off the ridges toward the coast.
        Weather { wind_dir_deg: 0.0, wind_speed_kmh: 35.0, moisture_pct: 6.0 }
    }
}

/// Per-cell fire state, flattened for rendering and for the exposure model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CellFire {
    Unburnt = 0,
    Burning = 1,
    Burnt = 2,
}

pub struct FireSim {
    sim: Propagator,
    world: World,
    weather: Weather,
    /// Simulated seconds elapsed.
    time_s: i64,
    /// Flattened fire state, refreshed after each advance.
    state: Vec<CellFire>,
    /// Simulated time at which each cell first became `Burning`, for the
    /// burn-out timer and for arrival-time readouts.
    ignited_at: Vec<i32>,
    /// Cells that are still actively burning this tick.
    active: Vec<Cell>,
    /// Per-cell fireline intensity in kW/m, from the core. Drives how far
    /// each burning cell can threaten a structure.
    intensity: Vec<f32>,
    exposure: StructureExposure,
    /// One-step spread probability ahead of the front, for the hazard overlay.
    hazard: HazardField,
    pending: Vec<Intervention>,
}

/// How long a cell keeps radiating after ignition before it is spent. The core
/// itself has no notion of burn-out -- once a cell is lit it stays lit -- so
/// the game layer ages cells out to drive fire rendering and exposure.
const BURNOUT_S: i32 = 20 * 60;

impl FireSim {
    pub fn new(scn: &Scenario, weather: Weather, seed: u64) -> Result<FireSim> {
        let (rows, cols) = (scn.world.fire_rows, scn.world.fire_cols);

        let veg = Grid2::from_vec(rows, cols, scn.fuel.clone());
        let dem = Grid2::from_vec(rows, cols, scn.dem.clone());

        let fuel_system = fuels::eu12_fuel_system(&scn.fuel_defs)?;
        let hazard = HazardField::new(scn, fuel_system.clone())?;

        let mut config = PropagatorConfig::new(veg, dem);
        config.fuels = Some(fuel_system);
        config.cellsize = scn.world.cellsize as f64;
        config.realizations = 1;
        config.do_spotting = true;
        config.seed = Some(seed);
        // The scenario window is fixed; a fire reaching the edge should stop
        // there rather than abort the run.
        config.oob_mode = OobMode::Ignore;

        let sim = Propagator::new(config).context("building PROPAGATOR core")?;

        let n = rows * cols;
        Ok(FireSim {
            sim,
            world: scn.world,
            weather,
            time_s: 0,
            state: vec![CellFire::Unburnt; n],
            ignited_at: vec![i32::MIN; n],
            active: Vec::new(),
            intensity: vec![0.0; n],
            exposure: StructureExposure::new(scn),
            hazard,
            pending: Vec::new(),
        })
    }

    pub fn time_s(&self) -> i64 {
        self.time_s
    }

    pub fn weather(&self) -> Weather {
        self.weather
    }

    pub fn state(&self) -> &[CellFire] {
        &self.state
    }

    pub fn active_cells(&self) -> &[Cell] {
        &self.active
    }

    /// Per-cell fireline intensity, kW/m (Byram). Zero where never burnt.
    pub fn intensity(&self) -> &[f32] {
        &self.intensity
    }

    pub fn cell_intensity(&self, c: Cell) -> f32 {
        self.intensity[c.row * self.world.fire_cols + c.col]
    }

    pub fn exposure(&self) -> &StructureExposure {
        &self.exposure
    }

    /// Where the fire is likely to go next: see [`hazard`].
    pub fn hazard(&self) -> &HazardField {
        &self.hazard
    }

    pub fn cell_state(&self, c: Cell) -> CellFire {
        self.state[c.row * self.world.fire_cols + c.col]
    }

    /// Light a patch of burnable fuel of `radius_m` around `centre`.
    ///
    /// Prefer this to a single-cell [`ignite`] for scenario starts. With one
    /// realization a single lit cell is a coin flip: measured over 20 seeds on
    /// the Spotorno raster, ~20% of them never establish and the fire is out
    /// within minutes. That is defensible physics -- a single cell can fail to
    /// find a neighbour to carry into -- but it is not a scenario. A patch a
    /// few cells across establishes reliably, and matches what a fire actually
    /// looks like by the time anyone is dispatched to it.
    pub fn ignite_patch(&mut self, centre: Cell, radius_m: f32, scn: &Scenario) -> Result<()> {
        let r = (radius_m / self.world.cellsize).ceil() as i64;
        let mut cells = Vec::new();
        for dr in -r..=r {
            for dc in -r..=r {
                if (dr * dr + dc * dc) as f32 > (r * r) as f32 {
                    continue;
                }
                let (row, col) = (centre.row as i64 + dr, centre.col as i64 + dc);
                if row < 0 || col < 0 {
                    continue;
                }
                let c = Cell { row: row as usize, col: col as usize };
                if c.row < self.world.fire_rows
                    && c.col < self.world.fire_cols
                    && scn.is_burnable(c)
                {
                    cells.push(c);
                }
            }
        }
        anyhow::ensure!(!cells.is_empty(), "no burnable fuel within {radius_m} m of {centre:?}");
        self.ignite(&cells)
    }

    /// Light the fire at exactly `cells`, at the current simulated time.
    pub fn ignite(&mut self, cells: &[Cell]) -> Result<()> {
        let points: Vec<(usize, usize)> = cells.iter().map(|c| (c.row, c.col)).collect();
        self.sim
            .set_boundary_conditions(BoundaryConditions {
                time: self.time_s,
                ignitions: Some(Ignitions::Points(points)),
                ..self.weather_bc()
            })
            .context("scheduling ignition")?;
        Ok(())
    }

    /// Change the weather from now on.
    pub fn set_weather(&mut self, weather: Weather) -> Result<()> {
        self.weather = weather;
        let bc = BoundaryConditions { time: self.time_s, ..self.weather_bc() };
        self.sim.set_boundary_conditions(bc).context("updating weather")?;
        Ok(())
    }

    fn weather_bc(&self) -> BoundaryConditions {
        BoundaryConditions {
            time: self.time_s,
            wind_dir: Some(FieldInput::Scalar(self.weather.wind_dir_deg)),
            wind_speed: Some(FieldInput::Scalar(self.weather.wind_speed_kmh)),
            moisture: Some(FieldInput::Scalar(self.weather.moisture_pct)),
            ..Default::default()
        }
    }

    /// Queue a crew or engine action. Applied on the next [`advance`], because
    /// the core takes them as scheduled boundary conditions rather than
    /// immediate mutations.
    pub fn queue(&mut self, action: Intervention) {
        self.pending.push(action);
    }

    /// Advance the fire by `seconds` of simulated time.
    ///
    /// Returns the number of cells that newly ignited, which is what the audio
    /// and alerting layers care about.
    pub fn advance(&mut self, seconds: i64) -> Result<usize> {
        if seconds <= 0 {
            return Ok(0);
        }
        self.flush_interventions()?;

        let target = self.time_s + seconds;
        self.sim.step_window(seconds).context("stepping fire core")?;
        self.time_s = target;

        let before = self.active.len();
        self.refresh_state()?;
        self.exposure.update(
            &self.state,
            &self.active,
            &self.intensity,
            self.world,
            self.weather,
            seconds as f32,
        );
        self.hazard.update(&self.active, &self.state, self.weather);
        Ok(self.active.len().saturating_sub(before))
    }

    /// Push queued interventions into the core as one merged boundary-condition
    /// event: fuel removal via `vegetation_changes`, water and retardant via
    /// `additional_moisture`.
    fn flush_interventions(&mut self) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let (rows, cols) = (self.world.fire_rows, self.world.fire_cols);
        let mut veg_change: Option<Grid2<f64>> = None;
        let mut moisture: Option<Grid2<f32>> = None;

        for action in std::mem::take(&mut self.pending) {
            match action.kind {
                InterventionKind::Fireline => {
                    let g = veg_change
                        .get_or_insert_with(|| Grid2::filled(rows, cols, 0.0));
                    for c in &action.cells {
                        // negative = fuel removed; the kernel treats a fully
                        // cleared cell as non-burnable
                        g.as_mut_slice()[c.row * cols + c.col] = -1.0;
                    }
                }
                InterventionKind::Water { litres_per_m2 } => {
                    let g = moisture
                        .get_or_insert_with(|| Grid2::filled(rows, cols, 0.0));
                    // 1 mm of water over a cell is ~1 percentage point of added
                    // fuel moisture; the core decays this over time on its own.
                    let add = (litres_per_m2 * 1.0) as f32;
                    for c in &action.cells {
                        let i = c.row * cols + c.col;
                        g.as_mut_slice()[i] = (g.as_slice()[i] + add).min(60.0);
                    }
                }
            }
        }

        self.sim
            .set_boundary_conditions(BoundaryConditions {
                time: self.time_s,
                vegetation_changes: veg_change,
                additional_moisture: moisture,
                ..self.weather_bc()
            })
            .context("applying interventions")?;
        Ok(())
    }

    /// Pull the fire mask out of the core and age cells toward burnt-out.
    fn refresh_state(&mut self) -> Result<()> {
        let fire = self.sim.get_fire().context("reading fire state")?;
        let mask = &fire[0]; // single realization
        let fli = self
            .sim
            .get_fireline_int()
            .context("reading fireline intensity")?;
        self.intensity.copy_from_slice(fli[0].as_slice());
        // Take ignition times from the core, not from when *we* happened to
        // look. Stamping the observation time makes every cell seen in the
        // same step expire as one cohort, so the active front collapses and
        // reappears in bursts and exposure flickers with the step size.
        let arrival = self
            .sim
            .get_arrival_time()
            .context("reading arrival time")?;
        let arrival = arrival[0].as_slice();
        let cols = self.world.fire_cols;
        let now = self.time_s as i32;

        self.active.clear();
        for (i, &lit) in mask.as_slice().iter().enumerate() {
            if lit == 0 {
                continue;
            }
            if self.ignited_at[i] == i32::MIN {
                self.ignited_at[i] = arrival[i];
                self.state[i] = CellFire::Burning;
            }
            if now - self.ignited_at[i] >= BURNOUT_S {
                self.state[i] = CellFire::Burnt;
            } else {
                self.state[i] = CellFire::Burning;
                self.active.push(Cell { row: i / cols, col: i % cols });
            }
        }
        Ok(())
    }

    /// Arrival times for the whole grid, `i32::MIN` where still unburnt.
    ///
    /// Exposed as a slice because the renderer interpolates it: drawing fire
    /// at the 20 m cell it was computed on is what makes a CA look like a CA.
    pub fn arrival_times(&self) -> &[i32] {
        &self.ignited_at
    }

    /// Simulated time each cell ignited, or `None` if still unburnt.
    pub fn arrival_time(&self, c: Cell) -> Option<i32> {
        let v = self.ignited_at[c.row * self.world.fire_cols + c.col];
        (v != i32::MIN).then_some(v)
    }
}
