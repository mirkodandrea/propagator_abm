//! The diorama base: strata walls under the terrain's four edges, and the
//! table the block stands on. The world outside the window is the table, softly
//! out of focus, not a far-away landscape.
//!
//! Both meshes use `cull_mode: None`: the skirt's winding depends on which way
//! each edge walks (finding 11), and a wall nobody can see is an invisible bug.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::Scenario;

use crate::retro::{self, RetroMaterial};

/// How far below the lowest ground the block extends, metres.
const BLOCK_DEPTH_M: f32 = 140.0;

/// Strata, top to bottom: (thickness m, sRGB colour).
const STRATA: [(f32, [f32; 3]); 5] = [
    (10.0, [0.36, 0.27, 0.18]),
    (26.0, [0.62, 0.50, 0.36]),
    (20.0, [0.50, 0.40, 0.30]),
    (34.0, [0.70, 0.60, 0.46]),
    (400.0, [0.44, 0.36, 0.30]),
];

/// The table's colour: pale, warm, and lighter than the sky behind it.
pub const TABLE: [f32; 3] = [0.80, 0.76, 0.70];

fn lin(c: [f32; 3]) -> [f32; 4] {
    let c = Color::srgb(c[0], c[1], c[2]).to_linear();
    [c.red, c.green, c.blue, 1.0]
}

/// Height of the table surface for a scenario.
pub fn base_y(scn: &Scenario) -> f32 {
    let lo = scn.terrain.elev.iter().cloned().fold(f32::INFINITY, f32::min);
    lo - BLOCK_DEPTH_M
}

pub fn build(
    scn: &Scenario,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<RetroMaterial>,
) {
    let t = &scn.terrain;
    let base = base_y(scn);
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nor: Vec<[f32; 3]> = Vec::new();
    let mut col: Vec<[f32; 4]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();

    // Each edge is a list of (x, z, elevation) along it, plus its outward normal.
    let (w, h) = (t.posting * (t.cols - 1) as f32, t.posting * (t.rows - 1) as f32);
    let top = |r: usize, c: usize| t.elev[r * t.cols + c];
    let mut edges: Vec<(Vec<[f32; 3]>, [f32; 3])> = Vec::new();
    // Row 0 is the north edge (world y = height -> bevy z = -height).
    edges.push(((0..t.cols).map(|c| [c as f32 * t.posting, top(0, c), -t.height_m]).collect(), [0.0, 0.0, -1.0]));
    edges.push(((0..t.cols).map(|c| [c as f32 * t.posting, top(t.rows - 1, c), -(t.height_m - h)]).collect(), [0.0, 0.0, 1.0]));
    edges.push(((0..t.rows).map(|r| [0.0, top(r, 0), -(t.height_m - r as f32 * t.posting)]).collect(), [-1.0, 0.0, 0.0]));
    edges.push(((0..t.rows).map(|r| [w, top(r, t.cols - 1), -(t.height_m - r as f32 * t.posting)]).collect(), [1.0, 0.0, 0.0]));

    for (pts, n) in &edges {
        // One ring of vertices per strata boundary, so colours stay in bands.
        for k in 0..pts.len() - 1 {
            let (a, b) = (pts[k], pts[k + 1]);
            let mut y_hi = [a[1], b[1]];
            let mut depth = 0.0;
            for (thick, c) in STRATA {
                depth += thick;
                let lo = [(a[1] - depth).max(base), (b[1] - depth).max(base)];
                let i = pos.len() as u32;
                pos.extend([[a[0], y_hi[0], a[2]], [b[0], y_hi[1], b[2]], [a[0], lo[0], a[2]], [b[0], lo[1], b[2]]]);
                nor.extend([*n; 4]);
                col.extend([lin(c); 4]);
                idx.extend([i, i + 2, i + 1, i + 1, i + 2, i + 3]);
                y_hi = lo;
                if lo[0] <= base && lo[1] <= base {
                    break;
                }
            }
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    let mat = materials.add(retro::material_with_style(
        StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.55,
            reflectance: 0.12,
            cull_mode: None,
            double_sided: true,
            ..default()
        },
        false,
        retro::RetroStyle::BACKGROUND,
    ));
    commands.spawn(MaterialMeshBundle::<RetroMaterial> { mesh: meshes.add(mesh), material: mat, ..default() });

    // The table: one big quad.
    let (cx, cz, r) = (w * 0.5, -(t.height_m - h * 0.5), 30_000.0);
    let tp = vec![[cx - r, base, cz - r], [cx + r, base, cz - r], [cx - r, base, cz + r], [cx + r, base, cz + r]];
    let mut table = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    table.insert_attribute(Mesh::ATTRIBUTE_POSITION, tp);
    table.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 4]);
    table.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![lin(TABLE); 4]);
    table.insert_indices(Indices::U32(vec![0, 2, 1, 1, 2, 3]));
    let tmat = materials.add(retro::material_with_style(
        StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            reflectance: 0.04,
            cull_mode: None,
            double_sided: true,
            ..default()
        },
        false,
        retro::RetroStyle::BACKGROUND,
    ));
    commands.spawn(MaterialMeshBundle::<RetroMaterial> { mesh: meshes.add(table), material: tmat, ..default() });
}
