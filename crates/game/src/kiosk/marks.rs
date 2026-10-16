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
/// The covered zone's boundary line.
const ZONE_LINE_M: f32 = 11.0;
/// Dash length of the zone boundary.
const ZONE_DASH_M: f32 = 60.0;
/// The drivable roads shown while a move is chosen.
const ROAD_M: f32 = 9.0;
/// Extra pull toward the camera for the hex lines: enough that the hills'
/// own bumps do not cut them into dashes.
const HEX_PULL_M: f32 = 60.0;
const ROUTE_M: f32 = 12.0;
const CLOSED_M: f32 = 18.0;
const DASH_M: f32 = 40.0;
/// How much wider the dark under-stroke of a route is than the route.
const UNDER: f32 = 1.8;
/// Closed roads are recomputed this often, in game steps.
const CLOSED_EVERY: u64 = 10;

#[derive(Component)]
pub struct HexMark;
/// The drivable roads, shown while a move is chosen.
#[derive(Component)]
pub struct RoadNetMark;
#[derive(Component)]
pub struct OrderMark;
#[derive(Component)]
pub struct ClosedMark;
/// The assigned team hexes, highlighted in steady yellow.
#[derive(Component)]
pub struct TeamMark;

#[derive(Resource)]
pub struct MarkAssets {
    hex: Handle<FlatMaterial>,
    /// The drivable roads, while a move is chosen.
    road: Handle<FlatMaterial>,
    /// Dark, under the hexes and the routes.
    under: Handle<FlatMaterial>,
    pick: Handle<FlatMaterial>,
    want: Handle<FlatMaterial>,
    /// The hexes a team placed on a hex covers.
    area: Handle<FlatMaterial>,
    route: Handle<FlatMaterial>,
    evac: Handle<FlatMaterial>,
    closed: Handle<FlatMaterial>,
    active: Handle<FlatMaterial>,
    /// The boundary of the zone covered by the deployed teams, assigned-hex yellow, dashed.
    zone: Handle<FlatMaterial>,
    team_key: u64,
    /// What the hex grid, the orders and the closed roads were built for.
    hex_key: Option<String>,
    /// The team, time and tutorial hex the usable hexes were built for.
    targets_key: Option<(usize, i64, Option<rocca::Axial>)>,
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
        hex: materials.add(FlatMaterial::with_alpha([1.0, 1.0, 1.0], 0.55).pulled(HEX_PULL_M)),
        road: materials.add(FlatMaterial::new([1.0, 0.93, 0.62])),
        under: materials.add(FlatMaterial::new([0.02, 0.03, 0.05]).pulled(HEX_PULL_M - 3.0)),
        pick: materials.add(FlatMaterial::new([1.0, 1.0, 1.0]).pulled(HEX_PULL_M + 4.0)),
        want: materials.add(FlatMaterial::new(WANT).pulled(HEX_PULL_M + 4.0)),
        area: materials.add(FlatMaterial::new(ROUTE).pulled(HEX_PULL_M + 2.0)),
        route: materials.add(FlatMaterial::new(ROUTE)),
        evac: materials.add(FlatMaterial::new(EVAC).pulled(2.0)),
        closed: materials.add(FlatMaterial::new(CLOSED).pulled(1.0)),
        active: materials.add(FlatMaterial::new([1.0, 0.9, 0.0]).pulled(HEX_PULL_M + 6.0)),
        // the same yellow as the assigned hexes, dashed
        zone: materials.add(FlatMaterial::new([1.0, 0.9, 0.0]).pulled(HEX_PULL_M + 3.0)),
        team_key: u64::MAX,
        hex_key: None,
        targets_key: None,
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

/// The hexes within the team's reach of `at` (its own not included).
fn reach(at: rocca::Axial) -> impl Iterator<Item = rocca::Axial> {
    let n = rocca::TEAM_REACH_HEXES;
    (-n..=n).flat_map(move |dq| (-n..=n).map(move |dr| rocca::Axial { q: at.q + dq, r: at.r + dr })).filter(move |h| (1..=n).contains(&h.distance(at)))
}

/// The hex a selected team is placed on, or being placed on.
fn area_hex(kiosk: &Kiosk, sim: &Sim) -> Option<rocca::Axial> {
    let Selection::Team(t) = kiosk.selection else { return None };
    kiosk.move_hex.or(sim.teams.get(t)?.hex)
}

fn outline(at: rocca::Axial) -> Vec<Pos> {
    let c = at.corners();
    c.iter().chain(std::iter::once(&c[0])).copied().collect()
}

/// The outer boundary of a set of hexes as closed paths: the edges that
/// belong to one hex of the set only, chained end to end.
fn boundary(hexes: &[rocca::Axial]) -> Vec<Vec<Pos>> {
    use std::collections::HashMap;
    let key = |p: Pos| ((p.x * 0.5).round() as i64, (p.y * 0.5).round() as i64);
    let mut edges: HashMap<((i64, i64), (i64, i64)), (Pos, Pos, u8)> = HashMap::new();
    for h in hexes {
        let c = h.corners();
        for i in 0..6 {
            let (a, b) = (c[i], c[(i + 1) % 6]);
            let k = if key(a) <= key(b) { (key(a), key(b)) } else { (key(b), key(a)) };
            edges.entry(k).or_insert((a, b, 0)).2 += 1;
        }
    }
    // from each boundary point, the boundary edges that start or end there
    let mut at: HashMap<(i64, i64), Vec<(Pos, Pos)>> = HashMap::new();
    for (a, b, _) in edges.into_values().filter(|e| e.2 == 1) {
        at.entry(key(a)).or_default().push((a, b));
        at.entry(key(b)).or_default().push((b, a));
    }
    let mut loops = vec![];
    while let Some(&start) = at.keys().next() {
        let mut path: Vec<Pos> = vec![];
        let mut here = start;
        loop {
            let Some(out) = at.get_mut(&here) else { break };
            let Some((a, b)) = out.pop() else { break };
            // the edge is used: take it off the other end too
            if let Some(back) = at.get_mut(&key(b)) {
                back.retain(|&(x, y)| !(key(x) == key(b) && key(y) == key(a)));
            }
            if path.is_empty() {
                path.push(a);
            }
            path.push(b);
            here = key(b);
            if here == start {
                break;
            }
        }
        at.retain(|_, v| !v.is_empty());
        if path.len() > 2 {
            loops.push(path);
        }
    }
    loops
}

fn spawn<M: Component>(commands: &mut Commands, meshes: &mut Assets<Mesh>, mesh: Mesh, mat: &Handle<FlatMaterial>, tag: M) -> Entity {
    commands.spawn((
        MaterialMeshBundle::<FlatMaterial> { mesh: meshes.add(mesh), material: mat.clone(), ..default() },
        tag,
        bevy::pbr::NotShadowCaster,
        bevy::pbr::NotShadowReceiver,
        bevy::render::view::NoFrustumCulling,
    )).id()
}

/// The hex grid is not drawn during play (user, 2026-10-15). While a team's
/// move is being chosen (in pause, the team taken or dragged) the drivable
/// roads show over the canopy, and the hexes the team can actually be sent to
/// ([`rocca::Game::move_targets`], the rule of the order itself; in the
/// tutorial only the one the step asks). The roads are built once, then shown
/// or hidden; the hexes are rebuilt for each team and pause.
pub fn update_hexes(
    mut commands: Commands,
    sim: Res<Sim>,
    kiosk: Res<Kiosk>,
    mut assets: ResMut<MarkAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut roads: Query<(Entity, &mut Visibility), (With<RoadNetMark>, Without<HexMark>)>,
    hexes: Query<Entity, With<HexMark>>,
) {
    if assets.hex_key.is_none() {
        for (e, _) in &roads {
            commands.entity(e).despawn();
        }
        assets.hex_key = Some(String::new());
        assets.targets_key = None;
        let net = &sim.net;
        let main = sim.hexes.hexes.first().map(|h| net.component(h.node, true));
        let (mut lines, mut under) = (vec![], vec![]);
        for n in 0..net.len() {
            let id = n as abm::network::NodeId;
            if Some(net.component(id, true)) != main {
                continue;
            }
            for e in net.neighbours(id) {
                if !e.drivable || (e.to as usize) < n {
                    continue;
                }
                let seg = [net.pos(id), net.pos(e.to)];
                under.push(path_mesh_at(&sim.scn, &seg, ROAD_M * UNDER, None, RING_LIFT_M - 2.0));
                lines.push(path_mesh(&sim.scn, &seg, ROAD_M, None));
            }
        }
        for (parts, mat) in [(under, assets.under.clone()), (lines, assets.road.clone())] {
            if !parts.is_empty() {
                let e = spawn(&mut commands, &mut meshes, merge(parts), &mat, RoadNetMark);
                commands.entity(e).insert(Visibility::Hidden);
            }
        }
        return;
    }
    let playing = matches!(kiosk.phase, Phase::Tutorial | Phase::Gioco);
    let team = kiosk.dragging.map(|(t, _)| t).or(match kiosk.selection {
        Selection::Team(t) => Some(t),
        _ => None,
    });
    let choosing = playing && sim.paused() && team.is_some();
    let vis = if choosing { Visibility::Inherited } else { Visibility::Hidden };
    for (_, mut v) in &mut roads {
        if *v != vis {
            *v = vis;
        }
    }
    let want = match kiosk.expect() {
        Some(Expect::Move(_, at)) => Some(at),
        _ => None,
    };
    let key = choosing.then(|| (team.unwrap_or(0), sim.time_s(), want));
    if key == assets.targets_key {
        return;
    }
    assets.targets_key = key;
    for e in &hexes {
        commands.entity(e).despawn();
    }
    let Some((t, _, want)) = key else { return };
    let targets: Vec<rocca::Axial> = sim.move_targets(t).into_iter().filter(|&h| want.map_or(true, |w| w == h)).collect();
    if !targets.is_empty() {
        let parts = targets.iter().map(|&h| path_mesh(&sim.scn, &outline(h), HEX_LINE_M, None)).collect();
        spawn(&mut commands, &mut meshes, merge(parts), &assets.hex, HexMark);
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
    if let Some(at) = area_hex(&kiosk, &sim) {
        mix((at.q as i64 as u64) << 8 ^ at.r as i64 as u64 ^ 0xa4ea);
    }
    if let Some(at) = kiosk.move_hex {
        mix((at.q as i64 as u64) << 20 ^ at.r as i64 as u64);
        if let Selection::Team(t) = kiosk.selection {
            mix(t as u64 + 77);
        }
    }
    match &kiosk.move_plan {
        Some(Ok(p)) => mix(p.path.len() as u64),
        Some(Err(_)) => mix(0xbad),
        None => {}
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
    // the area the team works around its hex, lit up
    if let Some(at) = area_hex(&kiosk, &sim).filter(|_| !matches!(kiosk.move_plan, Some(Err(_)))) {
        let rings = reach(at).map(|h| path_mesh(&sim.scn, &outline(h), HEX_PICK_M * 0.6, None)).collect();
        spawn(&mut commands, &mut meshes, merge(rings), &assets.area, OrderMark);
    }
    // the move being chosen: start hex and destination hex
    if let (Some(_), Selection::Team(t)) = (kiosk.move_hex, kiosk.selection) {
        let u = &sim.crews.units[sim.teams[t].unit];
        let from = sim.teams[t].hex.unwrap_or_else(|| rocca::Axial::of(u.pos));
        spawn(&mut commands, &mut meshes, path_mesh(&sim.scn, &outline(from), HEX_PICK_M, None), &assets.pick, OrderMark);
    }
    if let Some(at) = kiosk.move_hex {
        // a hex the team cannot go to is red, not white
        let refused = matches!(kiosk.move_plan, Some(Err(_)));
        let mat = if refused { &assets.closed } else { &assets.pick };
        spawn(&mut commands, &mut meshes, path_mesh(&sim.scn, &outline(at), HEX_PICK_M, None), mat, OrderMark);
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

/// Assigned team hexes stay yellow. Rebuilt when the set changes.
pub fn update_teams(
    mut commands: Commands,
    sim: Res<Sim>,
    kiosk: Res<Kiosk>,
    mut assets: ResMut<MarkAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<Entity, With<TeamMark>>,
) {
    let playing = matches!(kiosk.phase, Phase::Tutorial | Phase::Gioco);
    let mut hexes: Vec<_> = if playing {
        sim.teams.iter().filter_map(|t| t.hex).collect()
    } else {
        vec![]
    };
    hexes.sort_unstable();
    hexes.dedup();
    // the zone the deployed teams cover: only its outer boundary is drawn
    let mut zone: Vec<rocca::Axial> = vec![];
    if playing {
        for h in &hexes {
            zone.push(*h);
            zone.extend(reach(*h));
        }
    }
    zone.sort_unstable();
    zone.dedup();
    let mut key = playing as u64;
    for h in &zone {
        key = key.wrapping_mul(0x100_0000_01b3) ^ ((1u64 << 50) ^ (h.q as i64 as u64) << 24 ^ h.r as i64 as u64);
    }
    for h in &hexes {
        key = key.wrapping_mul(0x100_0000_01b3) ^ ((h.q as i64 as u64) << 24 ^ h.r as i64 as u64);
    }
    if key == assets.team_key {
        return;
    }
    assets.team_key = key;
    for e in &old {
        commands.entity(e).despawn();
    }
    let loops = boundary(&zone);
    if !loops.is_empty() {
        let parts = loops.iter().map(|l| path_mesh(&sim.scn, l, ZONE_LINE_M, Some(ZONE_DASH_M))).collect();
        spawn(&mut commands, &mut meshes, merge(parts), &assets.zone, TeamMark);
    }
    if !hexes.is_empty() {
        let parts = hexes.iter().map(|h| path_mesh(&sim.scn, &outline(*h), HEX_PICK_M * 1.4, None)).collect();
        spawn(&mut commands, &mut meshes, merge(parts), &assets.active, TeamMark);
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
        assets.team_key = u64::MAX;
    }
}
