//! Drawing the suppression units, their orders, and what their work left behind.
//!
//! Three layers, and they answer three different questions the commander asks
//! constantly:
//!
//! - **the units** — where is my engine *now*, and is it working or driving?
//! - **the orders** — what did I tell it to do, and where? A ring at the task
//!   point and, for a hand line, the alignment itself with the cut part drawn
//!   solid and the rest ghosted, so "how far have they got" is a glance.
//! - **the work** — cut line and wetted ground. Without this the suppression
//!   model is invisible: water and fuel removal only ever show up as fire that
//!   *didn't* spread, and a player cannot see a counterfactual.
//!
//! Everything is drawn as a map symbol rather than to scale, for the reason
//! [`crate::people`] spells out: at command altitude a fire engine is a pixel.
//! Same family, same exaggeration, and the [`SYMBOL_SCALE`] knob is deliberately
//! larger than the civilian one — a unit the player has to click on and re-task
//! has to be findable, and there are eight of them against 1,577 people.
//!
//! The one thing worth knowing before editing: the work overlay sits at
//! [`crate::ignition_edit::RING_LIFT_M`] above the ground like every other
//! symbol here, *not* on it. Vegetation on this map is 5–15 m of real plants, so
//! a cut line painted on the terrain is rendered perfectly and seen never.

use abm::suppression::{Task, UnitKind, UnitState};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::{Cell, Pos, Scenario};

use crate::frame;
use crate::retro;
use crate::retro::RetroMaterial;
use crate::ignition_edit::{ring_mesh, RING_LIFT_M};
use crate::sim::Sim;

/// How far above life size units are drawn.
pub(crate) const SYMBOL_SCALE: f32 = 5.0;

pub(crate) fn symbol_scale(vr: bool) -> f32 {
    SYMBOL_SCALE * if vr { 1.45 } else { 1.0 }
}

/// Altitude an air tanker is drawn at, metres above the ground beneath it. A
/// Canadair on a drop run is at 30-45 m; drawn there it is lost in the canopy
/// and the smoke, so it flies as a symbol well above both.
pub(crate) const AIR_ALTITUDE_M: f32 = 220.0;

/// Radius of the ring marking where a unit was ordered to work.
const ORDER_RING_M: f32 = 45.0;

#[derive(Component)]
pub struct UnitView {
    pub id: usize,
}

/// A ring or ribbon belonging to a unit's current order. Rebuilt wholesale
/// whenever the orders change, which is rare and cheap.
#[derive(Component)]
pub struct OrderMarker;

/// The cut-line and wetted-ground overlay.
#[derive(Component)]
pub struct WorkOverlay;

#[derive(Resource)]
pub struct UnitAssets {
    order_ring: Handle<RetroMaterial>,
    line_cut: Handle<RetroMaterial>,
    line_todo: Handle<RetroMaterial>,
    cleared: Handle<RetroMaterial>,
    wetted: Handle<RetroMaterial>,
    /// Suppression generation the markers were built for.
    orders_gen: u64,
    /// Fire generation the work overlay was built for, plus the counts it was
    /// built from: the overlay only changes when a cell is newly cut or newly
    /// wet, which is far rarer than the fire changing.
    work_gen: u64,
    cleared_count: usize,
}

pub fn colour(kind: UnitKind, state: UnitState) -> Color {
    match state {
        // A lost unit is the one outcome the player must not miss.
        UnitState::Lost => Color::srgb(0.10, 0.10, 0.10),
        UnitState::Withdrawing => Color::srgb(1.00, 0.35, 0.30),
        _ => match kind {
            // Italian fire service red, and the yellow-green of an AIB squad's
            // kit: these are the colours the real things are.
            UnitKind::Engine => Color::srgb(0.85, 0.13, 0.12),
            UnitKind::HandCrew => Color::srgb(0.95, 0.80, 0.15),
            UnitKind::AirTanker => Color::srgb(0.95, 0.95, 0.98),
        },
    }
}

pub fn setup(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RetroMaterial>>,
) {
    let engine = meshes.add(crate::models::mesh("fire_engine"));
    let crew = meshes.add(crate::models::mesh("firefighter"));
    let tanker = meshes.add(tanker_mesh());

    let vr = sim.scenario.vr_palette().is_some();
    let symbol_scale = symbol_scale(vr);
    let mut mat = |c: Color, emissive: f32| {
        materials.add(retro::material(StandardMaterial {
            base_color: c,
            // Strongly self-lit: these have to be findable in smoke shadow on a
            // north-facing slope, which is precisely where the work is.
            emissive: c.to_linear() * emissive,
            perceptual_roughness: 0.5,
            unlit: vr,
            ..default()
        }, vr))
    };

    for u in &sim.crews.units {
        let (mesh, scale) = match u.kind {
            UnitKind::Engine => (engine.clone(), symbol_scale),
            UnitKind::HandCrew => (crew.clone(), symbol_scale),
            UnitKind::AirTanker => (tanker.clone(), symbol_scale * 1.6),
        };
        let ground = sim.scenario.terrain.height_at(u.pos);
        commands.spawn((
                MaterialMeshBundle::<RetroMaterial> {
                mesh,
                material: mat(colour(u.kind, u.state), 1.2),
                transform: Transform::from_translation(frame::to_bevy(u.pos, ground))
                    .with_scale(Vec3::splat(scale)),
                // Air support is not on the map until it is actually here.
                visibility: if u.on_scene() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
                ..default()
            },
            UnitView { id: u.id },
        ));
    }
    info!("suppression layer: {} unit symbols", sim.crews.units.len());

    let mut unlit = |r: f32, g: f32, b: f32, a: f32| {
        materials.add(retro::material(StandardMaterial {
            base_color: Color::srgba(r, g, b, a),
            emissive: LinearRgba::rgb(r * 1.4, g * 1.4, b * 1.4),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        }, vr))
    };
    commands.insert_resource(UnitAssets {
        order_ring: unlit(0.30, 0.95, 1.00, 0.85),
        line_cut: unlit(0.55, 0.42, 0.30, 0.95),
        line_todo: unlit(0.30, 0.95, 1.00, 0.35),
        // Cut line reads as bare earth; wetted ground as a cool wash. The wash
        // is deliberately faint: a busy pair of aircraft saturates a hundred
        // cells inside an hour, and at any higher alpha that is a lake painted
        // over the hillside rather than an indication of where the water went.
        cleared: unlit(0.42, 0.34, 0.26, 0.85),
        wetted: unlit(0.25, 0.60, 1.00, 0.20),
        orders_gen: u64::MAX,
        work_gen: u64::MAX,
        cleared_count: usize::MAX,
    });
}

/// A restart rebuilds the roster, so every order marker and every metre of
/// overlay belongs to a run that no longer exists.
///
/// The unit *symbols* are not touched: unit ids are stable across a rebuild
/// (`Suppression::new` is deterministic), so they are in the same family as the
/// person entities — keyed by id, and the same roster either way. What has to go
/// is everything keyed by *state*: the markers, and the caches that would
/// otherwise decide the overlay is still current.
pub fn reset(
    mut commands: Commands,
    mut restarted: EventReader<crate::sim::SimRestarted>,
    mut assets: ResMut<UnitAssets>,
    markers: Query<Entity, With<OrderMarker>>,
    overlay: Query<Entity, With<WorkOverlay>>,
) {
    if restarted.is_empty() {
        return;
    }
    restarted.clear();
    for e in markers.iter().chain(overlay.iter()) {
        commands.entity(e).despawn();
    }
    assets.orders_gen = u64::MAX;
    assets.work_gen = u64::MAX;
    assets.cleared_count = usize::MAX;
}

pub fn update_units(
    sim: Res<Sim>,
    mut materials: ResMut<Assets<RetroMaterial>>,
    mut query: Query<(
        &UnitView,
        &mut Transform,
        &mut Visibility,
        &Handle<RetroMaterial>,
    )>,
) {
    if !sim.is_changed() {
        return;
    }
    for (view, mut tf, mut vis, mat) in &mut query {
        let Some(u) = sim.crews.units.get(view.id) else {
            continue;
        };
        let want = if u.on_scene() || u.state == UnitState::Lost {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if want == Visibility::Hidden {
            continue;
        }

        let ground = sim.scenario.terrain.height_at(u.pos);
        let lift = if u.kind.is_air() {
            AIR_ALTITUDE_M
        } else {
            0.05
        };
        tf.translation = frame::to_bevy(u.pos, ground + lift);
        // World bearing to Bevy yaw: north is -Z and the meshes' long axis is
        // +Z, so a heading of zero (due east) is a quarter turn. Same conversion
        // as the civilian vehicles, for the same reason.
        let forward_offset = if u.kind.is_air() {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        };
        tf.rotation = Quat::from_rotation_y(u.heading + forward_offset);

        // One material per entity here rather than a shared palette: there are
        // eight of these, and mutating the material in place is what makes a
        // unit visibly change state without a swap table.
        if let Some(m) = materials.get_mut(mat) {
            let c = colour(u.kind, u.state);
            if m.base.base_color != c {
                m.base.base_color = c;
                m.base.emissive = c.to_linear() * 1.2;
            }
        }
    }
}

/// Rebuild the order markers when the orders change.
pub fn sync_orders(
    mut commands: Commands,
    sim: Res<Sim>,
    mut assets: ResMut<UnitAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    existing: Query<Entity, With<OrderMarker>>,
) {
    // `Suppression::generation` bumps every step, so gate on the orders
    // themselves: a marker set that is rebuilt 30 times a second would be a
    // mesh allocation per frame for no visible change.
    let fingerprint = orders_fingerprint(&sim);
    if assets.orders_gen == fingerprint {
        return;
    }
    assets.orders_gen = fingerprint;
    for e in &existing {
        commands.entity(e).despawn();
    }

    for u in &sim.crews.units {
        if !u.assignable() {
            continue;
        }
        match u.task {
            Task::Hold | Task::Return => {}
            Task::Attack { at } | Task::Drop { at } => {
                commands.spawn((
                    MaterialMeshBundle::<RetroMaterial> {
                        mesh: meshes.add(ring_mesh(&sim.scenario, at, ORDER_RING_M)),
                        material: assets.order_ring.clone(),
                        ..default()
                    },
                    OrderMarker,
                ));
            }
            Task::Line { from, to } => {
                let total = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
                let done = (u.line_done_m / total.max(1e-3)).clamp(0.0, 1.0);
                let head = lerp(from, to, done);
                // Two ribbons: what has been cut, and what has not. The join
                // moves as the crew works, which is the only progress readout
                // that does not need a number.
                if done > 0.001 {
                    commands.spawn((
                        MaterialMeshBundle::<RetroMaterial> {
                            mesh: meshes.add(ribbon(&sim.scenario, from, head, 8.0)),
                            material: assets.line_cut.clone(),
                            ..default()
                        },
                        OrderMarker,
                    ));
                }
                if done < 0.999 {
                    commands.spawn((
                        MaterialMeshBundle::<RetroMaterial> {
                            mesh: meshes.add(ribbon(&sim.scenario, head, to, 4.0)),
                            material: assets.line_todo.clone(),
                            ..default()
                        },
                        OrderMarker,
                    ));
                }
            }
        }
    }
}

/// A cheap hash of every order, so the markers rebuild when an order changes
/// and not when a unit merely moved. Line progress is quantised to 10 m: the
/// ribbon does not need to be redrawn for every centimetre of a 120 m/h crew.
fn orders_fingerprint(sim: &Sim) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100_0000_01b3);
    };
    for u in &sim.crews.units {
        mix(u.id as u64);
        mix(u.state as u64);
        match u.task {
            Task::Hold => mix(1),
            Task::Return => mix(2),
            Task::Attack { at } => {
                mix(3);
                mix(at.x as u64);
                mix(at.y as u64);
            }
            Task::Drop { at } => {
                mix(4);
                mix(at.x as u64);
                mix(at.y as u64);
            }
            Task::Line { from, to } => {
                mix(5);
                mix(from.x as u64);
                mix(from.y as u64);
                mix(to.x as u64);
                mix(to.y as u64);
                mix((u.line_done_m / 10.0) as u64);
            }
        }
    }
    h
}

/// Rebuild the cut-line and wetted-ground overlay.
///
/// Two meshes, one draw each: at 20 m a cell is four vertices, and even a
/// heavily worked incident is a few hundred cells. Rebuilt only when the *set*
/// changes — cleared cells are monotonic within a run, and the wetted set is
/// thresholded, so this fires a handful of times a minute rather than per frame.
pub fn update_work_overlay(
    mut commands: Commands,
    sim: Res<Sim>,
    mut assets: ResMut<UnitAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    existing: Query<Entity, With<WorkOverlay>>,
) {
    let cleared_now = sim.fire.cleared().iter().filter(|c| **c).count();
    // The wetted set changes continuously as moisture decays, so it is sampled
    // on the sim generation rather than on a count; the cleared set is what
    // makes an overlay appear at all.
    let gen = sim.generation / WET_REFRESH_GENERATIONS;
    if assets.cleared_count == cleared_now && assets.work_gen == gen {
        return;
    }
    assets.cleared_count = cleared_now;
    assets.work_gen = gen;
    for e in &existing {
        commands.entity(e).despawn();
    }
    if cleared_now == 0 && !sim.fire.added_moisture().iter().any(|m| *m >= WET_SHOW_PTS) {
        return;
    }

    let w = sim.scenario.world;
    let mut cleared: Vec<Cell> = Vec::new();
    let mut wet: Vec<Cell> = Vec::new();
    for (i, flag) in sim.fire.cleared().iter().enumerate() {
        let c = Cell {
            row: i / w.fire_cols,
            col: i % w.fire_cols,
        };
        if *flag {
            cleared.push(c);
        } else if sim.fire.added_moisture()[i] >= WET_SHOW_PTS {
            wet.push(c);
        }
    }

    for (cells, material) in [
        (cleared, assets.cleared.clone()),
        (wet, assets.wetted.clone()),
    ] {
        if cells.is_empty() {
            continue;
        }
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(cell_patches(&sim.scenario, &cells)),
                material,
                ..default()
            },
            WorkOverlay,
        ));
    }
}

/// Added moisture at which a cell is worth showing as wet, percentage points.
/// A tenth of the core's moisture of extinction: enough to see where a drop
/// landed, high enough that the 1%/minute decay makes it fade rather than
/// linger as a stain for the rest of the incident.
const WET_SHOW_PTS: f32 = 3.0;
/// Rebuild the wet overlay every N sim generations. At 2 s steps this is a
/// refresh every ~20 simulated seconds, which is smooth for something that
/// decays with a 69-minute half life.
const WET_REFRESH_GENERATIONS: u64 = 10;

/// One flat quad per cell, draped at each corner.
fn cell_patches(scn: &Scenario, cells: &[Cell]) -> Mesh {
    let h = scn.world.cellsize * 0.5;
    let mut positions = Vec::with_capacity(cells.len() * 4);
    let mut normals = Vec::with_capacity(cells.len() * 4);
    let mut uvs = Vec::with_capacity(cells.len() * 4);
    let mut indices = Vec::with_capacity(cells.len() * 6);

    for (n, c) in cells.iter().enumerate() {
        let p = scn.world.centre_of(*c);
        let base = (n * 4) as u32;
        for (dx, dy) in [(-h, -h), (h, -h), (-h, h), (h, h)] {
            let q = Pos {
                x: p.x + dx,
                y: p.y + dy,
            };
            positions.push([q.x, scn.terrain.height_at(q) + RING_LIFT_M, -q.y]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([(dx > 0.0) as u32 as f32, (dy > 0.0) as u32 as f32]);
        }
        // Wound anticlockwise seen from above. The other order faces the ground
        // and is invisible rather than inside-out -- the trap the road ribbons
        // and the ignition rings both hit.
        indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// A flat ribbon of `width_m` from `a` to `b`, draped and resampled.
///
/// Resampled at the render posting for the reason the roads are: a straight run
/// over a ridge drawn as one long quad drapes as a chord *through* the hill and
/// disappears into it. Vertices only exist where you put them.
pub(crate) fn ribbon(scn: &Scenario, a: Pos, b: Pos, width_m: f32) -> Mesh {
    const RESAMPLE_M: f32 = 5.0;
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = (dx * dx + dy * dy).sqrt();
    let steps = (len / RESAMPLE_M).ceil().max(1.0) as usize;
    let (nx, ny) = if len > 1e-3 {
        (-dy / len * width_m * 0.5, dx / len * width_m * 0.5)
    } else {
        (width_m * 0.5, 0.0)
    };

    let mut positions = Vec::with_capacity((steps + 1) * 2);
    let mut normals = Vec::with_capacity((steps + 1) * 2);
    let mut uvs = Vec::with_capacity((steps + 1) * 2);
    let mut indices = Vec::with_capacity(steps * 6);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let c = lerp(a, b, t);
        for s in [-1.0f32, 1.0] {
            let p = Pos {
                x: c.x + nx * s,
                y: c.y + ny * s,
            };
            positions.push([p.x, scn.terrain.height_at(p) + RING_LIFT_M, -p.y]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([(s > 0.0) as u32 as f32, t]);
        }
    }
    for i in 0..steps as u32 {
        let k = i * 2;
        indices.extend_from_slice(&[k, k + 2, k + 1, k + 1, k + 2, k + 3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Fuselage plus wing, as one mesh: an unmistakable plan-view aircraft at any
/// zoom, and cheaper than loading a model for eight pixels of screen.
fn tanker_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Two crossed slabs, drawn top and bottom so it reads from above and below.
    let mut slab = |hx: f32, hz: f32, y: f32| {
        for (sy, ny) in [(y, 1.0f32), (-y, -1.0f32)] {
            let base = positions.len() as u32;
            for (x, z) in [(-hx, -hz), (hx, -hz), (-hx, hz), (hx, hz)] {
                positions.push([x, sy, z]);
                normals.push([0.0, ny, 0.0]);
                uvs.push([0.0, 0.0]);
            }
            if ny > 0.0 {
                indices.extend_from_slice(&[
                    base,
                    base + 2,
                    base + 1,
                    base + 1,
                    base + 2,
                    base + 3,
                ]);
            } else {
                indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base + 1,
                    base + 3,
                    base + 2,
                ]);
            }
        }
    };
    slab(0.6, 3.6, 0.25); // fuselage, nose along +Z
    slab(3.2, 0.7, 0.20); // wing

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn lerp(a: Pos, b: Pos, t: f32) -> Pos {
    let t = t.clamp(0.0, 1.0);
    Pos {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}
