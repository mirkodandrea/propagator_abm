//! Fuel-driven vegetation using Blender-authored tree and shrub archetypes.
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
//! built to at most 120 vertices, and the visual density comes from crown
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
//!
//! Every chunk is built twice: with the authored models, and with a proxy of
//! a couple of dozen faces per plant (a faceted crown on a three-sided
//! trunk) that stands in for it beyond [`crate::quality::Graphics::lod_m`].
//! A plant there covers a few pixels, and the GPU — which is what limits the
//! frame rate — draws every vegetation triangle once per pass, shadow
//! cascades included. Both meshes burn together.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use fire::CellFire;
use scenario::{Cell, Pos, Scenario};

use crate::field::{noise, FireField};
use crate::retro;
use crate::retro::RetroMaterial;
use crate::sim::Sim;

/// Chunk edge in fire cells. 32 cells = 640 m, matching the terrain chunking,
/// so a burning front dirties a similar number of both.
const CHUNK_CELLS: usize = 32;

/// Expected plants per 20 m fire cell (400 m²), per archetype, before the
/// patchiness modulation below. Scaled by the graphics quality
/// ([`crate::quality::Graphics::vegetation`], or `KIOSK_VEG_DENSITY`).
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
///
/// Broadleaf is 2.5 (was 3.4 for class 5 and 1.4 olives for class 4): the
/// chestnut crowns are wide, and the total plant count stays the same.
const DENSITY: [f32; 4] = [11.0, 2.5, 6.5, 3.8];

/// Within a group, how the fuel class scales [`DENSITY`]: moist broadleaf a
/// little denser than dry, macchia from sparse (7) to closed (9). Weighted by
/// the territory's cells, the mean stays at 1: no more plants overall.
fn class_density(fuel: i32) -> f32 {
    match fuel {
        4 => 1.04,
        5 => 0.97,
        7 => 0.75,
        9 => 1.15,
        _ => 1.0,
    }
}

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
    /// classes 4-6: broadleaves, drawn as chestnuts: a big round lobed crown
    /// on a short trunk
    Broadleaf = 1,
    /// classes 7-9: shrubland and macchia, low dense rounded clumps
    Shrub = 2,
    /// classes 10-12: conifers, drawn as maritime pines: a tall bare reddish
    /// trunk under a flat umbrella crown
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

    /// Foliage colour for a fuel class, as a pair the per-plant tint
    /// interpolates between.
    ///
    /// A single colour per class is what makes procedural vegetation look like
    /// plastic: a real stand varies within metres. The two ends are the dry
    /// and the vigorous extreme; each plant lands somewhere between them. The
    /// four groups are kept apart in hue as well as in shape (iteration 5b,
    /// the player must tell them apart): pine dark blue-green, chestnut a
    /// bright mid green, macchia dark olive, grass straw.
    fn foliage_range(fuel: i32) -> ([f32; 3], [f32; 3]) {
        match fuel {
            // gardens and lawns round the houses
            1 => ([0.46, 0.52, 0.24], [0.34, 0.48, 0.20]),
            // Cured Mediterranean grassland: straw; the fuel that carries fire
            // fastest. Class 3 (dry meadow) paler and more golden.
            2 => ([0.66, 0.56, 0.28], [0.54, 0.50, 0.24]),
            3 => ([0.80, 0.68, 0.36], [0.70, 0.60, 0.30]),
            // Chestnut; class 4 (moist) greener than 5.
            4 => ([0.24, 0.40, 0.13], [0.17, 0.34, 0.11]),
            5..=6 => ([0.32, 0.42, 0.15], [0.24, 0.36, 0.12]),
            // Macchia: dark olive, greyer than the chestnut, warmer than the pine.
            7..=9 => ([0.27, 0.32, 0.15], [0.18, 0.25, 0.12]),
            // Maritime pine: dark, toward blue.
            _ => ([0.13, 0.22, 0.18], [0.08, 0.17, 0.15]),
        }
    }

    fn wood(self) -> [f32; 3] {
        match self {
            Species::Grass => [0.32, 0.29, 0.17],
            // maritime pine bark is red-brown, and it is what shows: the
            // trunk is bare for most of its height
            Species::Conifer => [0.46, 0.23, 0.14],
            _ => [0.24, 0.18, 0.13],
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
    /// The same plant in the distant proxy mesh.
    low_start: u32,
    low_end: u32,
    /// Its undersides in the close-range mesh (see [`CLOSE_M`]).
    under_start: u32,
    under_end: u32,
}

struct Chunk {
    /// Authored models and distant proxies (see the module doc).
    mesh: Handle<Mesh>,
    low_mesh: Handle<Mesh>,
    full: Entity,
    low: Entity,
    /// The faces `full` leaves out because they look down (see
    /// [`crate::models::split_plant`]), shown only within [`CLOSE_M`]. `None`
    /// for a chunk without trees.
    under: Option<(Handle<Mesh>, Entity)>,
    /// Which of the two is on show.
    near: bool,
    /// Whether the undersides are on show.
    close: bool,
    /// World-space bounds (Bevy frame) of the plants, for the LOD distance.
    bounds: (Vec3, Vec3),
    /// Fire cells the chunk covers, `(row0, row1, col0, col1)`, end exclusive.
    cells: (usize, usize, usize, usize),
    plants: Vec<Plant>,
    /// Colours as generated, before any fire damage.
    base: Vec<[f32; 4]>,
    low_base: Vec<[f32; 4]>,
    under_base: Vec<[f32; 4]>,
    /// Vertex positions as generated, so a plant hidden on cleared ground
    /// (see [`CLEARED`]) can be brought back when the game restarts.
    pos_base: Vec<[f32; 3]>,
    low_pos_base: Vec<[f32; 3]>,
    under_pos_base: Vec<[f32; 3]>,
    /// Fire state each plant was last drawn with, so a chunk is rebuilt only
    /// when something in it actually changed.
    drawn: Vec<u8>,
    /// Whether any plant is drawn other than unburnt.
    touched: bool,
}

#[derive(Resource)]
pub struct Vegetation {
    chunks: Vec<Chunk>,
    /// Beyond this distance from the camera a chunk shows its proxies.
    lod_m: f32,
    /// Simulated time the plants were last drawn at: the plants are a
    /// function of the fire and the clock, so an unchanged clock (a paused
    /// world, the planning phase) costs nothing.
    drawn_at: Option<f32>,
    /// Cells cleared by crews when the plants were last drawn.
    drawn_cleared: usize,
}

/// Drawn state of a plant standing on ground a crew has cleared: it is gone,
/// leaving bare earth (the ground itself is tinted by `fire_view`). Above
/// the `CellFire` values.
const CLEARED: u8 = 10;
/// Share of the plants inside a cleared line that are taken away.
const CLEARED_SHARE: f32 = 0.85;

pub fn spawn(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RetroMaterial>>,
    gfx: Res<crate::quality::Graphics>,
) {
    let scn = &sim.scn;
    // Half the stand densities below at the top quality (user decision
    // 2026-10-08: fewer trees for now), less on weaker machines.
    let density = gfx.vegetation;

    // VR-training dev scenarios tint every plant toward the palette's grid
    // colour instead of leaving it white — vertex colour still carries the
    // per-species shape, but the hue reads as training-ground foliage, not
    // real macchia.
    let base_color = Color::WHITE;
    let material = materials.add(retro::material_with_style(StandardMaterial {
        base_color,
        perceptual_roughness: 0.92,
        metallic: 0.0,
        // Leaves are matte, not glassy: a near-zero reflectance is what
        // stops a sunlit canopy from picking up a specular sheen no real
        // macchia has.
        reflectance: 0.04,
        // Foliage is thin: lighting it from one side only makes a forest read
        // as a flat dark mass from the shaded quarter.
        double_sided: true,
        cull_mode: None,
        unlit: false,
        ..default()
    }, false, retro::RetroStyle::BACKGROUND));

    let (rows, cols) = (scn.world.fire_rows, scn.world.fire_cols);
    let chunks_y = rows.div_ceil(CHUNK_CELLS);
    let chunks_x = cols.div_ceil(CHUNK_CELLS);
    let mut chunks = Vec::with_capacity(chunks_x * chunks_y);
    let (mut plant_count, mut tri_count, mut low_tri_count) = (0usize, 0usize, 0usize);

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let mut builder = Lods::default();
            let mut plants = Vec::new();

            for r in cy * CHUNK_CELLS..((cy + 1) * CHUNK_CELLS).min(rows) {
                for c in cx * CHUNK_CELLS..((cx + 1) * CHUNK_CELLS).min(cols) {
                    let cell = Cell { row: r, col: c };
                    let Some(species) = Species::of_fuel(scn.fuel_at(cell)) else {
                        continue;
                    };
                    let mut rng = Rng::seeded(r as u64 * 65_536 + c as u64);
                    let centre = scn.world.centre_of(cell);
                    let stand_density = DENSITY[species as usize] * class_density(scn.fuel_at(cell));
                    let expected = stand_density * density * patchiness(centre);
                    let n = expected.floor() as u32 + u32::from(rng.unit() < expected.fract());

                    for _ in 0..n {
                        let start = builder.full.positions.len() as u32;
                        let low_start = builder.low.positions.len() as u32;
                        let under_start = builder.under.positions.len() as u32;
                        let Some(pos) = scatter_plant(scn, cell, species, &mut rng, &mut builder)
                        else {
                            continue;
                        };
                        plants.push(Plant {
                            pos,
                            lag_s: rng.unit() * 45.0,
                            vert_start: start,
                            vert_end: builder.full.positions.len() as u32,
                            low_start,
                            low_end: builder.low.positions.len() as u32,
                            under_start,
                            under_end: builder.under.positions.len() as u32,
                        });
                    }
                }
            }

            if builder.full.positions.is_empty() {
                continue;
            }
            plant_count += plants.len();
            tri_count += builder.full.indices.len() / 3;
            low_tri_count += builder.low.indices.len() / 3;

            let bounds = builder.full.positions.iter().fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(lo, hi), p| (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p))),
            );
            let base = builder.full.colors.clone();
            let low_base = builder.low.colors.clone();
            let under_base = builder.under.colors.clone();
            let drawn = vec![CellFire::Unburnt as u8; plants.len()];
            let pos_base = builder.full.positions.clone();
            let low_pos_base = builder.low.positions.clone();
            let under_pos_base = builder.under.positions.clone();
            let mesh = meshes.add(builder.full.finish());
            let low_mesh = meshes.add(builder.low.finish());
            let under = (!builder.under.positions.is_empty()).then(|| {
                let mesh = meshes.add(builder.under.finish());
                let entity = commands.spawn(MaterialMeshBundle::<RetroMaterial> {
                    mesh: mesh.clone(),
                    material: material.clone(),
                    ..default()
                }).id();
                (mesh, entity)
            });

            // Vegetation is visual only: Mario can run through trunks and
            // canopies, and the follow camera is not clipped by the forest.
            // The full models start on show, until the LOD system picks.
            let full = commands.spawn(MaterialMeshBundle::<RetroMaterial> {
                mesh: mesh.clone(),
                material: material.clone(),
                ..default()
            }).id();
            let low = commands.spawn(MaterialMeshBundle::<RetroMaterial> {
                mesh: low_mesh.clone(),
                material: material.clone(),
                visibility: Visibility::Hidden,
                ..default()
            }).id();
            chunks.push(Chunk {
                mesh,
                low_mesh,
                full,
                low,
                under,
                near: true,
                close: true,
                bounds,
                cells: (
                    cy * CHUNK_CELLS,
                    ((cy + 1) * CHUNK_CELLS).min(rows),
                    cx * CHUNK_CELLS,
                    ((cx + 1) * CHUNK_CELLS).min(cols),
                ),
                plants,
                base,
                low_base,
                under_base,
                pos_base,
                low_pos_base,
                under_pos_base,
                drawn,
                touched: false,
            });
        }
    }

    info!(
        "vegetation: {plant_count} plants in {} chunks, {:.2} M triangles ({:.2} M as distant proxies)",
        chunks.len(),
        tri_count as f32 / 1e6,
        low_tri_count as f32 / 1e6
    );
    commands.insert_resource(Vegetation { chunks, lod_m: gfx.lod_m, drawn_at: None, drawn_cleared: 0 });
}

/// Whether the 5 m land cover leaves room for this plant at `p`: nothing grows
/// on roads, tracks, buildings, yards or irrigated plots, and a tree keeps its
/// crown clear of them too. Without cover the fuel cell alone decides.
fn on_natural_cover(scn: &Scenario, species: Species, p: Pos) -> bool {
    let Some(cover) = &scn.cover else {
        return true;
    };
    let reach = match species {
        Species::Conifer | Species::Broadleaf => 3.0,
        Species::Shrub => 1.5,
        Species::Grass => 0.0,
    };
    [(0.0, 0.0), (reach, 0.0), (-reach, 0.0), (0.0, reach), (0.0, -reach)]
        .into_iter()
        .all(|(dx, dy)| cover.is_natural(Pos { x: p.x + dx, y: p.y + dy }))
}

/// Place one plant somewhere inside `cell` and emit its geometry, returning
/// where it ended up.
fn scatter_plant(
    scn: &Scenario,
    cell: Cell,
    species: Species,
    rng: &mut Rng,
    out: &mut Lods,
) -> Option<Pos> {
    let centre = scn.world.centre_of(cell);
    let half = scn.world.cellsize * 0.5;
    let p = Pos {
        x: centre.x + (rng.unit() * 2.0 - 1.0) * half,
        y: centre.y + (rng.unit() * 2.0 - 1.0) * half,
    };
    if !scn.world.contains(p) || !on_natural_cover(scn, species, p) {
        return None;
    }
    let ground = scn.terrain.height_at(p);
    let base = Vec3::new(p.x, ground, -p.y);

    // A stand of one species is not a stand of clones: size, tint and yaw all
    // jitter, which is most of what stops merged geometry looking stamped.
    let scale = (0.75 + rng.unit() * 0.5) * match species {
        Species::Conifer | Species::Broadleaf => crate::visual_scale::TREE,
        Species::Shrub => crate::visual_scale::SHRUB,
        Species::Grass => 1.0,
    };
    let yaw = rng.unit() * std::f32::consts::TAU;

    // Where this plant sits between the dry and vigorous ends of its species,
    // correlated over ~40 m so drying runs in patches — slopes and aspects
    // cure together — with a per-plant scatter on top.
    let (dry, green) = Species::foliage_range(scn.fuel_at(cell));
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

    // One model per group, no mixing: the player reads the fuel from the
    // silhouette (cypresses and olives used to stand among pines and oaks).
    match species {
        Species::Conifer => conifer(out, base, scale, yaw, foliage, wood, rng),
        Species::Broadleaf => broadleaf(out, base, scale, yaw, foliage, wood, rng),
        Species::Shrub => shrub(out, base, scale, yaw, foliage, rng),
        Species::Grass if true && rng.unit() < 0.025 => {
            // Lone field trees break the meadow silhouette without changing
            // its simulation fuel; they share the same interpolated burn field.
            let height = (7.0 + rng.unit() * 5.0) * scale * crate::visual_scale::TREE;
            out.model("olive", base, Vec3::splat(height), yaw, [0.33, 0.40, 0.26], wood);
        }
        Species::Grass => grass(out, base, scale, yaw, foliage, rng),
    }
    Some(p)
}

fn mul(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

// --- archetypes ------------------------------------------------------------
//
// Blender archetypes retain chunk batching, species palettes and burn ranges.
// Normalized trees scale to the existing ecological height distributions.
fn conifer(out: &mut Lods, base: Vec3, scale: f32, yaw: f32,
    foliage: [f32; 3], wood: [f32; 3], rng: &mut Rng) {
    // maritime pine: tall, the umbrella a little wider than the model
    let height = (13.0 + rng.unit() * 8.0) * scale;
    let width = height * (1.0 + rng.unit() * 0.3);
    out.model("pine", base, Vec3::new(width, height, width), yaw, foliage, wood);
}

fn broadleaf(out: &mut Lods, base: Vec3, scale: f32, yaw: f32,
    foliage: [f32; 3], wood: [f32; 3], rng: &mut Rng) {
    // chestnut: lower than the pines, the crown about as wide as tall
    let height = (8.0 + rng.unit() * 5.0) * scale;
    out.model("chestnut", base, Vec3::new(height * 1.1, height, height * 1.1), yaw, foliage, wood);
}

fn shrub(out: &mut Lods, base: Vec3, scale: f32, yaw: f32,
    foliage: [f32; 3], rng: &mut Rng) {
    // low, wide and stemless: macchia must not read as young trees
    let r = (2.6 + rng.unit() * 2.2) * scale;
    out.model("bush", base, Vec3::new(r, r * 0.8, r), yaw, foliage, foliage);
}

fn grass(out: &mut Lods, base: Vec3, scale: f32, yaw: f32, foliage: [f32; 3], rng: &mut Rng) {
    // A tussock, not a blade: the unit of grassland at this scale is a clump
    // half a metre high and a couple of metres across. Drawn as a low fan plus
    // a few upright blades, so it holds a silhouette from a low camera angle
    // and still covers ground when seen from above.
    let spread = (1.5 + rng.unit() * 1.4) * scale;
    let h = (0.5 + rng.unit() * 0.6) * scale;

    // Ground fan: what actually closes the cover from altitude. It is also
    // the whole distant proxy: the blades are under a pixel there.
    out.full.dome(base, spread, h * 0.55, yaw, mul(foliage, 0.9));
    out.low.dome(base, spread, h * 0.55, yaw, mul(foliage, 0.9));

    for i in 0..2 {
        let a = yaw + i as f32 * (std::f32::consts::TAU / 3.0) + rng.unit() * 0.5;
        out.full.blade(
            base + Vec3::new(a.cos() * spread * 0.3, 0.0, a.sin() * spread * 0.3),
            spread * 0.45,
            h * (1.2 + rng.unit() * 1.0),
            a,
            mul(foliage, 1.0 + rng.unit() * 0.2),
        );
    }
}

// --- mesh building ---------------------------------------------------------

/// The two meshes of a chunk, built side by side.
#[derive(Default)]
struct Lods {
    full: Builder,
    low: Builder,
    /// Only the faces the full models leave out, see [`CLOSE_M`].
    under: Builder,
}

impl Lods {
    fn model(&mut self, name: &str, base: Vec3, scale: Vec3, yaw: f32,
        foliage: [f32; 3], wood: [f32; 3]) {
        let (up, down) = crate::models::split_plant(name);
        self.full.model(up, base, scale, yaw, foliage, wood);
        self.under.model(down, base, scale, yaw, foliage, wood);
        self.low.proxy(name, base, scale, yaw, foliage, wood);
    }
}

/// What a distant plant keeps of its model: the crown's bounding box, its
/// tone low and high, and the trunk below it.
struct Proxy {
    crown: (Vec3, Vec3),
    shade_low: f32,
    shade_high: f32,
    /// Trunk radius, when the model has bare wood under the crown.
    trunk: Option<f32>,
}

fn proxy(name: &str) -> &'static Proxy {
    static PROXIES: std::sync::OnceLock<std::collections::HashMap<&'static str, Proxy>> =
        std::sync::OnceLock::new();
    &PROXIES.get_or_init(|| {
        ["pine", "chestnut", "bush", "olive"]
            .into_iter()
            .map(|name| {
                let m = crate::models::model(name);
                let leaves = || (0..m.positions.len()).filter(|&i| !m.wood[i]);
                let (lo, hi) = leaves().fold(
                    (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                    |(lo, hi), i| (lo.min(Vec3::from(m.positions[i])), hi.max(Vec3::from(m.positions[i]))),
                );
                let mid = (lo.y + hi.y) * 0.5;
                // the same tone `Builder::model` gives each leaf vertex
                let mean = |upper: bool| {
                    let (sum, n) = leaves()
                        .filter(|&i| (m.positions[i][1] > mid) == upper)
                        .fold((0.0, 0), |(s, n), i| (s + m.colors[i][1] / 0.38, n + 1));
                    if n == 0 { 1.0 } else { sum / n as f32 }
                };
                let bare: Vec<usize> = (0..m.positions.len())
                    .filter(|&i| m.wood[i] && m.positions[i][1] < lo.y)
                    .collect();
                let trunk = (!bare.is_empty()).then(|| {
                    let r = bare.iter()
                        .map(|&i| Vec2::new(m.positions[i][0], m.positions[i][2]).length())
                        .fold(0.0, f32::max);
                    r.max(0.01)
                });
                (name, Proxy { crown: (lo, hi), shade_low: mean(false), shade_high: mean(true), trunk })
            })
            .collect()
    })[name]
}

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
    fn model(&mut self, model: &crate::models::Model, base: Vec3, scale: Vec3, yaw: f32,
        foliage: [f32; 3], wood: [f32; 3]) {
        let start = self.positions.len() as u32;
        let rotation = Quat::from_rotation_y(yaw);
        for (i, p) in model.positions.iter().enumerate() {
            let color = if model.wood[i] { wood } else { foliage };
            // Preserve species colour and the authored crown's tonal variation.
            let shade = if model.wood[i] { 1.0 } else { model.colors[i][1] / 0.38 };
            self.vertex(base + rotation * (Vec3::from(*p) * scale), mul(color, shade));
        }
        self.indices.extend(model.indices.iter().map(|i| start + i));
    }

    /// The distant stand-in for [`Builder::model`]: a six-sided crown (a
    /// bottom point, two rings, a top point) over the model's leaves, and a
    /// three-sided trunk where the model shows bare wood. 18–30 triangles
    /// instead of 100–156.
    fn proxy(&mut self, name: &str, base: Vec3, scale: Vec3, yaw: f32,
        foliage: [f32; 3], wood: [f32; 3]) {
        const SIDES: u32 = 6;
        let p = proxy(name);
        // a few percent darker: the authored crown shades itself in its lobes
        let foliage = mul(foliage, 0.95);
        let rotation = Quat::from_rotation_y(yaw);
        let at = |v: Vec3| base + rotation * (v * scale);
        let (lo, hi) = p.crown;
        let (centre, radius) = ((lo + hi) * 0.5, (hi - lo) * 0.5);

        if let Some(r) = p.trunk {
            let start = self.positions.len() as u32;
            for i in 0..3 {
                let a = i as f32 * std::f32::consts::TAU / 3.0;
                let d = Vec3::new(a.cos() * r, 0.0, a.sin() * r);
                self.vertex(at(Vec3::new(centre.x, 0.0, centre.z) + d), wood);
                self.vertex(at(Vec3::new(centre.x, lo.y + radius.y * 0.4, centre.z) + d), wood);
            }
            for i in 0..3 {
                let (b0, b1) = (start + 2 * i, start + 2 * ((i + 1) % 3));
                self.indices.extend_from_slice(&[b0, b0 + 1, b1, b1, b0 + 1, b1 + 1]);
            }
        }

        let y = |t: f32| lo.y + (hi.y - lo.y) * t;
        let bottom = self.vertex(at(Vec3::new(centre.x, y(0.04), centre.z)), mul(foliage, p.shade_low * 0.8));
        let ring = self.positions.len() as u32;
        for (t, k, shade) in [(0.3, 0.85, p.shade_low), (0.62, 1.0, p.shade_high)] {
            for i in 0..SIDES {
                let a = (i as f32 + 0.5 * t) * std::f32::consts::TAU / SIDES as f32;
                let v = Vec3::new(centre.x + a.cos() * radius.x * k, y(t), centre.z + a.sin() * radius.z * k);
                self.vertex(at(v), mul(foliage, shade));
            }
        }
        let top = self.vertex(at(Vec3::new(centre.x, hi.y, centre.z)), mul(foliage, p.shade_high * 1.05));
        for i in 0..SIDES {
            let (a, b) = (ring + i, ring + (i + 1) % SIDES);
            let (c, d) = (a + SIDES, b + SIDES);
            // counter-clockwise from outside, so the face normals `finish`
            // averages point out of the crown
            self.indices.extend_from_slice(&[bottom, a, b, a, c, b, c, d, b, c, top, d]);
        }
    }

    fn vertex(&mut self, p: Vec3, c: [f32; 3]) -> u32 {
        let i = self.positions.len() as u32;
        self.positions.push([p.x, p.y, p.z]);
        self.normals.push([0.0, 1.0, 0.0]); // replaced in `finish`
        // Vertex colours are linear in Bevy. Ecological palettes above are
        // sRGB; without conversion dark foliage washes out to pastel mint.
        let linear = Color::srgb(c[0], c[1], c[2]).to_linear();
        self.colors.push([linear.red, linear.green, linear.blue, 1.0]);
        i
    }

    fn boulder(&mut self, base: Vec3, radius: f32, aspect: f32, yaw: f32) {
        let start = self.positions.len() as u32;
        for level in 0..2 {
            for i in 0..6 {
                let a = yaw + i as f32 * std::f32::consts::TAU / 6.0;
                let r = radius * if level == 0 { 1.0 } else { 0.65 + 0.12 * a.sin() };
                let h = if level == 0 { 0.0 } else { radius * aspect * (0.75 + 0.15 * a.cos()) };
                self.vertex(base + Vec3::new(a.cos()*r, h, a.sin()*r),
                    if level == 0 { [0.39, 0.38, 0.33] } else { [0.64, 0.63, 0.56] });
            }
        }
        let cap = self.vertex(base + Vec3::Y * radius * aspect, [0.70, 0.68, 0.61]);
        for i in 0..6u32 {
            let a = start+i;
            let b = start+(i+1)%6;
            self.indices.extend_from_slice(&[a, a+6, b, b, a+6, b+6, cap, b+6, a+6]);
        }
    }

    /// Low, irregular dome for the remaining procedural grass tussocks.
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
/// than 250; and a chunk the fire has not reached (no arrival time in its
/// cells or their neighbours, whose times its plants interpolate) is not even
/// looked at.
///
/// A rewritten chunk goes to the GPU whole (megabytes for a forest chunk), so
/// the plants are redrawn at most every [`BURN_REDRAW_S`] of real time: at
/// ×40 that is still a step of a few simulated seconds against a flaming
/// phase of four minutes.
pub fn burn(
    sim: Res<Sim>,
    time: Res<Time>,
    mut veg: ResMut<Vegetation>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut restarted: EventReader<crate::sim::SimRestarted>,
    mut since: Local<f32>,
) {
    let restarted = restarted.read().count() > 0;
    *since += time.delta_seconds();
    if !restarted && *since < BURN_REDRAW_S {
        return;
    }
    let now = sim.time_s() as f32;
    let line_cells = sim.fire.line_cells;
    if veg.drawn_at == Some(now) && veg.drawn_cleared == line_cells && !restarted {
        return;
    }
    *since = 0.0;
    veg.drawn_at = Some(now);
    veg.drawn_cleared = line_cells;
    let field = FireField {
        state: sim.fire.state(),
        arrival: sim.fire.arrival_times(),
        intensity: sim.fire.intensity(),
        cleared: sim.fire.cleared(),
        world: sim.scn.world,
    };
    let (rows, cols) = (sim.scn.world.fire_rows, sim.scn.world.fire_cols);
    let reached = |(r0, r1, c0, c1): (usize, usize, usize, usize)| {
        (r0.saturating_sub(1)..(r1 + 1).min(rows)).any(|r| {
            field.arrival[r * cols + c0.saturating_sub(1)..r * cols + (c1 + 1).min(cols)]
                .iter()
                .any(|&t| t != i32::MIN)
        })
    };

    let cleared = sim.fire.cleared();
    let any_cleared = |(r0, r1, c0, c1): (usize, usize, usize, usize)| {
        line_cells > 0 && (r0..r1).any(|r| cleared[r * cols + c0..r * cols + c1].iter().any(|&b| b))
    };

    for chunk in &mut veg.chunks {
        if !chunk.touched && !reached(chunk.cells) && !any_cleared(chunk.cells) {
            continue;
        }
        let mut wanted = Vec::with_capacity(chunk.plants.len());
        let mut dirty = false;
        for (plant, &drawn) in chunk.plants.iter().zip(&chunk.drawn) {
            let mut s = plant_state(&field, plant, now);
            if s == CellFire::Unburnt as u8 {
                // Thinned, not razed: the closer the plant is to the middle of
                // the cut, the likelier it is gone, decided by its own fixed
                // random number, so the same plants fall on every redraw and a
                // few survive inside the line. The edge is sharp (a narrow
                // ramp around the half-way point).
                let inside = ((field.cleared_fraction(plant.pos) - 0.45) / 0.15).clamp(0.0, 1.0);
                if plant.lag_s / 45.0 < CLEARED_SHARE * inside {
                    s = CLEARED;
                }
            }
            dirty |= s != drawn;
            wanted.push(s);
        }
        if !dirty {
            continue;
        }

        let mut colors = chunk.base.clone();
        let mut low_colors = chunk.low_base.clone();
        let mut under_colors = chunk.under_base.clone();
        let mut positions = chunk.pos_base.clone();
        let mut low_positions = chunk.low_pos_base.clone();
        let mut under_positions = chunk.under_pos_base.clone();
        chunk.touched = false;
        for ((plant, drawn), s) in chunk.plants.iter().zip(&mut chunk.drawn).zip(wanted) {
            *drawn = s;
            if s == CellFire::Unburnt as u8 {
                continue;
            }
            chunk.touched = true;
            if s == CLEARED {
                // collapse every vertex of the plant onto its first one:
                // degenerate triangles draw nothing
                for (pos, range) in [
                    (&mut positions, plant.vert_start..plant.vert_end),
                    (&mut low_positions, plant.low_start..plant.low_end),
                    (&mut under_positions, plant.under_start..plant.under_end),
                ] {
                    let (a, b) = (range.start as usize, range.end as usize);
                    if a < b {
                        let first = pos[a];
                        pos[a..b].fill(first);
                    }
                }
                continue;
            }
            // scaled by local intensity, so a grass flare and a crowning
            // conifer do not glow identically
            let k = (field.intensity(plant.pos) / 4000.0).clamp(0.2, 1.0);
            paint(&mut colors[plant.vert_start as usize..plant.vert_end as usize], s, k);
            paint(&mut low_colors[plant.low_start as usize..plant.low_end as usize], s, k);
            paint(&mut under_colors[plant.under_start as usize..plant.under_end as usize], s, k);
        }

        if let Some(mesh) = meshes.get_mut(&chunk.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
        if let Some(mesh) = meshes.get_mut(&chunk.low_mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, low_colors);
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, low_positions);
        }
        if let Some(mesh) = chunk.under.as_ref().and_then(|(m, _)| meshes.get_mut(m)) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, under_colors);
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, under_positions);
        }
    }
}

/// Real seconds between two redraws of the burning plants (see [`burn`]).
const BURN_REDRAW_S: f32 = 0.15;

/// Recolour one plant's vertices for its fire state.
fn paint(colors: &mut [[f32; 4]], state: u8, k: f32) {
    if state == CellFire::Burning as u8 {
        // Alight: pushed above 1.0 so the bloom pass catches it.
        for c in colors {
            *c = [1.0 + 2.2 * k, 0.42 + 0.25 * k, 0.06, 1.0];
        }
    } else {
        // Charcoal, keeping a trace of the original tint so a burnt
        // conifer stand still reads differently from burnt grass.
        for c in colors {
            *c = [
                0.07 + c[0] * 0.10,
                0.06 + c[1] * 0.08,
                0.06 + c[2] * 0.08,
                1.0,
            ];
        }
    }
}

/// Within this distance, and with the eye low enough over them, a chunk also
/// draws the undersides of its crowns: the kiosk camera never sees them, but
/// Mario's, close to the ground, does.
const CLOSE_M: f32 = 250.0;

/// Show each chunk's full models near the camera and its proxies beyond
/// [`Vegetation::lod_m`], with a 10 % band so a chunk on the edge does not
/// flicker while the camera drifts.
impl Vegetation {
    pub fn set_lod_m(&mut self, m: f32) {
        if self.lod_m != m {
            self.lod_m = m;
            // the next `lod` pass looks at every chunk again
            self.drawn_at = None;
        }
    }
}

pub fn lod(
    mut veg: ResMut<Vegetation>,
    camera: Query<&Transform, (With<Camera3d>, Without<crate::camera::AuxiliaryCamera>)>,
    mut visibility: Query<&mut Visibility>,
) {
    let Ok(eye) = camera.get_single().map(|t: &Transform| t.translation) else { return };
    let lod_m = veg.lod_m;
    for chunk in &mut veg.chunks {
        let (lo, hi) = chunk.bounds;
        let d = eye.distance(eye.clamp(lo, hi));
        let near = if chunk.near { d < lod_m * 1.1 } else { d < lod_m };
        // Seen from above a crown's underside is back-facing: a face tilted
        // 53° below the horizon (the shallowest of them, normal y = -0.6)
        // shows only to an eye lower than 53° over it, tan 53° ≈ 1.33. The
        // eye is measured against the chunk's nearest point, so a hillside
        // chunk (tall bounds) errs on the side of drawing.
        let q = eye.clamp(lo, hi);
        let (rise, run) = (eye.y - q.y, Vec2::new(eye.x - q.x, eye.z - q.z).length());
        let tan_max = if chunk.close { 1.5 } else { 1.2 };
        let low_angle = rise <= run * tan_max;
        let in_range = if chunk.close { d < CLOSE_M * 1.1 } else { d < CLOSE_M };
        let close = in_range && low_angle;
        if near != chunk.near {
            chunk.near = near;
            for (e, on) in [(chunk.full, near), (chunk.low, !near)] {
                if let Ok(mut v) = visibility.get_mut(e) {
                    *v = if on { Visibility::Inherited } else { Visibility::Hidden };
                }
            }
        }
        if close != chunk.close {
            chunk.close = close;
            if let Some((_, e)) = &chunk.under {
                if let Ok(mut v) = visibility.get_mut(*e) {
                    *v = if close { Visibility::Inherited } else { Visibility::Hidden };
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The distant proxies must stay a fraction of the models they stand in
    /// for, and face outward: `Builder::finish` averages face normals, and an
    /// inward crown would be lit from the wrong side.
    #[test]
    fn proxies_are_cheap_and_face_out() {
        for name in ["pine", "chestnut", "bush", "olive"] {
            let mut b = Builder::default();
            b.proxy(name, Vec3::ZERO, Vec3::splat(10.0), 0.7, [0.3, 0.4, 0.2], [0.4, 0.2, 0.1]);
            let model = crate::models::model(name).indices.len() / 3;
            let tris = b.indices.len() / 3;
            assert!(tris * 3 <= model, "{name}: {tris} proxy triangles for {model}");

            let mesh = b.finish();
            let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
            let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(nor)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else { panic!() };
            let (lo, hi) = pos.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p))));
            let (crown_lo, crown_hi) = proxy(name).crown;
            let centre = Vec3::new((lo.x + hi.x) * 0.5, (crown_lo.y + crown_hi.y) * 5.0, (lo.z + hi.z) * 0.5);
            let crown_start = if proxy(name).trunk.is_some() { 6 } else { 0 };
            for (p, n) in pos.iter().zip(nor).skip(crown_start) {
                let out = Vec3::from(*p) - centre;
                assert!(out.dot(Vec3::from(*n)) > 0.0, "{name}: normal {n:?} at {p:?} points in");
            }
        }
    }
}
