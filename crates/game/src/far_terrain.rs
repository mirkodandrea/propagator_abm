//! The world beyond the scenario window, at a resolution nobody is meant to
//! scrutinise.
//!
//! The real terrain, roads, buildings and vegetation stop at the 10.24 km
//! window the scenario data actually covers — outside that there is no DEM,
//! no fuel raster, no OSM. Cutting to sky at that edge reads as a diorama in
//! a box. This module paints a plausible, cheap continuation instead: a
//! coarse heightfield that matches the real terrain's edge exactly (so there
//! is no seam) and rolls into procedural hills further out, with a scatter of
//! low-poly "cardboard" trees and boxy houses standing in for a landscape
//! that was never simulated. None of it is real, none of it is walkable, and
//! none of it feeds any model — it exists only so the horizon has something
//! in it.
//!
//! [`crate::camera`] keeps the orbit focus inside the real window, so this
//! skirt is always background, never somewhere the player is asked to look
//! closely.

use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::{Pos, Scenario};

use crate::field::noise;
use crate::sim::Sim;

/// How far beyond the real window edge the skirt reaches, in every direction.
///
/// Large enough that the mesh's own true edge sits beyond where
/// `sky::update_sky`'s `FogFalloff` (11 km visibility, generous on purpose)
/// has already extinguished it — the same fog that fades the real coastline
/// to a clean haze in the distance, with no help from this module. An
/// earlier version tried to hand-roll that fade in vertex colour plus a
/// height "sink" to hide the mesh's edge. Both operate at this mesh's coarse
/// 150 m vertex spacing, and under a shallow, grazing camera angle that
/// spacing compresses to a handful of screen pixels — a smooth analytic
/// blend sampled that sparsely aliases into a hard, jagged line, which is
/// worse than the plain edge it was meant to hide. Trusting the real,
/// per-pixel fog and simply extending the mesh past its reach is both
/// simpler and looks right.
pub(crate) const SKIRT_EXTENT_M: f32 = 25_000.0;

/// Lattice spacing for the skirt terrain: twenty times the real terrain's 5 m
/// posting, since nobody stands on this ground and its only job is to fill
/// the horizon.
const SKIRT_STRIDE_M: f32 = 150.0;

/// Samples per skirt chunk edge, so the far ground still culls per-chunk
/// instead of drawing the whole ring as one mesh.
const SKIRT_CHUNK: usize = 48;

#[derive(Component)]
struct FarTerrain;

pub struct FarTerrainPlugin;

impl Plugin for FarTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(crate::AppState::Playing), (spawn_skirt, spawn_props));
    }
}

fn inside_real(scn: &Scenario, p: Pos) -> bool {
    p.x >= 0.0 && p.x <= scn.terrain.width_m && p.y >= 0.0 && p.y <= scn.terrain.height_m
}

/// Straight-line distance outside the real window's bounding box; zero
/// anywhere inside it.
fn dist_outside(scn: &Scenario, p: Pos) -> f32 {
    let ox = (-p.x).max(p.x - scn.terrain.width_m).max(0.0);
    let oy = (-p.y).max(p.y - scn.terrain.height_m).max(0.0);
    (ox * ox + oy * oy).sqrt()
}

/// `Terrain::height_at` already clamps its query to the real grid, so calling
/// it with a point beyond the window returns that edge's own height — the
/// value this blends *from*. The blend target is two octaves of noise standing
/// in for distant relief; it dominates only well past the edge, which is what
/// keeps the join invisible.
///
/// `land_weight` is the fix for hills growing out of open water: a boundary
/// point right at the waterline is the open sea continuing outward, not a
/// beach, so it must stay flat rather than sprout a rolling hill from noise
/// that has no idea it is standing over the Ligurian Sea. It ramps from 0 at
/// the shoreline to 1 a few metres inland, so only ground that was already
/// dry gets any relief.
pub(crate) fn far_height(scn: &Scenario, p: Pos) -> f32 {
    let dist = dist_outside(scn, p);
    if dist <= 0.0 {
        return scn.terrain.height_at(p);
    }
    // Clamping means the edge profile is extruded *exactly* straight outward,
    // and the one place that shows is the waterline: the coast past the
    // window becomes a set of dead-straight lines running due north (or east,
    // or south), which the sea's own lattice then cuts into a staircase of
    // rectangular headlands — a pixel-art coastline, and the more obviously
    // fake the further out it goes. Warping the query point by a slow noise
    // before sampling costs two more noise lookups and makes the same
    // extrusion wander like a coastline instead. The warp is zero at the
    // window edge, so the real terrain still joins it exactly.
    let warp = 300.0 * (dist / 1500.0).clamp(0.0, 1.0);
    let q = Pos {
        x: p.x + (noise(p.x / 900.0, p.y / 900.0, 0x9A33) - 0.5) * 2.0 * warp,
        y: p.y + (noise(p.x / 900.0, p.y / 900.0, 0x9A44) - 0.5) * 2.0 * warp,
    };
    let edge = scn.terrain.height_at(q);
    let land_weight = ((edge - 0.5) / 4.0).clamp(0.0, 1.0);
    let hills = (noise(p.x / 1600.0, p.y / 1600.0, 0x9A11) - 0.5) * 140.0
        + (noise(p.x / 480.0, p.y / 480.0, 0x9A22) - 0.5) * 30.0;
    let t = (dist / 1500.0).clamp(0.0, 1.0) * land_weight;
    (edge + hills * t).max(0.0)
}

/// Central-difference normal on [`far_height`], mirroring
/// `Terrain::normal_at`'s convention so lighting matches at the seam.
fn far_normal(scn: &Scenario, p: Pos) -> [f32; 3] {
    let d = 60.0;
    let hl = far_height(scn, Pos { x: p.x - d, ..p });
    let hr = far_height(scn, Pos { x: p.x + d, ..p });
    let hd = far_height(scn, Pos { y: p.y - d, ..p });
    let hu = far_height(scn, Pos { y: p.y + d, ..p });
    let nx = hl - hr;
    let nz = hd - hu;
    let ny = 2.0 * d;
    let len = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-6);
    [nx / len, ny / len, -nz / len]
}

/// Muted ground colour, unhazed: the real, per-pixel `FogSettings` (see
/// `sky::update_sky`) already fades the real coastline to a clean haze at
/// this range with no help, and this mesh is lit and fogged the same way.
fn far_ground_color(elev: f32, p: Pos) -> [f32; 3] {
    if elev <= 0.5 {
        return [0.05, 0.11, 0.22]; // the open sea continuing past the real window
    }
    let broad = noise(p.x / 320.0, p.y / 320.0, 0x1A2B);
    let mottle = 0.85 + 0.3 * broad;
    [0.22 * mottle, 0.27 * mottle, 0.15 * mottle]
}

fn spawn_skirt(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let scn = &sim.scenario;
    // VR-training dev scenarios are a void beyond the grid floor, not a
    // distant landscape.
    if scn.vr_palette().is_some() {
        return;
    }
    let w = scn.terrain.width_m;
    let h = scn.terrain.height_m;

    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        metallic: 0.0,
        reflectance: 0.08,
        ..default()
    });

    let x0 = -SKIRT_EXTENT_M;
    let y0 = -SKIRT_EXTENT_M;
    let cols = ((w + 2.0 * SKIRT_EXTENT_M) / SKIRT_STRIDE_M).ceil() as usize + 1;
    let rows = ((h + 2.0 * SKIRT_EXTENT_M) / SKIRT_STRIDE_M).ceil() as usize + 1;

    let chunks_x = (cols - 1).div_ceil(SKIRT_CHUNK);
    let chunks_y = (rows - 1).div_ceil(SKIRT_CHUNK);
    let (mut chunk_count, mut tri_count) = (0usize, 0usize);

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let c0 = cx * SKIRT_CHUNK;
            let r0 = cy * SKIRT_CHUNK;
            let cn = (SKIRT_CHUNK + 1).min(cols - c0);
            let rn = (SKIRT_CHUNK + 1).min(rows - r0);
            if cn < 2 || rn < 2 {
                continue;
            }

            let mut positions = Vec::with_capacity(cn * rn);
            let mut normals = Vec::with_capacity(cn * rn);
            let mut colors = Vec::with_capacity(cn * rn);
            let mut inside = Vec::with_capacity(cn * rn);

            for r in 0..rn {
                for c in 0..cn {
                    let gx = x0 + (c0 + c) as f32 * SKIRT_STRIDE_M;
                    let gy = y0 + (r0 + r) as f32 * SKIRT_STRIDE_M;
                    let p = Pos { x: gx, y: gy };
                    let elev = far_height(scn, p);

                    positions.push([gx, elev, -gy]);
                    normals.push(far_normal(scn, p));
                    let col = far_ground_color(elev, p);
                    let col = Color::srgb(col[0], col[1], col[2]).to_linear();
                    colors.push([col.red, col.green, col.blue, 1.0]);
                    inside.push(gx >= 0.0 && gx <= w && gy >= 0.0 && gy <= h);
                }
            }

            let mut indices = Vec::new();
            for r in 0..rn - 1 {
                for c in 0..cn - 1 {
                    let i = r * cn + c;
                    let right = i + 1;
                    let down = i + cn;
                    let diag = down + 1;
                    // The real terrain already draws anything fully inside
                    // the scenario window; the skirt only fills in around it.
                    if [i, right, down, diag].iter().all(|&k| inside[k]) {
                        continue;
                    }
                    let (i, right, down, diag) = (i as u32, right as u32, down as u32, diag as u32);
                    // `crate::terrain_mesh` winds the same lattice `i, down,
                    // right` — but its rows run *north to south* (row 0 is
                    // the north edge) while this one runs south to north, so
                    // copying its index order hands every triangle the
                    // opposite orientation and back-face culling eats the
                    // whole skirt. Finding 11 in CLAUDE.md, and it fails the
                    // same silent way: the meshes build, the counts log, and
                    // the world outside the window is simply not there.
                    indices.extend_from_slice(&[i, right, down, right, diag, down]);
                    tri_count += 2;
                }
            }
            if indices.is_empty() {
                continue;
            }

            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            mesh.insert_indices(Indices::U32(indices));

            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(mesh),
                    material: material.clone(),
                    ..default()
                },
                FarTerrain,
                // Well outside the directional light's cascade coverage
                // (`maximum_distance: 4000.0` in `main.rs`) anyway, so this
                // buys back a shadow pass over a huge area for nothing lost.
                NotShadowCaster,
                NotShadowReceiver,
            ));
            chunk_count += 1;
        }
    }

    info!(
        "far terrain: {chunk_count} skirt chunks, {} k triangles @ {SKIRT_STRIDE_M} m stride out to {SKIRT_EXTENT_M} m",
        tri_count / 1000
    );
}

// --- distant props: cardboard trees and boxy houses -------------------------

/// Tile edge for merging distant props into draw calls, well above the
/// terrain skirt's own chunking since there are far fewer of them.
const PROP_CHUNK_M: f32 = 1500.0;

/// Sub-grid pitch for tree candidates.
const TREE_STEP_M: f32 = 70.0;

/// Sub-grid pitch for village candidates.
const VILLAGE_STEP_M: f32 = 900.0;

/// How far past the real window edge props are worth placing at all. Fog
/// (and, at the very outer reaches of the now much larger [`SKIRT_EXTENT_M`],
/// plain distance) already erases anything past a few kilometres, so paying
/// to scatter thousands of trees and houses all the way out to the mesh's own
/// edge would be triangles nobody ever resolves.
const PROP_REACH_M: f32 = 4_000.0;

fn spawn_props(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let scn = &sim.scenario;
    if scn.vr_palette().is_some() {
        return;
    }
    let w = scn.terrain.width_m;
    let h = scn.terrain.height_m;

    let tree_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        metallic: 0.0,
        reflectance: 0.03,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let house_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.8,
        metallic: 0.0,
        reflectance: 0.3,
        ..default()
    });

    let x0 = -SKIRT_EXTENT_M;
    let y0 = -SKIRT_EXTENT_M;
    let x1 = w + SKIRT_EXTENT_M;
    let y1 = h + SKIRT_EXTENT_M;
    let chunks_x = ((x1 - x0) / PROP_CHUNK_M).ceil() as i32;
    let chunks_y = ((y1 - y0) / PROP_CHUNK_M).ceil() as i32;

    let (mut tree_count, mut house_count) = (0usize, 0usize);

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let bx0 = x0 + cx as f32 * PROP_CHUNK_M;
            let by0 = y0 + cy as f32 * PROP_CHUNK_M;
            let mut trees = Builder::default();
            let mut houses = Builder::default();

            let tn = (PROP_CHUNK_M / TREE_STEP_M).round() as i32;
            for j in 0..tn {
                for i in 0..tn {
                    let px = bx0 + i as f32 * TREE_STEP_M + TREE_STEP_M * 0.5;
                    let py = by0 + j as f32 * TREE_STEP_M + TREE_STEP_M * 0.5;
                    let p = Pos { x: px, y: py };
                    if inside_real(scn, p) {
                        continue; // the real vegetation already covers this
                    }
                    let dist = dist_outside(scn, p);
                    if dist > PROP_REACH_M {
                        continue;
                    }
                    let mut rng = Rng::seeded(hash2(cx * 100_000 + i, cy * 100_000 + j, 0xC0FF));
                    let density = tree_density(p, dist);
                    if rng.unit() >= density {
                        continue;
                    }
                    let elev = far_height(scn, p);
                    if elev <= 0.6 {
                        continue; // no trees standing in the sea
                    }
                    emit_cardboard_tree(&mut trees, px, elev, py, &mut rng);
                    tree_count += 1;
                }
            }

            let vn = (PROP_CHUNK_M / VILLAGE_STEP_M).round().max(1.0) as i32;
            for j in 0..vn {
                for i in 0..vn {
                    let cx_p = bx0 + i as f32 * VILLAGE_STEP_M + VILLAGE_STEP_M * 0.5;
                    let cy_p = by0 + j as f32 * VILLAGE_STEP_M + VILLAGE_STEP_M * 0.5;
                    let centre = Pos { x: cx_p, y: cy_p };
                    if inside_real(scn, centre) {
                        continue; // the real buildings already cover this
                    }
                    if dist_outside(scn, centre) > PROP_REACH_M * 0.8 {
                        continue; // fake villages only in the near skirt
                    }
                    let n = noise(cx_p / 2200.0, cy_p / 2200.0, 0x900D);
                    if n < 0.62 {
                        continue;
                    }
                    let mut rng = Rng::seeded(hash2(cx * 10_000 + i, cy * 10_000 + j, 0xB17A));
                    let houses_n = 3 + (rng.unit() * 5.0) as u32;
                    for _ in 0..houses_n {
                        let hx = cx_p + (rng.unit() - 0.5) * 260.0;
                        let hy = cy_p + (rng.unit() - 0.5) * 260.0;
                        let hp = Pos { x: hx, y: hy };
                        if inside_real(scn, hp) {
                            continue;
                        }
                        let elev = far_height(scn, hp);
                        if elev <= 0.6 {
                            continue;
                        }
                        emit_low_res_house(&mut houses, hx, elev, hy, &mut rng);
                        house_count += 1;
                    }
                }
            }

            if !trees.positions.is_empty() {
                commands.spawn(PbrBundle {
                    mesh: meshes.add(trees.finish()),
                    material: tree_material.clone(),
                    ..default()
                });
            }
            if !houses.positions.is_empty() {
                commands.spawn(PbrBundle {
                    mesh: meshes.add(houses.finish()),
                    material: house_material.clone(),
                    ..default()
                });
            }
        }
    }

    info!("far terrain: {tree_count} distant trees, {house_count} distant houses");
}

/// Probability of a tree candidate becoming a tree: patchy like the real
/// vegetation, fading out toward the far edge of the skirt so the forest
/// thins into open haze rather than stopping on a line.
fn tree_density(p: Pos, dist: f32) -> f32 {
    let n =
        noise(p.x / 220.0, p.y / 220.0, 0x7EE0) * 0.7 + noise(p.x / 70.0, p.y / 70.0, 0x7EE1) * 0.3;
    let fade = (1.0 - dist / SKIRT_EXTENT_M).clamp(0.0, 1.0);
    (0.30 * n * fade).clamp(0.0, 1.0)
}

fn hash2(a: i32, b: i32, salt: u64) -> u64 {
    (a as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (b as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ salt
}

/// Two crossed quads, solid-coloured: the cheapest possible stand-in for a
/// tree, visible from any horizontal angle because the material is
/// double-sided. Nobody is meant to get close enough to see that it is flat.
fn emit_cardboard_tree(b: &mut Builder, x: f32, elev: f32, y: f32, rng: &mut Rng) {
    let height = 6.0 + rng.unit() * 7.0;
    let width = height * 0.55;
    let yaw = rng.unit() * std::f32::consts::TAU;
    let shade = 0.85 + rng.unit() * 0.3;
    let green = [0.09 * shade, 0.16 * shade, 0.08 * shade];
    let base = Vec3::new(x, elev, -y);

    for k in 0..2 {
        let a = yaw + k as f32 * std::f32::consts::FRAC_PI_2;
        let (s, c) = a.sin_cos();
        let right = Vec3::new(c, 0.0, s) * width * 0.5;
        let p0 = base - right;
        let p1 = base + right;
        let p2 = p1 + Vec3::Y * height;
        let p3 = p0 + Vec3::Y * height;
        b.quad(
            p0.to_array(),
            p1.to_array(),
            p2.to_array(),
            p3.to_array(),
            green,
        );
    }
}

const HOUSE_WALLS: [[f32; 3]; 4] = [
    [0.82, 0.72, 0.56],
    [0.78, 0.62, 0.50],
    [0.85, 0.82, 0.72],
    [0.72, 0.68, 0.62],
];
const HOUSE_ROOFS: [[f32; 3]; 2] = [[0.58, 0.30, 0.22], [0.50, 0.50, 0.52]];

/// A flat-roofed box, four walls and a cap: not the traced-footprint
/// extrusion [`crate::buildings`] does, just enough silhouette to read as a
/// house from a kilometre away.
fn emit_low_res_house(b: &mut Builder, x: f32, elev: f32, y: f32, rng: &mut Rng) {
    let size = 8.0 + rng.unit() * 6.0;
    let height = 5.0 + rng.unit() * 3.0;
    let yaw = rng.unit() * std::f32::consts::TAU;
    let wall = HOUSE_WALLS[(rng.unit() * HOUSE_WALLS.len() as f32) as usize];
    let roof = HOUSE_ROOFS[(rng.unit() * HOUSE_ROOFS.len() as f32) as usize];

    let hs = size * 0.5;
    let (s, c) = yaw.sin_cos();
    let right = Vec3::new(c, 0.0, s) * hs;
    let fwd = Vec3::new(-s, 0.0, c) * hs;
    let base = Vec3::new(x, elev, -y);
    let corners = [
        base - right - fwd,
        base + right - fwd,
        base + right + fwd,
        base - right + fwd,
    ];
    let top: Vec<Vec3> = corners.iter().map(|v| *v + Vec3::Y * height).collect();

    for i in 0..4 {
        let j = (i + 1) % 4;
        b.quad(
            corners[i].to_array(),
            corners[j].to_array(),
            top[j].to_array(),
            top[i].to_array(),
            wall,
        );
    }
    b.quad(
        top[0].to_array(),
        top[1].to_array(),
        top[2].to_array(),
        top[3].to_array(),
        roof,
    );
}

#[derive(Default)]
struct Builder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Builder {
    /// Flat-shaded quad, wound a-b-c-d, vertices not shared between faces.
    ///
    /// Emitted in reverse (a-c-b, a-d-c): the callers lay their corners out
    /// clockwise seen from outside the solid, which is the back face. The
    /// trees never noticed — their material is `cull_mode: None` — so the
    /// only thing this ever showed as was distant houses with no walls and no
    /// roof, standing on ground that was itself inside-out (see `spawn_skirt`).
    fn quad(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], color: [f32; 3]) {
        let n = face_normal(a, c, b);
        let start = self.positions.len() as u32;
        for v in [a, b, c, d] {
            self.positions.push(v);
            self.normals.push(n);
            self.colors.push([color[0], color[1], color[2], 1.0]);
        }
        self.indices
            .extend_from_slice(&[start, start + 2, start + 1, start, start + 3, start + 2]);
    }

    fn finish(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
    [n[0] / len, n[1] / len, n[2] / len]
}

/// Small deterministic PRNG (SplitMix64), the same generator
/// [`crate::vegetation`] uses, seeded per candidate so a given run always
/// scatters the same distant landscape.
struct Rng(u64);

impl Rng {
    fn seeded(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xDA3E_39CB_94B9_5BDB)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }
}
