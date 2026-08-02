//! The synthetic population, placed on real footprints.
//!
//! Attribute names follow the wildfire evacuation literature rather than a
//! generic hazard model: what matters is warning receipt, milling time,
//! vehicle availability, defensible space, and who needs help to move.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    /// Goes at the first credible warning.
    LeaveEarly,
    /// Waits for direct cues -- the dangerous majority.
    WaitAndSee,
    /// Intends to defend the property.
    StayDefend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningChannel {
    MobileAlert,
    Neighbour,
    Siren,
    SelfObserved,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Normal,
    Warned,
    Preparing,
    Evacuating,
    Evacuated,
    Defending,
    Trapped,
    Casualty,
}

#[derive(Debug, Deserialize)]
pub struct Dwelling {
    pub osm_id: i64,
    pub kind: String,
    pub pos: [f32; 2],
    pub area_m2: f32,
    pub levels: u8,
    pub units: u32,
    pub cell: [usize; 2],
    pub dist_to_fuel_m: f32,
    pub fuel_at_site: i32,
}

#[derive(Debug, Deserialize)]
pub struct Household {
    pub id: usize,
    pub building: i64,
    pub pos: [f32; 2],
    pub cell: [usize; 2],
    pub size: u8,
    pub vehicles: u8,
    pub dist_to_fuel_m: f32,

    pub risk_perception: f32,
    pub prior_fire_experience: bool,
    pub warning_channel: WarningChannel,
    pub trust_authority: f32,
    pub intent: Intent,
    /// Minutes of milling before the household actually moves. The single
    /// biggest lever on whether an evacuation succeeds.
    pub prep_time_min: f32,
    pub defensible_space: f32,
    pub has_pets_livestock: bool,
    pub status: Status,
    pub members: Vec<usize>,
}

#[derive(Debug, Deserialize)]
pub struct Person {
    pub id: usize,
    pub household: usize,
    pub age: u8,
    /// Metres per second on flat ground.
    pub walk_speed: f32,
    pub needs_assistance: bool,
    pub at_home: bool,
}

#[derive(Debug, Deserialize)]
pub struct Population {
    pub synthetic: bool,
    pub seed: u64,
    pub dwellings: Vec<Dwelling>,
    pub households: Vec<Household>,
    pub people: Vec<Person>,
}

impl Population {
    pub fn load(dir: &Path) -> Result<Population> {
        let bytes = std::fs::read(dir.join("population.json"))
            .context("population.json")?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load_web(bytes: &[u8]) -> Result<Population> {
        Ok(serde_json::from_slice(bytes)?)
    }
}
