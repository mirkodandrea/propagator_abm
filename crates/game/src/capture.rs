//! Screenshot capture, for iterating on the rendering without a human at the
//! keyboard.
//!
//! `F12` grabs the current frame. Setting `SPOTORNO_SHOT=<dir>` instead runs
//! the scenario unattended to `SPOTORNO_SHOT_AT` simulated seconds (default
//! 30 min), captures one frame per fire layer, and exits — which is what makes
//! a visual change reviewable from a terminal.

use std::path::PathBuf;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::view::screenshot::ScreenshotManager;
use bevy::window::PrimaryWindow;

use scenario::Pos;

use crate::camera::OrbitCamera;
use crate::fire_view::FireLayer;
use crate::sim::Sim;

/// Frames to let the renderer settle after switching layer. The overlay
/// rebuild is throttled, so grabbing on the very next frame can catch the
/// previous layer still on screen.
const SETTLE_FRAMES: u32 = 30;

#[derive(Resource)]
pub struct Capture {
    dir: PathBuf,
    at_sim_s: i64,
    /// Camera orbit distance to shoot from, when asked. Close shots are how
    /// the vegetation and flame detail gets reviewed at all.
    distance: Option<f32>,
    /// World-frame point to look at, when the interesting thing is not the
    /// fire -- the town, a junction, a refuge.
    focus: Option<Pos>,
    remaining: Vec<FireLayer>,
    settle: u32,
}

/// `Some(Capture)` only when `SPOTORNO_SHOT` asks for one.
pub fn from_env() -> Option<Capture> {
    let dir = PathBuf::from(std::env::var("SPOTORNO_SHOT").ok()?);
    std::fs::create_dir_all(&dir).ok()?;
    let at_sim_s = std::env::var("SPOTORNO_SHOT_AT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1800);
    let distance = std::env::var("SPOTORNO_SHOT_DIST").ok().and_then(|v| v.parse().ok());
    // "x,y" in world metres.
    let focus = std::env::var("SPOTORNO_SHOT_FOCUS").ok().and_then(|v| {
        let (x, y) = v.split_once(',')?;
        Some(Pos { x: x.trim().parse().ok()?, y: y.trim().parse().ok()? })
    });
    // A single layer can be named, for iterating on one view.
    let only = std::env::var("SPOTORNO_SHOT_LAYER").ok();
    let mut remaining: Vec<FireLayer> = FireLayer::ALL
        .into_iter()
        .filter(|l| only.as_deref().map_or(true, |o| l.label().eq_ignore_ascii_case(o)))
        .collect();
    remaining.reverse(); // popped from the back
    Some(Capture { dir, at_sim_s, distance, focus, remaining, settle: SETTLE_FRAMES })
}

pub fn manual(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
    mut counter: Local<u32>,
) {
    if !keys.just_pressed(KeyCode::F12) {
        return;
    }
    let Ok(window) = windows.get_single() else {
        return;
    };
    let path = format!("spotorno_{:03}.png", *counter);
    *counter += 1;
    if let Err(e) = shots.save_screenshot_to_disk(window, &path) {
        warn!("screenshot failed: {e}");
    } else {
        info!("wrote {path}");
    }
}

pub fn scripted(
    mut capture: ResMut<Capture>,
    mut sim: ResMut<Sim>,
    mut layer: ResMut<FireLayer>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut orbit: Query<&mut OrbitCamera>,
    mut shots: ResMut<ScreenshotManager>,
    diagnostics: Res<DiagnosticsStore>,
    mut exit: EventWriter<AppExit>,
) {
    // Fast-forward to the moment of interest before touching the camera.
    if sim.time_s() < capture.at_sim_s {
        sim.playing = true;
        sim.speed = 512.0;
        return;
    }
    // Keep the fire alive under the camera: flames, smoke and embers are all
    // functions of simulated time, and a frozen sim photographs as a still.
    sim.playing = true;
    sim.speed = 1.0;

    if let Some(focus) = capture.focus.take() {
        if let Ok(mut orbit) = orbit.get_single_mut() {
            orbit.focus = crate::frame::to_bevy(focus, sim.scenario.terrain.height_at(focus));
        }
    }

    if let Some(distance) = capture.distance.take() {
        if let Ok(mut orbit) = orbit.get_single_mut() {
            orbit.distance = distance;
            // A close shot wants a shallower angle: from 300 m, looking
            // straight down shows nothing but the top of the canopy.
            orbit.pitch = -0.35;
        }
        capture.settle = SETTLE_FRAMES;
        return;
    }

    // Settling comes first, and also covers the frames after the *last* shot:
    // `save_screenshot_to_disk` hands the request to the render world, so
    // exiting on the same frame loses the file.
    if capture.settle > 0 {
        capture.settle -= 1;
        return;
    }
    let Some(next) = capture.remaining.last().copied() else {
        exit.send(AppExit::Success);
        return;
    };
    if *layer != next {
        *layer = next;
        capture.settle = SETTLE_FRAMES;
        return;
    }

    capture.remaining.pop();
    let Ok(window) = windows.get_single() else {
        return;
    };
    let path = capture.dir.join(format!("{}.png", next.label().to_lowercase()));
    // Frame rate at the moment of capture: the vegetation budget is decided
    // by this number, so it belongs next to the picture it paid for.
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(f64::NAN);
    match shots.save_screenshot_to_disk(window, &path) {
        Ok(()) => info!("wrote {} at {fps:.0} fps", path.display()),
        Err(e) => warn!("screenshot failed: {e}"),
    }
    capture.settle = SETTLE_FRAMES;
}
