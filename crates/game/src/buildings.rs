//! Buildings, extruded from their real OSM footprints.
//!
//! Every one of the 7,629 footprints in the window is drawn as actual
//! geometry: walls following the traced outline, an overhanging eave, and a
//! hipped or flat roof depending on what the building is. That matters beyond
//! looks. The player is asked to judge which streets are defensible and where
//! a fire can run into the town, and a field of identical boxes hides exactly
//! the things that decision turns on — how tightly packed the old centre is,
//! how the ridge-line houses stand alone in the fuel, where the industrial
//! sheds are.
//!
//! Heights come from data where there is data. OSM `building:levels` is
//! present on very few of these, so the storey count baked into the population
//! file is used where a dwelling exists, and otherwise a height is inferred
//! from the building's tag and footprint area. That inference is visible and
//! wrong in individual cases; it is not used by any model, only drawn.
//!
//! Colour is per-building, hashed from the OSM id off a Ligurian palette —
//! ochre, terracotta, cream, faded rose — so the town reads as a town rather
//! than as one material. The palette is the point where this stops being
//! documentation of data and starts being art direction, and it is confined to
//! [`palette`] so it can be replaced wholesale.
//!
//! Geometry is merged per 512 m chunk, the same trade [`crate::vegetation`]
//! makes: 7,629 entities would cost more in transform propagation and draw
//! calls than the triangles do, while chunks still cull.
//!
//! Buildings respond to the fire through [`fire::StructureExposure`], never
//! through the fire mask — a house sits on a non-burnable cell and can never
//! appear in it (see `fire::exposure`). A structure the exposure model has
//! ignited flares, then goes to charcoal.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::population::Status;
use scenario::{Building, Pos, Scenario};

use crate::sim::Sim;

/// Unlit window glass — dark enough that a daylit building reads as an
/// ordinary wall with punched openings, not a lattice of black holes.
const WINDOW_GLASS: [f32; 4] = [0.06, 0.07, 0.10, 1.0];

/// A lit window after dark. Pushed above 1.0 like the fire's own emissive
/// tricks (`Damage::Alight`), so the bloom pass catches it and a lit house
/// actually reads as a small light in the dark rather than a beige square.
const WINDOW_LIT: [f32; 4] = [2.1, 1.55, 0.85, 1.0];

/// Below this fraction of `SunState::brightness`, windows switch on. Not a
/// fade: a lit window is a binary fact about a house (is anyone home, is it
/// dark out), and a threshold crossed twice a run is a cheaper and just as
/// honest a signal as a continuous glow ramp.
const NIGHT_BRIGHTNESS: f32 = 0.15;

/// Chunk edge in metres.
const CHUNK_M: f32 = 512.0;

/// Metres per storey.
const STOREY_M: f32 = 3.2;

/// How far the roof overhangs the wall. Small, but it is what gives every
/// building a shadow line under the eave instead of a flat silhouette.
const EAVE_M: f32 = 0.55;

/// Simulated seconds a structure flames before it is a ruin.
const BURN_DOWN_S: f32 = 25.0 * 60.0;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Damage {
    None = 0,
    /// Taking enough heat to be worth watching, not yet alight.
    Threatened = 1,
    Alight = 2,
    Destroyed = 3,
}

struct Structure {
    /// Households living here, for the exposure lookup.
    households: Vec<u32>,
    pos: Pos,
    vert_start: u32,
    vert_end: u32,
    /// Window quads, a contiguous sub-range of `vert_start..vert_end` — see
    /// `emit_building`. `window_start == window_end` for a shed or
    /// industrial hall, which gets none.
    window_start: u32,
    window_end: u32,
    drawn: u8,
    /// Whether this structure's windows are currently lit. Cached rather
    /// than recomputed on every read, because both the damage pass and the
    /// hover pass need to know it and only the former has the sim handy.
    lit: bool,
    /// Simulated time this structure caught, if it has.
    alight_at_s: f32,
}

struct Chunk {
    mesh: Handle<Mesh>,
    structures: Vec<Structure>,
    /// Colours as built, before any fire damage.
    base: Vec<[f32; 4]>,
}

#[derive(Resource)]
pub struct Buildings {
    chunks: Vec<Chunk>,
    /// Household id -> (chunk index, structure index), for the hover pass
    /// and the click-select pipeline: both need to go from "which house" to
    /// "which few thousand vertices" without a linear scan.
    by_household: HashMap<u32, (usize, usize)>,
}

/// Which structure, if any, the cursor is currently over — a household id,
/// same key as `Buildings::by_household`. Kept apart from `inspect::Selected`
/// because hovering and selecting are different gestures: hovering never
/// requires a click and is not undone by one landing elsewhere.
#[derive(Resource, Default)]
pub struct HoveredHousehold(pub Option<usize>);

pub fn spawn(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let scn = &sim.scenario;

    // Storeys, where the population bake knows them.
    let mut levels: HashMap<i64, u8> = HashMap::new();
    for d in &scn.population.dwellings {
        levels.insert(d.osm_id, d.levels);
    }
    // Which households live in which building, for the damage layer.
    let mut residents: HashMap<i64, Vec<u32>> = HashMap::new();
    for h in &scn.population.households {
        residents.entry(h.building).or_default().push(h.id as u32);
    }

    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.75,
        metallic: 0.0,
        // Sunbaked lime-washed plaster and glazed terracotta both carry a
        // faint sheen a matte material (the terrain's own 0.12) does not —
        // enough to pick out a highlight along a wall at low sun, not enough
        // to look wet.
        reflectance: 0.35,
        ..default()
    });

    let cols = (scn.world.width_m / CHUNK_M).ceil() as usize + 1;
    let rows = (scn.world.height_m / CHUNK_M).ceil() as usize + 1;
    let mut buckets: Vec<Vec<&Building>> = vec![Vec::new(); rows * cols];
    for b in &scn.vectors.buildings {
        let cx = (b.centroid[0] / CHUNK_M).max(0.0) as usize;
        let cy = (b.centroid[1] / CHUNK_M).max(0.0) as usize;
        if cx < cols && cy < rows {
            buckets[cy * cols + cx].push(b);
        }
    }

    let mut chunks = Vec::new();
    let mut by_household: HashMap<u32, (usize, usize)> = HashMap::new();
    let (mut count, mut tris) = (0usize, 0usize);

    for bucket in buckets {
        if bucket.is_empty() {
            continue;
        }
        let mut builder = Builder::default();
        let mut structures = Vec::new();

        for b in bucket {
            let start = builder.positions.len() as u32;
            let Some((win_start, win_end)) =
                emit_building(scn, b, levels.get(&b.id).copied(), &mut builder)
            else {
                continue;
            };
            let households = residents.get(&b.id).cloned().unwrap_or_default();
            let struct_idx = structures.len();
            for &h in &households {
                by_household.insert(h, (chunks.len(), struct_idx));
            }
            structures.push(Structure {
                households,
                pos: Pos {
                    x: b.centroid[0],
                    y: b.centroid[1],
                },
                vert_start: start,
                vert_end: builder.positions.len() as u32,
                window_start: win_start,
                window_end: win_end,
                drawn: Damage::None as u8,
                lit: false,
                alight_at_s: f32::INFINITY,
            });
            count += 1;
        }

        if structures.is_empty() {
            continue;
        }
        tris += builder.indices.len() / 3;
        let base = builder.colors.clone();
        let mesh = meshes.add(builder.finish());
        commands.spawn(PbrBundle {
            mesh: mesh.clone(),
            material: material.clone(),
            ..default()
        });
        chunks.push(Chunk {
            mesh,
            structures,
            base,
        });
    }

    info!(
        "buildings: {count} footprints in {} chunks, {:.2} M triangles",
        chunks.len(),
        tris as f32 / 1e6
    );
    commands.insert_resource(Buildings {
        chunks,
        by_household,
    });
}

/// What kind of thing this is, which sets its height, its roof and its palette.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    House,
    Apartments,
    Industrial,
    Civic,
    Shed,
}

impl Kind {
    fn of(tag: Option<&str>, area: f32) -> Kind {
        match tag.unwrap_or("yes") {
            "house" | "detached" | "residential" | "cabin" | "bungalow" | "villa" => Kind::House,
            "apartments" | "hotel" | "dormitory" => Kind::Apartments,
            "industrial" | "warehouse" | "commercial" | "retail" | "supermarket" => {
                Kind::Industrial
            }
            "church" | "chapel" | "school" | "public" | "civic" | "hospital" | "train_station" => {
                Kind::Civic
            }
            "roof" | "garage" | "garages" | "shed" | "hut" | "construction" | "ruins" => Kind::Shed,
            // The overwhelming majority are tagged `building=yes`, so the
            // footprint has to do the work: a 40 m² box is a garage, a
            // 600 m² one on this coast is a block of flats.
            _ if area < 45.0 => Kind::Shed,
            _ if area > 420.0 => Kind::Apartments,
            _ => Kind::House,
        }
    }

    fn flat_roof(self) -> bool {
        matches!(self, Kind::Industrial | Kind::Shed)
    }
}

/// Ligurian coastal palette: lime-washed ochres and pinks with terracotta
/// roofs. Chosen by eye against photographs of Spotorno and Noli, and the one
/// place in this file that is taste rather than data.
mod palette {
    pub const WALLS: [[f32; 3]; 8] = [
        [0.87, 0.75, 0.55], // ochre
        [0.85, 0.66, 0.50], // apricot
        [0.92, 0.88, 0.79], // cream
        [0.80, 0.57, 0.49], // faded rose
        [0.88, 0.82, 0.63], // pale yellow
        [0.76, 0.72, 0.66], // grey-beige
        [0.83, 0.70, 0.58], // sand
        [0.90, 0.79, 0.66], // pale ochre
    ];
    pub const ROOFS: [[f32; 3]; 4] = [
        [0.63, 0.31, 0.21],
        [0.70, 0.37, 0.24],
        [0.57, 0.29, 0.22],
        [0.66, 0.34, 0.19],
    ];
    pub const INDUSTRIAL_WALL: [f32; 3] = [0.74, 0.74, 0.72];
    pub const INDUSTRIAL_ROOF: [f32; 3] = [0.52, 0.55, 0.57];
    pub const CIVIC_WALL: [f32; 3] = [0.90, 0.89, 0.85];
}

/// Emit one building, and return the vertex range of its window quads
/// (`start == end` if it has none). `None` for footprints too degenerate to
/// draw.
fn emit_building(
    scn: &Scenario,
    b: &Building,
    baked_levels: Option<u8>,
    out: &mut Builder,
) -> Option<(u32, u32)> {
    let n = b.ring.len();
    if n < 3 {
        return None;
    }
    let area = b.area();
    if area < 6.0 {
        return None;
    }

    // OSM rings repeat the first point as the last; drop it, and drop any
    // vertex that duplicates its predecessor, or the wall quads degenerate.
    let mut ring: Vec<Pos> = Vec::with_capacity(n);
    for v in &b.ring {
        let p = Pos { x: v[0], y: v[1] };
        if ring
            .last()
            .map(|q: &Pos| (q.x - p.x).abs() < 0.05 && (q.y - p.y).abs() < 0.05)
            .unwrap_or(false)
        {
            continue;
        }
        ring.push(p);
    }
    if ring.len() > 2 {
        let (f, l) = (ring[0], *ring.last().unwrap());
        if (f.x - l.x).abs() < 0.05 && (f.y - l.y).abs() < 0.05 {
            ring.pop();
        }
    }
    if ring.len() < 3 {
        return None;
    }
    // Wind counter-clockwise so wall normals face outward.
    if signed_area(&ring) < 0.0 {
        ring.reverse();
    }

    let kind = Kind::of(b.kind.as_deref(), area);
    let h = hash01(b.id as u64, 0x1F);
    // The population bake's storey count is itself synthetic and sits at 2 for
    // 98% of dwellings, which draws the old town as a field of bungalows. It
    // is used as a floor, not as truth: a 400 m² block on this coast is four
    // or five storeys whatever the file says.
    let by_area: f32 = match area {
        a if a < 60.0 => 1.0,
        a if a < 130.0 => 2.0,
        a if a < 260.0 => 3.0,
        a if a < 500.0 => 4.0,
        _ => 5.0,
    };
    let storeys = match kind {
        Kind::Shed => 1.0,
        Kind::Industrial => 1.0,
        Kind::Civic => by_area.max(2.0),
        // A detached house stays a house however wide its footprint is.
        Kind::House => baked_levels.map(f32::from).unwrap_or(2.0).max(2.0).min(3.0),
        Kind::Apartments => {
            baked_levels
                .map(f32::from)
                .unwrap_or(0.0)
                .max(by_area)
                .max(3.0)
                + (h * 2.0).floor()
        }
    };
    let wall_h = match kind {
        // A shed's storey is not 3.2 m, and an industrial hall's is much more.
        Kind::Shed => 2.6 + h * 0.8,
        Kind::Industrial => 6.5 + h * 3.0,
        Kind::Civic => storeys * STOREY_M * 1.35,
        _ => storeys * STOREY_M * (0.95 + 0.1 * h),
    };

    // Ground: sit the base below the lowest corner and the eave above the
    // highest, so a building on a slope is cut into the hill rather than
    // floating off it on the downhill side.
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for p in &ring {
        let g = scn.terrain.height_at(*p);
        lo = lo.min(g);
        hi = hi.max(g);
    }
    let base = lo - 1.5;
    let eave = hi + wall_h;

    let centroid = centroid(&ring);
    let wall = match kind {
        Kind::Industrial => palette::INDUSTRIAL_WALL,
        Kind::Civic => palette::CIVIC_WALL,
        _ => palette::WALLS[(hash01(b.id as u64, 0x7C) * 8.0) as usize % 8],
    };
    let roof = match kind {
        Kind::Industrial | Kind::Shed => palette::INDUSTRIAL_ROOF,
        _ => palette::ROOFS[(hash01(b.id as u64, 0x3D) * 4.0) as usize % 4],
    };
    // A plinth: the darker, damper base course every masonry building on this
    // coast has. It is also what stops a wall reading as an untextured plane.
    let plinth_top = base + (eave - base).min(1.0) * 0.9 + 0.6;
    let plinth = [wall[0] * 0.78, wall[1] * 0.76, wall[2] * 0.74];

    let m = ring.len();
    for i in 0..m {
        let a = ring[i];
        let c = ring[(i + 1) % m];
        out.quad(
            [a.x, base, -a.y],
            [c.x, base, -c.y],
            [c.x, plinth_top, -c.y],
            [a.x, plinth_top, -a.y],
            plinth,
        );
        out.quad(
            [a.x, plinth_top, -a.y],
            [c.x, plinth_top, -c.y],
            [c.x, eave, -c.y],
            [a.x, eave, -a.y],
            wall,
        );
    }

    // Windows: only where someone plausibly lives or works, punched into the
    // wall just emitted above. Emitted as their own small quads (not a
    // texture) so they can be recoloured independently of the wall behind
    // them — dark glass by day, lit or dark by night depending on whether the
    // household is home. Contiguous in the builder, so the whole run is one
    // vertex range.
    let window_start = out.positions.len() as u32;
    if matches!(kind, Kind::House | Kind::Apartments | Kind::Civic) {
        let rows = (storeys.round() as usize).clamp(1, 6);
        for i in 0..m {
            let a = ring[i];
            let c = ring[(i + 1) % m];
            emit_windows(a, c, plinth_top, eave, rows, out);
        }
    }
    let window_end = out.positions.len() as u32;

    // The eave ring is the footprint pushed outward, which both casts the
    // shadow line and hides the seam where roof meets wall.
    let eaves: Vec<Pos> = offset_ring(&ring, centroid, EAVE_M);
    // Underside of the overhang, in shadow.
    let soffit = [roof[0] * 0.35, roof[1] * 0.35, roof[2] * 0.35];
    for i in 0..m {
        let a = ring[i];
        let c = ring[(i + 1) % m];
        let ea = eaves[i];
        let ec = eaves[(i + 1) % m];
        out.quad(
            [ea.x, eave, -ea.y],
            [ec.x, eave, -ec.y],
            [c.x, eave, -c.y],
            [a.x, eave, -a.y],
            soffit,
        );
    }

    if kind.flat_roof() {
        // Flat roof with a low parapet: an industrial shed silhouette.
        let top = eave + 0.5;
        for i in 0..m {
            let a = eaves[i];
            let c = eaves[(i + 1) % m];
            out.quad(
                [a.x, eave, -a.y],
                [c.x, eave, -c.y],
                [c.x, top, -c.y],
                [a.x, top, -a.y],
                [roof[0] * 0.8, roof[1] * 0.8, roof[2] * 0.8],
            );
        }
        out.fan(&eaves, top, centroid, roof);
    } else {
        // Hipped roof: the eave ring pulled in toward the centroid and lifted.
        // Not a straight skeleton — on a concave footprint the inset ring can
        // pinch — but at this scale the silhouette is right and the cost is a
        // few triangles.
        let inset = (area.sqrt() * 0.22).clamp(1.0, 4.5);
        let ridge_h = (inset * 0.75).clamp(1.2, 3.5);
        let cap: Vec<Pos> = offset_ring(&eaves, centroid, -inset);
        let ridge = eave + ridge_h;
        for i in 0..m {
            let a = eaves[i];
            let c = eaves[(i + 1) % m];
            let ca = cap[i];
            let cc = cap[(i + 1) % m];
            out.quad(
                [a.x, eave, -a.y],
                [c.x, eave, -c.y],
                [cc.x, ridge, -cc.y],
                [ca.x, ridge, -ca.y],
                roof,
            );
        }
        // Slightly lighter along the ridge, as weathered tile is.
        out.fan(
            &cap,
            ridge,
            centroid,
            [roof[0] * 1.12, roof[1] * 1.1, roof[2] * 1.08],
        );
    }
    Some((window_start, window_end))
}

/// Punch up to two windows per storey into the wall segment `a -> c`, pushed
/// a few centimetres proud of the wall plane so they do not z-fight with it.
/// Skips edges too short to hold one convincingly — a gable end on a shed-
/// sized footprint gets no window rather than one spanning the whole wall.
fn emit_windows(a: Pos, c: Pos, plinth_top: f32, eave: f32, rows: usize, out: &mut Builder) {
    let elen = ((c.x - a.x).powi(2) + (c.y - a.y).powi(2)).sqrt();
    if elen < 3.0 {
        return;
    }
    let cols: &[f32] = if elen > 7.0 { &[0.32, 0.68] } else { &[0.5] };
    let half_w = (elen * 0.09).clamp(0.35, 0.9);
    // The wall's own outward normal, from the same three corners its quad
    // was built from — reusing it keeps the window coplanar-but-proud rather
    // than needing its own (and possibly inward-facing) offset.
    let wn = normal(
        [a.x, plinth_top, -a.y],
        [c.x, plinth_top, -c.y],
        [c.x, eave, -c.y],
    );
    let push = |p: [f32; 3]| {
        [
            p[0] + wn[0] * 0.04,
            p[1] + wn[1] * 0.04,
            p[2] + wn[2] * 0.04,
        ]
    };

    let band = (eave - plinth_top) / rows as f32;
    for r in 0..rows {
        let band_lo = plinth_top + band * r as f32;
        let win_h = band * 0.5;
        let y_lo = band_lo + band * 0.25;
        let y_hi = y_lo + win_h;
        for &t in cols {
            let cx = a.x + (c.x - a.x) * t;
            let cy = a.y + (c.y - a.y) * t;
            let (tx, ty) = ((c.x - a.x) / elen, (c.y - a.y) / elen);
            let (lx, ly) = (cx - tx * half_w, cy - ty * half_w);
            let (rx, ry) = (cx + tx * half_w, cy + ty * half_w);
            out.quad(
                push([lx, y_lo, -ly]),
                push([rx, y_lo, -ry]),
                push([rx, y_hi, -ry]),
                push([lx, y_hi, -ly]),
                [WINDOW_GLASS[0], WINDOW_GLASS[1], WINDOW_GLASS[2]],
            );
        }
    }
}

fn signed_area(ring: &[Pos]) -> f32 {
    let n = ring.len();
    let mut acc = 0.0;
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        acc += a.x * b.y - b.x * a.y;
    }
    acc * 0.5
}

fn centroid(ring: &[Pos]) -> Pos {
    let n = ring.len() as f32;
    Pos {
        x: ring.iter().map(|p| p.x).sum::<f32>() / n,
        y: ring.iter().map(|p| p.y).sum::<f32>() / n,
    }
}

/// Push every vertex `d` metres away from the centroid (negative pulls in).
/// A true polygon offset would use the edge bisectors; radial offset is
/// stable on the concave rings OSM is full of, and the difference at 0.5 m on
/// a 10 m building is not visible.
fn offset_ring(ring: &[Pos], c: Pos, d: f32) -> Vec<Pos> {
    ring.iter()
        .map(|p| {
            let (vx, vy) = (p.x - c.x, p.y - c.y);
            let len = (vx * vx + vy * vy).sqrt().max(1e-3);
            // Never let an inward offset cross the centroid.
            let k = if d < 0.0 { d.max(-len * 0.75) } else { d };
            Pos {
                x: p.x + vx / len * k,
                y: p.y + vy / len * k,
            }
        })
        .collect()
}

#[derive(Default)]
struct Builder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Builder {
    /// Flat-shaded quad, wound a-b-c-d. Vertices are not shared between faces:
    /// buildings want hard edges, and sharing would smooth the corners into
    /// something inflatable.
    fn quad(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], color: [f32; 3]) {
        let n = normal(a, b, c);
        let start = self.positions.len() as u32;
        for v in [a, b, c, d] {
            self.positions.push(v);
            self.normals.push(n);
            self.colors.push([color[0], color[1], color[2], 1.0]);
        }
        self.indices
            .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    }

    /// Cap a ring at height `y` with a fan around its centroid.
    fn fan(&mut self, ring: &[Pos], y: f32, c: Pos, color: [f32; 3]) {
        let start = self.positions.len() as u32;
        self.positions.push([c.x, y, -c.y]);
        self.normals.push([0.0, 1.0, 0.0]);
        self.colors.push([color[0], color[1], color[2], 1.0]);
        for p in ring {
            self.positions.push([p.x, y, -p.y]);
            self.normals.push([0.0, 1.0, 0.0]);
            self.colors.push([color[0], color[1], color[2], 1.0]);
        }
        let n = ring.len() as u32;
        for i in 0..n {
            self.indices
                .extend_from_slice(&[start, start + 1 + i, start + 1 + (i + 1) % n]);
        }
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

fn normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
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

fn hash01(id: u64, salt: u64) -> f32 {
    let mut h = id.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h >> 40) as f32 / (1u32 << 24) as f32
}

/// Recolour structures the fire has reached.
///
/// Threat comes from two places, and both are proximity models rather than the
/// fire mask: the household exposure model for anything with residents, and
/// the agent threat field for the rest. Only chunks containing a changed
/// structure are re-uploaded.
/// Forget what the fire did to the town.
///
/// `alight_at_s` is the one piece of state here that is *not* recomputed from
/// the sim each frame — it is a latch, deliberately, so a structure keeps
/// burning down after the front has moved on. That makes it also the one piece
/// a restart has to clear by hand, or the new run opens with the old run's
/// charred buildings still standing in it.
pub fn reset(
    mut restarted: EventReader<crate::sim::SimRestarted>,
    mut buildings: ResMut<Buildings>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if restarted.is_empty() {
        return;
    }
    restarted.clear();
    for chunk in &mut buildings.chunks {
        for s in &mut chunk.structures {
            s.alight_at_s = f32::INFINITY;
            s.drawn = Damage::None as u8;
            s.lit = false;
        }
        if let Some(mesh) = meshes.get_mut(&chunk.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, chunk.base.clone());
        }
    }
}

impl Buildings {
    /// Damage state of the structure a household lives in, for the inspector.
    /// `None` for a household whose building was too small or degenerate to
    /// draw (`emit_building` returned `None` for it).
    pub fn status_of(&self, household_id: usize) -> Option<&'static str> {
        let id = household_id as u32;
        for chunk in &self.chunks {
            for s in &chunk.structures {
                if s.households.contains(&id) {
                    return Some(match s.drawn {
                        x if x == Damage::Threatened as u8 => "threatened",
                        x if x == Damage::Alight as u8 => "alight",
                        x if x == Damage::Destroyed as u8 => "destroyed",
                        _ => "undamaged",
                    });
                }
            }
        }
        None
    }
}

/// Recompute one structure's colours from scratch: `base` plus whatever
/// `s.drawn`/`s.lit` currently say. Shared by the damage pass (which decides
/// those two fields) and the hover pass (which only ever replays them) so
/// the two can never disagree about what an un-hovered, undamaged building
/// looks like.
fn recolor_structure(colors: &mut [[f32; 4]], base: &[[f32; 4]], s: &Structure) {
    let range = s.vert_start as usize..s.vert_end as usize;
    match s.drawn {
        x if x == Damage::Threatened as u8 => {
            // Lit by the fire rather than damaged by it: warm, and just
            // enough to pick the building out of the street.
            for i in range {
                let c = base[i];
                colors[i] = [(c[0] * 1.15 + 0.10).min(1.4), c[1] * 0.95, c[2] * 0.80, 1.0];
            }
        }
        x if x == Damage::Alight as u8 => {
            // Above 1.0 so the bloom pass catches it, like the vegetation
            // flare.
            for i in range {
                colors[i] = [2.4, 0.75, 0.14, 1.0];
            }
        }
        x if x == Damage::Destroyed as u8 => {
            for i in range {
                let c = base[i];
                colors[i] = [
                    0.10 + c[0] * 0.08,
                    0.09 + c[1] * 0.07,
                    0.09 + c[2] * 0.07,
                    1.0,
                ];
            }
        }
        _ => {
            for i in range {
                colors[i] = base[i];
            }
            if s.lit {
                for i in (s.window_start as usize)..(s.window_end as usize) {
                    colors[i] = WINDOW_LIT;
                }
            }
        }
    }
}

/// A temporary brightening laid on top of whatever `recolor_structure` just
/// computed — the hover feedback that used to be a floating beacon.
fn hover_boost(colors: &mut [[f32; 4]], s: &Structure) {
    for c in &mut colors[s.vert_start as usize..s.vert_end as usize] {
        *c = [
            (c[0] * 1.35 + 0.18).min(2.2),
            (c[1] * 1.30 + 0.15).min(2.0),
            (c[2] * 1.25 + 0.12).min(2.0),
            c[3],
        ];
    }
}

pub fn damage(
    sim: Res<Sim>,
    sun: Res<crate::sky::SunState>,
    hovered: Res<HoveredHousehold>,
    mut buildings: ResMut<Buildings>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !sim.is_changed() {
        return;
    }
    let exposure = sim.fire.exposure();
    let threat = sim.fire.threat();
    let now = sim.time_s() as f32;
    let night = sun.brightness < NIGHT_BRIGHTNESS;

    let Buildings {
        chunks,
        by_household,
    } = &mut *buildings;
    let hovered_at = hovered
        .0
        .and_then(|h| by_household.get(&(h as u32)).copied());

    for (ci, chunk) in chunks.iter_mut().enumerate() {
        let mut dirty = false;
        for s in &mut chunk.structures {
            let (mut alight, mut load) = (false, 0.0f32);
            let mut occupied = false;
            for &h in &s.households {
                let Some(hh) = sim.agents.households.get(h as usize) else {
                    continue;
                };
                let f = exposure.get(h as usize);
                alight |= f.alight;
                load = load.max(f.radiant + f.ember);
                occupied |= !matches!(hh.status, Status::Evacuating | Status::Evacuated);
            }
            if s.households.is_empty() {
                // No residents, so no exposure record: fall back to how
                // survivable it is to stand there.
                load = threat.at(s.pos);
            }
            if alight && s.alight_at_s.is_infinite() {
                s.alight_at_s = now;
            }
            let want = if s.alight_at_s.is_finite() {
                if now - s.alight_at_s > BURN_DOWN_S {
                    Damage::Destroyed
                } else {
                    Damage::Alight
                }
            } else if load > 0.08 {
                Damage::Threatened
            } else {
                Damage::None
            } as u8;
            if want != s.drawn {
                s.drawn = want;
                dirty = true;
            }
            // Windows only matter while the structure reads as undamaged —
            // threatened/alight/destroyed already overwrite the whole range,
            // window quads included (finding #14's lesson generalises: don't
            // maintain two sources of truth for the same vertices).
            let lit = want == Damage::None as u8 && night && occupied;
            if lit != s.lit {
                s.lit = lit;
                dirty = true;
            }
        }
        if !dirty {
            continue;
        }

        let mut colors = chunk.base.clone();
        for s in &chunk.structures {
            recolor_structure(&mut colors, &chunk.base, s);
        }
        if let Some((hci, hsi)) = hovered_at {
            if hci == ci {
                hover_boost(&mut colors, &chunk.structures[hsi]);
            }
        }
        if let Some(mesh) = meshes.get_mut(&chunk.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        }
    }
}

/// Screen-space hover test against every occupiable household, the same
/// technique `inspect::pick_click` uses for the click itself — projecting a
/// few hundred candidate points is cheap enough to repeat every frame, and
/// it is what makes the hover zoom-invariant. Only rewrites the (at most
/// two) chunks whose highlight actually changed.
pub fn hover(
    ui_focus: Res<crate::ui::UiFocus>,
    tool: Res<crate::ignition_edit::IgnitionTool>,
    order: Res<crate::command::OrderTool>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    sim: Res<Sim>,
    mut hovered: ResMut<HoveredHousehold>,
    buildings: Res<Buildings>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let armed = tool.mode != crate::ignition_edit::EditMode::Off || order.is_armed();
    let mut new_hover = None;
    if !ui_focus.0 && !armed {
        if let (Ok(window), Ok((camera, cam_tf))) = (windows.get_single(), camera.get_single()) {
            if let Some(cursor) = window.cursor_position() {
                let mut best: Option<(f32, usize)> = None;
                for h in &sim.agents.households {
                    if h.status == Status::Evacuated {
                        continue;
                    }
                    let ground = sim.scenario.terrain.height_at(h.home);
                    let world = crate::frame::to_bevy(h.home, ground + 4.0);
                    if let Some(screen) = camera.world_to_viewport(cam_tf, world) {
                        let d = screen.distance(cursor);
                        if d < crate::inspect::PICK_PX && best.map_or(true, |(bd, _)| d < bd) {
                            best = Some((d, h.id));
                        }
                    }
                }
                new_hover = best.map(|(_, id)| id);
            }
        }
    }

    if new_hover == hovered.0 {
        return;
    }
    let old = hovered.0;
    hovered.0 = new_hover;

    for id in [old, new_hover].into_iter().flatten() {
        let Some(&(ci, si)) = buildings.by_household.get(&(id as u32)) else {
            continue;
        };
        let chunk = &buildings.chunks[ci];
        let mut colors = chunk.base.clone();
        for s in &chunk.structures {
            recolor_structure(&mut colors, &chunk.base, s);
        }
        if hovered.0 == Some(id) {
            hover_boost(&mut colors, &chunk.structures[si]);
        }
        if let Some(mesh) = meshes.get_mut(&chunk.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        }
    }
}
