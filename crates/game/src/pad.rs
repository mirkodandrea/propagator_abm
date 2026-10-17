//! The gamepad as a virtual mouse, for the serious game (never while Mario
//! plays: there the pad keeps its own controls, see `mario.rs`).
//!
//! The left stick moves a cursor, A is the left button (hold and move to drag
//! a cone), B is Esc. The right stick pans the map, the triggers zoom, the
//! bumpers turn it. The cursor is written as ordinary window and mouse events
//! before the input systems run, so egui, the map click and the cone drag
//! see no difference from a real mouse.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseButtonInput;
use bevy::input::{ButtonState, InputSystem};
use bevy::prelude::*;
use bevy::window::{CursorMoved, PrimaryWindow};

use crate::mario::Playground;

/// Logical px/s at full stick.
const CURSOR_SPEED: f32 = 1100.0;
const DEADZONE: f32 = 0.18;

/// What the pad asks of the camera this frame, read by `kiosk::view::camera`.
#[derive(Resource, Default)]
pub struct PadView {
    /// Stick units, +y is up on the screen.
    pub pan: Vec2,
    /// +1 closer, -1 farther.
    pub zoom: f32,
    /// +1 turns to the right.
    pub turn: f32,
}

impl PadView {
    pub fn active(&self) -> bool {
        self.pan != Vec2::ZERO || self.zoom != 0.0 || self.turn != 0.0
    }
}

pub struct PadPlugin;

impl Plugin for PadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PadView>().add_systems(
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

    let right = deadzone(Vec2::new(axis(GamepadAxisType::RightStickX), axis(GamepadAxisType::RightStickY)));
    view.pan = right;
    view.zoom = trigger(GamepadButtonType::RightTrigger2) - trigger(GamepadButtonType::LeftTrigger2);
    view.turn = f32::from(buttons.pressed(button(GamepadButtonType::RightTrigger)))
        - f32::from(buttons.pressed(button(GamepadButtonType::LeftTrigger)));

    // Start from where the cursor is, so a real mouse and the pad can mix.
    let size = Vec2::new(window.width(), window.height());
    let here = window.cursor_position().unwrap_or(size * 0.5);
    let stick = deadzone(Vec2::new(axis(GamepadAxisType::LeftStickX), -axis(GamepadAxisType::LeftStickY)));
    if stick != Vec2::ZERO {
        let step = stick * stick.length() * CURSOR_SPEED * time.delta_seconds().min(0.1);
        let to = (here + step).clamp(Vec2::ZERO, size - Vec2::ONE);
        window.set_cursor_position(Some(to));
        moved.send(CursorMoved { window: entity, position: to, delta: Some(to - here) });
    } else if window.cursor_position().is_none() {
        window.set_cursor_position(Some(here));
    }

    let south = button(GamepadButtonType::South);
    for (state, on) in [(ButtonState::Pressed, buttons.just_pressed(south)), (ButtonState::Released, buttons.just_released(south))] {
        if on {
            clicks.send(MouseButtonInput { button: MouseButton::Left, state, window: entity });
        }
    }
    let east = button(GamepadButtonType::East);
    for (state, on) in [(ButtonState::Pressed, buttons.just_pressed(east)), (ButtonState::Released, buttons.just_released(east))] {
        if on {
            keys.send(KeyboardInput { key_code: KeyCode::Escape, logical_key: Key::Escape, state, window: entity });
        }
    }
}
