//! Map overlays for the kiosk: the wind arrow on the terrain, and the rule
//! that 3D interface marks cast and take no shadows. The orders' signs are in
//! `kiosk::marks`.
//!
//! Everything sits well above the canopy (finding 13: a correct overlay
//! under the plants is seen never), is unlit and glowing so the bloom pass
//! catches it, and is rebuilt from `Sim` state with no latch of its own — so a
//! restart needs only the key cleared (finding 21).

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::Pos;

use crate::frame;
use crate::retro;
use crate::retro::RetroMaterial;

use crate::sim::Sim;

const WIND_LIFT_M: f32 = 40.0;

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

