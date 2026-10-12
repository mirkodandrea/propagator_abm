//! Map overlays for the kiosk: the wind arrow on the terrain, household
//! beacons, closed-road markers, the plan's rings and routes.
//!
//! Everything sits well above the canopy (finding 13: a correct overlay
//! under the plants is seen never), is unlit and glowing so the bloom pass
//! catches it, and is rebuilt from `Sim` state with no latch of its own — so a
//! restart needs only the key cleared (finding 21).

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::population::Status;
use scenario::Pos;

use crate::frame;
use crate::retro;
use crate::retro::RetroMaterial;
use crate::rings::{path_mesh, ring_mesh, tube_transform, wall_mesh, TubeSpin, HALO};

/// Width of a route on the map.
const ROUTE_M: f32 = 9.0;
use crate::sim::Sim;

const BEACON_LIFT_M: f32 = 26.0;
const WIND_LIFT_M: f32 = 40.0;
const PIN_LIFT_M: f32 = 30.0;

#[derive(Component)]
pub struct WindArrow;
#[derive(Component)]
pub struct DynamicMarker;

/// Any other 3D UI mesh that must not take part in shadows.
#[derive(Component)]
pub struct UiGlyph;
#[derive(Component)]
pub struct BeaconLayer;

/// Rings for the player's own orders: a warned district, an engine's post.
#[derive(Component, Clone, Copy)]
pub struct OrderRing;

/// Routes on the map: the ones being driven and the proposed ones.
#[derive(Component, Clone, Copy)]
pub struct RouteMark;

#[derive(Resource)]
pub struct OverlayAssets {
    unlit_white: Handle<RetroMaterial>,
    ring_orange: Handle<RetroMaterial>,
    beacons: Handle<Mesh>,
    key: (usize, usize, i64),
    /// What the order rings were built for: (warned districts, engine posts).
    order_key: (usize, usize),
    ring_cover: Handle<RetroMaterial>,
    ring_white: Handle<RetroMaterial>,
    /// Dark, under rings and routes.
    halo: Handle<RetroMaterial>,
    /// What the routes were built for.
    route_key: u64,
}

/// The colour of a glowing UI element: `c` as sRGB, its linear value scaled by
/// `gain`. Unlit output ignores emissive and goes through the tone mapper, so
/// anything near 1.0 comes out as muted as the terrain; >1 stays saturated and
/// feeds the bloom. No gain (0) is a plain colour.
fn hdr(c: [f32; 3], a: f32, gain: f32) -> Color {
    let l = Color::srgb(c[0], c[1], c[2]).to_linear();
    let g = if gain > 0.0 { gain } else { 1.0 };
    Color::LinearRgba(LinearRgba::new(l.red * g, l.green * g, l.blue * g, a))
}

fn glow(c: [f32; 3], a: f32, boost: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: hdr(c, a, boost),
        emissive: LinearRgba::rgb(c[0] * boost, c[1] * boost, c[2] * boost),
        unlit: true,
        alpha_mode: if a < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque },
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

/// Flat arrow along -Z (north in Bevy), tail at +Z, shaft + head, length 1.
fn arrow_mesh() -> Mesh {
    let (w, hw) = (0.10, 0.26); // shaft half width, head half width
    let pts: [[f32; 3]; 7] = [
        [-w, 0.0, 0.5],
        [w, 0.0, 0.5],
        [w, 0.0, -0.1],
        [hw, 0.0, -0.1],
        [0.0, 0.0, -0.5],
        [-hw, 0.0, -0.1],
        [-w, 0.0, -0.1],
    ];
    let idx = [0u32, 2, 1, 0, 6, 2, 6, 5, 3, 6, 3, 2, 5, 4, 3];
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pts.to_vec());
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 7]);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; 7]);
    m.insert_indices(Indices::U32(idx.to_vec()));
    m
}

/// A chevron pointing along -Z, in the arrow's unit space: the flow marks that
/// run down the shaft so the wind reads as moving air, not as a signpost.
fn chevron_mesh() -> Mesh {
    let pts: [[f32; 3]; 6] = [
        [-0.20, 0.0, 0.08],
        [0.0, 0.0, -0.05],
        [0.20, 0.0, 0.08],
        [-0.20, 0.0, 0.17],
        [0.0, 0.0, 0.04],
        [0.20, 0.0, 0.17],
    ];
    let idx = [0u32, 1, 3, 1, 4, 3, 1, 2, 4, 2, 5, 4];
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pts.to_vec());
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 6]);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; 6]);
    m.insert_indices(Indices::U32(idx.to_vec()));
    m
}

/// Flow chevrons of the wind arrow, with their phase in 0..1.
#[derive(Component)]
pub struct WindChevron(f32);

/// Slide the chevrons down the shaft, fading in at the tail and out at the head.
pub fn animate_wind(time: Res<Time>, mut q: Query<(&WindChevron, &mut Transform)>) {
    for (c, mut tf) in &mut q {
        let k = (time.elapsed_seconds() * 0.45 + c.0).fract();
        tf.translation = Vec3::new(0.0, 4.0, 0.45 - k * 0.8);
        let s = (k * std::f32::consts::PI).sin();
        tf.scale = Vec3::new(0.4 + 0.6 * s, 1.0, 0.4 + 0.6 * s);
    }
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RetroMaterial>>,
) {
    let mut mat = |c: [f32; 3], a: f32, boost: f32| materials.add(retro::material(glow(c, a, boost), false));
    let arrow_mat = mat([0.10, 0.40, 1.0], 0.95, 3.0);
    let chevron_mat = mat([0.70, 0.95, 1.0], 1.0, 4.0);
    let unlit_white = mat([1.0, 1.0, 1.0], 1.0, 1.4);
    let ring_orange = mat([1.0, 0.62, 0.1], 0.9, 1.6);
    let beacons = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
    commands.spawn((
        MaterialMeshBundle::<RetroMaterial> {
            mesh: beacons.clone(),
            material: unlit_white.clone(),
            ..default()
        },
        bevy::render::view::NoFrustumCulling,
        BeaconLayer,
    ));
    let arrow = meshes.add(arrow_mesh());
    let chevron = meshes.add(chevron_mesh());
    commands
        .spawn((
            MaterialMeshBundle::<RetroMaterial> { mesh: arrow.clone(), material: arrow_mat, ..default() },
            WindArrow,
        ))
        .with_children(|p| {
            for i in 0..3 {
                p.spawn((
                    MaterialMeshBundle::<RetroMaterial> { mesh: chevron.clone(), material: chevron_mat.clone(), ..default() },
                    WindChevron(i as f32 / 3.0),
                ));
            }
        });
    // UI in 3D is all blues: cyan = defended area and active routes, ice = proposed plan.
    let ring_cover = mat([0.00, 0.90, 1.00], 0.9, 1.6);
    let ring_white = mat([0.65, 0.90, 1.00], 0.9, 1.6);
    let halo = mat([0.03, 0.05, 0.06], 0.75, 0.0);
    commands.insert_resource(OverlayAssets {
        halo,
        unlit_white,
        ring_orange,
        beacons,
        key: (usize::MAX, 0, 0),
        order_key: (usize::MAX, 0),
        ring_cover,
        ring_white,
        route_key: u64::MAX,
    });
}

/// 3D UI is a graphic layer, not part of the scene: it casts and receives no
/// shadows. Runs on whatever was just spawned.
pub fn no_ui_shadows(
    mut commands: Commands,
    new: Query<
        Entity,
        (
            Or<(Added<OrderRing>, Added<RouteMark>, Added<WindArrow>, Added<WindChevron>, Added<DynamicMarker>, Added<BeaconLayer>, Added<UiGlyph>, Added<crate::units::OrderMarker>)>,
            Without<bevy::pbr::NotShadowCaster>,
        ),
    >,
) {
    for e in &new {
        // may already be despawned this frame (rings are rebuilt eagerly)
        commands.entity(e).try_insert((bevy::pbr::NotShadowCaster, bevy::pbr::NotShadowReceiver));
    }
}

/// Rings and routes breathe: a slow pulse of alpha and glow, out of phase per
/// colour, so they read as UI laid over the landscape and not as scenery.
pub fn pulse_rings(time: Res<Time>, assets: Res<OverlayAssets>, mut materials: ResMut<Assets<RetroMaterial>>) {
    let t = time.elapsed_seconds();
    let set = [
        (&assets.ring_cover, [0.00, 0.90, 1.00], 2.2),
        (&assets.ring_white, [0.65, 0.90, 1.00], 3.3),
    ];
    for (h, c, phase) in set {
        let k = 0.5 + 0.5 * (t * 2.6 + phase).sin();
        if let Some(m) = materials.get_mut(h) {
            m.base.base_color = hdr(c, 0.70 + 0.30 * k, 2.2 + 1.8 * k);
        }
    }
}

/// The plan in force, on the ground: a ring round each district with a civil
/// order (blue: evacuation, amber: pre-alert), and a green ring for the homes
/// each unit's post covers (`rocca::coordinator::DEFEND_REACH_M`). Rebuilt
/// only when the plan or the posts change. Rings sit +20 m (finding 13).
pub fn update_orders(
    mut commands: Commands,
    sim: Res<Sim>,
    mut assets: ResMut<OverlayAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<Entity, With<OrderRing>>,
) {
    let civil = sim.active.civil.iter().fold(0usize, |a, c| a * 3 + *c as usize);
    let posts: usize = sim.posts.iter().flatten().fold(0usize, |a, p| a.wrapping_mul(31).wrapping_add(p.at.x as usize ^ p.at.y as usize));
    if (civil, posts) == assets.order_key {
        return;
    }
    assets.order_key = (civil, posts);
    for e in &old {
        commands.entity(e).despawn();
    }
    for p in sim.posts.iter().flatten() {
        cover_zone(&mut commands, &mut meshes, &sim, p.at, &assets.ring_cover, OrderRing);
    }
}

/// The area a post defends, exactly as the simulation sees it
/// (`Abm::protection`: homes within `DEFEND_REACH_M` of the post): a filled
/// faded tube with a sharp rim on the ground, and a mark on each home that is inside it.
fn cover_zone<M: Component + Copy>(commands: &mut Commands, meshes: &mut Assets<Mesh>, sim: &Sim, at: Pos, rim: &Handle<RetroMaterial>, tag: M) {
    let r = rocca::coordinator::DEFEND_REACH_M;
    commands.spawn((MaterialMeshBundle::<RetroMaterial> { mesh: meshes.add(wall_mesh(&sim.scn, at, r)), material: rim.clone(), transform: tube_transform(at), ..default() }, tag, TubeSpin));
    commands.spawn((MaterialMeshBundle::<RetroMaterial> { mesh: meshes.add(ring_mesh(&sim.scn, at, r)), material: rim.clone(), ..default() }, tag));
    for h in &sim.agents.households {
        if (h.home.x - at.x).hypot(h.home.y - at.y) <= r {
            commands.spawn((MaterialMeshBundle::<RetroMaterial> { mesh: meshes.add(ring_mesh(&sim.scn, h.home, 14.0)), material: rim.clone(), ..default() }, tag));
        }
    }
}

/// A ring or route with its dark halo just under it. The halo sits lower, so
/// the transparent pass draws it first.
fn spawn_outlined<M: Component + Copy>(commands: &mut Commands, mesh: Handle<Mesh>, halo_mesh: Handle<Mesh>, mat: Handle<RetroMaterial>, halo: Handle<RetroMaterial>, tag: M) {
    // no dark halo: it read as a drop shadow under the blue UI
    let _ = (halo_mesh, halo);
    commands.spawn((MaterialMeshBundle::<RetroMaterial> { mesh, material: mat, ..default() }, tag));
}

/// How far under its ring or route a halo lies.
const HALO_DROP_M: f32 = 1.5;

/// Routes: green and solid for the way each unit is driving now; white and
/// dashed, with a white ring at the post, for a proposed move the player has
/// not confirmed yet. Rebuilt every two game steps or when the preview changes.
pub fn update_routes(
    mut commands: Commands,
    sim: Res<Sim>,
    kiosk: Res<crate::kiosk::Kiosk>,
    mut assets: ResMut<OverlayAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<Entity, With<RouteMark>>,
) {
    let proposed: Vec<&rocca::Post> = kiosk
        .preview
        .iter()
        .flat_map(|p| p.posts.iter().flatten())
        .filter(|p| sim.posts.get(p.unit).and_then(|x| x.as_ref()).is_none_or(|a| a.district != p.district || rocca::district::dist(a.at, p.at) > 1.0))
        .collect();
    let mut key = (sim.time_s() / (2 * rocca::STEP_S)) as u64;
    for p in &proposed {
        key = key.wrapping_mul(31).wrapping_add(p.unit as u64 * 7919 + p.at.x as u64 * 13 + p.at.y as u64);
    }
    if key == assets.route_key {
        return;
    }
    assets.route_key = key;
    for e in &old {
        commands.entity(e).despawn();
    }
    for (k, u) in sim.crews.units.iter().enumerate() {
        if u.kind.is_air() {
            continue;
        }
        let pts = sim.crews.route_points(k, &sim.agents.network);
        if pts.len() >= 2 {
            let (route, halo) = (meshes.add(path_mesh(&sim.scn, &pts, ROUTE_M, None)), meshes.add(path_mesh(&sim.scn, &pts, ROUTE_M * HALO, None)));
            spawn_outlined(&mut commands, route, halo, assets.ring_cover.clone(), assets.halo.clone(), RouteMark);
        }
    }
    if kiosk.phase == crate::kiosk::Phase::Fine {
        return;
    }
    for p in proposed {
        if p.route.len() >= 2 {
            let (route, halo) = (meshes.add(path_mesh(&sim.scn, &p.route, ROUTE_M, Some(30.0))), meshes.add(path_mesh(&sim.scn, &p.route, ROUTE_M * HALO, None)));
            spawn_outlined(&mut commands, route, halo, assets.ring_white.clone(), assets.halo.clone(), RouteMark);
        }
        cover_zone(&mut commands, &mut meshes, &sim, p.at, &assets.ring_white, RouteMark);
    }
}

pub fn reset(mut restarted: EventReader<crate::sim::SimRestarted>, mut assets: ResMut<OverlayAssets>) {
    if !restarted.is_empty() {
        restarted.clear();
        assets.key = (usize::MAX, 0, 0);
        assets.order_key = (usize::MAX, 0);
        assets.route_key = u64::MAX;
    }
}

/// The wind arrow: lies over the terrain upwind of the fire, pointing the way
/// the fire will be pushed. Every frame; cheap.
pub fn update_wind(sim: Res<Sim>, mut q: Query<&mut Transform, With<WindArrow>>) {
    let Ok(mut tf) = q.get_single_mut() else { return };
    let w = sim.fire.weather();
    let from = (w.wind_dir_deg as f32).to_radians();
    let (pos, _) = crate::terrain_mesh::cell_ground(&sim.scn, sim.scn.world.cell_of(sim.case.ignition()));
    // Beside the fire, not upwind of it: upwind is often under the top HUD band.
    // Prefer the east flank; if the wind runs east-west, the south one.
    let side = |a: f32| (a.sin(), a.cos());
    let (e1, e2) = (side(from + std::f32::consts::FRAC_PI_2), side(from - std::f32::consts::FRAC_PI_2));
    let (sx, sy) = if e1.0.abs().max(e2.0.abs()) > 0.4 {
        if e1.0 > e2.0 { e1 } else { e2 }
    } else if e1.1 < e2.1 {
        e1
    } else {
        e2
    };
    let down = from + std::f32::consts::PI;
    let up = Pos { x: pos.x + sx * 420.0 + down.sin() * 260.0, y: pos.y + sy * 420.0 + down.cos() * 260.0 };
    let len = 300.0 + 4.0 * w.wind_speed_kmh as f32;
    let ground = sim.scn.terrain.height_at(up);
    tf.translation = frame::to_bevy(up, ground + WIND_LIFT_M);
    tf.rotation = Quat::from_rotation_y(-(from + std::f32::consts::PI));
    tf.scale = Vec3::new(len * 0.55, 1.0, len);
}

struct Buf {
    pos: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
    nrm: Vec<[f32; 3]>,
}

fn octa(b: &mut Buf, c: Vec3, r: f32, color: [f32; 3]) {
    let s = b.pos.len() as u32;
    let v = [
        c + Vec3::new(0.0, r * 1.5, 0.0),
        c + Vec3::new(r, 0.0, 0.0),
        c + Vec3::new(0.0, 0.0, r),
        c + Vec3::new(-r, 0.0, 0.0),
        c + Vec3::new(0.0, 0.0, -r),
        c + Vec3::new(0.0, -r * 1.5, 0.0),
    ];
    for p in v {
        b.pos.push(p.into());
        b.nrm.push((p - c).normalize().into());
        b.col.push([color[0], color[1], color[2], 1.0]);
    }
    for (a, c2) in [(1, 2), (2, 3), (3, 4), (4, 1)] {
        b.idx.extend_from_slice(&[s, s + c2, s + a, s + 5, s + a, s + c2]);
    }
}

pub fn update_markers(
    mut commands: Commands,
    sim: Res<Sim>,
    mut assets: ResMut<OverlayAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<Entity, With<DynamicMarker>>,
) {
    if !sim.is_changed() {
        return;
    }
    let mut b = Buf { pos: vec![], col: vec![], idx: vec![], nrm: vec![] };
    // Only the families the fire is on: a red beacon over a home means
    // "someone is still in there". Warned, leaving and safe families are
    // already told by the district chips and the moving figures and cars; a
    // beacon over every roof (the first version) was noise.
    let threat = sim.fire.threat();
    for h in &sim.agents.households {
        let at_home = matches!(h.status, Status::Normal | Status::Warned | Status::Preparing | Status::Defending);
        let caught = matches!(h.status, Status::Trapped) || (at_home && threat.at(h.home) >= fire::threat::ALARMING);
        if !caught {
            continue;
        }
        let g = sim.scn.terrain.height_at(h.home);
        octa(&mut b, frame::to_bevy(h.home, g + BEACON_LIFT_M), 8.0, [2.0, 0.36, 0.24]);
    }
    let n = b.pos.len();
    if let Some(m) = meshes.get_mut(&assets.beacons) {
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, b.nrm);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, b.col);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n]);
        m.insert_indices(Indices::U32(b.idx));
    }

    // Closed roads change rarely: rebuild on a changed key. (Spot fires are
    // not marked: the player does not task units at them, and at the end of
    // a big fire they were dozens of red pins over the map.)
    let now = sim.time_s() as f32;
    let closures: Vec<(Pos, f32)> = sim
        .agents
        .closures()
        .iter()
        .filter(|c| c.active_at(now))
        .map(|c| (c.centre, c.radius_m))
        .collect();
    let sig = closures.first().map_or(0, |(c, _)| (c.x as i64) ^ ((c.y as i64) << 20));
    let key = (closures.len(), 0, sig);
    if key == assets.key {
        return;
    }
    assets.key = key;
    for e in &old {
        commands.entity(e).despawn_recursive();
    }
    for &(centre, radius) in &closures {
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(ring_mesh(&sim.scn, centre, radius.max(60.0))),
                material: assets.ring_orange.clone(),
                ..default()
            },
            DynamicMarker,
        ));
        let g = sim.scn.terrain.height_at(centre) + PIN_LIFT_M;
        let mut t = crate::toy::Toy::default();
        t.cuboid(Vec3::new(0.0, 12.0, 0.0), Vec3::new(18.0, 4.0, 3.0), [1.0, 0.55, 0.1]);
        t.cuboid(Vec3::new(-16.0, 5.0, 0.0), Vec3::new(2.5, 8.0, 2.5), [1.0, 1.0, 1.0]);
        t.cuboid(Vec3::new(16.0, 5.0, 0.0), Vec3::new(2.5, 8.0, 2.5), [1.0, 1.0, 1.0]);
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(t.finish()),
                material: assets.unlit_white.clone(),
                transform: Transform::from_translation(frame::to_bevy(centre, g)).with_scale(Vec3::splat(1.8)),
                ..default()
            },
            DynamicMarker,
        ));
    }
}
