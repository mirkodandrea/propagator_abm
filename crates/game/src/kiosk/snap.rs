//! `KIOSK_SNAP`: screenshots of arbitrary game states, without playing.
//!
//! `KIOSK_SNAP=<dir>` names the output folder; `KIOSK_SNAP_SPEC` lists the
//! states, separated by `;`. Each state is `key=value` pairs separated by `,`:
//!
//! | key      | meaning                                                              |
//! |----------|----------------------------------------------------------------------|
//! | `name`   | file name without `.png` (default `snap<N>`)                         |
//! | `case`   | case id (default: the kiosk's current one)                           |
//! | `t`      | simulated minutes since the start, run headless (default 0)          |
//! | `plan`   | `scripted` (default: near place first, pre-alert there), `none`, `keep` |
//! | `prio`   | district names in priority order, `+` between (overrides the plan)   |
//! | `pre`    | districts with a pre-alert, `+` between                              |
//! | `evac`   | districts evacuated at `t` if the mayor authorised them, `+` between |
//! | `phase`  | `esegui` (default, time frozen), `pianifica`, `fine`, `title`, `intro`|
//! | `at`     | camera focus: `home`, `fire`, `district:<name>`, `post:<n>`, `unit:<n>`, `<x>:<y>` (metres) |
//! | `dist`   | camera distance in metres (default: 1200, or the fire's span for `fire`) |
//! | `yaw`, `pitch` | camera angles in degrees (default: as it is, pitch -46)        |
//! | `select` | district whose badge is open                                         |
//! | `mario`, `wing` | `1` to show Mario and optionally his Wing Cap (native only)    |
//!
//! The view asked for is held *after* the kiosk camera has run
//! ([`hold`]), with none of its play limits: the kiosk camera keeps the focus
//! within 1.5 km of the home view, the pitch between -74 and -31 degrees, and
//! eases toward the fire, none of which a screenshot of the far coast wants.
//! `pitch` may be anything from -89 to -3 degrees, `yaw` any angle, `dist`
//! from 20 m.
//!
//! Time only goes forward within a case: a state with an earlier `t` (or
//! another `case`) restarts the game. The world is frozen at the shot, so
//! only the UI's own animations move. The district and case names are printed
//! at the start. Example, three views of one fire:
//!
//! ```text
//! KIOSK_SNAP=/tmp/snaps KIOSK_SNAP_SPEC='name=a,t=0;name=b,t=25,at=fire;name=c,t=60,at=district:Castelvento,dist=700,pitch=-35' cargo run -p game --release
//! ```

use bevy::prelude::*;
use bevy::render::view::screenshot::ScreenshotManager;
use bevy::window::PrimaryWindow;
use bevy::app::AppExit;
use rocca::{Civil, Plan};
use scenario::Pos;

use super::{commit, restart, Kiosk, Phase};
use crate::camera::OrbitCamera;
use crate::sim::{Sim, SimRestarted};

/// The view the state on show asks for, in radians and Bevy space; each part
/// `None` when the state leaves it as the kiosk camera has it. Written by
/// [`drive`], held by [`hold`].
#[derive(Resource, Default, Clone, Copy)]
pub struct SnapView {
    pub focus: Option<Vec3>,
    pub dist: Option<f32>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
}

/// Real seconds a state is shown before its picture is taken: the overlays,
/// meshes and the camera have to settle.
const SETTLE_S: f32 = 2.0;

#[derive(Default, Clone)]
struct State {
    name: String,
    case: Option<String>,
    t_min: f32,
    plan: String,
    prio: Vec<String>,
    pre: Vec<String>,
    evac: Vec<String>,
    phase: String,
    at: String,
    dist: Option<f32>,
    yaw: Option<f32>,
    pitch: Option<f32>,
    select: Option<String>,
    /// `conferma=<quartiere>`: «Evacua» pressed, waiting for its confirmation.
    confirm: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    mario: bool,
    #[cfg(not(target_arch = "wasm32"))]
    wing: bool,
}

fn names(v: &str) -> Vec<String> {
    v.split('+').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

fn parse(spec: &str) -> Vec<State> {
    spec.split(';')
        .filter(|s| !s.trim().is_empty())
        .enumerate()
        .map(|(i, s)| {
            let mut st = State { name: format!("snap{i}"), plan: "scripted".into(), phase: "esegui".into(), at: "home".into(), ..default() };
            for kv in s.split(',') {
                let Some((k, v)) = kv.split_once('=') else { continue };
                let v = v.trim();
                match k.trim() {
                    "name" => st.name = v.into(),
                    "case" => st.case = Some(v.into()),
                    "t" => st.t_min = v.parse().unwrap_or(0.0),
                    "plan" => st.plan = v.into(),
                    "prio" => st.prio = names(v),
                    "pre" => st.pre = names(v),
                    "evac" => st.evac = names(v),
                    "phase" => st.phase = v.into(),
                    "at" => st.at = v.into(),
                    "dist" => st.dist = v.parse().ok(),
                    "yaw" => st.yaw = v.parse().ok(),
                    "pitch" => st.pitch = v.parse().ok(),
                    "select" => st.select = Some(v.into()),
                    "conferma" => st.confirm = Some(v.into()),
                    #[cfg(not(target_arch = "wasm32"))]
                    "mario" => st.mario = v == "1",
                    #[cfg(not(target_arch = "wasm32"))]
                    "wing" => st.wing = v == "1",
                    other => eprintln!("snap: unknown key `{other}`"),
                }
            }
            st
        })
        .collect()
}

#[derive(Default)]
pub struct Progress {
    states: Option<Vec<State>>,
    at: usize,
    applied: bool,
    waited: f32,
    /// Camera goal of the state on show, kept up every frame.
    goal: Option<(Vec3, Option<f32>, Option<f32>, Option<f32>)>,
    done_s: f32,
    demo_frame: u32,
}

/// Is a snapshot run on? (Then the title's script and the tour stay out.)
pub fn active() -> bool {
    std::env::var_os("KIOSK_SNAP").is_some()
}

/// Bring the game to the state: restart if it must go back, set the plan, run
/// the clock headless, then the phase and what is selected.
fn apply(st: &State, kiosk: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) -> Option<(Vec3, Option<f32>, Option<f32>, Option<f32>)> {
    let t_s = (st.t_min * 60.0) as i64;
    let case = st.case.clone().unwrap_or_else(|| if kiosk.case.is_empty() { kiosk.pick.clone() } else { kiosk.case.clone() });
    let fresh = !kiosk.started || case != kiosk.case || t_s < sim.time_s() || st.plan != "keep";
    kiosk.title = false;
    kiosk.intro = false;
    kiosk.tour = None;
    kiosk.tour_on = false;
    kiosk.follow = false;
    kiosk.shot = None;
    kiosk.speed = 0.0;
    kiosk.still = true;
    if kiosk.player.is_empty() {
        kiosk.player = "Giulia".into();
    }
    if fresh {
        kiosk.case = case;
        restart(kiosk, sim, restarted, st.phase == "fine");
    }
    kiosk.started = true;
    // the level of this case, for the end screen's ranking
    kiosk.level = kiosk.territory.levels.iter().position(|l| l.case == kiosk.case);
    let n = sim.districts.len();
    let idx = |sim: &Sim, l: &[String]| -> Vec<usize> { l.iter().filter_map(|s| sim.district_index(s)).collect() };
    if st.plan != "keep" {
        let near = sim.district_index(&sim.case.near).unwrap_or(0);
        let mut plan = Plan::new(n);
        if st.plan == "scripted" {
            let mut prio = vec![near];
            prio.extend((0..n).filter(|&d| d != near));
            plan = plan.with_priorities(&prio).with_civil(near, Civil::Preallerta);
        }
        if !st.prio.is_empty() {
            let mut prio = idx(sim, &st.prio);
            let rest: Vec<usize> = (0..n).filter(|d| !prio.contains(d)).collect();
            prio.extend(rest);
            plan = plan.with_priorities(&prio);
        }
        for d in idx(sim, &st.pre) {
            plan = plan.with_civil(d, Civil::Preallerta);
        }
        kiosk.proposed = plan;
        commit(kiosk, sim);
    }
    if let Err(e) = sim.run_until(t_s) {
        kiosk.error = Some(format!("{e:#}"));
    }
    // the evacuations at `t`, where the mayor has authorised them by then
    // (spec §1, 2026-10-13: none before her authorisation)
    if st.plan != "keep" && !st.evac.is_empty() {
        let mut plan = sim.active.clone();
        for d in idx(sim, &st.evac) {
            if sim.may_evacuate(d) {
                plan = plan.with_civil(d, Civil::Evacua);
            } else {
                eprintln!("snap: {}: evacuazione di {} non ancora autorizzata a t={} min", st.name, sim.districts[d].name, st.t_min);
            }
        }
        kiosk.proposed = plan;
        commit(kiosk, sim);
    }
    // a crisis raised on the way is nobody's to answer here
    let _ = sim.take_crisis();
    sim.generation += 1;
    kiosk.dirty = true;
    kiosk.enter(match st.phase.as_str() {
        "pianifica" => Phase::Pianifica,
        "fine" => Phase::Fine,
        _ => Phase::Esegui,
    });
    kiosk.title = st.phase == "title";
    // `intro`: the opening screen (name and levels), as after «Gioca»
    kiosk.intro = st.phase == "intro";
    kiosk.started = !kiosk.title && !kiosk.intro;
    // after the phase: entering one closes the opened chip
    kiosk.selected = st.select.as_deref().and_then(|s| sim.district_index(s));
    kiosk.confirm_evac = st.confirm.as_deref().and_then(|s| sim.district_index(s)).map(|d| (d, 0.0));
    // Ahead of the clock the quiet-time boost would not run the world: speed is 0.

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
            } else if let Some(n) = other.strip_prefix("post:") {
                n.parse::<usize>().ok().and_then(|n| sim.posts.iter().flatten().nth(n)).map(|p| p.at)
            } else if let Some(n) = other.strip_prefix("unit:") {
                n.parse::<usize>().ok().and_then(|n| sim.crews.units.get(n)).map(|u| u.pos)
            } else {
                other.split_once(':').and_then(|(x, y)| Some(Pos { x: x.parse().ok()?, y: y.parse().ok()? }))
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
/// picture, go on; quit after the last. Runs before the camera system, which
/// turns the orbit it is given into the view.
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
    #[cfg(not(target_arch = "wasm32"))] mut mario: ResMut<crate::mario::Playground>,
) {
    let Ok(dir) = std::env::var("KIOSK_SNAP") else { return };
    let Ok(window) = windows.get_single() else { return };
    if p.states.is_none() {
        let spec = std::env::var("KIOSK_SNAP_SPEC").unwrap_or_else(|_| "name=snap0".into());
        p.states = Some(parse(&spec));
        let _ = std::fs::create_dir_all(&dir);
        eprintln!("snap: cases {:?}", kiosk.territory.playlist());
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
    #[cfg(not(target_arch = "wasm32"))]
    let mario_view = {
        if st.mario && p.waited > 0.25 && !mario.active {
            mario.request_start_for_snapshot(st.wing);
        }
        mario.active
    };
    #[cfg(target_arch = "wasm32")]
    let mario_view = false;
    // the camera is held where the state asks, every frame, until the shot:
    // here the orbit, which the kiosk camera then reads; `hold` has the last word
    if !mario_view {
        if let Some((focus, dist, yaw, pitch)) = p.goal {
            *view = SnapView {
                focus: (!focus.is_nan()).then_some(focus),
                dist,
                yaw: yaw.map(f32::to_radians),
                pitch: pitch.map(f32::to_radians),
            };
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
    }
    let ready = st.phase != "fine" || kiosk.baseline().is_some();
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("MARIO_DEMO").is_some() && st.mario {
        if mario.active && p.waited > SETTLE_S {
            if p.demo_frame < 720 {
                mgr.save_screenshot_to_disk(window, format!("{dir}/frame_{:05}.png", p.demo_frame))
                    .expect("demo frame capture failed");
                p.demo_frame += 1;
            } else {
                p.done_s += dt;
                if p.done_s > 2.0 { exit.send(AppExit::Success); }
            }
        }
        return;
    }
    if p.waited > SETTLE_S && ready {
        let _ = mgr.save_screenshot_to_disk(window, format!("{dir}/{}.png", st.name));
        eprintln!("snap: {} (t={} min, phase {})", st.name, st.t_min, st.phase);
        p.at += 1;
        p.applied = false;
    }
}

/// Runs after the kiosk camera: puts the camera exactly where the state on
/// show asks, past the limits the camera keeps for play (see the module doc).
pub fn hold(
    view: Res<SnapView>,
    sim: Res<Sim>,
    mut cam: Query<(&mut OrbitCamera, &mut Transform, Option<&mut bevy::core_pipeline::dof::DepthOfFieldSettings>)>,
    #[cfg(not(target_arch = "wasm32"))] mario: Res<crate::mario::Playground>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    if mario.active { return; }
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
    // not under the ground, but a low camera is what a shot of a cliff wants
    let ground = sim.scn.terrain.height_at(crate::frame::to_world(tf.translation));
    tf.translation.y = tf.translation.y.max(ground + 4.0);
    tf.look_at(orbit.focus, Vec3::Y);
    if let Some(dof) = dof.as_mut() {
        dof.focal_distance = tf.translation.distance(orbit.focus);
    }
}
