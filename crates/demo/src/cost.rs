//! What the commander's decisions cost, in euros (`docs/demo-spec-gameplay.md` 5.1).
//!
//! Money is **shown**, never scored: the end card prints it beside what it saved.
//! It exists so that pressing everything at T+0 is a *choice* and not a free win.
//!
//! **Cost is a pure function of the action log.** A [`Log`] is the list of things
//! the commander did (an evacuation, a unit tasked, an air load flown); [`Log::price`]
//! turns it into a [`Ledger`] and reads nothing else. The headless `Run` and the live
//! kiosk both feed one `Log`, so they price identically (tested).
//!
//! # Sources
//!
//! No network was available when these were written; they are **indicative figures
//! from memory of Italian public tariffs, to be verified** with the Protezione
//! Civile / regional AIB office before the event (spec 8, open question):
//!
//! * Air tanker (CL-415 Canadair, COAU-tasked): ~EUR 9-12 k per flight hour, a
//!   scoop-and-drop cycle on a lake 3-6 km away ~15 min => **EUR 3,000 per load**.
//! * AIB engine (autobotte) with a crew of 3-4: vehicle + fuel + personnel
//!   ~EUR 250 per hour on call-out.
//! * Volunteer AIB hand crew (squadra, 5): reimbursement of lost earnings and
//!   equipment, ~EUR 60 per hour.
//! * A general evacuation order: coaches and shelter, a lost working day for the
//!   household (~EUR 100-150), shops shut, municipal staff overtime, and the road
//!   closed to everyone else: indicative **EUR 250 per household** moved (EUR 60-90 k
//!   for these towns). Needless or needed, the same bill. The scale is chosen by sweep
//!   (`tests/cost_sweep.rs`): at 40 the order was nearly free and T+0 always won.

use abm::suppression::UnitKind;

/// **Analysis only, never shown to a player** (the card prints facts and money, no
/// score): how a sweep trades money against harm so that "which policy is better on
/// average" has an answer. EUR per household the fire caught at home, and per home
/// it reached. Results are reported across a range of `caught_eur` because the
/// ratio, not the level, is what the design depends on.
#[derive(Debug, Clone, Copy)]
pub struct Weights {
    pub caught_eur: f32,
    pub home_eur: f32,
}

impl Weights {
    pub const ANALYSIS: Weights = Weights { caught_eur: 20_000.0, home_eur: 20_000.0 };

    pub fn loss(&self, spent_eur: f32, caught: usize, homes_lost: usize) -> f32 {
        spent_eur + self.caught_eur * caught as f32 + self.home_eur * homes_lost as f32
    }
}

/// EUR per air-tanker load actually dropped.
pub const AIR_LOAD_EUR: f32 = 3_000.0;
/// EUR per engine-hour from the moment it is tasked.
pub const ENGINE_HOUR_EUR: f32 = 250.0;
/// EUR per hand-crew-hour from the moment it is tasked.
pub const CREW_HOUR_EUR: f32 = 60.0;
/// EUR per household covered by a general evacuation order.
pub const EVACUATION_PER_HOUSEHOLD_EUR: f32 = 250.0;

/// Something the commander did, with the simulated second it happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Entry {
    /// A general evacuation order covering `households` households.
    Evacuation { at_s: i64, households: usize },
    /// A unit tasked for the first time (later orders to the same unit add no cost
    /// of their own: it is already on call-out).
    Tasked { at_s: i64, unit: usize, kind: UnitKind },
    /// One air load flown.
    AirLoad { at_s: i64 },
}

/// What a line of the ledger is for. Stable, so the UI can map each to Italian.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Evacuation,
    Engines,
    Crews,
    Air,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Item {
    pub action: Action,
    pub eur: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ledger {
    pub items: Vec<Item>,
}

impl Ledger {
    pub fn total_eur(&self) -> f32 {
        self.items.iter().map(|i| i.eur).sum()
    }

    pub fn by_action(&self, a: Action) -> f32 {
        self.items.iter().filter(|i| i.action == a).map(|i| i.eur).sum()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Log {
    pub entries: Vec<Entry>,
}

impl Log {
    pub fn push(&mut self, e: Entry) {
        self.entries.push(e);
    }

    /// Record a unit being tasked, once per unit.
    pub fn task(&mut self, at_s: i64, unit: usize, kind: UnitKind) {
        let seen = self.entries.iter().any(|e| matches!(e, Entry::Tasked { unit: u, .. } if *u == unit));
        if !seen {
            self.push(Entry::Tasked { at_s, unit, kind });
        }
    }

    /// The bill as of simulated second `now_s`.
    pub fn price(&self, now_s: i64) -> Ledger {
        let mut items = vec![];
        for e in &self.entries {
            match *e {
                Entry::Evacuation { households, .. } => {
                    items.push(Item { action: Action::Evacuation, eur: households as f32 * EVACUATION_PER_HOUSEHOLD_EUR })
                }
                Entry::Tasked { at_s, kind, .. } => {
                    let hours = (now_s - at_s).max(0) as f32 / 3600.0;
                    match kind {
                        UnitKind::Engine => items.push(Item { action: Action::Engines, eur: hours * ENGINE_HOUR_EUR }),
                        UnitKind::HandCrew => items.push(Item { action: Action::Crews, eur: hours * CREW_HOUR_EUR }),
                        // Air is billed per load, not per hour on call.
                        UnitKind::AirTanker => {}
                    }
                }
                Entry::AirLoad { at_s } if at_s <= now_s => items.push(Item { action: Action::Air, eur: AIR_LOAD_EUR }),
                Entry::AirLoad { .. } => {}
            }
        }
        Ledger { items }
    }
}
