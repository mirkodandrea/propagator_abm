//! Building the terrain mesh from the 5 m render heightfield.
//!
//! The full field is 2048² = 4.19 M vertices, which is far too much for one
//! mesh: it defeats frustum culling and blows past the 16-bit index limit. It
//! is split into chunks instead, each its own entity, so the renderer can cull
//! and the driver gets sensibly sized buffers.
//!
//! The terrain is **ground**, not vegetation. It used to be painted with the
//! fuel raster's classes, which read as a camouflage patchwork however much
//! the boundaries were warped — the underlying data is 20 m and no amount of
//! colour dithering hides that. Vegetation is now drawn as actual plants (see
//! [`crate::vegetation`]), and this layer paints only what is under them:
//! soil, rock on the steep faces, sand at the shore, sea below the waterline.
//! Ground colour therefore varies with slope, elevation and noise — geology,
//! not land cover — which is both honest and far better looking.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::{Cell, Cover, CoverClass, Pos, Scenario};

use crate::field::noise;
use crate::retro;
use crate::retro::RetroMaterial;

/// Samples per chunk edge. 128 keeps each chunk at ~16 k vertices.
const CHUNK: usize = 128;
/// Terrain samples per mesh vertex: every sample natively; in the browser
/// every fourth (20 m), which WebGL can draw. The data (and so the agents and
/// every height lookup) stay at full resolution.
const STRIDE: usize = if cfg!(target_arch = "wasm32") { 4 } else { 1 };

#[derive(Component)]
pub struct TerrainChunk;

const PAVING: [f32; 3] = [0.84, 0.81, 0.74];
const GARDEN: [f32; 3] = [0.50, 0.66, 0.36];

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
            CoverClass::Irrigated => [0.46, 0.68, 0.32],
            CoverClass::Water => [0.20, 0.36, 0.44],
            CoverClass::Road | CoverClass::Building | CoverClass::Yard => PAVING,
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
            _ if scn.cover.is_some() => GARDEN,
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
fn ground_color(scn: &Scenario, elev: f32, slope_cos: f32, p: Pos) -> [f32; 3] {
    if elev <= 0.5 {
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
    c
}


pub fn build(
    scn: &Scenario,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<RetroMaterial>,
) {
    let t = &scn.terrain;
    let material = materials.add(retro::material(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        // Dry karst, not wet slate: a low reflectance keeps the sun's
        // specular lobe from painting a bright sheet across every ridge.
        reflectance: 0.02,
        ..default()
    // The terrain is the backdrop, not a participating neon object: keep the
    // dev floor matte and opt it out of the animated edge treatment.
    }, false));

    // the mesh's own grid: every STRIDE-th sample
    let (cols, rows, posting) = ((t.cols - 1) / STRIDE + 1, (t.rows - 1) / STRIDE + 1, t.posting * STRIDE as f32);
    let chunks_x = (cols - 1).div_ceil(CHUNK);
    let chunks_y = (rows - 1).div_ceil(CHUNK);
    let mut count = 0;

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let c0 = cx * CHUNK;
            let r0 = cy * CHUNK;
            // +1 so neighbouring chunks share an edge and leave no seam
            let cn = (CHUNK + 1).min(cols - c0);
            let rn = (CHUNK + 1).min(rows - r0);
            if cn < 2 || rn < 2 {
                continue;
            }

            let mut positions = Vec::with_capacity(cn * rn);
            let mut normals = Vec::with_capacity(cn * rn);
            let mut colors = Vec::with_capacity(cn * rn);
            let mut uvs = Vec::with_capacity(cn * rn);

            for r in 0..rn {
                for c in 0..cn {
                    let gx = (c0 + c) as f32 * posting;
                    // row 0 is the north edge; world +y is north
                    let gy = t.height_m - (r0 + r) as f32 * posting;
                    let p = Pos { x: gx, y: gy };
                    let elev = t.elev[(r0 + r) * STRIDE * t.cols + (c0 + c) * STRIDE];

                    positions.push([gx, elev, -gy]);
                    // Deliberately the plain heightfield normal, not a
                    // noise-perturbed one: a per-vertex jittered normal feeds
                    // straight into the directional light's shadow-map bias,
                    // and at a jitter scale finer than the shadow map texel
                    // it produces exactly the blotchy close-range
                    // self-shadowing this comment is here to stop someone
                    // reintroducing (it shipped once, briefly).
                    let n = t.normal_at(p);
                    normals.push([n[0], n[1], -n[2]]);

                    let col = ground_color(scn, elev, n[1], p);
                    let col = Color::srgb(col[0], col[1], col[2]).to_linear();
                    colors.push([col.red, col.green, col.blue, 1.0]);
                    uvs.push([c as f32 / cn as f32, r as f32 / rn as f32]);
                }
            }

            let mut indices = Vec::with_capacity((cn - 1) * (rn - 1) * 6);
            for r in 0..rn - 1 {
                for c in 0..cn - 1 {
                    let i = (r * cn + c) as u32;
                    let right = i + 1;
                    let down = i + cn as u32;
                    let diag = down + 1;
                    indices.extend_from_slice(&[i, down, right, right, down, diag]);
                }
            }

            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
            mesh.insert_indices(Indices::U32(indices));

            commands.spawn((
                MaterialMeshBundle::<RetroMaterial> {
                    mesh: meshes.add(mesh),
                    material: material.clone(),
                    ..default()
                },
                TerrainChunk,
            ));
            count += 1;
        }
    }

    info!(
        "terrain: {count} chunks, {:.2} M vertices @ {} m posting",
        (t.rows * t.cols) as f32 / 1e6,
        t.posting
    );

}

/// Centre of a fire cell, lifted onto the terrain.
pub fn cell_ground(scn: &Scenario, c: Cell) -> (Pos, f32) {
    let p = scn.world.centre_of(c);
    (p, scn.terrain.height_at(p))
}
