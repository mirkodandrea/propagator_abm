//! Map overlays for the kiosk: the wind arrow on the terrain, household
//! beacons, spot-fire rings, closed-road markers.
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

#[derive(Resource)]
pub struct OverlayAssets {
    unlit_white: Handle<RetroMaterial>,
    ring_red: Handle<RetroMaterial>,
    ring_orange: Handle<RetroMaterial>,
    beacons: Handle<Mesh>,
    key: (usize, usize, i64),
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
    let ring_red = mat([1.0, 0.25, 0.2], 0.9, 2.0);
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
    commands.insert_resource(OverlayAssets { unlit_white, ring_red, ring_orange, beacons, key: (usize::MAX, 0, 0) });
}

pub fn reset(mut restarted: EventReader<crate::sim::SimRestarted>, mut assets: ResMut<OverlayAssets>) {
    if !restarted.is_empty() {
        restarted.clear();
        assets.key = (usize::MAX, 0, 0);
    }
}

/// The wind arrow: lies over the terrain upwind of the fire, pointing the way
/// the fire will be pushed. Every frame; cheap.
pub fn update_wind(sim: Res<Sim>, mut q: Query<&mut Transform, With<WindArrow>>) {
    let Ok(mut tf) = q.get_single_mut() else { return };
    let w = sim.fire.weather();
    let from = (w.wind_dir_deg as f32).to_radians();
    let (pos, _) = crate::terrain_mesh::cell_ground(&sim.scenario, sim.ignition.centre);
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
    let ground = sim.scenario.terrain.height_at(up);
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
    for h in &sim.agents.households {
        let color = match h.status {
            Status::Warned | Status::Preparing => [1.0, 0.78, 0.1],
            Status::Trapped | Status::Defending => [1.0, 0.18, 0.12],
            Status::Casualty => [0.2, 0.2, 0.2],
            Status::Evacuating => [0.3, 0.7, 1.0],
            _ => continue,
        };
        let g = sim.scenario.terrain.height_at(h.home);
        octa(&mut b, frame::to_bevy(h.home, g + BEACON_LIFT_M), 6.0, [color[0] * 2.0, color[1] * 2.0, color[2] * 2.0]);
    }
    let n = b.pos.len();
    if let Some(m) = meshes.get_mut(&assets.beacons) {
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, b.nrm);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, b.col);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n]);
        m.insert_indices(Indices::U32(b.idx));
    }

    // Spot fires and closures change rarely: rebuild on a changed key.
    let now = sim.time_s() as f32;
    let spots: Vec<Pos> = sim.agents.spot_fires().spots().map(|s| s.pos).collect();
    let closures: Vec<(Pos, f32)> = sim
        .agents
        .closures()
        .iter()
        .filter(|c| c.active_at(now))
        .map(|c| (c.centre, c.radius_m))
        .collect();
    let sig = spots.first().map_or(0, |s| (s.x as i64) ^ ((s.y as i64) << 20));
    let key = (spots.len(), closures.len(), sig);
    if key == assets.key {
        return;
    }
    assets.key = key;
    for e in &old {
        commands.entity(e).despawn_recursive();
    }
    for &s in &spots {
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(ring_mesh(&sim.scenario, s, 90.0)),
                material: assets.ring_red.clone(),
                ..default()
            },
            DynamicMarker,
        ));
        // "!": a bar and a dot.
        let g = sim.scenario.terrain.height_at(s) + PIN_LIFT_M;
        let mut t = crate::toy::Toy::default();
        t.cuboid(Vec3::new(0.0, 22.0, 0.0), Vec3::new(5.0, 14.0, 5.0), [1.0, 0.9, 0.9]);
        t.cuboid(Vec3::new(0.0, 2.0, 0.0), Vec3::new(5.5, 5.5, 5.5), [1.0, 0.9, 0.9]);
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(t.finish()),
                material: assets.ring_red.clone(),
                transform: Transform::from_translation(frame::to_bevy(s, g)).with_scale(Vec3::splat(2.0)),
                ..default()
            },
            DynamicMarker,
        ));
    }
    for &(centre, radius) in &closures {
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(ring_mesh(&sim.scenario, centre, radius.max(60.0))),
                material: assets.ring_orange.clone(),
                ..default()
            },
            DynamicMarker,
        ));
        let g = sim.scenario.terrain.height_at(centre) + PIN_LIFT_M;
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
