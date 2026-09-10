//! Shared navigation for the orbit scene and north-up tactical map.
//! Pointer gestures belong to the surface where they begin; UI keyboard
//! focus and platform chords take priority over camera shortcuts.

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::inspect::{target_heading, target_pos, Target};
use crate::sim::{Sim, SimRestarted};

/// The camera's own height above the ground it is actually over, not the
/// focus point's — a shallow-pitch orbit close to a sloped focus can put the
/// *camera* metres from a hillside metres away, even though the focus itself
/// sits in a clear valley. Below this, the near geometry is close enough to
/// fill the frame at a grazing angle: a self-shadowed slope or canopy that
/// would be an unremarkable dark patch from altitude instead reads as one
/// huge, flat, near-black wedge. 25 m clears a typical Mediterranean canopy
/// (vegetation runs 5-15 m, see `crate::vegetation`) with room to spare.
const MIN_GROUND_CLEARANCE_M: f32 = 25.0;

/// Eye height for the first-person view, metres above the ground at the
/// followed entity's own position.
const FIRST_PERSON_EYE_M: f32 = 1.7;

#[derive(Component)]
pub struct OrbitCamera {
    /// Point on the ground the camera looks at, in Bevy space.
    pub focus: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera {
            focus: Vec3::ZERO,
            distance: 3500.0,
            yaw: 0.35,
            pitch: -0.75,
        }
    }
}

/// What the main camera is currently doing. `Follow` keeps the orbit's focus
/// pinned to a moving entity while leaving yaw/pitch/zoom to the player, the
/// same "camera on a leash" every RTS has; `FirstPerson` replaces the orbit
/// entirely with a rider's-eye view, heading off the entity's own travel
/// direction. Both targets are validated once a frame by [`validate_mode`]
/// before `controls` reads this, so `controls` can assume a `Follow` or
/// `FirstPerson` target still resolves to a live position.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub enum CameraMode {
    #[default]
    Free,
    Follow(Target),
    FirstPerson(Target),
}

/// Mouse-look accumulated while riding along in `CameraMode::FirstPerson`,
/// kept apart from the orbit's own yaw/pitch so leaving first person and
/// coming back to the orbit view resumes exactly where the commander's own
/// camera left off.
#[derive(Resource, Default)]
pub struct FirstPersonLook {
    pub yaw: f32,
    pub pitch: f32,
}

/// Drop a `Follow`/`FirstPerson` target that no longer resolves — the entity
/// arrived, was evacuated, or the sim restarted out from under it — back to
/// `Free`, so `controls` never has to handle a mode pointing at nothing.
pub fn validate_mode(sim: Res<Sim>, mut mode: ResMut<CameraMode>) {
    let alive = match *mode {
        CameraMode::Free => true,
        CameraMode::Follow(t) | CameraMode::FirstPerson(t) => target_pos(&sim, t).is_some(),
    };
    if !alive {
        *mode = CameraMode::Free;
    }
}

/// Clear the camera mode and its mouse-look state across a restart: the
/// travellers and units a mode might be riding along with do not survive one
/// (see `inspect::reset`).
pub fn reset(
    mut restarted: EventReader<SimRestarted>,
    mut mode: ResMut<CameraMode>,
    mut look: ResMut<FirstPersonLook>,
) {
    if restarted.is_empty() {
        return;
    }
    restarted.clear();
    *mode = CameraMode::Free;
    *look = FirstPersonLook::default();
}

/// A drag belongs to the surface where the button went down, even if the
/// cursor subsequently crosses a panel boundary.
#[derive(Default)]
pub struct DragOwner {
    left: bool,
    right: bool,
    middle: bool,
}

pub fn shift(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

/// Platform/editing chords must never fall through to single-key game actions.
pub fn modified(keys: &ButtonInput<KeyCode>) -> bool {
    [
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
        KeyCode::AltLeft,
        KeyCode::AltRight,
    ]
    .iter()
    .any(|key| keys.pressed(*key))
}

fn zoom(distance: f32, amount: f32) -> f32 {
    (distance * (-amount).exp()).clamp(60.0, 28_000.0)
}

pub fn controls(
    renderer: Res<crate::map2d::Renderer>,
    focus: Res<crate::ui::UiFocus>,
    tool: Res<crate::ignition_edit::IgnitionTool>,
    order: Res<crate::command::OrderTool>,
    mut mode: ResMut<CameraMode>,
    mut look: ResMut<FirstPersonLook>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    sim: Res<Sim>,
    mut query: Query<(&mut OrbitCamera, &mut Transform, &Camera)>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut owner: Local<DragOwner>,
) {
    let mut drag = motion.read().fold(Vec2::ZERO, |sum, ev| sum + ev.delta);
    let scroll: f32 = wheel
        .read()
        .map(|ev| match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 50.0,
        })
        .sum();
    let Ok((mut orbit, mut transform, camera)) = query.get_single_mut() else {
        return;
    };
    let window = windows.get_single().ok();
    let active = window.is_some_and(|w| w.focused);
    let map_pointer = active
        && !focus.pointer
        && window
            .and_then(|w| crate::pick::cursor_position(camera, w))
            .is_some();
    let left_free = tool.mode != crate::ignition_edit::EditMode::Place && !order.is_armed();
    let owner = &mut *owner;
    for (button, owned) in [
        (MouseButton::Left, &mut owner.left),
        (MouseButton::Right, &mut owner.right),
        (MouseButton::Middle, &mut owner.middle),
    ] {
        if !buttons.pressed(button) || !active {
            *owned = false;
        }
        if buttons.just_pressed(button) {
            *owned = map_pointer && (button != MouseButton::Left || left_free);
        }
    }
    if !map_pointer {
        drag = Vec2::ZERO;
    }
    let keyboard = active && !focus.typing() && !modified(&keys);
    let shifted = shift(&keys);
    let dt = time.delta_seconds().min(0.1);
    let map = *renderer == crate::map2d::Renderer::Map2d;
    if let CameraMode::FirstPerson(target) = *mode {
        if map {
            *mode = CameraMode::Follow(target);
        } else {
            if owner.left || owner.right {
                look.yaw -= drag.x * 0.005;
                look.pitch = (look.pitch - drag.y * 0.005).clamp(-1.3, 1.3);
            }
            if let Some(pos) = target_pos(&sim, target) {
                let heading = target_heading(&sim, target).unwrap_or(0.0) + look.yaw;
                let dir = Vec3::new(
                    heading.cos() * look.pitch.cos(),
                    look.pitch.sin(),
                    -heading.sin() * look.pitch.cos(),
                );
                let eye = crate::frame::to_bevy(
                    pos,
                    sim.scenario.terrain.height_at(pos) + FIRST_PERSON_EYE_M,
                );
                *transform = Transform::from_translation(eye).looking_at(eye + dir, Vec3::Y);
            }
            return;
        }
    }
    let axis = |negative, positive| {
        if keyboard {
            f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative))
        } else {
            0.0
        }
    };
    let yaw_input = if shifted {
        0.0
    } else {
        axis(KeyCode::KeyE, KeyCode::KeyQ)
    };
    let pitch_input = axis(KeyCode::PageDown, KeyCode::PageUp);
    if !map {
        if owner.left && left_free && !shifted {
            orbit.yaw -= drag.x * 0.005;
            orbit.pitch -= drag.y * 0.005;
        }
        orbit.yaw += yaw_input * dt * 1.2;
        orbit.pitch -= pitch_input * dt * 0.8;
        orbit.pitch = orbit.pitch.clamp(-1.5, -0.12);
    }
    let yaw = if map { 0.0 } else { orbit.yaw };
    let rotation = Quat::from_rotation_y(yaw);
    let height = camera
        .logical_viewport_size()
        .map(|s| s.y)
        .unwrap_or(1000.0)
        .max(1.0);
    // Perspective span at the focus, using the default camera's 45° FOV.
    let scale = orbit.distance
        * if map {
            1.0
        } else {
            2.0 * (std::f32::consts::FRAC_PI_4 * 0.5).tan()
        }
        / height;
    let mut pan = Vec3::ZERO;
    if owner.right || owner.middle || (owner.left && left_free && (map || shifted)) {
        pan += rotation
            * Vec3::new(
                -drag.x,
                0.0,
                drag.y / if map { 1.0 } else { -orbit.pitch.sin() },
            )
            * scale;
    }
    let kb = Vec3::new(
        axis(KeyCode::ArrowLeft, KeyCode::ArrowRight),
        0.0,
        axis(KeyCode::ArrowUp, KeyCode::ArrowDown),
    );
    pan +=
        rotation * kb.normalize_or_zero() * orbit.distance * dt * if shifted { 1.5 } else { 0.5 };
    if pan.length_squared() > 0.0 {
        *mode = CameraMode::Free;
        orbit.focus += pan;
    }
    let old_distance = orbit.distance;
    let key_zoom =
        axis(KeyCode::Minus, KeyCode::Equal) + axis(KeyCode::NumpadSubtract, KeyCode::NumpadAdd);
    orbit.distance = zoom(
        orbit.distance,
        if map_pointer {
            scroll.clamp(-6.0, 6.0) * 0.1
        } else {
            0.0
        } + key_zoom * dt * 1.5,
    );
    // Keep the map location under the pointer fixed during wheel zoom.
    if map && map_pointer && scroll != 0.0 && *mode == CameraMode::Free {
        if let Some(cursor) = window.and_then(|w| crate::pick::cursor_position(camera, w)) {
            let size = camera
                .logical_viewport_size()
                .unwrap_or(Vec2::splat(height));
            let offset = cursor - size * 0.5;
            let delta = (old_distance - orbit.distance) / height;
            orbit.focus += Vec3::new(offset.x, 0.0, offset.y) * delta;
        }
    }
    let world = &sim.scenario.world;
    orbit.focus.x = orbit.focus.x.clamp(0.0, world.width_m);
    orbit.focus.z = orbit.focus.z.clamp(-world.height_m, 0.0);
    if let CameraMode::Follow(target) = *mode {
        if let Some(pos) = target_pos(&sim, target) {
            orbit.focus = crate::frame::to_bevy(pos, 0.0);
        }
    }
    orbit.focus.y = sim
        .scenario
        .terrain
        .height_at(crate::frame::to_world(orbit.focus));
    let dir = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        -orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    );
    transform.translation = orbit.focus + dir * orbit.distance;
    let ground = sim
        .scenario
        .terrain
        .height_at(crate::frame::to_world(transform.translation));
    transform.translation.y = transform.translation.y.max(ground + MIN_GROUND_CLEARANCE_M);
    transform.look_at(orbit.focus, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn camera_app() -> (App, Entity) {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let scenario = scenario::Scenario::load_by_id(data, "abm_micro").unwrap();
        let (weather, radius) = crate::sim::opening_conditions("abm_micro");
        let sim = Sim::new(
            scenario,
            weather,
            radius,
            42,
            behavior::defaults::default_library(),
        )
        .unwrap();
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.016));
        app.insert_resource(sim)
            .insert_resource(time)
            .insert_resource(crate::map2d::Renderer::Map2d)
            .init_resource::<crate::ui::UiFocus>()
            .init_resource::<crate::ignition_edit::IgnitionTool>()
            .init_resource::<crate::command::OrderTool>()
            .insert_resource(CameraMode::Follow(Target::Household(0)))
            .init_resource::<FirstPersonLook>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_event::<MouseMotion>()
            .add_event::<MouseWheel>()
            .add_systems(Update, controls);
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        let camera = app
            .world_mut()
            .spawn((
                OrbitCamera::default(),
                Transform::default(),
                Camera::default(),
            ))
            .id();
        (app, camera)
    }

    #[test]
    fn follow_updates_over_panels_and_manual_pan_releases_it() {
        let (mut app, camera) = camera_app();
        app.world_mut().resource_mut::<crate::ui::UiFocus>().pointer = true;
        app.update();
        let expected = app.world().resource::<Sim>().agents.households[0].home;
        assert_eq!(
            crate::frame::to_world(app.world().get::<OrbitCamera>(camera).unwrap().focus),
            expected
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);
        app.update();
        assert_eq!(*app.world().resource::<CameraMode>(), CameraMode::Free);
        assert!(app.world().get::<OrbitCamera>(camera).unwrap().focus.x > expected.x);
    }

    #[test]
    fn typing_and_platform_chords_do_not_pan_or_release_follow() {
        let (mut app, camera) = camera_app();
        app.update();
        let before = app.world().get::<OrbitCamera>(camera).unwrap().focus;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);
        app.world_mut()
            .resource_mut::<crate::ui::UiFocus>()
            .keyboard = true;
        app.update();
        assert_eq!(
            app.world().get::<OrbitCamera>(camera).unwrap().focus,
            before
        );
        app.world_mut()
            .resource_mut::<crate::ui::UiFocus>()
            .keyboard = false;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::SuperLeft);
        app.update();
        assert_eq!(
            *app.world().resource::<CameraMode>(),
            CameraMode::Follow(Target::Household(0))
        );
        assert_eq!(
            app.world().get::<OrbitCamera>(camera).unwrap().focus,
            before
        );
    }

    #[test]
    fn zoom_is_reversible_and_independent_of_event_grouping() {
        assert!((zoom(zoom(1000.0, 0.2), -0.2) - 1000.0).abs() < 0.001);
        assert!((zoom(zoom(1000.0, 0.1), 0.1) - zoom(1000.0, 0.2)).abs() < 0.001);
        assert_eq!(zoom(60.0, 20.0), 60.0);
    }
    #[test]
    fn both_modifier_sides_block_plain_shortcuts() {
        for key in [
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::AltLeft,
            KeyCode::AltRight,
        ] {
            let mut keys = ButtonInput::default();
            keys.press(key);
            assert!(modified(&keys));
        }
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::ShiftRight);
        assert!(shift(&keys));
        assert!(!modified(&keys));
    }
}

pub fn navigation_hint(renderer: crate::map2d::Renderer) -> &'static str {
    match renderer {
        crate::map2d::Renderer::Map2d => {
            "2D · north up · drag pan · scroll/+− zoom · arrows pan · V 3D"
        }
        crate::map2d::Renderer::Scene3d => {
            "3D · drag orbit · Shift/right/middle-drag pan · scroll/+− zoom · V 2D"
        }
    }
}
