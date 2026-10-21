//! Where each team is assigned: a flat semi-transparent ring on the ground
//! at the road point of its hex (at its station before the first order),
//! with the egui badge floating over it. The badge is the order itself: in pause the
//! player drags it to another hex and lets it go there (user, 2026-10-14: no
//! «Conferma»). The vehicles drive on their own; the badge says only where
//! they are sent.
//!
//! The marks keep about the same size on screen at any zoom: [`scale`] grows
//! with the distance from the camera. The egui handle that is dragged is the
//! badge (`ui::team_flags`), so the camera does not pan under it.

use bevy::prelude::*;
use scenario::Pos;

use super::{Kiosk, Phase};
use crate::sim::Sim;
use bevy_egui::egui::Color32;

/// How high over the ground the team's handle floats, metres at scale 1.
pub const LIFT_M: f32 = 40.0;
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
    ring: Handle<Mesh>,
    /// One material per team colour.
    teams: Vec<Handle<StandardMaterial>>,
}

/// See-through and bright: the rings must read on any ground without hiding it.
fn glass(c: Color32) -> StandardMaterial {
    let [r, g, b, _] = c.to_array();
    let l = Color::srgb_u8(r, g, b).to_linear();
    StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::new(l.red, l.green, l.blue, 0.6)),
        emissive: LinearRgba::rgb(l.red * 1.4, l.green * 1.4, l.blue * 1.4),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let ring = Mesh::from(Annulus::new(RING_R_M * 0.75, RING_R_M)).rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
    commands.insert_resource(PinAssets {
        ring: meshes.add(ring),
        teams: super::marks::TEAM_COLOURS.iter().map(|c| materials.add(glass(Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)))).collect(),
    });
}

/// One ring per team, placed every frame from the game and the drag.
pub fn update(
    mut commands: Commands,
    sim: Res<Sim>,
    kiosk: Res<Kiosk>,
    assets: Res<PinAssets>,
    cameras: Query<&GlobalTransform, With<crate::camera::OrbitCamera>>,
    mut pins: Query<(Entity, &Pin, &mut Transform, &mut Visibility)>,
) {
    let Ok(cam) = cameras.single() else { return };
    let have = pins.iter().count();
    if have != sim.teams.len() {
        for (e, ..) in &pins {
            commands.entity(e).despawn();
        }
        for t in 0..sim.teams.len() {
            let colour = assets.teams[t % assets.teams.len()].clone();
            commands
                .spawn((Transform::default(), Visibility::Hidden, Pin(t)))
                .with_children(|c| {
                    for mesh in [&assets.ring] {
                        c.spawn((
                            (Mesh3d(mesh.clone()), MeshMaterial3d(colour.clone())),
                            bevy::light::NotShadowCaster,
                            bevy::light::NotShadowReceiver,
                        ));
                    }
                });
        }
        return;
    }
    let playing = matches!(kiosk.phase, Phase::Tutorial | Phase::Gioco);
    let eye = cam.translation();
    let feet = feet(&kiosk, &sim, eye, cam.right().into());
    
    for (_, pin, mut tf, mut vis) in &mut pins {
        let Some(Some((p, s))) = feet.get(pin.0).copied().filter(|_| playing) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        let ground = crate::frame::to_bevy(p, sim.scn.terrain.height_at(p) + crate::rings::RING_LIFT_M);
        *tf = Transform::from_translation(ground).with_scale(Vec3::splat(s));
    }
}
