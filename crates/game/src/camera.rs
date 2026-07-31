//! Orbit camera for a commander's view: left-drag orbits, right-drag pans,
//! scroll zooms. Ported in spirit from the igad-to-rust camera.

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

pub fn controls(
    focus: Res<crate::ui::UiFocus>,
    tool: Res<crate::ignition_edit::IgnitionTool>,
    order: Res<crate::command::OrderTool>,
    mode: Res<CameraMode>,
    mut look: ResMut<FirstPersonLook>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    sim: Res<Sim>,
    mut query: Query<(&mut OrbitCamera, &mut Transform)>,
) {
    let Ok((mut orbit, mut transform)) = query.get_single_mut() else {
        return;
    };

    if let CameraMode::FirstPerson(target) = *mode {
        // A rider's-eye view has no orbit, no pan, no zoom — it looks where
        // the entity is heading, with the mouse only adding look-around on
        // top. `target_pos`/`target_heading` are guaranteed `Some` here:
        // `validate_mode` ran earlier this frame.
        let mut drag = Vec2::ZERO;
        for ev in motion.read() {
            drag += ev.delta;
        }
        wheel.clear();
        if !focus.pointer && buttons.pressed(MouseButton::Left) {
            look.yaw -= drag.x * 0.005;
            look.pitch = (look.pitch - drag.y * 0.005).clamp(-1.3, 1.3);
        }
        if let Some(pos) = target_pos(&sim, target) {
            let heading = target_heading(&sim, target).unwrap_or(0.0) + look.yaw;
            let pitch = look.pitch;
            let dir = Vec3::new(
                heading.cos() * pitch.cos(),
                pitch.sin(),
                -heading.sin() * pitch.cos(),
            );
            let ground = sim.scenario.terrain.height_at(pos);
            let eye = crate::frame::to_bevy(pos, ground + FIRST_PERSON_EYE_M);
            transform.translation = eye;
            transform.look_at(eye + dir, Vec3::Y);
        }
        return;
    }

    // Dragging the speed slider must not also orbit the camera. The events are
    // cleared rather than simply ignored, so a drag over the panel does not
    // queue up and then snap the camera once the cursor leaves it.
    if focus.pointer {
        motion.clear();
        wheel.clear();
        return;
    }

    let mut drag = Vec2::ZERO;
    for ev in motion.read() {
        drag += ev.delta;
    }
    // Trackpads report pixel deltas (tens of units per event) while wheel
    // clicks report line deltas (~1 per notch); treating them the same made
    // trackpad scrolling wildly oversensitive. Normalise both to "notches".
    let mut scroll = 0.0;
    for ev in wheel.read() {
        scroll += match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 50.0,
        };
    }
    let scroll = scroll.clamp(-3.0, 3.0);

    // While the ignition tool or a suppression order is armed, left-drag
    // belongs to it: orbiting on the same button would move the ground out from
    // under the click. Pan, zoom and the keyboard all keep working, so the view
    // is never stuck.
    let orbit_button = tool.mode != crate::ignition_edit::EditMode::Place && !order.is_armed();

    if orbit_button && buttons.pressed(MouseButton::Left) && !keys.pressed(KeyCode::ShiftLeft) {
        orbit.yaw -= drag.x * 0.005;
        orbit.pitch = (orbit.pitch - drag.y * 0.005).clamp(-1.5, -0.05);
    }

    if buttons.pressed(MouseButton::Right)
        || (orbit_button && buttons.pressed(MouseButton::Left) && keys.pressed(KeyCode::ShiftLeft))
    {
        // Pan in the camera's ground plane, scaled by zoom so the world moves
        // with the cursor at any distance.
        let scale = orbit.distance * 0.0015;
        let right = Vec3::new(orbit.yaw.cos(), 0.0, -orbit.yaw.sin());
        let fwd = Vec3::new(orbit.yaw.sin(), 0.0, orbit.yaw.cos());
        orbit.focus += (-right * drag.x + fwd * drag.y) * scale;
    }

    // The arrow keys pan, for keyboard-only use.
    //
    // This was WASD, and WASD is what an RTS player reaches for — but `a` and
    // `d` are also "arm an attack" and "arm a drop" (`crate::command::controls`)
    // and `w`/`s` sat one keycap away from the rest of the order set. Pressing
    // `a` armed an attack *and* slid the map west, which reads as the order
    // having a mysterious side effect rather than as two bindings firing. One
    // of the two had to move, and the camera is the one with a full mouse
    // gesture already covering it: right-drag pans, and always did.
    let mut kb = Vec3::ZERO;
    if !focus.typing() {
            if keys.pressed(KeyCode::ArrowUp) {
            kb.z -= 1.0;
        }
        if keys.pressed(KeyCode::ArrowDown) {
            kb.z += 1.0;
        }
        if keys.pressed(KeyCode::ArrowLeft) {
            kb.x -= 1.0;
        }
        if keys.pressed(KeyCode::ArrowRight) {
            kb.x += 1.0;
        }
    }
    if kb != Vec3::ZERO {
        let step = Quat::from_rotation_y(orbit.yaw)
            * kb.normalize()
            * orbit.distance
            * time.delta_seconds();
        orbit.focus += step;
    }

    // Keep the free-look focus inside the scenario window: real ground only
    // exists there, and letting pan/WASD drift it out into the procedural
    // filler (`crate::far_terrain`) trades a purposeful view for an aimless
    // one over hills and villages that were never simulated. `Follow` below
    // overrides this with the target's own position regardless, including
    // one that has driven off the map (see finding 10) — this clamp only
    // bounds the player's own free pan.
    let world = &sim.scenario.world;
    orbit.focus.x = orbit.focus.x.clamp(0.0, world.width_m);
    orbit.focus.z = orbit.focus.z.clamp(-world.height_m, 0.0);

    if scroll != 0.0 {
        orbit.distance = (orbit.distance * (1.0 - scroll * 0.08)).clamp(60.0, 14000.0);
    }

    // Following overrides whatever pan/keyboard motion happened above with
    // the entity's own position, every frame — yaw, pitch and zoom stay the
    // player's, only the focus point is on a leash.
    if let CameraMode::Follow(target) = *mode {
        if let Some(pos) = target_pos(&sim, target) {
            let ground = sim.scenario.terrain.height_at(pos);
            orbit.focus = crate::frame::to_bevy(pos, ground);
        }
    }

    let dir = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        -orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    );
    transform.translation = orbit.focus + dir * orbit.distance;
    // Clamp against the ground under the *camera*, not the focus: see
    // `MIN_GROUND_CLEARANCE_M`.
    let ground = sim
        .scenario
        .terrain
        .height_at(crate::frame::to_world(transform.translation));
    transform.translation.y = transform.translation.y.max(ground + MIN_GROUND_CLEARANCE_M);
    transform.look_at(orbit.focus, Vec3::Y);
}
