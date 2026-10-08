//! What a game is played on: one territory, several fires. Read from the
//! scenario's `game.json`, written by the Scenario Factory (`game-cases`), so
//! nothing about the town is hard-coded here.

use std::path::Path;

use anyhow::{Context, Result};
use fire::Weather;
use scenario::Pos;
use serde::Deserialize;

use abm::suppression::UnitKind;

#[derive(Debug, Clone, Deserialize)]
pub struct Station {
    pub name: String,
    pub pos: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Engine,
    HandCrew,
}

impl Kind {
    pub fn unit_kind(self) -> UnitKind {
        match self {
            Kind::Engine => UnitKind::Engine,
            Kind::HandCrew => UnitKind::HandCrew,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Slot {
    pub kind: Kind,
    /// Index into [`Game::stations`].
    pub station: usize,
}

/// A change of wind during a case, at a fixed simulated minute. Part of the
/// case, not a forecast: the game never shows it before it happens.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Shift {
    pub at_min: i64,
    pub wind_from_deg: f64,
    pub wind_kmh: f64,
}

/// One fire on the territory.
#[derive(Debug, Clone, Deserialize)]
pub struct Case {
    pub name: String,
    /// The locality this start was picked for (the atlas' grouping).
    pub near: String,
    pub ignition: [f32; 2],
    pub radius_m: f32,
    /// Where the wind blows FROM, degrees.
    pub wind_from_deg: f64,
    pub wind_kmh: f64,
    pub minutes: i64,
    #[serde(default)]
    pub shift: Option<Shift>,
}

impl Case {
    pub fn ignition(&self) -> Pos {
        Pos::from(self.ignition)
    }

    pub fn duration_s(&self) -> i64 {
        self.minutes * 60
    }
}

/// The scenario's `game.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Territory {
    pub scenario: String,
    pub moisture_pct: f64,
    pub stations: Vec<Station>,
    pub roster: Vec<Slot>,
    /// The cases the kiosk plays, in order; the rest are for the headless
    /// runner and the operator.
    #[serde(default)]
    pub featured: Vec<String>,
    pub cases: Vec<Case>,
}

impl Territory {
    pub fn load(data_dir: &Path) -> Result<Territory> {
        let p = data_dir.join("scenarios").join(scenario::ID).join("game.json");
        let t: Territory = serde_json::from_slice(&datafs::read(&p).with_context(|| format!("reading {}", p.display()))?)
            .with_context(|| format!("parsing {}", p.display()))?;
        anyhow::ensure!(t.roster.iter().all(|s| s.station < t.stations.len()), "roster names a missing station");
        anyhow::ensure!(t.featured.iter().all(|n| t.case(n).is_some()), "featured names a missing case");
        Ok(t)
    }

    pub fn case(&self, name: &str) -> Option<&Case> {
        self.cases.iter().find(|c| c.name == name)
    }

    /// The kiosk's cases in order: the featured ones, or all of them.
    pub fn playlist(&self) -> Vec<String> {
        if self.featured.is_empty() {
            self.cases.iter().map(|c| c.name.clone()).collect()
        } else {
            self.featured.clone()
        }
    }

    pub fn weather(&self, c: &Case) -> Weather {
        Weather { wind_dir_deg: c.wind_from_deg, wind_speed_kmh: c.wind_kmh, moisture_pct: self.moisture_pct }
    }

    /// The weather after the case's wind shift, if it has one.
    pub fn shifted(&self, c: &Case) -> Option<(i64, Weather)> {
        c.shift.map(|s| (s.at_min * 60, Weather { wind_dir_deg: s.wind_from_deg, wind_speed_kmh: s.wind_kmh, moisture_pct: self.moisture_pct }))
    }
}
