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
use scenario::{Cell, Pos, Scenario};

use crate::field::noise;
use crate::retro;
use crate::retro::RetroMaterial;

/// Samples per chunk edge. 128 keeps each chunk at ~16 k vertices.
const CHUNK: usize = 128;

#[derive(Component)]
pub struct TerrainChunk;

/// Ground colour from elevation, steepness and noise.
///
/// `slope_cos` is the vertical component of the surface normal: 1 on the flat,
/// falling toward 0 on a cliff. Ligurian ground is dry pale limestone soil,
/// with bare rock showing wherever it is too steep to hold any.
fn ground_color(elev: f32, slope_cos: f32, p: Pos) -> [f32; 3] {
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
    let soil = [0.30, 0.28, 0.22];
    let duff = [0.25, 0.25, 0.19];
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

/// The dev floor is a void, not a coordinate display.  The old 40 m grid was
/// useful while debugging placement, but it dominated the scene and made the
/// terrain look like a wire lattice.  A broad, deliberately quiet variation
/// keeps the floor legible without exposing the simulation's cell boundaries.
fn vr_floor_color(pal: scenario::VrPalette, p: Pos) -> [f32; 3] {
    let drift = 0.94 + 0.10 * noise(p.x / 180.0, p.y / 180.0, 0x5A17);
    [
        pal.void[0] * drift + 0.008,
        pal.void[1] * drift + 0.012,
        pal.void[2] * drift + 0.024,
    ]
}

pub fn build(
    scn: &Scenario,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<RetroMaterial>,
) {
    let t = &scn.terrain;
    let pal = scn.vr_palette();
    let material = materials.add(retro::material(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        // Dry karst, not wet slate: a low reflectance keeps the sun's
        // specular lobe from painting a bright sheet across every ridge.
        reflectance: 0.02,
        // VR-training dev scenarios are a flat unlit void floor, not lit
        // ground — no sun to bounce off of.
        unlit: pal.is_some(),
        ..default()
    // The terrain is the backdrop, not a participating neon object: keep the
    // dev floor matte and opt it out of the animated edge treatment.
    }, false));

    let chunks_x = (t.cols - 1).div_ceil(CHUNK);
    let chunks_y = (t.rows - 1).div_ceil(CHUNK);
    let mut count = 0;

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let c0 = cx * CHUNK;
            let r0 = cy * CHUNK;
            // +1 so neighbouring chunks share an edge and leave no seam
            let cn = (CHUNK + 1).min(t.cols - c0);
            let rn = (CHUNK + 1).min(t.rows - r0);
            if cn < 2 || rn < 2 {
                continue;
            }

            let mut positions = Vec::with_capacity(cn * rn);
            let mut normals = Vec::with_capacity(cn * rn);
            let mut colors = Vec::with_capacity(cn * rn);
            let mut uvs = Vec::with_capacity(cn * rn);

            for r in 0..rn {
                for c in 0..cn {
                    let gx = (c0 + c) as f32 * t.posting;
                    // row 0 is the north edge; world +y is north
                    let gy = t.height_m - (r0 + r) as f32 * t.posting;
                    let p = Pos { x: gx, y: gy };
                    let elev = t.elev[(r0 + r) * t.cols + (c0 + c)];

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

                    let col = match pal {
                        Some(pal) => vr_floor_color(pal, p),
                        None => ground_color(elev, n[1], p),
                    };
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

/// Height lookup used when placing anything on the ground.
pub fn ground(scn: &Scenario, p: Pos) -> f32 {
    scn.terrain.height_at(p)
}

/// Centre of a fire cell, lifted onto the terrain.
pub fn cell_ground(scn: &Scenario, c: Cell) -> (Pos, f32) {
    let p = scn.world.centre_of(c);
    (p, scn.terrain.height_at(p))
}
