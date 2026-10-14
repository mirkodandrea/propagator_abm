//! `KIOSK_SNAP`: screenshots of arbitrary game states, without playing.
//!
//! `KIOSK_SNAP=<dir>` names the output folder; `KIOSK_SNAP_SPEC` lists the
//! states, separated by `;`. Each state is `key=value` pairs separated by `,`:
//!
//! | key      | meaning                                                              |
//! |----------|----------------------------------------------------------------------|
//! | `name`   | file name without `.png` (default `snap<N>`)                         |
//! | `phase`  | `intro`, `tutorial`, `giorno2`, `gioco` (default), `fine`             |
//! | `step`   | the tutorial's step on show (with `phase=tutorial`)                   |
//! | `t`      | simulated minutes since the start, run headless (default 0)          |
//! | `pause`  | `1`: in pause (the expert's forecast on the map)                     |
//! | `move`   | `<team>:<x>:<y>`, orders given in pause at `t`, `+` between            |
//! | `evac`   | a district evacuated in pause at `t`                                 |
//! | `team`   | the team selected; with `hex=<x>:<y>` the move previewed             |
//! | `select` | the district selected (its evacuation previewed)                     |
//! | `at`     | camera focus: `home`, `fire`, `district:<name>`, `team:<n>`, `<x>:<y>` |
//! | `dist`, `yaw`, `pitch` | the view, metres and degrees                           |
//!
//! The game is that of the phase (the tutorial's fire or the played one).
//! Time only goes forward: a state with an earlier `t` or another phase's
//! fire restarts it. The world is frozen at the shot. Example:
//!
//! ```text
//! KIOSK_SNAP=/tmp/snaps KIOSK_SNAP_SPEC='name=a,phase=intro;name=b,t=20,pause=1,team=0,hex=5300:4600' cargo run -p game --release
//! ```

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::ScreenshotManager;
use bevy::window::PrimaryWindow;
use scenario::Pos;

use super::{Kiosk, Phase, Selection};
use crate::camera::OrbitCamera;
use crate::sim::{Sim, SimRestarted};

/// The view the state on show asks for, in radians and Bevy space.
#[derive(Resource, Default, Clone, Copy)]
pub struct SnapView {
    pub focus: Option<Vec3>,
    pub dist: Option<f32>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
}

/// Real seconds a state is shown before its picture is taken.
const SETTLE_S: f32 = 2.0;

#[derive(Default, Clone)]
struct State {
    name: String,
    phase: String,
    step: usize,
    t_min: f32,
    pause: bool,
    moves: Vec<(usize, Pos)>,
    evac: Option<String>,
    team: Option<usize>,
    hex: Option<Pos>,
    select: Option<String>,
    at: String,
    dist: Option<f32>,
    yaw: Option<f32>,
    pitch: Option<f32>,
}

fn xy(v: &str) -> Option<Pos> {
    let (x, y) = v.split_once(':')?;
    Some(Pos { x: x.parse().ok()?, y: y.parse().ok()? })
}

fn parse(spec: &str) -> Vec<State> {
    spec.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .enumerate()
        .map(|(n, s)| {
            let mut st = State { name: format!("snap{n}"), phase: "gioco".into(), at: "home".into(), ..Default::default() };
            for kv in s.split(',') {
                let Some((k, v)) = kv.split_once('=') else { continue };
                let v = v.trim();
                match k.trim() {
                    "name" => st.name = v.into(),
                    "phase" => st.phase = v.into(),
                    "step" => st.step = v.parse().unwrap_or(0),
                    "t" => st.t_min = v.parse().unwrap_or(0.0),
                    "pause" => st.pause = v == "1",
                    "move" => {
                        st.moves = v.split('+').filter_map(|m| m.split_once(':').and_then(|(t, p)| Some((t.parse().ok()?, xy(p)?)))).collect();
                    }
                    "evac" => st.evac = Some(v.into()),
                    "team" => st.team = v.parse().ok(),
                    "hex" => st.hex = xy(v),
                    "select" => st.select = Some(v.into()),
                    "at" => st.at = v.into(),
                    "dist" => st.dist = v.parse().ok(),
                    "yaw" => st.yaw = v.parse().ok(),
                    "pitch" => st.pitch = v.parse().ok(),
                    other => eprintln!("snap: unknown key `{other}`"),
                }
            }
            st
        })
        .collect()
}

pub fn active() -> bool {
    std::env::var_os("KIOSK_SNAP").is_some()
}

#[derive(Default)]
pub struct Progress {
    states: Option<Vec<State>>,
    at: usize,
    applied: bool,
    waited: f32,
    goal: Option<(Vec3, Option<f32>, Option<f32>, Option<f32>)>,
    done_s: f32,
}

/// Bring the game to the state.
fn apply(st: &State, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) -> Option<(Vec3, Option<f32>, Option<f32>, Option<f32>)> {
    let phase = match st.phase.as_str() {
        "intro" => Phase::Intro,
        "tutorial" => Phase::Tutorial,
        "giorno2" => Phase::Giorno2,
        "fine" => Phase::Fine,
        _ => Phase::Gioco,
    };
    let case = if matches!(phase, Phase::Intro | Phase::Tutorial) { k.tutorial_case() } else { k.territory.game.clone() };
    let t_s = (st.t_min * 60.0) as i64;
    if k.player.is_empty() {
        k.player = "Giulia".into();
    }
    if case != sim.case.name || t_s < sim.time_s() {
        if let Err(e) = sim.restart(&case, k.seed) {
            k.error = Some(format!("{e:#}"));
        }
        restarted.send(SimRestarted);
        if phase == Phase::Fine {
            k.start_baseline(&case);
        }
    }
    if let Err(e) = sim.run_until(t_s) {
        k.error = Some(format!("{e:#}"));
    }
    let wants_pause = st.pause || !st.moves.is_empty() || st.evac.is_some();
    if wants_pause && !sim.paused() {
        if let Err(e) = sim.pause() {
            eprintln!("snap: {}: {e}", st.name);
        }
    }
    for &(t, p) in &st.moves {
        if let Some(h) = sim.hexes.nearest(p).map(|h| h.at) {
            if let Err(e) = sim.move_team(t, h) {
                eprintln!("snap: {}: {e}", st.name);
            }
        }
    }
    if let Some(d) = st.evac.as_deref().and_then(|n| sim.district_index(n)) {
        if let Err(e) = sim.evacuate(d) {
            eprintln!("snap: {}: {e}", st.name);
        }
    }
    if !st.pause && sim.paused() {
        sim.resume();
    }
    sim.generation += 1;
    k.enter(phase);
    if phase == Phase::Tutorial {
        k.script = rocca::tutorial::Script::of(sim);
        k.step = st.step;
    }
    k.frozen = true;
    k.follow = false;
    if let Some(t) = st.team {
        k.selection = Selection::Team(t);
        if let Some(p) = st.hex {
            let at = rocca::Axial::of(p);
            k.move_hex = Some(at);
            k.move_plan = Some(sim.plan_move(t, at));
            if let Some(Err(e)) = &k.move_plan {
                eprintln!("snap: {}: {e}", st.name);
            }
        }
    }
    if let Some(d) = st.select.as_deref().and_then(|n| sim.district_index(n)) {
        k.selection = Selection::District(d);
        k.evac_plan = Some(sim.plan_evacuation(d));
    }

    let on = |p: Pos| crate::frame::to_bevy(p, sim.scn.terrain.height_at(p));
    let (focus, span) = match st.at.as_str() {
        "home" => return Some((Vec3::NAN, st.dist, st.yaw, st.pitch)),
        "fire" => match super::view::fire_extent(sim) {
            Some((mid, span)) => (Vec3::new(mid.x, 0.0, mid.y), Some(span * 2.8)),
            None => return Some((Vec3::NAN, st.dist, st.yaw, st.pitch)),
        },
        other => {
            let p = if let Some(name) = other.strip_prefix("district:") {
                sim.district_index(name).map(|d| sim.districts[d].centre)
            } else if let Some(n) = other.strip_prefix("team:") {
                n.parse::<usize>().ok().and_then(|n| sim.teams.get(n)).map(|t| sim.crews.units[t.unit].pos)
            } else {
                xy(other)
            };
            match p {
                Some(p) => (on(p), None),
                None => {
                    eprintln!("snap: cannot place the camera at `{other}`");
                    return Some((Vec3::NAN, st.dist, st.yaw, st.pitch));
                }
            }
        }
    };
    Some((focus, st.dist.or(span).or(Some(1200.0)), st.yaw, st.pitch.or(Some(-46.0))))
}

/// Walk the states: apply one, hold it while the camera settles, take the
/// picture, go on; quit after the last.
#[allow(clippy::too_many_arguments)]
pub fn drive(
    time: Res<Time>,
    mut kiosk: ResMut<Kiosk>,
    mut sim: ResMut<Sim>,
    mut orbit: Query<&mut OrbitCamera>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut mgr: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut restarted: EventWriter<SimRestarted>,
    mut view: ResMut<SnapView>,
    mut p: Local<Progress>,
) {
    let Ok(dir) = std::env::var("KIOSK_SNAP") else { return };
    let Ok(window) = windows.get_single() else { return };
    if p.states.is_none() {
        let spec = std::env::var("KIOSK_SNAP_SPEC").unwrap_or_else(|_| "name=snap0".into());
        p.states = Some(parse(&spec));
        let _ = std::fs::create_dir_all(&dir);
        eprintln!("snap: cases {:?}", kiosk.territory.cases.iter().map(|c| c.name.as_str()).collect::<Vec<_>>());
        eprintln!("snap: districts {:?}", sim.districts.iter().map(|d| d.name.as_str()).collect::<Vec<_>>());
    }
    let dt = time.delta_seconds();
    let Some(st) = p.states.as_ref().and_then(|s| s.get(p.at)).cloned() else {
        p.done_s += dt;
        if p.done_s > 1.0 {
            exit.send(AppExit::Success);
        }
        return;
    };
    if !p.applied {
        p.goal = apply(&st, &mut kiosk, &mut sim, &mut restarted);
        p.applied = true;
        p.waited = 0.0;
    }
    p.waited += dt;
    if let Some((focus, dist, yaw, pitch)) = p.goal {
        *view = SnapView { focus: (!focus.is_nan()).then_some(focus), dist, yaw: yaw.map(f32::to_radians), pitch: pitch.map(f32::to_radians) };
        if let Ok(mut o) = orbit.get_single_mut() {
            if let Some(f) = view.focus {
                o.focus = f;
            }
            if let Some(d) = view.dist {
                o.distance = d;
            }
            if let Some(y) = view.yaw {
                o.yaw = y;
            }
            if let Some(pt) = view.pitch {
                o.pitch = pt;
            }
        }
    } else {
        *view = SnapView::default();
    }
    let ready = st.phase != "fine" || kiosk.baseline().is_some();
    if p.waited > SETTLE_S && ready {
        let _ = mgr.save_screenshot_to_disk(window, format!("{dir}/{}.png", st.name));
        eprintln!("snap: {} (t={} min, phase {})", st.name, st.t_min, st.phase);
        p.at += 1;
        p.applied = false;
    }
}

/// Runs after the kiosk camera: puts the camera exactly where the state asks.
pub fn hold(view: Res<SnapView>, sim: Res<Sim>, mut cam: Query<(&mut OrbitCamera, &mut Transform, Option<&mut bevy::core_pipeline::dof::DepthOfFieldSettings>)>) {
    if !active() || (view.focus.is_none() && view.dist.is_none() && view.yaw.is_none() && view.pitch.is_none()) {
        return;
    }
    let Ok((mut orbit, mut tf, mut dof)) = cam.get_single_mut() else { return };
    if let Some(f) = view.focus {
        orbit.focus = f;
    }
    if let Some(d) = view.dist {
        orbit.distance = d.max(20.0);
    }
    if let Some(y) = view.yaw {
        orbit.yaw = y;
    }
    if let Some(p) = view.pitch {
        orbit.pitch = p.clamp(-1.553, -0.05);
    }
    orbit.focus.y = sim.scn.terrain.height_at(crate::frame::to_world(orbit.focus));
    let dir = Vec3::new(orbit.yaw.sin() * orbit.pitch.cos(), -orbit.pitch.sin(), orbit.yaw.cos() * orbit.pitch.cos());
    tf.translation = orbit.focus + dir * orbit.distance;
    let ground = sim.scn.terrain.height_at(crate::frame::to_world(tf.translation));
    tf.translation.y = tf.translation.y.max(ground + 4.0);
    tf.look_at(orbit.focus, Vec3::Y);
    if let Some(dof) = dof.as_mut() {
        dof.focal_distance = tf.translation.distance(orbit.focus);
    }
}
