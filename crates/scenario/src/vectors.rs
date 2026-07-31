//! Real OSM geometry: buildings, roads, water sources.
//!
//! Kept in metres at full precision. Roads carry the drivable/track split,
//! which is the difference between where an engine can go and where a hand
//! crew can go.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FireGrid {
    pub rows: usize,
    pub cols: usize,
    pub cellsize: f32,
}

#[derive(Debug, Deserialize)]
pub struct Building {
    pub id: i64,
    pub kind: Option<String>,
    pub name: Option<String>,
    pub centroid: [f32; 2],
    /// Footprint outline, world metres.
    pub ring: Vec<[f32; 2]>,
}

#[derive(Debug, Deserialize)]
pub struct Road {
    pub id: i64,
    pub class: String,
    /// An engine can drive this.
    pub drivable: bool,
    /// Foot or small 4x4 only -- crew access and escape routes.
    pub track: bool,
    pub name: Option<String>,
    pub oneway: bool,
    pub line: Vec<[f32; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WaterSource {
    pub id: i64,
    /// `hydrant`, `open_water` or `water_tower`.
    pub kind: String,
    pub pos: [f32; 2],
}

#[derive(Debug, Deserialize)]
pub struct Vectors {
    pub world_size_m: [f32; 2],
    pub fire_grid: FireGrid,
    pub buildings: Vec<Building>,
    pub roads: Vec<Road>,
    pub water: Vec<WaterSource>,
}

impl Vectors {
    pub fn load(dir: &Path) -> Result<Vectors> {
        let bytes = std::fs::read(dir.join("osm.json"))
            .context("osm.json")?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load_web() -> Result<Vectors> {
        Ok(serde_json::from_slice(include_bytes!("../../../data/spotorno_osm.json"))?)
    }

    pub fn drivable_roads(&self) -> impl Iterator<Item = &Road> {
        self.roads.iter().filter(|r| r.drivable)
    }

    pub fn hydrants(&self) -> impl Iterator<Item = &WaterSource> {
        self.water.iter().filter(|w| w.kind == "hydrant")
    }
}

impl Building {
    /// Footprint area in m², by the shoelace formula.
    pub fn area(&self) -> f32 {
        let n = self.ring.len();
        if n < 3 {
            return 0.0;
        }
        let mut acc = 0.0;
        for i in 0..n {
            let a = self.ring[i];
            let b = self.ring[(i + 1) % n];
            acc += a[0] * b[1] - b[0] * a[1];
        }
        (acc / 2.0).abs()
    }
}
