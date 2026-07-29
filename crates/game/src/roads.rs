//! Roads, drawn as ribbons draped on the terrain.
//!
//! These are not decoration: the drivable/track split is what constrains where
//! engines can go and which escape routes exist, so the player has to be able
//! to read the network at a glance.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::{Pos, Scenario};

/// Half-widths in metres, by road role.
const DRIVABLE_HALF_W: f32 = 3.5;
const TRACK_HALF_W: f32 = 1.2;

pub fn build(
    scn: &Scenario,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mut drivable = RibbonBuilder::default();
    let mut tracks = RibbonBuilder::default();

    for road in &scn.vectors.roads {
        let (builder, half) = if road.drivable {
            (&mut drivable, DRIVABLE_HALF_W)
        } else if road.track {
            (&mut tracks, TRACK_HALF_W)
        } else {
            continue;
        };
        builder.add_line(scn, &road.line, half);
    }

    let drivable_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.28, 0.30),
        perceptual_roughness: 0.95,
        ..default()
    });
    let track_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.50, 0.42),
        perceptual_roughness: 1.0,
        ..default()
    });

    for (builder, mat) in [(drivable, drivable_mat), (tracks, track_mat)] {
        if builder.positions.is_empty() {
            continue;
        }
        commands.spawn(PbrBundle {
            mesh: meshes.add(builder.finish()),
            material: mat,
            ..default()
        });
    }
}

#[derive(Default)]
struct RibbonBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl RibbonBuilder {
    /// Emit a quad strip along the polyline, each vertex sampled onto the
    /// terrain so the ribbon follows the ground rather than cutting through it.
    fn add_line(&mut self, scn: &Scenario, line: &[[f32; 2]], half: f32) {
        if line.len() < 2 {
            return;
        }
        let start = self.positions.len() as u32;
        let n = line.len();

        for (i, p) in line.iter().enumerate() {
            let prev = line[i.saturating_sub(1)];
            let next = line[(i + 1).min(n - 1)];
            let dx = next[0] - prev[0];
            let dy = next[1] - prev[1];
            let len = (dx * dx + dy * dy).sqrt().max(1e-3);
            // left-hand normal in the ground plane
            let nx = -dy / len * half;
            let ny = dx / len * half;

            for side in [-1.0f32, 1.0] {
                let q = Pos { x: p[0] + nx * side, y: p[1] + ny * side };
                // lift clear of the terrain to avoid z-fighting
                let h = scn.terrain.height_at(q) + 0.6;
                self.positions.push([q.x, h, -q.y]);
                self.normals.push([0.0, 1.0, 0.0]);
                self.uvs.push([(side + 1.0) * 0.5, i as f32]);
            }
        }

        for i in 0..n as u32 - 1 {
            let a = start + i * 2;
            self.indices
                .extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
        }
    }

    fn finish(self) -> Mesh {
        let mut mesh =
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}
