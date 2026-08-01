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
use std::sync::mpsc;
use std::sync::Mutex;

use bevy::prelude::*;
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

pub fn setup(mut commands: Commands) {
    let (tx, rx) = mpsc::channel::<ApiRequest>();
    let spawned = std::thread::Builder::new().name("spotorno-api".into()).spawn(move || run_server(tx));
    match spawned {
        Ok(_) => info!("control API listening on http://127.0.0.1:{PORT}"),
        Err(e) => error!("control API: could not start listener thread: {e}"),
    }
    commands.insert_resource(ApiChannel(Mutex::new(rx)));
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
pub fn serve(
    channel: Res<ApiChannel>,
    mut sim: Option<ResMut<Sim>>,
    mut restarted: EventWriter<crate::sim::SimRestarted>,
) {
    let Ok(rx) = channel.0.lock() else { return };
    while let Ok(req) = rx.try_recv() {
        let (status, body) = match sim.as_deref_mut() {
            None => (503, json!({"error": "no scenario loaded yet"}).to_string()),
            Some(sim) => handle(&req, sim, &mut restarted),
        };
        let _ = req.reply.send((status, body));
    }
}

fn handle(req: &ApiRequest, sim: &mut Sim, restarted: &mut EventWriter<crate::sim::SimRestarted>) -> (u16, String) {
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/status") => (200, status_json(sim).to_string()),
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
        ("GET", "/agents/units") => (200, units_json(sim).to_string()),

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
        ("POST", "/control/unit_task") => handle_unit_task(&req.body, sim),

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

fn status_json(sim: &Sim) -> Value {
    let stats = sim.agents.stats();
    let ustats = sim.crews.stats();
    let w = sim.fire.weather();
    json!({
        "time_s": sim.time_s(),
        "clock": sim.clock(),
        "playing": sim.playing,
        "speed": sim.speed,
        "seed": sim.seed,
        "generation": sim.generation,
        "weather": {
            "wind_dir_deg": w.wind_dir_deg,
            "wind_speed_kmh": w.wind_speed_kmh,
            "moisture_pct": w.moisture_pct,
        },
        "households": {
            "aware": stats.aware,
            "preparing": stats.preparing,
            "moving": stats.moving,
            "safe": stats.safe,
            "defending": stats.defending,
            "cutoff": stats.cutoff,
            "casualties": stats.casualties,
        },
        "people": {
            "safe": stats.people_safe,
            "moving": stats.people_moving,
            "at_risk": stats.people_at_risk,
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

fn households_json(sim: &Sim) -> Value {
    json!(sim
        .agents
        .households
        .iter()
        .map(|h| json!({
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
        }))
        .collect::<Vec<_>>())
}

fn units_json(sim: &Sim) -> Value {
    json!(sim
        .crews
        .units
        .iter()
        .map(|u| json!({
            "id": u.id,
            "callsign": u.callsign,
            "kind": u.kind.label(),
            "state": u.state.label(),
            "task": crate::history::task_kind(u.task),
            "pos": {"x": u.pos.x, "y": u.pos.y},
            "note": u.note,
        }))
        .collect::<Vec<_>>())
}
