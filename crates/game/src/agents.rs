//! Household and person entities.
//!
//! Status used to be a floating beacon hovering over every house — 750 of
//! them, which buried the town under permanent markers. It is now read off
//! the building itself: `crate::buildings` glows the structure on hover and
//! lights its windows after dark for whoever is still home, and the
//! Inspector/Entities panels give the same status in words. This module now
//! only supplies the status palette shared with `crate::people` for vehicles.

use bevy::prelude::*;
use scenario::population::Status;

pub fn status_color(status: Status) -> Color {
    match status {
        Status::Normal => Color::srgb(0.35, 0.85, 0.40),
        Status::Warned => Color::srgb(0.95, 0.85, 0.25),
        Status::Preparing => Color::srgb(0.98, 0.65, 0.15),
        Status::Evacuating => Color::srgb(0.30, 0.70, 0.98),
        Status::Evacuated => Color::srgb(0.25, 0.40, 0.85),
        Status::Defending => Color::srgb(0.85, 0.45, 0.90),
        Status::Trapped => Color::srgb(1.00, 0.25, 0.15),
        Status::Casualty => Color::srgb(0.15, 0.15, 0.15),
    }
}

pub fn spawn(sim: Res<crate::sim::Sim>) {
    info!(
        "spawned {} households ({} people)",
        sim.scenario.population.households.len(),
        sim.scenario.population.people.len()
    );
}
