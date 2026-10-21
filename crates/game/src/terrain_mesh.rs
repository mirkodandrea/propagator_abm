//! Building the terrain mesh from the 5 m render heightfield.
//!
//! The full field is 2048² = 4.19 M vertices, which is far too much for one
//! mesh: it defeats frustum culling and blows past the 16-bit index limit. It
//! is split into chunks instead, each its own entity, so the renderer can cull
//! and the driver gets sensibly sized buffers.
//!
//! The ground colour and the normal are not vertex attributes but two
//! textures, one texel per base sample (`shaders/terrain.wgsl` reads them), so the picture does not depend on how dense the mesh is. That
//! lets every chunk be meshed at three levels (base stride x1, x2, x4: 32 k,
//! 8 k and 2 k triangles) and drawn by distance with a [`VisibilityRange`]
//! (abrupt: the terrain shader does not dither). Each level has a skirt
//! to hide the cracks between chunks at different levels. A ranged entity is
//! never a shadow caster, so the shadows come from one extra entity per chunk
//! on [`SHADOW_LAYER`] (the sun's layer, not the camera's), sharing the x2 mesh.
//!
//! The terrain is **ground**, not vegetation. It used to be painted with the
//! fuel raster's classes, which read as a camouflage patchwork however much
//! the boundaries were warped — the underlying data is 20 m and no amount of
//! colour dithering hides that. Vegetation is now drawn as actual plants (see
//! [`crate::vegetation`]), and this layer paints only what is under them:
//! soil, rock on the steep faces, sand at the shore, sea below the waterline.
//! Along the coast the ground has two materials of its own ([`coast_rock`],
//! [`coast_sand`]): banded limestone on the cliffs, rippled sand on the beaches.
//! Ground colour therefore varies with slope, elevation and noise — geology,
//! not land cover — which is both honest and far better looking.

use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::camera::visibility::{RenderLayers, VisibilityRange};
use scenario::{Cell, Cover, CoverClass, Pos, Scenario};

use crate::field::noise;
use crate::materials::TerrainMaterial;

/// Samples per chunk edge. 128 keeps each chunk at ~16 k vertices.
const CHUNK: usize = 128;

#[derive(Component)]
pub struct TerrainChunk;

/// Static rendered geometry used by the optional native pause playground.
#[derive(Component)]
pub struct PauseSolid;

/// Elevation at or below which a non-burnable fire cell is water: the rule
/// `abm::haven::is_water` uses, so the picture and the model agree.
const SEA_FIRE_DEM_M: f64 = 2.0;

/// A chunk whose ground is all below this is under the (opaque) sea even in
/// the deepest wave trough (`sea::WATER_BASE_Y` 0.8 m less a swell of at most
/// 0.75 m on the 15 m lattice, `shaders/water.wgsl`): it is never seen, so it
/// is not drawn. The island's sea floor is flat at 0 m, and over half the
/// window.
const SUBMERGED_M: f32 = 0.03;

/// Distance, metres, from each fire cell to the nearest water cell: what the
/// coast's materials key off, since a vertex does not know where the sea is.
pub(crate) struct ShoreField {
    cols: usize,
    rows: usize,
    cell: f32,
    height_m: f32,
    d: Vec<f32>,
}

impl ShoreField {
    pub(crate) fn new(scn: &Scenario) -> Self {
        let w = &scn.world;
        let (cols, rows) = (w.fire_cols, w.fire_rows);
        let mut d = vec![f32::INFINITY; cols * rows];
        for r in 0..rows {
            for c in 0..cols {
                let i = r * cols + c;
                if !scn.is_burnable(Cell { row: r, col: c }) && scn.dem.get(i).is_some_and(|e| *e <= SEA_FIRE_DEM_M) {
                    d[i] = 0.0;
                }
            }
        }
        // two-pass chamfer distance (1, sqrt 2 in cells)
        const D2: f32 = std::f32::consts::SQRT_2;
        for r in 0..rows {
            for c in 0..cols {
                let i = r * cols + c;
                let mut v = d[i];
                if c > 0 { v = v.min(d[i - 1] + 1.0); }
                if r > 0 {
                    v = v.min(d[i - cols] + 1.0);
                    if c > 0 { v = v.min(d[i - cols - 1] + D2); }
                    if c + 1 < cols { v = v.min(d[i - cols + 1] + D2); }
                }
                d[i] = v;
            }
        }
        for r in (0..rows).rev() {
            for c in (0..cols).rev() {
                let i = r * cols + c;
                let mut v = d[i];
                if c + 1 < cols { v = v.min(d[i + 1] + 1.0); }
                if r + 1 < rows {
                    v = v.min(d[i + cols] + 1.0);
                    if c + 1 < cols { v = v.min(d[i + cols + 1] + D2); }
                    if c > 0 { v = v.min(d[i + cols - 1] + D2); }
                }
                d[i] = v;
            }
        }
        for v in &mut d {
            *v = if v.is_finite() { *v * w.cellsize } else { 1.0e6 };
        }
        ShoreField { cols, rows, cell: w.cellsize, height_m: w.height_m, d }
    }

    /// Bilinear between cell centres.
    pub(crate) fn at(&self, p: Pos) -> f32 {
        let fx = (p.x / self.cell - 0.5).clamp(0.0, (self.cols - 1) as f32);
        let fy = ((self.height_m - p.y) / self.cell - 0.5).clamp(0.0, (self.rows - 1) as f32);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.cols - 1), (y0 + 1).min(self.rows - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let g = |r: usize, c: usize| self.d[r * self.cols + c];
        let top = g(y0, x0) * (1.0 - tx) + g(y0, x1) * tx;
        let bot = g(y1, x0) * (1.0 - tx) + g(y1, x1) * tx;
        top * (1.0 - ty) + bot * ty
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Sea cliffs, which a 5 m heightfield only knows as a smooth ramp: on steep
/// coastal faces the ground is displaced into strata ledges (the same bands
/// [`coast_rock`] paints, so the colour steps and the geometry steps agree)
/// and ridged crags, up to a few metres. Zero on the flat, on the beach, at
/// the foot (below 1.5 m the waterline is untouched) and beyond ~300 m from
/// the sea, so roads, houses and the water mesh meet the same ground as
/// before. A function of position only: chunk edges stay seamless.
pub(crate) fn cliff_elev(scn: &Scenario, shore: &ShoreField, p: Pos, elev: f32) -> f32 {
    if elev < 1.5 || shore.at(p) > COAST_REACH_M {
        return elev;
    }
    let slope_cos = scn.terrain.normal_at(p)[1];
    let steep = smooth((0.93 - slope_cos) / 0.12);
    let near = 1.0 - smooth((shore.at(p) - 60.0) / (COAST_REACH_M - 60.0));
    let k = steep * near * smooth((elev - 1.5) / 3.0);
    if k <= 0.0 {
        return elev;
    }
    // ledges: a staircase in the same strata as `coast_rock`
    let dip = noise(p.x / 160.0, p.y / 160.0, 0x5A) * 26.0 + noise(p.x / 38.0, p.y / 38.0, 0x5B) * 5.0;
    let u = (elev + dip) / 14.0;
    let stair = u.floor() + smooth((u.fract() - 0.25) / 0.5);
    let ledge = (stair - u) * 14.0 * 0.07;
    // crags: ridged noise, so the face has arêtes and gullies, not bumps
    // broad, rounded: the 5 m grid turns anything sharper into sawteeth
    let ridge = noise(p.x / 30.0, p.y / 30.0, 0x81);
    let crag = (ridge - 0.5) * 0.6 + (noise(p.x / 70.0, p.y / 70.0, 0x82) - 0.5) * 2.4;
    let d = k * (ledge + crag);
    // never carve below the foot of the face
    elev + d.max(-(elev - 1.0) * 0.8)
}

/// How far from the sea the cliffs are shaped, metres.
const COAST_REACH_M: f32 = 320.0;

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Limestone cliff: pale and ochre bands with a dark iron seam, whose
/// elevation wanders with noise so the strata dip and fault; vertical streaks
/// where water runs; dark and wet in the spray zone at the foot. The bands are
/// ~14 m thick: thinner than that and a 5 m vertex grid aliases them.
fn coast_rock(elev: f32, p: Pos) -> [f32; 3] {
    const PALE: [f32; 3] = [0.80, 0.76, 0.66];
    const OCHRE: [f32; 3] = [0.70, 0.57, 0.40];
    const GREY: [f32; 3] = [0.55, 0.54, 0.51];
    const IRON: [f32; 3] = [0.36, 0.30, 0.26];
    const WET: [f32; 3] = [0.27, 0.26, 0.25];
    let dip = noise(p.x / 160.0, p.y / 160.0, 0x5A) * 26.0 + noise(p.x / 38.0, p.y / 38.0, 0x5B) * 5.0;
    let t = ((elev + dip) / 14.0).rem_euclid(1.0);
    let band = if t < 0.34 {
        PALE
    } else if t < 0.40 {
        mix3(PALE, OCHRE, smooth((t - 0.34) / 0.06))
    } else if t < 0.62 {
        OCHRE
    } else if t < 0.70 {
        mix3(OCHRE, PALE, smooth((t - 0.62) / 0.08))
    } else if t < 0.86 {
        PALE
    } else if t < 0.92 {
        mix3(PALE, GREY, smooth((t - 0.86) / 0.06))
    } else if t < 0.96 {
        mix3(GREY, IRON, smooth((t - 0.92) / 0.04))
    } else {
        mix3(IRON, PALE, smooth((t - 0.96) / 0.04))
    };
    let streak = 0.94 + 0.12 * noise(p.x / 4.0, p.y / 30.0, 0x5C);
    let grain = 0.94 + 0.12 * noise(p.x / 7.0, p.y / 7.0, 0x5D);
    let lit = [band[0] * streak * grain, band[1] * streak * grain, band[2] * streak * grain];
    mix3(lit, WET, (1.0 - elev / 3.5).clamp(0.0, 1.0) * 0.65)
}

/// Beach sand: dark and wet at the waterline, pale and dry above it, with
/// ripples, a little shell, and tufts of dune grass on the high back beach.
fn coast_sand(elev: f32, p: Pos) -> [f32; 3] {
    const WET: [f32; 3] = [0.58, 0.52, 0.40];
    const DRY: [f32; 3] = [0.91, 0.84, 0.66];
    const GRASS: [f32; 3] = [0.60, 0.63, 0.38];
    let dry = smooth((elev - 0.7) / 1.6);
    let mut c = mix3(WET, DRY, dry);
    let ripple = (noise(p.x / 3.0, p.y / 8.0, 0x6C) - 0.5) * 0.10 + (noise(p.x / 20.0, p.y / 20.0, 0x6D) - 0.5) * 0.08;
    c = [c[0] + ripple, c[1] + ripple, c[2] + ripple * 0.9];
    if noise(p.x / 1.7, p.y / 1.7, 0x6E) > 0.92 {
        c = mix3(c, [0.96, 0.95, 0.90], 0.5);
    }
    let tuft = smooth((noise(p.x / 28.0, p.y / 28.0, 0x6F) - 0.52) / 0.2) * smooth((elev - 1.8) / 1.6);
    mix3(c, GRASS, tuft * 0.55)
}

const PAVING: [f32; 3] = [0.84, 0.81, 0.74];
const GARDEN: [f32; 3] = [0.50, 0.66, 0.36];
/// Cleared ground away from any house (the service areas, verges): mown dry
/// grass, not the lawn green of a garden.
const MEADOW: [f32; 3] = [0.73, 0.69, 0.42];

/// 0..1: how close the nearest house or yard is to fire cell `c`
/// (1 within 20 m, fading out by 60 m). Gardens stay green round the houses;
/// the rest of a cleared patch reads as meadow. Computed once per scenario.
pub(crate) fn built_nearby(scn: &Scenario, c: Cell) -> f32 {
    static NEAR: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let w = &scn.world;
    let near = NEAR.get_or_init(|| {
        let Some(cv) = &scn.cover else { return Vec::new() };
        let (rows, cols) = (w.fire_rows, w.fire_cols);
        // Built cells of the 5 m cover, then the distance to the nearest in
        // fire cells (chessboard, 3 cells reach), by a short sweep.
        let built: Vec<bool> = (0..rows * cols)
            .map(|i| {
                let (r, col) = (i / cols, i % cols);
                let (x0, y0) = (col as f32 * w.cellsize, w.height_m - (r as f32 + 1.0) * w.cellsize);
                (0..4).any(|a| (0..4).any(|b| matches!(cv.at(Pos { x: x0 + (a as f32 + 0.5) * 5.0, y: y0 + (b as f32 + 0.5) * 5.0 }), CoverClass::Building | CoverClass::Yard)))
            })
            .collect();
        (0..rows * cols)
            .map(|i| {
                let (r, col) = ((i / cols) as isize, (i % cols) as isize);
                let mut best = 4.0f32;
                for dr in -3..=3isize {
                    for dc in -3..=3isize {
                        let (rr, cc) = (r + dr, col + dc);
                        if rr >= 0 && cc >= 0 && (rr as usize) < rows && (cc as usize) < cols && built[rr as usize * cols + cc as usize] {
                            best = best.min(dr.abs().max(dc.abs()) as f32);
                        }
                    }
                }
                (1.0 - (best - 1.0) / 2.0).clamp(0.0, 1.0)
            })
            .collect()
    });
    near.get(c.row * w.fire_cols + c.col).copied().unwrap_or(1.0)
}

/// What the 5 m cover says is built at `p`, as a colour and a weight in 0..1:
/// the share of the four cover cells round `p` that are not natural ground.
/// Vertices sit on cell corners, so the weight gives edges one posting soft.
fn built_tint(cover: &Cover, p: Pos) -> ([f32; 3], f32) {
    let h = cover.cell_m * 0.5;
    let (mut sum, mut n) = ([0.0; 3], 0.0);
    for (dx, dy) in [(-h, -h), (h, -h), (-h, h), (h, h)] {
        let c = match cover.at(Pos { x: p.x + dx, y: p.y + dy }) {
            CoverClass::Natural => continue,
            // Tracks are drawn as ribbons too; beneath and beside them, bare
            // compacted earth.
            CoverClass::Track => [0.66, 0.58, 0.44],
            // Farmland is a mesh of parcels over the ground (`fields.rs`).
            CoverClass::Irrigated => continue,
            CoverClass::Water => [0.20, 0.36, 0.44],
            // Paving is drawn per pixel by the terrain shader (`ground_mask`).
            CoverClass::Road | CoverClass::Building | CoverClass::Yard => continue,
        };
        for i in 0..3 {
            sum[i] += c[i];
        }
        n += 1.0;
    }
    if n == 0.0 {
        return (PAVING, 0.0);
    }
    ([sum[0] / n, sum[1] / n, sum[2] / n], n / 4.0)
}

/// Land-cover tint at `p`, bilinear between fire-cell centres so the 20 m raster
/// reads as soft pastel meadows rather than pixels.
fn cover_tint(scn: &Scenario, p: Pos) -> [f32; 3] {
    let w = &scn.world;
    let fx = p.x / w.cellsize - 0.5;
    let fy = (w.height_m - p.y) / w.cellsize - 0.5;
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let tint = |dx: f32, dy: f32| -> [f32; 3] {
        let c = Cell {
            row: (y0 + dy).clamp(0.0, (w.fire_rows - 1) as f32) as usize,
            col: (x0 + dx).clamp(0.0, (w.fire_cols - 1) as f32) as usize,
        };
        match scn.fuel_at(c) {
            // Gardens and lawns round the houses: the green that makes a
            // town read as a town (class 1 is only ever painted there).
            1 => [0.56, 0.74, 0.40],
            // The hue of each group's plants, lighter, so the cover reads
            // where the plants are sparse (12 % in the browser): straw gold
            // (class 3 paler), chestnut green, olive macchia, pine
            // blue-green. Bilinear and warped below, never hard-edged.
            2 => [0.78, 0.68, 0.38],
            3 => [0.86, 0.76, 0.46],
            4 => [0.40, 0.60, 0.26],
            5..=6 => [0.46, 0.60, 0.28],
            7..=9 => [0.44, 0.48, 0.28],
            10..=12 => [0.30, 0.42, 0.33],
            // Non-burnable fire cell. With 5 m cover, what is actually built
            // is painted by `built_tint`; the rest of the cell is the yards
            // and verges round the houses.
            _ if scn.cover.is_some() => mix3(MEADOW, GARDEN, built_nearby(scn, c)),
            // Built-up ground: pale paving rather than bare earth.
            _ => PAVING,
        }
    };
    let mut out = [0.0; 3];
    for (dx, dy, k) in [(0.0, 0.0, (1.0 - tx) * (1.0 - ty)), (1.0, 0.0, tx * (1.0 - ty)), (0.0, 1.0, (1.0 - tx) * ty), (1.0, 1.0, tx * ty)] {
        let c = tint(dx, dy);
        for i in 0..3 {
            out[i] += c[i] * k;
        }
    }
    out
}

/// Ground colour from elevation, steepness and noise.
///
/// `slope_cos` is the vertical component of the surface normal: 1 on the flat,
/// falling toward 0 on a cliff. Ligurian ground is dry pale limestone soil,
/// with bare rock showing wherever it is too steep to hold any.
fn ground_color(scn: &Scenario, sea: &crate::ground_mask::SeaGrid, elev: f32, slope_cos: f32, p: Pos, shore: f32) -> [f32; 3] {
    if elev <= 0.5 && sea.is_sea(p) {
        return [0.07, 0.13, 0.26]; // sea
    }

    // Two scales of variation: broad soil banding, plus a fine grain that
    // keeps the 5 m posting from reading as flat facets.
    let broad = noise(p.x / 140.0, p.y / 140.0, 0x1A);
    let grain = noise(p.x / 11.0, p.y / 11.0, 0x2B);
    let mottle = 0.90 + 0.18 * (0.7 * broad + 0.3 * grain);

    // Grey-olive, not red-brown: this is limestone karst with a thin dry duff
    // over it. Saturated soil colour reads as Mars from altitude, especially
    // under a warm sun.
    // Toy-diorama ground: the land cover is painted here as a pastel tint
    // (a few hundred chunky props stand on top, see `vegetation`), warped by
    // noise so the 20 m fuel cells do not show.
    let warp = Pos {
        x: p.x + 38.0 * (noise(p.x / 55.0, p.y / 55.0, 0x71) - 0.5),
        y: p.y + 38.0 * (noise(p.x / 55.0, p.y / 55.0, 0x72) - 0.5),
    };
    let mut cover = cover_tint(scn, warp);
    // Built cover is sharp at 5 m, so it is sampled where it is, not warped.
    if let Some(cv) = &scn.cover {
        let (built, k) = built_tint(cv, p);
        for i in 0..3 {
            cover[i] = cover[i] * (1.0 - k) + built[i] * k;
        }
    }
    let soil = cover;
    let duff = [cover[0] * 0.92, cover[1] * 0.95, cover[2] * 0.92];
    let rock = [0.46, 0.45, 0.42];
    let sand = [0.60, 0.56, 0.46];

    // Flatter ground holds more litter and is darker; ridges wash out.
    let soil = {
        let litter = ((slope_cos - 0.90) / 0.10).clamp(0.0, 1.0) * broad;
        [
            soil[0] * (1.0 - litter) + duff[0] * litter,
            soil[1] * (1.0 - litter) + duff[1] * litter,
            soil[2] * (1.0 - litter) + duff[2] * litter,
        ]
    };

    // Rock takes over as the face steepens; sand only within a few metres of
    // sea level, where the beach actually is.
    let rockiness = ((0.86 - slope_cos) / 0.26).clamp(0.0, 1.0);
    let beach = ((6.0 - elev) / 6.0).clamp(0.0, 1.0) * (slope_cos - 0.8).max(0.0) * 5.0;
    let beach = beach.clamp(0.0, 1.0);

    let mut c = [0.0; 3];
    for i in 0..3 {
        let base = soil[i] * (1.0 - rockiness) + rock[i] * rockiness;
        c[i] = (base * (1.0 - beach) + sand[i] * beach) * mottle;
    }

    // The coast has its own materials. The non-burnable shore strip would
    // otherwise take the garden green of the cover tint.
    // Rocky scrub along the cliff edge, then bare banded limestone where it
    // is steep; sand where it is low and flat.
    let strip = 1.0 - smooth((shore - 40.0) / 70.0);
    c = mix3(c, [0.62 * mottle, 0.58 * mottle, 0.46 * mottle], strip * smooth((elev - 5.0) / 4.0));
    let cliff = smooth((0.95 - slope_cos) / 0.14) * (1.0 - smooth((shore - 90.0) / 280.0));
    c = mix3(c, coast_rock(elev, p), cliff);
    let flat = smooth((slope_cos - 0.88) / 0.08);
    let low = 1.0 - smooth((elev - 5.0) / 5.0);
    let sand_w = flat * low * (1.0 - smooth((shore - 90.0) / 80.0));
    mix3(c, coast_sand(elev, p), sand_w)
}


/// Layer of the terrain's shadow casters: the sun sees it (it is on the sun's
/// layers, see `main.rs`), the camera does not. A [`VisibilityRange`] entity is
/// never queued as a shadow caster (the light's views are not in the range
/// table), so the shadows come from one extra entity per chunk that shares the
/// middle level's mesh.
pub const SHADOW_LAYER: usize = 1;

/// Distance from the camera, metres, to a chunk's centre beyond which the
/// terrain is drawn with twice, and then four times, the base stride (the base
/// is `Graphics::terrain_stride`). The chunk is 640 m wide, so its near edge is
/// up to ~450 m closer than its centre. The switch is abrupt: the colour and
/// the normal come from textures, so only the silhouette changes.
pub const LOD1_M: f32 = 1100.0;
pub const LOD2_M: f32 = 2600.0;

/// Mesh strides per level, relative to the base stride.
const LEVELS: [usize; 3] = [1, 2, 4];

/// Skirt depth as a fraction of the level's posting: hides the cracks where
/// neighbouring chunks are drawn at different strides.
const SKIRT: f32 = 0.6;

fn f16_bits(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32 - 127 + 15;
    let mant = b & 0x7f_ffff;
    if exp >= 31 {
        sign | 0x7c00
    } else if exp <= 0 {
        if exp < -10 {
            return sign;
        }
        let m = mant | 0x80_0000;
        let shift = (14 - exp) as u32;
        sign | ((m >> shift) + ((m >> (shift - 1)) & 1)) as u16
    } else {
        sign.wrapping_add(((exp as u32) << 10) as u16).wrapping_add(((mant >> 13) + ((mant >> 12) & 1)) as u16)
    }
}

fn ground_texture(w: usize, h: usize, data: Vec<u8>, format: TextureFormat) -> Image {
    let mut image = Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// One chunk at one level: samples `c0..=c0+ncx` by `r0..=r0+ncy` of the base
/// grid, every `f`-th, the last one always the chunk's edge; plus a skirt.
#[allow(clippy::too_many_arguments)]
fn level_mesh(
    grid: &Grid,
    (c0, r0): (usize, usize),
    (ncx, ncy): (usize, usize),
    f: usize,
    origin: [f32; 2],
    height_m: f32,
) -> Mesh {
    let xs: Vec<usize> = (0..=ncx.div_ceil(f)).map(|c| c0 + (c * f).min(ncx)).collect();
    let ys: Vec<usize> = (0..=ncy.div_ceil(f)).map(|r| r0 + (r * f).min(ncy)).collect();
    let (nvx, nvy) = (xs.len(), ys.len());
    let mut positions = Vec::with_capacity(nvx * nvy + 2 * (nvx + nvy));
    let mut normals = Vec::with_capacity(positions.capacity());
    for &r in &ys {
        for &c in &xs {
            let i = r * grid.cols + c;
            positions.push([c as f32 * grid.posting - origin[0], grid.elev[i], r as f32 * grid.posting - height_m - origin[1]]);
            normals.push(grid.normal[i]);
        }
    }
    let mut indices = Vec::with_capacity((nvx - 1) * (nvy - 1) * 6 + 12 * (nvx + nvy));
    for r in 0..nvy - 1 {
        for c in 0..nvx - 1 {
            let i = (r * nvx + c) as u32;
            let (right, down) = (i + 1, i + nvx as u32);
            indices.extend_from_slice(&[i, down, right, right, down, down + 1]);
        }
    }
    // skirt: a copy of each edge vertex, dropped; both windings, so it shows
    // from whichever side the gap is seen
    let depth = SKIRT * f as f32 * grid.posting;
    let edges: [Vec<u32>; 4] = [
        (0..nvx).map(|c| c as u32).collect(),
        (0..nvx).map(|c| ((nvy - 1) * nvx + c) as u32).collect(),
        (0..nvy).map(|r| (r * nvx) as u32).collect(),
        (0..nvy).map(|r| (r * nvx + nvx - 1) as u32).collect(),
    ];
    for edge in edges {
        let base = positions.len() as u32;
        for &v in &edge {
            let mut p = positions[v as usize];
            p[1] -= depth;
            let n = normals[v as usize];
            positions.push(p);
            normals.push(n);
        }
        for k in 0..edge.len() as u32 - 1 {
            let (a, b, c, d) = (edge[k as usize], edge[k as usize + 1], base + k, base + k + 1);
            indices.extend_from_slice(&[a, b, c, c, b, d, a, c, b, c, d, b]);
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        // Static geometry: release the CPU copy after GPU upload on
        // native too. Height queries and Mario's floor use the terrain
        // data directly, not this rendered mesh.
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compress_indices();
    mesh
}

/// The shaped heightfield and its normals at the base stride.
struct Grid {
    cols: usize,
    posting: f32,
    elev: Vec<f32>,
    /// In Bevy's frame (z flipped).
    normal: Vec<[f32; 3]>,
}

pub fn build(
    scn: &Scenario,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<TerrainMaterial>,
    images: &mut Assets<Image>,
    gfx: &crate::quality::Graphics,
) {
    // Terrain samples per base mesh vertex (`Graphics::terrain_stride`): every
    // sample natively; in the browser every fourth (20 m), which WebGL can
    // draw. The data (and so the agents and every height lookup) stay at full
    // resolution. The colour and normal textures have one texel per base
    // vertex; natively, chunks farther away are meshed at 2x and 4x that
    // stride (`LOD1_M`, `LOD2_M`) and read the same textures.
    let stride = gfx.terrain_stride.max(1);
    let t = &scn.terrain;
    let mut mask = crate::ground_mask::build(scn, gfx);
    // beyond this distance from the eye there are no real blades: the terrain
    // shader adds a grain there so the meadow keeps its contrast (0 = no blades)
    mask.rect.w = if gfx.cover_m > 0.0 { crate::near_cover::BANDS_M[crate::near_cover::BANDS_M.len() - 1] } else { 0.0 };
    let _ = images.insert(&crate::ground_mask::GRASS_MAP, crate::ground_mask::grass_map(scn));

    // the base grid: every `stride`-th sample
    let (cols, rows, posting) = ((t.cols - 1) / stride + 1, (t.rows - 1) / stride + 1, t.posting * stride as f32);
    let shore = ShoreField::new(scn);
    let sea = mask.sea;

    // Per sample: shaped height, normal and ground colour. The colour and the
    // normal go to textures, so the look does not depend on the mesh density.
    let mut grid = Grid { cols, posting, elev: Vec::with_capacity(cols * rows), normal: Vec::with_capacity(cols * rows) };
    let mut colour_px: Vec<u8> = Vec::with_capacity(cols * rows * 4);
    let mut normal_px: Vec<u8> = Vec::with_capacity(cols * rows * 8);
    for r in 0..rows {
        for c in 0..cols {
            let gx = c as f32 * posting;
            // row 0 is the north edge; world +y is north
            let gy = t.height_m - r as f32 * posting;
            let p = Pos { x: gx, y: gy };
            let raw = t.elev[(r * stride).min(t.rows - 1) * t.cols + (c * stride).min(t.cols - 1)];
            let shore_d = shore.at(p);
            let elev = cliff_elev(scn, &shore, p, raw);
            // Deliberately the plain heightfield normal, not a
            // noise-perturbed one: a jittered normal feeds
            // straight into the directional light's shadow-map bias,
            // and at a jitter scale finer than the shadow map texel
            // it produces exactly the blotchy close-range
            // self-shadowing this comment is here to stop someone
            // reintroducing (it shipped once, briefly).
            let n = if shore_d > COAST_REACH_M {
                t.normal_at(p)
            } else {
                // the same formula as `normal_at`, over the shaped ground
                let d = posting;
                let h = |q: Pos| cliff_elev(scn, &shore, q, t.height_at(q));
                let (hl, hr) = (h(Pos { x: p.x - d, ..p }), h(Pos { x: p.x + d, ..p }));
                let (hd, hu) = (h(Pos { y: p.y - d, ..p }), h(Pos { y: p.y + d, ..p }));
                let (nx, ny, nz) = (hl - hr, 2.0 * d, hd - hu);
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                [nx / len, ny / len, nz / len]
            };
            let n = [n[0], n[1], -n[2]];
            grid.elev.push(elev);
            grid.normal.push(n);
            for v in [n[0], n[1], n[2], 0.0] {
                normal_px.extend_from_slice(&f16_bits(v).to_le_bytes());
            }
            let col = ground_color(scn, &sea, elev, n[1], p, shore_d);
            colour_px.extend(col.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
            colour_px.push(255);
        }
    }
    let colour_tex = images.add(ground_texture(cols, rows, colour_px, TextureFormat::Rgba8UnormSrgb));
    let normal_tex = images.add(ground_texture(cols, rows, normal_px, TextureFormat::Rgba16Float));
    // world metres to texel-centre uv (`terrain_uv` in the shader)
    let tex_rect = Vec4::new(
        1.0 / (posting * cols as f32),
        1.0 / (posting * rows as f32),
        0.5 / cols as f32,
        t.height_m / (posting * rows as f32) + 0.5 / rows as f32,
    );

    let _ = images.insert(&crate::ground_mask::ROAD_MASK, mask.image);
    let material = materials.add(TerrainMaterial::new(
        mask.rect,
        tex_rect,
        crate::ground_mask::ROAD_MASK,
        crate::ground_mask::GRASS_MAP,
        colour_tex,
        normal_tex,
    ));

    let levels: &[usize] = if stride >= 4 { &LEVELS[..1] } else { &LEVELS };
    // distances at which each level hands over to the next
    let cuts = [LOD1_M, LOD2_M];
    let (chunks_x, chunks_y) = ((cols - 1).div_ceil(CHUNK), (rows - 1).div_ceil(CHUNK));
    let (mut count, mut submerged, mut tris) = (0, 0, [0usize; 3]);

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let (c0, r0) = (cx * CHUNK, cy * CHUNK);
            // neighbouring chunks share an edge sample and leave no seam
            let (ncx, ncy) = (CHUNK.min(cols - 1 - c0), CHUNK.min(rows - 1 - r0));
            if ncx < 1 || ncy < 1 {
                continue;
            }
            // each level's entity sits at the chunk's centre: that is where
            // its visibility range measures from
            let origin = [(c0 as f32 + ncx as f32 * 0.5) * posting, (r0 as f32 + ncy as f32 * 0.5) * posting - t.height_m];
            let centre = Vec3::new(origin[0], 0.0, origin[1]);
            let under = (r0..=r0 + ncy).all(|r| (c0..=c0 + ncx).all(|c| grid.elev[r * cols + c] < SUBMERGED_M));
            submerged += usize::from(under);
            // a hidden chunk keeps only the finest level
            let used = if under { &levels[..1] } else { levels };
            for (k, &f) in used.iter().enumerate() {
                let mesh = level_mesh(&grid, (c0, r0), (ncx, ncy), f, origin, t.height_m);
                tris[k] += mesh.indices().map_or(0, |i| i.len() / 3);
                let mesh = meshes.add(mesh);
                let mut chunk = commands.spawn((
                    (
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::from_translation(centre),
                        // still spawned: Mario swims over it, and needs a floor
                        if under { Visibility::Hidden } else { Visibility::Inherited },
                    ),
                    TerrainChunk,
                    PauseSolid,
                ));
                if used.len() > 1 {
                    let near = if k == 0 { 0.0 } else { cuts[k - 1] };
                    let far = cuts.get(k).copied().unwrap_or(1.0e9);
                    chunk.insert((VisibilityRange::abrupt(near, far), bevy::light::NotShadowCaster));
                } else if !gfx.terrain_shadows {
                    chunk.insert(bevy::light::NotShadowCaster);
                }
                // the shadows come from the middle level, on the sun's layer only
                if used.len() > 1 && k == 1 {
                    let mut shadow = commands.spawn((
                        (
                            Mesh3d(mesh),
                            MeshMaterial3d(material.clone()),
                            Transform::from_translation(centre),
                        ),
                        TerrainChunk,
                        RenderLayers::layer(SHADOW_LAYER),
                    ));
                    if !gfx.terrain_shadows {
                        shadow.insert(bevy::light::NotShadowCaster);
                    }
                }
            }
            count += 1;
        }
    }

    info!(
        "terrain: {count} chunks ({submerged} under the sea, not drawn), {:.2} M samples @ {} m posting, levels {:?} x base stride {stride}, triangles per level {:?}",
        (t.rows * t.cols) as f32 / 1e6,
        t.posting,
        &levels,
        tris
    );
}

/// Centre of a fire cell, lifted onto the terrain.
pub fn cell_ground(scn: &Scenario, c: Cell) -> (Pos, f32) {
    let p = scn.world.centre_of(c);
    (p, scn.terrain.height_at(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cliffs are shaped only where they should be: a few metres at most,
    /// nothing at the waterline, a minority of the vertices.
    #[test]
    fn cliffs_are_shaped_gently_and_only_on_the_coast() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let Ok(scn) = Scenario::load(&dir) else { return };
        let shore = ShoreField::new(&scn);
        let t = &scn.terrain;
        let (mut shaped, mut total, mut worst) = (0usize, 0usize, 0.0f32);
        for r in (0..t.rows).step_by(3) {
            for c in (0..t.cols).step_by(3) {
                let p = Pos { x: c as f32 * t.posting, y: t.height_m - r as f32 * t.posting };
                let e = t.elev[r * t.cols + c];
                let d = cliff_elev(&scn, &shore, p, e) - e;
                total += 1;
                if d != 0.0 {
                    shaped += 1;
                    worst = worst.max(d.abs());
                    assert!(e >= 1.5, "{e} m shaped at {p:?}");
                    assert!(shore.at(p) <= COAST_REACH_M);
                }
            }
        }
        eprintln!("cliffs: {shaped}/{total} vertices shaped, worst {worst:.2} m");
        assert!(worst < 5.0, "{worst}");
        assert!(shaped * 6 < total, "{shaped}/{total}");
    }
}
