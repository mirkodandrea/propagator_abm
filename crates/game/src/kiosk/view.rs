//! The kiosk camera: a miniature seen from above, never lost.
//!
//! Pitch 45-55 degrees, yaw within +-30, zoom and pan clamped to the town
//! (spec §8). Replaces `camera::controls`, which is a free orbit. Any mouse
//! input cancels the scripted fly-in.

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use scenario::Pos;

use super::{Kiosk, Phase};
use crate::camera::OrbitCamera;
use crate::sim::Sim;

const MIN_PITCH: f32 = -0.96;
const MAX_PITCH: f32 = -0.78;
const MAX_YAW: f32 = 0.52;
const MIN_DIST: f32 = 800.0;
const MAX_DIST: f32 = 4600.0;
const PAN_RADIUS_M: f32 = 1500.0;
const PLAY_DIST: f32 = 1900.0;
const FLY_IN_S: f32 = 8.0;

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What the camera looks at: the middle of town, nudged toward the fire.
fn home_focus(sim: &Sim, kiosk: &Kiosk) -> Vec3 {
    if let Some((x, y)) = shot_focus() {
        let p = Pos { x, y };
        return crate::frame::to_bevy(p, sim.scenario.terrain.height_at(p));
    }
    // The households' centroid, not the window's: Porto's town sits at one edge
    // and framing the middle of the map put it under the HUD (playtest §16 #4).
    let hs = &sim.agents.households;
    let n = hs.len().max(1) as f32;
    let centre = Pos { x: hs.iter().map(|h| h.home.x).sum::<f32>() / n, y: hs.iter().map(|h| h.home.y).sum::<f32>() / n };
    let ig = kiosk.spec.ignition;
    let p = Pos { x: centre.x * 0.5 + ig.x * 0.5, y: centre.y * 0.5 + ig.y * 0.5 };
    // Nudged south, so the town clears the action bar at the bottom of the screen.
    crate::frame::to_bevy(Pos { x: p.x, y: p.y - 40.0 }, sim.scenario.terrain.height_at(p))
}

/// Far enough to hold both the town and the fire between the HUD bands.
fn play_dist(sim: &Sim, kiosk: &Kiosk) -> f32 {
    let f = crate::frame::to_world(home_focus(sim, kiosk));
    let ig = kiosk.spec.ignition;
    let d = |x: f32, y: f32| ((x - f.x).powi(2) + (y - f.y).powi(2)).sqrt();
    let mut rs: Vec<f32> = sim.agents.households.iter().map(|h| d(h.home.x, h.home.y)).collect();
    rs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let r95 = rs.get(rs.len() * 95 / 100).copied().unwrap_or(0.0);
    (r95.max(d(ig.x, ig.y)) * 3.4).clamp(PLAY_DIST * 0.8, 4400.0)
}

#[derive(Default)]
pub struct Drag {
    left: bool,
    right: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn camera(
    kiosk: Res<Kiosk>,
    sim: Res<Sim>,
    focus: Res<crate::ui::UiFocus>,
    order: Res<crate::command::OrderTool>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<(&mut OrbitCamera, &mut Transform, &Camera, &mut bevy::core_pipeline::dof::DepthOfFieldSettings)>,
    time: Res<Time>,
    mut drag: Local<Drag>,
    mut released: Local<Option<(Phase, f32)>>,
) {
    let delta = motion.read().fold(Vec2::ZERO, |s, e| s + e.delta);
    let scroll: f32 = wheel
        .read()
        .map(|e| match e.unit {
            MouseScrollUnit::Line => e.y,
            MouseScrollUnit::Pixel => e.y / 50.0,
        })
        .sum();
    let Ok((mut orbit, mut tf, camera, mut dof)) = query.get_single_mut() else { return };
    let home = home_focus(&sim, &kiosk);
    let t = kiosk.phase_t;
    let pd = play_dist(&sim, &kiosk) * shot_zoom();

    // The user took the wheel: remember it for the rest of this phase.
    let touched = delta.length_squared() > 0.0 && (buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right))
        || scroll != 0.0;
    if touched && released.map(|(p, _)| p) != Some(kiosk.phase) {
        *released = Some((kiosk.phase, t));
    }
    let user_has_it = released.map(|(p, _)| p) == Some(kiosk.phase);
    if kiosk.phase_t < 0.05 {
        *released = None;
    }

    match kiosk.phase {
        Phase::Attract => {
            orbit.focus = home;
            orbit.yaw = 0.45 * (time.elapsed_seconds() * 0.07).sin();
            orbit.pitch = -0.86;
            orbit.distance = pd;
        }
        Phase::Briefing if !user_has_it && t < FLY_IN_S => {
            let k = ease(t / FLY_IN_S);
            // The card takes the bottom third: lift the town clear of it.
            orbit.focus = home + Vec3::new(0.0, 0.0, 0.16 * pd * k);
            orbit.yaw = -0.45 * (1.0 - k);
            orbit.pitch = -1.15 + (1.15 - 0.86) * k;
            orbit.distance = 4600.0 + (pd - 4600.0) * k;
        }
        Phase::Outcome | Phase::Compare if !user_has_it => {
            // The result card takes the lower three quarters: lift the burnt
            // ground into the strip above it, and pull back a little.
            let target = home + Vec3::new(0.0, 0.0, 0.22 * pd);
            let k = (time.delta_seconds() * 2.5).min(1.0);
            orbit.focus = orbit.focus.lerp(target, k);
            orbit.distance += (pd * 1.25 - orbit.distance) * k;
            orbit.pitch += (-0.86 - orbit.pitch) * k;
            orbit.yaw += (0.0 - orbit.yaw) * k;
        }
        _ => {
            let window = windows.get_single().ok();
            let over_map = window.is_some_and(|w| w.focused)
                && !focus.pointer
                && window.and_then(|w| crate::pick::cursor_position(camera, w)).is_some();
            let left_free = !order.is_armed();
            if buttons.just_pressed(MouseButton::Left) {
                drag.left = over_map && left_free;
            }
            if buttons.just_pressed(MouseButton::Right) {
                drag.right = over_map;
            }
            if !buttons.pressed(MouseButton::Left) {
                drag.left = false;
            }
            if !buttons.pressed(MouseButton::Right) {
                drag.right = false;
            }
            if drag.left {
                orbit.yaw -= delta.x * 0.004;
                orbit.pitch -= delta.y * 0.003;
            }
            if drag.right {
                let h = camera.logical_viewport_size().map_or(1000.0, |s| s.y).max(1.0);
                let scale = orbit.distance * 0.83 / h;
                let rot = Quat::from_rotation_y(orbit.yaw);
                let tilt = orbit.pitch.sin().abs().max(0.3);
                orbit.focus += rot * Vec3::new(-delta.x, 0.0, delta.y / tilt) * scale;
            }
            if over_map && scroll != 0.0 {
                orbit.distance *= (-scroll.clamp(-6.0, 6.0) * 0.1).exp();
            }
            // First frame of play after the fly-in: settle on the home framing.
            if kiosk.phase == Phase::Play && t < 0.05 && !user_has_it {
                orbit.focus = home;
                orbit.yaw = 0.0;
                orbit.pitch = -0.86;
                orbit.distance = pd;
            }
        }
    }

    orbit.yaw = orbit.yaw.clamp(-MAX_YAW, MAX_YAW);
    orbit.pitch = orbit.pitch.clamp(MIN_PITCH, MAX_PITCH);
    orbit.distance = orbit.distance.clamp(MIN_DIST * shot_zoom().min(1.0), MAX_DIST);
    let off = Vec2::new(orbit.focus.x - home.x, orbit.focus.z - home.z);
    if off.length() > PAN_RADIUS_M {
        let c = off.normalize() * PAN_RADIUS_M;
        orbit.focus.x = home.x + c.x;
        orbit.focus.z = home.z + c.y;
    }
    orbit.focus.y = sim.scenario.terrain.height_at(crate::frame::to_world(orbit.focus));

    let dir = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        -orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    );
    tf.translation = orbit.focus + dir * orbit.distance;
    let ground = sim.scenario.terrain.height_at(crate::frame::to_world(tf.translation));
    tf.translation.y = tf.translation.y.max(ground + 25.0);
    tf.look_at(orbit.focus, Vec3::Y);
    dof.focal_distance = tf.translation.distance(orbit.focus);
}

/// Screen angle, clockwise from straight up, of a world bearing.
pub fn screen_angle(bearing_deg: f32, yaw: f32) -> f32 {
    bearing_deg.to_radians() + yaw
}

/// `KIOSK_SHOT_ZOOM=<k>` scales the play distance, to photograph close-ups.
fn shot_zoom() -> f32 {
    static Z: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *Z.get_or_init(|| std::env::var("KIOSK_SHOT_ZOOM").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0))
}

/// `KIOSK_SHOT_FOCUS=x,y` (world metres) for close-ups of a particular place.
fn shot_focus() -> Option<(f32, f32)> {
    static F: std::sync::OnceLock<Option<(f32, f32)>> = std::sync::OnceLock::new();
    *F.get_or_init(|| {
        let v = std::env::var("KIOSK_SHOT_FOCUS").ok()?;
        let (a, b) = v.split_once(',')?;
        Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
    })
}
