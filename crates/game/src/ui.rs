//! The screen furniture: controls on the left, entities on the right, the
//! incident workbench along the bottom, and the help window.
//!
//! The layout is deliberately three regions and no more. Everything the player
//! *acts on the incident with* is in the left dock; the right dock is a stable entity
//! navigator with the selected entity directly below it; and the bottom dock
//! holds the incident readout, conversations and developer work. The right
//! edge deliberately has no resize handle: entity rows and detail fields need
//! a predictable width, and resizing that dock used to produce a one-frame
//! camera viewport outside the render target.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use fire::CellFire;
use scenario::Pos;

use crate::fire_view::FireLayer;
use crate::ignition_edit::{clamp_radius, EditMode, IgnitionTool};
use crate::sim::{Sim, SimRestarted, MAX_IGNITION_RADIUS_M, MIN_IGNITION_RADIUS_M};
use crate::sky::DayClock;

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

/// Operational destinations used by shortcuts and menu commands.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockTab {
    /// Weather, ignitions, seed, restart — rewriting the scenario.
    Fire,
    /// The roster and the orders — directing the response.
    Units,
    /// The searchable roster of everything inspectable.
    Entities,
}

/// The work surfaces that share the bottom of the screen.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum BottomTab {
    Incident,
    Chat,
    Debug,
    Behaviour,
}

impl BottomTab {
    pub const ALL: [BottomTab; 4] = [
        BottomTab::Incident,
        BottomTab::Chat,
        BottomTab::Debug,
        BottomTab::Behaviour,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BottomTab::Incident => "Incident view",
            BottomTab::Chat => "Chat",
            BottomTab::Debug => "Live debugger",
            BottomTab::Behaviour => "Behavior editor",
        }
    }
}

/// Where a workspace panel lives.
///
/// A hidden panel gives the map every pixel back; a docked panel reserves a
/// stable strip and keeps map picking exact through [`sync_viewport`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelPlacement {
    Docked,
    Hidden,
}

impl PanelPlacement {
    pub fn visible(self) -> bool {
        self != PanelPlacement::Hidden
    }
}

/// Placement and navigation state for the three workspaces.
#[derive(Resource)]
pub struct PanelState {
    /// Bottom incident/chat/behavior workbench.
    pub incident: PanelPlacement,
    /// Left execution/fire/intervention controls.
    pub dock: PanelPlacement,
    /// Fixed-width right entity navigator and detail.
    pub inspector: PanelPlacement,
    pub bottom_tab: BottomTab,
}

impl Default for PanelState {
    fn default() -> Self {
        PanelState {
            incident: PanelPlacement::Docked,
            dock: PanelPlacement::Docked,
            inspector: PanelPlacement::Docked,
            bottom_tab: if std::env::var("SPOTORNO_COMPOSER").is_ok() {
                BottomTab::Behaviour
            } else if std::env::var("SPOTORNO_DEBUG").is_ok() {
                BottomTab::Debug
            } else {
                BottomTab::Incident
            },
        }
    }
}

impl PanelState {
    /// Bring the surface that owns an operational destination back on screen.
    pub fn focus_tab(&mut self, tab: DockTab) {
        match tab {
            DockTab::Fire | DockTab::Units => self.dock = PanelPlacement::Docked,
            DockTab::Entities => self.inspector = PanelPlacement::Docked,
        }
    }

    pub fn show_inspector(&mut self) {
        self.inspector = PanelPlacement::Docked;
    }

    pub fn focus_bottom(&mut self, tab: BottomTab) {
        self.bottom_tab = tab;
        self.incident = PanelPlacement::Docked;
    }

    pub fn reset_layout(&mut self) {
        self.incident = PanelPlacement::Docked;
        self.dock = PanelPlacement::Docked;
        self.inspector = PanelPlacement::Docked;
        self.bottom_tab = BottomTab::Incident;
    }
}

/// The bottom workbench: incident view, chat, live behavior debugger and editor.
///
/// Its height is fixed per tab. In particular, it does not inherit a previous
/// tab's dragged size, which made switching from the large behavior canvas back
/// to the incident readout consume most of the map.
#[allow(clippy::too_many_arguments)]
pub fn panel(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    layer: Res<FireLayer>,
    mut focus: ResMut<UiFocus>,
    mut panels: ResMut<PanelState>,
    selected: Res<crate::inspect::Selected>,
    mut interview: ResMut<crate::interview::Interview>,
    mut composer: ResMut<crate::composer::Composer>,
    mut apply: EventWriter<crate::composer::ApplyBehaviour>,
) {
    let ctx = contexts.ctx_mut();
    if panels.incident == PanelPlacement::Hidden {
        return;
    }
    let scenario_name = sim.scenario.metadata.name.clone();
    let is_dev = sim.scenario.is_dev();
    let mut order = None;
    let tab = panels.bottom_tab;
    let available = ctx.available_rect().height();
    let height = match tab {
        BottomTab::Incident => 260.0,
        BottomTab::Chat => 340.0,
        BottomTab::Debug => 420.0,
        BottomTab::Behaviour => (available * 0.68).clamp(520.0, 720.0),
    };

    egui::TopBottomPanel::bottom("bottom_workbench")
        .resizable(false)
        .exact_height(height.min((available - 120.0).max(180.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for candidate in BottomTab::ALL {
                    if ui
                        .selectable_label(panels.bottom_tab == candidate, candidate.label())
                        .clicked()
                    {
                        panels.bottom_tab = candidate;
                        composer.open = candidate == BottomTab::Behaviour;
                        interview.open =
                            candidate == BottomTab::Chat && interview.subject.is_some();
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("×")
                        .on_hover_text("Hide bottom workbench")
                        .clicked()
                    {
                        panels.incident = PanelPlacement::Hidden;
                        composer.open = false;
                        interview.open = false;
                    }
                    incident_identity(ui, &scenario_name, is_dev);
                });
            });
            ui.separator();

            match tab {
                BottomTab::Incident => {
                    order = incident_body(ui, &sim, *layer);
                }
                BottomTab::Chat => {
                    crate::interview::panel_body(
                        ui,
                        ctx,
                        &mut interview,
                        &mut sim,
                        selected.target,
                    );
                }
                BottomTab::Debug => {
                    crate::composer::live_debugger_body(ui, &mut composer);
                }
                BottomTab::Behaviour => {
                    crate::composer::panel_body(ui, &mut composer, &mut apply);
                }
            }
        });

    if let Some((centre, radius)) = order {
        let n = sim.agents.order_evacuation(centre, radius);
        info!("evacuation order issued to {n} households within {radius:.0} m");
    }
    focus.pointer |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
    focus.keyboard |= ctx.wants_keyboard_input();
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

    ui.columns(3, |columns| {
        columns[0].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Map: {}", layer.label())).strong());
                ui.label(egui::RichText::new(layer.legend()).small().weak());
            });

            if threatened > 0 || lost > 0 || evac.cutoff > 0 || evac.casualties > 0 {
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
        });

        columns[1].vertical(|ui| {
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
        });

        columns[2].vertical(|ui| {
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
        });
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
                    ("Esc", "cancel tool · leave follow/first-person view"),
                ],
            );
            shortcuts_group(
                ui,
                "Find & workspaces",
                &[
                    ("/", "open Entities and focus search"),
                    ("B", "show / hide Entities"),
                    ("G", "open the Behavior editor bottom tab"),
                    (
                        "T",
                        "open Chat for the selected agent — pauses the incident",
                    ),
                    ("F2", "live behavior debugger"),
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
    ui.label("3. Select a unit under Intervention on the left, choose an order, then click the map to place it.");
    ui.label("4. Press Play and adjust time acceleration as the incident develops.");
    ui.small("There is no score: use the information in the panels to see the consequences of each decision.");
    ui.add_space(8.0);
    ui.heading("Reading the panels");
    ui.label("The menu bar along the top reaches everything, and the clock, play button and speed sit at its right-hand end.");
    ui.label("The left Command panel contains execution controls, compact wind and moisture parameters, ignition settings, and crew intervention.");
    ui.label("The fixed-width right panel finds any household, person, vehicle or unit. Selecting a row or map symbol shows that entity's detail directly below the navigator.");
    ui.label("The bottom workbench switches between the incident view, agent chat, the selected agent's live behavior debugger, and the behavior editor.");
    ui.add_space(8.0);
    ui.heading("Why did they do that?");
    ui.label("Households, people caught away from home, and suppression units each decide for themselves, and every one of those decision models can be read and rewritten. Press G for the Behavior editor: it holds the decision graph for each kind of agent, a test bench, and its own help.");
    ui.label("Select an agent and press F2 for the live behavior debugger: its current decision, proposals, node values, active path and recent history. Press . to step one decision at a time, paused or not. The editor's Live view overlays the same trace on the graph.");
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
                "/ find · B Entities · G editor · F2 live debugger · ? shortcuts",
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
    ui.label("3. Seleziona una squadra in Intervento a sinistra, scegli un ordine e poi clicca sulla mappa per assegnarlo.");
    ui.label("4. Premi Play e regola l'accelerazione del tempo mentre l'emergenza evolve.");
    ui.small("Non c'è un punteggio: usa le informazioni nei pannelli per capire le conseguenze delle decisioni.");
    ui.add_space(8.0);
    ui.heading("Come leggere i pannelli");
    ui.label("La barra dei menu in alto raggiunge ogni funzione; orologio, play e velocità stanno alla sua destra.");
    ui.label("Il pannello Comando a sinistra contiene esecuzione, parametri compatti per vento e umidità, inneschi e intervento delle squadre.");
    ui.label("Il pannello a larghezza fissa sulla destra trova famiglie, persone, veicoli e squadre; il dettaglio dell'entità selezionata appare subito sotto l'elenco.");
    ui.label(
        "L'area in basso passa tra vista incidente, chat, debugger del comportamento dell'agente selezionato ed editor dei comportamenti.",
    );
    ui.add_space(8.0);
    ui.heading("Perché si comportano così?");
    ui.label("Famiglie, persone sorprese fuori casa e squadre di intervento decidono ciascuna per conto proprio, e ognuno di questi modelli decisionali si può leggere e riscrivere. Premi G per l'editor dei comportamenti: contiene il grafo decisionale di ogni tipo di agente, un banco di prova e la propria guida.");
    ui.label("Seleziona un agente e premi F2 per il debugger live: decisione corrente, proposte, valori dei nodi, percorso attivo e cronologia recente. Premi . per avanzare di una decisione alla volta, anche in pausa. La vista Live dell'editor sovrappone la stessa traccia al grafo.");
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
                "/ cerca · B Entities · G editor · F2 debugger live · ? scorciatoie",
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

/// The left command column: execution, fire parameters and intervention.
#[allow(clippy::too_many_arguments)]
pub fn dock(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    mut panels: ResMut<PanelState>,
    mut focus: ResMut<UiFocus>,
    mut tool: ResMut<IgnitionTool>,
    mut order: ResMut<crate::command::OrderTool>,
    mut selected: ResMut<crate::inspect::Selected>,
    mut restarted: EventWriter<SimRestarted>,
    mut camera: Query<&mut crate::camera::OrbitCamera>,
    mut day: ResMut<DayClock>,
) {
    let ctx = contexts.ctx_mut();
    if panels.dock == PanelPlacement::Hidden {
        return;
    }
    let mut show_inspector = false;

    egui::SidePanel::left("control_dock")
        .resizable(true)
        .default_width(330.0)
        .width_range(300.0..=430.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Command");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("×")
                        .on_hover_text("Hide command panel")
                        .clicked()
                    {
                        panels.dock = PanelPlacement::Hidden;
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_source("command_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    execution_body(ui, &mut sim, &mut restarted);
                    ui.add_space(12.0);
                    wildfire_body(ui, &mut sim, &mut tool, &mut restarted, &mut day);
                    ui.add_space(12.0);
                    section(ui, "Intervention");
                    show_inspector = crate::command::units_body(
                        ui,
                        &mut sim,
                        &mut order,
                        &mut tool,
                        &mut selected,
                        &mut camera,
                    );
                });
        });

    if show_inspector {
        panels.show_inspector();
    }
    focus.pointer |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

fn execution_body(ui: &mut egui::Ui, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    section(ui, "Execution control");
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("T+{}", sim.clock()))
                .monospace()
                .strong(),
        );
        if ui
            .button(if sim.playing { "⏸ Pause" } else { "▶ Play" })
            .clicked()
        {
            sim.playing = !sim.playing;
        }
        if ui
            .button("⏭ Step")
            .on_hover_text("Advance one agent decision interval")
            .clicked()
        {
            sim.request_step();
        }
    });

    ui.horizontal_wrapped(|ui| {
        for (speed, label) in PRESETS {
            if ui
                .selectable_label((sim.speed - speed).abs() < 0.5, label)
                .clicked()
            {
                sim.speed = speed;
            }
        }
    });
    ui.add(
        egui::Slider::new(&mut sim.speed, MIN_SPEED..=MAX_SPEED)
            .logarithmic(true)
            .text("speed")
            .custom_formatter(|value, _| speed_text(value as f32)),
    );

    let mut seed = sim.seed;
    ui.horizontal(|ui| {
        ui.label("Seed");
        ui.add(egui::DragValue::new(&mut seed).speed(1.0));
    });
    sim.seed = seed;
    if ui
        .add_sized(
            [ui.available_width(), 28.0],
            egui::Button::new("⟲ Restart incident  (Ctrl/⌘+R)")
                .fill(egui::Color32::from_rgb(120, 40, 32)),
        )
        .on_hover_text("Restart at T+0 with the current weather, seed and ignitions")
        .clicked()
    {
        match sim.restart() {
            Ok(()) => {
                restarted.send(SimRestarted);
            }
            Err(error) => error!("restart failed: {error:#}"),
        }
    }
}

fn speed_text(speed: f32) -> String {
    if speed >= 60.0 {
        format!("{:.1} min/s", speed / 60.0)
    } else {
        format!("{speed:.0}x")
    }
}

/// Compact wildfire parameters and ignition controls.
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
    day: &mut DayClock,
) {
    let mut weather = sim.weather;
    let mut commit_weather = false;
    let mut do_restart = false;
    let mut replan = false;
    let mut clear_extra = false;
    let mut mode = tool.mode;
    let mut radius = tool.radius_m;
    let dirty = sim.weather_dirty();
    let opening = sim.ignitions.iter().filter(|i| i.at_s == 0).count();
    let added = sim.ignitions.len() - opening;

    section(ui, "Time of day");
    let linked_hour = (day.start_hour + sim.time_s() as f32 / 3600.0).rem_euclid(24.0);
    let mut linked = day.linked();
    let shown = day.manual_hour.unwrap_or(linked_hour).rem_euclid(24.0);
    ui.horizontal(|ui| {
        ui.label("Clock");
        ui.strong(format!(
            "{:02}:{:02}",
            shown as u32,
            (shown.fract() * 60.0) as u32
        ));
        if linked {
            ui.small("(sim time)");
        }
    });
    if ui
        .checkbox(&mut linked, "linked to sim clock")
        .on_hover_text(
            "Unlink to scrub the sun and moon to any hour without \
             touching the incident clock — for lighting a shot or \
             checking how a scene reads at night.",
        )
        .changed()
    {
        day.manual_hour = if linked { None } else { Some(linked_hour) };
    }
    let mut manual = day.manual_hour.unwrap_or(linked_hour);
    let r = ui.add_enabled(
        !linked,
        egui::Slider::new(&mut manual, 0.0..=24.0)
            .custom_formatter(|v, _| format!("{:02}:{:02}", v as i64, (v.fract() * 60.0) as i64))
            .text("hour"),
    );
    if !linked && r.changed() {
        day.manual_hour = Some(manual);
    }

    ui.add_space(8.0);
    section(ui, "Fire parameters");
    let from = weather.wind_dir_deg as f32;
    ui.horizontal(|ui| {
        ui.label("Wind");
        ui.strong(format!(
            "from {} · pushes {}",
            cardinal(from),
            cardinal((from + 180.0) % 360.0)
        ));
        if dirty {
            ui.colored_label(egui::Color32::from_rgb(240, 180, 60), "● pending");
        }
    });
    let r = ui.add(
        egui::Slider::new(&mut weather.wind_dir_deg, 0.0..=359.0)
            .step_by(5.0)
            .custom_formatter(|v, _| format!("{v:.0}° from {}", cardinal(v as f32)))
            .text("from"),
    );
    commit_weather |= r.drag_stopped() || r.lost_focus();
    let r = ui.add(
        egui::Slider::new(&mut weather.wind_speed_kmh, 0.0..=90.0)
            .suffix(" km/h")
            .text("wind speed"),
    );
    commit_weather |= r.drag_stopped() || r.lost_focus();

    let r = ui.add(
        egui::Slider::new(&mut weather.moisture_pct, 2.0..=40.0)
            .suffix(" %")
            .text("fuel moisture"),
    );
    commit_weather |= r.drag_stopped() || r.lost_focus();
    ui.small(match weather.moisture_pct {
        m if m < 8.0 => "critically dry — fire spreads freely",
        m if m < 15.0 => "dry",
        m if m < 25.0 => "damp — spread slows sharply",
        _ => "wet — most fuels will not carry fire",
    });

    ui.horizontal(|ui| {
        let apply = ui
            .add_enabled(dirty, egui::Button::new("Apply weather"))
            .on_hover_text(
                "Takes effect from now on. What the fire has already \
         burnt is not rewritten — this is a wind shift, not a \
         different scenario.",
            );
        commit_weather |= apply.clicked();
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

    // Radius and mode are pure view state, so they go back immediately.
    if radius != tool.radius_m {
        tool.radius_m = clamp_radius(radius);
    }
    if mode != tool.mode {
        tool.mode = mode;
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

/// Nearest 16-point compass name for a bearing.
fn cardinal(deg: f32) -> &'static str {
    const NAMES: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    let i = (((deg.rem_euclid(360.0)) / 22.5).round() as usize) % 16;
    NAMES[i]
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
