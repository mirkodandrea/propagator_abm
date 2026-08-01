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

/// The stage grid is deliberately coarser than the simulation raster. It is a
/// navigation aid, not a rendering of implementation detail.
const VR_GRID_MINOR_M: f32 = 100.0;
const VR_GRID_MAJOR_M: f32 = 500.0;
const VR_GRID_LIFT_M: f32 = 0.8;

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

/// The dev floor stays quiet beneath a separate sparse stage grid. A broad,
/// deliberately subtle variation keeps the ground legible without exposing
/// the simulation's cell boundaries.
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

    if let Some(pal) = pal {
        build_vr_grid(scn, pal, commands, meshes, materials);
    }
}

fn build_vr_grid(
    scn: &Scenario,
    pal: scenario::VrPalette,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<RetroMaterial>,
) {
    let colour = |c: [f32; 3], strength: f32, alpha: f32| {
        Color::srgba(
            c[0] * strength,
            c[1] * strength,
            c[2] * strength,
            alpha,
        )
    };
    let mut material = |base_color: Color| {
        materials.add(retro::material_with_style(
            StandardMaterial {
                base_color,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            },
            true,
            retro::RetroStyle::BACKGROUND,
        ))
    };

    let minor_mat = material(colour(pal.grid, 0.28, 0.30));
    let major_mat = material(colour(pal.grid, 0.48, 0.48));
    let border_mat = material(colour(pal.accent, 0.62, 0.68));

    let mut minor = VrGridBuilder::default();
    let mut major = VrGridBuilder::default();

    let mut x = 0.0;
    while x <= scn.world.width_m + 0.1 {
        let is_major = is_major_grid_line(x);
        let dst = if is_major { &mut major } else { &mut minor };
        dst.line(
            scn,
            Pos { x, y: 0.0 },
            Pos {
                x,
                y: scn.world.height_m,
            },
            if is_major { 2.1 } else { 0.9 },
        );
        x += VR_GRID_MINOR_M;
    }

    let mut y = 0.0;
    while y <= scn.world.height_m + 0.1 {
        let is_major = is_major_grid_line(y);
        let dst = if is_major { &mut major } else { &mut minor };
        dst.line(
            scn,
            Pos { x: 0.0, y },
            Pos {
                x: scn.world.width_m,
                y,
            },
            if is_major { 2.1 } else { 0.9 },
        );
        y += VR_GRID_MINOR_M;
    }

    let mut border = VrGridBuilder::default();
    let w = scn.world.width_m;
    let h = scn.world.height_m;
    border.line(scn, Pos { x: 0.0, y: 0.0 }, Pos { x: w, y: 0.0 }, 3.2);
    border.line(scn, Pos { x: w, y: 0.0 }, Pos { x: w, y: h }, 3.2);
    border.line(scn, Pos { x: w, y: h }, Pos { x: 0.0, y: h }, 3.2);
    border.line(scn, Pos { x: 0.0, y: h }, Pos { x: 0.0, y: 0.0 }, 3.2);

    for (builder, material) in [
        (minor, minor_mat),
        (major, major_mat),
        (border, border_mat),
    ] {
        if let Some(mesh) = builder.finish() {
            commands.spawn((
                MaterialMeshBundle::<RetroMaterial> {
                    mesh: meshes.add(mesh),
                    material,
                    ..default()
                },
                VrStageGrid,
            ));
        }
    }
}

fn is_major_grid_line(v: f32) -> bool {
    (v / VR_GRID_MAJOR_M).fract().abs() < 0.001
}

#[derive(Component)]
pub struct VrStageGrid;

#[derive(Default)]
struct VrGridBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl VrGridBuilder {
    fn line(&mut self, scn: &Scenario, a: Pos, b: Pos, half_width: f32) {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.1 {
            return;
        }

        let tx = dx / len;
        let ty = dy / len;
        let px = -ty * half_width;
        let py = tx * half_width;
        let step = scn.terrain.posting.max(5.0);
        let segments = (len / step).ceil().max(1.0) as usize;
        let start = self.positions.len() as u32;

        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let p = Pos {
                x: a.x + dx * t,
                y: a.y + dy * t,
            };
            for side in [-1.0, 1.0] {
                let q = Pos {
                    x: p.x + px * side,
                    y: p.y + py * side,
                };
                let height = scn.terrain.height_at(q) + VR_GRID_LIFT_M;
                self.positions.push([q.x, height, -q.y]);
                self.normals.push([0.0, 1.0, 0.0]);
                self.uvs.push([t, (side + 1.0) * 0.5]);
            }
        }

        for i in 0..segments as u32 {
            let v = start + i * 2;
            self.indices
                .extend_from_slice(&[v, v + 1, v + 2, v + 1, v + 3, v + 2]);
        }
    }

    fn finish(self) -> Option<Mesh> {
        if self.positions.is_empty() {
            return None;
        }

        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_indices(Indices::U32(self.indices));
        Some(mesh)
    }
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
