//! A tiny local HTTP API for inspecting and controlling a running incident.
//!
//! Exists so an external tool — the MCP server in `tools/mcp`, or just
//! `curl` — can read the agent history and drive the simulation without
//! being a Bevy plugin itself. Bound to `127.0.0.1` only, and native-only for
//! the same reason `telemetry` is: the listener is a background OS thread,
//! and wasm32-unknown-unknown has neither threads-with-blocking-IO nor a
//! socket to bind.
//!
//! **One request at a time, deliberately synchronous.** The listener thread
//! sends each request across a channel and blocks on a one-shot reply; the
//! ECS side drains that channel once a frame ([`serve`]) and answers each
//! request against `&mut Sim` with the exact same borrow it would have from
//! any other system. Nothing here needs a second copy of the simulation or
//! its own locking story — a request just waits its turn on the main thread,
//! the same as an egui panel would.
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;

use bevy::prelude::*;
use bevy::render::view::screenshot::ScreenshotManager;
use bevy::window::PrimaryWindow;
use serde_json::{json, Value};

use crate::sim::Sim;

/// Loopback only, fixed: this is a development/inspection surface for one
/// local incident, not a service with clients to version against.
pub const PORT: u16 = 8731;

struct ApiRequest {
    method: String,
    path: String,
    query: String,
    body: String,
    reply: mpsc::Sender<(u16, String)>,
}

#[derive(Resource)]
pub struct ApiChannel(Mutex<mpsc::Receiver<ApiRequest>>);

/// A screenshot queued by `POST /control/screenshot`, waiting out its settle
/// window. See [`take_pending_shot`].
struct ShotRequest {
    path: PathBuf,
    settle: u32,
}

/// Frames to let the camera move / the fire-overlay rebuild land on screen
/// before the shot is taken. Mirrors `capture::SETTLE_FRAMES`, which exists
/// for the same reason: grabbing on the very next frame can catch the
/// previous state still on screen.
const SHOT_SETTLE_FRAMES: u32 = 20;

#[derive(Resource, Default)]
pub struct PendingShot(Option<ShotRequest>);

pub fn setup(mut commands: Commands) {
    let (tx, rx) = mpsc::channel::<ApiRequest>();
    let spawned = std::thread::Builder::new().name("spotorno-api".into()).spawn(move || run_server(tx));
    match spawned {
        Ok(_) => info!("control API listening on http://127.0.0.1:{PORT}"),
        Err(e) => error!("control API: could not start listener thread: {e}"),
    }
    commands.insert_resource(ApiChannel(Mutex::new(rx)));
    commands.insert_resource(PendingShot::default());
}

/// Save a screenshot queued by [`handle_screenshot`], once its settle window
/// has elapsed. A separate system, run every frame regardless of state,
/// because `ScreenshotManager::save_screenshot_to_disk` hands the request to
/// the render world rather than writing inline (see `capture::scripted`,
/// the unattended-review path this mirrors) -- the API call that queued it
/// has already answered by the time the file actually lands.
pub fn take_pending_shot(
    mut pending: ResMut<PendingShot>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut shots: ResMut<ScreenshotManager>,
) {
    let Some(req) = pending.0.as_mut() else { return };
    if req.settle > 0 {
        req.settle -= 1;
        return;
    }
    let Ok(window) = windows.get_single() else {
        pending.0 = None;
        return;
    };
    let path = req.path.clone();
    match shots.save_screenshot_to_disk(window, &path) {
        Ok(()) => info!("wrote {}", path.display()),
        Err(e) => error!("screenshot failed: {e}"),
    }
    pending.0 = None;
}

fn run_server(tx: mpsc::Sender<ApiRequest>) {
    let server = match tiny_http::Server::http(("127.0.0.1", PORT)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("control API: could not bind 127.0.0.1:{PORT}: {e}");
            return;
        }
    };
    for mut request in server.incoming_requests() {
        let mut body = String::new();
        let _ = std::io::Read::read_to_string(request.as_reader(), &mut body);
        let url = request.url().to_string();
        let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
        let (reply_tx, reply_rx) = mpsc::channel();
        let req = ApiRequest {
            method: request.method().as_str().to_string(),
            path: path.to_string(),
            query: query.to_string(),
            body,
            reply: reply_tx,
        };
        if tx.send(req).is_err() {
            break; // the game has exited
        }
        let (status, body) = reply_rx
            .recv()
            .unwrap_or((503, json!({"error": "game did not respond"}).to_string()));
        let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
            .expect("static header");
        let response = tiny_http::Response::from_string(body).with_status_code(status).with_header(header);
        let _ = request.respond(response);
    }
}

/// Drain every request queued since the last frame and answer it against the
/// live `Sim`, if one is loaded. Runs every frame in both app states — the
/// scenario selector included — so a client is never left hanging just
/// because no incident has been picked yet.
///
/// Scenario listing and loading are handled before the `Option<ResMut<Sim>>`
/// branch, deliberately: they are the two requests a client needs to be able
/// to make *without* an incident already running, the same way the human
/// scenario selector is its own screen rather than something bolted onto the
/// play view.
pub fn serve(
    channel: Res<ApiChannel>,
    mut sim: Option<ResMut<Sim>>,
    mut restarted: EventWriter<crate::sim::SimRestarted>,
    mut selector: ResMut<crate::scenario_selector::ScenarioSelector>,
    state: Res<State<crate::AppState>>,
    mut next_state: ResMut<NextState<crate::AppState>>,
    buildings: Option<Res<crate::buildings::Buildings>>,
    mut pending_shot: ResMut<PendingShot>,
    mut orbit: Query<&mut crate::camera::OrbitCamera>,
    mut layer: Option<ResMut<crate::fire_view::FireLayer>>,
) {
    let Ok(rx) = channel.0.lock() else { return };
    while let Ok(req) = rx.try_recv() {
        let scenario_route = match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/scenarios") => Some(handle_list_scenarios(&selector)),
            ("POST", "/control/load_scenario") => {
                Some(handle_load_scenario(&req.body, &mut selector, &state, &mut next_state))
            }
            _ => None,
        };
        let (status, body) = match scenario_route {
            Some(r) => r,
            None => match sim.as_deref_mut() {
                None => (503, json!({"error": "no scenario loaded yet"}).to_string()),
                Some(sim) => handle(
                    &req,
                    sim,
                    &mut restarted,
                    buildings.as_deref(),
                    &mut pending_shot,
                    &mut orbit,
                    layer.as_deref_mut(),
                ),
            },
        };
        let _ = req.reply.send((status, body));
    }
}

/// Every scenario the registry knows about -- real incidents and the
/// synthetic ABM laboratories alike -- so a client can pick one without
/// reading `data/scenarios.json` by hand.
fn handle_list_scenarios(selector: &crate::scenario_selector::ScenarioSelector) -> (u16, String) {
    let Some(registry) = &selector.registry else {
        return (503, err("scenario registry not loaded yet"));
    };
    let rows: Vec<Value> = registry
        .list()
        .into_iter()
        .map(|s| {
            json!({
                "id": s.id,
                "name": s.name,
                "description": s.description,
                "location": s.location,
                "is_dev": s.is_dev,
                "buildings": s.buildings_count,
                "households": s.households_count,
                "people": s.people_count,
                "fire_grid_size": s.fire_grid_size,
                "tags": s.tags,
            })
        })
        .collect();
    (
        200,
        json!({"default": registry.default_id(), "scenarios": rows}).to_string(),
    )
}

/// Switch the running incident to another scenario, in the same process --
/// what the "Scenario ▸ Load scenario…" menu item does, minus the human
/// standing in front of the picker. Works from either app state: from
/// `Playing` it routes back through `SelectingScenario` first, so the scene
/// gets torn down (`teardown_scene`) exactly as it would for a player; from
/// `SelectingScenario` itself (nothing launched yet) `handle_launch_selection`
/// picks the confirmed selection up on the very next frame.
///
/// Answers immediately with `"loading"` rather than waiting for the load to
/// finish -- the scenario read, the fire model build and the scene spawn all
/// happen in later systems, over one or more subsequent frames. A caller
/// should poll `GET /status` until `scenario.id` matches.
fn handle_load_scenario(
    body: &str,
    selector: &mut crate::scenario_selector::ScenarioSelector,
    state: &State<crate::AppState>,
    next_state: &mut NextState<crate::AppState>,
) -> (u16, String) {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let Some(id) = v.get("id").and_then(Value::as_str) else {
        return (400, err("expected {\"id\": scenario id -- see GET /scenarios}"));
    };
    let Some(registry) = &selector.registry else {
        return (503, err("scenario registry not loaded yet"));
    };
    if registry.get(id).is_none() {
        return (404, err(format!("no scenario \"{id}\" (see GET /scenarios)")));
    }
    selector.selected = Some(id.to_string());
    selector.confirmed = true;
    selector.error = None;
    if *state.get() == crate::AppState::Playing {
        next_state.set(crate::AppState::SelectingScenario);
    }
    (200, json!({"ok": true, "loading": id}).to_string())
}

fn handle(
    req: &ApiRequest,
    sim: &mut Sim,
    restarted: &mut EventWriter<crate::sim::SimRestarted>,
    buildings: Option<&crate::buildings::Buildings>,
    pending_shot: &mut PendingShot,
    orbit: &mut Query<&mut crate::camera::OrbitCamera>,
    layer: Option<&mut crate::fire_view::FireLayer>,
) -> (u16, String) {
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/status") => (200, status_json(sim, buildings).to_string()),
        ("GET", "/history/recent") => {
            let limit = query_get(&req.query, "limit").and_then(|s| s.parse().ok()).unwrap_or(50usize).min(1000);
            (200, json!(sim.history.log.recent(limit).iter().map(event_json).collect::<Vec<_>>()).to_string())
        }
        ("GET", "/history/subject") => {
            let kind = query_get(&req.query, "kind");
            let id = query_get(&req.query, "id").and_then(|s| s.parse::<i64>().ok());
            let Some(subject) = kind.and_then(|k| telemetry::Subject::from_parts(k, id)) else {
                return (
                    400,
                    json!({"error": "need ?kind=household|person|traveller|unit|command and, except for command, &id=N"}).to_string(),
                );
            };
            (200, json!(sim.history.log.events_for(subject).iter().map(event_json).collect::<Vec<_>>()).to_string())
        }
        ("GET", "/agents/households") => (200, households_json(sim).to_string()),
        ("GET", "/agents/people") => (200, people_json(sim).to_string()),
        ("GET", "/agents/units") => (200, units_json(sim).to_string()),

        ("GET", "/behaviour/graphs") => (200, graphs_json(sim).to_string()),
        ("GET", "/behaviour/profiles") => (200, profiles_json(sim).to_string()),

        ("POST", "/control/play") => match body_bool(&req.body, "playing") {
            Some(playing) => {
                sim.playing = playing;
                (200, ok())
            }
            None => (400, err("expected {\"playing\": bool}")),
        },
        ("POST", "/control/speed") => match body_f32(&req.body, "speed") {
            Some(speed) => {
                sim.speed = speed.clamp(crate::ui::MIN_SPEED, crate::ui::MAX_SPEED);
                (200, ok())
            }
            None => (400, err("expected {\"speed\": number}")),
        },
        ("POST", "/control/step") => {
            sim.request_step();
            (200, ok())
        }
        ("POST", "/control/restart") => match sim.restart() {
            Ok(()) => {
                restarted.send(crate::sim::SimRestarted);
                (200, ok())
            }
            Err(e) => (500, err(e)),
        },
        ("POST", "/control/order_evacuation") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(Value::Null);
            let n = match (v.get("x").and_then(Value::as_f64), v.get("y").and_then(Value::as_f64)) {
                (Some(x), Some(y)) => {
                    let radius_m = v.get("radius_m").and_then(Value::as_f64).unwrap_or(2000.0) as f32;
                    sim.agents.order_evacuation(scenario::Pos { x: x as f32, y: y as f32 }, radius_m)
                }
                _ => sim.agents.order_evacuation_all(),
            };
            (200, json!({"ok": true, "households_ordered": n}).to_string())
        }
        ("POST", "/control/ignite") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(Value::Null);
            let (Some(x), Some(y)) = (v.get("x").and_then(Value::as_f64), v.get("y").and_then(Value::as_f64)) else {
                return (400, err("expected {\"x\": number, \"y\": number, \"radius_m\": number}"));
            };
            let radius_m = (v.get("radius_m").and_then(Value::as_f64).unwrap_or(120.0) as f32)
                .clamp(crate::sim::MIN_IGNITION_RADIUS_M, crate::sim::MAX_IGNITION_RADIUS_M);
            let cell = sim.scenario.world.cell_of(scenario::Pos { x: x as f32, y: y as f32 });
            match sim.add_ignition(cell, radius_m) {
                Ok(()) => (200, ok()),
                Err(e) => (500, err(e)),
            }
        }
        ("POST", "/control/weather") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(Value::Null);
            if let Some(x) = v.get("wind_dir_deg").and_then(Value::as_f64) {
                sim.weather.wind_dir_deg = x;
            }
            if let Some(x) = v.get("wind_speed_kmh").and_then(Value::as_f64) {
                sim.weather.wind_speed_kmh = x;
            }
            if let Some(x) = v.get("moisture_pct").and_then(Value::as_f64) {
                sim.weather.moisture_pct = x;
            }
            match sim.apply_weather() {
                Ok(()) => (200, ok()),
                Err(e) => (500, err(e)),
            }
        }
        // The commander's other two levers. Both live here rather than on the
        // map because both need a *place* and three tools already contend for
        // left-click — a fourth needs a rule, not a patch (see the note in
        // CLAUDE.md). A closure is an area and the API is where an area can be
        // given without inventing one.
        ("POST", "/control/close_road") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(Value::Null);
            let (Some(x), Some(y)) = (
                v.get("x").and_then(Value::as_f64),
                v.get("y").and_then(Value::as_f64),
            ) else {
                return (
                    400,
                    err("expected {\"x\": number, \"y\": number, \"radius_m\": number, \"minutes\": number}"),
                );
            };
            let radius_m = v.get("radius_m").and_then(Value::as_f64).unwrap_or(300.0) as f32;
            let minutes = v.get("minutes").and_then(Value::as_f64).unwrap_or(30.0) as f32;
            let links = sim.agents.close_road(
                scenario::Pos { x: x as f32, y: y as f32 },
                radius_m,
                minutes * 60.0,
            );
            // Zero links is not an error and is worth reporting: a closure over
            // open ground looks exactly like one that worked.
            (200, json!({"ok": true, "links_closed": links}).to_string())
        }
        ("POST", "/control/reopen_roads") => {
            sim.agents.reopen_roads();
            (200, ok())
        }
        ("POST", "/control/boat_lift") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(Value::Null);
            let minutes = v.get("minutes").and_then(Value::as_f64).unwrap_or(30.0) as f32;
            let rate = v.get("rate_per_min").and_then(Value::as_f64).unwrap_or(3.5) as f32;
            match sim.agents.request_boat_lift(minutes * 60.0, rate) {
                Ok(()) => (200, ok()),
                Err(e) => (409, err(e)),
            }
        }
        ("POST", "/control/unit_task") => handle_unit_task(&req.body, sim),

        // The composer's two levers, reachable without opening the editor:
        // nudge one profile's share/enabled and restart on it, or swap in a
        // whole library read fresh off disk. Both go through `sim.behaviour`
        // directly rather than through `composer::Composer` — the composer's
        // `Snarl` editing session is a separate, human-facing copy (see
        // `composer/mod.rs`'s module doc), and these two do not touch it, the
        // same way the in-game "Apply and restart" only ever moves data the
        // other way.
        ("POST", "/control/set_profile") => handle_set_profile(&req.body, sim, restarted),
        ("POST", "/control/reload_behaviour") => handle_reload_behaviour(&req.body, sim, restarted),

        // A client with no window of its own -- an MCP tool, a `curl` prompt
        // -- otherwise has no way to see what the incident looks like.
        // Answers once the shot is queued, not once it is written; see
        // `take_pending_shot`.
        ("POST", "/control/screenshot") => handle_screenshot(&req.body, sim, pending_shot, orbit, layer),

        _ => (404, err("no such route")),
    }
}

fn handle_unit_task(body: &str, sim: &mut Sim) -> (u16, String) {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let Some(id) = v.get("id").and_then(Value::as_u64).map(|n| n as usize) else {
        return (400, err("expected \"id\": unit index (see /agents/units)"));
    };
    let f = |k: &str| v.get(k).and_then(Value::as_f64).map(|n| n as f32);
    let pos = |xk: &str, yk: &str| match (f(xk), f(yk)) {
        (Some(x), Some(y)) => Some(scenario::Pos { x, y }),
        _ => None,
    };
    let task = match v.get("task").and_then(Value::as_str) {
        Some("hold") => abm::Task::Hold,
        Some("return") => abm::Task::Return,
        Some("attack") => match pos("x", "y") {
            Some(at) => abm::Task::Attack { at },
            None => return (400, err("attack needs \"x\"/\"y\"")),
        },
        Some("drop") => match pos("x", "y") {
            Some(at) => abm::Task::Drop { at },
            None => return (400, err("drop needs \"x\"/\"y\"")),
        },
        Some("line") => match (pos("from_x", "from_y"), pos("to_x", "to_y")) {
            (Some(from), Some(to)) => abm::Task::Line { from, to },
            _ => return (400, err("line needs \"from_x\"/\"from_y\"/\"to_x\"/\"to_y\"")),
        },
        _ => return (400, err("\"task\" must be one of hold, return, attack, drop, line")),
    };
    match sim.crews.assign(id, task) {
        Ok(()) => (200, ok()),
        Err(e) => (409, err(e)),
    }
}

/// Nudge one profile's share (households/people) or enabled flag (units) and
/// restart on it -- the composer's "Apply and restart" for a single number,
/// with no editor window required.
///
/// Restart is transactional in `Sim` itself, but the mutation below happens
/// first and is not: if the edit leaves no domain with a runnable policy (say,
/// the last household profile's share zeroed), `sim.restart()` fails and the
/// mutation has to be undone by hand, the same guarantee
/// `Sim::apply_behaviour` gives by swapping the whole library instead.
fn handle_set_profile(
    body: &str,
    sim: &mut Sim,
    restarted: &mut EventWriter<crate::sim::SimRestarted>,
) -> (u16, String) {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let Some(id) = v.get("id").and_then(Value::as_str) else {
        return (400, err("expected \"id\": profile id (see /behaviour/profiles)"));
    };
    let share = v.get("share").and_then(Value::as_f64);
    let enabled = v.get("enabled").and_then(Value::as_bool);
    if share.is_none() && enabled.is_none() {
        return (400, err("expected \"share\" (number) and/or \"enabled\" (bool)"));
    }
    let Some(previous) = sim.behaviour.subtypes.get(id).cloned() else {
        return (404, err(format!("no profile \"{id}\" (see /behaviour/profiles)")));
    };
    let sub = sim.behaviour.subtypes.get_mut(id).expect("just found it above");
    if let Some(share) = share {
        sub.share = share as f32;
    }
    if let Some(enabled) = enabled {
        sub.enabled = enabled;
    }
    match sim.restart() {
        Ok(()) => {
            restarted.send(crate::sim::SimRestarted);
            let s = &sim.behaviour.subtypes[id];
            (200, json!({"ok": true, "id": id, "share": s.share, "enabled": s.enabled}).to_string())
        }
        Err(e) => {
            sim.behaviour.subtypes.insert(id.to_string(), previous);
            (500, err(e))
        }
    }
}

/// Read a behaviour library fresh off disk and adopt it, like the composer's
/// own "Reload" followed by "Apply and restart" -- for regenerating
/// `data/behaviours/` from `defaults.rs` (see
/// `cargo test -p behavior -- --ignored write_shipped_library` in CLAUDE.md)
/// or hand-editing a profile's JSON outside the game entirely.
///
/// Lenient the same way the composer is: one malformed file costs that file,
/// not the whole reload, and `file_errors` says which. An empty result (every
/// file failed, or the directory has none) is refused rather than silently
/// leaving the incident on its previous library.
fn handle_reload_behaviour(
    body: &str,
    sim: &mut Sim,
    restarted: &mut EventWriter<crate::sim::SimRestarted>,
) -> (u16, String) {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let root = match v.get("path").and_then(Value::as_str) {
        Some(p) => std::path::PathBuf::from(p),
        None => std::path::PathBuf::from(std::env::var("SPOTORNO_DATA").unwrap_or_else(|_| "data".into()))
            .join(behavior::library::DEFAULT_DIR),
    };
    let report = match behavior::Library::load_dir_reported(&root) {
        Ok(r) => r,
        Err(e) => return (500, err(format!("reading {}: {e:#}", root.display()))),
    };
    if report.library.graphs.is_empty() {
        return (400, err(format!("no behaviours found at {}", root.display())));
    }
    let file_errors: Vec<Value> = report
        .files
        .iter()
        .filter(|f| !f.ok())
        .map(|f| json!({"file": f.name(), "error": f.error.clone().unwrap_or_default()}))
        .collect();
    let (graphs, profiles) = (report.library.graphs.len(), report.library.subtypes.len());
    match sim.apply_behaviour(report.library) {
        Ok(()) => {
            restarted.send(crate::sim::SimRestarted);
            (
                200,
                json!({
                    "ok": true,
                    "path": root.display().to_string(),
                    "graphs": graphs,
                    "profiles": profiles,
                    "file_errors": file_errors,
                })
                .to_string(),
            )
        }
        Err(e) => (500, err(e)),
    }
}

/// Move the camera and/or switch the fire-overlay layer, then queue a
/// screenshot. `path` has to be a path the game process can write to --
/// typically somewhere under the caller's own scratch directory, since this
/// process and the client are not usually the same user-facing sandbox.
///
/// `focus_x`/`focus_y` are scenario-frame world metres, the same coordinates
/// every other endpoint here uses (`/control/ignite`, `/control/close_road`,
/// ...); the conversion to Bevy space needs `Terrain::height_at`, which is
/// why this is folded into the ordinary `sim`-bearing dispatch rather than
/// handled up in `serve` alongside the scenario routes.
fn handle_screenshot(
    body: &str,
    sim: &Sim,
    pending: &mut PendingShot,
    orbit: &mut Query<&mut crate::camera::OrbitCamera>,
    layer: Option<&mut crate::fire_view::FireLayer>,
) -> (u16, String) {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let Some(path) = v.get("path").and_then(Value::as_str) else {
        return (400, err("expected {\"path\": \"/absolute/path.png\"}"));
    };
    if pending.0.is_some() {
        return (409, err("a screenshot is already pending -- wait for it to land"));
    }
    let Ok(mut cam) = orbit.get_single_mut() else {
        return (503, err("no camera -- is a scenario loaded?"));
    };
    if let (Some(x), Some(y)) = (
        v.get("focus_x").and_then(Value::as_f64),
        v.get("focus_y").and_then(Value::as_f64),
    ) {
        let focus = scenario::Pos { x: x as f32, y: y as f32 };
        cam.focus = crate::frame::to_bevy(focus, sim.scenario.terrain.height_at(focus));
    }
    if let Some(d) = v.get("distance_m").and_then(Value::as_f64) {
        cam.distance = d as f32;
    }
    if let Some(yaw) = v.get("yaw_deg").and_then(Value::as_f64) {
        cam.yaw = (yaw as f32).to_radians();
    }
    if let Some(pitch) = v.get("pitch_deg").and_then(Value::as_f64) {
        cam.pitch = (pitch as f32).to_radians();
    }
    if let Some(name) = v.get("layer").and_then(Value::as_str) {
        match (layer, crate::fire_view::FireLayer::ALL.into_iter().find(|l| l.label().eq_ignore_ascii_case(name))) {
            (Some(layer), Some(found)) => *layer = found,
            (_, None) => {
                return (
                    400,
                    err(format!("no such layer \"{name}\" (flames, intensity, arrival, spread risk)")),
                )
            }
            (None, Some(_)) => {} // no fire-view resource; nothing to switch
        }
    }
    let settle = v
        .get("settle_frames")
        .and_then(Value::as_u64)
        .unwrap_or(SHOT_SETTLE_FRAMES as u64) as u32;
    let path = PathBuf::from(path);
    pending.0 = Some(ShotRequest { path: path.clone(), settle });
    (200, json!({"ok": true, "path": path.display().to_string(), "settle_frames": settle}).to_string())
}

fn ok() -> String {
    json!({"ok": true}).to_string()
}

fn err(e: impl std::fmt::Display) -> String {
    json!({"error": e.to_string()}).to_string()
}

fn query_get<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').filter_map(|kv| kv.split_once('=')).find(|(k, _)| *k == key).map(|(_, v)| v)
}

fn body_bool(body: &str, key: &str) -> Option<bool> {
    serde_json::from_str::<Value>(body).ok()?.get(key)?.as_bool()
}

fn body_f32(body: &str, key: &str) -> Option<f32> {
    Some(serde_json::from_str::<Value>(body).ok()?.get(key)?.as_f64()? as f32)
}

fn event_json(e: &telemetry::Event) -> Value {
    json!({
        "sim_time_s": e.sim_time_s,
        "subject_kind": e.subject.kind(),
        "subject_id": e.subject.id(),
        "kind": e.kind,
        "detail": e.detail,
        "summary": crate::history::summarize(&e.kind, &e.detail),
    })
}

fn status_json(sim: &Sim, buildings: Option<&crate::buildings::Buildings>) -> Value {
    let stats = sim.agents.stats();
    let ustats = sim.crews.stats();
    let w = sim.fire.weather();
    let cell_ha = (sim.scenario.world.cellsize * sim.scenario.world.cellsize) / 10_000.0;
    let burnt_ha = sim
        .fire
        .state()
        .iter()
        .filter(|c| **c != fire::CellFire::Unburnt)
        .count() as f32
        * cell_ha;
    let peak_fli = sim.fire.intensity().iter().cloned().fold(0.0f32, f32::max);
    let damage = buildings.map(|b| b.damage_counts()).unwrap_or_default();
    json!({
        "time_s": sim.time_s(),
        "clock": sim.clock(),
        "playing": sim.playing,
        "speed": sim.speed,
        "seed": sim.seed,
        "generation": sim.generation,
        "scenario": {
            "id": sim.scenario.id,
            "name": sim.scenario.metadata.name,
            "is_dev": sim.scenario.metadata.is_dev,
        },
        "weather": {
            "wind_dir_deg": w.wind_dir_deg,
            "wind_speed_kmh": w.wind_speed_kmh,
            "moisture_pct": w.moisture_pct,
        },
        "fire": {
            "burnt_ha": burnt_ha,
            "active_front_cells": sim.fire.active_cells().len(),
            "peak_fireline_kw_m": peak_fli,
        },
        "structures": {
            "threatened": damage.threatened,
            "alight": damage.alight,
            "destroyed": damage.destroyed,
        },
        "households": {
            "aware": stats.aware,
            "preparing": stats.preparing,
            "moving": stats.moving,
            "safe": stats.safe,
            "defending": stats.defending,
            "cutoff": stats.cutoff,
            "casualties": stats.casualties,
            // Alive, out of the house, off the road and *not* evacuated. A
            // separate count because collapsing it into either of the two
            // either overstates the evacuation or understates who is alive.
            "sheltering": stats.sheltering,
            "at_the_shore": stats.at_the_shore,
        },
        "people": {
            "safe": stats.people_safe,
            "moving": stats.people_moving,
            "at_risk": stats.people_at_risk,
            "lifted_by_boat": stats.lifted,
        },
        "incident": {
            "road_closures": sim.agents.closures().len(),
            "boat_lift_min": sim
                .agents
                .boat_lift()
                .map(|b| b.minutes_out(sim.agents.time_s()) as f64),
            "masts_down": sim.agents.comms().down(),
            "spot_fires": sim.agents.spot_fires().len(),
        },
        "units": {
            "staged": ustats.staged,
            "responding": ustats.responding,
            "working": ustats.working,
            "refilling": ustats.refilling,
            "withdrawing": ustats.withdrawing,
        },
    })
}

/// A running decision, for a client checking whether a behaviour change (see
/// `set_profile`/`reload_behaviour`) actually altered what an agent is doing
/// rather than just which file it reads from.
fn decision_json(d: behavior::Decision) -> Value {
    json!({
        "action": d.action,
        "priority": d.priority,
        "prep_scale": d.prep_scale,
        "urgency": d.urgency,
    })
}

fn households_json(sim: &Sim) -> Value {
    json!(sim
        .agents
        .households
        .iter()
        .map(|h| {
            let subtype = sim.agents.behaviour_of(h.id);
            json!({
                "id": h.id,
                "status": crate::inspect::status_text(h.status),
                "ordered": h.ordered,
                "warning_received": h.warning_received,
                "intent": match h.intent {
                    scenario::population::Intent::LeaveEarly => "leave_early",
                    scenario::population::Intent::WaitAndSee => "wait_and_see",
                    scenario::population::Intent::StayDefend => "stay_defend",
                },
                "members": h.members.len(),
                "home": {"x": h.home.x, "y": h.home.y},
                // Which authored profile is driving this household, and what
                // it decided most recently -- absent when no household
                // behaviour library is loaded (the hand-written model runs
                // instead, see `Sim::behaviour`).
                "subtype": subtype.map(|(id, name, _)| json!({"id": id, "name": name})),
                "decision": subtype.map(|(_, _, d)| decision_json(d)),
            })
        })
        .collect::<Vec<_>>())
}

fn units_json(sim: &Sim) -> Value {
    json!(sim
        .crews
        .units
        .iter()
        .map(|u| {
            let policy = sim.crews.policy_of(u.id);
            json!({
                "id": u.id,
                "callsign": u.callsign,
                "kind": u.kind.label(),
                "state": u.state.label(),
                "task": crate::history::task_kind(u.task),
                "pos": {"x": u.pos.x, "y": u.pos.y},
                "note": u.note,
                "policy": policy.map(|(id, name)| json!({"id": id, "name": name})),
            })
        })
        .collect::<Vec<_>>())
}

/// The separated-people roster -- the ~400 of 1,577 who are not with their
/// household (finding: `PersonAgent::away`) and are agents in their own
/// right. Absent from the API until now, which left half the civilian model
/// (everyone the household roster cannot represent) unreachable to a client.
fn people_json(sim: &Sim) -> Value {
    json!(sim
        .agents
        .people
        .iter()
        .map(|p| {
            let subtype = sim.agents.person_behaviour_of(p.id);
            json!({
                "id": p.id,
                "household": p.household,
                "status": crate::inspect::status_text(p.status),
                "away": p.away,
                "needs_assistance": p.needs_assistance,
                "pos": {"x": p.pos.x, "y": p.pos.y},
                "subtype": subtype.map(|(id, name, _)| json!({"id": id, "name": name})),
                "decision": subtype.map(|(_, _, d)| decision_json(d)),
            })
        })
        .collect::<Vec<_>>())
}

/// Which domain a subtype governs. `AgentSubtype` itself carries no domain
/// field, only the graph id it runs (`crates/behavior/src/subtype.rs`), so
/// this is a lookup rather than a field read -- a profile whose graph has
/// been deleted from the library has no answer, which is reported as `null`
/// rather than guessed.
fn subtype_domain(lib: &behavior::Library, s: &behavior::AgentSubtype) -> Option<behavior::Domain> {
    lib.graphs.get(&s.graph).map(|g| g.domain)
}

fn graphs_json(sim: &Sim) -> Value {
    json!(sim
        .behaviour
        .graphs
        .values()
        .map(|g| json!({
            "id": g.id,
            "name": g.name,
            "domain": g.domain.key(),
            "description": g.description,
            "nodes": g.nodes.len(),
        }))
        .collect::<Vec<_>>())
}

/// The composer's "which behaviour is each agent actually running" view,
/// without opening the editor: every profile, its graph, its domain, and the
/// one number that decides whether it is in play -- `share` for households
/// and separated people, `enabled` (plus which unit kinds) for suppression
/// units. This is what `set_profile` reads before a client picks an `id`.
fn profiles_json(sim: &Sim) -> Value {
    json!(sim
        .behaviour
        .subtypes
        .values()
        .map(|s| json!({
            "id": s.id,
            "name": s.name,
            "description": s.description,
            "graph": s.graph,
            "domain": subtype_domain(&sim.behaviour, s).map(|d| d.key()),
            "share": s.share,
            "enabled": s.enabled,
            "unit_kinds": s.unit_kinds.iter().map(|k| k.key()).collect::<Vec<_>>(),
            "tags": s.tags,
        }))
        .collect::<Vec<_>>())
}
