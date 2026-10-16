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
//! [`crate::rings::RING_LIFT_M`] above the ground like every other
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
use crate::rings::{ring_mesh, tube_transform, wall_mesh, TubeSpin, RING_LIFT_M};
use crate::sim::Sim;

/// How far above life size units are drawn.
pub(crate) const SYMBOL_SCALE: f32 = crate::visual_scale::UNIT;

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

/// One drop of a fire engine's jet.
#[derive(Component)]
pub struct Droplet {
    unit: usize,
    jet: usize,
    k: usize,
}

/// Jets an engine throws at once (one per cell it is wetting), and drops per jet.
const JETS: usize = 4;
const DROPS: usize = 12;
/// Height of the nozzle above the ground, and of the arc's crown over the chord, metres.
const NOZZLE_M: f32 = 9.0;
const ARC_BASE_M: f32 = 8.0;
const ARC_PER_M: f32 = 0.18;
/// Drop size in flight and when it bursts on the fuel, metres.
const DROP_M: f32 = 2.6;
const SPLASH_M: f32 = 9.0;
/// Seconds a drop takes from nozzle to fuel.
const FLIGHT_S: f32 = 0.9;

/// The cut-line overlay (nothing is drawn on the wetted ground any more).
#[derive(Component)]
pub struct WorkOverlay;

#[derive(Resource)]
pub struct UnitAssets {
    order_ring: Handle<RetroMaterial>,
    line_cut: Handle<RetroMaterial>,
    line_todo: Handle<RetroMaterial>,
    /// Suppression generation the markers were built for.
    orders_gen: u64,
    /// Fire generation the work overlay was built for, plus the counts it was
    /// built from: the overlay only changes when a cell is newly cut or newly
    /// wet, which is far rarer than the fire changing.
    work_gen: u64,
    /// Wet cells seen at the last scan (so a cell is announced once, when it
    /// first gets wet) and the ones on show now, with the simulated time they appeared.
    wet_seen: std::collections::HashSet<usize>,
    wet_shown: Vec<(usize, f32)>,
    /// What the overlay was last built from, to rebuild only on a change.
    wet_built: usize,
}


/// The volunteers' (AIB) yellow-green kit; the fire brigade's engines are red.
pub const VOLUNTEERS: Color = Color::srgb(0.80, 0.85, 0.20);

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
    // Blender models (scripts/build_unit_models.py): the engine, a firefighter
    // on foot (each person of a team is a unit of its own) and the air tanker.
    let engine = meshes.add(crate::models::unit_mesh("engine"));
    let crew = meshes.add(crate::models::unit_mesh("firefighter"));
    let tanker = meshes.add(crate::models::unit_mesh("tanker"));

    let vr = false;
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
        let ground = sim.scn.terrain.height_at(u.pos);
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

    // The jets: a pool of droplets per engine, moved every frame by `animate_spray`.
    let drop_mesh = meshes.add(Sphere::new(1.0));
    let drop_mat = materials.add(retro::material(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::new(0.55, 1.3, 2.6, 0.75)),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }, vr));
    for u in sim.crews.units.iter().filter(|u| u.kind == UnitKind::Engine) {
        for jet in 0..JETS {
            for k in 0..DROPS {
                commands.spawn((
                    MaterialMeshBundle::<RetroMaterial> {
                        mesh: drop_mesh.clone(),
                        material: drop_mat.clone(),
                        visibility: Visibility::Hidden,
                        ..default()
                    },
                    Droplet { unit: u.id, jet, k },
                ));
            }
        }
    }

    let order_ring = hdr_blue(&mut materials, vr, 0.95);
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
        order_ring,
        // Lines to cut: dashed yellow ahead of the crew, solid yellow once cut (user, 2026-10-15).
        line_cut: unlit(1.0, 0.85, 0.10, 0.95),
        line_todo: unlit(1.0, 0.85, 0.10, 0.95),
        // Cut line reads as bare earth; wetted ground as a cool wash. The wash
        // is deliberately faint: a busy pair of aircraft saturates a hundred
        // cells inside an hour, and at any higher alpha that is a lake painted
        // over the hillside rather than an indication of where the water went.
        orders_gen: u64::MAX,
        work_gen: u64::MAX,
        wet_seen: Default::default(),
        wet_shown: Vec::new(),
        wet_built: 0,
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
    assets.wet_seen.clear();
    assets.wet_shown.clear();
    assets.wet_built = 0;
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
        // people sitting in the engine are the engine's symbol
        let want = if (u.on_scene() && !u.aboard) || u.state == UnitState::Lost {
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

        let ground = sim.scn.terrain.height_at(u.pos);
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
            let volunteers = sim.teams.iter().any(|t| t.unit == view.id && t.volunteers);
            let c = match colour(u.kind, u.state) {
                c if volunteers && matches!(u.state, UnitState::Staged | UnitState::Moving | UnitState::Working | UnitState::Refilling) => {
                    let _ = c;
                    VOLUNTEERS
                }
                c => c,
            };
            if m.base.base_color != c {
                m.base.base_color = c;
                m.base.emissive = c.to_linear() * 1.2;
            }
        }
    }
}

/// Move the messenger patrols: on the road, at the door they knock on, hidden
/// while parked at their station; dark once caught by the fire.
/// UI-in-3D blue, overbright: unlit output goes through the tone mapper, which
/// greys anything at or under 1.0 against the terrain, so the glow is >1.
fn hdr_blue(materials: &mut Assets<RetroMaterial>, vr: bool, a: f32) -> Handle<RetroMaterial> {
    materials.add(retro::material(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::new(0.15, 1.2, 3.0, a)),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    }, vr))
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
            // where a team is sent is shown by its flag (`kiosk::pins`)
            Task::Hold | Task::Return | Task::Attack { .. } => {}
            Task::Drop { at } => {
                commands.spawn((
                    MaterialMeshBundle::<RetroMaterial> {
                        mesh: meshes.add(ring_mesh(&sim.scn, at, ORDER_RING_M)),
                        material: assets.order_ring.clone(),
                        ..default()
                    },
                    OrderMarker,
                ));
                commands.spawn((
                    MaterialMeshBundle::<RetroMaterial> {
                        mesh: meshes.add(wall_mesh(&sim.scn, at, ORDER_RING_M)),
                        material: assets.order_ring.clone(),
                        transform: tube_transform(at),
                        ..default()
                    },
                    OrderMarker,
                    TubeSpin,
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
                            mesh: meshes.add(ribbon(&sim.scn, from, head, 8.0)),
                            material: assets.line_cut.clone(),
                            ..default()
                        },
                        OrderMarker,
                    ));
                }
                if done < 0.999 {
                    commands.spawn((
                        MaterialMeshBundle::<RetroMaterial> {
                            mesh: meshes.add(dashed_ribbon(&sim.scn, head, to, 4.0)),
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

/// Keep the list of cells the engines have just wetted. They are no longer
/// painted blue (user, 2026-10-16): [`animate_spray`] aims a jet of water at them.
pub fn update_work_overlay(sim: Res<Sim>, mut assets: ResMut<UnitAssets>) {
    // Simulated time, so a jet stays up while the world is paused.
    let now = sim.time_s() as f32;
    // Scan for newly wet cells on the sim generation (the moisture changes
    // slowly); each one is shown for WET_SHOW_S of real time, then dropped.
    let gen = sim.generation;
    let scan = assets.work_gen != gen;
    if scan {
        let moist = sim.fire.added_moisture();
        let mut seen = std::collections::HashSet::new();
        for (i, flag) in sim.fire.cleared().iter().enumerate() {
            if !*flag && moist[i] >= WET_SHOW_PTS {
                seen.insert(i);
                if !assets.wet_seen.contains(&i) {
                    assets.wet_shown.push((i, now));
                }
            }
        }
        assets.wet_seen = seen;
    }
    assets.work_gen = gen;
    assets.wet_shown.retain(|(_, t0)| now - *t0 < WET_SHOW_S);
}

/// How long the water an engine laid stays drawn, in simulated seconds.
const WET_SHOW_S: f32 = 600.0;
/// Added moisture at which a cell is worth showing as wet, percentage points.
/// A tenth of the core's moisture of extinction: enough to see where a drop
/// landed, high enough that the 1%/minute decay makes it fade rather than
/// linger as a stain for the rest of the incident.
const WET_SHOW_PTS: f32 = 1.5;

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
    ribbon_pieces(scn, &[(a, b)], width_m)
}

/// A ribbon of dashes: DASH_M drawn, GAP_M left out.
pub(crate) fn dashed_ribbon(scn: &Scenario, a: Pos, b: Pos, width_m: f32) -> Mesh {
    const DASH_M: f32 = 12.0;
    const GAP_M: f32 = 8.0;
    let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    let mut pieces = Vec::new();
    let mut s = 0.0;
    while s < len {
        let e = (s + DASH_M).min(len);
        pieces.push((lerp(a, b, s / len), lerp(a, b, e / len)));
        s = e + GAP_M;
    }
    if pieces.is_empty() {
        pieces.push((a, b));
    }
    ribbon_pieces(scn, &pieces, width_m)
}

fn ribbon_pieces(scn: &Scenario, pieces: &[(Pos, Pos)], width_m: f32) -> Mesh {
    const RESAMPLE_M: f32 = 5.0;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for &(a, b) in pieces {
        let base = positions.len() as u32;
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = (dx * dx + dy * dy).sqrt();
        let steps = (len / RESAMPLE_M).ceil().max(1.0) as usize;
        let (nx, ny) = if len > 1e-3 {
            (-dy / len * width_m * 0.5, dx / len * width_m * 0.5)
        } else {
            (width_m * 0.5, 0.0)
        };
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
            let k = base + i * 2;
            indices.extend_from_slice(&[k, k + 2, k + 1, k + 1, k + 2, k + 3]);
        }
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

/// The engines' water: a jet of drops from each working engine to the cells it
/// has just wetted, hottest first (the same order the model wets them in), with
/// a burst of spray where it lands.
pub fn animate_spray(
    sim: Res<Sim>,
    time: Res<Time>,
    assets: Res<UnitAssets>,
    mut drops: Query<(&Droplet, &mut Transform, &mut Visibility)>,
) {
    let now = time.elapsed_seconds();
    let w = sim.scn.world;
    let reach = rocca::game::ENGINE_RANGE_M * 1.5;
    // The targets of each engine, computed once per frame and not per drop.
    let mut jets: std::collections::HashMap<usize, Vec<Pos>> = Default::default();
    for u in sim.crews.units.iter().filter(|u| u.kind == UnitKind::Engine) {
        if u.state != UnitState::Working || u.aboard || !matches!(u.task, Task::Attack { .. }) {
            continue;
        }
        let mut near: Vec<(f32, Pos)> = assets
            .wet_shown
            .iter()
            .map(|(i, _)| w.centre_of(Cell { row: i / w.fire_cols, col: i % w.fire_cols }))
            .filter(|p| (p.x - u.pos.x).hypot(p.y - u.pos.y) <= reach)
            .map(|p| (sim.fire.threat().at(p), p))
            .collect();
        near.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        jets.insert(u.id, near.into_iter().take(JETS).map(|(_, p)| p).collect());
    }
    for (d, mut tf, mut vis) in &mut drops {
        let target = jets.get(&d.unit).and_then(|j| j.get(d.jet));
        let (Some(&to), Some(u)) = (target, sim.crews.units.get(d.unit)) else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        let phase = (now / FLIGHT_S + d.k as f32 / DROPS as f32 + d.jet as f32 * 0.37).fract();
        let from_y = sim.scn.terrain.height_at(u.pos) + NOZZLE_M;
        let to_y = sim.scn.terrain.height_at(to) + RING_LIFT_M;
        let len = (to.x - u.pos.x).hypot(to.y - u.pos.y);
        // A little scatter so the jet is a stream rather than a string of beads.
        let wob = ((d.k * 7 + d.jet * 13) as f32).sin();
        let t = phase.min(1.0);
        let p = Pos {
            x: u.pos.x + (to.x - u.pos.x) * t + wob * 1.5 * (t * (1.0 - t)),
            y: u.pos.y + (to.y - u.pos.y) * t + wob * 1.5 * (t * (1.0 - t)),
        };
        let crown = ARC_BASE_M + ARC_PER_M * len;
        let y = from_y + (to_y - from_y) * t + crown * 4.0 * t * (1.0 - t);
        tf.translation = frame::to_bevy(p, y);
        // Bursts into mist on landing: grows and thins over the last tenth.
        let burst = ((phase - 0.9) / 0.1).clamp(0.0, 1.0);
        tf.scale = Vec3::splat(DROP_M + (SPLASH_M - DROP_M) * burst);
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
    }
}
