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
use crate::rings::ring_mesh;
use crate::sim::Sim;

const BEACON_LIFT_M: f32 = 26.0;
const WIND_LIFT_M: f32 = 40.0;
const PIN_LIFT_M: f32 = 30.0;

#[derive(Component)]
pub struct WindArrow;
#[derive(Component)]
pub struct DynamicMarker;
#[derive(Component)]
pub struct BeaconLayer;

/// Rings for the player's own orders: a warned district, an engine's post.
#[derive(Component)]
pub struct OrderRing;

/// Routes on the map: the ones being driven and the proposed ones.
#[derive(Component)]
pub struct RouteMark;

#[derive(Resource)]
pub struct OverlayAssets {
    unlit_white: Handle<RetroMaterial>,
    ring_orange: Handle<RetroMaterial>,
    beacons: Handle<Mesh>,
    key: (usize, usize, i64),
    /// What the order rings were built for: (warned districts, engine posts).
    order_key: (usize, usize),
    ring_blue: Handle<RetroMaterial>,
    ring_amber: Handle<RetroMaterial>,
    ring_green: Handle<RetroMaterial>,
    ring_white: Handle<RetroMaterial>,
    /// What the routes were built for.
    route_key: u64,
}

fn glow(c: [f32; 3], a: f32, boost: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(c[0], c[1], c[2], a),
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

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RetroMaterial>>,
) {
    let mut mat = |c: [f32; 3], a: f32, boost: f32| materials.add(retro::material(glow(c, a, boost), false));
    let arrow_mat = mat([1.0, 1.0, 1.0], 0.78, 0.9);
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
    commands.spawn((
        MaterialMeshBundle::<RetroMaterial> {
            mesh: meshes.add(arrow_mesh()),
            material: arrow_mat,
            ..default()
        },
        WindArrow,
    ));
    let ring_blue = mat([0.35, 0.75, 1.0], 0.85, 1.6);
    let ring_amber = mat([1.0, 0.75, 0.25], 0.85, 1.6);
    let ring_green = mat([0.35, 1.0, 0.6], 0.85, 1.4);
    let ring_white = mat([1.0, 1.0, 1.0], 0.9, 1.6);
    commands.insert_resource(OverlayAssets {
        unlit_white,
        ring_orange,
        beacons,
        key: (usize::MAX, 0, 0),
        order_key: (usize::MAX, 0),
        ring_blue,
        ring_amber,
        ring_green,
        ring_white,
        route_key: u64::MAX,
    });
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
    for (k, d) in sim.districts.iter().enumerate() {
        let mat = match sim.active.civil[k] {
            rocca::Civil::Nessuno => continue,
            rocca::Civil::Preallerta => assets.ring_amber.clone(),
            rocca::Civil::Evacua => assets.ring_blue.clone(),
        };
        let homes: Vec<Pos> = d.households.iter().map(|&i| sim.agents.households[i].home).collect();
        for (centre, radius) in clusters(&homes) {
            commands.spawn((
                MaterialMeshBundle::<RetroMaterial> { mesh: meshes.add(ring_mesh(&sim.scn, centre, radius + 40.0)), material: mat.clone(), ..default() },
                OrderRing,
            ));
        }
    }
    for p in sim.posts.iter().flatten() {
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(ring_mesh(&sim.scn, p.at, rocca::coordinator::DEFEND_REACH_M)),
                material: assets.ring_green.clone(),
                ..default()
            },
            OrderRing,
        ));
    }
}

/// Homes grouped into the places a ring can hold: a district of four hamlets
/// gets four rings, not one across the woods between them.
fn clusters(homes: &[Pos]) -> Vec<(Pos, f32)> {
    const LINK_M: f32 = 250.0;
    let mut group: Vec<usize> = (0..homes.len()).collect();
    fn root(g: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while g[r] != r {
            r = g[r];
        }
        g[i] = r;
        r
    }
    for i in 0..homes.len() {
        for j in 0..i {
            if rocca::district::dist(homes[i], homes[j]) <= LINK_M {
                let (a, b) = (root(&mut group, i), root(&mut group, j));
                group[a] = b;
            }
        }
    }
    let mut out = vec![];
    for r in 0..homes.len() {
        let members: Vec<Pos> = (0..homes.len()).filter(|&i| root(&mut group, i) == r).map(|i| homes[i]).collect();
        if members.is_empty() {
            continue;
        }
        let n = members.len() as f32;
        let c = Pos { x: members.iter().map(|p| p.x).sum::<f32>() / n, y: members.iter().map(|p| p.y).sum::<f32>() / n };
        let radius = members.iter().map(|p| rocca::district::dist(*p, c)).fold(0.0, f32::max);
        out.push((c, radius));
    }
    out
}

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
            commands.spawn((
                MaterialMeshBundle::<RetroMaterial> { mesh: meshes.add(crate::rings::path_mesh(&sim.scn, &pts, 9.0, None)), material: assets.ring_green.clone(), ..default() },
                RouteMark,
            ));
        }
    }
    if kiosk.phase == crate::kiosk::Phase::Fine {
        return;
    }
    for p in proposed {
        if p.route.len() >= 2 {
            commands.spawn((
                MaterialMeshBundle::<RetroMaterial> {
                    mesh: meshes.add(crate::rings::path_mesh(&sim.scn, &p.route, 9.0, Some(30.0))),
                    material: assets.ring_white.clone(),
                    ..default()
                },
                RouteMark,
            ));
        }
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(ring_mesh(&sim.scn, p.at, rocca::coordinator::DEFEND_REACH_M)),
                material: assets.ring_white.clone(),
                ..default()
            },
            RouteMark,
        ));
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
