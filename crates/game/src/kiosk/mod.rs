//! The kiosk: the flow of a visit, around the one simulation and gameplay
//! authority, [`rocca::Game`]. No rules live here: this module holds what the
//! player is choosing (a team, a hex, a district), asks the game for its
//! previews, gives the orders the player confirms, and runs the clock at the
//! game's speed (docs/01-GIOCO.md).
//!
//! Flow: **Intro** (name, «Inizia») → **Tutorial** (day 1: a scripted small
//! fire, one prompted step at a time; the world stands still while a step
//! waits for the player) → **Giorno2** (one screen: a new day, a new fire) →
//! **Gioco** (×300, pauses at ×60, orders only in pause) → **Fine** (stars
//! against the same fire with no orders, the day's board). A new visit starts
//! from the end screen or the operator menu (F2), never by itself.

pub mod board;
pub mod icons;
pub mod marks;
pub mod snap;
pub mod ui;
pub mod view;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::window::WindowMode;
use rocca::tutorial::{Expect, Script};
use rocca::{Axial, EvacPlan, MovePlan, Outcome, Territory};
use scenario::Pos;

use crate::sim::{Sim, SimRestarted};
use crate::{AppState, DataPath};

pub const TITLE: &str = "Isola di Roccavento";
/// Real seconds a notice (a refused order, a closed road) stays on screen.
pub const NOTICE_S: f32 = 4.0;

pub fn window_mode() -> WindowMode {
    if cfg!(target_arch = "wasm32") || std::env::var("KIOSK_WINDOWED").is_ok() {
        WindowMode::Windowed
    } else {
        WindowMode::BorderlessFullscreen
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// The player's name and «Inizia»; the tutorial's fire is on the map, still.
    Intro,
    /// Day 1, the scripted tutorial.
    Tutorial,
    /// Between the tutorial and the game: «Giorno 2».
    Giorno2,
    /// The played game.
    Gioco,
    /// The end: stars and board.
    Fine,
}

/// What the player has selected on the map or in the side bar.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Selection {
    #[default]
    None,
    Team(usize),
    District(usize),
}

#[derive(Resource)]
pub struct Kiosk {
    pub phase: Phase,
    /// Real seconds since the phase began.
    pub phase_t: f32,
    pub territory: Territory,
    pub data: PathBuf,
    pub seed: u64,
    pub player: String,
    /// The tutorial's script and the step on show.
    pub script: Option<Script>,
    pub step: usize,
    pub selection: Selection,
    /// The move being previewed: the team, the hex and the game's answer.
    pub move_plan: Option<Result<MovePlan, String>>,
    pub move_hex: Option<Axial>,
    /// The evacuation being previewed.
    pub evac_plan: Option<Result<EvacPlan, String>>,
    /// `Sim::generation` the previews were computed at.
    plans_at: u64,
    /// A short message (refused order, event) and its age in real seconds.
    pub notice: Option<(String, f32)>,
    /// Log entries already turned into notices.
    pub log_seen: usize,
    /// The same fire with no orders, run beside the game: (case, seed) and
    /// the result when ready.
    pub baseline: Arc<Mutex<Option<(String, u64, Result<Outcome, String>)>>>,
    /// The day's board, and the stamp (day, time) of this game once saved.
    pub board: rocca::score::Board,
    pub saved: Option<(String, u64)>,
    /// Width of the side bar, logical px: the camera frames the map left of it.
    pub side_w: f32,
    /// A ground point clicked on the map this frame (set by [`view::camera`]).
    pub click: Option<Pos>,
    pub follow: bool,
    pub reset_view: bool,
    /// The operator menu (F2).
    pub operator: bool,
    /// «Ferma il mondo» (operator).
    pub frozen: bool,
    pub error: Option<String>,
}

impl Kiosk {
    fn new(territory: Territory, data: PathBuf) -> Kiosk {
        Kiosk {
            phase: Phase::Intro,
            phase_t: 0.0,
            territory,
            data,
            seed: 1,
            player: String::new(),
            script: None,
            step: 0,
            selection: Selection::None,
            move_plan: None,
            move_hex: None,
            evac_plan: None,
            plans_at: 0,
            notice: None,
            log_seen: 0,
            baseline: Default::default(),
            board: board::load(),
            saved: None,
            side_w: 0.0,
            click: None,
            follow: true,
            reset_view: false,
            operator: false,
            frozen: false,
            error: None,
        }
    }

    pub fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_t = 0.0;
        self.clear_selection();
    }

    pub fn clear_selection(&mut self) {
        self.selection = Selection::None;
        self.move_plan = None;
        self.move_hex = None;
        self.evac_plan = None;
    }

    pub fn notify(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), 0.0));
    }

    pub fn tutorial_case(&self) -> String {
        self.territory.tutorial.as_ref().map_or_else(|| self.territory.game.clone(), |t| t.case.clone())
    }

    /// The tutorial step on show.
    pub fn expect(&self) -> Option<Expect> {
        (self.phase == Phase::Tutorial).then(|| self.script.as_ref().and_then(|s| s.steps.get(self.step)).map(|s| s.expect)).flatten()
    }

    /// Whether the player may do this now: in the tutorial only what the
    /// step asks; in the game anything, the game itself refusing what its
    /// rules do not allow.
    pub fn allows(&self, what: Expect) -> bool {
        match self.phase {
            Phase::Gioco => true,
            Phase::Tutorial => match (self.expect(), what) {
                (Some(Expect::Move(t, _)), Expect::Select(u)) => t == u,
                (Some(Expect::Move(t, at)), Expect::Move(u, bt)) => t == u && at == bt,
                (Some(e), w) => e == w,
                (None, _) => false,
            },
            _ => false,
        }
    }

    /// The tutorial goes on when the player did what the step asked.
    pub fn done(&mut self, what: Expect) {
        if self.phase == Phase::Tutorial && self.expect() == Some(what) {
            self.step += 1;
        }
    }

    /// The no-orders game of `case`, when it is done.
    pub fn baseline(&self) -> Option<Result<Outcome, String>> {
        self.baseline.lock().ok().and_then(|b| b.as_ref().map(|(_, _, r)| r.clone()))
    }

    fn start_baseline(&mut self, case: &str) {
        let slot = self.baseline.clone();
        if let Ok(mut s) = slot.lock() {
            *s = None;
        }
        let (data, case, seed) = (self.data.clone(), case.to_string(), self.seed);
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let r = rocca::Game::without_orders(&data, &case, seed).map_err(|e| format!("{e:#}"));
            if let Ok(mut s) = slot.lock() {
                *s = Some((case, seed, r));
            }
        });
        #[cfg(target_arch = "wasm32")]
        {
            // no threads in the browser: computed at the end ([`step`])
            let _ = (data, case, seed, slot);
        }
    }
}

pub fn launch(data: Res<DataPath>, mut commands: Commands, mut next: ResMut<NextState<AppState>>, mut exit: EventWriter<AppExit>) {
    let built = Territory::load(&data.0).and_then(|t| {
        let k = Kiosk::new(t, data.0.clone());
        let sim = Sim::new(&data.0, &k.tutorial_case(), k.seed)?;
        Ok((k, sim))
    });
    match built {
        Ok((k, sim)) => {
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

/// A new visit: the tutorial's fire, still, under the name screen.
pub fn new_visit(k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    let case = k.tutorial_case();
    if let Err(e) = sim.restart(&case, k.seed) {
        k.error = Some(format!("{e:#}"));
    }
    restarted.send(SimRestarted);
    k.player.clear();
    k.saved = None;
    k.log_seen = 0;
    k.script = None;
    k.step = 0;
    k.enter(Phase::Intro);
}

/// «Inizia»: the tutorial, or straight to the game if there is none.
pub fn start_tutorial(k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    if k.territory.tutorial.is_none() {
        start_game(k, sim, restarted);
        return;
    }
    let case = k.tutorial_case();
    if sim.case.name != case || sim.time_s() > rocca::STEP_S {
        if let Err(e) = sim.restart(&case, k.seed) {
            k.error = Some(format!("{e:#}"));
        }
        restarted.send(SimRestarted);
    }
    k.script = Script::of(sim);
    k.step = 0;
    k.log_seen = sim.log.len();
    k.enter(Phase::Tutorial);
}

/// Day 2: the played fire, with its no-orders twin started beside it.
pub fn start_game(k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    let case = k.territory.game.clone();
    if let Err(e) = sim.restart(&case, k.seed) {
        k.error = Some(format!("{e:#}"));
    }
    restarted.send(SimRestarted);
    k.start_baseline(&case);
    k.saved = None;
    k.log_seen = 0;
    k.script = None;
    k.follow = true;
    k.enter(Phase::Gioco);
}

/// The clock and the previews. Runs before the view systems.
pub fn step(time: Res<Time>, mut kiosk: ResMut<Kiosk>, mut sim: ResMut<Sim>) {
    let dt = crate::playtest::delta(time.delta_seconds());
    let k = &mut *kiosk;
    if !k.frozen {
        k.phase_t += dt;
    }
    if let Some((_, age)) = &mut k.notice {
        *age += dt;
        if *age > NOTICE_S {
            k.notice = None;
        }
    }
    sim.speed = match k.phase {
        _ if k.frozen => 0.0,
        // the tutorial waits for the player, then runs by itself
        Phase::Tutorial if k.expect() == Some(Expect::Over) => sim.game.speed(),
        Phase::Gioco => sim.game.speed(),
        _ => 0.0,
    };
    if sim.speed > 0.0 {
        if let Err(e) = crate::playtest::tick(&mut sim, dt) {
            k.error = Some(format!("{e:#}"));
        }
    }
    // what happened, as notices (the latest only)
    if sim.log.len() > k.log_seen {
        if k.phase == Phase::Gioco {
            let e = sim.log.last().unwrap();
            k.notice = Some((format!("{} {}", rocca::words::clock(e.at_s), e.text), 0.0));
        }
        k.log_seen = sim.log.len();
    }
    match k.phase {
        Phase::Tutorial if k.expect() == Some(Expect::Over) && sim.is_over() => k.enter(Phase::Giorno2),
        Phase::Gioco if sim.is_over() => {
            #[cfg(target_arch = "wasm32")]
            if k.baseline().is_none() {
                let r = rocca::Game::without_orders(&k.data, &sim.case.name, k.seed).map_err(|e| format!("{e:#}"));
                if let Ok(mut s) = k.baseline.lock() {
                    *s = Some((sim.case.name.clone(), k.seed, r));
                }
            }
            k.enter(Phase::Fine);
        }
        _ => {}
    }
    // the pause is over: nothing stays half-chosen
    if k.phase == Phase::Gioco && !sim.paused() && k.selection != Selection::None {
        k.clear_selection();
    }
    // previews follow the world (roads close, teams move)
    let stale = sim.generation >= k.plans_at.saturating_add(10);
    if stale {
        k.plans_at = sim.generation;
        if let (Selection::Team(t), Some(at), Some(_)) = (k.selection, k.move_hex, &k.move_plan) {
            k.move_plan = Some(sim.plan_move(t, at));
        }
        if let (Selection::District(d), Some(_)) = (k.selection, &k.evac_plan) {
            k.evac_plan = Some(sim.plan_evacuation(d));
        }
    }
}
