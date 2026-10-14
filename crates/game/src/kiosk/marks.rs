//! Signs on the map for the orders (docs/01-GIOCO.md §6): the hexes a team
//! can be sent to (in pause, or with a team selected), the hex chosen and the
//! road to it, the roads the teams are driving, the evacuation's route
//! (proposed, dashed; in course, solid), and the roads the fire has closed.
//!
//! Everything sits above the canopy (`rings::RING_LIFT_M`), in flat bright
//! colours drawn over the trees (`flat::FlatMaterial`), and is rebuilt from the game's state behind a key, so a restart needs nothing
//! but the key cleared.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use rocca::tutorial::Expect;
use scenario::Pos;

use super::{Kiosk, Phase, Selection};
use crate::flat::FlatMaterial;
use crate::rings::{path_mesh, path_mesh_at, RING_LIFT_M};
use crate::sim::Sim;

/// Widths on the map, metres.
const HEX_LINE_M: f32 = 12.0;
/// The dark under-stroke of the hex lines, wider, a little lower.
const HEX_UNDER_M: f32 = 24.0;
const HEX_PICK_M: f32 = 16.0;
const ROUTE_M: f32 = 12.0;
const CLOSED_M: f32 = 18.0;
const DASH_M: f32 = 40.0;
/// How much wider the dark under-stroke of a route is than the route.
const UNDER: f32 = 1.8;
/// Closed roads are recomputed this often, in game steps.
const CLOSED_EVERY: u64 = 10;

#[derive(Component)]
pub struct HexMark;
#[derive(Component)]
pub struct OrderMark;
#[derive(Component)]
pub struct ClosedMark;

#[derive(Resource)]
pub struct MarkAssets {
    hex: Handle<FlatMaterial>,
    /// Dark, under the hexes and the routes.
    under: Handle<FlatMaterial>,
    pick: Handle<FlatMaterial>,
    want: Handle<FlatMaterial>,
    route: Handle<FlatMaterial>,
    evac: Handle<FlatMaterial>,
    closed: Handle<FlatMaterial>,
    /// What the hex grid, the orders and the closed roads were built for.
    hex_key: Option<String>,
    order_key: u64,
    closed_key: u64,
}

/// The colours of the signs (sRGB). Each means one thing: the teams' roads
/// are cyan, the evacuation's green (the escape-route sign), the closed
/// roads red, the hex the tutorial wants amber.
pub const ROUTE: [f32; 3] = [0.0, 0.85, 1.0];
pub const EVAC: [f32; 3] = [0.25, 1.0, 0.2];
const CLOSED: [f32; 3] = [1.0, 0.12, 0.08];
const WANT: [f32; 3] = [1.0, 0.72, 0.0];

pub fn setup(mut commands: Commands, mut materials: ResMut<Assets<FlatMaterial>>) {
    commands.insert_resource(MarkAssets {
        hex: materials.add(FlatMaterial::new([1.0, 1.0, 1.0])),
        under: materials.add(FlatMaterial::new([0.02, 0.03, 0.05]).pulled(-12.0)),
        pick: materials.add(FlatMaterial::new([1.0, 1.0, 1.0]).pulled(4.0)),
        want: materials.add(FlatMaterial::new(WANT).pulled(4.0)),
        route: materials.add(FlatMaterial::new(ROUTE)),
        evac: materials.add(FlatMaterial::new(EVAC).pulled(2.0)),
        closed: materials.add(FlatMaterial::new(CLOSED).pulled(1.0)),
        hex_key: None,
        order_key: u64::MAX,
        closed_key: u64::MAX,
    });
}

/// Several meshes as one draw.
fn merge(parts: Vec<Mesh>) -> Mesh {
    let (mut pos, mut idx): (Vec<[f32; 3]>, Vec<u32>) = (vec![], vec![]);
    for m in parts {
        let base = pos.len() as u32;
        if let Some(VertexAttributeValues::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION) {
            pos.extend_from_slice(p);
        }
        if let Some(Indices::U32(i)) = m.indices() {
            idx.extend(i.iter().map(|v| v + base));
        }
    }
    let n = pos.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n]);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

fn outline(at: rocca::Axial) -> Vec<Pos> {
    let c = at.corners();
    c.iter().chain(std::iter::once(&c[0])).copied().collect()
}

fn spawn<M: Component>(commands: &mut Commands, meshes: &mut Assets<Mesh>, mesh: Mesh, mat: &Handle<FlatMaterial>, tag: M) {
    commands.spawn((
        MaterialMeshBundle::<FlatMaterial> { mesh: meshes.add(mesh), material: mat.clone(), ..default() },
        tag,
        bevy::pbr::NotShadowCaster,
        bevy::pbr::NotShadowReceiver,
        bevy::render::view::NoFrustumCulling,
    ));
}

/// The hex grid: built once per game, shown in pause or with a team selected.
pub fn update_hexes(
    mut commands: Commands,
    sim: Res<Sim>,
    kiosk: Res<Kiosk>,
    mut assets: ResMut<MarkAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut q: Query<(Entity, &mut Visibility), With<HexMark>>,
) {
    let key = format!("{}:{}", sim.case.name, sim.hexes.hexes.len());
    if assets.hex_key.as_deref() != Some(key.as_str()) {
        assets.hex_key = Some(key);
        for (e, _) in &q {
            commands.entity(e).despawn();
        }
        let parts: Vec<Mesh> = sim.hexes.hexes.iter().map(|h| path_mesh(&sim.scn, &outline(h.at), HEX_LINE_M, None)).collect();
        let under: Vec<Mesh> = sim.hexes.hexes.iter().map(|h| path_mesh_at(&sim.scn, &outline(h.at), HEX_UNDER_M, None, RING_LIFT_M - 4.0)).collect();
        spawn(&mut commands, &mut meshes, merge(under), &assets.under, HexMark);
        spawn(&mut commands, &mut meshes, merge(parts), &assets.hex, HexMark);
        return;
    }
    let playing = matches!(kiosk.phase, Phase::Tutorial | Phase::Gioco);
    let show = playing && (sim.paused() || matches!(kiosk.selection, Selection::Team(_)));
    let want = if show { Visibility::Inherited } else { Visibility::Hidden };
    for (_, mut v) in &mut q {
        if *v != want {
            *v = want;
        }
    }
}

/// The chosen hex and its road, the tutorial's hex, the teams' roads, the
/// evacuation's route.
pub fn update_orders(
    mut commands: Commands,
    sim: Res<Sim>,
    kiosk: Res<Kiosk>,
    mut assets: ResMut<MarkAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<Entity, With<OrderMark>>,
) {
    let playing = matches!(kiosk.phase, Phase::Tutorial | Phase::Gioco);
    let mut key = (sim.time_s() / (2 * rocca::STEP_S)) as u64 ^ (playing as u64) << 63;
    let mut mix = |v: u64| key = key.wrapping_mul(0x100_0000_01b3) ^ v;
    if let Some(at) = kiosk.move_hex {
        mix((at.q as i64 as u64) << 20 ^ at.r as i64 as u64);
    }
    if let Some(Ok(p)) = &kiosk.move_plan {
        mix(p.path.len() as u64);
    }
    if let Some(Ok(p)) = &kiosk.evac_plan {
        mix(p.district as u64 + 1000 + p.path.points.len() as u64);
    }
    if let Some((_, _, Ok(p))) = &kiosk.evac_hover {
        mix(p.district as u64 + 5000 + p.path.points.len() as u64);
    }
    if let Some(Expect::Move(t, at)) = kiosk.expect() {
        mix(t as u64 * 7 + at.q as u64 * 13 + at.r as u64);
    }
    if key == assets.order_key {
        return;
    }
    assets.order_key = key;
    for e in &old {
        commands.entity(e).despawn();
    }
    if !playing {
        return;
    }
    // the hex the tutorial asks for
    if let Some(Expect::Move(_, at)) = kiosk.expect() {
        spawn(&mut commands, &mut meshes, path_mesh(&sim.scn, &outline(at), HEX_PICK_M, None), &assets.want, OrderMark);
    }
    // the move being chosen
    if let Some(at) = kiosk.move_hex {
        spawn(&mut commands, &mut meshes, path_mesh(&sim.scn, &outline(at), HEX_PICK_M, None), &assets.pick, OrderMark);
    }
    if let Some(Ok(p)) = &kiosk.move_plan {
        if p.path.len() >= 2 {
            spawn(&mut commands, &mut meshes, path_mesh(&sim.scn, &p.path, ROUTE_M, Some(DASH_M)), &assets.pick, OrderMark);
        }
    }
    // the teams' roads
    let routes: Vec<Vec<Pos>> = (0..sim.teams.len()).map(|t| sim.team_route(t)).filter(|r| r.len() >= 2).collect();
    if !routes.is_empty() {
        let under = routes.iter().map(|r| path_mesh_at(&sim.scn, r, ROUTE_M * UNDER, None, RING_LIFT_M - 2.0)).collect();
        spawn(&mut commands, &mut meshes, merge(under), &assets.under, OrderMark);
        let lines = routes.iter().map(|r| path_mesh(&sim.scn, r, ROUTE_M, None)).collect();
        spawn(&mut commands, &mut meshes, merge(lines), &assets.route, OrderMark);
    }
    // the evacuation: under the pointer or chosen (dashed), and in course
    let hover = kiosk.evac_hover.as_ref().and_then(|(_, _, p)| p.as_ref().ok());
    let dashed = kiosk.evac_plan.as_ref().and_then(|p| p.as_ref().ok()).into_iter().chain(hover).map(|p| (&p.path.points, Some(DASH_M)));
    for (pts, dash) in dashed.chain(sim.evac.active().map(|e| (&e.path.points, None))) {
        spawn(&mut commands, &mut meshes, path_mesh_at(&sim.scn, pts, ROUTE_M * 1.4 * UNDER, None, RING_LIFT_M - 2.0), &assets.under, OrderMark);
        spawn(&mut commands, &mut meshes, path_mesh(&sim.scn, pts, ROUTE_M * 1.4, dash), &assets.evac, OrderMark);
    }
}

/// The roads the fire has closed, red.
pub fn update_closed(
    mut commands: Commands,
    sim: Res<Sim>,
    kiosk: Res<Kiosk>,
    mut assets: ResMut<MarkAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<Entity, With<ClosedMark>>,
) {
    let playing = matches!(kiosk.phase, Phase::Tutorial | Phase::Gioco);
    let key = if playing { sim.generation / CLOSED_EVERY } else { u64::MAX - 1 };
    if key == assets.closed_key {
        return;
    }
    assets.closed_key = key;
    for e in &old {
        commands.entity(e).despawn();
    }
    if !playing {
        return;
    }
    let parts: Vec<Mesh> = sim.closed_roads().into_iter().map(|(a, b)| path_mesh(&sim.scn, &[a, b], CLOSED_M, None)).collect();
    if !parts.is_empty() {
        spawn(&mut commands, &mut meshes, merge(parts), &assets.closed, ClosedMark);
    }
}

pub fn reset(mut restarted: EventReader<crate::sim::SimRestarted>, mut assets: ResMut<MarkAssets>) {
    if !restarted.is_empty() {
        restarted.clear();
        assets.hex_key = None;
        assets.order_key = u64::MAX;
        assets.closed_key = u64::MAX;
    }
}
