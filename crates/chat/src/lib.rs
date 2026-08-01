//! Interviewing a simulated agent through an LLM.
//!
//! The event log (`telemetry`) records what happened to each agent; this crate
//! is what turns one agent's row of that log into somebody you can talk to. A
//! household is a set of numbers — `risk_perception`, `trust_authority`,
//! `prep_time_min`, an intent and a status — plus a timeline of the discrete
//! things that happened to it. A [`Persona`] gives those numbers a name, an
//! age, an occupation and a voice, and the [`prompt`] module renders the pair
//! into something a model can answer as.
//!
//! **In character, and only in character.** The single design constraint here
//! is that an agent may only be told what that agent could know: its own
//! traits, its own history, and what it can perceive from where it is standing.
//! It is never handed the incident-wide roster or the status counts, and the
//! system prompt says so in as many words. This is not decoration — a civilian
//! who answers "264 of us are safe" is not a simulated civilian, it is the
//! simulation's telemetry wearing a name, and the thing that makes that
//! failure dangerous is that it reads as a *better* answer than the honest
//! one. The type that enforces it is [`Dossier`]: it has no field for an
//! aggregate, so there is no incident-wide fact to leak, and the only way to
//! add one is to add it here on purpose.
//!
//! **The crate is a leaf.** It knows nothing about `abm`, `fire`, `scenario`,
//! `telemetry` or Bevy — a [`Dossier`] is plain strings and numbers, assembled
//! by `game`, which is the only thing that has a `Sim` to assemble it from.
//! There is no database here either: a transcript is stored in the run's own
//! event log (`telemetry::Recorder::record_message`), which means it has
//! exactly the lifetime of every other piece of run state — a restart discards
//! the history each answer was drawn from, so it discards the answers with it.
//! What that leaves this crate is the part worth testing without a simulation
//! loaded: prompt rendering, persona parsing, and the two stream parsers.
//!
//! Personas are the one thing held longer than a run, and they are held in
//! memory by `game` rather than written anywhere. Household 42's traits are
//! baked into the population and identical in every run of a scenario, so the
//! person built on them should be too — and regenerating one on every restart
//! would be a paid API call to reinvent somebody the model already knew.

pub mod config;
pub mod persona;
pub mod prompt;
pub mod provider;
pub mod subject;

pub use config::{LlmConfig, Provider};
pub use persona::Persona;
pub use prompt::{Dossier, Fact, TimelineEntry};
pub use provider::{Client, Message, Role};
pub use subject::{SubjectKind, SubjectRef};
