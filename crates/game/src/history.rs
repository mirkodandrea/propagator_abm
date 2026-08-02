//! The per-run agent-history log: a thin diffing layer over
//! [`telemetry::Recorder`].
//!
//! `telemetry` knows nothing about `Abm` or `Suppression` — deliberately, so
//! it never has to change when the model does. This module is the other
//! half: it holds a snapshot of the fields worth watching, and on every
//! [`Sim::advance`](crate::sim::Sim::advance) it diffs the live model against
//! that snapshot and records one event per thing that actually changed.
//!
//! **Diffing, not instrumenting.** The alternative would be to call into the
//! log from inside `abm::decide`, `move_travellers`, `suppression::step` and
//! so on — the exact moment each change happens, plus the "why". That would
//! mean `abm` depending on `telemetry` and every one of those signatures
//! changing, which is real surface area to add to a crate whose tests exist
//! precisely to be fast and to pin step-size invariance (findings 5 and the
//! rules under "Working agreements"). Reading the already-`pub` fields once
//! per `advance` after the model has moved costs a linear scan over a few
//! thousand agents — nothing next to the routing refresh — and touches no
//! invariant `abm` holds.
//!
//! **What is *not* diffed.** Continuous quantities — a household's `cue`,
//! a unit's `water_l`, a traveller's position — change on effectively every
//! tick and would flood the log with noise instead of signal. Only the
//! discrete things a scientist or an interview would actually ask about are
//! watched: status, the winning action of an authored decision, order and
//! warning receipt, travel state, and a unit's state and task.
use scenario::population::Status;
use telemetry::{Recorder, Subject};

#[derive(Clone, Copy, PartialEq)]
struct HouseholdSnap {
    status: Status,
    ordered: bool,
    warning_received: bool,
    action: Option<behavior::ActionKind>,
}

#[derive(Clone, Copy, PartialEq)]
struct PersonSnap {
    status: Status,
    away: bool,
    action: Option<behavior::ActionKind>,
}

#[derive(Clone, Copy, PartialEq)]
struct UnitSnap {
    state: abm::UnitState,
    task: abm::Task,
    drops: u32,
}

/// Owns the log and the last-seen state of every agent, so [`History::observe`]
/// can tell what changed since the previous call.
pub struct History {
    pub log: Recorder,
    households: Vec<HouseholdSnap>,
    people: Vec<PersonSnap>,
    travellers: Vec<abm::TravelState>,
    units: Vec<UnitSnap>,
}

impl History {
    /// A fresh log with a baseline snapshot of `agents`/`crews` and no events
    /// yet — there is nothing to compare the baseline against, and logging
    /// "changed from nothing to normal" for every household would be noise,
    /// not signal.
    pub fn new(agents: &abm::Abm, crews: &abm::Suppression) -> History {
        let mut history = History {
            log: Recorder::new(),
            households: Vec::new(),
            people: Vec::new(),
            travellers: Vec::new(),
            units: Vec::new(),
        };
        history.rebase(agents, crews);
        history
    }

    fn rebase(&mut self, agents: &abm::Abm, crews: &abm::Suppression) {
        self.households = (0..agents.households.len())
            .map(|i| HouseholdSnap {
                status: agents.households[i].status,
                ordered: agents.households[i].ordered,
                warning_received: agents.households[i].warning_received,
                action: agents.behaviour_of(i).map(|(_, _, d)| d.action),
            })
            .collect();
        self.people = (0..agents.people.len())
            .map(|i| PersonSnap {
                status: agents.people[i].status,
                away: agents.people[i].away,
                action: agents.person_behaviour_of(i).map(|(_, _, d)| d.action),
            })
            .collect();
        self.travellers = agents.travellers.iter().map(|t| t.state).collect();
        self.units = crews
            .units
            .iter()
            .map(|u| UnitSnap {
                state: u.state,
                task: u.task,
                drops: u.drops,
            })
            .collect();
    }

    /// The scenario this run opened with — provenance for a history that
    /// otherwise has no way to say what fire and what seed it belongs to.
    pub fn record_run_started(&self, seed: u64, weather: fire::Weather) {
        self.log.record(
            0,
            Subject::Command,
            "run_started",
            serde_json::json!({
                "seed": seed,
                "wind_dir_deg": weather.wind_dir_deg,
                "wind_speed_kmh": weather.wind_speed_kmh,
                "moisture_pct": weather.moisture_pct,
            }),
        );
    }

    /// One-off, at t=0: which household is in which town, for whoever the
    /// OSM bake could place — a scenario with no address data at all records
    /// nothing here, rather than every household getting an identical empty
    /// event. Mirrors `record_run_started`/`record_ignition`: provenance for
    /// the run, not a thing that changed during it, so it is not diffed by
    /// [`History::observe`].
    pub fn record_locations(&self, population: &scenario::Population) {
        for h in &population.households {
            if h.locality.is_none() && h.address.is_none() {
                continue;
            }
            self.log.record(
                0,
                Subject::Household(h.id),
                "address",
                serde_json::json!({"locality": h.locality, "address": h.address}),
            );
        }
    }

    pub fn record_ignition(&self, time_s: i64, row: usize, col: usize, radius_m: f32) {
        self.log.record(
            time_s,
            Subject::Command,
            "ignition",
            serde_json::json!({"row": row, "col": col, "radius_m": radius_m}),
        );
    }

    pub fn record_weather(&self, time_s: i64, weather: fire::Weather) {
        self.log.record(
            time_s,
            Subject::Command,
            "weather_changed",
            serde_json::json!({
                "wind_dir_deg": weather.wind_dir_deg,
                "wind_speed_kmh": weather.wind_speed_kmh,
                "moisture_pct": weather.moisture_pct,
            }),
        );
    }

    /// Diff the live model against the last snapshot and record one event per
    /// change. Called once per [`Sim::advance`], so an event lands within one
    /// decision tick of whatever produced it.
    pub fn observe(&mut self, time_s: i64, agents: &abm::Abm, crews: &abm::Suppression) {
        self.observe_households(time_s, agents);
        self.observe_people(time_s, agents);
        self.observe_travellers(time_s, agents);
        self.observe_units(time_s, crews);
    }

    fn observe_households(&mut self, time_s: i64, agents: &abm::Abm) {
        let mut orders_issued = 0;
        let mut casualties = 0;
        for (i, h) in agents.households.iter().enumerate() {
            let Some(prev) = self.households.get_mut(i) else {
                continue;
            };
            if prev.status != h.status {
                self.log.record(
                    time_s,
                    Subject::Household(i),
                    "status",
                    serde_json::json!({
                        "from": crate::inspect::status_text(prev.status),
                        "to": crate::inspect::status_text(h.status),
                    }),
                );
                if h.status == Status::Casualty {
                    casualties += 1;
                }
                prev.status = h.status;
            }
            if !prev.ordered && h.ordered {
                self.log.record(
                    time_s,
                    Subject::Household(i),
                    "order_issued",
                    serde_json::json!({}),
                );
                orders_issued += 1;
                prev.ordered = true;
            }
            if !prev.warning_received && h.warning_received {
                self.log.record(
                    time_s,
                    Subject::Household(i),
                    "warning_received",
                    serde_json::json!({}),
                );
                prev.warning_received = true;
            }
            if let Some((subtype_id, subtype_name, d)) = agents.behaviour_of(i) {
                if prev.action != Some(d.action) {
                    self.log.record(
                        time_s,
                        Subject::Household(i),
                        "decision",
                        serde_json::json!({
                            "subtype": subtype_id,
                            "subtype_name": subtype_name,
                            "action": d.action,
                            "priority": d.priority,
                            "prep_scale": d.prep_scale,
                            "urgency": d.urgency,
                        }),
                    );
                    prev.action = Some(d.action);
                }
            }
        }
        if orders_issued > 0 {
            self.log.record(
                time_s,
                Subject::Command,
                "evacuation_order",
                serde_json::json!({"households": orders_issued}),
            );
        }
        if casualties > 0 {
            self.log.record(
                time_s,
                Subject::Command,
                "casualties",
                serde_json::json!({"households": casualties}),
            );
        }
    }

    fn observe_people(&mut self, time_s: i64, agents: &abm::Abm) {
        for (i, p) in agents.people.iter().enumerate() {
            let Some(prev) = self.people.get_mut(i) else {
                continue;
            };
            if prev.status != p.status {
                self.log.record(
                    time_s,
                    Subject::Person(i),
                    "status",
                    serde_json::json!({
                        "from": crate::inspect::status_text(prev.status),
                        "to": crate::inspect::status_text(p.status),
                    }),
                );
                prev.status = p.status;
            }
            if prev.away != p.away {
                self.log.record(
                    time_s,
                    Subject::Person(i),
                    "separation",
                    serde_json::json!({"away": p.away}),
                );
                prev.away = p.away;
            }
            if let Some((subtype_id, subtype_name, d)) = agents.person_behaviour_of(i) {
                if prev.action != Some(d.action) {
                    self.log.record(
                        time_s,
                        Subject::Person(i),
                        "decision",
                        serde_json::json!({
                            "subtype": subtype_id,
                            "subtype_name": subtype_name,
                            "action": d.action,
                            "priority": d.priority,
                            "urgency": d.urgency,
                        }),
                    );
                    prev.action = Some(d.action);
                }
            }
        }
    }

    fn observe_travellers(&mut self, time_s: i64, agents: &abm::Abm) {
        // Append-only, like `agents.travellers` itself (findings on restart):
        // a traveller's index never changes once assigned, so growing the
        // snapshot to match is all a new one needs.
        while self.travellers.len() < agents.travellers.len() {
            let idx = self.travellers.len();
            let t = &agents.travellers[idx];
            let detail = serde_json::json!({
                "household": t.household,
                "mode": if t.mode == abm::Mode::Car { "car" } else { "foot" },
                "members": t.members.len(),
            });
            self.log
                .record(time_s, Subject::Traveller(idx), "departed", detail.clone());
            if t.solo {
                if let Some(&pid) = t.members.first() {
                    self.log
                        .record(time_s, Subject::Person(pid), "departed", detail);
                }
            } else {
                self.log
                    .record(time_s, Subject::Household(t.household), "departed", detail);
            }
            self.travellers.push(t.state);
        }
        for (i, t) in agents.travellers.iter().enumerate() {
            if self.travellers[i] != t.state {
                self.log.record(
                    time_s,
                    Subject::Traveller(i),
                    "travel_state",
                    serde_json::json!({
                        "from": crate::inspect::travel_state_text(self.travellers[i]),
                        "to": crate::inspect::travel_state_text(t.state),
                    }),
                );
                self.travellers[i] = t.state;
            }
        }
    }

    fn observe_units(&mut self, time_s: i64, crews: &abm::Suppression) {
        for (i, u) in crews.units.iter().enumerate() {
            let Some(prev) = self.units.get_mut(i) else {
                continue;
            };
            if prev.state != u.state {
                self.log.record(
                    time_s,
                    Subject::Unit(i),
                    "unit_state",
                    serde_json::json!({
                        "from": prev.state.label(),
                        "to": u.state.label(),
                        "note": u.note,
                    }),
                );
                prev.state = u.state;
            }
            if prev.task != u.task {
                self.log
                    .record(time_s, Subject::Unit(i), "unit_task", task_detail(u.task));
                prev.task = u.task;
            }
            if prev.drops != u.drops {
                self.log.record(
                    time_s,
                    Subject::Unit(i),
                    "drop_completed",
                    serde_json::json!({"total_drops": u.drops}),
                );
                prev.drops = u.drops;
            }
        }
    }
}

/// The bare tag for a task, with no position — what `/agents/units` in
/// `crate::api` wants, where `task_detail` below carries more than a roster
/// listing needs.
pub(crate) fn task_kind(t: abm::Task) -> &'static str {
    match t {
        abm::Task::Hold => "hold",
        abm::Task::Return => "return",
        abm::Task::Attack { .. } => "attack",
        abm::Task::Drop { .. } => "drop",
        abm::Task::Line { .. } => "line",
    }
}

fn task_detail(t: abm::Task) -> serde_json::Value {
    match t {
        abm::Task::Hold => serde_json::json!({"task": "hold"}),
        abm::Task::Return => serde_json::json!({"task": "return"}),
        abm::Task::Attack { at } => serde_json::json!({"task": "attack", "x": at.x, "y": at.y}),
        abm::Task::Drop { at } => serde_json::json!({"task": "drop", "x": at.x, "y": at.y}),
        abm::Task::Line { from, to } => serde_json::json!({
            "task": "line",
            "from": {"x": from.x, "y": from.y},
            "to": {"x": to.x, "y": to.y},
        }),
    }
}

/// A short, human line for one event — the Inspector's history list and,
/// later, whatever renders a log for an LLM to read.
pub fn summarize(kind: &str, detail: &serde_json::Value) -> String {
    let s = |k: &str| {
        detail
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string()
    };
    let f = |k: &str| detail.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    match kind {
        "status" => format!("status: {} -> {}", s("from"), s("to")),
        "order_issued" => "evacuation order issued".to_string(),
        "warning_received" => "warning received".to_string(),
        "separation" => {
            if detail
                .get("away")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                "separated from household".to_string()
            } else {
                "reunited with household".to_string()
            }
        }
        "decision" => format!("decision: {} (priority {:.2})", s("action"), f("priority")),
        "departed" => format!(
            "departed on foot/by car ({} member(s))",
            detail.get("members").and_then(|v| v.as_u64()).unwrap_or(0)
        ),
        "travel_state" => format!("travel: {} -> {}", s("from"), s("to")),
        "unit_state" => {
            let note = s("note");
            if note != "?" && !note.is_empty() {
                format!("unit: {} -> {} ({note})", s("from"), s("to"))
            } else {
                format!("unit: {} -> {}", s("from"), s("to"))
            }
        }
        "unit_task" => format!("tasked: {}", s("task")),
        "drop_completed" => format!(
            "drop completed (#{})",
            detail
                .get("total_drops")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        ),
        "ignition" => format!("ignition placed, r={:.0} m", f("radius_m")),
        "run_started" => "run started".to_string(),
        "weather_changed" => format!(
            "weather: wind {:.0} km/h from {:.0}\u{00b0}, moisture {:.0}%",
            f("wind_speed_kmh"),
            f("wind_dir_deg"),
            f("moisture_pct")
        ),
        "evacuation_order" => format!(
            "evacuation order reached {} household(s)",
            detail
                .get("households")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        ),
        "casualties" => format!(
            "{} household(s) lost",
            detail
                .get("households")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        ),
        other => other.to_string(),
    }
}

/// [`Target`](crate::inspect::Target) to [`Subject`], for the Inspector.
pub fn subject_of(target: crate::inspect::Target) -> Subject {
    match target {
        crate::inspect::Target::Household(id) => Subject::Household(id),
        crate::inspect::Target::Person(id) => Subject::Person(id),
        crate::inspect::Target::Traveller(i) => Subject::Traveller(i),
        crate::inspect::Target::Unit(id) => Subject::Unit(id),
    }
}
