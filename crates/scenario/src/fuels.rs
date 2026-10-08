//! The `eu_fuel12` fuel table, as baked data.
//!
//! propagator-core ships only the 7-class legacy table, and the Spotorno
//! rasters use the 12-class EU system, so the definitions travel as JSON. They
//! live here rather than in the `fire` crate because they are a property of
//! the scenario's rasters, not of the engine.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// One fuel class, in the core's *config* units (v0 m/h, humidity percent).
#[derive(Debug, Clone, Deserialize)]
pub struct FuelDefRaw {
    pub id: i32,
    pub name: String,
    pub v0: f64,
    pub d0: f64,
    pub d1: f64,
    pub hhv: f64,
    pub humidity: Option<f64>,
    pub spotting: bool,
    pub prob_ign_by_embers: f64,
    pub burn: bool,
    pub spread_probability: Vec<(i32, f64)>,
}

pub fn load(dir: &Path) -> Result<Vec<FuelDefRaw>> {
    let bytes =
        std::fs::read(dir.join("fuels_eu12.json")).context("fuels_eu12.json")?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Human-readable class name, for the inspection UI.
pub fn class_name(defs: &[FuelDefRaw], id: i32) -> &str {
    defs.iter()
        .find(|d| d.id == id)
        .map(|d| d.name.as_str())
        .unwrap_or("unknown")
}
