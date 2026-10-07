//! The kiosk demo (`docs/demo-spec.md`): a session shell around the model.
//!
//! This is the only mode the game has: a state machine -- attract, briefing,
//! play, outcome -- and a small Italian UI. The briefing *is* the first decision:
//! the clock is stopped while the visitor reads the wind and the forecast and
//! warns or defends districts, and starts when they press "Via!".
//!
//! **No shortcut can fire here.** `UiFocus::keyboard` is held true for the whole
//! session (finding 25) -- every shortcut system stands down on it, and none of
//! the ones that would not are registered.
//!
//! **One set of books.** Everything the end card says -- families caught, each
//! district's story, the bill, the town's trust, the events the advisors speak
//! about -- is kept by a `demo::Referee`, the same type the headless twin
//! (`demo::Run`) keeps. Both step in `demo::STEP_S` chunks with the same
//! variant, so the "senza ordini" column is the same fire with the same agents
//! and one thing different: nobody gave an order.

pub mod strings_it;
mod ui;
mod view;

use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::WindowMode;
use demo::{Draw, Event, Forecast, Order, Outcome, Parts, Referee, Spec, Variant, STEP_S};

use crate::sim::{Sim, SimRestarted};
use crate::AppState;
use crate::DataPath;

pub use ui::draw;
pub use view::camera;

pub const SEED: u64 = 42;
/// Real seconds per simulated minute while the decisions are live (the first
/// twenty minutes: warnings stop paying by about T+12, a shift lands by T+26),
/// and after, when the visitor is mostly watching what they set going.
const REAL_S_PER_MIN_EARLY: f32 = 3.0;
const REAL_S_PER_MIN_LATE: f32 = 1.6;
const EARLY_UNTIL_S: i64 = 22 * 60;
/// The fast-forward button multiplies the pace by this.
pub const FAST_X: f32 = 3.0;

/// `KIOSK_PLAY_S=<s>` squeezes a whole session into about this many seconds
/// (screenshot harness). Unset: the pace above.
fn pace_override() -> Option<f32> {
    std::env::var("KIOSK_PLAY_S").ok().and_then(|v| v.parse().ok())
}
/// Simulated seconds per real second while the attract loop runs.
const ATTRACT_SPEED: f32 = 60.0;
/// How long the finished attract fire is left on screen before it relights.
const ATTRACT_HOLD_S: f32 = 5.0;
/// Idle on any screen but PLAY, then back to ATTRACT (spec §5).
const IDLE_RESET_BASE_S: f32 = 60.0;
/// Idle during PLAY before "Sei ancora lì?", and how long that waits.
const PLAY_IDLE_WARN_BASE_S: f32 = 150.0;
const PLAY_IDLE_GRACE_BASE_S: f32 = 30.0;
/// The briefing waits this long for "Via!" before starting by itself.
const BRIEFING_MAX_S: f32 = 75.0;
/// An advisor's bubble stays up this long, and the next waits for it.
const ADVISOR_S: f32 = 7.0;
/// The twin is given this long before COMPARE says it is not available.
const TWIN_TIMEOUT_S: f32 = 20.0;

/// `KIOSK_IDLE_S=<s>` sets the idle reset (default 60) and scales the in-play
/// warning with it, so the reset path can be tested in seconds.
fn idle_scale() -> f32 {
    static S: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *S.get_or_init(|| std::env::var("KIOSK_IDLE_S").ok().and_then(|v| v.parse::<f32>().ok()).map_or(1.0, |s| (s / 60.0).max(0.02)))
}
pub(crate) fn idle_reset_s() -> f32 {
    IDLE_RESET_BASE_S * idle_scale()
}
pub(crate) fn play_idle_warn_s() -> f32 {
    PLAY_IDLE_WARN_BASE_S * idle_scale()
}
pub(crate) fn play_idle_grace_s() -> f32 {
    PLAY_IDLE_GRACE_BASE_S * idle_scale()
}
/// Hold the top-right corner this long for the operator panel.
const OPERATOR_HOLD_S: f32 = 3.0;
/// Most steps one frame may run.
const MAX_STEPS_PER_FRAME: u32 = 8;

/// The rules a session is played under, and its twin replayed under: engines
/// defend the homes they are posted at (spec §4 option B). Inert until an
/// engine is posted, so the no-orders twin is the published model. Cry-wolf
/// (spec 5.2) stays off: no in-session judgement time is both fair to an
/// early warning and early enough to matter (`trust::JUDGE_AFTER_S`); false
/// alarms are judged on the end card instead.
pub fn variant() -> Variant {
    Variant { defend_homes: true, ..Variant::default() }
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
    /// The clock is stopped: read the wind, give the first orders, press "Via!".
    Briefing,
    Play,
    Outcome,
}

/// Something the UI asked for, run by [`step`] where the whole world is at hand.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cmd {
    /// Attract -> briefing.
    Begin,
    /// Briefing -> play.
    Go,
    /// Same town, fresh run.
    Retry,
    /// Back to attract, on the next town.
    NextTown,
    /// Give an order (the model's own vocabulary).
    Order(Order),
    /// Operator: close the kiosk (exit code 0, so the supervisor stops).
    Quit,
}

/// Who is speaking in an advisor bubble.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Speaker {
    /// Capo squadra dei Vigili del fuoco: the fire.
    Fire,
    /// Sindaco: the town, warnings, trust.
    Mayor,
    /// Meteo: the wind and the forecast.
    Weather,
}

/// What the twin came back with: the no-orders run's books.
#[derive(Clone)]
pub struct Twin {
    pub outcome: Outcome,
    pub districts: Vec<demo::district::Report>,
}

#[derive(Resource)]
pub struct Kiosk {
    pub phase: Phase,
    /// Real seconds spent in this phase.
    pub phase_t: f32,
    pub town: usize,
    pub spec: Spec,
    /// This session's seed: weather, forecast and fire all come from it, so a
    /// retry replays the same session and a new visitor gets a new one.
    pub session: u64,
    /// The realised weather and the forecast made from it (spec 6.1).
    pub draw: Draw,
    data_dir: std::path::PathBuf,
    /// Which (town, seed) the running twin was started for.
    twin_for: Option<(&'static str, u64)>,
    /// The books for this session. `None` only before the first town loads.
    pub referee: Option<Referee>,
    /// Real seconds since the last mouse input.
    pub idle_s: f32,
    pub paused: bool,
    pub fast: bool,
    pub cmd: VecDeque<Cmd>,
    accumulator: f32,
    /// When the player first gave any order (idle rule: watching after an
    /// order is playing, not idling).
    pub ordered_at_s: Option<i64>,
    forecast_said: bool,
    /// What the mission comes to with nobody giving an order.
    pub twin: Option<Twin>,
    twin_rx: Option<Mutex<Receiver<Result<Twin, String>>>>,
    /// Real seconds the twin has been running for, and whether it failed.
    pub twin_wait_s: f32,
    pub twin_failed: bool,
    /// The player's own result, frozen when the mission ends.
    pub result: Option<Outcome>,
    /// A one-sentence refusal or confirmation, and how long it stays up.
    pub banner: Option<(String, f32)>,
    /// The advisor speaking now, and how long they have left.
    pub advisor: Option<(Speaker, String, f32)>,
    advisor_queue: VecDeque<(Speaker, String)>,
    events_seen: usize,
    /// The district the pointer last opened, for the map chip's menu.
    pub open_district: Option<usize>,
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
        let draw = demo::draw(demo::ALL[town], SEED).expect("demo town has a spec");
        let spec = draw.spec;
        Kiosk {
            phase: Phase::Attract,
            phase_t: 0.0,
            town,
            spec,
            session: SEED,
            draw,
            data_dir: std::path::PathBuf::new(),
            twin_for: None,
            referee: None,
            idle_s: 0.0,
            paused: false,
            fast: false,
            cmd: VecDeque::new(),
            accumulator: 0.0,
            ordered_at_s: None,
            forecast_said: false,
            twin: None,
            twin_rx: None,
            twin_wait_s: 0.0,
            twin_failed: false,
            result: None,
            banner: None,
            advisor: None,
            advisor_queue: VecDeque::new(),
            events_seen: 0,
            open_district: None,
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

    /// Simulated seconds per real second during PLAY, at this simulated time.
    pub fn play_speed(&self, time_s: i64) -> f32 {
        let base = match pace_override() {
            Some(real_s) => self.spec.duration_s as f32 / real_s.max(1.0),
            None => 60.0 / if time_s < EARLY_UNTIL_S { REAL_S_PER_MIN_EARLY } else { REAL_S_PER_MIN_LATE },
        };
        if self.fast { base * FAST_X } else { base }
    }

    pub fn say(&mut self, text: impl Into<String>) {
        self.banner = Some((text.into(), 6.0));
    }

    /// Queue an advisor's line; they speak one at a time.
    pub fn advise(&mut self, who: Speaker, text: impl Into<String>) {
        let text = text.into();
        if self.advisor_queue.len() < 4 && !self.advisor_queue.iter().any(|(_, t)| *t == text) {
            self.advisor_queue.push_back((who, text));
        }
    }

    /// Draw this town's weather for `seed` and make it the session.
    fn redraw(&mut self, seed: u64) {
        self.session = seed;
        self.draw = demo::draw(demo::ALL[self.town], seed).expect("demo town has a spec");
        self.spec = self.draw.spec;
    }

    /// The forecast on show at this simulated time: the briefing's until the
    /// second issue is due.
    pub fn forecast_at(&self, time_s: i64) -> Forecast {
        self.draw.forecast(if time_s >= demo::ISSUE_2_AT_S { 2 } else { 1 })
    }

    /// Start the headless no-orders twin for this session unless it is already
    /// running for it. Long finished by the time anyone has played a minute.
    fn ensure_twin(&mut self) {
        let key = (self.spec.id, self.session);
        if self.twin_for == Some(key) && (self.twin_rx.is_some() || self.twin.is_some()) {
            return;
        }
        let (tx, rx) = channel();
        let (dir, spec, seed) = (self.data_dir.clone(), self.spec, self.session);
        std::thread::spawn(move || {
            let r = demo::Run::with_variant(&dir, spec, seed, variant())
                .and_then(|mut r| {
                    let outcome = r.play(&[])?;
                    Ok(Twin { outcome, districts: r.referee.reports.clone() })
                })
                .map_err(|e| format!("{e:#}"));
            // The receiver is dropped when a newer twin replaces this one.
            let _ = tx.send(r);
        });
        self.twin_rx = Some(Mutex::new(rx));
        self.twin = None;
        self.twin_wait_s = 0.0;
        self.twin_failed = false;
        self.twin_for = Some(key);
    }

    fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_t = 0.0;
        self.idle_s = 0.0;
        self.open_district = None;
    }

    /// The districts' names, in the referee's order.
    pub fn district_names(&self) -> Vec<String> {
        self.referee.as_ref().map(|r| r.districts.iter().map(|d| d.name.clone()).collect()).unwrap_or_default()
    }
}

/// A fresh set of books for the town the `Sim` has loaded.
fn new_referee(kiosk: &Kiosk, sim: &Sim) -> Referee {
    Referee::new(kiosk.spec, &sim.scenario, &sim.agents, variant())
}

/// Load the town this session is on and start its headless twin.
#[allow(clippy::too_many_arguments)]
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
    kiosk.data_dir = data.0.clone();
    let built = scenario::Scenario::load_by_id(&data.0, spec.id).and_then(|scn| {
        Sim::at_ignition(scn, spec.weather, spec.ignition, spec.radius_m, kiosk.session, library.lib.clone())
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
    kiosk.ensure_twin();
    kiosk.referee = Some(new_referee(&kiosk, &sim));
    clear_session(&mut kiosk);
    kiosk.enter(Phase::Attract);
    commands.insert_resource(sim);
    next_state.set(AppState::Playing);
}

/// Everything a session latches, back to its opening state (finding 21).
fn clear_session(kiosk: &mut Kiosk) {
    kiosk.ordered_at_s = None;
    kiosk.forecast_said = false;
    kiosk.result = None;
    kiosk.accumulator = 0.0;
    kiosk.paused = false;
    kiosk.fast = false;
    kiosk.banner = None;
    kiosk.advisor = None;
    kiosk.advisor_queue.clear();
    kiosk.events_seen = 0;
    kiosk.open_district = None;
}

/// A clean run of the current town: same fire, same seed, nothing ordered.
fn restart_session(sim: &mut Sim, kiosk: &mut Kiosk, restarted: &mut EventWriter<SimRestarted>) {
    sim.weather = kiosk.spec.weather;
    sim.seed = kiosk.session;
    kiosk.ensure_twin();
    match sim.restart() {
        Ok(()) => {
            restarted.send(SimRestarted);
        }
        Err(e) => error!("kiosk restart failed: {e:#}"),
    }
    sim.playing = false;
    kiosk.referee = Some(new_referee(kiosk, sim));
    clear_session(kiosk);
}

/// Any pointer input resets the idle clock.
pub fn activity(
    mut kiosk: ResMut<Kiosk>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    mut pinch: EventReader<bevy::input::gestures::PinchGesture>,
    mut rotation: EventReader<bevy::input::gestures::RotationGesture>,
    buttons: Res<ButtonInput<MouseButton>>,
) {
    let moved = motion.read().any(|m| m.delta.length_squared() > 0.0);
    let scrolled = wheel.read().count() > 0;
    let pinched = pinch.read().count() > 0;
    let rotated = rotation.read().count() > 0;
    if moved || scrolled || pinched || rotated || buttons.get_just_pressed().next().is_some() {
        kiosk.idle_s = 0.0;
    }
}

/// Give an order through the books, so the twin, the bill and the trust meter
/// all see it.
fn give(kiosk: &mut Kiosk, sim: &mut Sim, o: Order) {
    let Some(referee) = kiosk.referee.as_mut() else { return };
    let Sim { scenario, fire, agents, crews, .. } = sim;
    referee.order(o, Parts { scn: scenario, fire, agents, crews });
    kiosk.ordered_at_s.get_or_insert(sim.fire.time_s());
    sim.generation += 1;
}

/// Turn new model events into advisor lines (the model says what happened, the
/// strings file says it in Italian).
fn listen(kiosk: &mut Kiosk) {
    let Some(referee) = kiosk.referee.as_ref() else { return };
    let names: Vec<String> = referee.districts.iter().map(|d| d.name.clone()).collect();
    let new: Vec<Event> = referee.events[kiosk.events_seen.min(referee.events.len())..].to_vec();
    kiosk.events_seen = referee.events.len();
    let warned: Vec<bool> = referee.reports.iter().map(|r| r.warned_at_s.is_some()).collect();
    for e in new {
        if let Some((who, text)) = strings_it::advisor(&e.kind, &names, &warned) {
            kiosk.advise(who, text);
        }
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
    mut exit: EventWriter<AppExit>,
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
    // The twin: a result, a failure, or a timeout -- never an endless spinner.
    if kiosk.twin.is_none() && !kiosk.twin_failed {
        kiosk.twin_wait_s += dt;
        let got = kiosk.twin_rx.as_ref().and_then(|rx| rx.lock().ok().and_then(|rx| rx.try_recv().ok()));
        match got {
            Some(Ok(t)) => {
                kiosk.twin = Some(t);
                kiosk.twin_rx = None;
            }
            Some(Err(e)) => {
                error!("kiosk: counterfactual failed: {e}");
                kiosk.twin_failed = true;
                kiosk.twin_rx = None;
            }
            None if kiosk.twin_wait_s > TWIN_TIMEOUT_S => kiosk.twin_failed = true,
            None => {}
        }
    }
    // Advisors: one at a time.
    if let Some((_, _, left)) = kiosk.advisor.as_mut() {
        *left -= dt;
        if *left <= 0.0 {
            kiosk.advisor = None;
        }
    }
    if kiosk.advisor.is_none() && kiosk.phase == Phase::Play {
        if let Some((who, text)) = kiosk.advisor_queue.pop_front() {
            kiosk.advisor = Some((who, text, ADVISOR_S));
        }
    }

    // Idle: spec §5.
    let (phase, idle) = (kiosk.phase, kiosk.idle_s);
    let idle_out = match phase {
        Phase::Attract => false,
        // Watching the fire after an order is playing, not idling; pause suspends it.
        Phase::Play => !kiosk.paused && kiosk.ordered_at_s.is_none() && idle >= play_idle_warn_s() + play_idle_grace_s(),
        _ => idle >= idle_reset_s(),
    };
    if idle_out {
        kiosk.cmd.push_back(Cmd::NextTown);
    }

    while let Some(cmd) = kiosk.cmd.pop_front() {
        match cmd {
            Cmd::Begin => {
                // A new visitor gets a new draw; a retry (below) replays this one.
                let next = kiosk.session.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                kiosk.redraw(next);
                restart_session(&mut sim, &mut kiosk, &mut restarted);
                kiosk.enter(Phase::Briefing);
            }
            Cmd::Go => {
                kiosk.enter(Phase::Play);
                kiosk.advise(Speaker::Fire, strings_it::ADVISOR_START);
            }
            Cmd::Retry => {
                restart_session(&mut sim, &mut kiosk, &mut restarted);
                kiosk.enter(Phase::Briefing);
            }
            Cmd::NextTown => {
                if !kiosk.pinned {
                    let next = (kiosk.town + 1) % demo::ALL.len();
                    kiosk.town = next;
                    let seed = kiosk.session;
                    kiosk.redraw(seed);
                }
                if kiosk.spec.id != sim.scenario.metadata.id {
                    next_state.set(AppState::SelectingScenario);
                    kiosk.cmd.clear();
                    return;
                }
                restart_session(&mut sim, &mut kiosk, &mut restarted);
                kiosk.enter(Phase::Attract);
            }
            Cmd::Order(o) => {
                if matches!(kiosk.phase, Phase::Briefing | Phase::Play) {
                    give(&mut kiosk, &mut sim, o);
                }
            }
            Cmd::Quit => {
                exit.send(AppExit::Success);
                return;
            }
        }
    }

    // The briefing waits for "Via!", but not forever.
    if kiosk.phase == Phase::Briefing && kiosk.phase_t >= BRIEFING_MAX_S * idle_scale().max(0.2) {
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
    let speed = if kiosk.phase == Phase::Attract { ATTRACT_SPEED } else { kiosk.play_speed(sim.time_s()) };
    kiosk.accumulator += dt * speed;
    let mut steps = 0;
    while kiosk.accumulator >= STEP_S as f32 && steps < MAX_STEPS_PER_FRAME {
        kiosk.accumulator -= STEP_S as f32;
        steps += 1;
        if !kiosk.forecast_said && sim.time_s() >= demo::ISSUE_2_AT_S && kiosk.phase == Phase::Play {
            kiosk.forecast_said = true;
            let f = kiosk.forecast_at(sim.time_s());
            kiosk.advise(Speaker::Weather, strings_it::forecast_update(f.shift_p, ui::bearing_name(f.shift_to_deg)));
        }
        let shifted = {
            let Sim { fire, crews, .. } = &mut *sim;
            match kiosk.referee.as_mut().map(|r| r.before_step(fire, crews)) {
                Some(Ok(s)) => s,
                Some(Err(e)) => {
                    error!("wind shift failed: {e:#}");
                    false
                }
                None => false,
            }
        };
        if shifted {
            sim.weather = sim.fire.weather();
            sim.generation += 1;
        }
        if let Err(e) = sim.advance(STEP_S) {
            error!("fire core failed: {e:#}");
            kiosk.paused = true;
            break;
        }
        if let Some(referee) = kiosk.referee.as_mut() {
            let Sim { scenario, fire, agents, crews, .. } = &mut *sim;
            referee.after_step(Parts { scn: scenario, fire, agents, crews });
        }
        if kiosk.phase == Phase::Play {
            listen(&mut kiosk);
        } else if let Some(r) = kiosk.referee.as_ref() {
            kiosk.events_seen = r.events.len();
        }
        if sim.time_s() >= kiosk.spec.duration_s {
            break;
        }
    }
    // A slow frame must not bank an unbounded backlog.
    kiosk.accumulator = kiosk.accumulator.min(STEP_S as f32 * MAX_STEPS_PER_FRAME as f32);

    if sim.time_s() >= kiosk.spec.duration_s {
        match kiosk.phase {
            Phase::Play => {
                kiosk.result = kiosk.referee.as_ref().map(|r| r.tally.outcome(&sim.agents, &sim.fire, &sim.scenario.world));
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

/// `KIOSK_SHOT=<dir>`: walk one session unattended and photograph each screen.
/// The only way to review the kiosk without a person at the mouse. It plays a
/// sensible commander: the downwind district warned and defended at the
/// briefing, so the screens show a session with something going on.
#[allow(clippy::too_many_arguments)]
pub fn shots(
    time: Res<Time>,
    mut kiosk: ResMut<Kiosk>,
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
        0 if stage.1 > 4.0 => {
            snap("1_attract");
            kiosk.cmd.push_back(Cmd::Begin);
            *stage = (1, 0.0);
        }
        1 if stage.1 > 5.0 => {
            kiosk.open_district = Some(0);
            *stage = (8, 0.0);
        }
        8 if stage.1 > 1.0 => {
            snap("2_briefing");
            kiosk.cmd.push_back(Cmd::Order(Order::EvacuateDistrict(0)));
            kiosk.cmd.push_back(Cmd::Order(Order::Defend { kind: abm::suppression::UnitKind::Engine, district: 0 }));
            kiosk.cmd.push_back(Cmd::Order(Order::Defend { kind: abm::suppression::UnitKind::Engine, district: 0 }));
            *stage = (9, 0.0);
        }
        9 if stage.1 > 1.0 => {
            snap("2b_briefing_ordered");
            kiosk.cmd.push_back(Cmd::Go);
            *stage = (2, 0.0);
        }
        2 if stage.1 > 6.0 => {
            snap("3_play");
            *stage = (7, 0.0);
        }
        7 if stage.1 > 8.0 => {
            snap("3b_play_late");
            *stage = (4, 0.0);
        }
        4 if kiosk.phase == Phase::Outcome && kiosk.phase_t > 1.0 && (kiosk.twin.is_some() || kiosk.twin_failed) => {
            snap("4_outcome");
            *stage = (5, 0.0);
        }
        5 if stage.1 > 2.0 => {
            snap("5_outcome_later");
            *stage = (6, 0.0);
        }
        6 if stage.1 > 1.0 => {
            exit.send(AppExit::Success);
        }
        _ => {}
    }
}
