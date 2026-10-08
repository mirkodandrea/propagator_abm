//! The kiosk: one territory, one game, started and restarted by the operator.
//!
//! Everything the game decides is in [`rocca::Game`]; this module only holds
//! what the player is composing (the proposed plan), asks the game for its
//! preview, commits it on request and runs the clock. No rules live here.
//!
//! Flow: **Pianifica** (×0, the proposed plan is previewed on the map) →
//! **Esegui** (×N) → **Fine** (facts per district). No inactivity reset: a new
//! game starts only from the operator panel.

pub mod ui;
pub mod view;

use bevy::prelude::*;
use bevy::window::WindowMode;
use rocca::{Plan, Proposal, Territory};

use crate::sim::{Sim, SimRestarted};
use crate::{AppState, DataPath};

pub const TITLE: &str = "Rocca Ventosa";
/// Simulated seconds per real second while the game runs (spec: ~×20).
pub const RUN_SPEED: f32 = 20.0;

pub fn window_mode() -> WindowMode {
    if std::env::var("KIOSK_WINDOWED").is_ok() {
        WindowMode::Windowed
    } else {
        WindowMode::BorderlessFullscreen
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// ×0: the player ranks places and chooses civil orders.
    Pianifica,
    /// ×N: the plan in force is being carried out.
    Esegui,
    /// The case has run its course.
    Fine,
}

#[derive(Resource)]
pub struct Kiosk {
    pub phase: Phase,
    /// Real seconds since the phase began.
    pub phase_t: f32,
    pub territory: Territory,
    pub case: String,
    pub seed: u64,
    /// The plan being composed: what the map previews.
    pub proposed: Plan,
    /// The coordinator's answer to `proposed`, from the state now.
    pub preview: Option<Proposal>,
    /// `Sim::generation` the preview was computed at, and whether the
    /// proposed plan changed since.
    preview_at: u64,
    pub dirty: bool,
    /// Simulated seconds per real second while running.
    pub speed: f32,
    pub error: Option<String>,
}

impl Kiosk {
    fn new(territory: Territory) -> Kiosk {
        let case = std::env::var("KIOSK_CASE").ok().filter(|c| territory.case(c).is_some()).unwrap_or_else(|| territory.cases[0].name.clone());
        let speed = std::env::var("KIOSK_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(RUN_SPEED);
        Kiosk {
            phase: Phase::Pianifica,
            phase_t: 0.0,
            territory,
            case,
            seed: 1,
            proposed: Plan::default(),
            preview: None,
            preview_at: u64::MAX,
            dirty: true,
            speed,
            error: None,
        }
    }

    pub fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_t = 0.0;
    }
}

/// Load the territory and start the first game.
pub fn launch(data: Res<DataPath>, mut commands: Commands, mut next: ResMut<NextState<AppState>>, mut exit: EventWriter<AppExit>) {
    let built = Territory::load(&data.0).and_then(|t| {
        let k = Kiosk::new(t);
        let sim = Sim::new(&data.0, &k.case, k.seed)?;
        Ok((k, sim))
    });
    match built {
        Ok((mut k, sim)) => {
            k.proposed = Plan::new(sim.districts.len());
            commands.insert_resource(k);
            commands.insert_resource(sim);
            next.set(AppState::Playing);
        }
        Err(e) => {
            eprintln!("kiosk: {e:#}");
            exit.send(AppExit::error());
        }
    }
}

/// New game, same territory: the operator's manual restart.
pub fn new_game(kiosk: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    match sim.restart(&kiosk.case, kiosk.seed) {
        Ok(()) => {
            kiosk.proposed = Plan::new(sim.districts.len());
            kiosk.dirty = true;
            kiosk.error = None;
            kiosk.enter(Phase::Pianifica);
            restarted.send(SimRestarted);
        }
        Err(e) => kiosk.error = Some(format!("{e:#}")),
    }
}

/// Commit the proposed plan: the game revalidates it from the state now.
pub fn commit(kiosk: &mut Kiosk, sim: &mut Sim) {
    match sim.commit(kiosk.proposed.clone()) {
        Ok(_) => {
            kiosk.proposed = sim.active.clone();
            kiosk.dirty = true;
            sim.generation += 1;
        }
        Err(e) => kiosk.error = Some(format!("{e:#}")),
    }
}

/// Run the clock and keep the preview current.
pub fn step(time: Res<Time>, mut kiosk: ResMut<Kiosk>, mut sim: ResMut<Sim>) {
    let dt = time.delta_seconds();
    kiosk.phase_t += dt;
    sim.speed = if kiosk.phase == Phase::Esegui { kiosk.speed } else { 0.0 };
    if kiosk.phase == Phase::Esegui {
        if let Err(e) = sim.tick(dt) {
            kiosk.error = Some(format!("{e:#}"));
            kiosk.enter(Phase::Fine);
        }
        if sim.time_s() >= sim.case.duration_s() {
            kiosk.enter(Phase::Fine);
        }
    }
    // The preview follows the plan being composed and the world as it moves,
    // at most once a simulated review interval while running.
    let stale = kiosk.dirty || (kiosk.phase == Phase::Esegui && sim.generation >= kiosk.preview_at.saturating_add(20));
    if stale && kiosk.phase != Phase::Fine {
        kiosk.preview = sim.preview(&kiosk.proposed).ok();
        kiosk.preview_at = sim.generation;
        kiosk.dirty = false;
    }
}

/// `KIOSK_SHOT=<dir>`: play one scripted game and save screenshots of each
/// phase, then quit. For checking the rendering without a person at the mouse.
#[allow(clippy::too_many_arguments)]
pub fn shots(
    time: Res<Time>,
    mut kiosk: ResMut<Kiosk>,
    mut sim: ResMut<Sim>,
    windows: Query<Entity, With<bevy::window::PrimaryWindow>>,
    mut mgr: ResMut<bevy::render::view::screenshot::ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut stage: Local<(u32, f32)>,
) {
    let Ok(dir) = std::env::var("KIOSK_SHOT") else { return };
    let Ok(window) = windows.get_single() else { return };
    let _ = std::fs::create_dir_all(&dir);
    stage.1 += time.delta_seconds();
    let mut snap = |name: &str| {
        let _ = mgr.save_screenshot_to_disk(window, format!("{dir}/{name}.png"));
    };
    match stage.0 {
        0 if stage.1 > 4.0 => {
            snap("1_pianifica");
            // the locality the fire was picked for first, then the others
            let near = sim.district_index(&sim.case.near).unwrap_or(0);
            let mut prio = vec![near];
            prio.extend((0..sim.districts.len()).filter(|&d| d != near));
            kiosk.proposed = Plan::new(sim.districts.len()).with_priorities(&prio).with_civil(near, rocca::Civil::Preallerta);
            kiosk.dirty = true;
            *stage = (1, 0.0);
        }
        1 if stage.1 > 2.0 => {
            snap("2_anteprima");
            commit(&mut kiosk, &mut sim);
            kiosk.enter(Phase::Esegui);
            *stage = (2, 0.0);
        }
        2 if stage.1 > 8.0 => {
            snap("3_esegui");
            *stage = (3, 0.0);
        }
        3 if kiosk.phase == Phase::Fine && stage.1 > 1.0 => {
            snap("4_fine");
            *stage = (4, 0.0);
        }
        4 if stage.1 > 1.0 => {
            exit.send(AppExit::Success);
        }
        _ => {}
    }
}
