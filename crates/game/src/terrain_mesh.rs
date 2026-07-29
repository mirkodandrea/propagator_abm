//! Building the terrain mesh from the 5 m render heightfield.
//!
//! The full field is 2048² = 4.19 M vertices, which is far too much for one
//! mesh: it defeats frustum culling and blows past the 16-bit index limit. It
//! is split into chunks instead, each its own entity, so the renderer can cull
//! and the driver gets sensibly sized buffers.
//!
//! Vertex colour comes from the 20 m fuel raster, sampled per vertex. That is
//! the one place the coarse grid is allowed to show through visually, and it
//! reads as vegetation patches rather than stair-steps because the geometry
//! underneath is smooth.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::{Cell, Pos, Scenario};

/// Samples per chunk edge. 128 keeps each chunk at ~16 k vertices.
const CHUNK: usize = 128;

#[derive(Component)]
pub struct TerrainChunk;

/// Colour per `eu_fuel12` class. Deliberately desaturated: the fire overlay
/// has to read clearly on top of this.
fn fuel_color(fuel: i32, elev: f32) -> Color {
    let c = match fuel {
        1..=3 => Color::srgb(0.62, 0.63, 0.36),   // grassland
        4..=6 => Color::srgb(0.24, 0.38, 0.21),   // broadleaves
        7..=9 => Color::srgb(0.44, 0.47, 0.26),   // shrubs / macchia
        10..=12 => Color::srgb(0.15, 0.29, 0.20), // conifers
        _ if elev <= 0.5 => Color::srgb(0.10, 0.16, 0.30), // sea
        _ => Color::srgb(0.42, 0.40, 0.36),       // bare / built
    };
    c
}

pub fn build(
    scn: &Scenario,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let t = &scn.terrain;
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..default()
    });

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
                    let n = t.normal_at(p);
                    normals.push([n[0], n[1], -n[2]]);

                    let cell = scn.world.cell_of(p);
                    let fuel = scn.fuel[cell.row * scn.world.fire_cols + cell.col];
                    let col = fuel_color(fuel, elev).to_linear();
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
                PbrBundle {
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
