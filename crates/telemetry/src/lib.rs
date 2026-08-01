//! A per-run log of what happened to each simulated agent.
//!
//! This exists for two things: answering "why did this household do that"
//! from data rather than from whatever the model happens to be doing on the
//! current frame, and — later — an "interview an agent" feature that reads
//! this same table through an LLM. Both want the same shape: a timeline of
//! discrete, named events per agent, not a dense trace of every field on
//! every tick.
//!
//! **One run, one log, and it is not durable.** [`Recorder::new`] opens a
//! fresh **in-memory** SQLite database with no file on disk, so it costs
//! nothing to throw away and there is nothing to clean up. There is no
//! explicit "save" yet — a caller who wants one can add `VACUUM INTO` later —
//! so today the whole log is discarded the moment the process exits or a
//! caller replaces it with a new one. That is the same lifetime as every
//! other piece of run-only state in this project (the fire's event heap, the
//! household roster): a restart has to be a genuinely clean run, and a log
//! that survived one would misattribute the previous run's history to the
//! new one.
//!
//! **Native only.** `rusqlite`'s bundled SQLite is a C library compiled by
//! `cc`, which has no wasm32-unknown-unknown target. [`Recorder`] exists on
//! every target regardless — the type is unconditional, only its innards are
//! `cfg`-gated — so callers in `game` (which does build for wasm32) need no
//! `cfg` of their own; on web, every method is a no-op and every query comes
//! back empty.
//!
//! This crate knows nothing about `abm`, `fire`, or `scenario` on purpose:
//! [`Subject`] is spelled out by kind and a plain `usize` id rather than by
//! importing an agent type, so nothing here has to change when the model
//! does. The diffing that decides *when* to call [`Recorder::record`] lives
//! in `game`, next to the `Sim` it is watching.

use serde_json::Value;

/// Who an event is about. Deliberately just a kind and an id — this crate
/// does not know what an `abm::HouseholdAgent` is, only that something called
/// "household" has a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    Household(usize),
    Person(usize),
    Traveller(usize),
    Unit(usize),
    /// The incident as a whole: orders, ignitions, weather — nothing with an
    /// id of its own.
    Command,
}

impl Subject {
    fn kind(self) -> &'static str {
        match self {
            Subject::Household(_) => "household",
            Subject::Person(_) => "person",
            Subject::Traveller(_) => "traveller",
            Subject::Unit(_) => "unit",
            Subject::Command => "command",
        }
    }

    fn id(self) -> Option<i64> {
        match self {
            Subject::Household(i) | Subject::Person(i) | Subject::Traveller(i) | Subject::Unit(i) => {
                Some(i as i64)
            }
            Subject::Command => None,
        }
    }

    fn from_parts(kind: &str, id: Option<i64>) -> Option<Subject> {
        let i = || id.map(|v| v as usize);
        match kind {
            "household" => Some(Subject::Household(i()?)),
            "person" => Some(Subject::Person(i()?)),
            "traveller" => Some(Subject::Traveller(i()?)),
            "unit" => Some(Subject::Unit(i()?)),
            "command" => Some(Subject::Command),
            _ => None,
        }
    }
}

/// One recorded row, as read back.
#[derive(Debug, Clone)]
pub struct Event {
    pub sim_time_s: i64,
    pub subject: Subject,
    /// A short, stable tag (`"status"`, `"decision"`, `"ignition"`, ...).
    /// Free text rather than an enum: this crate does not know the set of
    /// event kinds any better than it knows `Subject`'s owner, and pinning
    /// one here would mean this crate changing every time `game` adds a new
    /// kind of thing worth recording.
    pub kind: String,
    pub detail: Value,
}

const SCHEMA: &str = "
CREATE TABLE events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    sim_time_s  INTEGER NOT NULL,
    subject_kind TEXT NOT NULL,
    subject_id  INTEGER,
    kind        TEXT NOT NULL,
    detail      TEXT NOT NULL
);
CREATE INDEX events_subject ON events(subject_kind, subject_id);
CREATE INDEX events_time ON events(sim_time_s);
";

pub struct Recorder {
    // `rusqlite::Connection` uses interior `RefCell`s of its own (statement
    // cache) and so is `Send` but not `Sync`; `Sim` is a Bevy `Resource` and
    // Bevy resources have to be both, even for a single-threaded game. The
    // mutex is never contended -- everything here runs on the main thread --
    // it exists only to make the type sound to share.
    #[cfg(not(target_arch = "wasm32"))]
    conn: std::sync::Mutex<rusqlite::Connection>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Recorder {
    /// A fresh, empty, in-memory log.
    pub fn new() -> Recorder {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory sqlite connection");
        conn.execute_batch(SCHEMA).expect("create event log schema");
        Recorder { conn: std::sync::Mutex::new(conn) }
    }

    /// Append one event. Best-effort: a log write is never allowed to be the
    /// thing that takes the simulation down, so a failure here is swallowed
    /// rather than propagated.
    pub fn record(&self, sim_time_s: i64, subject: Subject, kind: &str, detail: Value) {
        let Ok(conn) = self.conn.lock() else { return };
        let _ = conn.execute(
            "INSERT INTO events (sim_time_s, subject_kind, subject_id, kind, detail) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![sim_time_s, subject.kind(), subject.id(), kind, detail.to_string()],
        );
    }

    /// Every event about one subject, oldest first.
    pub fn events_for(&self, subject: Subject) -> Vec<Event> {
        let sql = "SELECT sim_time_s, subject_kind, subject_id, kind, detail FROM events \
                    WHERE subject_kind = ?1 AND subject_id IS ?2 ORDER BY id";
        self.query(sql, rusqlite::params![subject.kind(), subject.id()])
    }

    /// The most recent events across every subject, newest first — an
    /// incident-wide activity feed.
    pub fn recent(&self, limit: usize) -> Vec<Event> {
        let sql = "SELECT sim_time_s, subject_kind, subject_id, kind, detail FROM events \
                    ORDER BY id DESC LIMIT ?1";
        self.query(sql, rusqlite::params![limit as i64])
    }

    pub fn len(&self) -> usize {
        let Ok(conn) = self.conn.lock() else { return 0 };
        conn.query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0)).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn query(&self, sql: &str, params: impl rusqlite::Params) -> Vec<Event> {
        let Ok(conn) = self.conn.lock() else { return Vec::new() };
        let mut stmt = match conn.prepare(sql) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map(params, |r| {
            let kind: String = r.get(1)?;
            let id: Option<i64> = r.get(2)?;
            let detail_text: String = r.get(4)?;
            Ok((r.get::<_, i64>(0)?, kind, id, r.get::<_, String>(3)?, detail_text))
        });
        let Ok(rows) = rows else { return Vec::new() };
        rows.filter_map(|row| {
            let (sim_time_s, subject_kind, subject_id, kind, detail_text) = row.ok()?;
            let subject = Subject::from_parts(&subject_kind, subject_id)?;
            let detail = serde_json::from_str(&detail_text).unwrap_or(Value::Null);
            Some(Event { sim_time_s, subject, kind, detail })
        })
        .collect()
    }
}

#[cfg(target_arch = "wasm32")]
impl Recorder {
    pub fn new() -> Recorder {
        Recorder {}
    }

    pub fn record(&self, _sim_time_s: i64, _subject: Subject, _kind: &str, _detail: Value) {}

    pub fn events_for(&self, _subject: Subject) -> Vec<Event> {
        Vec::new()
    }

    pub fn recent(&self, _limit: usize) -> Vec<Event> {
        Vec::new()
    }

    pub fn len(&self) -> usize {
        0
    }

    pub fn is_empty(&self) -> bool {
        true
    }
}

impl Default for Recorder {
    fn default() -> Self {
        Recorder::new()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn records_and_reads_back_in_order() {
        let log = Recorder::new();
        log.record(0, Subject::Household(3), "status", serde_json::json!({"to": "warned"}));
        log.record(30, Subject::Household(3), "status", serde_json::json!({"to": "preparing"}));
        log.record(10, Subject::Household(7), "status", serde_json::json!({"to": "warned"}));

        let events = log.events_for(Subject::Household(3));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sim_time_s, 0);
        assert_eq!(events[1].sim_time_s, 30);
        assert_eq!(events[0].detail["to"], "warned");

        assert_eq!(log.events_for(Subject::Household(7)).len(), 1);
        assert!(log.events_for(Subject::Household(99)).is_empty());
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn command_subject_has_no_id() {
        let log = Recorder::new();
        log.record(0, Subject::Command, "run_started", serde_json::json!({"seed": 42}));
        let events = log.events_for(Subject::Command);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].subject, Subject::Command);
    }

    #[test]
    fn recent_is_newest_first_across_subjects() {
        let log = Recorder::new();
        log.record(0, Subject::Unit(0), "unit_state", serde_json::json!({}));
        log.record(5, Subject::Household(1), "status", serde_json::json!({}));
        let recent = log.recent(10);
        assert_eq!(recent[0].subject, Subject::Household(1));
        assert_eq!(recent[1].subject, Subject::Unit(0));
    }
}
