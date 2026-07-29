//! Control panel: playback, time acceleration, and the incident readout.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use fire::CellFire;

use crate::sim::Sim;

/// Speed presets, in simulated seconds per wall-clock second. An initial
/// attack runs for hours of simulated time, so the useful range spans three
/// orders of magnitude and the slider has to be logarithmic to be usable.
pub const MIN_SPEED: f32 = 1.0;
pub const MAX_SPEED: f32 = 512.0;
const PRESETS: [(f32, &str); 5] =
    [(1.0, "1x"), (8.0, "8x"), (30.0, "30x"), (120.0, "2min/s"), (512.0, "max")];

/// True while the cursor is over the panel, so the camera does not orbit when
/// the user is dragging the slider.
#[derive(Resource, Default)]
pub struct UiFocus(pub bool);

pub fn panel(mut contexts: EguiContexts, mut sim: ResMut<Sim>, mut focus: ResMut<UiFocus>) {
    let ctx = contexts.ctx_mut();

    let burning = sim.fire.state().iter().filter(|s| **s == CellFire::Burning).count();
    let burnt = sim.fire.state().iter().filter(|s| **s == CellFire::Burnt).count();
    let cell_ha = (sim.scenario.world.cellsize * sim.scenario.world.cellsize) / 10_000.0;
    let front = sim.fire.active_cells().len();
    let peak_fli = sim
        .fire
        .active_cells()
        .iter()
        .map(|c| sim.fire.cell_intensity(*c))
        .fold(0.0f32, f32::max);
    let threatened = sim.fire.exposure().threatened(0.05).count();
    let lost = sim.fire.exposure().fields().iter().filter(|f| f.alight).count();
    let clock = sim.clock();
    let playing = sim.playing;
    let mut speed = sim.speed;
    let mut toggle = false;

    egui::Window::new("Incident")
        .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(format!("T+{clock}"));
                ui.add_space(8.0);
                if ui
                    .button(if playing { "⏸ Pause" } else { "▶ Play" })
                    .clicked()
                {
                    toggle = true;
                }
            });

            ui.separator();
            ui.label("Time acceleration");
            ui.add(
                // No `.suffix()`: egui appends it *after* the custom formatter,
                // which turns "9 min/s" into "9 min/sx".
                egui::Slider::new(&mut speed, MIN_SPEED..=MAX_SPEED)
                    .logarithmic(true)
                    .custom_formatter(|v, _| {
                        // Above a minute per second, wall-clock multipliers stop
                        // being meaningful; show simulated time per second.
                        if v >= 60.0 {
                            format!("{:.0} min/s", v / 60.0)
                        } else {
                            format!("{v:.0}x")
                        }
                    }),
            );
            ui.horizontal(|ui| {
                for (v, label) in PRESETS {
                    if ui.selectable_label((speed - v).abs() < 0.5, label).clicked() {
                        speed = v;
                    }
                }
            });

            ui.separator();
            egui::Grid::new("stats").num_columns(2).show(ui, |ui| {
                ui.label("Burnt");
                ui.label(format!("{:.1} ha", (burning + burnt) as f32 * cell_ha));
                ui.end_row();
                ui.label("Active front");
                ui.label(format!("{front} cells"));
                ui.end_row();
                ui.label("Peak intensity");
                ui.label(format!(
                    "{peak_fli:.0} kW/m  ({:.1} m flames)",
                    fire::exposure::flame_length_m(peak_fli)
                ));
                ui.end_row();
                ui.label("Households threatened");
                ui.label(format!("{threatened}"));
                ui.end_row();
                ui.label("Structures lost");
                ui.label(format!("{lost}"));
                ui.end_row();
            });

            ui.separator();
            ui.small("space play/pause · [ ] speed · drag orbit · right-drag pan · scroll zoom");
        });

    if toggle {
        sim.playing = !sim.playing;
    }
    if (speed - sim.speed).abs() > f32::EPSILON {
        sim.speed = speed.clamp(MIN_SPEED, MAX_SPEED);
    }

    focus.0 = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}
