//! The kiosk demo (`docs/demo-spec.md`): a session shell around the same model
//! and views the workbench uses.
//!
//! Enabled with `DEMO=1` (or `--kiosk`). It replaces the menus, panels and every
//! single-key shortcut with a state machine -- attract, briefing, play, outcome,
//! compare -- and a small Italian UI. The rendering systems are shared with the
//! workbench, so a fix to the terrain or the fire reaches both.
//!
//! **No shortcut can fire here.** `UiFocus::keyboard` is held true for the whole
//! session, which is how the workbench already stops a keystroke reaching the
//! map while a text field is up (finding 25) -- every shortcut system stands
//! down on it, and none of the ones that would not are registered.
//!
//! The headless twin of a session is `demo::Run`; both count with `demo::Tally`,
//! and both step in `demo::STEP_S` chunks, so the COMPARE screen is the same
//! fire with the same agents and one thing different: nobody gave an order.

pub mod strings_it;
mod ui;
mod view;

use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};
use demo::{Outcome, Spec, Tally, STEP_S};

use crate::scenario_selector::DataPath;
use crate::sim::{Sim, SimRestarted};
use crate::AppState;

pub use ui::draw;
pub use view::camera;

pub const SEED: u64 = 42;
/// Real seconds a session's PLAY phase takes, at the fixed demo speed.
pub const PLAY_REAL_S: f32 = 180.0;

fn play_real_s() -> f32 {
    std::env::var("KIOSK_PLAY_S").ok().and_then(|v| v.parse().ok()).unwrap_or(PLAY_REAL_S)
}
/// Simulated seconds per real second while the attract loop runs.
const ATTRACT_SPEED: f32 = 90.0;
/// How long the finished attract fire is left on screen before it relights.
const ATTRACT_HOLD_S: f32 = 5.0;
/// Idle on any screen but PLAY, then back to ATTRACT (spec §5).
const IDLE_RESET_S: f32 = 60.0;
/// Idle during PLAY before "Sei ancora lì?", and how long that waits.
const PLAY_IDLE_WARN_S: f32 = 90.0;
const PLAY_IDLE_GRACE_S: f32 = 30.0;
/// Hold the top-right corner this long for the operator panel.
const OPERATOR_HOLD_S: f32 = 3.0;
/// Most steps one frame may run; at the demo speed this never binds.
const MAX_STEPS_PER_FRAME: u32 = 5;

pub fn enabled() -> bool {
    std::env::var("DEMO").is_ok_and(|v| v != "0") || std::env::args().any(|a| a == "--kiosk")
}

pub fn window_mode() -> WindowMode {
    if std::env::var("KIOSK_WINDOWED").is_ok() {
        WindowMode::Windowed
    } else {
        WindowMode::BorderlessFullscreen
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Attract,
    Briefing,
    Play,
    Outcome,
    Compare,
}

/// Something the UI asked for, run by [`step`] where the whole world is at hand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    /// Attract -> briefing.
    Begin,
    /// Briefing -> play.
    Go,
    /// Same town, fresh run.
    Retry,
    /// Back to attract, on the next town.
    NextTown,
}

#[derive(Resource)]
pub struct Kiosk {
    pub phase: Phase,
    /// Real seconds spent in this phase.
    pub phase_t: f32,
    pub town: usize,
    pub spec: Spec,
    pub tally: Tally,
    /// Real seconds since the last mouse input.
    pub idle_s: f32,
    pub paused: bool,
    pub cmd: Option<Cmd>,
    /// A town switch waiting for the scenario to reload.
    reload: bool,
    accumulator: f32,
    shift_pending: bool,
    pub ordered_at_s: Option<i64>,
    /// What the mission would have come to with nobody giving an order.
    pub counterfactual: Option<Outcome>,
    cf_rx: Option<Mutex<Receiver<Outcome>>>,
    /// The player's own result, frozen when the mission ends.
    pub result: Option<Outcome>,
    /// A one-sentence refusal or confirmation, and how long it stays up.
    pub banner: Option<(String, f32)>,
    /// How long the operator corner has been held.
    pub corner_hold: f32,
    pub operator_open: bool,
    /// Operator pin: stay on this town rather than rotating.
    pub pinned: bool,
    styled: bool,
    pub logo: Option<bevy_egui::egui::TextureId>,
    /// Keeps the logo asset alive: a dropped strong handle unloads it.
    pub(super) logo_handle: Option<Handle<Image>>,
}

impl Kiosk {
    pub fn new(town: usize) -> Kiosk {
        let spec = demo::spec(demo::ALL[town]).expect("demo town has a spec");
        Kiosk {
            phase: Phase::Attract,
            phase_t: 0.0,
            town,
            spec,
            tally: Tally::new(0),
            idle_s: 0.0,
            paused: false,
            cmd: None,
            reload: false,
            accumulator: 0.0,
            shift_pending: spec.shift.is_some(),
            ordered_at_s: None,
            counterfactual: None,
            cf_rx: None,
            result: None,
            banner: None,
            corner_hold: 0.0,
            operator_open: false,
            pinned: false,
            styled: false,
            logo: None,
            logo_handle: None,
        }
    }

    pub fn from_env() -> Kiosk {
        let town = std::env::var("KIOSK_TOWN")
            .ok()
            .and_then(|t| demo::ALL.iter().position(|id| *id == t || id.trim_start_matches("demo_") == t))
            .unwrap_or(0);
        Kiosk::new(town)
    }

    /// Simulated seconds per real second during PLAY.
    pub fn play_speed(&self) -> f32 {
        self.spec.duration_s as f32 / play_real_s()
    }

    pub fn say(&mut self, text: impl Into<String>) {
        self.banner = Some((text.into(), 6.0));
    }

    fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_t = 0.0;
        self.idle_s = 0.0;
    }
}

/// Load the town this session is on and start its headless twin.
pub fn launch(
    data: Res<DataPath>,
    library: Res<crate::library::BehaviourLibrary>,
    mut kiosk: ResMut<Kiosk>,
    mut next_state: ResMut<NextState<AppState>>,
    mut commands: Commands,
    mut windows: Query<&mut Window>,
    mut exit: EventWriter<AppExit>,
) {
    if let Some(bad) = library.load_report.iter().find(|f| !f.ok()) {
        eprintln!("kiosk: behaviour file {} did not load", bad.path.display());
        exit.send(AppExit::error());
        return;
    }
    let spec = kiosk.spec;
    let built = scenario::Scenario::load_by_id(&data.0, spec.id).and_then(|scn| {
        Sim::at_ignition(scn, spec.weather, spec.ignition, spec.radius_m, SEED, library.lib.clone())
    });
    let sim = match built {
        Ok(sim) => sim,
        Err(e) => {
            eprintln!("kiosk: cannot load {}: {e:#}", spec.id);
            exit.send(AppExit::error());
            return;
        }
    };
    if let Ok(mut w) = windows.get_single_mut() {
        w.title = strings_it::TITLE.into();
    }
    // The twin runs while the town is being looked at; it is long finished by
    // the time anyone has played three minutes.
    let (tx, rx) = channel();
    let dir = data.0.clone();
    std::thread::spawn(move || {
        let outcome = demo::Run::new(&dir, spec, SEED).and_then(|mut r| r.play(&[]));
        match outcome {
            Ok(o) => {
                let _ = tx.send(o);
            }
            Err(e) => eprintln!("kiosk: counterfactual failed: {e:#}"),
        }
    });
    kiosk.cf_rx = Some(Mutex::new(rx));
    kiosk.counterfactual = None;
    kiosk.tally = Tally::new(sim.agents.households.len());
    kiosk.shift_pending = spec.shift.is_some();
    kiosk.ordered_at_s = None;
    kiosk.result = None;
    kiosk.reload = false;
    kiosk.accumulator = 0.0;
    kiosk.paused = false;
    kiosk.enter(Phase::Attract);
    commands.insert_resource(sim);
    next_state.set(AppState::Playing);
}

/// A clean run of the current town: same fire, same seed, nothing ordered.
fn restart_session(sim: &mut Sim, kiosk: &mut Kiosk, restarted: &mut EventWriter<SimRestarted>) {
    sim.weather = kiosk.spec.weather;
    match sim.restart() {
        Ok(()) => {
            restarted.send(SimRestarted);
        }
        Err(e) => error!("kiosk restart failed: {e:#}"),
    }
    sim.playing = false;
    kiosk.tally = Tally::new(sim.agents.households.len());
    kiosk.shift_pending = kiosk.spec.shift.is_some();
    kiosk.ordered_at_s = None;
    kiosk.result = None;
    kiosk.accumulator = 0.0;
    kiosk.paused = false;
    kiosk.banner = None;
}

/// Any pointer input resets the idle clock.
pub fn activity(
    mut kiosk: ResMut<Kiosk>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    buttons: Res<ButtonInput<MouseButton>>,
) {
    let moved = motion.read().any(|m| m.delta.length_squared() > 0.0);
    let scrolled = wheel.read().count() > 0;
    if moved || scrolled || buttons.get_just_pressed().next().is_some() {
        kiosk.idle_s = 0.0;
    }
}

/// Advance the session: commands, phase timers, idle resets, and the model.
#[allow(clippy::too_many_arguments)]
pub fn step(
    time: Res<Time>,
    mut kiosk: ResMut<Kiosk>,
    mut sim: ResMut<Sim>,
    mut restarted: EventWriter<SimRestarted>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let dt = time.delta_seconds().min(0.1);
    kiosk.phase_t += dt;
    kiosk.idle_s += dt;
    if let Some((_, left)) = kiosk.banner.as_mut() {
        *left -= dt;
        if *left <= 0.0 {
            kiosk.banner = None;
        }
    }
    let got = kiosk.cf_rx.as_ref().and_then(|rx| rx.lock().unwrap().try_recv().ok());
    if got.is_some() {
        kiosk.counterfactual = got;
    }

    // Idle: spec §5.
    let (phase, idle) = (kiosk.phase, kiosk.idle_s);
    let idle_out = match phase {
        Phase::Attract => false,
        Phase::Play => idle >= PLAY_IDLE_WARN_S + PLAY_IDLE_GRACE_S,
        _ => idle >= IDLE_RESET_S,
    };
    if idle_out {
        kiosk.cmd = Some(Cmd::NextTown);
    }

    if let Some(cmd) = kiosk.cmd.take() {
        match cmd {
            Cmd::Begin => {
                restart_session(&mut sim, &mut kiosk, &mut restarted);
                kiosk.enter(Phase::Briefing);
            }
            Cmd::Go => kiosk.enter(Phase::Play),
            Cmd::Retry => {
                restart_session(&mut sim, &mut kiosk, &mut restarted);
                kiosk.enter(Phase::Play);
            }
            Cmd::NextTown => {
                if !kiosk.pinned {
                    let next = (kiosk.town + 1) % demo::ALL.len();
                    kiosk.town = next;
                    kiosk.spec = demo::spec(demo::ALL[next]).expect("demo town has a spec");
                }
                if kiosk.spec.id != sim.scenario.metadata.id {
                    kiosk.reload = true;
                    next_state.set(AppState::SelectingScenario);
                    return;
                }
                restart_session(&mut sim, &mut kiosk, &mut restarted);
                kiosk.enter(Phase::Attract);
            }
        }
    }

    // The fly-in is skippable and ends by itself.
    if kiosk.phase == Phase::Briefing && kiosk.phase_t >= 20.0 {
        kiosk.enter(Phase::Play);
    }

    let running = match kiosk.phase {
        Phase::Attract => true,
        Phase::Play => !kiosk.paused,
        _ => false,
    };
    sim.playing = running;
    if !running {
        return;
    }
    let speed = if kiosk.phase == Phase::Attract { ATTRACT_SPEED } else { kiosk.play_speed() };
    kiosk.accumulator += dt * speed;
    let mut steps = 0;
    while kiosk.accumulator >= STEP_S as f32 && steps < MAX_STEPS_PER_FRAME {
        kiosk.accumulator -= STEP_S as f32;
        steps += 1;
        if kiosk.shift_pending {
            if let Some(shift) = kiosk.spec.shift {
                if sim.time_s() >= shift.at_s {
                    sim.weather = shift.weather;
                    if let Err(e) = sim.apply_weather() {
                        error!("scripted wind shift failed: {e:#}");
                    }
                    kiosk.shift_pending = false;
                }
            }
        }
        if let Err(e) = sim.advance(STEP_S) {
            error!("fire core failed: {e:#}");
            kiosk.paused = true;
            break;
        }
        let Sim { agents, fire, .. } = &*sim;
        kiosk.tally.note(agents, fire);
        if sim.time_s() >= kiosk.spec.duration_s {
            break;
        }
    }
    // A slow frame must not bank an unbounded backlog.
    kiosk.accumulator = kiosk.accumulator.min(STEP_S as f32 * MAX_STEPS_PER_FRAME as f32);

    if sim.time_s() >= kiosk.spec.duration_s {
        match kiosk.phase {
            Phase::Play => {
                kiosk.result = Some(kiosk.tally.outcome(&sim.agents, &sim.fire, &sim.scenario.world));
                sim.playing = false;
                kiosk.enter(Phase::Outcome);
            }
            // Leave the finished fire on screen a moment, then relight it.
            Phase::Attract if kiosk.phase_t >= ATTRACT_HOLD_S + kiosk.spec.duration_s as f32 / ATTRACT_SPEED => {
                restart_session(&mut sim, &mut kiosk, &mut restarted);
                kiosk.phase_t = 0.0;
            }
            _ => {}
        }
    }
}

/// Reload the scenario when a town switch asked for it. Runs in the selecting
/// state, where `launch` picks up the (new) `Kiosk::spec`.
pub fn wants_reload(kiosk: Res<Kiosk>) -> bool {
    kiosk.reload || kiosk.cf_rx.is_none()
}

pub fn fullscreen(mut windows: Query<&mut Window>) {
    if let Ok(mut w) = windows.get_single_mut() {
        w.mode = window_mode();
        if w.mode != WindowMode::Windowed {
            w.cursor.visible = true;
        }
        let _ = MonitorSelection::Current;
    }
}

/// `KIOSK_SHOT=<dir>`: walk one session unattended and photograph each screen.
/// The only way to review the kiosk without a person at the mouse.
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
    kiosk.idle_s = 0.0;
    match stage.0 {
        0 if stage.1 > 4.0 => { snap("1_attract"); kiosk.cmd = Some(Cmd::Begin); *stage = (1, 0.0); }
        1 if stage.1 > 4.0 => { snap("2_briefing"); kiosk.cmd = Some(Cmd::Go); *stage = (2, 0.0); }
        2 if stage.1 > 2.0 => { sim.agents.order_evacuation_all(); kiosk.ordered_at_s = Some(sim.time_s()); *stage = (3, 0.0); }
        3 if stage.1 > 6.0 => { snap("3_play"); *stage = (4, 0.0); }
        4 if kiosk.phase == Phase::Outcome && kiosk.phase_t > 1.0 => { snap("4_outcome"); kiosk.enter(Phase::Compare); *stage = (5, 0.0); }
        5 if stage.1 > 2.5 && kiosk.counterfactual.is_some() => { snap("5_compare"); *stage = (6, 0.0); }
        6 if stage.1 > 1.0 => { exit.send(AppExit::Success); }
        _ => {}
    }
}
