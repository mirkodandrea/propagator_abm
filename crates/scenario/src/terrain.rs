//! The rendering heightfield: finer than the fire grid, and independent of it.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{read_raw, Pos};
#[cfg(target_arch = "wasm32")]
use crate::read_raw_bytes;

#[derive(Debug, Deserialize)]
struct TerrainMeta {
    rows: usize,
    cols: usize,
    posting_m: f32,
    world_size_m: [f32; 2],
    elev_min: f32,
    elev_max: f32,
}

/// Smoothed elevation samples for rendering. Row 0 is the north edge.
pub struct Terrain {
    pub rows: usize,
    pub cols: usize,
    /// Sample spacing in metres (5 m at present, vs the fire grid's 20 m).
    pub posting: f32,
    pub width_m: f32,
    pub height_m: f32,
    pub elev_min: f32,
    pub elev_max: f32,
    pub elev: Vec<f32>,
}

impl Terrain {
    pub fn load(dir: &Path) -> Result<Terrain> {
        let meta: TerrainMeta = serde_json::from_slice(
            &std::fs::read(dir.join("render_terrain.json"))
                .context("render_terrain.json")?,
        )?;
        let elev = read_raw::<f32>(
            &dir.join("render_terrain.f32"),
            meta.rows * meta.cols,
        )?;
        Ok(Terrain {
            rows: meta.rows,
            cols: meta.cols,
            posting: meta.posting_m,
            width_m: meta.world_size_m[0],
            height_m: meta.world_size_m[1],
            elev_min: meta.elev_min,
            elev_max: meta.elev_max,
            elev,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load_web(metadata: &[u8], terrain: &[u8]) -> Result<Terrain> {
        let meta: TerrainMeta = serde_json::from_slice(metadata)?;
        let step = meta.rows.div_ceil(512).max(meta.cols.div_ceil(512)).max(1);
        let rows = meta.rows.div_ceil(step);
        let cols = meta.cols.div_ceil(step);
        let elev = read_raw_bytes::<f32>(terrain, rows * cols)?;
        Ok(Terrain {
            rows,
            cols,
            posting: meta.posting_m * step as f32,
            width_m: meta.world_size_m[0],
            height_m: meta.world_size_m[1],
            elev_min: meta.elev_min,
            elev_max: meta.elev_max,
            elev,
        })
    }

    /// Bilinear elevation at a world position. Agents, buildings and the
    /// camera all sit on this, so it must be continuous -- sampling the 20 m
    /// fire DEM instead would make everything visibly step.
    pub fn height_at(&self, p: Pos) -> f32 {
        let fx = (p.x / self.posting).clamp(0.0, (self.cols - 1) as f32);
        // world +y is north, row 0 is the north edge
        let fy = ((self.height_m - p.y) / self.posting).clamp(0.0, (self.rows - 1) as f32);

        let x0 = fx.floor() as usize;
        let y0 = fy.floor() as usize;
        let x1 = (x0 + 1).min(self.cols - 1);
        let y1 = (y0 + 1).min(self.rows - 1);
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;

        let h = |r: usize, c: usize| self.elev[r * self.cols + c];
        let top = h(y0, x0) * (1.0 - tx) + h(y0, x1) * tx;
        let bot = h(y1, x0) * (1.0 - tx) + h(y1, x1) * tx;
        top * (1.0 - ty) + bot * ty
    }

    /// Surface normal from central differences, for lighting and for slope
    /// effects on agent walking speed.
    pub fn normal_at(&self, p: Pos) -> [f32; 3] {
        let d = self.posting;
        let hl = self.height_at(Pos { x: p.x - d, ..p });
        let hr = self.height_at(Pos { x: p.x + d, ..p });
        let hd = self.height_at(Pos { y: p.y - d, ..p });
        let hu = self.height_at(Pos { y: p.y + d, ..p });
        let nx = hl - hr;
        let nz = hd - hu;
        let ny = 2.0 * d;
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        [nx / len, ny / len, nz / len]
    }

    /// Slope in degrees -- drives how fast people can move off-road.
    pub fn slope_deg_at(&self, p: Pos) -> f32 {
        let n = self.normal_at(p);
        n[1].clamp(-1.0, 1.0).acos().to_degrees()
    }
}
