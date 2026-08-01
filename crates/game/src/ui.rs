//! The screen furniture: the incident readout on the left, the working dock on
//! the right, and the help window.
//!
//! The layout is deliberately three regions and no more. Everything the player
//! *reads while watching the fire* is the left dock; everything they *do* is
//! either the menu bar (`crate::menu`) or one tab of the right dock; whatever is
//! selected explains itself along the bottom. Before this there were five
//! independent docks competing for the same screen and the 3D view was the
//! narrow strip left over — a panel per feature is a rational way to *build* a
//! UI and a poor way to use one.
//!
//! The right dock is tabbed rather than stacked because its three jobs are
//! mutually exclusive in practice: you are rewriting the scenario (**Fire**),
//! directing units (**Units**), or looking something up (**Entities**). Only one
//! of those is ever the current task, so only one of them should be costing
//! screen space.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use fire::CellFire;
use scenario::Pos;

use crate::fire_view::FireLayer;
use crate::ignition_edit::{clamp_radius, EditMode, IgnitionTool};
use crate::sim::{Sim, SimRestarted, MAX_IGNITION_RADIUS_M, MIN_IGNITION_RADIUS_M};

/// Speed presets, in simulated seconds per wall-clock second. An initial
/// attack runs for hours of simulated time, so the useful range spans three
/// orders of magnitude and the slider has to be logarithmic to be usable.
pub const MIN_SPEED: f32 = 1.0;
pub const MAX_SPEED: f32 = 512.0;
pub const PRESETS: [(f32, &str); 5] = [
    (1.0, "1x"),
    (8.0, "8x"),
    (30.0, "30x"),
    (120.0, "2min/s"),
    (512.0, "max"),
];

/// Who owns the input this frame — the UI, or the map.
///
/// `pointer` is the older of the two: the camera must not orbit while a slider
/// is being dragged. `keyboard` is what makes single-letter shortcuts safe at
/// all. Every shortcut here is one keystroke with no modifier, so without this
/// gate typing "Bergeggi" into the Entities search box toggles the browser,
/// arms an attack, drops a load and restarts the incident — and the same is
/// true of every text field in the behaviour composer. A shortcut system reads
/// this before it reads the keyboard.
#[derive(Resource, Default)]
pub struct UiFocus {
    pub pointer: bool,
    pub keyboard: bool,
}

impl UiFocus {
    /// True when the map should ignore the keyboard: egui has a text field,
    /// drag value or focused widget that wants the keystroke instead.
    pub fn typing(&self) -> bool {
        self.keyboard
    }
}

/// State for the short, player-facing introduction. It opens with the
/// scenario so a first-time player understands the role before unpausing,
/// then remains one click away from the Incident panel.
#[derive(Resource)]
pub struct HelpUi {
    pub open: bool,
    pub shortcuts_open: bool,
    pub language: HelpLanguage,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HelpLanguage {
    English,
    Italian,
}

impl Default for HelpUi {
    fn default() -> Self {
        Self {
            // Open for a player, shut for a script. An unattended run — a
            // capture, the self-test, an autoplay demo — has nobody to dismiss
            // it, and a modal window across the middle of every screenshot is
            // the one thing that makes the captures useless for reviewing the
            // thing they were taken to review.
            open: ["SPOTORNO_AUTOPLAY", "SPOTORNO_SHOT", "SPOTORNO_SELFTEST"]
                .iter()
                .all(|k| std::env::var(k).is_err()),
            shortcuts_open: false,
            language: HelpLanguage::English,
        }
    }
}

/// The three jobs the right-hand dock does, one at a time.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockTab {
    /// Weather, ignitions, seed, restart — rewriting the scenario.
    Fire,
    /// The roster and the orders — directing the response.
    Units,
    /// The searchable roster of everything inspectable.
    Entities,
}

impl DockTab {
    pub const ALL: [DockTab; 3] = [DockTab::Fire, DockTab::Units, DockTab::Entities];

    pub fn label(self) -> &'static str {
        match self {
            DockTab::Fire => "🔥 Fire",
            DockTab::Units => "🚒 Units",
            DockTab::Entities => "🔍 Entities",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            DockTab::Fire => "Wind, moisture, ignitions, seed and restart.",
            DockTab::Units => "Crews, engines and aircraft, and the orders you give them.",
            DockTab::Entities => "Find any household, person, vehicle or unit.",
        }
    }
}

/// Where a workspace panel lives.
///
/// A hidden panel gives the map every pixel back; a floating panel keeps the
/// map full-width and is useful on a second monitor; a docked panel reserves a
/// stable strip and keeps map picking exact through [`sync_viewport`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelPlacement {
    Docked,
    Floating,
    Hidden,
}

impl PanelPlacement {
    pub fn visible(self) -> bool {
        self != PanelPlacement::Hidden
    }
}

/// Placement and navigation state for the three workspaces and the developer
/// diagnostics window.
#[derive(Resource)]
pub struct PanelState {
    pub incident: PanelPlacement,
    pub dock: PanelPlacement,
    pub tab: DockTab,
    pub inspector: PanelPlacement,
    pub debug: bool,
}

impl Default for PanelState {
    fn default() -> Self {
        // Useful for unattended visual review and for people who always work
        // with multiple monitors. Interactive placement changes remain in the
        // View menu and panel headers.
        let placement = if std::env::var("SPOTORNO_LAYOUT").as_deref() == Ok("floating") {
            PanelPlacement::Floating
        } else {
            PanelPlacement::Docked
        };
        PanelState {
            incident: placement,
            dock: placement,
            // `SPOTORNO_TAB=fire` opens on a chosen tab, which is the only way
            // to screenshot one unattended — the same reason `SPOTORNO_COMPOSER`
            // takes the composer's tab name rather than just a flag.
            tab: match std::env::var("SPOTORNO_TAB").as_deref() {
                Ok("fire") => DockTab::Fire,
                Ok("entities") => DockTab::Entities,
                _ => DockTab::Units,
            },
            inspector: placement,
            debug: std::env::var("SPOTORNO_DEBUG").is_ok(),
        }
    }
}

impl PanelState {
    /// Bring a tab to the front, expanding the dock if it was collapsed —
    /// "show me the units" from a menu or a shortcut has to actually show them,
    /// not silently select a tab behind a closed chevron.
    pub fn focus_tab(&mut self, tab: DockTab) {
        self.tab = tab;
        if self.dock == PanelPlacement::Hidden {
            self.dock = PanelPlacement::Docked;
        }
    }

    pub fn show_inspector(&mut self) {
        if self.inspector == PanelPlacement::Hidden {
            self.inspector = PanelPlacement::Docked;
        }
    }

    pub fn reset_layout(&mut self) {
        self.incident = PanelPlacement::Docked;
        self.dock = PanelPlacement::Docked;
        self.inspector = PanelPlacement::Docked;
        self.debug = false;
    }
}

/// The **Incident** dock: what the commander reads. No controls beyond the two
/// evacuation orders, because those are the only decisions that are made *from*
/// this information rather than from the map.
pub fn panel(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    layer: Res<FireLayer>,
    mut focus: ResMut<UiFocus>,
    mut panels: ResMut<PanelState>,
) {
    let ctx = contexts.ctx_mut();
    let mut placement = panels.incident;
    if placement == PanelPlacement::Hidden {
        return;
    }
    let scenario_name = sim.scenario.metadata.name.clone();
    let is_dev = sim.scenario.is_dev();
    let mut order = None;

    match placement {
        PanelPlacement::Docked => {
            egui::SidePanel::left("incident_dock")
                .resizable(true)
                .default_width(300.0)
                .width_range(270.0..=500.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Incident");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("×").on_hover_text("Hide panel").clicked() {
                                placement = PanelPlacement::Hidden;
                            }
                            if ui.small_button("↗").on_hover_text("Float panel").clicked() {
                                placement = PanelPlacement::Floating;
                            }
                            incident_identity(ui, &scenario_name, is_dev);
                        });
                    });
                    ui.separator();
                    order = incident_body(ui, &sim, *layer);
                });
        }
        PanelPlacement::Floating => {
            let mut visible = true;
            egui::Window::new(format!("Incident — {scenario_name}"))
                .id(egui::Id::new("incident_float"))
                .open(&mut visible)
                .default_pos(egui::pos2(18.0, 82.0))
                .default_width(330.0)
                .min_width(270.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Dock left").clicked() {
                            placement = PanelPlacement::Docked;
                        }
                        incident_identity(ui, &scenario_name, is_dev);
                    });
                    ui.separator();
                    order = incident_body(ui, &sim, *layer);
                });
            if !visible {
                placement = PanelPlacement::Hidden;
            }
        }
        PanelPlacement::Hidden => {}
    }

    if let Some((centre, radius)) = order {
        let n = sim.agents.order_evacuation(centre, radius);
        info!("evacuation order issued to {n} households within {radius:.0} m");
    }
    panels.incident = placement;
    focus.pointer |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

fn incident_identity(ui: &mut egui::Ui, scenario_name: &str, is_dev: bool) {
    if is_dev {
        ui.colored_label(egui::Color32::YELLOW, "🔧 DEV")
            .on_hover_text("A synthetic test scenario, not real data.");
    }
    ui.label(egui::RichText::new(scenario_name).weak());
}

fn incident_body(ui: &mut egui::Ui, sim: &Sim, layer: FireLayer) -> Option<(Pos, f32)> {
    let burning = sim
        .fire
        .state()
        .iter()
        .filter(|s| **s == CellFire::Burning)
        .count();
    let burnt = sim
        .fire
        .state()
        .iter()
        .filter(|s| **s == CellFire::Burnt)
        .count();
    let cell_ha = (sim.scenario.world.cellsize * sim.scenario.world.cellsize) / 10_000.0;
    let front = sim.fire.active_cells().len();
    let peak_fli = sim
        .fire
        .active_cells()
        .iter()
        .map(|c| sim.fire.cell_intensity(*c))
        .fold(0.0f32, f32::max);
    let threatened = sim.fire.exposure().threatened(0.05).count();
    let lost = sim
        .fire
        .exposure()
        .fields()
        .iter()
        .filter(|f| f.alight)
        .count();
    let peak_hazard = sim.fire.hazard().peak();
    let evac = sim.agents.stats();
    let median_evac = sim.agents.median_evacuation_s();
    let ordered = sim.agents.households.iter().filter(|h| h.ordered).count();
    let households = sim.agents.households.len();
    let ignition_pos = sim.scenario.world.centre_of(sim.ignition.centre);
    let mut order = None;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Map: {}", layer.label())).strong());
                ui.label(egui::RichText::new(layer.legend()).small().weak());
            });

            if threatened > 0 || lost > 0 || evac.cutoff > 0 || evac.casualties > 0 {
                ui.add_space(6.0);
                egui::Frame::group(ui.style())
                    .fill(egui::Color32::from_rgb(62, 31, 26))
                    .show(ui, |ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 150, 118),
                            format!(
                                "⚠ {threatened} threatened · {lost} structures lost · {} cut off",
                                evac.cutoff
                            ),
                        );
                    });
            }

            ui.add_space(8.0);
            section(ui, "Fire");
            egui::Grid::new("stats").num_columns(2).show(ui, |ui| {
                ui.label("Burnt");
                ui.strong(format!("{:.1} ha", (burning + burnt) as f32 * cell_ha));
                ui.end_row();
                ui.label("Active front");
                ui.label(format!("{front} cells"));
                ui.end_row();
                ui.label("Peak intensity");
                ui.label(format!(
                    "{peak_fli:.0} kW/m · {:.1} m flames",
                    fire::exposure::flame_length_m(peak_fli)
                ));
                ui.end_row();
                ui.label("Spread risk");
                ui.label(format!("{:.0}% next step", peak_hazard * 100.0));
                ui.end_row();
                ui.label("Threatened / lost");
                ui.label(format!("{threatened} / {lost}"));
                ui.end_row();
            });

            ui.add_space(8.0);
            section(ui, "Evacuation");
            let done = evac.safe as f32 / households.max(1) as f32;
            ui.horizontal(|ui| {
                ui.label("Households out");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.strong(format!("{} of {households}", evac.safe));
                });
            });
            ui.add(egui::ProgressBar::new(done).desired_height(8.0));
            egui::Grid::new("evac").num_columns(2).show(ui, |ui| {
                for (label, value) in [
                    ("Ordered", format!("{ordered} households")),
                    ("Preparing", evac.preparing.to_string()),
                    (
                        "Moving",
                        format!(
                            "{} hh · {} cars · {} foot",
                            evac.moving, evac.cars_moving, evac.on_foot
                        ),
                    ),
                    (
                        "Safe",
                        format!("{} hh · {} people", evac.safe, evac.people_safe),
                    ),
                    ("Defending", evac.defending.to_string()),
                    (
                        "Cut off / casualties",
                        format!("{} / {}", evac.cutoff, evac.casualties),
                    ),
                    (
                        "Median time out",
                        median_evac.map_or_else(|| "—".into(), |s| format!("{:.0} min", s / 60.0)),
                    ),
                ] {
                    ui.label(label);
                    ui.label(value);
                    ui.end_row();
                }
            });

            ui.add_space(8.0);
            section(ui, "Evacuation order");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Evacuate 2 km")
                    .on_hover_text("Order households within 2 km of the opening fire.")
                    .clicked()
                {
                    order = Some((ignition_pos, 2000.0));
                }
                if ui
                    .button("Evacuate everyone  (Shift+E)")
                    .on_hover_text("Issue a general order to the whole scenario.")
                    .clicked()
                {
                    order = Some((ignition_pos, 20_000.0));
                }
            });

            if sim.scenario.is_dev() {
                ui.add_space(8.0);
                ui.collapsing("🔧 Scenario detail", |ui| {
                    let w = &sim.scenario.world;
                    egui::Grid::new("dev").num_columns(2).show(ui, |ui| {
                        for (k, v) in [
                            ("Scenario id", sim.scenario.metadata.id.clone()),
                            ("People", sim.agents.people.len().to_string()),
                            ("Households", households.to_string()),
                            (
                                "Buildings",
                                sim.scenario.vectors.buildings.len().to_string(),
                            ),
                            (
                                "Fire grid",
                                format!("{}×{} @ {:.0} m", w.fire_rows, w.fire_cols, w.cellsize),
                            ),
                            ("World", format!("{:.0} × {:.0} m", w.width_m, w.height_m)),
                        ] {
                            ui.label(k);
                            ui.label(v);
                            ui.end_row();
                        }
                    });
                });
            }
            ui.add_space(6.0);
        });

    order
}

/// A section rule: a heading with a hairline under it. The docks were a wall of
/// undifferentiated `separator()`s, which reads as one long list rather than as
/// grouped answers to different questions.
pub(crate) fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title.to_uppercase()).small().weak());
    ui.separator();
}

/// A plain-language guide for the simulation. This intentionally describes a
/// useful first turn before listing shortcuts: a new player should know what
/// to try, rather than having to infer a workflow from the control panels.
pub fn help_panel(
    mut contexts: EguiContexts,
    mut help: ResMut<HelpUi>,
    mut focus: ResMut<UiFocus>,
) {
    if !help.open {
        return;
    }

    let ctx = contexts.ctx_mut();
    let mut open = help.open;
    let mut language = help.language;
    egui::Window::new("Help & quick start")
        .open(&mut open)
        .default_width(480.0)
        .max_width(620.0)
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Language / Lingua:");
                ui.selectable_value(&mut language, HelpLanguage::English, "English");
                ui.selectable_value(&mut language, HelpLanguage::Italian, "Italiano");
            });
            ui.separator();
            match language {
                HelpLanguage::English => help_english(ui),
                HelpLanguage::Italian => help_italian(ui),
            }
        });

    help.open = open;
    help.language = language;
    focus.pointer |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

/// The complete key map in a small, persistent window. Keeping this separate
/// from the Help menu means opening the menu never turns into a two-screen
/// wall of text, and it can stay beside the map while somebody learns it.
pub fn shortcuts_panel(
    mut contexts: EguiContexts,
    mut help: ResMut<HelpUi>,
    mut focus: ResMut<UiFocus>,
) {
    if !help.shortcuts_open {
        return;
    }
    let ctx = contexts.ctx_mut();
    let mut open = true;
    egui::Window::new("Keyboard shortcuts")
        .open(&mut open)
        .default_pos(egui::pos2(470.0, 110.0))
        .default_width(520.0)
        .resizable(true)
        .show(ctx, |ui| {
            shortcuts_group(
                ui,
                "Map & navigation",
                &[
                    ("Click", "select an entity · empty ground clears selection"),
                    (
                        "F",
                        "focus the selection; focus the fire when nothing is selected",
                    ),
                    ("Shift+F", "focus the opening fire"),
                    ("Home", "whole-scenario overview"),
                    ("Arrows", "pan"),
                    ("Drag / Shift-drag", "orbit / pan · right-drag also pans"),
                    ("Scroll", "zoom"),
                ],
            );
            shortcuts_group(
                ui,
                "Run & layers",
                &[
                    ("Space", "play / pause"),
                    (".", "step one agent-decision interval"),
                    ("[  ]", "slower / faster"),
                    ("1 – 4", "Flames / Intensity / Arrival / Hazard"),
                ],
            );
            shortcuts_group(
                ui,
                "Operations",
                &[
                    ("Shift+E", "general evacuation order"),
                    ("I", "arm ignition placement"),
                    ("Tab", "next available unit"),
                    ("A / L / D", "attack / line / drop"),
                    ("X", "stand down selected unit"),
                    ("C", "request air support"),
                    (
                        "Esc",
                        "cancel tool · leave follow/first-person view · close an interview",
                    ),
                ],
            );
            shortcuts_group(
                ui,
                "Find, panels & developer",
                &[
                    ("/", "open Entities and focus search"),
                    ("B", "show / hide Entities"),
                    ("G", "Agent Behaviour Composer"),
                    (
                        "T",
                        "interview the selected agent — pauses the incident, and takes the \
                         keyboard until you close it",
                    ),
                    ("F2", "developer diagnostics"),
                    ("?", "this shortcut window"),
                    ("F1", "quick start"),
                    ("Ctrl/⌘+R", "restart the incident from T+0"),
                    ("F12", "save a screenshot"),
                ],
            );
        });
    help.shortcuts_open = open;
    focus.pointer |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

fn shortcuts_group(ui: &mut egui::Ui, title: &str, rows: &[(&str, &str)]) {
    section(ui, title);
    egui::Grid::new(("shortcuts", title))
        .num_columns(2)
        .spacing([20.0, 3.0])
        .show(ui, |ui| {
            for (key, action) in rows {
                ui.label(egui::RichText::new(*key).monospace().strong());
                ui.label(*action);
                ui.end_row();
            }
        });
    ui.add_space(8.0);
}

fn help_english(ui: &mut egui::Ui) {
    ui.heading("What is this?");
    ui.label("You are the incident commander for a wildfire near Spotorno. The fire, weather, roads, households and response crews are simulated in real time. Your job is to watch the situation, protect people, and use the available crews where they can make a difference.");
    ui.add_space(8.0);
    ui.heading("A simple first run");
    ui.label(
        "1. Look at the fire, and try the four map layers under View ▸ Fire layer (or press 1–4).",
    );
    ui.label("2. Order an evacuation when people may be at risk.");
    ui.label("3. Select a unit in the Units tab, choose an order, then click the map to place it.");
    ui.label("4. Press Play and adjust time acceleration as the incident develops.");
    ui.small("There is no score: use the information in the panels to see the consequences of each decision.");
    ui.add_space(8.0);
    ui.heading("Reading the panels");
    ui.label("The menu bar along the top reaches everything, and the clock, play button and speed sit at its right-hand end.");
    ui.label("Incident, on the left, shows fire size, intensity, risk, structure losses and evacuation progress.");
    ui.label("The Operations workspace has three tabs. Fire changes wind, moisture and ignitions, and restarts from T+0. Units is where you select crews and assign direct attack, a fire line or a water drop — request air support early, it takes 25 minutes to arrive. Entities finds any household, person, vehicle or unit.");
    ui.label("Incident, Operations and Inspector can be docked, floated or hidden from View. Their header buttons provide the same controls.");
    ui.label("Click something on the map or in a list and the Inspector opens along the bottom with everything the model knows about it.");
    ui.add_space(8.0);
    ui.heading("Why did they do that?");
    ui.label("Households, people caught away from home, and suppression units each decide for themselves, and every one of those decision models can be read and rewritten. Press G for the Agent Behaviour Composer: it holds the decision graph for each kind of agent, a test bench, and its own help.");
    ui.label("Select an agent first and the composer's Live tab shows their behaviour running — every value on every box, with the path that produced the decision picked out. Press . to step one decision at a time, paused or not.");
    controls_guide(
        ui,
        [
            (
                "Move the view",
                "left-drag orbit · right-drag pan · scroll zoom · arrow keys pan",
            ),
            (
                "Run time",
                "Space play/pause · . one decision · [ and ] change speed",
            ),
            ("Evacuate", "Shift+E orders everyone out"),
            ("Fire layers", "1–4 switch the map overlay"),
            ("Ignition", "I, then click the map · Ctrl/⌘+R restarts"),
            (
                "Crew orders",
                "Tab next unit · A attack · L line · D drop · X stand down · C request air",
            ),
            (
                "Panels",
                "/ find · B Entities · G Composer · F2 diagnostics · ? shortcuts",
            ),
            ("Cancel", "Esc cancels the active map tool"),
        ],
    );
}

fn help_italian(ui: &mut egui::Ui) {
    ui.heading("Che cos'è?");
    ui.label("Sei il responsabile delle operazioni per un incendio boschivo vicino a Spotorno. Incendio, meteo, strade, famiglie e squadre di intervento sono simulati in tempo reale. Il tuo compito è osservare la situazione, proteggere le persone e usare le squadre dove possono fare la differenza.");
    ui.add_space(8.0);
    ui.heading("Una prima simulazione semplice");
    ui.label("1. Osserva l'incendio e prova i quattro livelli in View ▸ Fire layer (o premi 1–4).");
    ui.label("2. Ordina l'evacuazione quando le persone potrebbero essere in pericolo.");
    ui.label("3. Seleziona una squadra nella scheda Units, scegli un ordine e poi clicca sulla mappa per assegnarlo.");
    ui.label("4. Premi Play e regola l'accelerazione del tempo mentre l'emergenza evolve.");
    ui.small("Non c'è un punteggio: usa le informazioni nei pannelli per capire le conseguenze delle decisioni.");
    ui.add_space(8.0);
    ui.heading("Come leggere i pannelli");
    ui.label("La barra dei menu in alto raggiunge ogni funzione; orologio, play e velocità stanno alla sua destra.");
    ui.label("Incident, a sinistra, mostra estensione e intensità dell'incendio, rischio, strutture perse e avanzamento dell'evacuazione.");
    ui.label("L'area Operazioni ha tre schede. Fire cambia vento, umidità e inneschi e riparte da T+0. Units serve a selezionare le squadre e assegnare attacco diretto, linea tagliafuoco o lancio d'acqua — richiedi presto il supporto aereo, servono 25 minuti. Entities trova qualsiasi famiglia, persona, veicolo o squadra.");
    ui.label("Incident, Operazioni e Inspector si possono agganciare, rendere mobili o nascondere dal menu View e dai pulsanti nell'intestazione.");
    ui.label("Clicca un elemento sulla mappa o in un elenco e l'Inspector si apre in basso con tutto ciò che il modello sa su di esso.");
    ui.add_space(8.0);
    ui.heading("Perché si comportano così?");
    ui.label("Famiglie, persone sorprese fuori casa e squadre di intervento decidono ciascuna per conto proprio, e ognuno di questi modelli decisionali si può leggere e riscrivere. Premi G per l'Agent Behaviour Composer: contiene il grafo decisionale di ogni tipo di agente, un banco di prova e la propria guida.");
    ui.label("Seleziona prima un agente e la scheda Live del composer mostra il suo comportamento in funzione — ogni valore su ogni nodo, con evidenziato il percorso che ha prodotto la decisione. Premi . per avanzare di una decisione alla volta, anche in pausa.");
    controls_guide(
        ui,
        [
            (
                "Muovere la visuale",
                "trascina a sinistra per ruotare · a destra per spostare · rotella per zoom · frecce per scorrere",
            ),
            ("Tempo", "Spazio avvia/pausa · . un passo decisionale · [ e ] cambiano velocità"),
            ("Evacuazione", "Shift+E ordina l'evacuazione generale"),
            (
                "Livelli incendio",
                "1–4 cambiano la sovrapposizione sulla mappa",
            ),
            ("Innesco", "I, poi clicca la mappa · Ctrl/⌘+R riavvia"),
            (
                "Ordini alle squadre",
                "Tab squadra successiva · A attacco · L linea · D lancio · X rientro · C supporto aereo",
            ),
            (
                "Pannelli",
                "/ cerca · B Entities · G Composer · F2 diagnostica · ? scorciatoie",
            ),
            ("Annulla", "Esc annulla lo strumento attivo sulla mappa"),
        ],
    );
}

fn controls_guide(ui: &mut egui::Ui, rows: [(&str, &str); 8]) {
    ui.add_space(8.0);
    ui.heading("Controls / Comandi");
    egui::Grid::new("help_controls")
        .num_columns(2)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            for (label, action) in rows {
                ui.strong(label);
                ui.label(action);
                ui.end_row();
            }
        });
}

/// Live developer information in one deliberate surface instead of small
/// diagnostic fragments scattered through operational panels.
pub fn debug_panel(
    mut contexts: EguiContexts,
    mut panels: ResMut<PanelState>,
    sim: Res<Sim>,
    orbit: Query<&crate::camera::OrbitCamera>,
    mode: Res<crate::camera::CameraMode>,
    selected: Res<crate::inspect::Selected>,
    time: Res<Time>,
    mut focus: ResMut<UiFocus>,
) {
    if !panels.debug {
        return;
    }
    let ctx = contexts.ctx_mut();
    let mut open = true;
    let fps = if time.delta_seconds() > 0.0 {
        1.0 / time.delta_seconds()
    } else {
        0.0
    };
    egui::Window::new("Developer diagnostics")
        .open(&mut open)
        .default_pos(egui::pos2(430.0, 95.0))
        .default_width(430.0)
        .min_width(360.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(
                    if fps >= 45.0 {
                        egui::Color32::from_rgb(110, 210, 145)
                    } else {
                        egui::Color32::from_rgb(245, 170, 80)
                    },
                    format!("{fps:.0} fps · {:.1} ms", time.delta_seconds() * 1000.0),
                );
                ui.separator();
                ui.label(format!("generation {}", sim.generation));
                ui.label(format!("{} events", sim.history.log.len()));
            });

            ui.add_space(6.0);
            section(ui, "Simulation");
            egui::Grid::new("debug_sim").num_columns(2).show(ui, |ui| {
                for (label, value) in [
                    (
                        "Scenario",
                        format!(
                            "{} ({})",
                            sim.scenario.metadata.name, sim.scenario.metadata.id
                        ),
                    ),
                    (
                        "Clock / speed",
                        format!("T+{} · {:.0}x", sim.clock(), sim.speed),
                    ),
                    ("Seed", sim.seed.to_string()),
                    (
                        "Fire",
                        format!("{} active cells", sim.fire.active_cells().len()),
                    ),
                    (
                        "Agents",
                        format!(
                            "{} households · {} people · {} travellers",
                            sim.agents.households.len(),
                            sim.agents.people.len(),
                            sim.agents.travellers.len()
                        ),
                    ),
                    (
                        "Units",
                        format!(
                            "{} total · {} working",
                            sim.crews.units.len(),
                            sim.crews.stats().working
                        ),
                    ),
                ] {
                    ui.label(label);
                    ui.label(value);
                    ui.end_row();
                }
            });

            ui.add_space(8.0);
            section(ui, "View & selection");
            if let Ok(orbit) = orbit.get_single() {
                ui.label(format!(
                    "camera {:?} · focus [{:.0}, {:.0}] · {:.0} m away",
                    *mode, orbit.focus.x, -orbit.focus.z, orbit.distance
                ));
            }
            ui.label(match selected.target {
                Some(t) => format!("selected: {:?}", t),
                None => "selected: none".to_string(),
            });

            ui.add_space(8.0);
            section(ui, "Recent event log");
            let events = sim.history.log.recent(10);
            if events.is_empty() {
                ui.weak("No events yet.");
            } else {
                for event in events {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "T+{:02}:{:02}",
                                event.sim_time_s / 60,
                                event.sim_time_s % 60
                            ))
                            .monospace()
                            .weak(),
                        );
                        ui.label(format!("{:?}", event.subject));
                        ui.label(crate::history::summarize(&event.kind, &event.detail));
                    });
                }
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Reset panel layout").clicked() {
                    panels.reset_layout();
                }
                ui.weak("F12 saves a screenshot");
            });
        });
    panels.debug = open && panels.debug;
    focus.pointer |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

/// Lightweight feedback over the map. It is deliberately non-interactive so
/// it never steals an entity click or a camera drag.
pub fn map_hud(
    mut contexts: EguiContexts,
    hovered: Res<crate::inspect::HoveredTarget>,
    selected: Res<crate::inspect::Selected>,
    sim: Res<Sim>,
    ignition: Res<IgnitionTool>,
    order: Res<crate::command::OrderTool>,
    mode: Res<crate::camera::CameraMode>,
) {
    let ctx = contexts.ctx_mut();
    if hovered.0.is_some() && ignition.mode == EditMode::Off && !order.is_armed() {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let text = if let Some(hint) = crate::menu::armed_hint(
        ignition.mode == EditMode::Place,
        order.armed,
        order.line_from.is_some(),
    ) {
        format!("{hint}  ·  Esc cancels")
    } else if let Some(target) = hovered.0 {
        format!(
            "{}  ·  click to inspect",
            crate::inspect::target_label(&sim, target)
        )
    } else if let Some(target) = selected.target {
        let camera = match *mode {
            crate::camera::CameraMode::Free => "",
            crate::camera::CameraMode::Follow(_) => " · following",
            crate::camera::CameraMode::FirstPerson(_) => " · first person",
        };
        format!(
            "Selected: {}{camera}  ·  F focus · Esc clear",
            crate::inspect::target_label(&sim, target)
        )
    } else {
        "Click an entity to inspect  ·  drag orbit · right-drag pan · scroll zoom".to_string()
    };
    let rect = ctx.available_rect();
    egui::Area::new(egui::Id::new("map_hud"))
        .order(egui::Order::Foreground)
        .interactable(false)
        .fixed_pos(egui::pos2(rect.left() + 12.0, rect.bottom() - 38.0))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(egui::Color32::from_black_alpha(205))
                .rounding(egui::Rounding::same(5.0))
                .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.label(text);
                });
        });
}

/// A quieter, higher-contrast egui theme sized for a dense command interface.
pub fn setup_style(mut contexts: EguiContexts) {
    let ctx = contexts.ctx_mut();
    let mut style = (*ctx.style()).clone();
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(18.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(13.5));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(11.5));
    style.spacing.item_spacing = egui::vec2(7.0, 5.0);
    style.spacing.button_padding = egui::vec2(7.0, 3.0);
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(224, 229, 235));
    style.visuals.panel_fill = egui::Color32::from_rgb(22, 25, 29);
    style.visuals.window_fill = egui::Color32::from_rgb(25, 29, 34);
    style.visuals.faint_bg_color = egui::Color32::from_rgb(32, 37, 43);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(12, 15, 18);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(25, 102, 154);
    style.visuals.selection.stroke.color = egui::Color32::from_rgb(119, 197, 255);
    ctx.set_style(style);
}

/// The right-hand dock: one panel, three tabs, one of them showing.
///
/// All three used to be their own `SidePanel`, which is why the 3D view was a
/// letterbox. Sharing one dock also means they share one width the player sets
/// once, rather than three they have to negotiate every time one is opened.
#[allow(clippy::too_many_arguments)]
pub fn dock(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    mut panels: ResMut<PanelState>,
    mut focus: ResMut<UiFocus>,
    mut tool: ResMut<IgnitionTool>,
    mut order: ResMut<crate::command::OrderTool>,
    mut browser: ResMut<crate::browser::BrowserUi>,
    mut selected: ResMut<crate::inspect::Selected>,
    mut restarted: EventWriter<SimRestarted>,
    mut camera: Query<&mut crate::camera::OrbitCamera>,
) {
    let ctx = contexts.ctx_mut();
    let mut placement = panels.dock;
    let mut tab = panels.tab;
    if placement == PanelPlacement::Hidden {
        return;
    }
    let mut show_inspector = false;

    match placement {
        PanelPlacement::Docked => {
            egui::SidePanel::right("work_dock")
                .resizable(true)
                .default_width(320.0)
                .width_range(290.0..=540.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        workspace_tabs(ui, &mut tab);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("×")
                                .on_hover_text("Hide workspace")
                                .clicked()
                            {
                                placement = PanelPlacement::Hidden;
                            }
                            if ui
                                .small_button("↗")
                                .on_hover_text("Float workspace")
                                .clicked()
                            {
                                placement = PanelPlacement::Floating;
                            }
                        });
                    });
                    ui.separator();
                    show_inspector = workspace_body(
                        ui,
                        tab,
                        &mut sim,
                        &mut tool,
                        &mut order,
                        &mut browser,
                        &mut selected,
                        &mut restarted,
                        &mut camera,
                    );
                });
        }
        PanelPlacement::Floating => {
            let mut visible = true;
            egui::Window::new("Operations workspace")
                .id(egui::Id::new("work_float"))
                .open(&mut visible)
                .default_pos(egui::pos2(1030.0, 82.0))
                .default_width(360.0)
                .min_width(290.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Dock right").clicked() {
                            placement = PanelPlacement::Docked;
                        }
                        workspace_tabs(ui, &mut tab);
                    });
                    ui.separator();
                    show_inspector = workspace_body(
                        ui,
                        tab,
                        &mut sim,
                        &mut tool,
                        &mut order,
                        &mut browser,
                        &mut selected,
                        &mut restarted,
                        &mut camera,
                    );
                });
            if !visible {
                placement = PanelPlacement::Hidden;
            }
        }
        PanelPlacement::Hidden => {}
    }

    panels.dock = placement;
    panels.tab = tab;
    if show_inspector {
        panels.show_inspector();
    }
    focus.pointer |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

fn workspace_tabs(ui: &mut egui::Ui, tab: &mut DockTab) {
    for t in DockTab::ALL {
        if ui
            .selectable_label(*tab == t, t.label())
            .on_hover_text(t.hint())
            .clicked()
        {
            *tab = t;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn workspace_body(
    ui: &mut egui::Ui,
    tab: DockTab,
    sim: &mut Sim,
    tool: &mut IgnitionTool,
    order: &mut crate::command::OrderTool,
    browser: &mut crate::browser::BrowserUi,
    selected: &mut crate::inspect::Selected,
    restarted: &mut EventWriter<SimRestarted>,
    camera: &mut Query<&mut crate::camera::OrbitCamera>,
) -> bool {
    let mut show_inspector = false;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| match tab {
            DockTab::Fire => wildfire_body(ui, sim, tool, restarted),
            DockTab::Units => {
                show_inspector = crate::command::units_body(ui, sim, order, tool, selected, camera);
            }
            DockTab::Entities => {
                show_inspector = crate::browser::entities_body(ui, browser, selected, sim, camera);
            }
        });
    show_inspector
}

/// The wildfire controls: weather, ignitions, restart.
///
/// Weather is staged rather than applied per-pixel-of-drag. Every change is a
/// scheduled boundary condition in the core, so applying on each frame of a
/// slider drag would push a hundred events for one gesture — the fire would
/// still be right, but the event heap would carry the whole drag. It commits on
/// release, and the Apply button covers keyboard entry.
pub fn wildfire_body(
    ui: &mut egui::Ui,
    sim: &mut Sim,
    tool: &mut IgnitionTool,
    restarted: &mut EventWriter<SimRestarted>,
) {
    let mut weather = sim.weather;
    let mut commit_weather = false;
    let mut do_restart;
    let mut replan = false;
    let mut clear_extra = false;
    let mut mode = tool.mode;
    let mut radius = tool.radius_m;
    let mut seed = sim.seed;
    let live = sim.fire.weather();
    let dirty = sim.weather_dirty();
    let opening = sim.ignitions.iter().filter(|i| i.at_s == 0).count();
    let added = sim.ignitions.len() - opening;
    let running = sim.time_s();

    section(ui, "Wind");
    ui.horizontal(|ui| {
        compass(ui, live.wind_dir_deg as f32, weather.wind_dir_deg as f32);
        ui.vertical(|ui| {
            // Both directions spelled out, always. `w_dir` is the
            // bearing the wind blows *from*, the kernel reads it in a
            // way that looks like the opposite convention, and a bare
            // number here is the single easiest thing in this UI to
            // misread by 180 degrees.
            let from = weather.wind_dir_deg as f32;
            ui.label(format!("from {} ({:.0}°)", cardinal(from), from));
            ui.label(
                egui::RichText::new(format!("drives fire {}", cardinal((from + 180.0) % 360.0)))
                    .strong(),
            );
        });
    });
    let r = ui.add(
        egui::Slider::new(&mut weather.wind_dir_deg, 0.0..=359.0)
            .step_by(5.0)
            .custom_formatter(|v, _| format!("{v:.0}° from {}", cardinal(v as f32)))
            .text("direction"),
    );
    commit_weather |= r.drag_stopped() || r.lost_focus();
    let r = ui.add(
        egui::Slider::new(&mut weather.wind_speed_kmh, 0.0..=90.0)
            .suffix(" km/h")
            .text("speed"),
    );
    commit_weather |= r.drag_stopped() || r.lost_focus();

    ui.add_space(8.0);
    section(ui, "Fuel moisture");
    let r = ui.add(
        egui::Slider::new(&mut weather.moisture_pct, 2.0..=40.0)
            .suffix(" %")
            .text("dead fine fuel"),
    );
    commit_weather |= r.drag_stopped() || r.lost_focus();
    ui.small(match weather.moisture_pct {
        m if m < 8.0 => "critically dry — fire spreads freely",
        m if m < 15.0 => "dry",
        m if m < 25.0 => "damp — spread slows sharply",
        _ => "wet — most fuels will not carry fire",
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let apply = ui
            .add_enabled(dirty, egui::Button::new("Apply weather"))
            .on_hover_text(
                "Takes effect from now on. What the fire has already \
         burnt is not rewritten — this is a wind shift, not a \
         different scenario.",
            );
        commit_weather |= apply.clicked();
        if dirty {
            ui.colored_label(egui::Color32::from_rgb(240, 180, 60), "● pending");
        }
    });

    ui.add_space(8.0);
    section(ui, "Ignition");
    let placing = mode == EditMode::Place;
    if ui
        .selectable_label(
            placing,
            if placing {
                "▶ Click the map to light a fire"
            } else {
                "Place ignition  (I)"
            },
        )
        .on_hover_text(
            "Left-click lights a patch where you point. Right-drag \
     still orbits, so you keep the camera.",
        )
        .clicked()
    {
        mode = if placing {
            EditMode::Off
        } else {
            EditMode::Place
        };
    }
    let r = ui.add(
        egui::Slider::new(&mut radius, MIN_IGNITION_RADIUS_M..=MAX_IGNITION_RADIUS_M)
            .suffix(" m")
            .text("radius"),
    );
    // Not applied on release: the cursor ring has to resize as it is
    // dragged, or the control has no feedback at all.
    let _ = r;
    ui.small(format!(
        "≈{:.0} ha. Below {MIN_IGNITION_RADIUS_M:.0} m a single patch \
                 often fails to establish.",
        std::f32::consts::PI * radius * radius / 10_000.0
    ));

    egui::Grid::new("ign").num_columns(2).show(ui, |ui| {
        ui.label("Opening fire");
        ui.label(format!("{opening} patch(es)"));
        ui.end_row();
        ui.label("Added since start");
        ui.label(format!("{added}"));
        ui.end_row();
    });
    ui.horizontal(|ui| {
        if ui
            .add_enabled(added > 0, egui::Button::new("Forget added"))
            .on_hover_text("Drop the fires you lit mid-run from the restart list.")
            .clicked()
        {
            clear_extra = true;
        }
        if ui
            .button("Replan for wind")
            .on_hover_text(
                "Move the opening fire to the best-measured start for \
         this wind direction — inland, in continuous fuel, \
         upwind of the town.",
            )
            .clicked()
        {
            replan = true;
        }
    });

    ui.add_space(8.0);
    section(ui, "Run");
    ui.horizontal(|ui| {
        ui.label("Seed");
        ui.add(egui::DragValue::new(&mut seed).speed(1.0));
    });
    let restart = ui
        .add_sized(
            [ui.available_width(), 28.0],
            egui::Button::new("⟲  Restart incident  (Ctrl/⌘+R)")
                .fill(egui::Color32::from_rgb(120, 40, 32)),
        )
        .on_hover_text(
            "Relights the scenario at T+0 with the current weather, \
     seed and ignitions. The terrain, buildings and population \
     are kept; the fire and every household's decision are not.",
        );
    do_restart = restart.clicked();
    if running > 0 {
        ui.small(format!(
            "{} of simulated time will be discarded.",
            hhmm(running)
        ));
    }

    // Radius and mode are pure view state, so they go back immediately.
    if radius != tool.radius_m {
        tool.radius_m = clamp_radius(radius);
    }
    if mode != tool.mode {
        tool.mode = mode;
    }

    if seed != sim.seed {
        sim.seed = seed;
    }
    if weather.wind_dir_deg != sim.weather.wind_dir_deg
        || weather.wind_speed_kmh != sim.weather.wind_speed_kmh
        || weather.moisture_pct != sim.weather.moisture_pct
    {
        sim.weather = weather;
    }
    if commit_weather && sim.weather_dirty() {
        if let Err(e) = sim.apply_weather() {
            error!("applying weather failed: {e:#}");
        }
    }
    if clear_extra {
        sim.ignitions.retain(|i| i.at_s == 0);
    }
    if replan {
        let dir = sim.weather.wind_dir_deg;
        let plan = fire::plan_ignition(&sim.scenario, dir, crate::sim::START_RADIUS_M);
        info!(
            "ignition replanned for wind from {dir:.0}°: ({}, {}), {} households downwind",
            plan.centre.row, plan.centre.col, plan.households_downwind
        );
        sim.ignitions.retain(|i| i.at_s != 0);
        sim.ignitions.push(crate::sim::Ignition {
            centre: plan.centre,
            radius_m: plan.radius_m,
            at_s: 0,
        });
        sim.ignition = plan;
        do_restart = true;
    }
    if do_restart {
        match sim.restart() {
            Ok(()) => {
                restarted.send(SimRestarted);
            }
            Err(e) => error!("restart failed: {e:#}"),
        }
    }
}

/// A wind rose. `live` is what the fire is running on, `staged` what the
/// sliders currently say; they differ only while a change is pending.
///
/// Draws the arrow pointing the way the wind *pushes* — down-wind — because
/// that is the direction the player is reasoning about, with a tick on the rim
/// for the meteorological bearing the number reports.
fn compass(ui: &mut egui::Ui, live: f32, staged: f32) {
    let size = 64.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    let r = size * 0.44;

    p.circle_stroke(c, r, egui::Stroke::new(1.0, egui::Color32::from_gray(90)));
    for (label, ang) in [("N", 0.0f32), ("E", 90.0), ("S", 180.0), ("W", 270.0)] {
        let v = bearing_vec(ang) * (r + 7.0);
        p.text(
            c + v,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(9.0),
            egui::Color32::from_gray(140),
        );
    }

    // Arrow along the down-wind direction.
    let draw = |ang: f32, color: egui::Color32, width: f32| {
        let down = bearing_vec((ang + 180.0) % 360.0);
        let tail = c - down * r * 0.85;
        let head = c + down * r * 0.85;
        p.line_segment([tail, head], egui::Stroke::new(width, color));
        // Head, as two barbs rather than a filled triangle: at 64 px a filled
        // head is a blob.
        let side = egui::vec2(-down.y, down.x);
        for s in [-1.0, 1.0] {
            p.line_segment(
                [head, head - down * (r * 0.34) + side * s * (r * 0.22)],
                egui::Stroke::new(width, color),
            );
        }
        // Rim tick at the bearing the wind comes from.
        let from = bearing_vec(ang);
        p.line_segment(
            [c + from * r * 0.86, c + from * r],
            egui::Stroke::new(width, color),
        );
    };

    if (live - staged).abs() > 0.5 {
        draw(live, egui::Color32::from_gray(90), 1.0);
    }
    draw(staged, egui::Color32::from_rgb(255, 170, 60), 2.0);
}

/// Unit screen vector for a compass bearing: north up, east right, clockwise.
/// Screen y grows downward, hence the negated cosine.
fn bearing_vec(deg: f32) -> egui::Vec2 {
    let a = deg.to_radians();
    egui::vec2(a.sin(), -a.cos())
}

/// Nearest 16-point compass name for a bearing.
fn cardinal(deg: f32) -> &'static str {
    const NAMES: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    let i = (((deg.rem_euclid(360.0)) / 22.5).round() as usize) % 16;
    NAMES[i]
}

fn hhmm(s: i64) -> String {
    format!("{}h {:02}m", s / 3600, (s / 60) % 60)
}

/// Point the 3D camera's own viewport at whatever screen area the docked
/// panels have *not* claimed this frame.
///
/// The panels are docked (`SidePanel`/`TopBottomPanel`), not floating windows,
/// specifically so the game reads as one application with an external menu
/// rather than a 3D view with widgets glued on top of it. That only pays off
/// if the 3D render actually retreats into the leftover space instead of
/// rendering full-screen underneath the panels' opaque backgrounds — and
/// setting `Camera::viewport` is also what keeps click-to-inspect accurate,
/// since `Camera::world_to_viewport`/`viewport_to_world` already account for
/// it. Must run after every docked panel has called `.show()` this frame, or
/// `ctx.available_rect()` reports space a panel is about to claim.
pub fn sync_viewport(
    mut contexts: EguiContexts,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut camera: Query<&mut Camera, With<crate::camera::OrbitCamera>>,
) {
    let ctx = contexts.ctx_mut();
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Ok(mut cam) = camera.get_single_mut() else {
        return;
    };

    let rect = ctx.available_rect();
    let scale = window.scale_factor() as f32;
    let phys_w = window.physical_width();
    let phys_h = window.physical_height();

    let min_x = (rect.min.x * scale).round().max(0.0) as u32;
    let min_y = (rect.min.y * scale).round().max(0.0) as u32;
    let w = (rect.width() * scale).round().max(0.0) as u32;
    let h = (rect.height() * scale).round().max(0.0) as u32;
    // Clamp against the render target: a panel resize can momentarily report
    // a rect that runs past the window edge, and Bevy panics on a viewport
    // that does not fit inside its target.
    let w = w.min(phys_w.saturating_sub(min_x));
    let h = h.min(phys_h.saturating_sub(min_y));
    if w == 0 || h == 0 {
        return;
    }

    cam.viewport = Some(bevy::render::camera::Viewport {
        physical_position: UVec2::new(min_x, min_y),
        physical_size: UVec2::new(w, h),
        depth: 0.0..1.0,
    });
}
