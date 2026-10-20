//! Road mask: a signed-distance texture of the road network, sampled by the
//! terrain's fragment shader (`shaders/terrain.wgsl`).
//!
//! The terrain mesh colours per vertex, one every 5 m, which is too coarse for
//! a 10 m road. Rasterising the network once into a texture lets the shader draw
//! surface, kerb and junctions per pixel: no geometry laid over the ground, so
//! nothing to z-fight, and a junction is the union of its roads by construction.
//!
//! Channels: drivable roads and tracks (distances), and the share of paved
//! ground (town squares and yards).
//! The first two hold Each stores the distance to the
//! nearest ribbon *edge* (negative inside), clamped to ±[`RANGE_M`] and mapped
//! to 0..255. A distance field survives box-filtered mip levels, so a trilinear
//! sample is a plain blur of the roads at a distance, never a hole in them.

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::texture::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use scenario::{Pos, Scenario};

/// Metres per texel. The browser takes a quarter of the memory.
const RES_M: f32 = if cfg!(target_arch = "wasm32") { 4.0 } else { 2.0 };
/// Half the encoded range, metres. Must match `RANGE_M` in the shader.
const RANGE_M: f32 = 12.0;
/// Half-widths of the ribbons, metres. Must match the shader.
pub const DRIVABLE_HALF_W: f32 = 4.8;
pub const TRACK_HALF_W: f32 = 1.6;

/// Where the mask lives in `Assets<Image>`: a fixed weak handle, so the fire's
/// ground overlay can bind it without waiting for the terrain to be built.
pub const ROAD_MASK: Handle<Image> = Handle::weak_from_u128(0x5A3C_91E0_77B2_4D18_8E41_C0A6_1F2D_7B09);

/// Which texels are open sea (low ground connected to the window's edge), so
/// an inland hollow below sea level is not painted as water.
pub struct SeaGrid {
    nx: usize,
    ny: usize,
    res: f32,
    sea: Vec<bool>,
}

impl SeaGrid {
    pub fn is_sea(&self, p: Pos) -> bool {
        let i = ((p.x / self.res) as usize).min(self.nx - 1);
        let j = ((p.y / self.res) as usize).min(self.ny - 1);
        self.sea[j * self.nx + i]
    }
}

pub struct RoadMask {
    pub image: Image,
    pub sea: SeaGrid,
    /// `(1 / width, 1 / height, 0, 0)`: world metres to texture coordinates.
    pub rect: Vec4,
}

pub fn build(scn: &Scenario) -> RoadMask {
    let (w_m, h_m) = (scn.world.width_m, scn.world.height_m);
    let (nx, ny) = ((w_m / RES_M).ceil() as usize, (h_m / RES_M).ceil() as usize);
    let mut field = [vec![RANGE_M; nx * ny], vec![RANGE_M; nx * ny]];

    for road in &scn.vectors.roads {
        let (ch, half) = if road.drivable {
            (0, DRIVABLE_HALF_W)
        } else if road.track {
            (1, TRACK_HALF_W)
        } else {
            continue;
        };
        for seg in road.line.windows(2) {
            stamp(&mut field[ch], nx, ny, seg[0], seg[1], half);
        }
    }

    let paved = paved_field(scn, nx, ny);
    let (shore, sea) = shore_field(scn, nx, ny);

    // Row j is world y = j * RES_M (south first), so v = y / height.
    // R: drivable distance, G: track distance, B: paved share, A: spare.
    let mut level: Vec<u8> = Vec::with_capacity(nx * ny * 4);
    for i in 0..nx * ny {
        level.push(encode(field[0][i]));
        level.push(encode(field[1][i]));
        level.push(paved[i]);
        level.push(shore[i]);
    }
    drop(field);

    let mut data = level.clone();
    let (mut lw, mut lh, mut levels) = (nx, ny, 1);
    while lw > 1 || lh > 1 {
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        let mut next = Vec::with_capacity(nw * nh * 4);
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let at = |xx: usize, yy: usize| level[(yy.min(lh - 1) * lw + xx.min(lw - 1)) * 4 + c] as u32;
                    let s = at(2 * x, 2 * y) + at(2 * x + 1, 2 * y) + at(2 * x, 2 * y + 1) + at(2 * x + 1, 2 * y + 1);
                    next.push(((s + 2) / 4) as u8);
                }
            }
        }
        data.extend_from_slice(&next);
        level = next;
        (lw, lh) = (nw, nh);
        levels += 1;
    }

    let mut image = Image::new(
        Extent3d { width: nx as u32, height: ny as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    info!("road mask: {nx}x{ny} at {RES_M} m, {levels} levels");
    RoadMask { image, rect: Vec4::new(1.0 / w_m, 1.0 / h_m, if shader_grass() { 1.0 } else { 0.0 }, 0.0), sea: SeaGrid { nx, ny, res: RES_M, sea } }
}

/// Share of paved ground (roads, buildings and yards of the cover raster) at
/// each texel, as 0..255. The cover raster is 5 m; sampling its four nearest
/// cells and blurring once turns the squares into a smooth field the shader
/// can threshold into a crisp, noise-ragged edge.
fn paved_field(scn: &Scenario, nx: usize, ny: usize) -> Vec<u8> {
    use scenario::CoverClass;
    let mut out = vec![0u8; nx * ny];
    let Some(cover) = &scn.cover else { return out };
    let h = cover.cell_m * 0.5;
    let is = |x: f32, y: f32| {
        matches!(cover.at(scenario::Pos { x, y }), CoverClass::Road | CoverClass::Building | CoverClass::Yard) as u8
    };
    let mut raw = vec![0u8; nx * ny];
    for j in 0..ny {
        let y = (j as f32 + 0.5) * RES_M;
        for i in 0..nx {
            let x = (i as f32 + 0.5) * RES_M;
            raw[j * nx + i] = is(x - h, y - h) + is(x + h, y - h) + is(x - h, y + h) + is(x + h, y + h);
        }
    }
    // 5x5 box blur, separable: 0..4 -> 0..255.
    let r = 2usize;
    let mut tmp = vec![0u16; nx * ny];
    for j in 0..ny {
        for i in 0..nx {
            let lo = i.saturating_sub(r);
            let hi = (i + r).min(nx - 1);
            tmp[j * nx + i] = (lo..=hi).map(|k| u16::from(raw[j * nx + k])).sum::<u16>() * 8 / (hi - lo + 1) as u16;
        }
    }
    for j in 0..ny {
        let lo = j.saturating_sub(r);
        let hi = (j + r).min(ny - 1);
        for i in 0..nx {
            let sum: u32 = (lo..=hi).map(|k| u32::from(tmp[k * nx + i])).sum();
            out[j * nx + i] = ((sum / (hi - lo + 1) as u32) * 255 / 32).min(255) as u8;
        }
    }
    out
}

/// Ground at or below this is sea: the rule `sea::WATER_ELEV_MAX` uses.
const WATER_ELEV_MAX: f32 = 0.6;

/// Signed distance to the waterline, metres, as 0..255: 127.5 at the shore,
/// above it in the water, below it on land; 1 m a step, so ±127 m.
fn shore_field(scn: &Scenario, nx: usize, ny: usize) -> (Vec<u8>, Vec<bool>) {
    let n = nx * ny;
    let mut water = vec![false; n];
    for j in 0..ny {
        let y = (j as f32 + 0.5) * RES_M;
        for i in 0..nx {
            let x = (i as f32 + 0.5) * RES_M;
            water[j * nx + i] = scn.terrain.height_at(Pos { x, y }) <= WATER_ELEV_MAX;
        }
    }
    // Only low ground reachable from the window's edge is sea.
    let mut sea = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..nx {
        stack.push(i);
        stack.push((ny - 1) * nx + i);
    }
    for j in 0..ny {
        stack.push(j * nx);
        stack.push(j * nx + nx - 1);
    }
    while let Some(k) = stack.pop() {
        if sea[k] || !water[k] {
            continue;
        }
        sea[k] = true;
        let (i, j) = (k % nx, k / nx);
        if i > 0 { stack.push(k - 1); }
        if i + 1 < nx { stack.push(k + 1); }
        if j > 0 { stack.push(k - nx); }
        if j + 1 < ny { stack.push(k + nx); }
    }
    let water = sea.clone();
    // Distance from each water texel to land, and from each land texel to water.
    let to_land = chamfer(&water.iter().map(|w| !w).collect::<Vec<_>>(), nx, ny);
    let to_water = chamfer(&water, nx, ny);
    let field = (0..n)
        .map(|i| {
            let d = if water[i] { to_land[i] } else { -to_water[i] } * RES_M;
            (d.clamp(-127.0, 127.0) + 127.5) as u8
        })
        .collect();
    (field, sea)
}

/// Distance in texels to the nearest `seed` texel (0 on it), by the two-pass
/// chamfer: within a few percent of Euclidean, and linear in the texel count.
fn chamfer(seed: &[bool], nx: usize, ny: usize) -> Vec<f32> {
    const D: f32 = std::f32::consts::SQRT_2;
    let mut d: Vec<f32> = seed.iter().map(|&s| if s { 0.0 } else { 1.0e6 }).collect();
    for j in 0..ny {
        for i in 0..nx {
            let k = j * nx + i;
            let mut v = d[k];
            if i > 0 { v = v.min(d[k - 1] + 1.0); }
            if j > 0 {
                v = v.min(d[k - nx] + 1.0);
                if i > 0 { v = v.min(d[k - nx - 1] + D); }
                if i + 1 < nx { v = v.min(d[k - nx + 1] + D); }
            }
            d[k] = v;
        }
    }
    for j in (0..ny).rev() {
        for i in (0..nx).rev() {
            let k = j * nx + i;
            let mut v = d[k];
            if i + 1 < nx { v = v.min(d[k + 1] + 1.0); }
            if j + 1 < ny {
                v = v.min(d[k + nx] + 1.0);
                if i + 1 < nx { v = v.min(d[k + nx + 1] + D); }
                if i > 0 { v = v.min(d[k + nx - 1] + D); }
            }
            d[k] = v;
        }
    }
    d
}

fn encode(d: f32) -> u8 {
    ((d / RANGE_M).clamp(-1.0, 1.0) * 0.5 + 0.5).mul_add(255.0, 0.0).round() as u8
}

/// Min the distance-to-edge of one segment of half-width `half` into `field`.
fn stamp(field: &mut [f32], nx: usize, ny: usize, a: [f32; 2], b: [f32; 2], half: f32) {
    let reach = half + RANGE_M;
    let lo = |v: f32| ((v - reach) / RES_M).floor().max(0.0) as usize;
    let (x0, x1) = (lo(a[0].min(b[0])), (((a[0].max(b[0]) + reach) / RES_M).ceil() as usize).min(nx));
    let (y0, y1) = (lo(a[1].min(b[1])), (((a[1].max(b[1]) + reach) / RES_M).ceil() as usize).min(ny));
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    for j in y0..y1 {
        let py = (j as f32 + 0.5) * RES_M;
        for i in x0..x1 {
            let px = (i as f32 + 0.5) * RES_M;
            let t = if len2 < 1e-6 { 0.0 } else { (((px - a[0]) * dx + (py - a[1]) * dy) / len2).clamp(0.0, 1.0) };
            let (cx, cy) = (a[0] + dx * t - px, a[1] + dy * t - py);
            let d = (cx * cx + cy * cy).sqrt() - half;
            let v = &mut field[j * nx + i];
            if d < *v {
                *v = d;
            }
        }
    }
}

/// Whether meadows are drawn by the terrain shader (default) or as meshes
/// (`ROCCA_GRASS_MESH=1`, for comparing the two).
pub fn shader_grass() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("ROCCA_GRASS_MESH").is_none())
}

pub const GRASS_MAP: Handle<Image> = Handle::weak_from_u128(0x2F7B_C4D1_0A93_4E66_B5C8_91D3_6E0A_48F2);

/// Meadow cover on the fire grid, one texel per cell, row 0 at the south so
/// that `v = y / height` indexes it like the mask. 255 where the fuel is
/// grassland (classes 1-3). The shader thresholds the bilinear blend with
/// noise, which is what makes the meadow's edge ragged and not 20 m squares.
pub fn grass_map(scn: &Scenario) -> Image {
    let w = &scn.world;
    let mut data = vec![0u8; w.fire_cols * w.fire_rows];
    for r in 0..w.fire_rows {
        for c in 0..w.fire_cols {
            if matches!(scn.fuel_at(scenario::Cell { row: r, col: c }), 1..=3) {
                data[(w.fire_rows - 1 - r) * w.fire_cols + c] = 255;
            }
        }
    }
    let mut image = Image::new(
        Extent3d { width: w.fire_cols as u32, height: w.fire_rows as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
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
