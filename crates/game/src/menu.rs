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
use crate::inspect::Selected;
use crate::scenario_selector::ScenarioSelector;
use crate::sim::{Sim, SimRestarted};
use crate::ui::{
    BottomTab, DockTab, HelpUi, PanelPlacement, PanelState, UiFocus, MAX_SPEED, MIN_SPEED, PRESETS,
};
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
    Interview,
    LlmSettings,
    Tab(DockTab),
    Bottom(BottomTab),
    Help,
    Shortcuts,
    Debug,
    FocusSelection,
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
    mut interview: ResMut<crate::interview::Interview>,
    selected: Res<Selected>,
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
    let unit_kind = selected_unit
        .and_then(|id| sim.crews.units.get(id))
        .map(|u| u.kind);

    egui::TopBottomPanel::top("menubar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Scenario", |ui| {
                if item(ui, "Load scenario…", "").clicked() {
                    a(Action::LoadScenario, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                if item(ui, "Restart incident", "Ctrl/⌘+R")
                    .on_hover_text("Relight at T+0 with the current weather, seed and ignitions.")
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

            ui.menu_button("Operations", |ui| {
                ui.label(egui::RichText::new("CIVILIANS").small().weak());
                if item(ui, "Evacuate 2 km around the fire", "").clicked() {
                    a(Action::EvacuateNear, &mut act);
                    ui.close_menu();
                }
                if item(ui, "Evacuate everyone", "Shift+E").clicked() {
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
                let r = ui.add(shortcut_button(
                    if placing {
                        "▶ Place ignition"
                    } else {
                        "Place ignition"
                    },
                    "I",
                ));
                if r.on_hover_text("Then click the map to light a patch.")
                    .clicked()
                {
                    a(Action::ArmIgnition, &mut act);
                    ui.close_menu();
                }
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
                ui.label(egui::RichText::new("WORKSPACE").small().weak());
                placement_menu(ui, "Command controls", &mut panels.dock);
                placement_menu(ui, "Entities & detail", &mut panels.inspector);
                placement_menu(ui, "Bottom workbench", &mut panels.incident);
                ui.menu_button("Bottom tab", |ui| {
                    for tab in BottomTab::ALL {
                        if ui
                            .selectable_label(panels.bottom_tab == tab, tab.label())
                            .clicked()
                        {
                            a(Action::Bottom(tab), &mut act);
                            ui.close_menu();
                        }
                    }
                });
                if item(ui, "Reset panel layout", "").clicked() {
                    panels.reset_layout();
                    composer.open = false;
                    interview.open = false;
                    ui.close_menu();
                }
                ui.separator();
                ui.label(egui::RichText::new("NAVIGATION").small().weak());
                if ui
                    .add_enabled(
                        selected.target.is_some(),
                        shortcut_button("Focus selection", "F"),
                    )
                    .clicked()
                {
                    a(Action::FocusSelection, &mut act);
                    ui.close_menu();
                }
                if item(ui, "Centre on the fire", "Shift+F").clicked() {
                    a(Action::CentreOnFire, &mut act);
                    ui.close_menu();
                }
                if item(ui, "Whole scenario", "Home").clicked() {
                    a(Action::Overview, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                ui.small("Drag orbit · Shift/right-drag pan · scroll zoom · arrows pan");
            });

            ui.menu_button("Debug", |ui| {
                if item(
                    ui,
                    if panels.bottom_tab == BottomTab::Debug && panels.incident.visible() {
                        "Hide developer diagnostics"
                    } else {
                        "Developer diagnostics"
                    },
                    "F2",
                )
                .clicked()
                {
                    a(Action::Debug, &mut act);
                    ui.close_menu();
                }
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
                ui.label(egui::RichText::new("INTERVIEW").small().weak());
                if ui
                    .add_enabled(
                        selected.target.is_some(),
                        shortcut_button("💬 Talk to the selected agent", "T"),
                    )
                    .on_hover_text(
                        "Ask this household, person or crew what they are doing and why, in \
                         their own words. Pauses the incident. They know only their own day.",
                    )
                    .on_disabled_hover_text("Select an agent on the map first.")
                    .clicked()
                {
                    a(Action::Interview, &mut act);
                    ui.close_menu();
                }
                if item(ui, "LLM settings…", "")
                    .on_hover_text(
                        "Which model answers an interview: OpenRouter with a key, or a local \
                         Ollama.",
                    )
                    .clicked()
                {
                    a(Action::LlmSettings, &mut act);
                    ui.close_menu();
                }
                ui.separator();
                ui.label("F12 saves the current frame as a PNG.");
                ui.small(
                    "Diagnostics are read-only; simulation editing stays in Fire and Composer.",
                );
            });

            ui.menu_button("Help", |ui| {
                if item(ui, "Quick start / Guida", "F1").clicked() {
                    a(Action::Help, &mut act);
                    ui.close_menu();
                }
                if item(ui, "Keyboard shortcuts", "?").clicked() {
                    a(Action::Shortcuts, &mut act);
                    ui.close_menu();
                }
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
                    .on_hover_text(if playing {
                        "Pause (Space)"
                    } else {
                        "Play (Space)"
                    })
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

        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("MAP").small().weak());
            for (i, map_layer) in FireLayer::ALL.iter().enumerate() {
                if ui
                    .selectable_label(
                        current_layer == *map_layer,
                        format!("{}  {}", i + 1, map_layer.label()),
                    )
                    .on_hover_text(map_layer.legend())
                    .clicked()
                {
                    a(Action::Layer(*map_layer), &mut act);
                }
            }
            ui.separator();
            if ui
                .add_enabled(selected.target.is_some(), egui::Button::new("⌖ Selection"))
                .on_hover_text("Focus the selected entity (F)")
                .clicked()
            {
                a(Action::FocusSelection, &mut act);
            }
            if ui
                .button("🔥 Fire")
                .on_hover_text("Centre on the opening fire (Shift+F)")
                .clicked()
            {
                a(Action::CentreOnFire, &mut act);
            }
            if ui
                .button("Overview")
                .on_hover_text("Show the whole scenario (Home)")
                .clicked()
            {
                a(Action::Overview, &mut act);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak("/ find entity · ? shortcuts");
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
            Action::Composer => {
                if composer.open && panels.bottom_tab == BottomTab::Behaviour {
                    composer.open = false;
                    panels.focus_bottom(BottomTab::Incident);
                } else {
                    composer.open = true;
                    panels.focus_bottom(BottomTab::Behaviour);
                }
            }
            Action::Interview => {
                match selected
                    .target
                    .and_then(|t| crate::interview::subject_of(&sim, t))
                {
                    Some(subject) => {
                        interview.open_for(subject);
                        panels.focus_bottom(BottomTab::Chat);
                    }
                    None => interview.status = "select an agent on the map first".to_string(),
                }
            }
            Action::LlmSettings => interview.settings_open = true,
            Action::Tab(tab) => panels.focus_tab(tab),
            Action::Bottom(tab) => {
                panels.focus_bottom(tab);
                composer.open = tab == BottomTab::Behaviour;
                interview.open = tab == BottomTab::Chat && interview.subject.is_some();
            }
            Action::Help => help.open = true,
            Action::Shortcuts => help.shortcuts_open = !help.shortcuts_open,
            Action::Debug => {
                if panels.bottom_tab == BottomTab::Debug && panels.incident.visible() {
                    panels.incident = PanelPlacement::Hidden;
                } else {
                    panels.focus_bottom(BottomTab::Debug);
                }
            }
            Action::FocusSelection => {
                if let (Some(target), Ok(mut orbit)) = (selected.target, camera.get_single_mut()) {
                    if let Some(p) = crate::inspect::target_pos(&sim, target) {
                        let h = sim.scenario.terrain.height_at(p);
                        orbit.focus = crate::frame::to_bevy(p, h);
                        orbit.distance = orbit.distance.clamp(120.0, 650.0);
                    }
                }
            }
            Action::CentreOnFire => {
                if let Ok(mut orbit) = camera.get_single_mut() {
                    let (p, h) =
                        crate::terrain_mesh::cell_ground(&sim.scenario, sim.ignition.centre);
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
    // An open interview owns the keyboard outright, whether or not the question
    // box currently holds egui's focus. `wants_keyboard_input` alone is not
    // enough here: the moment focus slips off that box — clicking the play
    // button, pressing Enter, a frame where the widget was disabled — every
    // letter of the next question becomes a map shortcut, so typing "we need to
    // evacuate" silently orders a general evacuation, restarts the incident and
    // arms two tools. That is finding 25 with a text field in front of it, and
    // the fix is the same: one system decides ownership, and a conversation is
    // not a moment for single-key commands. Esc is unaffected — it is what gets
    // you out — and the menu bar's own buttons still work with the mouse.
    focus.keyboard = ctx.wants_keyboard_input() || interview.open;
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

fn placement_menu(ui: &mut egui::Ui, title: &str, placement: &mut PanelPlacement) {
    let state = match *placement {
        PanelPlacement::Docked => "shown",
        PanelPlacement::Hidden => "hidden",
    };
    ui.menu_button(format!("{title}  · {state}"), |ui| {
        ui.radio_value(placement, PanelPlacement::Docked, "Show");
        ui.radio_value(placement, PanelPlacement::Hidden, "Hide");
    });
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
pub(crate) fn armed_hint(
    placing: bool,
    order: Option<OrderKind>,
    line_started: bool,
) -> Option<String> {
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
