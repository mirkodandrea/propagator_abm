//! Scenario selector UI panel shown at startup.

use crate::sim::Sim;
use crate::AppState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use fire::Weather;
use scenario::ScenarioRegistry;
use std::path::PathBuf;

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
///
/// Web builds embed exactly one scenario at compile time (see
/// `crates/scenario/build.rs`) and have no filesystem to discover a registry
/// from, so there is nothing to choose between: skip the UI outright.
#[cfg(target_arch = "wasm32")]
pub fn init_selector(mut selector: ResMut<ScenarioSelector>) {
    selector.confirmed = true;
}

#[cfg(not(target_arch = "wasm32"))]
pub fn init_selector(data_path: Res<DataPath>, mut selector: ResMut<ScenarioSelector>) {
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
    if !selector.confirmed {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    if selector.selected.is_none() {
        return;
    }

    // Web builds embed a single scenario at compile time and never populate
    // `selected` (there is nothing to choose); `Scenario::load` on wasm32
    // reads that embedded scenario directly, ignoring the id.
    let scenario_id = selector
        .selected
        .clone()
        .unwrap_or_else(|| "web".to_string());

    #[cfg(not(target_arch = "wasm32"))]
    let result = scenario::Scenario::load_by_id(&data_path.0, &scenario_id);
    #[cfg(target_arch = "wasm32")]
    let result = scenario::Scenario::load(&data_path.0);

    // Load scenario
    match result {
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
                        win.title =
                            format!("{} — wildfire incident command", sim.scenario.metadata.name);
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

/// The scenario chooser.
///
/// Also the screen the game returns to from Scenario ▸ Load scenario…, which is
/// why it is a `CentralPanel` with no close button rather than a floating
/// window: closing it mid-game used to mean "launch the default", which is a
/// destructive answer to an accidental click on a ✕.
pub fn show_selector_ui(
    mut contexts: EguiContexts,
    mut selector: ResMut<ScenarioSelector>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if selector.confirmed || selector.registry.is_none() {
        return;
    }

    let registry = match &selector.registry {
        Some(r) => r,
        None => return,
    };

    // Prepare data to avoid borrow conflicts
    let scenarios_data: Vec<(
        String,
        String,
        usize,
        usize,
        usize,
        String,
        String,
        [usize; 2],
    )> = registry
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
    let mut launch = false;
    let ids: Vec<&str> = scenarios_data.iter().map(|row| row.0.as_str()).collect();
    let current = selector
        .selected
        .as_deref()
        .and_then(|id| ids.iter().position(|candidate| *candidate == id))
        .unwrap_or(0);
    if !ids.is_empty()
        && (keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::ArrowUp))
    {
        let next = if keys.just_pressed(KeyCode::ArrowDown) {
            (current + 1) % ids.len()
        } else {
            (current + ids.len() - 1) % ids.len()
        };
        selector.selected = Some(ids[next].to_string());
    }
    if keys.just_pressed(KeyCode::Enter) && selector.selected.is_some() {
        launch = true;
    }

    // Not closable: this is a screen, not a dialogue. It is also reachable
    // again mid-game from Scenario ▸ Load scenario…, so it has to stand on its
    // own rather than assume it is the first thing the player ever sees.
    egui::CentralPanel::default().show(contexts.ctx_mut(), |ui| {
        ui.add_space(ui.available_height() * 0.12);
        ui.vertical_centered(|ui| {
            ui.heading("Wildfire incident command");
            ui.label(egui::RichText::new("Choose a scenario to command").weak());
        });
        ui.add_space(16.0);

        let width = 560.0f32.min(ui.available_width() - 40.0);
        ui.vertical_centered(|ui| {
            ui.allocate_ui(egui::vec2(width, ui.available_height()), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(ui.available_height() - 70.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (
                            id,
                            name,
                            buildings,
                            households,
                            people,
                            location,
                            description,
                            grid_size,
                        ) in scenarios_data
                        {
                            let is_selected = selector.selected.as_ref() == Some(&id);
                            let is_test = id.starts_with("test_");

                            // One frame per scenario, so the metadata belongs to
                            // the card it describes. The old list put the detail
                            // *below* the row, which meant selecting a different
                            // scenario reflowed everything under the cursor.
                            let frame = egui::Frame::group(ui.style())
                                .fill(if is_selected {
                                    ui.visuals().selection.bg_fill.gamma_multiply(0.35)
                                } else {
                                    ui.visuals().faint_bg_color
                                })
                                .stroke(if is_selected {
                                    egui::Stroke::new(1.5, ui.visuals().selection.stroke.color)
                                } else {
                                    ui.visuals().widgets.noninteractive.bg_stroke
                                });

                            let r = frame.show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&name).strong().size(15.0));
                                    if is_test {
                                        ui.colored_label(egui::Color32::YELLOW, "🔧 DEV");
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.small(format!("📍 {location}"));
                                        },
                                    );
                                });
                                ui.small(description.as_str());
                                ui.add_space(2.0);
                                ui.small(format!(
                                    "{buildings} buildings · {households} households · \
                                     {people} people · {}×{} cells @ 20 m",
                                    grid_size[0], grid_size[1]
                                ));
                            });

                            // The whole card is the hit target, not just its
                            // title: a click that lands between two words of the
                            // description and does nothing reads as a broken UI.
                            let hit = ui.interact(
                                r.response.rect,
                                egui::Id::new(("scenario_card", &id)),
                                egui::Sense::click(),
                            );
                            if hit.clicked() {
                                selector.selected = Some(id.clone());
                            }
                            if hit.double_clicked() {
                                selector.selected = Some(id.clone());
                                launch = true;
                            }
                            ui.add_space(6.0);
                        }
                    });

                ui.add_space(10.0);
                let has_selection = selector.selected.is_some();
                if ui
                    .add_enabled(
                        has_selection,
                        egui::Button::new(egui::RichText::new("▶  Launch").size(15.0))
                            .min_size(egui::vec2(ui.available_width(), 34.0)),
                    )
                    .on_hover_text("Or double-click a scenario.")
                    .clicked()
                {
                    launch = true;
                }
                ui.vertical_centered(|ui| {
                    ui.weak("↑ ↓ choose · Enter launch · double-click a card to launch");
                });
            });
        });
    });

    if launch {
        selector.confirmed = true;
        if selector.selected.is_none() {
            selector.selected = Some(default_id);
        }
    }
}
