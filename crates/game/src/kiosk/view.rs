//! The kiosk camera: a miniature seen from above, never lost.
//!
//! Pitch 45-55 degrees, yaw within +-30, zoom and pan clamped to the town
//! (spec §8). Replaces `camera::controls`, which is a free orbit. Any mouse
//! input cancels the scripted fly-in.

use bevy::ecs::system::SystemParam;
use bevy::input::gestures::{PinchGesture, RotationGesture};
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use scenario::Pos;

use super::{Kiosk, Phase};
use crate::camera::OrbitCamera;
use crate::sim::Sim;

const MIN_PITCH: f32 = -1.30;
const MAX_PITCH: f32 = -0.55;
const MAX_YAW: f32 = 1.75;
const MIN_DIST: f32 = 90.0;
const MAX_DIST: f32 = 4600.0;
const PAN_RADIUS_M: f32 = 1500.0;
const PLAY_DIST: f32 = 1900.0;
const FLY_IN_S: f32 = 4.0;

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The places the frame has to hold: every district and the fire's origin.
fn frame_points(sim: &Sim, kiosk: &Kiosk) -> Vec<Pos> {
    let mut pts: Vec<Pos> = kiosk.referee.as_ref().map(|r| r.districts.iter().map(|d| d.centre).collect()).unwrap_or_default();
    if pts.is_empty() {
        pts = sim.agents.households.iter().map(|h| h.home).collect();
    }
    pts.push(kiosk.spec.ignition);
    pts
}

/// What the camera looks at: the middle of the districts and the fire.
fn home_focus(sim: &Sim, kiosk: &Kiosk) -> Vec3 {
    if let Some((x, y)) = shot_focus() {
        let p = Pos { x, y };
        return crate::frame::to_bevy(p, sim.scenario.terrain.height_at(p));
    }
    let pts = frame_points(sim, kiosk);
    let (x0, x1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    // Nudged south: the chips stand above their districts and the action bar
    // takes the bottom of the screen, so the frame's centre sits a little low.
    let p = Pos { x: (x0 + x1) * 0.5, y: (y0 + y1) * 0.5 - 60.0 };
    crate::frame::to_bevy(p, sim.scenario.terrain.height_at(p))
}

/// Close enough that the houses read, far enough to hold every district.
fn play_dist(sim: &Sim, kiosk: &Kiosk) -> f32 {
    let f = crate::frame::to_world(home_focus(sim, kiosk));
    let pts = frame_points(sim, kiosk);
    let (ex, ey) = pts.iter().fold((0.0f32, 0.0f32), |(a, b), p| (a.max((p.x - f.x).abs()), b.max((p.y - f.y).abs())));
    // The screen is wider than tall and tilted: north-south extent costs more.
    (ex.max(ey * 1.35) * 2.3 + 500.0).clamp(1300.0, 4200.0)
}

#[derive(Default)]
pub struct Drag {
    left: bool,
    right: bool,
    middle: bool,
}

#[derive(SystemParam)]
pub struct ViewInput<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    motion: EventReader<'w, 's, MouseMotion>,
    wheel: EventReader<'w, 's, MouseWheel>,
    pinch: EventReader<'w, 's, PinchGesture>,
    rotation: EventReader<'w, 's, RotationGesture>,
}

fn pan(orbit: &mut OrbitCamera, delta: Vec2, viewport_height: f32) {
    let scale = orbit.distance * 0.83 / viewport_height.max(1.0);
    let rot = Quat::from_rotation_y(orbit.yaw);
    let tilt = orbit.pitch.sin().abs().max(0.3);
    orbit.focus += rot * Vec3::new(-delta.x, 0.0, -delta.y / tilt) * scale;
}

#[allow(clippy::too_many_arguments)]
pub fn camera(
    kiosk: Res<Kiosk>,
    sim: Res<Sim>,
    focus: Res<crate::ui::UiFocus>,
    order: Res<crate::command::OrderTool>,
    mut input: ViewInput,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<(&mut OrbitCamera, &mut Transform, &Camera, &mut bevy::core_pipeline::dof::DepthOfFieldSettings)>,
    time: Res<Time>,
    mut drag: Local<Drag>,
    mut released: Local<Option<(Phase, f32)>>,
) {
    let delta = input.motion.read().fold(Vec2::ZERO, |s, e| s + e.delta);
    let shift = input.keys.pressed(KeyCode::ShiftLeft) || input.keys.pressed(KeyCode::ShiftRight);
    let zoom_modifier = input.keys.pressed(KeyCode::ControlLeft) || input.keys.pressed(KeyCode::ControlRight)
        || input.keys.pressed(KeyCode::SuperLeft) || input.keys.pressed(KeyCode::SuperRight);
    let mut scroll = 0.0;
    let mut trackpad = Vec2::ZERO;
    for event in input.wheel.read() {
        match event.unit {
            MouseScrollUnit::Line => scroll += event.y,
            MouseScrollUnit::Pixel if zoom_modifier => scroll += event.y / 50.0,
            MouseScrollUnit::Pixel => trackpad += Vec2::new(event.x, event.y),
        }
    }
    let pinch: f32 = input.pinch.read().map(|e| e.0).sum();
    let rotation: f32 = input.rotation.read().map(|e| e.0).sum();
    let buttons = &input.buttons;
    let Ok((mut orbit, mut tf, camera, mut dof)) = query.get_single_mut() else { return };
    let home = home_focus(&sim, &kiosk);
    let t = kiosk.phase_t;
    let pd = play_dist(&sim, &kiosk) * shot_zoom();
    // A lower view lets the hill village, tree silhouettes and layered ridges
    // read as a landscape; analytical stages retain their overhead framing.
    let scene_pitch = if sim.scenario.metadata.id == "demo_borgo" { -0.68 } else { -0.86 };

    let window = windows.get_single().ok();
    let over_map = window.is_some_and(|w| w.focused)
        && !focus.pointer
        && window.and_then(|w| crate::pick::cursor_position(camera, w)).is_some();
    if buttons.just_pressed(MouseButton::Left) {
        // Shift-drag rotates even while choosing an order, on one-button trackpads.
        drag.left = over_map && (!order.is_armed() || shift);
    }
    if buttons.just_pressed(MouseButton::Right) {
        drag.right = over_map;
    }
    if buttons.just_pressed(MouseButton::Middle) {
        drag.middle = over_map;
    }
    drag.left &= buttons.pressed(MouseButton::Left) && window.is_some_and(|w| w.focused);
    drag.right &= buttons.pressed(MouseButton::Right) && window.is_some_and(|w| w.focused);
    drag.middle &= buttons.pressed(MouseButton::Middle) && window.is_some_and(|w| w.focused);

    // Only a gesture that actually controls the map cancels its scripted view.
    if kiosk.phase_t < 0.05 {
        *released = None;
    }
    let touched = delta.length_squared() > 0.0 && (drag.left || drag.right || drag.middle)
        || over_map && (scroll != 0.0 || trackpad != Vec2::ZERO || pinch != 0.0 || rotation != 0.0);
    if touched {
        *released = Some((kiosk.phase, t));
    }
    let user_has_it = released.map(|(p, _)| p) == Some(kiosk.phase);

    match kiosk.phase {
        Phase::Attract => {
            orbit.focus = home;
            orbit.yaw = 0.45 * (time.elapsed_seconds() * 0.07).sin();
            orbit.pitch = scene_pitch;
            orbit.distance = pd;
        }
        Phase::Briefing if !user_has_it && t < FLY_IN_S => {
            let k = ease(t / FLY_IN_S);
            orbit.focus = home;
            orbit.yaw = -0.45 * (1.0 - k);
            orbit.pitch = -1.15 + (1.15 + scene_pitch) * k;
            orbit.distance = 4600.0 + (pd - 4600.0) * k;
        }
        Phase::Outcome if !user_has_it => {
            // The result panel takes the right half: slide the burnt town into
            // the left half, and pull back a little.
            let right = Quat::from_rotation_y(orbit.yaw) * Vec3::X;
            let target = home + right * 0.30 * pd;
            let k = (time.delta_seconds() * 2.5).min(1.0);
            orbit.focus = orbit.focus.lerp(target, k);
            orbit.distance += (pd * 1.15 - orbit.distance) * k;
            orbit.pitch += (-0.9 - orbit.pitch) * k;
            orbit.yaw += (0.0 - orbit.yaw) * k;
        }
        _ => {
            let height = camera.logical_viewport_size().map_or(1000.0, |s| s.y);
            if (drag.left && !shift) || drag.middle {
                pan(&mut orbit, delta, height);
            }
            if drag.right || (drag.left && shift) {
                orbit.yaw -= delta.x * 0.005;
                orbit.pitch -= delta.y * 0.004;
            }
            if over_map {
                // macOS delivers two-finger scrolling in pixels, including x.
                // Keep mouse wheel zoom; trackpad pinch provides native zoom.
                if shift {
                    orbit.yaw -= trackpad.x * 0.005;
                    orbit.pitch -= trackpad.y * 0.004;
                } else {
                    pan(&mut orbit, trackpad, height);
                }
                orbit.yaw += rotation.to_radians();
                orbit.distance *= (-scroll.clamp(-6.0, 6.0) * 0.1 - pinch.clamp(-1.0, 1.0)).exp();
            }
            // First frame of play after the fly-in: settle on the home framing.
            if kiosk.phase == Phase::Play && t < 0.05 && !user_has_it {
                orbit.focus = home;
                orbit.yaw = 0.0;
                orbit.pitch = scene_pitch;
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
