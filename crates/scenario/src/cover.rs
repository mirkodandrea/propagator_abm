//! Land cover at the render resolution (5 m): what the ground *is* — road,
//! building, paved yard, irrigated plot — finer than the 20 m fuel raster can
//! say. Graphics only: the fire never reads it.
//!
//! Optional. Scenarios baked before the scenario factory have no `cover.u8`,
//! and everything that uses it falls back to the fuel raster.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::Pos;

#[derive(Debug, Deserialize)]
struct CoverMeta {
    rows: usize,
    cols: usize,
    cell_m: f32,
    world_size_m: [f32; 2],
}

/// Cover classes, as coded in `cover.u8` (see `tools/factory/fine.py`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CoverClass {
    Natural = 0,
    Road = 1,
    Track = 2,
    Building = 3,
    Yard = 4,
    Irrigated = 5,
    Water = 6,
}

impl CoverClass {
    fn from_u8(v: u8) -> CoverClass {
        match v {
            1 => CoverClass::Road,
            2 => CoverClass::Track,
            3 => CoverClass::Building,
            4 => CoverClass::Yard,
            5 => CoverClass::Irrigated,
            6 => CoverClass::Water,
            _ => CoverClass::Natural,
        }
    }
}

/// Row 0 is the north edge; cell (r, c) spans x in [c, c+1)·cell_m.
pub struct Cover {
    pub rows: usize,
    pub cols: usize,
    pub cell_m: f32,
    pub height_m: f32,
    classes: Vec<u8>,
}

impl Cover {
    /// `Ok(None)` when the scenario has no cover raster.
    pub fn load(dir: &Path) -> Result<Option<Cover>> {
        let meta_path = dir.join("cover.json");
        if !meta_path.exists() {
            return Ok(None);
        }
        let meta: CoverMeta =
            serde_json::from_slice(&datafs::read(&meta_path)?).context("cover.json")?;
        let classes = datafs::read(dir.join("cover.u8")).context("cover.u8")?;
        anyhow::ensure!(
            classes.len() == meta.rows * meta.cols,
            "cover.u8 has {} bytes, expected {}x{}",
            classes.len(),
            meta.rows,
            meta.cols
        );
        Ok(Some(Cover {
            rows: meta.rows,
            cols: meta.cols,
            cell_m: meta.cell_m,
            height_m: meta.world_size_m[1],
            classes,
        }))
    }

    pub fn at(&self, p: Pos) -> CoverClass {
        let c = (p.x / self.cell_m).floor().clamp(0.0, (self.cols - 1) as f32) as usize;
        let r = ((self.height_m - p.y) / self.cell_m)
            .floor()
            .clamp(0.0, (self.rows - 1) as f32) as usize;
        CoverClass::from_u8(self.classes[r * self.cols + c])
    }

    pub fn is_natural(&self, p: Pos) -> bool {
        self.at(p) == CoverClass::Natural
    }
}
