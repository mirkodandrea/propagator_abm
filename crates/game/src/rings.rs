//! Draped ring meshes: the one symbol the map overlays share.
//!
//! The rings are annuli draped on the 5 m render field rather than flat discs:
//! a disc at this scale is either buried in a hillside or floating off it, and
//! a filled shape hides what it marks. Wound counter-clockwise from above for
//! the reason the road ribbons are (finding 11).

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::{Pos, Scenario};

/// Segments around a ring. Fixed rather than radius-dependent: even the
/// smallest ring is 60 m across on screen, where faceting would show.
const RING_SEGMENTS: usize = 96;

/// Lift above the ground.
///
/// Not a z-fighting margin: a ring on the ground is *under the canopy*, and the
/// plants are 5-15 m tall, so at 2 m it is drawn perfectly and seen never
/// (finding 13). Same layer as the household beacons and refuge markers.
pub(crate) const RING_LIFT_M: f32 = 20.0;

/// Ring thickness in metres, scaled with the radius.
fn ring_width(radius_m: f32) -> f32 {
    (radius_m * 0.06).clamp(4.0, 20.0)
}

/// An annulus in world space, draped on the render terrain.
pub(crate) fn ring_mesh(scn: &Scenario, centre: Pos, radius_m: f32) -> Mesh {
    annulus(scn, centre, radius_m, ring_width(radius_m))
}

/// The dark band drawn just under a ring, wider than it, so the ring stands
/// out on the green of the woods (playtest 3).
pub(crate) fn halo_mesh(scn: &Scenario, centre: Pos, radius_m: f32) -> Mesh {
    annulus(scn, centre, radius_m, ring_width(radius_m) * HALO)
}

/// How much wider a halo is than what it outlines.
pub(crate) const HALO: f32 = 2.2;

fn annulus(scn: &Scenario, centre: Pos, radius_m: f32, width_m: f32) -> Mesh {
    let w = width_m * 0.5;
    let (inner, outer) = ((radius_m - w).max(1.0), radius_m + w);

    let mut positions = Vec::with_capacity((RING_SEGMENTS + 1) * 2);
    let mut normals = Vec::with_capacity((RING_SEGMENTS + 1) * 2);
    let mut uvs = Vec::with_capacity((RING_SEGMENTS + 1) * 2);
    let mut indices = Vec::with_capacity(RING_SEGMENTS * 6);

    for i in 0..=RING_SEGMENTS {
        let a = i as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        for r in [inner, outer] {
            let p = Pos { x: centre.x + c * r, y: centre.y + s * r };
            let h = scn.terrain.height_at(p) + RING_LIFT_M;
            positions.push([p.x, h, -p.y]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([if r == inner { 0.0 } else { 1.0 }, i as f32]);
        }
    }
    for i in 0..RING_SEGMENTS as u32 {
        let a = i * 2;
        indices.extend_from_slice(&[a, a + 2, a + 1, a + 1, a + 2, a + 3]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// A route drawn on the map: a ribbon `width_m` wide along `pts`, draped like
/// the rings. With `dash_m`, drawn and skipped in turns of that length, so a
/// proposed route reads apart from one being driven.
pub(crate) fn path_mesh(scn: &Scenario, pts: &[Pos], width_m: f32, dash_m: Option<f32>) -> Mesh {
    const STEP_M: f32 = 8.0;
    let mut positions: Vec<[f32; 3]> = vec![];
    let mut indices: Vec<u32> = vec![];
    let mut along = 0.0f32;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.5 {
            continue;
        }
        let (nx, ny) = (-dy / len * width_m * 0.5, dx / len * width_m * 0.5);
        let steps = (len / STEP_M).ceil() as usize;
        for i in 0..steps {
            let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
            let on = dash_m.is_none_or(|d| ((along + len * t0) / d) as i64 % 2 == 0);
            if !on {
                continue;
            }
            let s = positions.len() as u32;
            for t in [t0, t1] {
                let c = Pos { x: a.x + dx * t, y: a.y + dy * t };
                for side in [-1.0f32, 1.0] {
                    let p = Pos { x: c.x + nx * side, y: c.y + ny * side };
                    positions.push([p.x, scn.terrain.height_at(c) + RING_LIFT_M, -p.y]);
                }
            }
            indices.extend_from_slice(&[s, s + 2, s + 1, s + 1, s + 2, s + 3]);
        }
        along += len;
    }
    let n = positions.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n]);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
