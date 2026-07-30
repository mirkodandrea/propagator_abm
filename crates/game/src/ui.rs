//! Control panel: playback, time acceleration, and the incident readout.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use fire::CellFire;
use scenario::Pos;

use crate::fire_view::FireLayer;
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

pub fn panel(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    mut layer: ResMut<FireLayer>,
    mut focus: ResMut<UiFocus>,
) {
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
    let peak_hazard = sim.fire.hazard().peak();
    let evac = sim.agents.stats();
    let median_evac = sim.agents.median_evacuation_s();
    let ordered = sim.agents.households.iter().filter(|h| h.ordered).count();
    let ignition_pos = sim.scenario.world.centre_of(sim.ignition.centre);
    let mut order: Option<(Pos, f32)> = None;
    let clock = sim.clock();
    let playing = sim.playing;
    let mut speed = sim.speed;
    let mut toggle = false;
    let mut selected = *layer;

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
            ui.label("Fire layer");
            ui.horizontal_wrapped(|ui| {
                for (i, l) in FireLayer::ALL.iter().enumerate() {
                    let label = format!("{}  {}", i + 1, l.label());
                    if ui.selectable_label(selected == *l, label).clicked() {
                        selected = *l;
                    }
                }
            });
            ui.small(selected.legend());

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
                ui.label("Peak spread risk");
                ui.label(format!("{:.0}% next step", peak_hazard * 100.0));
                ui.end_row();
                ui.label("Households threatened");
                ui.label(format!("{threatened}"));
                ui.end_row();
                ui.label("Structures lost");
                ui.label(format!("{lost}"));
                ui.end_row();
            });

            ui.separator();
            ui.label("Evacuation");
            egui::Grid::new("evac").num_columns(2).show(ui, |ui| {
                ui.label("Ordered out");
                ui.label(format!("{ordered} households"));
                ui.end_row();
                ui.label("Milling / preparing");
                ui.label(format!("{}", evac.preparing));
                ui.end_row();
                ui.label("On the road");
                ui.label(format!(
                    "{} households · {} cars · {} on foot",
                    evac.moving, evac.cars_moving, evac.on_foot
                ));
                ui.end_row();
                ui.label("Out");
                ui.label(format!("{} households · {} people", evac.safe, evac.people_safe));
                ui.end_row();
                ui.label("Staying to defend");
                ui.label(format!("{}", evac.defending));
                ui.end_row();
                ui.label("Cut off / trapped");
                ui.label(format!("{}", evac.cutoff));
                ui.end_row();
                ui.label("Casualties");
                ui.label(format!("{} households", evac.casualties));
                ui.end_row();
                ui.label("Median time out");
                ui.label(match median_evac {
                    Some(s) => format!("{:.0} min", s / 60.0),
                    None => "—".to_string(),
                });
                ui.end_row();
            });

            ui.horizontal(|ui| {
                if ui
                    .button("Evacuate 2 km around the fire")
                    .on_hover_text(
                        "Households still hear the order over their own channel, \
                         and still have to decide and pack.",
                    )
                    .clicked()
                {
                    order = Some((ignition_pos, 2000.0));
                }
                if ui.button("Evacuate everyone").clicked() {
                    order = Some((ignition_pos, 20_000.0));
                }
            });

            ui.separator();
            ui.small(
                "space play/pause · [ ] speed · 1-4 fire layer · e evacuate · \
                 drag orbit · right-drag pan · scroll zoom",
            );
        });

    if let Some((centre, radius)) = order {
        let n = sim.agents.order_evacuation(centre, radius);
        info!("evacuation order issued to {n} households within {radius:.0} m");
    }
    if toggle {
        sim.playing = !sim.playing;
    }
    if selected != *layer {
        *layer = selected;
    }
    if (speed - sim.speed).abs() > f32::EPSILON {
        sim.speed = speed.clamp(MIN_SPEED, MAX_SPEED);
    }

    focus.0 = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}
