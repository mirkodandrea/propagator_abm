//! Baked scenario assets for Spotorno, and the coordinate frames that tie
//! them together.
//!
//! Three resolutions coexist deliberately:
//!
//! - the **fire grid**, 20 m, fixed by the PROPAGATOR input rasters;
//! - the **render terrain**, currently 5 m, purely a visual concern;
//! - **vector geometry and agents**, unquantised metres.
//!
//! Everything is expressed in one *world frame*: origin at the scenario
//! window's south-west corner, +x east, +y north, metres. Converting to fire
//! cells is [`World::cell_of`]; nothing else in the game should need to know
//! the raster spacing exists.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

pub mod fuels;
pub mod metadata;
pub mod population;
pub mod terrain;
pub mod vectors;

pub use fuels::FuelDefRaw;
pub use metadata::{ScenarioMetadata, ScenarioRegistry};
pub use population::{Dwelling, Household, Person, Population};
pub use terrain::Terrain;
pub use vectors::{Building, Road, Vectors, WaterSource};

/// A cell index into the 20 m fire grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cell {
    pub row: usize,
    pub col: usize,
}

/// Metric position in the scenario world frame.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Pos {
    pub x: f32,
    pub y: f32,
}

impl From<[f32; 2]> for Pos {
    fn from(v: [f32; 2]) -> Self {
        Pos { x: v[0], y: v[1] }
    }
}

/// Geometry shared by every layer: how big the world is and how it maps onto
/// the fire raster.
#[derive(Debug, Clone, Copy)]
pub struct World {
    pub width_m: f32,
    pub height_m: f32,
    pub fire_rows: usize,
    pub fire_cols: usize,
    pub cellsize: f32,
}

impl World {
    /// World metres -> fire grid cell. Note the row flip: the world frame has
    /// +y north, the raster has row 0 at the north edge.
    pub fn cell_of(&self, p: Pos) -> Cell {
        let col = (p.x / self.cellsize).floor().clamp(0.0, (self.fire_cols - 1) as f32);
        let row = ((self.height_m - p.y) / self.cellsize)
            .floor()
            .clamp(0.0, (self.fire_rows - 1) as f32);
        Cell { row: row as usize, col: col as usize }
    }

    /// Centre of a fire cell, in world metres.
    pub fn centre_of(&self, c: Cell) -> Pos {
        Pos {
            x: (c.col as f32 + 0.5) * self.cellsize,
            y: self.height_m - (c.row as f32 + 0.5) * self.cellsize,
        }
    }

    pub fn contains(&self, p: Pos) -> bool {
        p.x >= 0.0 && p.y >= 0.0 && p.x < self.width_m && p.y < self.height_m
    }
}

/// Everything needed to start a scenario.
pub struct Scenario {
    pub id: String,
    pub metadata: ScenarioMetadata,
    pub world: World,
    pub terrain: Terrain,
    pub vectors: Vectors,
    pub population: Population,
    /// 20 m fuel classes, row-major, row 0 = north. `eu_fuel12` coding.
    pub fuel: Vec<i32>,
    /// 20 m elevation, matching `fuel`.
    pub dem: Vec<f64>,
    /// The eu_fuel12 class table these rasters are coded against.
    pub fuel_defs: Vec<FuelDefRaw>,
}

impl Scenario {
    /// Load scenario by ID from the scenarios directory.
    /// Example: load_by_id("data", "spotorno") loads from "data/scenarios/spotorno/"
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_by_id(data_dir: impl AsRef<Path>, id: impl AsRef<str>) -> Result<Scenario> {
        let data_dir = data_dir.as_ref();
        let id = id.as_ref();

        let scenario_dir = data_dir.join("scenarios").join(id);

        // Load metadata from scenario.json
        let metadata_path = scenario_dir.join("scenario.json");
        let metadata_bytes = std::fs::read(&metadata_path)
            .with_context(|| format!("reading {}", metadata_path.display()))?;
        let metadata: ScenarioMetadata = serde_json::from_slice(&metadata_bytes)
            .context("parsing scenario.json")?;

        // Load scenario assets
        let terrain = Terrain::load(&scenario_dir).context("render terrain")?;
        let vectors = Vectors::load(&scenario_dir).context("osm vectors")?;
        let population = Population::load(&scenario_dir).context("population")?;

        let world = World {
            width_m: vectors.world_size_m[0],
            height_m: vectors.world_size_m[1],
            fire_rows: vectors.fire_grid.rows,
            fire_cols: vectors.fire_grid.cols,
            cellsize: vectors.fire_grid.cellsize,
        };

        let (fuel, dem) = load_fire_rasters(&scenario_dir, world.fire_rows, world.fire_cols)?;
        let fuel_defs = fuels::load(data_dir).context("fuel table")?;

        Ok(Scenario {
            id: id.to_string(),
            metadata,
            world,
            terrain,
            vectors,
            population,
            fuel,
            dem,
            fuel_defs,
        })
    }

    /// Load the baked assets from a data directory.
    /// For backward compatibility: if data directory contains "scenarios" subdir, loads default scenario.
    /// Otherwise, tries to load from directory directly (legacy mode).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(dir: impl AsRef<Path>) -> Result<Scenario> {
        let dir = dir.as_ref();
        let scenarios_dir = dir.join("scenarios");

        // If scenarios directory exists, load default scenario from it
        if scenarios_dir.exists() {
            let registry = ScenarioRegistry::discover(dir)?;
            let default_id = registry.default_id().to_string();
            Self::load_by_id(dir, &default_id)
        } else {
            // Legacy mode: load directly from directory
            let terrain = Terrain::load(dir).context("render terrain")?;
            let vectors = Vectors::load(dir).context("osm vectors")?;
            let population = Population::load(dir).context("population")?;

            let world = World {
                width_m: vectors.world_size_m[0],
                height_m: vectors.world_size_m[1],
                fire_rows: vectors.fire_grid.rows,
                fire_cols: vectors.fire_grid.cols,
                cellsize: vectors.fire_grid.cellsize,
            };

            let (fuel, dem) = load_fire_rasters(dir, world.fire_rows, world.fire_cols)?;
            let fuel_defs = fuels::load(dir).context("fuel table")?;

            Ok(Scenario {
                id: "unknown".to_string(),
                metadata: ScenarioMetadata {
                    id: "unknown".to_string(),
                    name: "Unknown".to_string(),
                    description: "Loaded from legacy format".to_string(),
                    location: String::new(),
                    country: String::new(),
                    coordinates: [0.0, 0.0],
                    utm_zone: 0,
                    world_size_m: [world.width_m, world.height_m],
                    fire_grid_size: [world.fire_rows, world.fire_cols],
                    buildings_count: 0,
                    households_count: 0,
                    people_count: 0,
                    scenario_type: metadata::ScenarioType::Real,
                    creation_date: String::new(),
                    version: String::new(),
                    tags: vec![],
                },
                world,
                terrain,
                vectors,
                population,
                fuel,
                dem,
                fuel_defs,
            })
        }
    }

    /// Web builds are self-contained: GitHub Pages has no filesystem for the
    /// game to read, so the needed baked assets are compiled into the wasm.
    /// The terrain is deliberately reduced to 20 m posting by `build.rs`.
    #[cfg(target_arch = "wasm32")]
    pub fn load(_dir: impl AsRef<Path>) -> Result<Scenario> {
        let terrain = Terrain::load_web().context("embedded render terrain")?;
        let vectors = Vectors::load_web().context("embedded osm vectors")?;
        let population = Population::load_web().context("embedded population")?;

        let world = World {
            width_m: vectors.world_size_m[0],
            height_m: vectors.world_size_m[1],
            fire_rows: vectors.fire_grid.rows,
            fire_cols: vectors.fire_grid.cols,
            cellsize: vectors.fire_grid.cellsize,
        };
        let fuel = read_raw_bytes::<i32>(include_bytes!(concat!(env!("OUT_DIR"), "/web_fuel.i32")), world.fire_rows * world.fire_cols)?;
        let dem = read_raw_bytes::<f64>(include_bytes!(concat!(env!("OUT_DIR"), "/web_dem.f64")), world.fire_rows * world.fire_cols)?;
        let fuel_defs = fuels::load_web().context("embedded fuel table")?;

        // Create metadata for web build (not loaded from file)
        let metadata = ScenarioMetadata {
            id: env!("SPOTORNO_SCENARIO_ID").to_string(),
            name: "Spotorno, Liguria".to_string(),
            description: "Web build scenario".to_string(),
            location: "Spotorno, Italy".to_string(),
            country: "Italy".to_string(),
            coordinates: [44.2265, 8.4176],
            utm_zone: 32,
            world_size_m: [world.width_m, world.height_m],
            fire_grid_size: [world.fire_rows, world.fire_cols],
            buildings_count: vectors.buildings.len(),
            households_count: population.households.len(),
            people_count: population.people.len(),
            scenario_type: metadata::ScenarioType::Real,
            creation_date: String::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            tags: vec!["web".to_string()],
        };

        Ok(Scenario {
            id: env!("SPOTORNO_SCENARIO_ID").to_string(),
            metadata,
            world,
            terrain,
            vectors,
            population,
            fuel,
            dem,
            fuel_defs,
        })
    }

    pub fn fuel_at(&self, c: Cell) -> i32 {
        self.fuel[c.row * self.world.fire_cols + c.col]
    }

    pub fn is_burnable(&self, c: Cell) -> bool {
        matches!(self.fuel_at(c), 1..=12)
    }
}

/// Fuel and DEM are baked to raw little-endian arrays alongside the GeoTIFFs,
/// because pulling a TIFF decoder in just to read two fixed-size grids is not
/// worth the dependency.
#[cfg(not(target_arch = "wasm32"))]
fn load_fire_rasters(dir: &Path, rows: usize, cols: usize) -> Result<(Vec<i32>, Vec<f64>)> {
    let fuel = read_raw::<i32>(&dir.join("fuel.i32"), rows * cols)
        .context("fuel.i32 (run scripts/bake_fire_rasters.py)")?;
    let dem = read_raw::<f64>(&dir.join("dem.f64"), rows * cols)
        .context("dem.f64 (run scripts/bake_fire_rasters.py)")?;
    Ok((fuel, dem))
}

pub(crate) fn read_raw<T: Copy>(path: &Path, count: usize) -> Result<Vec<T>> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let want = count * std::mem::size_of::<T>();
    anyhow::ensure!(
        bytes.len() == want,
        "{}: expected {want} bytes ({count} elements), found {}",
        path.display(),
        bytes.len()
    );
    read_raw_bytes(&bytes, count)
}

pub(crate) fn read_raw_bytes<T: Copy>(bytes: &[u8], count: usize) -> Result<Vec<T>> {
    let want = count * std::mem::size_of::<T>();
    anyhow::ensure!(
        bytes.len() == want,
        "expected {want} bytes ({count} elements), found {}",
        bytes.len()
    );
    let mut out = Vec::<T>::with_capacity(count);
    // SAFETY: the file length matches `count` elements exactly, the source is
    // a byte buffer with no alignment guarantees, and `T` is a plain numeric
    // type with no invalid bit patterns.
    unsafe {
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            out.as_mut_ptr() as *mut u8,
            want,
        );
        out.set_len(count);
    }
    Ok(out)
}
