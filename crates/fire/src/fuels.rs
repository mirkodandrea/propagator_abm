//! Turning the scenario's baked `eu_fuel12` table into a core [`FuelSystem`].

use anyhow::Result;
use propagator_core::{FuelDef, FuelSystem};
use scenario::FuelDefRaw;

pub fn eu12_fuel_system(defs: &[FuelDefRaw]) -> Result<FuelSystem> {
    let defs: Vec<FuelDef> = defs
        .iter()
        .map(|f| FuelDef {
            id: f.id,
            name: f.name.clone(),
            v0: f.v0,
            d0: f.d0,
            d1: f.d1,
            hhv: f.hhv,
            humidity: f.humidity,
            spotting: f.spotting,
            prob_ign_by_embers: f.prob_ign_by_embers,
            burn: f.burn,
            spread_probability: f.spread_probability.clone(),
        })
        .collect();
    FuelSystem::from_defs(&defs).map_err(|e| anyhow::anyhow!("{e:?}"))
}
