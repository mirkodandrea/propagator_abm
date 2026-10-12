//! The kiosk: one territory, one game, started and restarted by the operator.
//!
//! Everything the game decides is in [`rocca::Game`]; this module only holds
//! what the player is composing (the proposed plan), asks the game for its
//! preview, commits it on request and runs the clock. No rules live here.
//!
//! Flow: **Pianifica** (×0: first the map with orders locked until «Inizia
//! la partita», then a short tour of the screen (`tutorial`), then the
//! proposed plan is previewed) →
//! **Esegui** (×N) ⇄ **Crisi** (×1 with a countdown, raised by the game) →
//! **Fine** (facts per district, against the same fire with no orders). No
//! inactivity reset: a new game starts from the debrief's «Riprova» / «Altro
//! incendio» or from the operator bar (F2), never by itself.
//!
//! Before all of it, the **title screen**: a scripted game plays behind the
//! title (same engine, same camera, a fixed plan, crises closed by a timer)
//! until someone presses «Gioca». A game can be abandoned at any time
//! (`Kiosk::leave`) and a finished one goes back to the title: both reach the
//! title the same way, by [`to_title`].

pub mod board;
pub mod characters;
pub mod icons;
pub mod snap;
pub mod tutorial;
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

pub const TITLE: &str = "Isola di Roccavento";
/// Simulated seconds per real second while the game runs (spec §2: ×40; user,
/// 2026-10-12: faster, with a selector in the side bar).
pub const RUN_SPEED: f32 = 80.0;
/// Real seconds the player has at a crisis (spec: ~25 s at ×1).
pub const CRISIS_S: f32 = 25.0;
/// Pauses a game allows once the time runs (user, 2026-10-09): few enough
/// that the game is played in real time, not a pause per order. Crises do
/// not count.
pub const MAX_PAUSES: u32 = 3;
/// Simulated seconds after «Avvia» / «Riprendi» before the next pause
/// (user, 2026-10-09: no pause straight after another). Orders change only
/// in a pause, so this also spaces the changes of plan.
pub const PAUSE_GAP_S: i64 = 15 * 60;
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

/// A camera move asked by the interface: look at `at` (the home view's
/// middle if `None`) from `zoom` times the home view's distance, with the
/// home view's yaw and pitch.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shot {
    pub at: Option<scenario::Pos>,
    pub zoom: f32,
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
    /// Per district, the evacuation journey in minutes (median, longest)
    /// and the families with no way out.
    pub evacuation: Vec<(Option<(u32, u32)>, usize)>,
    /// Per district, the typical minutes families still at home would take
    /// to get ready once ordered out (the preparation the pre-alert has not
    /// already done), if anyone is at home.
    pub readying: Vec<Option<u32>>,
    /// The same case and seed with no orders, run beside the game for the
    /// debrief: (case, seed) and the result when it is ready.
    pub baseline: Arc<Mutex<Option<(String, u64, Result<Outcome, String>)>>>,
    /// The operator bar (F2).
    pub operator: bool,
    /// «Vista iniziale» was pressed: the camera goes back to the home view.
    pub reset_view: bool,
    /// While time runs the camera follows the fire; a hand on the map (or
    /// the button in the view controls) turns it off.
    pub follow: bool,
    /// The title screen is showing, with a scripted game behind it ([`demo`]).
    pub title: bool,
    /// Real seconds the title screen's game has been on show: the camera
    /// director's clock ([`demo_shot`]).
    pub demo_t: f32,
    /// The simulation speed the director asks for, eased.
    demo_speed: f32,
    /// The shot being held (its index) and what it looks at.
    pub demo_pick: (usize, Subject),
    /// Leave the game for the title screen at the next frame (the sidebar's
    /// «Abbandona», the tour's «Esci», the debrief's «Schermata iniziale»).
    pub leave: bool,
    /// Real time «Abbandona» was pressed, waiting for the confirmation.
    pub abort_armed: Option<f64>,
    /// The opening screen is showing: at launch, after the operator's «Nuova
    /// partita» (a new visitor) and after «Riprova» (the same one, to pick a
    /// level; the name is kept).
    pub intro: bool,
    /// The visitor picked a difficulty on the opening screen. Before that the map, the fire
    /// and the chips show, but no order can be given (user, 2026-10-09).
    pub started: bool,
    /// How to play, shown once the game has started, until «Ho capito».
    /// Pauses left in this game ([`MAX_PAUSES`]).
    pub pauses_left: u32,
    /// The level being played (`Territory::levels`), and the one just
    /// picked on the opening screen, for `ui::draw` to start.
    pub level: Option<usize>,
    pub picked_level: Option<usize>,
    /// The district whose badge is opened on the map (a click), if any.
    pub selected: Option<usize>,
    /// Simulated time the clock last started running («Avvia», «Riprendi»).
    pub resumed_at_s: i64,
    /// The leaderboard, one for all fires (user, 2026-10-09).
    pub board: rocca::score::Board,
    /// The player's first name, typed on the opening screen before the
    /// first game (user, 2026-10-12): the characters speak to them by it and
    /// the board shows it. Kept for «Riprova», cleared by the operator's
    /// «Nuova partita» (a new visitor).
    pub player: String,
    /// The place this game took on the board once saved.
    pub placed: Option<Option<usize>>,
    /// Real time «Azzera classifica» was first pressed (operator bar).
    pub board_reset_armed: Option<f64>,
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
    /// Logical pixels the side bar takes on the right; the camera draws the
    /// map in the rest (`view::camera`).
    pub side_w: f32,
    /// Where the camera is flying to, if anywhere; it clears it on arrival.
    pub shot: Option<Shot>,
    /// The step of the tour being shown (`tutorial`), if any.
    pub tour: Option<usize>,
    /// The tour was shown to this visitor: not again at «Riprova», again
    /// after the operator's «Nuova partita».
    pub toured: bool,
    /// The operator can turn the tour off (F2, or `KIOSK_TUTORIAL=0`).
    pub tour_on: bool,
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
            readying: vec![],
            baseline: Arc::new(Mutex::new(None)),
            operator: false,
            reset_view: false,
            follow: true,
            zoom: 1.0,
            side_w: 0.0,
            shot: None,
            tour: None,
            toured: false,
            tour_on: std::env::var("KIOSK_TUTORIAL").map_or(true, |v| v != "0"),
            crisis_s: CRISIS_S,
            title: true,
            demo_t: 0.0,
            demo_speed: 1.0,
            demo_pick: (usize::MAX, Subject::Crew(usize::MAX)),
            leave: false,
            abort_armed: None,
            intro: false,
            started: false,
            pauses_left: MAX_PAUSES,
            level: None,
            picked_level: None,
            selected: None,
            resumed_at_s: 0,
            board: board::load(),
            player: String::new(),
            placed: None,
            board_reset_armed: None,
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

    /// Whether the chips take orders: not before the opening screen's start.
    pub fn can_order(&self) -> bool {
        self.started
    }

    /// Minutes until a pause is allowed again; 0 when it is.
    pub fn pause_wait_min(&self, now_s: i64) -> i64 {
        ((self.resumed_at_s + PAUSE_GAP_S - now_s).max(0) + 59) / 60
    }

    pub fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_t = 0.0;
        // an opened chip would cover the pause's or the crisis's own panel
        self.selected = None;
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
    restart(kiosk, sim, restarted, true);
}

/// Back to the title screen: a new visitor (the name, the tour and the level
/// are forgotten) and the next case of the playlist playing by itself.
pub fn to_title(kiosk: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    kiosk.title = true;
    kiosk.demo_t = 0.0;
    kiosk.demo_pick = (usize::MAX, Subject::Crew(usize::MAX));
    kiosk.intro = false;
    kiosk.leave = false;
    kiosk.abort_armed = None;
    kiosk.tour = None;
    kiosk.toured = false;
    kiosk.level = None;
    kiosk.picked_level = None;
    kiosk.player.clear();
    kiosk.case = kiosk.next_case();
    restart(kiosk, sim, restarted, false);
}

/// The place a crisis is about, if it is about one.
fn crisis_place(c: &rocca::Crisis) -> Option<usize> {
    match c.kind {
        rocca::crisis::Kind::Scoperto { district } | rocca::crisis::Kind::Previsione { district } | rocca::crisis::Kind::Vento { district } => Some(district),
        rocca::crisis::Kind::MezzoPerso { .. } => None,
    }
}

/// Real seconds a shot of the title screen's camera lasts: long enough to
/// see something happen, not a montage.
pub const SHOT_S: f32 = 26.0;

/// What the title screen's camera is looking at; chosen once per shot, by
/// what the game is showing (`view::pick_shot`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Subject {
    /// The whole burning front and the towns near it.
    Wide,
    /// The front, close.
    Front,
    /// The town the fire is nearest to.
    District(usize),
    /// A ground crew working near the fire.
    Crew(usize),
}

impl Subject {
    /// How fast the game runs while this is on show, against the kiosk's
    /// speed: fast over the whole front, slow enough on a crew to see it move.
    fn pace(self) -> f32 {
        match self {
            Subject::Wide => 1.5,
            Subject::Front => 0.8,
            Subject::District(_) => 0.8,
            Subject::Crew(_) => 0.4,
        }
    }
}

/// The index of the shot at real time `t` of the title screen.
pub fn demo_shot(t: f32) -> usize {
    (t / SHOT_S) as usize
}

/// The plan every scripted game starts with: the place the fire was picked
/// for first, the others after it, a pre-alert there.
fn scripted_plan(sim: &Sim) -> Plan {
    let near = sim.district_index(&sim.case.near).unwrap_or(0);
    let mut prio = vec![near];
    prio.extend((0..sim.districts.len()).filter(|&d| d != near));
    Plan::new(sim.districts.len()).with_priorities(&prio).with_civil(near, rocca::Civil::Preallerta)
}

/// The game behind the title screen, played by a script: the plan above, the
/// place a crisis names put first after a few seconds, and when the case has
/// run its course the next one starts. Nothing here is scored or saved.
fn demo(kiosk: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    match kiosk.phase {
        Phase::Pianifica if kiosk.phase_t > 2.0 => {
            kiosk.proposed = scripted_plan(sim);
            commit(kiosk, sim);
            kiosk.resumed_at_s = sim.time_s();
            kiosk.enter(Phase::Esegui);
        }
        Phase::Crisi if kiosk.phase_t > 3.0 => {
            if let Some(d) = kiosk.crisis.as_ref().and_then(crisis_place) {
                let mut prio = vec![d];
                prio.extend(kiosk.proposed.priorities.iter().copied().filter(|&x| x != d));
                kiosk.proposed.priorities = prio;
            }
            close_crisis(kiosk, sim);
        }
        Phase::Fine if kiosk.phase_t > 6.0 => {
            kiosk.case = kiosk.next_case();
            restart(kiosk, sim, restarted, false);
        }
        _ => {}
    }
}

/// Start the case again from its beginning; `baseline` also starts the
/// no-orders run the debrief compares with (not needed by the scripted game).
fn restart(kiosk: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>, baseline: bool) {
    match sim.restart(&kiosk.case, kiosk.seed) {
        Ok(()) => {
            kiosk.proposed = Plan::new(sim.districts.len());
            kiosk.dirty = true;
            kiosk.error = None;
            kiosk.crisis = None;
            kiosk.started = false;
            kiosk.pauses_left = MAX_PAUSES;
            kiosk.selected = None;
            kiosk.resumed_at_s = 0;
            kiosk.placed = None;
            if baseline {
                kiosk.start_baseline();
            }
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
pub fn step(time: Res<Time>, mut kiosk: ResMut<Kiosk>, mut sim: ResMut<Sim>, mut restarted: EventWriter<SimRestarted>,
    #[cfg(not(target_arch = "wasm32"))] mario: Res<crate::mario::Playground>,
    #[cfg(not(target_arch = "wasm32"))] mut was_mario: Local<bool>,
) {
    let dt = time.delta_seconds();
    #[cfg(not(target_arch = "wasm32"))]
    let mario_clock = {
        let active = mario.active && kiosk.phase == Phase::Pianifica;
        if *was_mario && !active { kiosk.dirty = true; }
        *was_mario = active;
        active
    };
    #[cfg(target_arch = "wasm32")]
    let mario_clock = false;
    #[cfg(target_arch = "wasm32")]
    kiosk.advance_baseline();
    kiosk.phase_t += dt;
    if kiosk.title {
        kiosk.demo_t += dt;
        demo(&mut kiosk, &mut sim, &mut restarted);
        // the director's pace: eased, so speed changes are felt, not jumped
        let want = kiosk.demo_pick.1.pace();
        kiosk.demo_speed += (want - kiosk.demo_speed) * (1.0 - (-0.4 * dt).exp());
    }
    sim.speed = match kiosk.phase {
        Phase::Pianifica if mario_clock => 1.0,
        // faster while nothing is happening and nothing is being composed
        Phase::Esegui if kiosk.title => kiosk.speed * kiosk.demo_speed,
        Phase::Esegui if kiosk.proposed == sim.active && sim.is_quiet(QUIET_S) => kiosk.speed * QUIET_BOOST,
        Phase::Esegui => kiosk.speed,
        Phase::Crisi => 1.0,
        _ => 0.0,
    };
    if matches!(kiosk.phase, Phase::Esegui | Phase::Crisi) || mario_clock {
        if let Err(e) = sim.tick(dt) {
            kiosk.error = Some(format!("{e:#}"));
            kiosk.enter(Phase::Fine);
        }
        if let Some(c) = sim.take_crisis() {
            if kiosk.phase == Phase::Esegui || mario_clock {
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
    let stale = kiosk.dirty || ((kiosk.phase != Phase::Pianifica || mario_clock) && sim.generation >= kiosk.preview_at.saturating_add(20));
    if stale && kiosk.phase != Phase::Fine && !kiosk.title {
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
            // the typical journey and the longest, not the shortest: the
            // range "~4–102 min" did not say when it would be over (playtest 3)
            minutes.sort_unstable();
            (minutes.last().map(|&b| (minutes[minutes.len() / 2], b)), blocked)
        }).collect();
        // the households' own preparation, less what a pre-alert has done
        // (`Abm::prealert_of`): a fact of the population, not a rule
        kiosk.readying = sim.districts.iter().map(|d| {
            let mut minutes: Vec<u32> = d.households.iter()
                .map(|&i| &sim.agents.households[i])
                .filter(|h| matches!(h.status, scenario::population::Status::Normal | scenario::population::Status::Warned | scenario::population::Status::Defending))
                .map(|h| ((h.prep_time_min * 60.0 - h.readied_s).max(60.0) / 60.0).round() as u32)
                .collect();
            minutes.sort_unstable();
            minutes.get(minutes.len() / 2).copied()
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
    // `KIOSK_SHOT_END=1`: only the debrief. A plan is committed, the game is
    // run headless to its end, and the end screen is saved (a few seconds).
    let end_only = std::env::var_os("KIOSK_SHOT_END").is_some();
    match stage.0 {
        0 if end_only && stage.1 > 3.0 => {
            kiosk.player = "Giulia".into();
            kiosk.title = false;
            new_game(&mut kiosk, &mut sim, &mut restarted);
            kiosk.intro = false;
            kiosk.started = true;
            let near = sim.district_index(&sim.case.near).unwrap_or(0);
            let mut prio = vec![near];
            prio.extend((0..sim.districts.len()).filter(|&d| d != near));
            kiosk.proposed = Plan::new(sim.districts.len()).with_priorities(&prio).with_civil(near, rocca::Civil::Preallerta);
            commit(&mut kiosk, &mut sim);
            let end = sim.case.duration_s();
            if let Err(e) = sim.run_until(end) {
                kiosk.error = Some(format!("{e:#}"));
            }
            sim.generation += 1;
            kiosk.enter(Phase::Fine);
            *stage = (6, 0.0);
        }
        // the title screen with its game behind, then «Gioca»
        0 if kiosk.title && stage.1 > 12.0 => {
            snap("0_titolo");
            kiosk.title = false;
            kiosk.case = kiosk.pick.clone();
            new_game(&mut kiosk, &mut sim, &mut restarted);
            kiosk.intro = true;
            stage.1 = 0.0;
        }
        // a name typed, as a visitor would, so the intro and the lines show it
        0 if kiosk.intro && kiosk.player.is_empty() && stage.1 > 4.0 => {
            snap("0_intro_vuota");
            kiosk.player = "Giulia".into();
        }
        0 if kiosk.intro && stage.1 > 8.0 => {
            snap("0_intro");
            kiosk.intro = false;
            stage.1 = 0.0;
        }
        0 if !kiosk.intro && !kiosk.started && stage.1 > 3.0 => {
            kiosk.started = true;
            if kiosk.tour_on {
                tutorial::go(&mut kiosk, &sim, 0);
            }
            stage.1 = 0.0;
        }
        // each step of the tour once the camera has arrived
        0 if kiosk.shot.is_none() && stage.1 > 1.5 && kiosk.tour.is_some() => {
            let i = kiosk.tour.unwrap_or(0);
            snap(&format!("0_tour_{i}"));
            tutorial::go(&mut kiosk, &sim, i + 1);
            stage.1 = 0.0;
        }
        0 if kiosk.tour.is_none() && kiosk.shot.is_none() && kiosk.can_order() && stage.1 > 1.5 => {
            snap("1_pianifica");
            // the locality the fire was picked for first, then the others
            let near = sim.district_index(&sim.case.near).unwrap_or(0);
            let mut prio = vec![near];
            prio.extend((0..sim.districts.len()).filter(|&d| d != near));
            kiosk.proposed = Plan::new(sim.districts.len()).with_priorities(&prio).with_civil(near, rocca::Civil::Preallerta);
            kiosk.dirty = true;
            // the badge opened, as a click would (the next shot shows it)
            kiosk.selected = Some(near);
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
            if let Some(d) = kiosk.crisis.as_ref().and_then(crisis_place) {
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
        // later in the game: what the volunteer says from the game
        3 if kiosk.phase == Phase::Esegui && sim.time_s() >= 100 * 60 => {
            snap("3d_esegui_dopo");
            *stage = (11, 0.0);
        }
        3 | 11 if kiosk.phase == Phase::Crisi && kiosk.phase_t > 1.0 => {
            snap("3c_crisi");
            close_crisis(&mut kiosk, &mut sim);
            *stage = (stage.0, 0.0);
        }
        3 | 11 if kiosk.phase == Phase::Fine => *stage = (6, 0.0),
        6 if stage.1 > 1.0 && kiosk.baseline().is_some() => {
            snap("4_fine");
            *stage = (if end_only { 4 } else { 8 }, 0.0);
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod mario_clock_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn mario_uses_the_real_clock_at_one_x_and_returns_to_pause() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let territory = Territory::load(&data).unwrap();
        let mut kiosk = Kiosk::new(territory, data.clone());
        kiosk.title = false;
        kiosk.intro = false;
        kiosk.started = true;
        let sim = Sim::new(&data, &kiosk.case, 1).unwrap();
        kiosk.proposed = Plan::new(sim.districts.len());
        let active = sim.active.clone();
        let proposed = kiosk.proposed.clone();
        let pauses = kiosk.pauses_left;
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(kiosk)
            .insert_resource(sim)
            .init_resource::<crate::mario::Playground>()
            .add_event::<SimRestarted>()
            .add_systems(Update, step);
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(6));
        app.update();
        assert_eq!(app.world().resource::<Sim>().time_s(), 0);
        app.world_mut().resource_mut::<crate::mario::Playground>().active = true;
        app.update();
        assert_eq!(app.world().resource::<Sim>().speed, 1.0);
        assert_eq!(app.world().resource::<Sim>().time_s(), 6);
        assert_eq!(app.world().resource::<Kiosk>().phase, Phase::Pianifica);
        app.world_mut().resource_mut::<crate::mario::Playground>().active = false;
        app.update();
        let sim = app.world().resource::<Sim>();
        let kiosk = app.world().resource::<Kiosk>();
        assert_eq!(sim.speed, 0.0);
        assert_eq!(sim.time_s(), 6);
        assert_eq!(sim.active, active);
        assert_eq!(kiosk.proposed, proposed);
        assert_eq!(kiosk.pauses_left, pauses);
        assert_eq!(kiosk.preview_at, sim.generation, "refresh the plan when Mario returns");
    }
}
