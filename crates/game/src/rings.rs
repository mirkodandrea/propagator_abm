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
    let w = ring_width(radius_m) * 0.5;
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
