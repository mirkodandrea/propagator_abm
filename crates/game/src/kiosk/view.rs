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

use super::{Kiosk, Phase, Subject};
use rocca::Game;
use crate::camera::OrbitCamera;
use crate::sim::Sim;

const MIN_PITCH: f32 = -1.30;
const MAX_PITCH: f32 = -0.55;
const MAX_YAW: f32 = 1.75;
const MIN_DIST: f32 = 90.0;
const MAX_DIST: f32 = 12000.0;
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
                s.x >= 0.08 * size.x && s.x <= 0.92 * size.x && s.y >= 0.18 * size.y && s.y <= 0.80 * size.y
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
    touches: Res<'w, Touches>,
}

fn pan(orbit: &mut OrbitCamera, delta: Vec2, viewport_height: f32) {
    let scale = orbit.distance * 0.83 / viewport_height.max(1.0);
    let rot = Quat::from_rotation_y(orbit.yaw);
    let tilt = orbit.pitch.sin().abs().max(0.3);
    orbit.focus += rot * Vec3::new(-delta.x, 0.0, -delta.y / tilt) * scale;
}

#[allow(clippy::too_many_arguments)]
pub fn camera(
    mut kiosk: ResMut<Kiosk>,
    sim: Res<Sim>,
    focus: Res<crate::ui::UiFocus>,
    mut input: ViewInput,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<(&mut OrbitCamera, &mut Transform, &mut Camera, &mut bevy::core_pipeline::dof::DepthOfFieldSettings)>,
    time: Res<Time>,
    mut drag: Local<Drag>,
    mut released: Local<Option<(Phase, f32)>>,
    mut settled: Local<u64>,
    mut home_view: Local<Option<(f32, f32)>>,
    mut refit: Local<(Option<UVec2>, u8)>,
    mut last_cursor: Local<Option<Vec2>>,
    mut tracked: Local<Option<(Vec2, f32)>>,
) {
    // Drag deltas from the cursor's own movement: `MouseMotion` (raw device
    // motion) does not arrive in the browser, and dragging the map did nothing.
    input.motion.clear();
    let cursor = windows.get_single().ok().and_then(|w| w.cursor_position());
    let delta = match (cursor, *last_cursor) {
        (Some(c), Some(l)) => c - l,
        _ => Vec2::ZERO,
    };
    *last_cursor = cursor;
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
    let Ok((mut orbit, mut tf, mut camera, mut dof)) = query.get_single_mut() else { return };
    // The map is drawn left of the side bar, so it is framed and centred in
    // what can be seen. A new size takes a frame to reach the projection:
    // the home view is fitted again two frames later.
    if let Some(w) = windows.get_single().ok() {
        let side = (kiosk.side_w * w.scale_factor()).round() as u32;
        let full = UVec2::new(w.physical_width(), w.physical_height());
        let size = UVec2::new(full.x.saturating_sub(side).max(1), full.y.max(1));
        if refit.0 != Some(size) {
            refit.0 = Some(size);
            refit.1 = 2;
            camera.viewport = (size != full).then(|| bevy::render::camera::Viewport { physical_position: UVec2::ZERO, physical_size: size, ..default() });
        }
    }
    let camera = &*camera;
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
    let touched = input.touches.iter().any(|t| t.delta() != Vec2::ZERO)
        || delta.length_squared() > 0.0 && (drag.left || drag.right || drag.middle)
        || over_map && (scroll != 0.0 || trackpad != Vec2::ZERO || pinch != 0.0 || rotation != 0.0);
    // the scripted game behind the title screen is nobody's to steer
    if touched && !kiosk.title {
        kiosk.shot = None;
        kiosk.follow = false;
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
            // Touch screens: one finger drags the map, two fingers pinch to zoom.
            // Not while a finger is on the interface.
            if !focus.pointer {
                let active: Vec<_> = input.touches.iter().collect();
                match active.as_slice() {
                    [t] => pan(&mut orbit, t.delta(), height),
                    [a, b] => {
                        let now = a.position().distance(b.position());
                        let before = (a.position() - a.delta()).distance(b.position() - b.delta());
                        if now > 1.0 && before > 1.0 {
                            orbit.distance *= before / now;
                        }
                    }
                    _ => {}
                }
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
                    kiosk.follow = true;
                    *tracked = None;
                    *home_view = Some((yaw, d));
                    orbit.focus = home;
                    orbit.yaw = yaw;
                    orbit.pitch = scene_pitch;
                    orbit.distance = d * shot_zoom();
                }
            }
            // the map's size changed (the side bar came or went): fit the
            // home view to it, and go there if the game has not begun
            if refit.1 > 0 {
                refit.1 -= 1;
                if refit.1 == 0 {
                    if let Some((yaw, d)) = fit_view(camera, &sim, home, scene_pitch) {
                        *home_view = Some((yaw, d));
                        if sim.time_s() <= rocca::STEP_S && !user_has_it && kiosk.shot.is_none() {
                            orbit.focus = home;
                            orbit.yaw = yaw;
                            orbit.pitch = scene_pitch;
                            orbit.distance = d * shot_zoom();
                        }
                    }
                }
            }
            // the on-screen zoom buttons
            if kiosk.zoom != 1.0 {
                orbit.distance *= kiosk.zoom;
                kiosk.zoom = 1.0;
            }
            // a move asked by the interface (the tour): eased, not a jump
            if let (Some(shot), Some((yaw, d))) = (kiosk.shot, *home_view) {
                let mut goal = shot.at.map_or(home, |p| crate::frame::to_bevy(p, sim.scn.terrain.height_at(p)));
                let off = Vec2::new(goal.x - home.x, goal.z - home.z).clamp_length_max(PAN_RADIUS_M);
                goal = Vec3::new(home.x + off.x, goal.y, home.z + off.y);
                let distance = (d * shot_zoom() * shot.zoom).clamp(MIN_DIST, MAX_DIST);
                let a = 1.0 - (-3.5 * time.delta_seconds()).exp();
                orbit.focus = orbit.focus.lerp(goal, a);
                orbit.distance += (distance - orbit.distance) * a;
                orbit.yaw += (yaw - orbit.yaw) * a;
                orbit.pitch += (scene_pitch - orbit.pitch) * a;
                let near = Vec2::new(orbit.focus.x - goal.x, orbit.focus.z - goal.z).length() < 2.0;
                if near && (orbit.distance / distance - 1.0).abs() < 0.01 && (orbit.yaw - yaw).abs() < 0.01 && (orbit.pitch - scene_pitch).abs() < 0.01 {
                    kiosk.shot = None;
                }
            }
            // Behind the title screen: a director, not a follower. A shot
            // lasts `SHOT_S`; its subject is picked when it begins, from what
            // the game shows then, and the camera glides to it slowly.
            if kiosk.title {
                if let Some((yaw0, d)) = *home_view {
                    let n = super::demo_shot(kiosk.demo_t);
                    if kiosk.demo_pick.0 != n {
                        kiosk.demo_pick = (n, pick_shot(&sim, n, kiosk.demo_pick.1));
                    }
                    let on = |p: Pos| crate::frame::to_bevy(p, sim.scn.terrain.height_at(p));
                    let front = fire_extent(&sim);
                    let wide = |front: Option<(Vec2, f32)>| match front {
                        Some((mid, span)) => (Vec3::new(mid.x, orbit.focus.y, mid.y), (span * 2.8).clamp(d * 0.5, d), scene_pitch, 0.0),
                        None => (home, d, scene_pitch, 0.0),
                    };
                    let (goal, dist, pitch, sway) = match kiosk.demo_pick.1 {
                        Subject::Front => match front {
                            Some((mid, _)) => (Vec3::new(mid.x, orbit.focus.y, mid.y), d * 0.32, -0.62, 0.6),
                            None => wide(front),
                        },
                        Subject::District(i) if i < sim.districts.len() => (on(sim.districts[i].centre), d * 0.26, -0.55, -0.5),
                        Subject::Crew(i) if i < sim.crews.units.len() => (on(sim.crews.units[i].pos), d * 0.17, -0.5, 0.7),
                        _ => wide(front),
                    };
                    let off = Vec2::new(goal.x - home.x, goal.z - home.z).clamp_length_max(PAN_RADIUS_M);
                    let goal = Vec3::new(home.x + off.x, goal.y, home.z + off.y);
                    // the goal itself is smoothed (a crew moves in jumps), then
                    // the camera eases to it: two slow stages, no sudden move
                    let dt = time.delta_seconds().min(0.1);
                    let (g, gd) = tracked.get_or_insert((Vec2::new(goal.x, goal.z), dist));
                    let s = 1.0 - (-0.9 * dt).exp();
                    *g += (Vec2::new(goal.x, goal.z) - *g) * s;
                    *gd += (dist - *gd) * s;
                    let a = 1.0 - (-0.5 * dt).exp();
                    orbit.focus = orbit.focus.lerp(Vec3::new(g.x, goal.y, g.y), a);
                    orbit.distance += (*gd - orbit.distance) * a;
                    orbit.pitch += (pitch - orbit.pitch) * a;
                    // always a slow drift of the angle on top of the shot's
                    let yaw = yaw0 + sway * 0.5 + 0.2 * (kiosk.demo_t * 0.08).sin();
                    orbit.yaw += (yaw - orbit.yaw) * (1.0 - (-0.35 * dt).exp());
                }
            // Time is running: follow the fire, slowly, keeping it framed.
            // The goal is smoothed first (the front's middle jumps as cells
            // catch), then the camera eases to it.
            } else if kiosk.follow && matches!(kiosk.phase, Phase::Esegui | Phase::Crisi) && kiosk.shot.is_none() && !user_has_it {
                if let (Some((_, d)), Some((mid, span))) = (*home_view, fire_extent(&sim)) {
                    let dt = time.delta_seconds().min(0.1);
                    let (t_mid, t_span) = tracked.get_or_insert((mid, span));
                    let s = 1.0 - (-0.6 * dt).exp();
                    *t_mid += (mid - *t_mid) * s;
                    *t_span += (span - *t_span) * s;
                    let goal_d = (*t_span * 2.8).clamp(d * 0.5, d);
                    let goal = Vec3::new(t_mid.x, orbit.focus.y, t_mid.y);
                    let a = 1.0 - (-1.2 * dt).exp();
                    orbit.focus = orbit.focus.lerp(goal, a);
                    orbit.distance += (goal_d - orbit.distance) * a;
                }
            }
            // «Vista iniziale»: back to the home view of this game.
            if kiosk.reset_view {
                kiosk.reset_view = false;
                if let Some((yaw, d)) = *home_view {
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

/// What the follow camera frames: the burning front and the districts it
/// is near (a home within `NEAR_M` of it). Returns the middle (Bevy x, z)
/// and the widest extent in metres. `None` if nothing burns.
pub(super) fn fire_extent(sim: &Sim) -> Option<(Vec2, f32)> {
    const NEAR_M: f32 = 1500.0;
    let cells = sim.fire.active_cells();
    if cells.is_empty() {
        return None;
    }
    let step = (cells.len() / 400).max(1);
    let fire: Vec<Pos> = cells.iter().step_by(step).map(|&c| sim.scn.world.centre_of(c)).collect();
    let mut pts = fire.clone();
    for d in &sim.districts {
        let homes = d.households.iter().map(|&i| sim.agents.households[i].home);
        let near = homes.clone().any(|h| fire.iter().any(|f| (f.x - h.x).hypot(f.y - h.y) < NEAR_M));
        if near {
            pts.extend(homes);
        }
    }
    let v: Vec<Vec2> = pts.iter().map(|&p| { let b = crate::frame::to_bevy(p, 0.0); Vec2::new(b.x, b.z) }).collect();
    let (lo, hi) = v.iter().fold((Vec2::MAX, Vec2::MIN), |(l, h), p| (l.min(*p), h.max(*p)));
    Some(((lo + hi) * 0.5, (hi - lo).max_element().max(400.0)))
}

/// What the title screen's shot `n` looks at: the whole front first, then, in
/// turn, the town the fire is nearest to, a crew working close to it and the
/// front close; a subject that is not there (no crew near the fire) is
/// skipped, and the one just shown is not shown again.
fn pick_shot(sim: &Sim, n: usize, last: Subject) -> Subject {
    let cells = sim.fire.active_cells();
    if cells.is_empty() {
        return Subject::Wide;
    }
    let step = (cells.len() / 300).max(1);
    let fire: Vec<Pos> = cells.iter().step_by(step).map(|&c| sim.scn.world.centre_of(c)).collect();
    let gap = |p: Pos| fire.iter().map(|f| (f.x - p.x).hypot(f.y - p.y)).fold(f32::MAX, f32::min);
    // towns by how near the fire is, within what the eye can take in
    let mut towns: Vec<(usize, f32)> = sim.districts.iter().enumerate().map(|(i, d)| (i, gap(d.centre))).filter(|t| t.1 < 3000.0).collect();
    towns.sort_by(|a, b| a.1.total_cmp(&b.1));
    // ground crews close to the fire, not parked at the base
    let mut crews: Vec<(usize, f32)> = sim.crews.units.iter().enumerate().filter(|(_, u)| !u.kind.is_air()).map(|(i, u)| (i, gap(u.pos))).filter(|t| t.1 < 900.0).collect();
    crews.sort_by(|a, b| a.1.total_cmp(&b.1));
    let order = [0usize, 1, 2, 3, 0, 1, 2];
    for k in 0..order.len() {
        let want = order[(n + k) % order.len()];
        let subject = match want {
            0 => Some(Subject::Wide),
            1 => towns.iter().map(|t| Subject::District(t.0)).find(|s| *s != last),
            2 => crews.iter().map(|t| Subject::Crew(t.0)).find(|s| *s != last),
            _ => Some(Subject::Front),
        };
        if let Some(s) = subject.filter(|s| *s != last) {
            return s;
        }
    }
    Subject::Wide
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

/// Behind the title screen only the landscape and the people in it show: the
/// layer of the game's own signs (wind arrow, order rings, routes, beacons,
/// marks on homes) is hidden, and shown again when a game begins. Runs after
/// the systems that build it, so nothing flashes.
#[allow(clippy::type_complexity)]
pub fn hide_game_overlays(
    kiosk: Res<Kiosk>,
    mut hidden: Local<bool>,
    mut arrow: Query<&mut Transform, With<crate::overlays::WindArrow>>,
    mut signs: Query<
        &mut Visibility,
        Or<(
            With<crate::overlays::WindArrow>,
            With<crate::overlays::WindChevron>,
            With<crate::overlays::DynamicMarker>,
            With<crate::overlays::UiGlyph>,
            With<crate::overlays::BeaconLayer>,
            With<crate::overlays::OrderRing>,
            With<crate::overlays::RouteMark>,
            With<crate::units::OrderMarker>,
            With<crate::units::WorkOverlay>,
        )>,
    >,
) {
    if kiosk.title {
        // every frame: signs are rebuilt while the game runs
        for mut v in &mut signs {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
        // the wind arrow still draws when hidden: shrunk to nothing as well
        // (`update_wind` sets its size again every frame of a game)
        for mut t in &mut arrow {
            t.scale = Vec3::ZERO;
        }
        *hidden = true;
    } else if *hidden {
        for mut v in &mut signs {
            *v = Visibility::Inherited;
        }
        *hidden = false;
    }
}
