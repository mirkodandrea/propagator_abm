//! The kiosk: one territory, one game, started and restarted by the operator.
//!
//! Everything the game decides is in [`rocca::Game`]; this module only holds
//! what the player is composing (the proposed plan), asks the game for its
//! preview, commits it on request and runs the clock. No rules live here.
//!
//! Flow: **Pianifica** (×0, the proposed plan is previewed on the map) →
//! **Esegui** (×N) ⇄ **Crisi** (×1 with a countdown, raised by the game) →
//! **Fine** (facts per district, against the same fire with no orders). No
//! inactivity reset: a new game starts from the debrief's «Riprova» / «Altro
//! incendio» or from the operator bar (F2), never by itself.

pub mod characters;
pub mod icons;
pub mod ui;
pub mod view;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::window::WindowMode;
use rocca::district::Exposure;
use rocca::{Game, Outcome, Plan, Proposal, Territory};

use crate::sim::{Sim, SimRestarted};
use crate::{AppState, DataPath};

pub const TITLE: &str = "Rocca Ventosa";
/// Simulated seconds per real second while the game runs (spec §2: ×40;
/// ×20 gave 9-minute games and a lull after the orders).
pub const RUN_SPEED: f32 = 40.0;
/// Real seconds the player has at a crisis (spec: ~25 s at ×1).
pub const CRISIS_S: f32 = 25.0;
/// How many times faster the game runs while nothing is happening
/// (`rocca::Game::is_quiet`): the waits after the orders were the dull part.
/// ×40·3 = ×120.
pub const QUIET_BOOST: f32 = 3.0;
/// Simulated seconds without events before the game counts as quiet.
pub const QUIET_S: i64 = 10 * 60;

pub fn window_mode() -> WindowMode {
    if cfg!(target_arch = "wasm32") || std::env::var("KIOSK_WINDOWED").is_ok() {
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
    /// ×1: a crisis. The active plan goes on; the proposed one is applied on
    /// confirmation or when the countdown runs out.
    Crisi,
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
    /// The crisis being decided, while in [`Phase::Crisi`].
    pub crisis: Option<rocca::Crisis>,
    /// Per district, how the fire stands to it now (refreshed with the preview).
    pub risk: Vec<Option<Exposure>>,
    /// Free-flow journey range in minutes and homes without a route.
    pub evacuation: Vec<(Option<(u32, u32)>, usize)>,
    /// The same case and seed with no orders, run beside the game for the
    /// debrief: (case, seed) and the result when it is ready.
    pub baseline: Arc<Mutex<Option<(String, u64, Result<Outcome, String>)>>>,
    /// The operator bar (F2).
    pub operator: bool,
    /// «Vista iniziale» was pressed: the camera goes back to the home view.
    pub reset_view: bool,
    /// The opening screen is showing (a new visitor): at launch and after the
    /// operator's «Nuova partita», not after «Riprova» / «Altro incendio».
    pub intro: bool,
    /// Real seconds the player has at a crisis; the operator can lengthen it
    /// for slow readers.
    pub crisis_s: f32,
    /// The case chosen in the operator bar for the next game.
    pub pick: String,
    /// Real time «Nuova partita» was first pressed, waiting for the second
    /// press that confirms throwing away a game in progress.
    pub reset_armed: Option<f64>,
    /// Zoom asked from the on-screen buttons, as a distance factor; the
    /// camera takes it and puts it back to 1.
    pub zoom: f32,
    /// In the browser (no threads) the no-orders game runs here, a few steps
    /// a frame.
    #[cfg(target_arch = "wasm32")]
    run: Option<Box<Game>>,
    data: PathBuf,
}

impl Kiosk {
    fn new(territory: Territory, data: PathBuf) -> Kiosk {
        let case = std::env::var("KIOSK_CASE").ok().filter(|c| territory.case(c).is_some()).unwrap_or_else(|| territory.playlist()[0].clone());
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
            crisis: None,
            risk: vec![],
            evacuation: vec![],
            baseline: Arc::new(Mutex::new(None)),
            operator: false,
            reset_view: false,
            zoom: 1.0,
            crisis_s: CRISIS_S,
            intro: true,
            pick: String::new(),
            reset_armed: None,
            #[cfg(target_arch = "wasm32")]
            run: None,
            data,
        }
    }

    /// The next case of the kiosk's playlist («Altro incendio»).
    pub fn next_case(&self) -> String {
        let list = self.territory.playlist();
        let at = list.iter().position(|c| *c == self.case).map_or(0, |i| i + 1);
        list[at % list.len()].clone()
    }

    /// The no-orders run for this case and seed, if it has finished.
    pub fn baseline(&self) -> Option<Result<Outcome, String>> {
        let b = self.baseline.lock().ok()?;
        b.as_ref().filter(|(c, s, _)| *c == self.case && *s == self.seed).map(|(_, _, r)| r.clone())
    }

    /// Start the no-orders run in the background, unless it is already done.
    fn start_baseline(&mut self) {
        if self.baseline().is_some() {
            return;
        }
        let (slot, data, case, seed) = (self.baseline.clone(), self.data.clone(), self.case.clone(), self.seed);
        if let Ok(mut b) = slot.lock() {
            *b = None;
        }
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let r = Game::without_orders(&data, &case, seed).map_err(|e| format!("{e:#}"));
            if let Ok(mut b) = slot.lock() {
                *b = Some((case, seed, r));
            }
        });
        #[cfg(target_arch = "wasm32")]
        match Game::new(&data, &case, seed) {
            Ok(g) => self.run = Some(Box::new(g)),
            Err(e) => {
                if let Ok(mut b) = slot.lock() {
                    *b = Some((case, seed, Err(format!("{e:#}"))));
                }
            }
        }
    }

    /// Browser only: advance the no-orders game a little (`Game::without_orders`
    /// in steps).
    #[cfg(target_arch = "wasm32")]
    fn advance_baseline(&mut self) {
        const STEPS_PER_FRAME: usize = 4;
        let Some(g) = self.run.as_mut() else { return };
        let end = g.case.duration_s();
        for _ in 0..STEPS_PER_FRAME {
            if g.time_s() >= end {
                break;
            }
            if let Err(e) = g.step() {
                let r = Err(format!("{e:#}"));
                if let Ok(mut b) = self.baseline.lock() {
                    *b = Some((self.case.clone(), self.seed, r));
                }
                self.run = None;
                return;
            }
        }
        if g.time_s() >= end {
            let r = Ok(g.outcome());
            if let Ok(mut b) = self.baseline.lock() {
                *b = Some((g.case.name.clone(), g.seed, r));
            }
            self.run = None;
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
        let k = Kiosk::new(t, data.0.clone());
        let sim = Sim::new(&data.0, &k.case, k.seed)?;
        Ok((k, sim))
    });
    match built {
        Ok((mut k, sim)) => {
            k.proposed = Plan::new(sim.districts.len());
            k.pick = k.case.clone();
            k.start_baseline();
            commands.insert_resource(k);
            commands.insert_resource(sim);
            commands.insert_resource(characters::Speech::default());
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
            kiosk.crisis = None;
            kiosk.start_baseline();
            kiosk.enter(Phase::Pianifica);
            restarted.send(SimRestarted);
        }
        Err(e) => kiosk.error = Some(format!("{e:#}")),
    }
}

/// End a crisis: the proposed plan becomes active if the player changed it
/// (revalidated now); otherwise the active plan simply goes on.
pub fn close_crisis(kiosk: &mut Kiosk, sim: &mut Sim) {
    if kiosk.proposed != sim.active {
        commit(kiosk, sim);
    }
    kiosk.crisis = None;
    kiosk.enter(Phase::Esegui);
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
    #[cfg(target_arch = "wasm32")]
    kiosk.advance_baseline();
    kiosk.phase_t += dt;
    sim.speed = match kiosk.phase {
        // faster while nothing is happening and nothing is being composed
        Phase::Esegui if kiosk.proposed == sim.active && sim.is_quiet(QUIET_S) => kiosk.speed * QUIET_BOOST,
        Phase::Esegui => kiosk.speed,
        Phase::Crisi => 1.0,
        _ => 0.0,
    };
    if matches!(kiosk.phase, Phase::Esegui | Phase::Crisi) {
        if let Err(e) = sim.tick(dt) {
            kiosk.error = Some(format!("{e:#}"));
            kiosk.enter(Phase::Fine);
        }
        if let Some(c) = sim.take_crisis() {
            if kiosk.phase == Phase::Esegui {
                kiosk.crisis = Some(c);
                kiosk.proposed = sim.active.clone();
                kiosk.dirty = true;
                kiosk.enter(Phase::Crisi);
            }
        }
        if kiosk.phase == Phase::Crisi && kiosk.phase_t >= kiosk.crisis_s {
            let now = sim.time_s();
            let what = if kiosk.proposed != sim.active { "tempo scaduto: si applica il piano proposto" } else { "tempo scaduto: resta il piano attuale" };
            sim.log.push(rocca::game::LogEntry { at_s: now, text: what.into() });
            close_crisis(&mut kiosk, &mut sim);
        }
        if sim.time_s() >= sim.case.duration_s() {
            kiosk.enter(Phase::Fine);
        }
    }
    // The preview follows the plan being composed and the world as it moves,
    // at most once a simulated review interval while running.
    let stale = kiosk.dirty || (kiosk.phase != Phase::Pianifica && sim.generation >= kiosk.preview_at.saturating_add(20));
    if stale && kiosk.phase != Phase::Fine {
        kiosk.preview = sim.preview(&kiosk.proposed).ok();
        kiosk.risk = sim.districts.iter().map(|d| rocca::district::exposure(d, &sim.agents, &sim.fire, &sim.scn)).collect();
        kiosk.evacuation = sim.districts.iter().map(|d| {
            let mut minutes = Vec::new();
            let mut blocked = 0;
            for &i in &d.households {
                match sim.agents.evacuation_journey_s(i) {
                    Some(s) => minutes.push((s / 60.0).ceil().max(1.0) as u32),
                    None => blocked += 1,
                }
            }
            (minutes.iter().min().zip(minutes.iter().max()).map(|(&a, &b)| (a, b)), blocked)
        }).collect();
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
    mut restarted: EventWriter<SimRestarted>,
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
        0 if kiosk.intro && stage.1 > 8.0 => {
            snap("0_intro");
            kiosk.intro = false;
            stage.1 = 0.0;
        }
        0 if !kiosk.intro && stage.1 > 3.0 => {
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
            *stage = (5, 0.0);
        }
        // at the first crisis, put the place it names first, and show the proposal
        5 if kiosk.phase == Phase::Crisi && kiosk.phase_t > 1.0 => {
            if let Some(d) = kiosk.crisis.as_ref().and_then(|c| match c.kind {
                rocca::crisis::Kind::Scoperto { district } | rocca::crisis::Kind::Previsione { district } | rocca::crisis::Kind::Vento { district } => Some(district),
                rocca::crisis::Kind::MezzoPerso { .. } => None,
            }) {
                let mut prio = vec![d];
                prio.extend(kiosk.proposed.priorities.iter().copied().filter(|&x| x != d));
                kiosk.proposed.priorities = prio;
                kiosk.dirty = true;
            }
            *stage = (7, 0.0);
        }
        7 if stage.1 > 1.5 => {
            snap("3b_crisi");
            close_crisis(&mut kiosk, &mut sim);
            *stage = (3, 0.0);
        }
        5 if kiosk.phase == Phase::Fine => *stage = (3, 0.0),
        3 if kiosk.phase == Phase::Crisi && kiosk.phase_t > 1.0 => {
            snap("3c_crisi");
            close_crisis(&mut kiosk, &mut sim);
            *stage = (3, 0.0);
        }
        3 if kiosk.phase == Phase::Fine => *stage = (6, 0.0),
        6 if stage.1 > 1.0 && kiosk.baseline().is_some() => {
            snap("4_fine");
            *stage = (8, 0.0);
        }
        // «Altro incendio»: the manual restart must leave a clean scene
        8 if stage.1 > 1.0 => {
            kiosk.case = kiosk.next_case();
            new_game(&mut kiosk, &mut sim, &mut restarted);
            *stage = (9, 0.0);
        }
        9 if stage.1 > 4.0 => {
            snap("5_altro_incendio");
            *stage = (4, 0.0);
        }
        4 if stage.1 > 1.0 => {
            exit.send(AppExit::Success);
        }
        _ => {}
    }
}
