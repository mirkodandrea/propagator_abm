//! Native development launcher: remember the window across rebuilds.
use bevy::{app::AppExit, prelude::*, window::PrimaryWindow};
use std::{num::NonZeroU8, path::PathBuf};

#[derive(Resource)]
struct SavedWindow(PathBuf);

pub fn install(app: &mut App) {
    // Explicit opt-in: kiosk and screenshot runs keep their existing defaults.
    let Some(path) = std::env::var_os("KIOSK_WINDOW_STATE") else { return };
    let path = PathBuf::from(path);
    if let Some(mut window) = app.world_mut().query_filtered::<&mut Window, With<PrimaryWindow>>()
        .iter_mut(app.world_mut()).next()
    {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                restore(&mut window, &value);
            }
        }
    }
    app.insert_resource(SavedWindow(path))
        .add_systems(Update, (save, restart).chain());
}

fn restore(window: &mut Window, value: &serde_json::Value) {
    if let (Some(x), Some(y)) = (value["x"].as_i64(), value["y"].as_i64()) {
        if let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) {
            window.position = bevy::window::WindowPosition::At(IVec2::new(x, y));
        }
    }
    // An explicit size on the command line takes precedence over the saved one.
    if std::env::var_os("KIOSK_SIZE").is_none() {
        if let (Some(w), Some(h)) = (value["width"].as_f64(), value["height"].as_f64()) {
            if w.is_finite() && h.is_finite() && w >= 100.0 && h >= 100.0 && w <= 16384.0 && h <= 16384.0 {
                window.resolution.set(w as f32, h as f32);
            }
        }
    }
}

fn save(
    windows: Query<&Window, With<PrimaryWindow>>,
    path: Res<SavedWindow>,
    mut previous: Local<Option<serde_json::Value>>,
) {
    let Ok(window) = windows.get_single() else { return };
    if window.mode != bevy::window::WindowMode::Windowed { return; }
    let bevy::window::WindowPosition::At(pos) = window.position else { return };
    if window.physical_width() == 0 || window.physical_height() == 0 { return; }
    let value = serde_json::json!({
        "x": pos.x, "y": pos.y,
        "width": window.width(), "height": window.height(),
    });
    if previous.as_ref() == Some(&value) { return; }
    if let Err(error) = std::fs::write(&path.0, value.to_string()) {
        warn!("Impossibile salvare la posizione della finestra: {error}");
    }
    *previous = Some(value);
}

fn restart(
    keys: Res<ButtonInput<KeyCode>>,
    mut contexts: bevy_egui::EguiContexts,
    mut exit: EventWriter<AppExit>,
) {
    if std::env::var_os("KIOSK_RELOAD").is_some()
        && keys.just_pressed(KeyCode::F5)
        && !contexts.ctx_mut().wants_keyboard_input()
    {
        exit.send(AppExit::Error(NonZeroU8::new(75).unwrap()));
    }
}
