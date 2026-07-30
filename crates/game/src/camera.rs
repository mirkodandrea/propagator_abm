//! Orbit camera for a commander's view: left-drag orbits, right-drag pans,
//! scroll zooms. Ported in spirit from the igad-to-rust camera.

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;

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

pub fn controls(
    focus: Res<crate::ui::UiFocus>,
    tool: Res<crate::ignition_edit::IgnitionTool>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<(&mut OrbitCamera, &mut Transform)>,
) {
    let Ok((mut orbit, mut transform)) = query.get_single_mut() else {
        return;
    };

    // Dragging the speed slider must not also orbit the camera. The events are
    // cleared rather than simply ignored, so a drag over the panel does not
    // queue up and then snap the camera once the cursor leaves it.
    if focus.0 {
        motion.clear();
        wheel.clear();
        return;
    }

    let mut drag = Vec2::ZERO;
    for ev in motion.read() {
        drag += ev.delta;
    }
    let mut scroll = 0.0;
    for ev in wheel.read() {
        scroll += ev.y;
    }

    // While the ignition tool is armed, left-drag belongs to it: orbiting on
    // the same button would move the ground out from under the click. Pan,
    // zoom and the keyboard all keep working, so the view is never stuck.
    let orbit_button = tool.mode != crate::ignition_edit::EditMode::Place;

    if orbit_button && buttons.pressed(MouseButton::Left) && !keys.pressed(KeyCode::ShiftLeft)
    {
        orbit.yaw -= drag.x * 0.005;
        orbit.pitch = (orbit.pitch - drag.y * 0.005).clamp(-1.5, -0.05);
    }

    if buttons.pressed(MouseButton::Right)
        || (orbit_button
            && buttons.pressed(MouseButton::Left)
            && keys.pressed(KeyCode::ShiftLeft))
    {
        // Pan in the camera's ground plane, scaled by zoom so the world moves
        // with the cursor at any distance.
        let scale = orbit.distance * 0.0015;
        let right = Vec3::new(orbit.yaw.cos(), 0.0, -orbit.yaw.sin());
        let fwd = Vec3::new(orbit.yaw.sin(), 0.0, orbit.yaw.cos());
        orbit.focus += (-right * drag.x + fwd * drag.y) * scale;
    }

    // WASD pans too, for keyboard-only use.
    let mut kb = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        kb.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        kb.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        kb.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        kb.x += 1.0;
    }
    if kb != Vec3::ZERO {
        let step = Quat::from_rotation_y(orbit.yaw) * kb.normalize()
            * orbit.distance
            * time.delta_seconds();
        orbit.focus += step;
    }

    if scroll != 0.0 {
        orbit.distance = (orbit.distance * (1.0 - scroll * 0.12)).clamp(60.0, 14000.0);
    }

    let dir = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        -orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    );
    transform.translation = orbit.focus + dir * orbit.distance;
    transform.look_at(orbit.focus, Vec3::Y);
}
