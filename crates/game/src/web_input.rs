//! Winit's web pointer coordinates use the browser DPR, even when Bevy's
//! backing canvas uses a scale-factor override. Normalize before both Bevy
//! and egui read them, so a 1x canvas has CSS-pixel hit targets on a 3x phone.
use bevy::ecs::event::ManualEventReader;
use bevy::input::touch::TouchInput;
use bevy::prelude::*;
use bevy::window::{CursorMoved, PrimaryWindow};

pub fn normalize(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut touches: ResMut<Events<TouchInput>>,
    mut cursors: ResMut<Events<CursorMoved>>,
    mut touch_reader: Local<ManualEventReader<TouchInput>>,
    mut cursor_reader: Local<ManualEventReader<CursorMoved>>,
) {
    let Ok(mut window) = windows.get_single_mut() else { return };
    let ratio = window.resolution.scale_factor() / window.resolution.base_scale_factor();
    let touch_events: Vec<_> = touch_reader.read(&touches).cloned().map(|mut e| {
        e.position *= ratio;
        e
    }).collect();
    let cursor_events: Vec<_> = cursor_reader.read(&cursors).cloned().map(|mut e| {
        e.position *= ratio;
        e.delta = e.delta.map(|d| d * ratio);
        e
    }).collect();
    if let Some(last) = cursor_events.last() { window.set_cursor_position(Some(last.position)); }
    // Publish only fresh events. Advance our readers past the replacement
    // events so the next frame never repeats a press/release or rescales it.
    touches.clear();
    touches.send_batch(touch_events);
    touch_reader.clear(&touches);
    cursors.clear();
    cursors.send_batch(cursor_events);
    cursor_reader.clear(&cursors);
}
