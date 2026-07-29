//! Procedural vegetation.
//!
//! The fuel raster is the only vegetation source this scenario has — there is
//! no tree inventory, and there never will be one at 10 km scale — so the
//! plants are generated from it: one archetype per `eu_fuel12` group, scattered
//! deterministically from a hash of the cell they stand in. Deterministic
//! matters more than it sounds: the same seed must produce the same forest on
//! every run, or a debrief replay would show a different landscape than the
//! incident did.
//!
//! The terrain under the plants is bare ground (see [`crate::terrain_mesh`]),
//! so *all* of the land cover a player sees is geometry: grass tussocks and
//! macchia as much as trees. That raises the plant count into the hundreds of
//! thousands, which sets the budget for everything below — each archetype is
//! built to a few dozen vertices, and the visual density comes from crown
//! width and overlap rather than from stem count, because a hundred wide
//! crowns close a canopy that a thousand narrow ones would not.
//!
//! Geometry is merged into per-chunk meshes rather than spawned per plant: one
//! entity each would cost more in transform propagation and draw calls than
//! the triangles themselves, while merged chunks give the renderer a couple of
//! hundred draws for the entire landscape and still allow frustum culling.
//!
//! The plants respond to the fire: each one remembers which fire cell it
//! stands in, and when that cell's state changes the chunk's colour attribute
//! is rewritten — foliage flares, then goes to charcoal. Only chunks whose
//! cells actually changed are touched, so the cost tracks the front.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use fire::CellFire;
use scenario::{Cell, Pos, Scenario};

use crate::field::{noise, FireField};
use crate::sim::Sim;

/// Chunk edge in fire cells. 32 cells = 640 m, matching the terrain chunking,
/// so a burning front dirties a similar number of both.
const CHUNK_CELLS: usize = 32;

/// Expected plants per 20 m fire cell (400 m²), per archetype, before the
/// patchiness modulation below. Scaled by `SPOTORNO_VEG_DENSITY` for machines
/// that would rather not draw several million triangles of macchia.
///
/// These are stand densities, not decoration: 8 tussocks per cell is 200/ha of
/// grassland, 6 macchia clumps is 150/ha of shrubland, and 3.6 stems is 90/ha
/// of open Mediterranean pine. Still below a real forest inventory, but with
/// the crown widths below it closes the cover, which is the point — bare
/// ground showing between plants is what made this look like decoration
/// scattered on a heightfield rather than like a landscape.
///
/// Set by measurement, not by taste: 10.5 M triangles rendered at 119 fps in
/// release on an M4 Pro, so there was room for roughly another half.
const DENSITY: [f32; 4] = [11.0, 3.4, 6.5, 3.8];

/// Vegetation does not honour cell boundaries, and it is not uniform inside
/// one either: real stands are patchy at tens of metres. Two octaves of value
/// noise at 60 m and 25 m modulate the local count, which is what breaks up
/// the 20 m grid the fuel raster imposes.
fn patchiness(p: Pos) -> f32 {
    let coarse = noise(p.x / 60.0, p.y / 60.0, 0x5A17);
    let fine = noise(p.x / 25.0, p.y / 25.0, 0x91C3);
    (0.25 + 1.75 * (0.65 * coarse + 0.35 * fine)).clamp(0.0, 2.0)
}

/// How long a plant flames after the fire reaches it, in simulated seconds.
/// Well short of the cell's 20-minute burn-out: the crown goes fast, the
/// ground under it keeps smouldering.
const PLANT_FLAMING_S: f32 = 240.0;

/// Vegetation archetypes, one per `eu_fuel12` group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Species {
    /// classes 1-3: grassland, drawn as crossed blade tufts
    Grass = 0,
    /// classes 4-6: broadleaves, a rounded canopy on a short trunk
    Broadleaf = 1,
    /// classes 7-9: shrubland and macchia, overlapping low domes
    Shrub = 2,
    /// classes 10-12: conifers, stacked cones on a bare trunk
    Conifer = 3,
}

impl Species {
    fn of_fuel(fuel: i32) -> Option<Species> {
        Some(match fuel {
            1..=3 => Species::Grass,
            4..=6 => Species::Broadleaf,
            7..=9 => Species::Shrub,
            10..=12 => Species::Conifer,
            _ => return None,
        })
    }

    /// Foliage colour, as a pair the per-plant tint interpolates between.
    ///
    /// A single colour per species is what makes procedural vegetation look
    /// like plastic: a real stand runs from cured straw to dark green within
    /// metres. The two ends are the dry and the vigorous extreme of the same
    /// species; each plant lands somewhere between them. Kept dark — plants
    /// are a pixel or two at commander altitude, and anything bright at that
    /// size aliases into sparkle.
    fn foliage_range(self) -> ([f32; 3], [f32; 3]) {
        match self {
            // Cured Mediterranean grassland: straw, with green only in the
            // draws. This is also the fuel that carries fire fastest. The dry
            // end sits close to the soil colour on purpose — grassland should
            // read as continuous cover, not as green dots on tan.
            Species::Grass => ([0.42, 0.36, 0.19], [0.26, 0.29, 0.15]),
            Species::Broadleaf => ([0.19, 0.24, 0.12], [0.09, 0.17, 0.09]),
            // Macchia is a mix of species by definition; the widest range.
            Species::Shrub => ([0.30, 0.27, 0.14], [0.13, 0.20, 0.10]),
            Species::Conifer => ([0.12, 0.18, 0.11], [0.06, 0.13, 0.09]),
        }
    }

    fn wood(self) -> [f32; 3] {
        match self {
            Species::Grass => [0.32, 0.29, 0.17],
            _ => [0.21, 0.17, 0.13],
        }
    }
}

/// One scattered plant, remembered so it can react to the fire.
struct Plant {
    /// Where it stands, in world metres. The plant burns on the *interpolated*
    /// arrival time at this point rather than on its cell's, so a stand
    /// catches progressively in the direction the fire is running instead of
    /// flipping as a 20 m block.
    pos: Pos,
    /// Seconds this plant lags the interpolated front, hashed per plant: even
    /// two shrubs a metre apart do not catch at the same instant.
    lag_s: f32,
    vert_start: u32,
    vert_end: u32,
}

struct Chunk {
    mesh: Handle<Mesh>,
    plants: Vec<Plant>,
    /// Colours as generated, before any fire damage.
    base: Vec<[f32; 4]>,
    /// Fire state each plant was last drawn with, so a chunk is rebuilt only
    /// when something in it actually changed.
    drawn: Vec<u8>,
}

#[derive(Resource)]
pub struct Vegetation {
    chunks: Vec<Chunk>,
}

pub fn spawn(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let scn = &sim.scenario;
    let density: f32 = std::env::var("SPOTORNO_VEG_DENSITY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);

    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        metallic: 0.0,
        // Foliage is thin: lighting it from one side only makes a forest read
        // as a flat dark mass from the shaded quarter.
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    let (rows, cols) = (scn.world.fire_rows, scn.world.fire_cols);
    let chunks_y = rows.div_ceil(CHUNK_CELLS);
    let chunks_x = cols.div_ceil(CHUNK_CELLS);
    let mut chunks = Vec::with_capacity(chunks_x * chunks_y);
    let (mut plant_count, mut tri_count) = (0usize, 0usize);

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let mut builder = Builder::default();
            let mut plants = Vec::new();

            for r in cy * CHUNK_CELLS..((cy + 1) * CHUNK_CELLS).min(rows) {
                for c in cx * CHUNK_CELLS..((cx + 1) * CHUNK_CELLS).min(cols) {
                    let cell = Cell { row: r, col: c };
                    let Some(species) = Species::of_fuel(scn.fuel_at(cell)) else {
                        continue;
                    };
                    let mut rng = Rng::seeded(r as u64 * 65_536 + c as u64);
                    let centre = scn.world.centre_of(cell);
                    let expected =
                        DENSITY[species as usize] * density * patchiness(centre);
                    let n = expected.floor() as u32
                        + u32::from(rng.unit() < expected.fract());

                    for _ in 0..n {
                        let start = builder.positions.len() as u32;
                        let Some(pos) = scatter_plant(scn, cell, species, &mut rng, &mut builder)
                        else {
                            continue;
                        };
                        plants.push(Plant {
                            pos,
                            lag_s: rng.unit() * 45.0,
                            vert_start: start,
                            vert_end: builder.positions.len() as u32,
                        });
                    }
                }
            }

            if plants.is_empty() {
                continue;
            }
            plant_count += plants.len();
            tri_count += builder.indices.len() / 3;

            let base = builder.colors.clone();
            let drawn = vec![CellFire::Unburnt as u8; plants.len()];
            let mesh = meshes.add(builder.finish());

            commands.spawn(PbrBundle {
                mesh: mesh.clone(),
                material: material.clone(),
                ..default()
            });
            chunks.push(Chunk { mesh, plants, base, drawn });
        }
    }

    info!(
        "vegetation: {plant_count} plants in {} chunks, {:.2} M triangles",
        chunks.len(),
        tri_count as f32 / 1e6
    );
    commands.insert_resource(Vegetation { chunks });
}

/// Place one plant somewhere inside `cell` and emit its geometry, returning
/// where it ended up.
fn scatter_plant(
    scn: &Scenario,
    cell: Cell,
    species: Species,
    rng: &mut Rng,
    out: &mut Builder,
) -> Option<Pos> {
    let centre = scn.world.centre_of(cell);
    let half = scn.world.cellsize * 0.5;
    let p = Pos {
        x: centre.x + (rng.unit() * 2.0 - 1.0) * half,
        y: centre.y + (rng.unit() * 2.0 - 1.0) * half,
    };
    if !scn.world.contains(p) {
        return None;
    }
    let ground = scn.terrain.height_at(p);
    let base = Vec3::new(p.x, ground, -p.y);

    // A stand of one species is not a stand of clones: size, tint and yaw all
    // jitter, which is most of what stops merged geometry looking stamped.
    let scale = 0.75 + rng.unit() * 0.5;
    let yaw = rng.unit() * std::f32::consts::TAU;

    // Where this plant sits between the dry and vigorous ends of its species,
    // correlated over ~40 m so drying runs in patches — slopes and aspects
    // cure together — with a per-plant scatter on top.
    let (dry, green) = species.foliage_range();
    let local = noise(p.x / 40.0, p.y / 40.0, 0xC0FF);
    let t = (0.72 * local + 0.28 * rng.unit()).clamp(0.0, 1.0);
    let foliage = [
        dry[0] + (green[0] - dry[0]) * t,
        dry[1] + (green[1] - dry[1]) * t,
        dry[2] + (green[2] - dry[2]) * t,
    ];
    let shade = 0.88 + rng.unit() * 0.22;
    let foliage = mul(foliage, shade);
    let wood = mul(species.wood(), shade);

    match species {
        Species::Conifer => conifer(out, base, scale, yaw, foliage, wood, rng),
        Species::Broadleaf => broadleaf(out, base, scale, yaw, foliage, wood, rng),
        Species::Shrub => shrub(out, base, scale, yaw, foliage, rng),
        Species::Grass => grass(out, base, scale, yaw, foliage, rng),
    }
    Some(p)
}

fn mul(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

// --- archetypes ------------------------------------------------------------
//
// All four are built from three primitives (a tapered prism, a cone fan and a
// squashed octahedron) and stay in the tens of vertices, because there are
// hundreds of thousands of them. Detail buys nothing here: at the altitude a
// commander plays from, a tree is a handful of pixels, and only its
// silhouette, its crown width and the shadow it casts read at all.
//
// Crowns are deliberately wide. Canopy closure is what makes a hillside look
// vegetated, and it is far cheaper to reach with overlapping crowns than with
// more stems.

fn conifer(
    out: &mut Builder,
    base: Vec3,
    scale: f32,
    yaw: f32,
    foliage: [f32; 3],
    wood: [f32; 3],
    rng: &mut Rng,
) {
    // Aleppo and maritime pine: tall, bare-stemmed, with the crown carried in
    // the top third — the profile that makes a Ligurian ridge look like one.
    let height = (12.0 + rng.unit() * 9.0) * scale;
    let radius = height * (0.26 + rng.unit() * 0.10);
    out.prism(base, height * 0.62, height * 0.040, height * 0.022, yaw, wood, 4);

    // Three skirts, each narrower and higher; the lowest is the widest.
    for i in 0..3 {
        let t = i as f32 / 2.0;
        let bottom = height * (0.34 + 0.20 * i as f32);
        let r = radius * (1.0 - 0.45 * t);
        // Foliage darkens toward the shaded interior of the crown.
        let shade = mul(foliage, 0.82 + 0.18 * t);
        out.cone(
            base + Vec3::Y * bottom,
            r,
            height * (0.36 - 0.06 * t),
            yaw + i as f32 * 0.7,
            shade,
            6,
        );
    }
}

fn broadleaf(
    out: &mut Builder,
    base: Vec3,
    scale: f32,
    yaw: f32,
    foliage: [f32; 3],
    wood: [f32; 3],
    rng: &mut Rng,
) {
    // Holm oak: short trunk, crown wider than the tree is tall.
    let height = (7.0 + rng.unit() * 6.0) * scale;
    let trunk = height * 0.34;
    out.prism(base, trunk * 1.2, height * 0.055, height * 0.038, yaw, wood, 4);

    // Three offset lobes read as a broad, lumpy crown; one alone is a
    // lollipop from every angle.
    let crown = height * (0.46 + rng.unit() * 0.12);
    out.blob(base + Vec3::Y * (trunk + crown * 0.62), crown, yaw, foliage);
    out.blob(
        base + Vec3::new(crown * 0.55, trunk + crown * 0.95, crown * 0.25),
        crown * 0.68,
        yaw + 1.1,
        mul(foliage, 1.08),
    );
    out.blob(
        base + Vec3::new(-crown * 0.45, trunk + crown * 0.80, -crown * 0.4),
        crown * 0.62,
        yaw + 2.4,
        mul(foliage, 0.88),
    );
}

fn shrub(out: &mut Builder, base: Vec3, scale: f32, yaw: f32, foliage: [f32; 3], rng: &mut Rng) {
    // Macchia: a clump of cistus and heather, knee to shoulder high and much
    // wider than it is tall. The spread matters more than the height — this is
    // the fuel that carries fire across a Ligurian hillside, and it should
    // look like continuous cover, not like scattered bushes.
    let r = (2.6 + rng.unit() * 2.2) * scale;
    let lobes = 3 + (rng.unit() * 2.0) as usize;
    for i in 0..lobes {
        let a = yaw + i as f32 * (std::f32::consts::TAU / lobes as f32) + rng.unit() * 0.6;
        let reach = r * (0.35 + rng.unit() * 0.5);
        let off = Vec3::new(a.cos() * reach, 0.0, a.sin() * reach);
        // Lobes vary in tone: macchia is a mix of species, never one green.
        let tone = mul(foliage, 0.80 + rng.unit() * 0.45);
        out.dome(base + off, r * (0.45 + rng.unit() * 0.35), r * (0.5 + rng.unit() * 0.4), a, tone);
    }
}

fn grass(out: &mut Builder, base: Vec3, scale: f32, yaw: f32, foliage: [f32; 3], rng: &mut Rng) {
    // A tussock, not a blade: the unit of grassland at this scale is a clump
    // half a metre high and a couple of metres across. Drawn as a low fan plus
    // a few upright blades, so it holds a silhouette from a low camera angle
    // and still covers ground when seen from above.
    let spread = (1.5 + rng.unit() * 1.4) * scale;
    let h = (0.5 + rng.unit() * 0.6) * scale;

    // Ground fan: what actually closes the cover from altitude.
    out.dome(base, spread, h * 0.55, yaw, mul(foliage, 0.9));

    for i in 0..2 {
        let a = yaw + i as f32 * (std::f32::consts::TAU / 3.0) + rng.unit() * 0.5;
        out.blade(
            base + Vec3::new(a.cos() * spread * 0.3, 0.0, a.sin() * spread * 0.3),
            spread * 0.45,
            h * (1.2 + rng.unit() * 1.0),
            a,
            mul(foliage, 1.0 + rng.unit() * 0.2),
        );
    }
}

// --- mesh building ---------------------------------------------------------

/// No UV attribute: the vegetation material carries no texture, and at these
/// vertex counts eight bytes each is tens of megabytes of nothing.
#[derive(Default)]
struct Builder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Builder {
    fn vertex(&mut self, p: Vec3, c: [f32; 3]) -> u32 {
        let i = self.positions.len() as u32;
        self.positions.push([p.x, p.y, p.z]);
        self.normals.push([0.0, 1.0, 0.0]); // replaced in `finish`
        self.colors.push([c[0], c[1], c[2], 1.0]);
        i
    }

    /// Tapered vertical prism: trunks and stems.
    fn prism(
        &mut self,
        base: Vec3,
        height: f32,
        r_bottom: f32,
        r_top: f32,
        yaw: f32,
        color: [f32; 3],
        sides: usize,
    ) {
        let start = self.positions.len() as u32;
        for i in 0..sides {
            let a = yaw + i as f32 / sides as f32 * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            self.vertex(base + Vec3::new(c * r_bottom, 0.0, s * r_bottom), color);
            self.vertex(base + Vec3::new(c * r_top, height, s * r_top), color);
        }
        for i in 0..sides {
            let a = start + (i as u32) * 2;
            let b = start + (((i + 1) % sides) as u32) * 2;
            self.indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
        }
    }

    /// Low lumpy dome: shrub lobes and grass tussocks. An apex fan like
    /// [`Builder::cone`], but squat and with an irregular rim, which is what
    /// separates a bush from a party hat. Five segments — there are hundreds
    /// of thousands of these.
    fn dome(&mut self, base: Vec3, radius: f32, height: f32, yaw: f32, color: [f32; 3]) {
        const SEGMENTS: usize = 5;
        // The tip leans, so a clump of domes does not read as a row of cones.
        let (ls, lc) = yaw.sin_cos();
        let apex = self.vertex(
            base + Vec3::new(lc * radius * 0.18, height, ls * radius * 0.18),
            color,
        );
        let ring = self.positions.len() as u32;
        for i in 0..SEGMENTS {
            let a = yaw + i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            // Deterministic rim wobble, and a darker skirt where the lobe
            // meets the ground.
            let r = radius * (0.78 + 0.44 * ((a * 2.7).sin() * 0.5 + 0.5));
            let skirt = [color[0] * 0.72, color[1] * 0.72, color[2] * 0.72];
            self.vertex(base + Vec3::new(c * r, height * 0.06, s * r), skirt);
        }
        for i in 0..SEGMENTS {
            let a = ring + i as u32;
            let b = ring + ((i + 1) % SEGMENTS) as u32;
            self.indices.extend_from_slice(&[apex, a, b]);
        }
    }

    /// Cone as an apex fan over a ring: conifer skirts.
    fn cone(
        &mut self,
        base: Vec3,
        radius: f32,
        height: f32,
        yaw: f32,
        color: [f32; 3],
        segments: usize,
    ) {
        let apex = self.vertex(base + Vec3::Y * height, color);
        let ring = self.positions.len() as u32;
        for i in 0..segments {
            let a = yaw + i as f32 / segments as f32 * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            self.vertex(base + Vec3::new(c * radius, 0.0, s * radius), color);
        }
        for i in 0..segments {
            let a = ring + i as u32;
            let b = ring + ((i + 1) % segments) as u32;
            self.indices.extend_from_slice(&[apex, a, b]);
        }
    }

    /// Squashed octahedron: a broadleaf crown in 8 triangles.
    fn blob(&mut self, centre: Vec3, radius: f32, yaw: f32, color: [f32; 3]) {
        let top = self.vertex(centre + Vec3::Y * radius, color);
        let bottom = self.vertex(centre - Vec3::Y * radius * 0.8, color);
        let ring = self.positions.len() as u32;
        for i in 0..4 {
            let a = yaw + i as f32 * std::f32::consts::FRAC_PI_2;
            let (s, c) = a.sin_cos();
            // Slight per-lobe stretch so the crown is not perfectly regular.
            let k = radius * if i % 2 == 0 { 1.15 } else { 0.85 };
            self.vertex(centre + Vec3::new(c * k, 0.0, s * k), color);
        }
        for i in 0..4u32 {
            let a = ring + i;
            let b = ring + (i + 1) % 4;
            self.indices.extend_from_slice(&[top, a, b, bottom, b, a]);
        }
    }

    /// Single upright quad, leaning slightly: one blade of a grass tuft.
    fn blade(&mut self, base: Vec3, width: f32, height: f32, yaw: f32, color: [f32; 3]) {
        let (s, c) = yaw.sin_cos();
        let across = Vec3::new(c, 0.0, s) * width * 0.5;
        let lean = Vec3::new(-s, 0.0, c) * height * 0.25;
        let start = self.positions.len() as u32;
        // Root colour is darker: a lit blade should not read as a flat card.
        let root = mul(color, 0.65);
        self.vertex(base - across, root);
        self.vertex(base + across, root);
        self.vertex(base + across * 0.35 + lean + Vec3::Y * height, color);
        self.vertex(base - across * 0.35 + lean + Vec3::Y * height, color);
        self.indices
            .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    }

    /// Area-weighted vertex normals from the faces, then the mesh.
    fn finish(mut self) -> Mesh {
        for n in &mut self.normals {
            *n = [0.0, 0.0, 0.0];
        }
        for tri in self.indices.chunks_exact(3) {
            let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let (pa, pb, pc) = (
                Vec3::from(self.positions[a]),
                Vec3::from(self.positions[b]),
                Vec3::from(self.positions[c]),
            );
            let face = (pb - pa).cross(pc - pa);
            for i in [a, b, c] {
                self.normals[i] = (Vec3::from(self.normals[i]) + face).into();
            }
        }
        for n in &mut self.normals {
            *n = Vec3::from(*n).normalize_or_zero().into();
        }

        let mut mesh =
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

// --- burning ---------------------------------------------------------------

/// Burn the plants the fire has reached.
///
/// Each plant is tested against the *interpolated* arrival time at its own
/// position plus its own hashed lag, so a stand ignites as a wave crossing it
/// rather than as a cell-shaped block, and a burning cell contains flaring,
/// charred and untouched plants at once — which is what a fire edge looks like.
///
/// Only chunks with at least one changed plant get their colour attribute
/// rewritten, so a front crossing two chunks costs two buffer uploads rather
/// than 250.
pub fn burn(sim: Res<Sim>, mut veg: ResMut<Vegetation>, mut meshes: ResMut<Assets<Mesh>>) {
    if !sim.is_changed() {
        return;
    }
    let field = FireField {
        state: sim.fire.state(),
        arrival: sim.fire.arrival_times(),
        intensity: sim.fire.intensity(),
        world: sim.scenario.world,
    };
    let now = sim.time_s() as f32;

    for chunk in &mut veg.chunks {
        let mut wanted = Vec::with_capacity(chunk.plants.len());
        let mut dirty = false;
        for (plant, &drawn) in chunk.plants.iter().zip(&chunk.drawn) {
            let s = plant_state(&field, plant, now);
            dirty |= s != drawn;
            wanted.push(s);
        }
        if !dirty {
            continue;
        }

        let mut colors = chunk.base.clone();
        for ((plant, drawn), s) in chunk.plants.iter().zip(&mut chunk.drawn).zip(wanted) {
            *drawn = s;
            if s == CellFire::Unburnt as u8 {
                continue;
            }
            let range = plant.vert_start as usize..plant.vert_end as usize;
            if s == CellFire::Burning as u8 {
                // Alight: pushed above 1.0 so the bloom pass catches it, and
                // scaled by local intensity so a grass flare and a crowning
                // conifer do not glow identically.
                let k = (field.intensity(plant.pos) / 4000.0).clamp(0.2, 1.0);
                for c in &mut colors[range] {
                    *c = [1.0 + 2.2 * k, 0.42 + 0.25 * k, 0.06, 1.0];
                }
            } else {
                // Charcoal, keeping a trace of the original tint so a burnt
                // conifer stand still reads differently from burnt grass.
                for c in &mut colors[range] {
                    *c = [0.07 + c[0] * 0.10, 0.06 + c[1] * 0.08, 0.06 + c[2] * 0.08, 1.0];
                }
            }
        }

        if let Some(mesh) = meshes.get_mut(&chunk.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        }
    }
}

/// Fire state of a single plant, from the interpolated front.
fn plant_state(field: &FireField, plant: &Plant, now: f32) -> u8 {
    let Some(arrival) = field.arrival(plant.pos) else {
        return CellFire::Unburnt as u8;
    };
    let age = now - (arrival + plant.lag_s);
    if age < 0.0 {
        CellFire::Unburnt as u8
    } else if age < PLANT_FLAMING_S {
        CellFire::Burning as u8
    } else {
        CellFire::Burnt as u8
    }
}

/// Small deterministic PRNG (SplitMix64), seeded per cell so the same forest
/// grows on every run.
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

    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }
}
