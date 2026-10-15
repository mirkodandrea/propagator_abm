//! The kiosk camera: a miniature seen from above, never lost; and the click
//! on the map that chooses a hex (`Kiosk::click`).
//!
//! Drag to pan (Shift-drag or right-drag to turn), wheel or pinch to zoom,
//! Home for the starting view. A press and release that barely moved is a
//! click, not a drag. While the fire runs the camera follows it, until a hand
//! takes the map.

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
const PAN_RADIUS_M: f32 = 2500.0;
/// A press that moved less than this (logical px) is a click.
const CLICK_PX: f32 = 6.0;

/// The places the frame has to hold: every home and where the fire started.
fn frame_points(game: &Game) -> Vec<Pos> {
    let mut pts: Vec<Pos> = game.scn.population.households.iter().map(|h| Pos::from(h.pos)).collect();
    pts.push(game.case.ignition());
    pts
}

/// What the camera looks at: the middle of the homes and the fire.
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
    /// How far the left press has moved, logical px.
    moved: f32,
    touch_id: Option<u64>,
    touch_map: bool,
    touch_moved: f32,
    multi: bool,
    pair: Option<(u64, u64, Vec2, Vec2)>,
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
    mut query: Query<(&mut OrbitCamera, &mut Transform, &mut Camera, &GlobalTransform, Option<&mut bevy::core_pipeline::dof::DepthOfFieldSettings>)>,
    time: Res<Time>,
    mut drag: Local<Drag>,
    mut settled: Local<Option<(String, u64)>>,
    mut home_view: Local<Option<(f32, f32)>>,
    mut refit: Local<(Option<UVec2>, u8)>,
    mut last_cursor: Local<Option<Vec2>>,
    mut tracked: Local<Option<(Vec2, f32)>>,
    mut smooth_h: Local<Option<(f32, f32)>>,
) {
    input.motion.clear();
    let cursor = windows.get_single().ok().and_then(|w| w.cursor_position());
    let delta = match (cursor, *last_cursor) {
        (Some(c), Some(l)) => c - l,
        _ => Vec2::ZERO,
    };
    *last_cursor = cursor;
    let shift = input.keys.pressed(KeyCode::ShiftLeft) || input.keys.pressed(KeyCode::ShiftRight);
    let mut scroll = 0.0;
    for event in input.wheel.read() {
        match event.unit {
            MouseScrollUnit::Line => scroll += event.y,
            MouseScrollUnit::Pixel => scroll += event.y / 50.0,
        }
    }
    let pinch: f32 = input.pinch.read().map(|e| e.0).sum();
    let rotation: f32 = input.rotation.read().map(|e| e.0).sum();
    let buttons = &input.buttons;
    let Ok((mut orbit, mut tf, mut camera, gt, mut dof)) = query.get_single_mut() else { return };
    // The map is drawn left of the side bar, framed in what can be seen.
    if let Ok(w) = windows.get_single() {
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
    let scene_pitch = -0.80;

    let window = windows.get_single().ok();
    let touch_frame = input.touches.iter().next().is_some()
        || input.touches.iter_just_released().next().is_some()
        || input.touches.iter_just_canceled().next().is_some();
    let over_map = !touch_frame && window.is_some_and(|w| w.focused) && !focus.pointer && window.and_then(|w| crate::pick::cursor_position(camera, w)).is_some();
    if buttons.just_pressed(MouseButton::Left) {
        drag.left = over_map;
        drag.moved = 0.0;
    }
    if buttons.just_pressed(MouseButton::Right) {
        drag.right = over_map;
    }
    if buttons.just_pressed(MouseButton::Middle) {
        drag.middle = over_map;
    }
    if drag.left {
        drag.moved += delta.length();
    }
    // a click: pressed and released on the map, barely moved
    if buttons.just_released(MouseButton::Left) && drag.left && drag.moved < CLICK_PX && !shift {
        kiosk.click = window.and_then(|w| crate::pick::cursor_ground(&sim.scn, camera, gt, w));
    }
    drag.left &= buttons.pressed(MouseButton::Left) && window.is_some_and(|w| w.focused);
    drag.right &= buttons.pressed(MouseButton::Right) && window.is_some_and(|w| w.focused);
    drag.middle &= buttons.pressed(MouseButton::Middle) && window.is_some_and(|w| w.focused);

    let dragging = (drag.left && drag.moved >= CLICK_PX) || drag.right || drag.middle;
    let touched = input.touches.iter_just_pressed().next().is_some()
        || input.touches.iter().any(|t| t.delta() != Vec2::ZERO)
        || delta.length_squared() > 0.0 && dragging
        || over_map && (scroll != 0.0 || pinch != 0.0 || rotation != 0.0);
    if touched {
        kiosk.follow = false;
        kiosk.shot = None;
    }

    let height = camera.logical_viewport_size().map_or(1000.0, |s| s.y);
    if (drag.left && drag.moved >= CLICK_PX && !shift) || drag.middle {
        pan(&mut orbit, delta, height);
    }
    if drag.right || (drag.left && shift) {
        orbit.yaw -= delta.x * 0.005;
        orbit.pitch -= delta.y * 0.004;
    }
    // Capture the owner at touch-down. Crossing a badge while panning must
    // not steal the gesture; starting on a cone must never pan the map.
    let active: Vec<_> = input.touches.iter().collect();
    if drag.touch_id.is_none() {
        if let Some(t) = input.touches.iter_just_pressed().next() {
            drag.touch_id = Some(t.id());
            drag.touch_map = !focus.pointer && crate::pick::window_to_viewport(camera, t.position()).is_some();
            drag.touch_moved = 0.0;
            drag.multi = false;
        }
    }
    if active.len() >= 2 { drag.multi = true; }
    if drag.touch_map {
        match active.as_slice() {
            [t] if !drag.multi => {
                drag.touch_moved = drag.touch_moved.max(t.distance().length());
                if drag.touch_moved >= CLICK_PX { pan(&mut orbit, t.delta(), height); }
                drag.pair = None;
            }
            [a, b] => {
                // Stable ordering avoids a 180-degree jump when the touch map reorders.
                let (a, b) = if a.id() < b.id() { (a, b) } else { (b, a) };
                if let Some((ia, ib, pa, pb)) = drag.pair {
                    if (ia, ib) == (a.id(), b.id()) {
                        let before = pb - pa;
                        let now = b.position() - a.position();
                        if before.length() > 8.0 && now.length() > 8.0 {
                            orbit.distance *= before.length() / now.length();
                            orbit.yaw -= before.perp_dot(now).atan2(before.dot(now));
                            pan(&mut orbit, (a.position() + b.position() - pa - pb) * 0.5, height);
                        }
                    }
                }
                drag.pair = Some((a.id(), b.id(), a.position(), b.position()));
            }
            _ => { drag.pair = None; }
        }
        for t in input.touches.iter_just_released() {
            if Some(t.id()) == drag.touch_id && !drag.multi && drag.touch_moved.max(t.distance().length()) < CLICK_PX && !focus.pointer {
                kiosk.click = crate::pick::window_to_viewport(camera, t.position())
                    .and_then(|p| camera.viewport_to_world(gt, p))
                    .and_then(|ray| crate::pick::ray_ground(&sim.scn, ray.origin, *ray.direction));
            }
        }
    }
    if active.is_empty() {
        drag.touch_id = None;
        drag.touch_map = false;
        drag.pair = None;
    }
    if kiosk.zoom_steps != 0.0 {
        orbit.distance *= (-kiosk.zoom_steps * 0.25).exp();
        kiosk.zoom_steps = 0.0;
        kiosk.follow = false;
        kiosk.shot = None;
    }
    if over_map {
        orbit.yaw += rotation.to_radians();
        orbit.distance *= (-scroll.clamp(-6.0, 6.0) * 0.1 - pinch.clamp(-1.0, 1.0)).exp();
    }
    // a new game: settle on the home framing once
    let game_key = (sim.case.name.clone(), sim.generation.min(1));
    if settled.as_ref().map(|s| &s.0) != Some(&game_key.0) || sim.time_s() <= rocca::STEP_S && settled.as_ref().is_none_or(|s| s.1 == 0) {
        if let Some((yaw, d)) = fit_view(camera, &sim, home, scene_pitch) {
            *settled = Some(game_key);
            *tracked = None;
            *home_view = Some((yaw, d));
            orbit.focus = home;
            orbit.yaw = yaw;
            orbit.pitch = scene_pitch;
            orbit.distance = d * shot_zoom();
        }
    }
    if refit.1 > 0 {
        refit.1 -= 1;
        if refit.1 == 0 {
            if let Some((yaw, d)) = fit_view(camera, &sim, home, scene_pitch) {
                *home_view = Some((yaw, d));
                if kiosk.phase == Phase::Intro {
                    orbit.focus = home;
                    orbit.yaw = yaw;
                    orbit.pitch = scene_pitch;
                    orbit.distance = d * shot_zoom();
                }
            }
        }
    }
    // the tutorial's flight to what the step is about
    if let (Some(shot), Some((_, d))) = (kiosk.shot, *home_view) {
        let dt = time.delta_seconds().min(0.1);
        let goal = shot.at.map_or(home, |p| crate::frame::to_bevy(p, sim.scn.terrain.height_at(p)));
        let goal_d = (d * shot_zoom() * shot.zoom).clamp(MIN_DIST, MAX_DIST);
        let a = 1.0 - (-2.5 * dt).exp();
        orbit.focus = orbit.focus.lerp(goal, a);
        orbit.distance += (goal_d - orbit.distance) * a;
        if orbit.focus.distance(goal) < 8.0 && (orbit.distance / goal_d - 1.0).abs() < 0.02 {
            kiosk.shot = None;
        }
    }
    // Behind the name screen: a director, not a follower. A shot lasts
    // `SHOT_S`; its subject is picked when it begins, from what the fire is
    // doing then, and the camera glides to it slowly.
    if kiosk.phase == Phase::Intro && !kiosk.frozen {
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
            // the goal itself is smoothed (a crew moves in jumps), then the
            // camera eases to it: two slow stages, no sudden move
            let dt = time.delta_seconds().min(0.1);
            let (g, gd) = tracked.get_or_insert((Vec2::new(goal.x, goal.z), dist));
            let s = 1.0 - (-0.9 * dt).exp();
            *g += (Vec2::new(goal.x, goal.z) - *g) * s;
            *gd += (dist - *gd) * s;
            let a = 1.0 - (-0.5 * dt).exp();
            orbit.focus = orbit.focus.lerp(Vec3::new(g.x, goal.y, g.y), a);
            orbit.distance += (*gd - orbit.distance) * a;
            // a slow drift of the heading around the shot's own, so the
            // picture is never still
            let yaw = yaw0 + sway * 0.5 + 0.2 * (kiosk.demo_t * 0.08).sin();
            orbit.yaw += (yaw - orbit.yaw) * (1.0 - (-0.35 * dt).exp());
            orbit.pitch += (pitch - orbit.pitch) * a;
        }
    }
    // the fire runs: follow it, slowly
    if kiosk.follow && kiosk.shot.is_none() && matches!(kiosk.phase, Phase::Tutorial | Phase::Gioco) {
        if let (Some((_, d)), Some((mid, span))) = (*home_view, fire_extent(&sim)) {
            let dt = time.delta_seconds().min(0.1);
            let (t_mid, t_span) = tracked.get_or_insert((mid, span));
            let s = 1.0 - (-0.6 * dt).exp();
            *t_mid += (mid - *t_mid) * s;
            *t_span += (span - *t_span) * s;
            let goal_d = (*t_span * 2.8).clamp(d * 0.45, d);
            let goal = Vec3::new(t_mid.x, orbit.focus.y, t_mid.y);
            let a = 1.0 - (-1.2 * dt).exp();
            orbit.focus = orbit.focus.lerp(goal, a);
            orbit.distance += (goal_d - orbit.distance) * a;
        }
    }
    if input.keys.just_pressed(KeyCode::Home) {
        kiosk.reset_view = true;
    }
    if kiosk.reset_view {
        kiosk.reset_view = false;
        kiosk.follow = false;
        if let Some((yaw, d)) = *home_view {
            orbit.focus = home;
            orbit.yaw = yaw;
            orbit.pitch = scene_pitch;
            orbit.distance = d * shot_zoom();
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
    let terrain = &sim.scn.terrain;
    // the intro flies low over the relief: the raw height under the focus and
    // under the eye wiggles with every ridge, so there both are averaged over
    // a wide ring and then eased in time (the eye never dips, it only rises
    // with the broad shape of the ground)
    let area = |p: Pos, r: f32| {
        let mut sum = terrain.height_at(p);
        let mut top = sum;
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            let h = terrain.height_at(Pos { x: p.x + r * a.cos(), y: p.y + r * a.sin() });
            sum += h;
            top = top.max(h);
        }
        (sum / 9.0, top)
    };
    let intro = kiosk.phase == Phase::Intro && !kiosk.frozen;
    if intro {
        let dt = time.delta_seconds().min(0.1);
        let k = 1.0 - (-1.5 * dt).exp();
        let (fy, _) = area(crate::frame::to_world(orbit.focus), 300.0);
        let (sf, sg) = *smooth_h.get_or_insert((fy, 0.0));
        let sf = sf + (fy - sf) * k;
        *smooth_h = Some((sf, sg));
        orbit.focus.y = sf;
    } else {
        orbit.focus.y = terrain.height_at(crate::frame::to_world(orbit.focus));
    }
    let dir = Vec3::new(orbit.yaw.sin() * orbit.pitch.cos(), -orbit.pitch.sin(), orbit.yaw.cos() * orbit.pitch.cos());
    tf.translation = orbit.focus + dir * orbit.distance;
    if intro {
        let dt = time.delta_seconds().min(0.1);
        let k = 1.0 - (-1.0 * dt).exp();
        let (mean, top) = area(crate::frame::to_world(tf.translation), 400.0);
        let want = mean.max(top - 40.0) + 60.0;
        let (sf, sg) = smooth_h.unwrap_or((orbit.focus.y, want));
        let sg = if sg == 0.0 { want } else { sg };
        let sg = sg + (want - sg) * k;
        *smooth_h = Some((sf, sg));
        tf.translation.y = tf.translation.y.max(sg);
        orbit.focus.y = sf;
    } else {
        let ground = terrain.height_at(crate::frame::to_world(tf.translation));
        tf.translation.y = tf.translation.y.max(ground + 25.0);
    }
    tf.look_at(orbit.focus, Vec3::Y);
    if let Some(dof) = dof.as_mut() {
        dof.focal_distance = tf.translation.distance(orbit.focus);
    }
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
        let homes = d.households.iter().map(|&i| Pos::from(sim.scn.population.households[i].pos));
        let near = homes.clone().any(|h| fire.iter().any(|f| (f.x - h.x).hypot(f.y - h.y) < NEAR_M));
        if near {
            pts.extend(homes);
        }
    }
    let v: Vec<Vec2> = pts.iter().map(|&p| { let b = crate::frame::to_bevy(p, 0.0); Vec2::new(b.x, b.z) }).collect();
    let (lo, hi) = v.iter().fold((Vec2::MAX, Vec2::MIN), |(l, h), p| (l.min(*p), h.max(*p)));
    Some(((lo + hi) * 0.5, (hi - lo).max_element().max(400.0)))
}

/// What the opening screen's shot `n` looks at: the whole front first, then,
/// in turn, the town the fire is nearest to, a crew working close to it and
/// the front close; a subject that is not there is skipped, and the one just
/// shown is not shown again.
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

