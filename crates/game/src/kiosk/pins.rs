//! Where each team is assigned: a semi-transparent cone, point down,
//! floating and bobbing over the road point of its hex (over its station
//! before the first order). The cone is the order itself: in pause the
//! player drags it to another hex and lets it go there (user, 2026-10-14: no
//! «Conferma»). The vehicles drive on their own; the cone says only where
//! they are sent.
//!
//! The cones keep about the same size on screen at any zoom: [`scale`] grows
//! with the distance from the camera. The egui handle that is dragged sits on
//! the cone (`ui::team_flags`), so the camera does not pan under it.

use bevy::prelude::*;
use scenario::Pos;

use super::{Kiosk, Phase};
use crate::retro::{self, RetroMaterial};
use crate::sim::Sim;
use bevy_egui::egui::Color32;

/// The cone at scale 1, metres: its tip floats [`LIFT_M`] over the ground.
pub const CONE_H_M: f32 = 120.0;
pub const CONE_R_M: f32 = 55.0;
pub const LIFT_M: f32 = 40.0;
/// How far the cone bobs up and down, metres at scale 1.
const BOB_M: f32 = 12.0;
const RING_R_M: f32 = 45.0;
/// At this distance from the camera a cone is drawn at scale 1.
const UNIT_DIST_M: f32 = 2200.0;
/// Cones on the same spot stand side by side, this far apart at scale 1.
const SPREAD_M: f32 = 130.0;

#[derive(Component)]
pub struct Pin(pub usize);

/// How much a flag at `at` (Bevy space) is scaled, seen from `eye`.
pub fn scale(eye: Vec3, at: Vec3) -> f32 {
    (eye.distance(at) / UNIT_DIST_M).clamp(0.12, 6.0)
}

/// Where team `t`'s flag stands on the ground, before spreading: under the
/// pointer while dragged, else its hex's road point, else its station.
/// `None` for a team out of service.
fn spot(k: &Kiosk, sim: &Sim, t: usize) -> Option<Pos> {
    let team = &sim.teams[t];
    let u = &sim.crews.units[team.unit];
    if u.state == abm::suppression::UnitState::Lost {
        return None;
    }
    if let Some((d, p)) = k.dragging {
        if d == t {
            // over a hex it can go to: the flag snaps to its road point
            return Some(match &k.move_plan {
                Some(Ok(plan)) if Some(plan.hex.at) == k.move_hex && rocca::Axial::of(p) == plan.hex.at => plan.hex.target,
                _ => p,
            });
        }
    }
    Some(team.hex.and_then(|h| sim.hexes.get(h)).map_or(u.base, |h| h.target))
}

/// Every flag's foot (world metres), with the scale it is drawn at; flags
/// sharing a spot are spread across the view.
pub fn feet(k: &Kiosk, sim: &Sim, eye: Vec3, right: Vec3) -> Vec<Option<(Pos, f32)>> {
    let spots: Vec<Option<Pos>> = (0..sim.teams.len()).map(|t| spot(k, sim, t)).collect();
    (0..spots.len())
        .map(|t| {
            let p = spots[t]?;
            let s = scale(eye, crate::frame::to_bevy(p, sim.scn.terrain.height_at(p)));
            let dragged = k.dragging.is_some_and(|(d, _)| d == t);
            let same: Vec<usize> = (0..spots.len())
                .filter(|&o| !dragged && !k.dragging.is_some_and(|(d, _)| d == o))
                .filter(|&o| spots[o].is_some_and(|q| (q.x - p.x).hypot(q.y - p.y) < 30.0))
                .collect();
            let i = same.iter().position(|&o| o == t).unwrap_or(0) as f32;
            let off = (i - (same.len().max(1) - 1) as f32 * 0.5) * SPREAD_M * s;
            // `right` is in Bevy space (x, z); world y is -z
            Some((Pos { x: p.x + right.x * off, y: p.y - right.z * off }, s))
        })
        .collect()
}

#[derive(Resource)]
pub struct PinAssets {
    cone: Handle<Mesh>,
    ring: Handle<Mesh>,
    vvf: Handle<RetroMaterial>,
    aib: Handle<RetroMaterial>,
}

/// A cone with its tip at the origin, opening upward to radius `r` at `h`,
/// closed by a lid.
fn cone_mesh(r: f32, h: f32) -> Mesh {
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    use bevy::render::render_asset::RenderAssetUsages;
    const N: u32 = 32;
    let mut pos = vec![[0.0, 0.0, 0.0], [0.0, h, 0.0]];
    let mut nor = vec![[0.0, -1.0, 0.0], [0.0, 1.0, 0.0]];
    for i in 0..N {
        let a = i as f32 / N as f32 * std::f32::consts::TAU;
        pos.push([r * a.cos(), h, r * a.sin()]);
        nor.push(Vec3::new(a.cos() * h, -r, a.sin() * h).normalize().into());
    }
    let mut idx = vec![];
    for i in 0..N {
        let (a, b) = (2 + i, 2 + (i + 1) % N);
        idx.extend([0, b, a, 1, a, b]);
    }
    let n = pos.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n]);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

/// See-through and bright: the cones must read on any ground without hiding it.
fn glass(c: Color32) -> RetroMaterial {
    let [r, g, b, _] = c.to_array();
    let l = Color::srgb_u8(r, g, b).to_linear();
    retro::material(
        StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::new(l.red, l.green, l.blue, 0.6)),
            emissive: LinearRgba::rgb(l.red * 1.4, l.green * 1.4, l.blue * 1.4),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        },
        false,
    )
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<RetroMaterial>>) {
    let ring = Mesh::from(Annulus::new(RING_R_M * 0.75, RING_R_M)).rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
    commands.insert_resource(PinAssets {
        cone: meshes.add(cone_mesh(CONE_R_M, CONE_H_M)),
        ring: meshes.add(ring),
        vvf: materials.add(glass(super::ui::VVF)),
        aib: materials.add(glass(super::ui::AIB)),
    });
}

/// One cone per team, placed every frame from the game and the drag.
pub fn update(
    mut commands: Commands,
    sim: Res<Sim>,
    kiosk: Res<Kiosk>,
    assets: Res<PinAssets>,
    time: Res<Time>,
    cameras: Query<&GlobalTransform, With<crate::camera::OrbitCamera>>,
    mut pins: Query<(Entity, &Pin, &mut Transform, &mut Visibility, &Children)>,
    mut parts: Query<&mut Transform, Without<Pin>>,
) {
    let Ok(cam) = cameras.get_single() else { return };
    let have = pins.iter().count();
    if have != sim.teams.len() {
        for (e, ..) in &pins {
            commands.entity(e).despawn_recursive();
        }
        for t in 0..sim.teams.len() {
            let colour = if sim.teams[t].volunteers { assets.aib.clone() } else { assets.vvf.clone() };
            commands
                .spawn((SpatialBundle { visibility: Visibility::Hidden, ..default() }, Pin(t)))
                .with_children(|c| {
                    for mesh in [&assets.cone, &assets.ring] {
                        c.spawn((
                            MaterialMeshBundle::<RetroMaterial> { mesh: mesh.clone(), material: colour.clone(), ..default() },
                            bevy::pbr::NotShadowCaster,
                            bevy::pbr::NotShadowReceiver,
                        ));
                    }
                });
        }
        return;
    }
    let playing = matches!(kiosk.phase, Phase::Tutorial | Phase::Gioco);
    let eye = cam.translation();
    let feet = feet(&kiosk, &sim, eye, cam.right().into());
    let t_s = time.elapsed_seconds();
    for (_, pin, mut tf, mut vis, children) in &mut pins {
        let Some(Some((p, s))) = feet.get(pin.0).copied().filter(|_| playing) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        // the ground ring stays put; the cone floats over it, bobbing
        let ground = crate::frame::to_bevy(p, sim.scn.terrain.height_at(p) + crate::rings::RING_LIFT_M);
        *tf = Transform::from_translation(ground).with_scale(Vec3::splat(s));
        let bob = LIFT_M + BOB_M * (t_s * 2.2 + pin.0 as f32 * 1.3).sin();
        if let Some(&cone) = children.first() {
            if let Ok(mut c) = parts.get_mut(cone) {
                c.translation.y = bob;
                c.rotation = Quat::from_rotation_y(t_s * 0.8);
            }
        }
    }
}
