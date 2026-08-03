//! Talking to one simulated agent, through an LLM.
//!
//! The Inspector answers "what is this household doing" in numbers. This
//! answers it in the household's own words — and the two are the same data.
//! Everything the model sees is assembled here, in [`dossier`], out of exactly
//! three sources: the baked traits the population gave this agent, what they
//! can perceive from where they are standing right now, and their own row of
//! the run's event log. Nothing else is sent, and the `chat` crate's
//! [`chat::Dossier`] has no field an incident-wide fact would fit in.
//!
//! **The translation is the work.** The event log stores `status: normal ->
//! warned`, which is exactly right for a log and useless as a memory: handed
//! that, a model repeats the jargon back at you in quotation marks. Every kind
//! of event has an in-character reading here ([`recollection`]), and every
//! trait a phrase ([`facts_for`]) — "you always said you would go at the first
//! sign" rather than `intent = leave_early`. An agent that says "my risk
//! perception is 0.31" has told you nothing the panel above it did not, which
//! is the whole failure this module is trying to avoid.
//!
//! **Starting an interview pauses the incident.** The agent is answering as of
//! one instant; letting the fire run underneath the conversation would produce
//! answers about a situation that had already changed by the time they were
//! read. It is the same reason applying a behaviour restarts rather than hot
//! swaps.
//!
//! **A transcript lives exactly as long as the run does.** It is stored in the
//! run's own event log (`telemetry`), beside the events every answer was drawn
//! from, and a restart discards both together — see
//! `telemetry::Recorder::record_message`. What outlives a run is the *persona*,
//! held in [`Interview::personas`] for the life of the process: household 42's
//! traits are baked and identical in every run, so the person built on them
//! should be too, and regenerating one on every restart would be a paid call
//! to reinvent somebody the model already knew.
//!
//! Native requests run on a worker thread blocking on a socket. Browser
//! requests use async `fetch`; both report through the same polled channel so
//! rendering never waits on a model response.

use std::collections::HashMap;
use std::sync::mpsc;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use chat::{Client, Dossier, Fact, LlmConfig, Message, Persona, SubjectKind, SubjectRef};
use scenario::population::{Intent, Status, WarningChannel};
use scenario::Pos;

use crate::inspect::{Selected, Target};
use crate::sim::Sim;

/// What a worker thread is doing, so the reply can be routed when it lands.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Job {
    /// Inventing the person, before the first question.
    Persona,
    /// Answering a question.
    Reply,
    /// The settings dialog's "Test" button.
    Test,
    Models,
}

/// One thing a worker thread has to say.
enum Note {
    Delta(String),
    Done(String),
    Failed(String),
    Models(Result<Vec<String>, String>),
}

struct Pending {
    job: Job,
    subject: SubjectRef,
    /// `mpsc::Receiver` is `Send` but not `Sync`, and a Bevy resource has to be
    /// both — the same mutex `api::ApiChannel` wraps its receiver in, for the
    /// same reason and with the same non-contention: only the main thread ever
    /// locks it.
    rx: std::sync::Mutex<mpsc::Receiver<Note>>,
    /// The reply so far, for the streaming bubble.
    partial: String,
}

#[derive(Resource)]
pub struct Interview {
    pub open: bool,
    pub settings_open: bool,
    /// Who is being interviewed. `None` before anything has been selected.
    pub subject: Option<SubjectRef>,
    pub input: String,
    pub config: LlmConfig,
    pub models: Vec<String>,
    /// Text entered inside either provider's searchable model picker.
    pub model_search: String,
    pub model_status: String,
    /// A line under the transcript: the last failure, or what is happening.
    pub status: String,
    /// The settings dialog's own result line, kept apart from `status` so a
    /// failed test does not overwrite an interview's error.
    pub settings_status: String,
    /// Personas by scenario and agent, for the life of the process. See the
    /// module docs for why this is the one thing that outlives a run.
    personas: HashMap<(String, SubjectRef), Persona>,
    pending: Option<Pending>,
    /// `SPOTORNO_INTERVIEW=selftest` — see [`selftest`].
    selftest: bool,
    /// Set when the panel opens, cleared by the first frame that pauses the
    /// incident.
    ///
    /// A one-shot rather than a rule the window enforces every frame: pausing
    /// on open is the point (the agent answers as of one instant), but a window
    /// that re-paused continuously would swallow the play button with no
    /// explanation — the same invisible override as two systems reading the
    /// same key (finding 25). Play is allowed; the panel says so, and says the
    /// clock has moved on since the last answer.
    pause_on_open: bool,
}

impl Default for Interview {
    fn default() -> Self {
        let (config, error) = LlmConfig::load_reported();
        Interview {
            open: false,
            settings_open: false,
            subject: None,
            input: String::new(),
            config,
            models: Vec::new(),
            model_search: String::new(),
            model_status: String::new(),
            status: error
                .map(|e| format!("LLM settings: {e}"))
                .unwrap_or_default(),
            settings_status: String::new(),
            personas: HashMap::new(),
            pending: None,
            selftest: false,
            pause_on_open: false,
        }
    }
}

impl Interview {
    /// Open the panel on one agent, and pause the incident.
    pub fn open_for(&mut self, subject: SubjectRef) {
        if self.subject != Some(subject) {
            // A half-typed question belongs to the agent it was being asked
            // of, and a streaming reply to the agent it came from.
            self.input.clear();
            self.pending = None;
            self.status.clear();
        }
        if !self.open {
            self.pause_on_open = true;
        }
        self.subject = Some(subject);
        self.open = true;
    }

    pub fn persona_for(&self, scenario: &str, subject: SubjectRef) -> Option<&Persona> {
        self.personas.get(&(scenario.to_string(), subject))
    }

    fn busy(&self) -> bool {
        self.pending.is_some()
    }
}

/// [`Target`] to the three kinds of agent that can be interviewed.
///
/// A traveller is a vehicle rather than a person, so it resolves to whoever is
/// in it: the household it belongs to, or — for someone who left on their own —
/// that person. Clicking a car and asking it questions should reach the people
/// inside, not fail.
pub fn subject_of(sim: &Sim, target: Target) -> Option<SubjectRef> {
    match target {
        Target::Household(id) => Some(SubjectRef::new(SubjectKind::Household, id as i64)),
        Target::Person(id) => Some(SubjectRef::new(SubjectKind::Person, id as i64)),
        Target::Unit(id) => Some(SubjectRef::new(SubjectKind::Unit, id as i64)),
        Target::Traveller(i) => {
            let t = sim.agents.travellers.get(i)?;
            if t.solo {
                t.members
                    .first()
                    .map(|&p| SubjectRef::new(SubjectKind::Person, p as i64))
            } else {
                Some(SubjectRef::new(SubjectKind::Household, t.household as i64))
            }
        }
    }
}

/// The [`telemetry::Subject`] a transcript is filed under.
fn telemetry_subject(subject: SubjectRef) -> telemetry::Subject {
    let id = subject.id as usize;
    match subject.kind {
        SubjectKind::Household => telemetry::Subject::Household(id),
        SubjectKind::Person => telemetry::Subject::Person(id),
        SubjectKind::Unit => telemetry::Subject::Unit(id),
    }
}

/// A short identification for the window title and the menu.
pub fn label(sim: &Sim, subject: SubjectRef) -> String {
    match subject.kind {
        SubjectKind::Household => format!("Household #{}", subject.id),
        SubjectKind::Person => format!("Person #{}", subject.id),
        SubjectKind::Unit => sim
            .crews
            .units
            .get(subject.id as usize)
            .map(|u| u.callsign.clone())
            .unwrap_or_else(|| format!("Unit #{}", subject.id)),
    }
}

// --- what the agent knows -------------------------------------------------

/// Everything this agent could possibly know, and nothing else.
pub fn dossier(sim: &Sim, subject: SubjectRef) -> Option<Dossier> {
    let (facts, perceptions, callsign) = match subject.kind {
        SubjectKind::Household => {
            let (f, p) = household_dossier(sim, subject.id as usize)?;
            (f, p, None)
        }
        SubjectKind::Person => {
            let (f, p) = person_dossier(sim, subject.id as usize)?;
            (f, p, None)
        }
        SubjectKind::Unit => {
            let (f, p, cs) = unit_dossier(sim, subject.id as usize)?;
            (f, p, Some(cs))
        }
    };

    let timeline = sim
        .history
        .log
        .events_for(telemetry_subject(subject))
        .iter()
        .filter_map(|e| {
            recollection(subject.kind, &e.kind, &e.detail).map(|line| chat::TimelineEntry {
                sim_time_s: e.sim_time_s,
                line,
            })
        })
        .collect();

    let (locality, address) = match subject.kind {
        SubjectKind::Household => sim
            .scenario
            .population
            .households
            .get(subject.id as usize)
            .map(|h| (h.locality.clone(), h.address.clone()))
            .unwrap_or_default(),
        SubjectKind::Person => sim
            .agents
            .people
            .get(subject.id as usize)
            .and_then(|p| sim.scenario.population.households.get(p.household))
            .map(|h| (h.locality.clone(), h.address.clone()))
            .unwrap_or_default(),
        SubjectKind::Unit => (None, None),
    };

    Some(Dossier {
        kind: subject.kind,
        id: subject.id,
        callsign,
        sim_time_s: sim.time_s(),
        clock: sim.clock(),
        facts,
        perceptions,
        timeline,
        nationality: sim.scenario.metadata.nationality.clone(),
        region: sim.scenario.metadata.region.clone(),
        localities: sim.scenario.metadata.localities.clone(),
        locality,
        address,
    })
}

fn household_dossier(sim: &Sim, id: usize) -> Option<(Vec<Fact>, Vec<String>)> {
    let h = sim.agents.households.get(id)?;
    let baked = sim.scenario.population.households.get(id);
    let mut facts = Vec::new();

    let ages: Vec<String> = h
        .members
        .iter()
        .filter_map(|&p| sim.agents.people.get(p))
        .map(|p| p.age.to_string())
        .collect();
    facts.push(Fact::new(
        "Your household",
        format!(
            "{} of you{}{}",
            h.members.len(),
            if ages.is_empty() {
                String::new()
            } else {
                format!(" (ages {})", ages.join(", "))
            },
            match h.vehicles {
                0 => ", and no car".to_string(),
                1 => ", one car".to_string(),
                n => format!(", {n} cars"),
            }
        ),
    ));

    if h.members
        .iter()
        .filter_map(|&p| sim.agents.people.get(p))
        .any(|p| p.needs_assistance)
    {
        facts.push(Fact::new(
            "Someone needs help to move",
            "one of your household cannot get out on their own",
        ));
    }

    if let Some(b) = baked {
        if let Some(locality) = &b.locality {
            facts.push(Fact::new(
                "Home",
                match &b.address {
                    Some(addr) => format!("{addr}, {locality}"),
                    None => locality.clone(),
                },
            ));
        }
        if b.has_pets_livestock {
            facts.push(Fact::new("Animals", "you have animals to think about"));
        }
        if b.prior_fire_experience {
            facts.push(Fact::new(
                "Before",
                "you have been through a fire near here before",
            ));
        }
        facts.push(Fact::new(
            "Where you live",
            if b.dist_to_fuel_m < 30.0 {
                "the macchia comes right up to the house"
            } else if b.dist_to_fuel_m < 120.0 {
                "there is scrub and pine a short walk from the house"
            } else {
                "you are in the built-up part of town, away from the brush"
            },
        ));
    }

    facts.push(Fact::new(
        "What you always said you would do",
        match h.intent {
            Intent::LeaveEarly => {
                "go at the first credible warning, without waiting to be told twice"
            }
            Intent::WaitAndSee => "wait and see how it develops before doing anything drastic",
            Intent::StayDefend => "stay and defend the property",
        },
    ));
    facts.push(Fact::new(
        "How you take a warning",
        band(
            h.risk_perception,
            "you think this kind of thing is usually overstated",
            "you take it seriously enough, without panicking",
            "you frighten easily and you know it",
        ),
    ));
    facts.push(Fact::new(
        "What you make of the authorities",
        band(
            h.trust_authority,
            "you do not much trust what officials tell you",
            "you would take an official warning at face value",
            "if the authorities say go, you go",
        ),
    ));
    facts.push(Fact::new(
        "How you would hear",
        match h.channel {
            WarningChannel::MobileAlert => "an alert on your phone, straight away",
            WarningChannel::Neighbour => "a neighbour knocking, once it got round",
            WarningChannel::Siren => "the siren in the town, if the wind carried it",
            WarningChannel::SelfObserved => "your own eyes and nose, and nothing else",
            WarningChannel::None => "nobody would tell you; you would have to notice yourself",
        },
    ));
    facts.push(Fact::new(
        "Before you could actually leave",
        format!(
            "about {:.0} minutes of things you would have to do first{}",
            h.prep_time_min,
            if h.prep_time_min > 30.0 {
                " — you are slow to get moving"
            } else {
                ""
            }
        ),
    ));
    facts.push(Fact::new(
        "Around the house",
        band(
            h.defensible_space,
            "the garden is overgrown right up to the walls",
            "the ground round the house is reasonably clear",
            "you keep everything cut back and cleared",
        ),
    ));
    facts.push(Fact::new("Right now you are", situation(h.status)));
    if h.status == Status::Preparing && h.prep_remaining_s > 0.0 {
        facts.push(Fact::new(
            "Still to do",
            format!(
                "about {:.0} more minutes of getting ready",
                h.prep_remaining_s / 60.0
            ),
        ));
    }
    if h.ordered && !h.warning_received {
        facts.push(Fact::new(
            "The order",
            "an evacuation order has gone out for your area, but nobody has reached you yet — \
             you do not know about it",
        ));
    }

    let mut perceptions = senses(sim, h.home);
    if let Some(traveller) = h.traveller.and_then(|t| sim.agents.travellers.get(t)) {
        perceptions = senses(sim, traveller.pos);
        perceptions.push(match traveller.mode {
            abm::Mode::Car => "You are in the car, on the road out.".to_string(),
            abm::Mode::Foot => "You are on foot, moving.".to_string(),
        });
        if traveller.state == abm::TravelState::Cutoff {
            perceptions.push("There is no way through: you are cut off.".to_string());
        }
    }
    let exposure = sim.fire.exposure().get(id);
    if exposure.alight {
        perceptions.push("Your house is alight.".to_string());
    } else if exposure.ember > 0.15 {
        perceptions.push("Embers are landing around the house.".to_string());
    } else if exposure.radiant > 0.2 {
        perceptions.push("You can feel the heat of it on the walls of the house.".to_string());
    }
    if h.cue > 0.6 {
        perceptions.push("You are frightened.".to_string());
    }

    Some((facts, perceptions))
}

fn person_dossier(sim: &Sim, id: usize) -> Option<(Vec<Fact>, Vec<String>)> {
    let p = sim.agents.people.get(id)?;
    let mut facts = vec![Fact::new("You", format!("{} years old", p.age))];

    if let Some(locality) = sim
        .scenario
        .population
        .households
        .get(p.household)
        .and_then(|h| h.locality.as_ref())
    {
        facts.push(Fact::new("Home", format!("your family lives in {locality}")));
    }

    if p.needs_assistance {
        facts.push(Fact::new("Moving", "you cannot get far on your own"));
    } else if p.walk_speed < 1.0 {
        facts.push(Fact::new("Moving", "you are slow on your feet"));
    }

    if p.away {
        facts.push(Fact::new(
            "Where you are",
            "you are out, away from home, and not with your family",
        ));
        if let Some(h) = sim.agents.households.get(p.household) {
            let others = h.members.len().saturating_sub(1);
            facts.push(Fact::new(
                "Your family",
                match others {
                    0 => "you live alone; there is nobody at the house".to_string(),
                    1 => "there is one other person at your house".to_string(),
                    n => format!("there are {n} others at your house"),
                },
            ));
            facts.push(Fact::new("They are", situation(h.status)));
        }
    } else {
        facts.push(Fact::new("Where you are", "you are with your household"));
    }
    facts.push(Fact::new("Right now you are", situation(p.status)));

    let mut perceptions = senses(sim, p.pos);
    if p.cue > 0.6 {
        perceptions.push("You are frightened.".to_string());
    }
    Some((facts, perceptions))
}

fn unit_dossier(sim: &Sim, id: usize) -> Option<(Vec<Fact>, Vec<String>, String)> {
    let u = sim.crews.units.get(id)?;
    let mut facts = vec![
        Fact::new(
            "Your crew",
            match u.kind {
                abm::UnitKind::HandCrew => "a hand crew, on foot, cutting line with tools",
                abm::UnitKind::Engine => "a water tender — an autobotte, roads only, with a hose",
                abm::UnitKind::AirTanker => "an air tanker, working from the air",
            },
        ),
        Fact::new("Right now", u.state.label()),
    ];
    // Where they are, coarsely — asked first, every time, and without it the
    // model invents a depot in a town the roster has never heard of.
    let from_base = ((u.pos.x - u.base.x).powi(2) + (u.pos.y - u.base.y).powi(2)).sqrt();
    facts.push(Fact::new(
        "Where you are",
        if from_base < 60.0 {
            "at your staging point, on the edge of the town"
        } else if sim.agents.fire_distance(u.pos) < 300.0 {
            "on the fire ground"
        } else {
            "out on the road, somewhere between the town and the fire"
        },
    ));
    facts.push(Fact::new(
        "Your orders",
        match u.task {
            abm::Task::Hold => "stand by where you are".to_string(),
            abm::Task::Return => "return to staging".to_string(),
            abm::Task::Attack { .. } => "work the fire edge where you were sent".to_string(),
            abm::Task::Drop { .. } => "put a load on the point you were given".to_string(),
            abm::Task::Line { .. } => "cut a break on the alignment you were given".to_string(),
        },
    ));
    if u.tank_l > 0.0 {
        let frac = u.water_l / u.tank_l;
        facts.push(Fact::new(
            "Water",
            if frac <= 0.02 {
                "your tank is dry".to_string()
            } else {
                format!("about {:.0}% of your tank left", frac * 100.0)
            },
        ));
    }
    if u.line_cut_m > 0.0 {
        facts.push(Fact::new(
            "Line cut so far",
            format!("{:.0} metres", u.line_cut_m),
        ));
    }
    if u.drops > 0 {
        facts.push(Fact::new("Drops made", format!("{}", u.drops)));
    }
    if !u.note.is_empty() {
        facts.push(Fact::new("What is stopping you", u.note));
    }

    let mut perceptions = senses(sim, u.pos);
    if u.heat_s > 60.0 {
        perceptions.push("You have been taking heat for a while now.".to_string());
    }
    Some((facts, perceptions, u.callsign.clone()))
}

/// What anyone standing at `p` can see, hear and feel.
///
/// The one place perception is derived, so a household, a person and a crew
/// all read the same fire the same way. Distances are the coarse field the
/// agents themselves use (`Abm::fire_distance`), not a true nearest-cell
/// measurement: what an agent knows about how far away a fire is, is roughly
/// how far away it looks.
fn senses(sim: &Sim, p: Pos) -> Vec<String> {
    let mut out = Vec::new();

    let d = sim.agents.fire_distance(p);
    let bearing = nearest_fire(sim, p).map(|f| {
        let up = sim.scenario.terrain.height_at(f) > sim.scenario.terrain.height_at(p) + 20.0;
        format!(
            "{}{}",
            compass(f.x - p.x, f.y - p.y),
            if up { ", up the slope" } else { "" }
        )
    });
    let where_ = bearing.map(|b| format!(" to the {b}")).unwrap_or_default();

    if d < 100.0 {
        out.push(format!(
            "The fire is right there{where_} — you can hear it."
        ));
    } else if d < 400.0 {
        out.push(format!(
            "The fire is a few hundred metres away{where_}. You can see flame."
        ));
    } else if d < 1200.0 {
        out.push(format!(
            "There is a column of smoke perhaps a kilometre off{where_}."
        ));
    } else if d < 4000.0 {
        out.push(format!("You can see smoke in the distance{where_}."));
    } else {
        out.push("You cannot see any fire from here.".to_string());
    }

    let threat = sim.fire.threat().at(p);
    if threat > 0.6 {
        out.push("The air here is hot and hard to breathe; ash is falling.".to_string());
    } else if threat > 0.3 {
        out.push("There is smoke on the ground here and it stings.".to_string());
    } else if threat > 0.1 {
        out.push("You can smell it strongly.".to_string());
    }

    let w = sim.fire.weather();
    if w.wind_speed_kmh > 25.0 {
        out.push(format!(
            "A hard wind, {:.0} km/h, blowing from the {}.",
            w.wind_speed_kmh,
            compass_from_bearing(w.wind_dir_deg as f32)
        ));
    } else if w.wind_speed_kmh > 10.0 {
        out.push(format!(
            "A steady wind from the {}.",
            compass_from_bearing(w.wind_dir_deg as f32)
        ));
    }
    out
}

/// Centre of the burning cell nearest `p`, for a direction to point in.
fn nearest_fire(sim: &Sim, p: Pos) -> Option<Pos> {
    sim.fire
        .active_cells()
        .iter()
        .map(|c| sim.scenario.world.centre_of(*c))
        .min_by(|a, b| {
            let d = |q: &Pos| (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
            d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Compass point of a world-frame offset (+x east, +y north).
fn compass(dx: f32, dy: f32) -> &'static str {
    compass_from_bearing(dx.atan2(dy).to_degrees())
}

/// Compass point of a bearing in degrees clockwise from north. Used both for
/// "which way is the fire" and for the wind, which is quoted as the direction
/// it blows *from* — the meteorological convention the whole model uses.
fn compass_from_bearing(deg: f32) -> &'static str {
    const POINTS: [&str; 8] = [
        "north",
        "north-east",
        "east",
        "south-east",
        "south",
        "south-west",
        "west",
        "north-west",
    ];
    let i = ((deg.rem_euclid(360.0) + 22.5) / 45.0) as usize % 8;
    POINTS[i]
}

fn band(v: f32, low: &str, mid: &str, high: &str) -> String {
    if v < 0.35 {
        low.to_string()
    } else if v < 0.7 {
        mid.to_string()
    } else {
        high.to_string()
    }
}

/// A status, said the way the person in it would say it.
fn situation(s: Status) -> &'static str {
    match s {
        Status::Normal => "going about your day, as far as you are concerned",
        Status::Warned => "aware something is happening, and not yet doing anything about it",
        Status::Preparing => "getting ready to go",
        Status::Evacuating => "on your way out",
        Status::Evacuated => "out, somewhere safe",
        Status::Defending => "staying, and trying to protect the house",
        Status::Trapped => "cut off — the way out is gone and you are sheltering where you are",
        Status::Casualty => "badly hurt; it went wrong",
    }
}

/// One logged event as the agent would remember it, or `None` for the ones
/// that are the model talking to itself.
///
/// The translation the module docs are about. `status: normal -> warned` is
/// exactly right in a log and useless as a memory — a model handed it quotes
/// the jargon straight back.
fn recollection(kind: SubjectKind, event: &str, detail: &serde_json::Value) -> Option<String> {
    let to = detail.get("to").and_then(|v| v.as_str()).unwrap_or("");
    let n = |k: &str| detail.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
    Some(match event {
        "status" => match to {
            "warned" => "you realised something was going on".to_string(),
            "preparing" => "you started getting ready to leave".to_string(),
            "evacuating" => "you left".to_string(),
            "evacuated / safe" => "you got clear".to_string(),
            "defending" => "you decided to stay and hold the house".to_string(),
            "trapped" => "the way out was cut and you had to shelter".to_string(),
            "casualty" => "it went badly wrong for you".to_string(),
            _ => return None,
        },
        "warning_received" => "the evacuation order reached you".to_string(),
        // Deliberately not recorded as a memory: `order_issued` is the moment
        // the *commander* included this household in an order, which the
        // household knows nothing about until `warning_received`. Putting it in
        // the timeline is the exact god-view leak this module exists to avoid.
        "order_issued" => return None,
        "separation" => {
            if detail
                .get("away")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                "you found yourself away from the others".to_string()
            } else {
                "you got back to your family".to_string()
            }
        }
        "departed" => {
            let mode = detail
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("foot");
            match (mode, n("members")) {
                ("car", 1) => "you got in the car and went".to_string(),
                ("car", m) => format!("all {m} of you got in the car and went"),
                (_, 1) => "you set off on foot".to_string(),
                (_, m) => format!("the {m} of you set off on foot"),
            }
        }
        "travel_state" => match to {
            "abandoned the car" => "you had to leave the car and go on foot".to_string(),
            "blocked" => "the road ahead was blocked".to_string(),
            "safe" => "you reached somewhere safe".to_string(),
            _ => return None,
        },
        "unit_state" if kind == SubjectKind::Unit => {
            let note = detail.get("note").and_then(|v| v.as_str()).unwrap_or("");
            let base = match to {
                "responding" => "you were sent out",
                "working" => "you got to work",
                "refilling" => "you broke off to take on water",
                "withdrawing" => "you pulled back",
                "staged" => "you were back at staging",
                "inbound" => "you were called in and started the run",
                _ => return None,
            };
            if note.is_empty() {
                base.to_string()
            } else {
                format!("{base} — {note}")
            }
        }
        "unit_task" if kind == SubjectKind::Unit => {
            match detail.get("task").and_then(|v| v.as_str()) {
                Some("attack") => "you were ordered onto the fire edge".to_string(),
                Some("line") => "you were ordered to cut a line".to_string(),
                Some("drop") => "you were given a drop to make".to_string(),
                Some("return") => "you were ordered back to staging".to_string(),
                Some("hold") => "you were told to stand by".to_string(),
                _ => return None,
            }
        }
        "drop_completed" if kind == SubjectKind::Unit => "you put a load down".to_string(),
        // A decision is the authored graph's own answer, not something that
        // happened to the agent. It reads as a memory only when a behaviour is
        // loaded, which is why it is phrased as making up one's mind.
        "decision" => format!(
            "you made up your mind: {}",
            detail.get("action").and_then(|v| v.as_str()).unwrap_or("")
        ),
        _ => return None,
    })
}

// --- talking to the model -------------------------------------------------

/// Spawn a worker for one request. The main thread never blocks on a socket —
/// see the module docs.
#[cfg(not(target_arch = "wasm32"))]
fn spawn(config: LlmConfig, messages: Vec<Message>) -> mpsc::Receiver<Note> {
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("spotorno-interview".into())
        .spawn(move || {
            let client = Client::new(config);
            let deltas = tx.clone();
            let mut on_delta = |d: &str| {
                let _ = deltas.send(Note::Delta(d.to_string()));
            };
            let note = match client.complete(&messages, &mut on_delta) {
                Ok(text) => Note::Done(text),
                Err(e) => Note::Failed(format!("{e:#}")),
            };
            let _ = tx.send(note);
        });
    if let Err(e) = spawned {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(Note::Failed(format!(
            "could not start a worker thread: {e}"
        )));
        return rx;
    }
    rx
}

#[cfg(target_arch = "wasm32")]
fn spawn(config: LlmConfig, messages: Vec<Message>) -> mpsc::Receiver<Note> {
    let (tx, rx) = mpsc::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let client = Client::new(config);
        let deltas = tx.clone();
        let mut on_delta = |d: &str| {
            let _ = deltas.send(Note::Delta(d.to_string()));
        };
        let note = match client.complete(&messages, &mut on_delta).await {
            Ok(text) => Note::Done(text),
            Err(e) => Note::Failed(format!("{e:#}")),
        };
        let _ = tx.send(note);
    });
    rx
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_models(config: LlmConfig) -> mpsc::Receiver<Note> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = chat::fetch_models(&config).map_err(|e| format!("{e:#}"));
        let _ = tx.send(Note::Models(result));
    });
    rx
}

#[cfg(target_arch = "wasm32")]
fn spawn_models(config: LlmConfig) -> mpsc::Receiver<Note> {
    let (tx, rx) = mpsc::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let result = chat::fetch_models(&config).await.map_err(|e| format!("{e:#}"));
        let _ = tx.send(Note::Models(result));
    });
    rx
}

/// The messages one question turns into: the system prompt rebuilt against the
/// current clock, the transcript so far, and the question.
///
/// Rebuilt rather than cached because the interview can be paused, the fire
/// run on, and the panel reopened — and then the agent is answering from a
/// different afternoon than the one the first system prompt described.
fn conversation(sim: &Sim, persona: &Persona, subject: SubjectRef, question: &str) -> Vec<Message> {
    let mut messages = Vec::new();
    if let Some(d) = dossier(sim, subject) {
        messages.push(Message::system(chat::prompt::system(persona, &d)));
    }
    for m in sim.history.log.messages_for(telemetry_subject(subject)) {
        match m.role.as_str() {
            "user" => messages.push(Message::user(m.content)),
            "assistant" => messages.push(Message::assistant(m.content)),
            _ => {}
        }
    }
    messages.push(Message::user(question));
    messages
}

/// Drain whatever the worker has produced, and file a finished reply.
pub fn poll(mut interview: ResMut<Interview>, sim: ResMut<Sim>) {
    let Some(pending) = interview.pending.as_mut() else {
        return;
    };
    let mut finished: Option<(Job, SubjectRef, Result<String, String>)> = None;
    let mut models_result = None;
    let mut deltas = String::new();
    {
        let Ok(rx) = pending.rx.lock() else { return };
        loop {
            match rx.try_recv() {
                Ok(Note::Delta(d)) => deltas.push_str(&d),
                Ok(Note::Done(text)) => {
                    finished = Some((pending.job, pending.subject, Ok(text)));
                    break;
                }
                Ok(Note::Failed(e)) => {
                    finished = Some((pending.job, pending.subject, Err(e)));
                    break;
                }
                Ok(Note::Models(result)) => {
                    models_result = Some(result);
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                // The worker vanished without answering — treat as a failure
                // rather than leaving the panel spinning forever.
                Err(mpsc::TryRecvError::Disconnected) => {
                    finished = Some((
                        pending.job,
                        pending.subject,
                        Err("the request ended without a reply".to_string()),
                    ));
                    break;
                }
            }
        }
    }
    if let Some(result) = models_result {
        interview.pending = None;
        match result {
            Ok(models) => {
                interview.models = models;
                interview.model_status = format!("{} models available", interview.models.len());
            }
            Err(e) => interview.model_status = format!("✖ {e}"),
        }
        return;
    }
    pending.partial.push_str(&deltas);

    let Some((job, subject, result)) = finished else {
        return;
    };
    interview.pending = None;
    let scenario = sim.scenario.id.clone();
    match (job, result) {
        (Job::Persona, Ok(text)) => match chat::persona::parse(&text, &interview.config.model()) {
            Ok(p) => {
                interview.personas.insert((scenario, subject), p);
                interview.status.clear();
            }
            Err(e) => {
                interview.status = format!("could not build a persona: {e:#}");
                let anonymous = dossier(&sim, subject)
                    .map(|d| Persona::anonymous(&d))
                    .unwrap_or_else(|| Persona::anonymous(&blank_dossier(subject)));
                interview.personas.insert((scenario, subject), anonymous);
            }
        },
        (Job::Persona, Err(e)) => {
            // An interview with no persona is still an interview: the agent
            // answers from their traits, unnamed. Failing the whole panel
            // because the flavour call failed would be the wrong trade.
            interview.status = format!("no persona ({e}) — answering unnamed");
            let anonymous = dossier(&sim, subject)
                .map(|d| Persona::anonymous(&d))
                .unwrap_or_else(|| Persona::anonymous(&blank_dossier(subject)));
            interview.personas.insert((scenario, subject), anonymous);
        }
        (Job::Reply, Ok(text)) => {
            let t = sim.time_s();
            sim.history
                .log
                .record_message(t, telemetry_subject(subject), "assistant", &text);
            interview.status.clear();
        }
        (Job::Reply, Err(e)) => interview.status = e,
        (Job::Test, Ok(text)) => {
            interview.settings_status =
                format!("✔ {} answered: {}", interview.config.model(), text.trim());
        }
        (Job::Test, Err(e)) => interview.settings_status = format!("✖ {e}"),
        (Job::Models, _) => {}
    }
}

/// A dossier with nothing in it, for the one case where the agent has gone
/// (a unit removed, an id out of range) while a request was in flight.
fn blank_dossier(subject: SubjectRef) -> Dossier {
    Dossier {
        kind: subject.kind,
        id: subject.id,
        callsign: None,
        sim_time_s: 0,
        clock: "00:00:00".into(),
        facts: Vec::new(),
        perceptions: Vec::new(),
        timeline: Vec::new(),
        nationality: String::new(),
        region: String::new(),
        localities: Vec::new(),
        locality: None,
        address: None,
    }
}

/// Clear anything that belonged to the run being thrown away.
///
/// The transcript itself needs no help — it lived in the log the restart
/// replaced (finding 21: only *latched* state needs clearing). What does is an
/// in-flight request, whose answer would otherwise be filed against the new
/// run's log as if the new agent had said it.
pub fn reset(mut events: EventReader<crate::sim::SimRestarted>, mut interview: ResMut<Interview>) {
    if events.read().count() == 0 {
        return;
    }
    if interview.pending.is_some() {
        interview.pending = None;
        interview.status =
            "the incident was restarted while that answer was in flight — it was dropped"
                .to_string();
    } else if interview.open {
        interview.status =
            "the incident was restarted: this transcript went with the run it was about."
                .to_string();
    }
}

/// `SPOTORNO_INTERVIEW=1` opens the interview on whatever `SPOTORNO_WATCH`
/// selected; `=settings` opens the settings dialog instead.
///
/// The same reason `SPOTORNO_COMPOSER` and `SPOTORNO_WATCH` exist: opening an
/// interview takes a click on the 3D view and a click in a panel, neither of
/// which an unattended run can produce, so without this the window can only
/// ever be screenshotted shut.
pub fn open_from_env(
    sim: Option<Res<Sim>>,
    selected: Res<Selected>,
    mut interview: ResMut<Interview>,
    mut panels: ResMut<crate::ui::PanelState>,
) {
    let Ok(spec) = std::env::var("SPOTORNO_INTERVIEW") else {
        return;
    };
    if spec == "settings" {
        interview.settings_open = true;
        return;
    }
    let Some(sim) = sim else { return };
    match selected.target.and_then(|t| subject_of(&sim, t)) {
        Some(subject) => {
            interview.open_for(subject);
            panels.focus_bottom(crate::ui::BottomTab::Chat);
        }
        None => eprintln!("SPOTORNO_INTERVIEW: nothing selected — set SPOTORNO_WATCH too"),
    }
    interview.selftest = spec == "selftest";
}

/// `t` opens an interview with whatever is selected.
pub fn shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<crate::ui::UiFocus>,
    selected: Res<Selected>,
    sim: Res<Sim>,
    mut interview: ResMut<Interview>,
    mut panels: ResMut<crate::ui::PanelState>,
) {
    if focus.typing() || !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    match selected.target.and_then(|t| subject_of(&sim, t)) {
        Some(subject) => {
            interview.open_for(subject);
            panels.focus_bottom(crate::ui::BottomTab::Chat);
        }
        None => interview.status = "select an agent on the map first".to_string(),
    }
}

// --- the panel ------------------------------------------------------------

/// The question box's id, fixed rather than auto-generated.
///
/// Two reasons, both about focus surviving a frame that redraws differently:
/// the panel above the box grows a "said 12 min ago" label once the clock moves
/// on, and an auto-generated id is a function of what was laid out before it.
/// A fixed id is also what lets [`window`] hand focus back after a send.
fn input_id() -> egui::Id {
    egui::Id::new("interview-question")
}

/// Conversation surface embedded in the application's bottom workbench.
///
/// `Interview::open` means that the current subject owns the chat tab; the
/// transcript itself remains in the run log when another bottom tab is shown.
/// Keeping the chat inside the dock gives it the same stable map boundary as
/// the incident and developer views instead of stacking another floating
/// window over the entity the player is discussing.
pub fn panel_body(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    interview: &mut Interview,
    sim: &mut Sim,
    selected: Option<Target>,
) {
    let selected_subject = selected.and_then(|target| subject_of(sim, target));
    if interview.subject.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(28.0);
            ui.heading("Chat with an entity");
            ui.label("Select a household, person or unit in the right panel first.");
            if let Some(subject) = selected_subject {
                if ui
                    .button(format!("Start chat with {}", label(sim, subject)))
                    .clicked()
                {
                    interview.open_for(subject);
                }
            }
        });
        return;
    }

    let subject = interview.subject.expect("checked above");
    if interview.pause_on_open {
        interview.pause_on_open = false;
        sim.playing = false;
    }
    interview.open = true;

    let scenario = sim.scenario.id.clone();
    let has_persona = interview
        .personas
        .contains_key(&(scenario.clone(), subject));
    let mut ask: Option<String> = None;
    let mut make_persona = false;
    let mut clear = false;
    let mut switch_to = None;

    ui.horizontal(|ui| {
        let title = match interview.persona_for(&scenario, subject) {
            Some(p) if !p.is_anonymous() => format!("{} — {}", p.name, label(sim, subject)),
            _ => label(sim, subject),
        };
        ui.heading(title);
        if let Some(candidate) = selected_subject.filter(|candidate| *candidate != subject) {
            if ui
                .button(format!("Chat with selected: {}", label(sim, candidate)))
                .clicked()
            {
                switch_to = Some(candidate);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("⚙").on_hover_text("LLM settings").clicked() {
                interview.settings_open = true;
            }
            if ui
                .small_button("🗑")
                .on_hover_text("Discard this transcript")
                .clicked()
            {
                clear = true;
            }
            ui.small(format!(
                "T+{} · {}",
                sim.clock(),
                if sim.playing { "running" } else { "paused" }
            ));
        });
    });
    if let Some(persona) = interview.persona_for(&scenario, subject) {
        if !persona.is_anonymous() {
            ui.small(&persona.background);
        }
    }
    ui.separator();

    let messages = sim.history.log.messages_for(telemetry_subject(subject));
    let streaming = interview
        .pending
        .as_ref()
        .filter(|pending| pending.job == Job::Reply && pending.subject == subject)
        .map(|pending| pending.partial.clone());

    egui::TopBottomPanel::bottom("interview-dock-input")
        .resizable(false)
        .show_inside(ui, |ui| {
            if !interview.status.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(235, 150, 80), &interview.status);
            }
            if let Err(why) = interview.config.readiness() {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(235, 150, 80), why);
                    if ui.button("LLM settings…").clicked() {
                        interview.settings_open = true;
                    }
                });
                return;
            }
            if !has_persona {
                if interview.busy() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Working out who this is…");
                    });
                } else if ui.button("Meet this agent").clicked() {
                    make_persona = true;
                }
                return;
            }

            ui.horizontal(|ui| {
                let busy = interview.busy();
                let hint = if busy {
                    "Waiting for their answer…"
                } else if messages.is_empty() {
                    chat::prompt::opening_question(subject.kind)
                } else {
                    "Ask something…"
                };
                let response = ui.add(
                    egui::TextEdit::singleline(&mut interview.input)
                        .id(input_id())
                        .desired_width(ui.available_width() - 64.0)
                        .hint_text(hint),
                );
                let ready = !busy && !interview.input.trim().is_empty();
                let entered = ready
                    && response.lost_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter));
                if ui.add_enabled(ready, egui::Button::new("Ask")).clicked() || entered {
                    ask = Some(interview.input.trim().to_string());
                }
                if busy {
                    ui.spinner();
                }
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        egui::ScrollArea::vertical()
            .id_source("interview-dock-messages")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if messages.is_empty() && streaming.is_none() {
                    ui.weak(
                        "Nothing said yet. Ask where they are, what they can see, or why they have not left.",
                    );
                }
                for message in &messages {
                    bubble(ui, &message.role, &message.content, message.sim_time_s);
                }
                if let Some(partial) = &streaming {
                    if partial.is_empty() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.weak("thinking…");
                        });
                    } else {
                        bubble(ui, "assistant", partial, sim.time_s());
                    }
                }
            });
    });

    if clear {
        sim.history.log.clear_messages(telemetry_subject(subject));
        interview.status.clear();
    }
    if make_persona {
        start_persona(interview, sim, subject);
    }
    if let Some(question) = ask {
        start_reply(interview, sim, subject, &question);
        ctx.memory_mut(|memory| memory.request_focus(input_id()));
    }
    if let Some(candidate) = switch_to {
        interview.open_for(candidate);
    }
}

/// The floating interview window.
#[allow(dead_code)]
pub fn window(
    mut contexts: EguiContexts,
    mut interview: ResMut<Interview>,
    mut sim: ResMut<Sim>,
    mut focus: ResMut<crate::ui::UiFocus>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if !interview.open {
        return;
    }
    let Some(subject) = interview.subject else {
        interview.open = false;
        return;
    };
    let ctx = contexts.ctx_mut();
    // Read before the window draws: pressing Esc inside the question box is
    // egui's own "give up focus", and it clears this the same frame. Taking it
    // first makes the two presses read the way they do everywhere else — one
    // out of the field, one out of the window — rather than one press doing
    // both and the panel vanishing mid-question.
    let was_typing = ctx.memory(|m| m.has_focus(input_id()));

    // Opening an interview stops the clock: the agent answers as of one
    // instant, and letting the fire run under the conversation would date every
    // answer before it was read. Once only — see `pause_on_open`.
    if interview.pause_on_open {
        interview.pause_on_open = false;
        sim.playing = false;
    }

    let scenario = sim.scenario.id.clone();
    let has_persona = interview
        .personas
        .contains_key(&(scenario.clone(), subject));
    let mut ask: Option<String> = None;
    let mut make_persona = false;
    let mut clear = false;
    let mut open = true;

    let title = match interview.persona_for(&scenario, subject) {
        Some(p) if !p.is_anonymous() => format!("{} — {}", p.name, label(&sim, subject)),
        _ => format!("Interview — {}", label(&sim, subject)),
    };

    egui::Window::new(title)
        .id(egui::Id::new("interview-window"))
        .open(&mut open)
        .default_size([520.0, 620.0])
        .min_size([380.0, 360.0])
        .vscroll(false)
        .show(ctx, |ui| {
            // Who they are, and what they are doing — so the transcript can be
            // read against the state it came from.
            if let Some(p) = interview.persona_for(&scenario, subject) {
                if !p.is_anonymous() {
                    ui.label(egui::RichText::new(p.headline()).strong());
                    ui.small(&p.background);
                }
            }
            let messages = sim.history.log.messages_for(telemetry_subject(subject));
            ui.horizontal(|ui| {
                ui.small(format!(
                    "T+{} · {}",
                    sim.clock(),
                    if sim.playing { "running" } else { "paused" }
                ));
                // An answer is only true of the moment it was given. Saying so
                // is the alternative to freezing the clock behind the player's
                // back: the transcript above stops being a description of the
                // incident as it is now, and nothing else on screen would say.
                if let Some(last) = messages.last() {
                    if last.sim_time_s < sim.time_s() {
                        ui.small(
                            egui::RichText::new(format!(
                                "· said {} ago",
                                elapsed(sim.time_s() - last.sim_time_s)
                            ))
                            .color(egui::Color32::from_rgb(235, 150, 80)),
                        )
                        .on_hover_text(
                            "The incident has moved on since that answer. Ask again to get \
                             what they would say now.",
                        );
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("⚙").on_hover_text("LLM settings").clicked() {
                        interview.settings_open = true;
                    }
                    if ui
                        .small_button("🗑")
                        .on_hover_text("Discard this transcript")
                        .clicked()
                    {
                        clear = true;
                    }
                });
            });
            ui.separator();

            let streaming = interview
                .pending
                .as_ref()
                .filter(|p| p.job == Job::Reply && p.subject == subject)
                .map(|p| p.partial.clone());

            egui::TopBottomPanel::bottom("interview-input")
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.add_space(4.0);
                    if !interview.status.is_empty() {
                        ui.colored_label(egui::Color32::from_rgb(235, 150, 80), &interview.status);
                    }
                    if let Err(why) = interview.config.readiness() {
                        ui.colored_label(egui::Color32::from_rgb(235, 150, 80), why);
                        if ui.button("Open LLM settings…").clicked() {
                            interview.settings_open = true;
                        }
                        return;
                    }
                    if !has_persona {
                        ui.horizontal(|ui| {
                            if interview.busy() {
                                ui.spinner();
                                ui.label("Working out who this is…");
                            } else if ui
                                .button("Meet this agent")
                                .on_hover_text(
                                    "Build a person from this agent's baked traits, once. \
                                     Kept for the rest of the session.",
                                )
                                .clicked()
                            {
                                make_persona = true;
                            }
                        });
                        return;
                    }
                    ui.horizontal(|ui| {
                        let busy = interview.busy();
                        let hint = if busy {
                            "Waiting for their answer…"
                        } else if messages.is_empty() {
                            chat::prompt::opening_question(subject.kind)
                        } else {
                            "Ask something…"
                        };
                        // Never disabled, even while an answer is streaming.
                        // egui drops focus from a disabled widget and will not
                        // give it back, so a field that greyed out for the
                        // duration of a reply came back unfocused — and an
                        // unfocused field in a chat window means every letter
                        // typed goes to the map's single-key shortcuts instead
                        // (finding 25). Typing ahead while they answer is also
                        // simply what a chat box should let you do.
                        let field = egui::TextEdit::singleline(&mut interview.input)
                            .id(input_id())
                            .desired_width(f32::INFINITY)
                            .hint_text(hint);
                        let r = ui.add(field);
                        let ready = !busy && !interview.input.trim().is_empty();
                        let entered = ready
                            && r.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let clicked = ui.add_enabled(ready, egui::Button::new("Ask")).clicked();
                        if entered || clicked {
                            ask = Some(interview.input.trim().to_string());
                        }
                        if busy {
                            ui.spinner();
                        }
                    });
                    ui.small(
                        "They only know their own day: their traits, what they can see from \
                         where they are, and what has happened to them.",
                    );
                    ui.add_space(2.0);
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if messages.is_empty() && streaming.is_none() {
                            ui.add_space(8.0);
                            ui.weak(
                                "Nothing said yet. Ask them where they are, what they can see, \
                                 or why they have not left.",
                            );
                        }
                        for m in &messages {
                            bubble(ui, &m.role, &m.content, m.sim_time_s);
                        }
                        if let Some(partial) = &streaming {
                            if partial.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.weak("thinking…");
                                });
                            } else {
                                bubble(ui, "assistant", partial, sim.time_s());
                            }
                        }
                    });
            });
        });

    if !open {
        interview.open = false;
    }

    // --- act on what the panel asked for ---
    if clear {
        sim.history.log.clear_messages(telemetry_subject(subject));
        interview.status.clear();
    }
    if make_persona {
        start_persona(&mut interview, &sim, subject);
    }
    if let Some(question) = ask {
        start_reply(&mut interview, &mut sim, subject, &question);
        // Enter releases focus — that is how `lost_focus()` fires at all — so
        // without this the box you just typed into is dead for the next
        // question, and the keys meant for it reach the map instead.
        ctx.memory_mut(|m| m.request_focus(input_id()));
    }

    // The way out. `menu::menubar` hands the whole keyboard to an open
    // interview, so Esc is the only key left that can close one — which is
    // exactly the job it already has everywhere else in this game, and the one
    // shortcut deliberately not gated on focus.
    if keys.just_pressed(KeyCode::Escape) && !was_typing {
        interview.open = false;
    }

    focus.pointer |= ctx.is_pointer_over_area() || ctx.wants_keyboard_input();
}

/// Ask the model to invent this agent's person. Does nothing if a request is
/// already in flight.
///
/// A free function rather than panel code because two callers need it: the
/// button, and [`selftest`] — and a self-test that took a different path
/// through this would be testing something other than what the button does.
fn start_persona(interview: &mut Interview, sim: &Sim, subject: SubjectRef) {
    if interview.busy() {
        return;
    }
    let Some(d) = dossier(sim, subject) else {
        return;
    };
    let rx = spawn(interview.config.clone(), chat::persona::request(&d));
    interview.pending = Some(Pending {
        job: Job::Persona,
        subject,
        rx: std::sync::Mutex::new(rx),
        partial: String::new(),
    });
}

/// Put one question to the agent.
fn start_reply(interview: &mut Interview, sim: &mut Sim, subject: SubjectRef, question: &str) {
    if interview.busy() {
        return;
    }
    let persona = interview
        .persona_for(&sim.scenario.id, subject)
        .cloned()
        .unwrap_or_else(|| Persona::anonymous(&blank_dossier(subject)));
    let messages = conversation(sim, &persona, subject, question);
    let t = sim.time_s();
    // Recorded before the reply is asked for, so a question that fails is still
    // in the transcript: what was asked is as much a part of the record as what
    // came back.
    sim.history
        .log
        .record_message(t, telemetry_subject(subject), "user", question);
    interview.input.clear();
    let rx = spawn(interview.config.clone(), messages);
    interview.pending = Some(Pending {
        job: Job::Reply,
        subject,
        rx: std::sync::Mutex::new(rx),
        partial: String::new(),
    });
}

/// `SPOTORNO_INTERVIEW=selftest` drives one whole interview with no keyboard:
/// meet the agent, ask them the opening question, print both to stdout, exit
/// non-zero on failure.
///
/// The only unattended exercise of the wiring between the worker thread, the
/// poll and the transcript — everything below it is covered by the `chat`
/// crate's own tests, and everything above it needs a person clicking. It is
/// not part of `SPOTORNO_SELFTEST` deliberately: **it spends real credit on a
/// real provider**, and a self-test that quietly bought API calls every time
/// somebody ran the suite would be a bad neighbour.
pub fn selftest(
    mut interview: ResMut<Interview>,
    mut sim: ResMut<Sim>,
    mut quit: EventWriter<bevy::app::AppExit>,
) {
    if !interview.selftest || interview.busy() {
        return;
    }
    let Some(subject) = interview.subject else {
        eprintln!("interview selftest: nothing selected — set SPOTORNO_WATCH");
        quit.send(bevy::app::AppExit::error());
        return;
    };
    if let Err(why) = interview.config.readiness() {
        eprintln!("interview selftest: {why}");
        quit.send(bevy::app::AppExit::error());
        return;
    }

    let scenario = sim.scenario.id.clone();
    let messages = sim.history.log.messages_for(telemetry_subject(subject));
    match interview.persona_for(&scenario, subject) {
        // Step 1: who is this?
        None => {
            println!("interview selftest: meeting {}…", label(&sim, subject));
            start_persona(&mut interview, &sim, subject);
        }
        Some(persona) => {
            let persona = persona.clone();
            if messages.is_empty() {
                // Step 2: ask them something.
                println!("interview selftest: {}", persona.headline());
                println!("  {}", persona.background);
                let question = chat::prompt::opening_question(subject.kind).to_string();
                start_reply(&mut interview, &mut sim, subject, &question);
            } else if messages.iter().any(|m| m.role == "assistant") {
                // Step 3: read it back out of the transcript, which is the
                // half a live provider test cannot reach.
                for m in &messages {
                    println!("  [{}] {}", m.role, m.content.trim());
                }
                let answered = messages
                    .iter()
                    .any(|m| m.role == "assistant" && !m.content.trim().is_empty());
                interview.selftest = false;
                if answered {
                    println!("interview selftest: ok");
                    quit.send(bevy::app::AppExit::Success);
                } else {
                    eprintln!("interview selftest: the agent said nothing");
                    quit.send(bevy::app::AppExit::error());
                }
            } else if !interview.status.is_empty() {
                eprintln!("interview selftest: {}", interview.status);
                interview.selftest = false;
                quit.send(bevy::app::AppExit::error());
            }
        }
    }
}

/// Simulated seconds as a span, for "said 12 min ago".
#[allow(dead_code)]
fn elapsed(s: i64) -> String {
    if s < 90 {
        format!("{s} s")
    } else {
        format!("{} min", s / 60)
    }
}

fn bubble(ui: &mut egui::Ui, role: &str, content: &str, sim_time_s: i64) {
    let (who, colour) = if role == "user" {
        ("You", egui::Color32::from_rgb(150, 190, 235))
    } else {
        ("Them", egui::Color32::from_rgb(240, 210, 150))
    };
    ui.horizontal(|ui| {
        ui.colored_label(colour, egui::RichText::new(who).strong().small());
        ui.weak(egui::RichText::new(Dossier::stamp(sim_time_s)).small());
    });
    ui.add(egui::Label::new(content).wrap());
    ui.add_space(8.0);
}

/// A model menu that stays usable when a provider exposes hundreds of ids.
/// The selected id remains directly editable so a newly released or private
/// model can still be used before it appears in the refreshed list.
fn model_picker(
    ui: &mut egui::Ui,
    id: &'static str,
    selected: &mut String,
    search: &mut String,
    models: &[String],
) {
    ui.vertical(|ui| {
        egui::ComboBox::from_id_source(id)
            .width(310.0)
            .selected_text(if selected.trim().is_empty() {
                "Choose a model…"
            } else {
                selected.as_str()
            })
            .show_ui(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(search)
                        .hint_text("Search models…")
                        .desired_width(290.0),
                );
                ui.separator();

                let needle = search.trim().to_lowercase();
                let mut shown = 0;
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        for model in models {
                            if !needle.is_empty() && !model.to_lowercase().contains(&needle) {
                                continue;
                            }
                            shown += 1;
                            if ui.selectable_label(selected == model, model).clicked() {
                                selected.clone_from(model);
                                search.clear();
                                ui.close_menu();
                            }
                        }
                        if shown == 0 {
                            ui.weak(if models.is_empty() {
                                "Refresh models to load choices."
                            } else {
                                "No matching models."
                            });
                        }
                    });
            });
        ui.add(
            egui::TextEdit::singleline(selected)
                .hint_text("or enter an exact model id")
                .desired_width(310.0),
        );
    });
}

/// Provider, model and key — the whole of what makes an interview possible.
pub fn settings_window(
    mut contexts: EguiContexts,
    mut interview: ResMut<Interview>,
    mut focus: ResMut<crate::ui::UiFocus>,
) {
    if !interview.settings_open {
        return;
    }
    let ctx = contexts.ctx_mut();
    let mut open = true;
    let mut save = false;
    let mut test = false;
    let mut refresh_models = false;

    egui::Window::new("LLM settings")
        .open(&mut open)
        .default_width(460.0)
        .vscroll(false)
        .show(ctx, |ui| {
            ui.label("Who answers when you interview a simulated agent.");
            ui.add_space(6.0);

            for p in chat::Provider::ALL {
                let selected = interview.config.provider == p;
                if ui
                    .radio(selected, p.label())
                    .on_hover_text(p.hint())
                    .clicked()
                {
                    if interview.config.provider != p {
                        interview.models.clear();
                        interview.model_search.clear();
                        interview.model_status.clear();
                    }
                    interview.config.select_provider(p);
                }
            }
            ui.separator();

            match interview.config.provider {
                chat::Provider::OpenRouter => {
                    let models = interview.models.clone();
                    let env_model = interview.config.model_from_env();
                    egui::Grid::new("openrouter").num_columns(2).show(ui, |ui| {
                        ui.label("Model");
                        // Shown as a label rather than an editable field when
                        // the environment is imposing one: an edit that the
                        // next request would silently ignore is worse than no
                        // field at all.
                        match &env_model {
                            Some(m) => {
                                ui.label(m).on_hover_text("set by OPENROUTER_MODEL");
                            }
                            None => {
                                let state = interview.as_mut();
                                model_picker(
                                    ui,
                                    "openrouter-model",
                                    &mut state.config.openrouter_model,
                                    &mut state.model_search,
                                    &models,
                                );
                            }
                        }
                        ui.end_row();

                        ui.label("API key");
                        if interview.config.key_from_env() {
                            ui.label("set by OPENROUTER_API_KEY");
                        } else {
                            ui.add(
                                egui::TextEdit::singleline(&mut interview.config.openrouter_key)
                                    .password(true)
                                    .hint_text("sk-or-…"),
                            );
                        }
                        ui.end_row();
                    });
                    if ui.button("Refresh models from OpenRouter").clicked() { refresh_models = true; }
                    if !interview.model_status.is_empty() { ui.small(&interview.model_status); }
                    if interview.config.key_from_env() || env_model.is_some() {
                        ui.small(
                            "Read from the environment (a .env at the repository root counts), \
                             which wins over anything saved here.",
                        );
                    } else {
                        ui.small("Get one at openrouter.ai ▸ Keys.");
                    }
                }
                chat::Provider::Ollama => {
                    let models = interview.models.clone();
                    egui::Grid::new("ollama").num_columns(2).show(ui, |ui| {
                        ui.label("Server");
                        ui.text_edit_singleline(&mut interview.config.ollama_url);
                        ui.end_row();

                        ui.label("Model");
                        let state = interview.as_mut();
                        model_picker(
                            ui,
                            "ollama-model",
                            &mut state.config.ollama_model,
                            &mut state.model_search,
                            &models,
                        );
                        ui.end_row();
                    });
                    if ui.button("Refresh models from Ollama").clicked() { refresh_models = true; }
                    if !interview.model_status.is_empty() { ui.small(&interview.model_status); }
                    ui.small("Needs `ollama serve` running, and the model already pulled.");
                }
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Temperature");
                ui.add(egui::Slider::new(
                    &mut interview.config.temperature,
                    0.0..=1.5,
                ));
            });
            ui.horizontal(|ui| {
                ui.label("Reply cap");
                ui.add(
                    egui::Slider::new(&mut interview.config.max_tokens, 100..=2000)
                        .suffix(" tokens"),
                );
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    save = true;
                }
                let busy = interview.busy();
                if ui
                    .add_enabled(!busy, egui::Button::new("Test"))
                    .on_hover_text("Send one short message and show what comes back.")
                    .clicked()
                {
                    test = true;
                }
                if busy {
                    ui.spinner();
                }
            });
            if !interview.settings_status.is_empty() {
                ui.add_space(4.0);
                ui.add(egui::Label::new(&interview.settings_status).wrap());
            }
            ui.add_space(4.0);
            ui.small(format!(
                "Saved in {}",
                chat::config::storage_label()
            ));
            ui.small("The API key is stored locally and is never written into the repository.");
        });

    if save {
        interview.settings_status = match interview.config.save() {
            Ok(()) => "Saved.".to_string(),
            Err(e) => format!("✖ could not save: {e:#}"),
        };
    }
    if refresh_models && !interview.busy() {
        interview.model_status = "loading models…".to_string();
        let subject = interview.subject.unwrap_or_else(|| SubjectRef::new(SubjectKind::Household, 0));
        interview.pending = Some(Pending { job: Job::Models, subject, rx: std::sync::Mutex::new(spawn_models(interview.config.clone())), partial: String::new() });
    }
    if test && !interview.busy() {
        match interview.config.readiness() {
            Err(why) => interview.settings_status = format!("✖ {why}"),
            Ok(()) => {
                interview.settings_status = "asking…".to_string();
                let messages = vec![
                    Message::system("Reply with exactly five words."),
                    Message::user("Are you there?"),
                ];
                let rx = spawn(interview.config.clone(), messages);
                let subject = interview
                    .subject
                    .unwrap_or_else(|| SubjectRef::new(SubjectKind::Household, 0));
                interview.pending = Some(Pending {
                    job: Job::Test,
                    subject,
                    rx: std::sync::Mutex::new(rx),
                    partial: String::new(),
                });
            }
        }
    }

    interview.settings_open &= open;
    focus.pointer |= ctx.is_pointer_over_area() || ctx.wants_keyboard_input();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass_points_the_right_way() {
        // World frame: +x east, +y north.
        assert_eq!(compass(0.0, 100.0), "north");
        assert_eq!(compass(100.0, 0.0), "east");
        assert_eq!(compass(0.0, -100.0), "south");
        assert_eq!(compass(-100.0, -100.0), "south-west");
    }

    #[test]
    fn a_commanders_order_is_not_a_memory_until_it_arrives() {
        // The god-view leak this translation exists to prevent: a household
        // knows an order exists when somebody reaches them, not when the
        // commander gives it.
        let d = serde_json::json!({});
        assert!(recollection(SubjectKind::Household, "order_issued", &d).is_none());
        assert!(recollection(SubjectKind::Household, "warning_received", &d).is_some());
    }

    #[test]
    fn a_status_change_becomes_something_a_person_would_say() {
        let d = serde_json::json!({"from": "normal", "to": "preparing"});
        let line = recollection(SubjectKind::Household, "status", &d).unwrap();
        assert!(line.contains("getting ready"));
        // And the log's own vocabulary does not survive the translation.
        assert!(!line.contains("status"));
        assert!(!line.contains("preparing ->"));
    }

    #[test]
    fn unrecognised_events_are_dropped_rather_than_guessed() {
        let d = serde_json::json!({"to": "something new"});
        assert!(recollection(SubjectKind::Household, "status", &d).is_none());
        assert!(recollection(SubjectKind::Household, "no_such_event", &d).is_none());
    }

    #[test]
    fn a_unit_remembers_its_own_orders_and_a_household_does_not() {
        let d = serde_json::json!({"task": "line"});
        assert!(recollection(SubjectKind::Unit, "unit_task", &d).is_some());
        assert!(recollection(SubjectKind::Household, "unit_task", &d).is_none());
    }
}
