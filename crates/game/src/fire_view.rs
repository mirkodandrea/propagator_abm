//! Rendering the fire.
//!
//! Burning and burnt cells are drawn as one rebuilt mesh rather than one
//! entity per cell: an active front can be thousands of cells, and spawning
//! entities per cell per tick would dominate the frame. The mesh is rebuilt
//! only when the sim generation changes, not every frame.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use fire::CellFire;
use scenario::Cell;

use crate::sim::Sim;

#[derive(Component)]
pub struct FireMesh;

#[derive(Resource)]
pub struct FireView {
    mesh: Handle<Mesh>,
    last_generation: u64,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(empty_mesh());
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        // The flame front should glow rather than be lit; without this the
        // fire disappears in terrain shadow, which is exactly where it matters.
        emissive: LinearRgba::new(3.0, 1.0, 0.15, 1.0),
        unlit: false,
        perceptual_roughness: 1.0,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands.spawn((
        PbrBundle { mesh: mesh.clone(), material, ..default() },
        FireMesh,
    ));
    commands.insert_resource(FireView { mesh, last_generation: u64::MAX });
}

fn empty_mesh() -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    m.insert_indices(Indices::U32(Vec::new()));
    m
}

pub fn update(sim: Res<Sim>, mut view: ResMut<FireView>, mut meshes: ResMut<Assets<Mesh>>) {
    if sim.generation == view.last_generation {
        return;
    }
    view.last_generation = sim.generation;

    let scn = &sim.scenario;
    let w = scn.world;
    let half = w.cellsize * 0.5;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let now = sim.time_s() as i32;
    for (i, state) in sim.fire.state().iter().enumerate() {
        if *state == CellFire::Unburnt {
            continue;
        }
        let cell = Cell { row: i / w.fire_cols, col: i % w.fire_cols };
        let p = w.centre_of(cell);

        // Colour by age: fresh flame is bright, then it darkens to scar.
        let age = sim.fire.arrival_time(cell).map(|t| now - t).unwrap_or(0) as f32;
        let color = match state {
            CellFire::Burning => {
                let k = (age / 600.0).clamp(0.0, 1.0);
                [1.0, 0.75 - 0.55 * k, 0.12 * (1.0 - k), 1.0]
            }
            CellFire::Burnt => [0.10, 0.08, 0.07, 1.0],
            CellFire::Unburnt => continue,
        };

        // Lift slightly so the quad never z-fights the terrain it sits on.
        let lift = if *state == CellFire::Burning { 1.6 } else { 0.4 };
        let base = positions.len() as u32;
        for (dx, dy) in [(-half, -half), (half, -half), (half, half), (-half, half)] {
            let q = scenario::Pos { x: p.x + dx, y: p.y + dy };
            let h = scn.terrain.height_at(q) + lift;
            positions.push([q.x, h, -q.y]);
            normals.push([0.0, 1.0, 0.0]);
            colors.push(color);
            uvs.push([0.0, 0.0]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mut mesh = empty_mesh();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    meshes.insert(&view.mesh, mesh);
}
