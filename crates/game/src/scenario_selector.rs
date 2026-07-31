//! Scenario selector UI panel shown at startup.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use scenario::ScenarioRegistry;
use std::path::PathBuf;
use fire::Weather;
use crate::sim::Sim;
use crate::AppState;

/// Wrapper for data directory path
#[derive(Resource, Clone)]
pub struct DataPath(pub PathBuf);

/// State for the scenario selector
#[derive(Resource)]
pub struct ScenarioSelector {
    pub registry: Option<ScenarioRegistry>,
    pub selected: Option<String>,
    pub confirmed: bool,
}

impl Default for ScenarioSelector {
    fn default() -> Self {
        Self {
            registry: None,
            selected: None,
            confirmed: false,
        }
    }
}

/// Initialize the scenario selector with the registry.
pub fn init_selector(
    data_path: Res<DataPath>,
    mut selector: ResMut<ScenarioSelector>,
) {
    if let Ok(registry) = scenario::ScenarioRegistry::discover(&data_path.0) {
        // Auto-select default or env var if present
        if let Ok(id) = std::env::var("SPOTORNO_SCENARIO") {
            selector.selected = Some(id);
            selector.confirmed = true; // Fast path: skip UI for env var
        } else {
            selector.selected = Some(registry.default_id().to_string());
        }
        selector.registry = Some(registry);
    }
}

/// Handle launching the selected scenario.
pub fn handle_launch_selection(
    data_path: Res<DataPath>,
    selector: Res<ScenarioSelector>,
    mut next_state: ResMut<NextState<AppState>>,
    mut commands: Commands,
    mut window: Query<&mut Window>,
) {
    if !selector.confirmed || selector.selected.is_none() {
        return;
    }

    let scenario_id = selector.selected.as_ref().unwrap();

    // Load scenario
    match scenario::Scenario::load_by_id(&data_path.0, scenario_id) {
        Ok(scenario) => {
            println!(
                "scenario: {:.1} x {:.1} km, {} fire cells, {} buildings, {} households",
                scenario.world.width_m / 1000.0,
                scenario.world.height_m / 1000.0,
                scenario.world.fire_rows * scenario.world.fire_cols,
                scenario.vectors.buildings.len(),
                scenario.population.households.len(),
            );

            // Create Sim
            match Sim::new(scenario, Weather::default(), 42) {
                Ok(sim) => {
                    // Update window title
                    if let Ok(mut win) = window.get_single_mut() {
                        win.title = format!("{} — wildfire incident command", sim.scenario.metadata.name);
                    }

                    // Insert Sim resource
                    commands.insert_resource(sim);

                    // Transition to Playing state
                    next_state.set(AppState::Playing);
                }
                Err(e) => {
                    eprintln!("Failed to create Sim: {e:#}");
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to load scenario '{scenario_id}': {e:#}");
        }
    }
}

/// Show the scenario selector window and handle selection.
pub fn show_selector_ui(
    mut contexts: EguiContexts,
    mut selector: ResMut<ScenarioSelector>,
) {
    if selector.confirmed || selector.registry.is_none() {
        return;
    }

    let registry = match &selector.registry {
        Some(r) => r,
        None => return,
    };

    // Prepare data to avoid borrow conflicts
    let scenarios_data: Vec<(String, String, usize, usize, usize, String, String, [usize; 2])> = registry
        .list()
        .iter()
        .map(|s| {
            (
                s.id.clone(),
                s.name.clone(),
                s.buildings_count,
                s.households_count,
                s.people_count,
                s.location.clone(),
                s.description.clone(),
                s.fire_grid_size,
            )
        })
        .collect();

    let default_id = registry.default_id().to_string();

    let mut open = true;
    egui::Window::new("Select Scenario")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(contexts.ctx_mut(), |ui| {
            ui.vertical(|ui| {
                ui.heading("Choose a Scenario");
                ui.separator();

                for (id, name, buildings, households, people, location, description, grid_size) in scenarios_data {
                    let is_selected = selector.selected.as_ref() == Some(&id);
                    let button_text = format!(
                        "{}\n{} buildings · {} households · {} people",
                        name, buildings, households, people
                    );

                    if ui.selectable_label(is_selected, button_text).clicked() {
                        selector.selected = Some(id.clone());
                    }

                    if is_selected {
                        ui.label(format!("📍 {}", location));
                        ui.label(description.as_str());
                        ui.label(format!("Grid: {}×{} cells (20 m)", grid_size[0], grid_size[1]));

                        // Show dev badge for test scenarios
                        if id.starts_with("test_") {
                            ui.colored_label(egui::Color32::YELLOW, "🔧 DEV SCENARIO");
                        }
                    }
                }

                ui.separator();

                // Confirm button
                let has_selection = selector.selected.is_some();
                if ui.add_enabled(has_selection, egui::Button::new("Launch")).clicked() {
                    selector.confirmed = true;
                }
            });
        });

    if !open {
        // User closed the window without selecting - use default
        selector.confirmed = true;
        if selector.selected.is_none() {
            selector.selected = Some(default_id);
        }
    }
}
