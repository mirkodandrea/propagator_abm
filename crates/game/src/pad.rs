//! The gamepad for the serious game (never while Mario plays: there the pad
//! keeps its own controls, see `mario.rs`).
//!
//! In the game (2026-10-17): the left stick pans the map, the right stick
//! turns and tilts it, the triggers zoom, RB goes back to the whole island.
//! The d-pad chooses the team (left/right) and the district (up/down). With
//! a team chosen in the pause the aim is the
//! centre of the screen, moved with the camera, and A sends the team there;
//! with a district chosen X evacuates it. Y stops and starts the world, B is
//! Esc. These are only recorded in [`PadView`]; `kiosk::ui` turns them into
//! the same calls the mouse makes.
//!
//! Holding LB, and everywhere outside the game (buttons, the operator menu),
//! the left stick moves a cursor and A is the left button, written as
//! ordinary window and mouse events before the input systems run, so egui
//! sees a real mouse.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseButtonInput;
use bevy::input::{ButtonState, InputSystem};
use bevy::prelude::*;
use bevy::window::{CursorMoved, PrimaryWindow};

use crate::mario::Playground;

/// Logical px/s at full stick.
const CURSOR_SPEED: f32 = 1100.0;
const DEADZONE: f32 = 0.18;

/// What the pad asks of the game this frame, read by `kiosk::view::camera`
/// and `kiosk::ui`.
#[derive(Resource, Default)]
pub struct PadView {
    /// Left stick, +y is up on the screen.
    pub pan: Vec2,
    /// Right stick: x turns to the right, y tilts towards the horizon.
    pub look: Vec2,
    /// +1 closer, -1 farther.
    pub zoom: f32,
    /// Back to the whole island (RB).
    pub home: bool,
    /// Team to choose, relative to the one chosen: -1 previous, +1 next.
    pub team: i32,
    /// District to choose, likewise.
    pub district: i32,
    /// Stop or start the world (Y).
    pub pause: bool,
    /// A was pressed in the game (see [`PadMode`]).
    pub confirm: bool,
    /// Evacuate the chosen district (X).
    pub evacuate: bool,
}

/// Set by `kiosk::ui`: `game` is the map with its list (the sticks are the
/// camera).
#[derive(Resource, Default)]
pub struct PadMode {
    pub game: bool,
}

impl PadView {
    pub fn active(&self) -> bool {
        self.pan != Vec2::ZERO || self.zoom != 0.0 || self.look != Vec2::ZERO
    }
}

pub struct PadPlugin;

impl Plugin for PadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PadView>().init_resource::<PadMode>().add_systems(
            PreUpdate,
            virtual_mouse
                .before(InputSystem)
                .before(bevy_egui::EguiSet::ProcessInput),
        );
    }
}

fn deadzone(v: Vec2) -> Vec2 {
    let len = v.length();
    if len < DEADZONE {
        return Vec2::ZERO;
    }
    v / len * ((len - DEADZONE) / (1.0 - DEADZONE)).min(1.0)
}

#[allow(clippy::too_many_arguments)]
fn virtual_mouse(
    pads: Res<Gamepads>,
    buttons: Res<ButtonInput<GamepadButton>>,
    axes: Res<Axis<GamepadAxis>>,
    triggers: Res<Axis<GamepadButton>>,
    mario: Res<Playground>,
    mode: Res<PadMode>,
    mut held: Local<bool>,
    time: Res<Time>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut view: ResMut<PadView>,
    mut moved: EventWriter<CursorMoved>,
    mut clicks: EventWriter<MouseButtonInput>,
    mut keys: EventWriter<KeyboardInput>,
) {
    *view = PadView::default();
    let (Ok((entity, mut window)), Some(pad)) = (windows.get_single_mut(), pads.iter().next()) else { return };
    if mario.active || !window.focused {
        return;
    }
    let axis = |a| axes.get(GamepadAxis::new(pad, a)).unwrap_or(0.0);
    let button = |b| GamepadButton::new(pad, b);
    let trigger = |b| triggers.get(button(b)).unwrap_or(0.0).max(f32::from(buttons.pressed(button(b))));
    let pressed = |b| buttons.pressed(button(b));
    let just = |b| buttons.just_pressed(button(b));

    let left = deadzone(Vec2::new(axis(GamepadAxisType::LeftStickX), -axis(GamepadAxisType::LeftStickY)));
    let south = button(GamepadButtonType::South);
    // the sticks are the camera in the game, a cursor elsewhere or with LB held
    let cursor = !mode.game || pressed(GamepadButtonType::LeftTrigger);
    if mode.game {
        if !cursor {
            view.pan = Vec2::new(left.x, -left.y);
        }
        view.look = deadzone(Vec2::new(axis(GamepadAxisType::RightStickX), axis(GamepadAxisType::RightStickY)));
        view.zoom = trigger(GamepadButtonType::RightTrigger2) - trigger(GamepadButtonType::LeftTrigger2);
        view.home = just(GamepadButtonType::RightTrigger);
        view.pause = just(GamepadButtonType::North);
        view.confirm = !cursor && buttons.just_pressed(south);
        view.evacuate = just(GamepadButtonType::West);
        view.team = i32::from(just(GamepadButtonType::DPadRight)) - i32::from(just(GamepadButtonType::DPadLeft));
        view.district = i32::from(just(GamepadButtonType::DPadDown)) - i32::from(just(GamepadButtonType::DPadUp));
    }

    let size = Vec2::new(window.width(), window.height());
    let here = window.cursor_position().unwrap_or(size * 0.5);
    if cursor {
        // Start from where the cursor is, so a real mouse and the pad can mix.
        if left != Vec2::ZERO {
            let step = left * left.length() * CURSOR_SPEED * time.delta_seconds().min(0.1);
            let to = (here + step).clamp(Vec2::ZERO, size - Vec2::ONE);
            window.set_cursor_position(Some(to));
            moved.send(CursorMoved { window: entity, position: to, delta: Some(to - here) });
        } else if window.cursor_position().is_none() {
            window.set_cursor_position(Some(here));
        }
        if buttons.just_pressed(south) {
            *held = true;
            clicks.send(MouseButtonInput { button: MouseButton::Left, state: ButtonState::Pressed, window: entity });
        }
    }
    // a press that began as a mouse button always gets its release
    if *held && buttons.just_released(south) {
        *held = false;
        clicks.send(MouseButtonInput { button: MouseButton::Left, state: ButtonState::Released, window: entity });
    }
    let east = button(GamepadButtonType::East);
    for (state, on) in [(ButtonState::Pressed, buttons.just_pressed(east)), (ButtonState::Released, buttons.just_released(east))] {
        if on {
            keys.send(KeyboardInput { key_code: KeyCode::Escape, logical_key: Key::Escape, state, window: entity });
        }
    }
}
