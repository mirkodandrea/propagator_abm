//! The kiosk camera: a miniature seen from above, never lost.
//!
//! Pitch 45-55 degrees, yaw within +-30, zoom and pan clamped to the town
//! (spec §8). Any mouse input takes over from the home framing.

use bevy::ecs::system::SystemParam;
use bevy::input::gestures::{PinchGesture, RotationGesture};
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use scenario::Pos;

use super::{Kiosk, Phase};
use rocca::Game;
use crate::camera::OrbitCamera;
use crate::sim::Sim;

const MIN_PITCH: f32 = -1.30;
const MAX_PITCH: f32 = -0.55;
const MAX_YAW: f32 = 1.75;
const MIN_DIST: f32 = 90.0;
const MAX_DIST: f32 = 6000.0;
const PAN_RADIUS_M: f32 = 1500.0;

/// The places the frame has to hold: every home of every district, and where
/// the fire started. The distance is fitted to them (`fit_view`), so a fire
/// away from the middle widens the frame rather than pushing a district off it.
fn frame_points(game: &Game) -> Vec<Pos> {
    let mut pts: Vec<Pos> = game.districts.iter().flat_map(|d| d.households.iter().map(|&i| game.agents.households[i].home)).collect();
    pts.push(game.case.ignition());
    pts
}

/// What the camera looks at: the middle of the districts and the fire.
fn home_focus(sim: &Sim) -> Vec3 {
    if let Some((x, y)) = shot_focus() {
        let p = Pos { x, y };
        return crate::frame::to_bevy(p, sim.scn.terrain.height_at(p));
    }
    let pts = frame_points(sim);
    let (x0, x1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    let p = Pos { x: (x0 + x1) * 0.5, y: (y0 + y1) * 0.5 };
    crate::frame::to_bevy(p, sim.scn.terrain.height_at(p))
}

/// Where the camera stands for a focus, yaw, pitch and distance.
fn eye(focus: Vec3, yaw: f32, pitch: f32, distance: f32) -> Transform {
    let dir = Vec3::new(yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos());
    Transform::from_translation(focus + dir * distance).looking_at(focus, Vec3::Y)
}

/// The closest distance at which every home lands in the part of the screen
/// the interface leaves free: below the top band with room for a chip over
/// it, above the button and the panels. `None` until the camera has a size.
fn fit_distance(camera: &Camera, sim: &Sim, focus: Vec3, yaw: f32, pitch: f32) -> Option<f32> {
    let size = camera.logical_viewport_size()?;
    let pts: Vec<Vec3> = frame_points(sim).into_iter().map(|p| crate::frame::to_bevy(p, sim.scn.terrain.height_at(p))).collect();
    let fits = |d: f32| {
        let gt = GlobalTransform::from(eye(focus, yaw, pitch, d));
        pts.iter().all(|p| {
            camera.world_to_viewport(&gt, *p).is_some_and(|s| {
                s.x >= 0.10 * size.x && s.x <= 0.90 * size.x && s.y >= 0.30 * size.y && s.y <= 0.78 * size.y
            })
        })
    };
    // Not projected yet (first frames): every test would fail; wait.
    camera.world_to_viewport(&GlobalTransform::default(), Vec3::NEG_Z)?;
    let (mut lo, mut hi) = (MIN_DIST * 4.0, MAX_DIST);
    if !fits(hi) {
        return Some(hi);
    }
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if fits(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
}

/// The home view: the yaw (within ±50°) that brings the districts closest,
/// and that distance. The territory's places lie on a diagonal; turned, they
/// use the width of the screen.
fn fit_view(camera: &Camera, sim: &Sim, focus: Vec3, pitch: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for k in -6..=6 {
        let yaw = k as f32 * 0.15;
        let d = fit_distance(camera, sim, focus, yaw, pitch)?;
        if best.is_none_or(|(_, b)| d < b * 0.97) {
            best = Some((yaw, d));
        }
    }
    best
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
    mut input: ViewInput,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<(&mut OrbitCamera, &mut Transform, &Camera, &mut bevy::core_pipeline::dof::DepthOfFieldSettings)>,
    time: Res<Time>,
    mut drag: Local<Drag>,
    mut released: Local<Option<(Phase, f32)>>,
    mut settled: Local<u64>,
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
    let home = home_focus(&sim);
    let t = kiosk.phase_t;
    let scene_pitch = -0.80;

    let window = windows.get_single().ok();
    let over_map = window.is_some_and(|w| w.focused)
        && !focus.pointer
        && window.and_then(|w| crate::pick::cursor_position(camera, w)).is_some();
    if buttons.just_pressed(MouseButton::Left) {
        // Shift-drag rotates even while choosing an order, on one-button trackpads.
        drag.left = over_map;
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

    let _ = (t, &time);
    match kiosk.phase {
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
            // A new game: settle on the home framing once. (A game starts one
            // step in: `rocca::Game::new` puts the ignition into the state.)
            if *settled != sim.generation.max(1) && sim.time_s() <= rocca::STEP_S && !user_has_it {
                if let Some((yaw, d)) = fit_view(camera, &sim, home, scene_pitch) {
                    *settled = sim.generation.max(1);
                    orbit.focus = home;
                    orbit.yaw = yaw;
                    orbit.pitch = scene_pitch;
                    orbit.distance = d * shot_zoom();
                }
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
    orbit.focus.y = sim.scn.terrain.height_at(crate::frame::to_world(orbit.focus));

    let dir = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        -orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    );
    tf.translation = orbit.focus + dir * orbit.distance;
    let ground = sim.scn.terrain.height_at(crate::frame::to_world(tf.translation));
    tf.translation.y = tf.translation.y.max(ground + 25.0);
    tf.look_at(orbit.focus, Vec3::Y);
    dof.focal_distance = tf.translation.distance(orbit.focus);
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
