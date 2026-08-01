//! The application menu bar, and the status strip along the top of it.
//!
//! Everything the game can do is reachable from here in at most two clicks.
//! That is the point: before this, the only route to half the functionality was
//! a keyboard shortcut printed in small text at the bottom of a panel, which
//! means a player who has not read that line does not know the feature exists.
//! A menu is a discoverable index of the whole application — and it is also the
//! only honest place to *document* the shortcuts, next to the thing they do.
//!
//! This system is also where input ownership is decided for the frame. It runs
//! before every other panel and before every shortcut system, and it sets both
//! halves of [`UiFocus`]: `pointer`, reset here and OR-ed into by each panel in
//! turn, and `keyboard`, which is read by every shortcut system as its first
//! act. See [`UiFocus`] for why a single-letter shortcut cannot be safe without
//! it.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::camera::OrbitCamera;
use crate::command::{OrderKind, OrderTool};
use crate::composer::Composer;
use crate::fire_view::FireLayer;
use crate::ignition_edit::{EditMode, IgnitionTool};
use crate::scenario_selector::ScenarioSelector;
use crate::sim::{Sim, SimRestarted};
use crate::ui::{DockTab, HelpUi, PanelState, UiFocus, MAX_SPEED, MIN_SPEED, PRESETS};
use crate::AppState;

/// What a menu item asked for. Collected rather than applied inline: the menu
/// closure borrows half a dozen resources at once, and threading `&mut Sim`
/// through it to run a restart from inside a nested `menu_button` is how a
/// borrow-checker fight becomes a design.
#[derive(Clone, Copy, PartialEq)]
enum Action {
    TogglePlay,
    Step,
    Speed(f32),
    Layer(FireLayer),
    Restart,
    LoadScenario,
    Quit,
    EvacuateNear,
    EvacuateAll,
    NextUnit,
    Arm(OrderKind),
    StandDown,
    RequestAir,
    Cancel,
    ArmIgnition,
    Composer,
    Tab(DockTab),
    Help,
    CentreOnFire,
    Overview,
}

/// Height of the status strip's clock, in points. Large enough to read at a
/// glance from across a desk, which is what a wall clock in an operations room
/// is for.
const CLOCK_SIZE: f32 = 15.0;

#[allow(clippy::too_many_arguments)]
pub fn menubar(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    mut layer: ResMut<FireLayer>,
    mut panels: ResMut<PanelState>,
    mut focus: ResMut<UiFocus>,
    mut help: ResMut<HelpUi>,
    mut ignition: ResMut<IgnitionTool>,
    mut order: ResMut<OrderTool>,
    mut composer: ResMut<Composer>,
    mut restarted: EventWriter<SimRestarted>,
    mut next_state: ResMut<NextState<AppState>>,
    mut selector: ResMut<ScenarioSelector>,
    mut quit: EventWriter<AppExit>,
    mut camera: Query<&mut OrbitCamera>,
) {
    let ctx = contexts.ctx_mut();
    let mut act: Option<Action> = None;
    let a = |action: Action, slot: &mut Option<Action>| *slot = Some(action);

    let playing = sim.playing;
    let speed = sim.speed;
    let clock = sim.clock();
    let current_layer = *layer;
    let armed_order = order.armed;
    let placing = ignition.mode == EditMode::Place;
    let selected_unit = order.selected;
    let unit_kind = selected_unit.and_then(|id| sim.crews.units.get(id)).map(|u| u.kind);

    egui::TopBottomPanel::top("menubar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Scenario", |ui| {
                if item(ui, "Load scenario…", "").clicked() {
                    a(Action::LoadScenario, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                if item(ui, "Restart incident", "R")
                    .on_hover_text(
                        "Relight at T+0 with the current weather, seed and ignitions.",
                    )
                    .clicked()
                {
                    a(Action::Restart, &mut act);
                    ui.close_menu();
                }
                if item(ui, "Fire settings…", "").clicked() {
                    a(Action::Tab(DockTab::Fire), &mut act);
                    ui.close_menu();
                }
                ui.separator();
                if item(ui, "Quit", "").clicked() {
                    a(Action::Quit, &mut act);
                    ui.close_menu();
                }
            });

            ui.menu_button("Simulation", |ui| {
                if item(ui, if playing { "Pause" } else { "Play" }, "Space").clicked() {
                    a(Action::TogglePlay, &mut act);
                    ui.close_menu();
                }
                if item(ui, "Step one decision", ".")
                    .on_hover_text(
                        "Advance far enough for every agent to decide exactly once, whether \
                         or not the clock is running. The granularity a behaviour is \
                         authored at.",
                    )
                    .clicked()
                {
                    a(Action::Step, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                ui.label(egui::RichText::new("SPEED").small().weak());
                for (v, label) in PRESETS {
                    if ui
                        .radio(near(speed, v), format!("{label}  ({v:.0}x)"))
                        .clicked()
                    {
                        a(Action::Speed(v), &mut act);
                        ui.close_menu();
                    }
                }
                ui.separator();
                if item(ui, "Slower", "[").clicked() {
                    a(Action::Speed((speed / 2.0).max(MIN_SPEED)), &mut act);
                    ui.close_menu();
                }
                if item(ui, "Faster", "]").clicked() {
                    a(Action::Speed((speed * 2.0).min(MAX_SPEED)), &mut act);
                    ui.close_menu();
                }
            });

            ui.menu_button("Orders", |ui| {
                ui.label(egui::RichText::new("CIVILIANS").small().weak());
                if item(ui, "Evacuate 2 km around the fire", "").clicked() {
                    a(Action::EvacuateNear, &mut act);
                    ui.close_menu();
                }
                if item(ui, "Evacuate everyone", "E").clicked() {
                    a(Action::EvacuateAll, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                ui.label(egui::RichText::new("UNITS").small().weak());
                if item(ui, "Next unit", "Tab").clicked() {
                    a(Action::NextUnit, &mut act);
                    ui.close_menu();
                }
                // Greyed out with a reason rather than hidden: which orders a
                // unit can take is one of the three constraints the whole game
                // is about, and a menu that quietly omits "cut line" for an
                // engine teaches nothing.
                for kind in [OrderKind::Attack, OrderKind::Line, OrderKind::Drop] {
                    let ok = unit_kind.map_or(false, |k| kind.allowed_for(k));
                    let key = match kind {
                        OrderKind::Attack => "A",
                        OrderKind::Line => "L",
                        OrderKind::Drop => "D",
                    };
                    let label = if armed_order == Some(kind) {
                        format!("▶ {}", kind.label())
                    } else {
                        kind.label().to_string()
                    };
                    let r = ui.add_enabled(ok, shortcut_button(&label, key));
                    let r = if unit_kind.is_none() {
                        r.on_disabled_hover_text("Select a unit first.")
                    } else if !ok {
                        r.on_disabled_hover_text("This unit cannot take that order.")
                    } else {
                        r.on_hover_text("Then click the ground to place it.")
                    };
                    if r.clicked() {
                        a(Action::Arm(kind), &mut act);
                        ui.close_menu();
                    }
                }
                if ui
                    .add_enabled(selected_unit.is_some(), shortcut_button("Stand down", "X"))
                    .clicked()
                {
                    a(Action::StandDown, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                if item(ui, "✈ Request air support", "C")
                    .on_hover_text("25 minutes out. Ask early.")
                    .clicked()
                {
                    a(Action::RequestAir, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                if item(ui, "Cancel active tool", "Esc").clicked() {
                    a(Action::Cancel, &mut act);
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                ui.label(egui::RichText::new("FIRE LAYER").small().weak());
                for (i, l) in FireLayer::ALL.iter().enumerate() {
                    if ui
                        .radio(current_layer == *l, format!("{}   ({})", l.label(), i + 1))
                        .on_hover_text(l.legend())
                        .clicked()
                    {
                        a(Action::Layer(*l), &mut act);
                        ui.close_menu();
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new("PANELS").small().weak());
                ui.checkbox(&mut panels.incident, "Incident");
                ui.checkbox(&mut panels.dock, "Right dock");
                ui.checkbox(&mut panels.inspector, "Inspector");
                ui.separator();
                ui.label(egui::RichText::new("CAMERA").small().weak());
                if item(ui, "Centre on the fire", "").clicked() {
                    a(Action::CentreOnFire, &mut act);
                    ui.close_menu();
                }
                if item(ui, "Whole scenario", "").clicked() {
                    a(Action::Overview, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                ui.small("Arrow keys pan · drag orbits · right-drag pans · scroll zooms");
            });

            ui.menu_button("Tools", |ui| {
                let r = ui.add(shortcut_button(
                    if placing {
                        "▶ Place ignition"
                    } else {
                        "Place ignition"
                    },
                    "I",
                ));
                if r.on_hover_text("Then click the map to light a patch.").clicked() {
                    a(Action::ArmIgnition, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                if item(ui, "Agent Behaviour Composer", "G")
                    .on_hover_text(
                        "Author the decision model for households, separated people or \
                         suppression units as a node graph — and watch the selected agent \
                         run it.",
                    )
                    .clicked()
                {
                    a(Action::Composer, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                for tab in DockTab::ALL {
                    if item(ui, tab.label(), "").on_hover_text(tab.hint()).clicked() {
                        a(Action::Tab(tab), &mut act);
                        ui.close_menu();
                    }
                }
            });

            ui.menu_button("Help", |ui| {
                if item(ui, "Quick start / Guida", "F1").clicked() {
                    a(Action::Help, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                shortcut_table(ui);
            });

            // The status strip: the clock, the transport, and the speed, in the
            // one place that is always visible whatever else is collapsed.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                if ui
                    .selectable_label(false, speed_label(speed))
                    .on_hover_text("Time acceleration — [ and ] step it")
                    .clicked()
                {
                    // Cycling the presets is the fast gesture; the full
                    // logarithmic slider lives in Simulation ▸ Speed.
                    a(Action::Speed(next_preset(speed)), &mut act);
                }
                if ui
                    .button("⏭")
                    .on_hover_text(
                        "Step one decision (.) — every agent decides exactly once, paused \
                         or not",
                    )
                    .clicked()
                {
                    a(Action::Step, &mut act);
                }
                if ui
                    .button(if playing { "⏸" } else { "▶" })
                    .on_hover_text(if playing { "Pause (Space)" } else { "Play (Space)" })
                    .clicked()
                {
                    a(Action::TogglePlay, &mut act);
                }
                ui.label(
                    egui::RichText::new(format!("T+{clock}"))
                        .size(CLOCK_SIZE)
                        .monospace()
                        .strong(),
                );
                // A live reminder of what the next left-click will do. The
                // three map tools are mutually exclusive and invisible from the
                // map itself; an armed tool the player has forgotten about is
                // the one way to light a fire by accident.
                if let Some(hint) = armed_hint(placing, armed_order, order.line_from.is_some()) {
                    ui.colored_label(egui::Color32::from_rgb(255, 180, 70), hint)
                        .on_hover_text("Esc cancels");
                }
            });
        });
    });

    // --- apply -------------------------------------------------------------
    if let Some(action) = act {
        match action {
            Action::TogglePlay => sim.playing = !sim.playing,
            Action::Step => sim.request_step(),
            Action::Speed(v) => sim.speed = v.clamp(MIN_SPEED, MAX_SPEED),
            Action::Layer(l) => *layer = l,
            Action::Restart => match sim.restart() {
                Ok(()) => {
                    restarted.send(SimRestarted);
                }
                Err(e) => error!("restart failed: {e:#}"),
            },
            Action::LoadScenario => {
                // Back to the selector, which tears the scene down on the way
                // out (`crate::teardown_scene`) and rebuilds it on the way in.
                selector.confirmed = false;
                next_state.set(AppState::SelectingScenario);
            }
            Action::Quit => {
                quit.send(AppExit::Success);
            }
            Action::EvacuateNear => {
                let centre = sim.scenario.world.centre_of(sim.ignition.centre);
                let n = sim.agents.order_evacuation(centre, 2000.0);
                info!("evacuation ordered within 2 km: {n} households");
            }
            Action::EvacuateAll => {
                let n = sim.agents.order_evacuation_all();
                info!("general evacuation ordered: {n} households");
            }
            Action::NextUnit => {
                let n = sim.crews.units.len();
                let start = order.selected.map(|s| s + 1).unwrap_or(0);
                order.selected = (0..n)
                    .map(|k| (start + k) % n)
                    .find(|id| sim.crews.units[*id].assignable());
                panels.focus_tab(DockTab::Units);
            }
            Action::Arm(kind) => {
                ignition.mode = EditMode::Off;
                order.toggle(kind);
                panels.focus_tab(DockTab::Units);
            }
            Action::StandDown => {
                if let Some(id) = order.selected {
                    let _ = sim.crews.assign(id, abm::suppression::Task::Return);
                }
            }
            Action::RequestAir => {
                let n = sim.crews.request_air();
                info!("air support requested: {n} aircraft");
            }
            Action::Cancel => {
                ignition.mode = EditMode::Off;
                order.disarm();
            }
            Action::ArmIgnition => {
                order.disarm();
                ignition.mode = if placing {
                    EditMode::Off
                } else {
                    EditMode::Place
                };
                if ignition.mode == EditMode::Place {
                    panels.focus_tab(DockTab::Fire);
                }
            }
            Action::Composer => composer.open = !composer.open,
            Action::Tab(tab) => panels.focus_tab(tab),
            Action::Help => help.open = true,
            Action::CentreOnFire => {
                if let Ok(mut orbit) = camera.get_single_mut() {
                    let (p, h) = crate::terrain_mesh::cell_ground(&sim.scenario, sim.ignition.centre);
                    orbit.focus = crate::frame::to_bevy(p, h);
                    orbit.distance = 1400.0;
                }
            }
            Action::Overview => {
                if let Ok(mut orbit) = camera.get_single_mut() {
                    let w = &sim.scenario.world;
                    let p = scenario::Pos {
                        x: w.width_m * 0.5,
                        y: w.height_m * 0.5,
                    };
                    let h = sim.scenario.terrain.height_at(p);
                    orbit.focus = crate::frame::to_bevy(p, h);
                    orbit.distance = w.width_m.max(w.height_m) * 1.1;
                }
            }
        }
    }

    // Input ownership for the frame. `pointer` is assigned (not OR-ed) because
    // this is the first UI system to run; every panel after it ORs its own
    // area in. `keyboard` is egui's own answer to "is a widget taking these
    // keystrokes", which is the only reliable one — it covers text fields,
    // drag values and the node editor's inline edits alike.
    focus.pointer = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
    focus.keyboard = ctx.wants_keyboard_input();
}

/// A menu row with its shortcut right-aligned in grey — the shape every desktop
/// menu uses, and the reason a player ever learns the keyboard at all.
fn item(ui: &mut egui::Ui, label: &str, key: &str) -> egui::Response {
    ui.add(shortcut_button(label, key))
}

/// A borrowed-label menu button. `frame(false)` is what makes a column of them
/// read as menu rows rather than as a column of buttons.
fn shortcut_button<'a>(label: &'a str, key: &'a str) -> impl egui::Widget + 'a {
    move |ui: &mut egui::Ui| {
        ui.add(
            egui::Button::new(label)
                .shortcut_text(key)
                .frame(false)
                .min_size(egui::vec2(ui.available_width().min(240.0), 0.0)),
        )
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn speed_label(speed: f32) -> String {
    if speed >= 60.0 {
        format!("{:.0} min/s", speed / 60.0)
    } else {
        format!("{speed:.0}x")
    }
}

/// The next preset above the current speed, wrapping at the top.
fn next_preset(speed: f32) -> f32 {
    PRESETS
        .iter()
        .map(|(v, _)| *v)
        .find(|v| *v > speed + 0.5)
        .unwrap_or(PRESETS[0].0)
}

/// What an armed left-click will do, for the status strip.
fn armed_hint(placing: bool, order: Option<OrderKind>, line_started: bool) -> Option<String> {
    if placing {
        return Some("▶ click to light a fire".into());
    }
    match order {
        Some(OrderKind::Line) if line_started => Some("▶ click where the line ends".into()),
        Some(OrderKind::Line) => Some("▶ click where the line starts".into()),
        Some(k) => Some(format!("▶ click to order: {}", k.label().to_lowercase())),
        None => None,
    }
}

/// The whole keyboard, in the menu that is meant to answer "what can I press".
/// Kept here rather than only in the help window because the help window is
/// modal and this is a glance.
fn shortcut_table(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("KEYBOARD").small().weak());
    egui::Grid::new("menu_shortcuts")
        .num_columns(2)
        .spacing([16.0, 2.0])
        .show(ui, |ui| {
            for (key, what) in [
                ("Space", "play / pause"),
                (".", "step one decision"),
                ("[  ]", "slower / faster"),
                ("1 – 4", "fire layer"),
                ("Arrows", "pan the camera"),
                ("E", "evacuate everyone"),
                ("I", "place an ignition"),
                ("R", "restart the incident"),
                ("Tab", "next unit"),
                ("A / L / D", "attack / line / drop"),
                ("X", "stand down"),
                ("C", "request air support"),
                ("B", "entities"),
                ("G", "behaviour composer"),
                ("F1", "help"),
                ("F12", "screenshot"),
                ("Esc", "cancel the active tool"),
            ] {
                ui.label(egui::RichText::new(key).monospace());
                ui.label(what);
                ui.end_row();
            }
        });
}
