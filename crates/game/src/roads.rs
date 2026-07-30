//! Roads, drawn as ribbons draped on the terrain.
//!
//! These are not decoration: the drivable/track split is what constrains where
//! engines can go and which escape routes exist, and it is the graph the
//! civilians evacuate along (`abm::network`), so the player has to be able to
//! read the network at a glance.
//!
//! Three things make a draped ribbon actually work, and all three cost a
//! rewrite to discover:
//!
//! **Winding.** The strip alternates right/left across the centreline, so
//! emitting the quad in the obvious index order winds it *clockwise seen from
//! above* — the faces point at the ground and back-face culling silently eats
//! the entire network. This is a total invisibility with no warning anywhere.
//!
//! **Resampling.** OSM ways carry vertices only where the road *bends*, so a
//! straight run over a hill can be one 200 m segment. Draping samples the
//! terrain at the vertices only, so that segment cuts a chord through the
//! ridge and vanishes into it. Every polyline is resampled to [`STEP_M`] first.
//!
//! **Mitred joints.** Offsetting each side by the segment normal pinches the
//! ribbon to nothing on a hairpin. The offset is divided by `cos(half-angle)`
//! so the two segments' edges actually meet, capped so a switchback does not
//! throw a spike across the hillside.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::{Pos, Scenario};

/// Half-widths in metres, by road role. Generous: at command altitude a real
/// 3 m lane is under a pixel, and the network has to stay legible while zoomed
/// out. Same map-symbol exaggeration as `people::FIGURE_SCALE`.
const DRIVABLE_HALF_W: f32 = 4.0;
const TRACK_HALF_W: f32 = 1.6;

/// How much wider the casing is than the surface it sits under. A dark edge is
/// what separates a road from bare limestone soil of a similar tone — without
/// it the network reads as a smudge from anywhere above a few hundred metres.
const CASING_M: f32 = 1.6;

/// Resampling interval along a polyline. Below the 5 m render posting there is
/// no more terrain detail to follow, so this is as fine as it needs to be.
const STEP_M: f32 = 5.0;

/// Lift above the ground, in metres. Enough to clear the depth precision at
/// this draw distance without the ribbon visibly floating when seen edge-on.
const SURFACE_LIFT_M: f32 = 0.55;
/// The casing sits *below* the surface so the two never z-fight with each
/// other, only ever against the terrain.
const CASING_LIFT_M: f32 = 0.35;

/// Mitre limit: past this the joint is cut off square rather than extended.
const MITRE_LIMIT: f32 = 2.5;

/// Chunk edge in metres. One mesh for all 1,793 drivable ways spans the whole
/// 10 km window, so it is never culled and never off the critical path; a
/// coarse grid gets that back for the cost of a few extra draw calls.
const CHUNK_M: f32 = 1280.0;

#[derive(Component)]
pub struct RoadChunk;

pub fn build(
    scn: &Scenario,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    // Asphalt, and the darker shoulder under it. Rough and unlit-ish: a road
    // that specularly flares in the low sun draws the eye away from the fire.
    let drivable_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.20, 0.22),
        perceptual_roughness: 0.95,
        ..default()
    });
    let drivable_casing = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.11, 0.12),
        perceptual_roughness: 1.0,
        ..default()
    });
    // Tracks are pale dirt: the contrast against dark asphalt is the whole
    // point of the split, because it is also the drive/walk distinction.
    let track_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.58, 0.52, 0.41),
        perceptual_roughness: 1.0,
        ..default()
    });
    let track_casing = materials.add(StandardMaterial {
        base_color: Color::srgb(0.38, 0.33, 0.26),
        perceptual_roughness: 1.0,
        ..default()
    });

    // Four layers, each chunked independently: casings under surfaces, tracks
    // under drivable roads, so a lane crossing a path reads the right way.
    let mut layers = [
        Grid::default(), // 0 track casing
        Grid::default(), // 1 track surface
        Grid::default(), // 2 drivable casing
        Grid::default(), // 3 drivable surface
    ];

    let mut drivable_n = 0;
    let mut track_n = 0;
    for road in &scn.vectors.roads {
        let (base, half) = if road.drivable {
            drivable_n += 1;
            (2, DRIVABLE_HALF_W)
        } else if road.track {
            track_n += 1;
            (0, TRACK_HALF_W)
        } else {
            continue;
        };
        let line = resample(&road.line, STEP_M);
        if line.len() < 2 {
            continue;
        }
        layers[base].add_line(scn, &line, half + CASING_M, CASING_LIFT_M);
        layers[base + 1].add_line(scn, &line, half, SURFACE_LIFT_M);
    }

    let mats = [track_casing, track_mat, drivable_casing, drivable_mat];
    let mut chunk_count = 0;
    let mut tri_count = 0;
    for (grid, mat) in layers.into_iter().zip(mats) {
        for builder in grid.chunks.into_values() {
            tri_count += builder.indices.len() / 3;
            chunk_count += 1;
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(builder.finish()),
                    material: mat.clone(),
                    ..default()
                },
                RoadChunk,
            ));
        }
    }

    info!(
        "roads: {drivable_n} drivable, {track_n} tracks -> {chunk_count} chunks, \
         {} k triangles",
        tri_count / 1000
    );
}

/// Insert vertices along `line` so no segment is longer than `step`.
///
/// Draping only samples the terrain where there is a vertex, so an
/// unsubdivided long segment is a chord through whatever it crosses.
fn resample(line: &[[f32; 2]], step: f32) -> Vec<[f32; 2]> {
    if line.len() < 2 {
        return line.to_vec();
    }
    let mut out = Vec::with_capacity(line.len() * 2);
    out.push(line[0]);
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        // Coincident vertices happen in OSM data and would divide by zero.
        if len < 1e-3 {
            continue;
        }
        let n = (len / step).ceil() as usize;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            out.push([a[0] + dx * t, a[1] + dy * t]);
        }
    }
    out
}

/// Ribbon builders bucketed by world-space chunk, so each mesh has a tight
/// enough AABB to cull.
#[derive(Default)]
struct Grid {
    chunks: std::collections::HashMap<(i32, i32), RibbonBuilder>,
}

impl Grid {
    /// Emit a quad strip along the polyline, each vertex draped on the terrain.
    ///
    /// Split across chunks by segment: a segment is emitted whole into the
    /// chunk its midpoint falls in, so the strip is cut rather than stretched
    /// and no seam opens up at a boundary.
    fn add_line(&mut self, scn: &Scenario, line: &[[f32; 2]], half: f32, lift: f32) {
        let n = line.len();
        if n < 2 {
            return;
        }

        // Offset both edges once per vertex; segments then just index into it.
        let mut edges: Vec<(Pos, Pos)> = Vec::with_capacity(n);
        for i in 0..n {
            let p = Pos {
                x: line[i][0],
                y: line[i][1],
            };
            let into = dir(line[i.saturating_sub(1)], line[i]);
            let out = dir(line[i], line[(i + 1).min(n - 1)]);
            // Average the two directions, then mitre: dividing by the cosine of
            // the half-angle pushes the joint out to where the offset edges of
            // the two segments actually intersect.
            let bx = into.0 + out.0;
            let by = into.1 + out.1;
            let blen = (bx * bx + by * by).sqrt();
            let (tx, ty) = if blen < 1e-4 {
                // A perfect reversal (a spur doubling back): no bisector
                // exists, so fall back to the incoming direction.
                into
            } else {
                (bx / blen, by / blen)
            };
            // cos of the half-angle between the tangent and either segment
            let cos_half = (tx * into.0 + ty * into.1).abs().max(1.0 / MITRE_LIMIT);
            let w = half / cos_half;
            // left-hand normal of the tangent, in the ground plane
            let (nx, ny) = (-ty * w, tx * w);
            edges.push((
                Pos {
                    x: p.x - nx,
                    y: p.y - ny,
                },
                Pos {
                    x: p.x + nx,
                    y: p.y + ny,
                },
            ));
        }

        for i in 0..n - 1 {
            let key = (
                ((line[i][0] + line[i + 1][0]) * 0.5 / CHUNK_M).floor() as i32,
                ((line[i][1] + line[i + 1][1]) * 0.5 / CHUNK_M).floor() as i32,
            );
            let builder = self.chunks.entry(key).or_default();
            builder.quad(scn, edges[i], edges[i + 1], lift);
        }
    }
}

/// Unit direction from `a` to `b`, or east for a degenerate pair.
fn dir(a: [f32; 2], b: [f32; 2]) -> (f32, f32) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-4 {
        (1.0, 0.0)
    } else {
        (dx / len, dy / len)
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
    /// One quad between two pairs of already-offset edge points.
    ///
    /// Each quad carries its own four vertices rather than sharing them with
    /// its neighbour. That is twice the vertices, but it is what lets a strip
    /// be cut across a chunk boundary at all, and roads are a rounding error
    /// next to the 15 M triangles of vegetation.
    fn quad(&mut self, scn: &Scenario, a: (Pos, Pos), b: (Pos, Pos), lift: f32) {
        let start = self.positions.len() as u32;
        for (q, u) in [(a.0, 0.0), (a.1, 1.0), (b.0, 0.0), (b.1, 1.0)] {
            let h = scn.terrain.height_at(q) + lift;
            self.positions.push([q.x, h, -q.y]);
            // Flat up-normal. The ribbon is thin enough that following the
            // terrain normal would only make it pick up shading noise the road
            // surface itself does not have.
            self.normals.push([0.0, 1.0, 0.0]);
            self.uvs.push([u, 0.0]);
        }
        // Counter-clockwise seen from +Y. Getting this backwards points every
        // face at the ground and culls the whole network invisibly.
        self.indices.extend_from_slice(&[
            start,
            start + 2,
            start + 1,
            start + 1,
            start + 2,
            start + 3,
        ]);
    }

    fn finish(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}
