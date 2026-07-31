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
const PRESETS: [(f32, &str); 5] = [
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
    pub language: HelpLanguage,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HelpLanguage {
    English,
    Italian,
}

/// Dev mode HUD for displaying debug info
pub fn dev_hud(
    mut contexts: EguiContexts,
    sim: Res<Sim>,
) {
    if !sim.scenario.is_dev() {
        return;
    }

    let ctx = contexts.ctx_mut();
    egui::Window::new("🔧 DEV MODE")
        .default_pos(egui::pos2(10.0, 10.0))
        .auto_sized()
        .show(ctx, |ui| {
            ui.label(format!("Scenario: {}", sim.scenario.metadata.id));
            ui.label(format!("People: {}", sim.agents.people.len()));
            ui.label(format!("Households: {}", sim.agents.households.len()));
            ui.label(format!("Buildings: {}", sim.scenario.vectors.buildings.len()));
            ui.separator();
            ui.label(format!("Fire grid: {}×{}", sim.scenario.world.fire_rows, sim.scenario.world.fire_cols));
            ui.label(format!("World: {:.0} × {:.0} m", sim.scenario.world.width_m, sim.scenario.world.height_m));
        });
}

impl Default for HelpUi {
    fn default() -> Self {
        Self {
            open: true,
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

/// Collapsed/expanded state of the three docks. Collapsing leaves a thin strip
/// with its chevron rather than removing the dock, so nothing the player opened
/// can go missing — and the Inspector keeps this separate from its "✕", which
/// also drops the selection.
#[derive(Resource)]
pub struct PanelState {
    pub incident: bool,
    pub dock: bool,
    pub tab: DockTab,
    pub inspector: bool,
}

impl Default for PanelState {
    fn default() -> Self {
        PanelState {
            incident: true,
            dock: true,
            tab: DockTab::Units,
            inspector: true,
        }
    }
}

impl PanelState {
    /// Bring a tab to the front, expanding the dock if it was collapsed —
    /// "show me the units" from a menu or a shortcut has to actually show them,
    /// not silently select a tab behind a closed chevron.
    pub fn focus_tab(&mut self, tab: DockTab) {
        self.tab = tab;
        self.dock = true;
    }
}

/// A collapse/expand chevron, consistent across every docked panel. Returns
/// the new state; callers still own writing it back to `PanelState` since
/// each panel's `open` is a local copy taken out for the frame.
pub(crate) fn collapse_button(
    ui: &mut egui::Ui,
    open: bool,
    collapse_glyph: &str,
    expand_glyph: &str,
) -> bool {
    let clicked = ui
        .small_button(if open { collapse_glyph } else { expand_glyph })
        .on_hover_text(if open { "Collapse" } else { "Expand" })
        .clicked();
    if clicked {
        !open
    } else {
        open
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
    let mut open = panels.incident;

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
    let ignition_pos = sim.scenario.world.centre_of(sim.ignition.centre);
    let mut order: Option<(Pos, f32)> = None;
    let scenario_name = sim.scenario.metadata.name.clone();
    let is_dev = sim.scenario.is_dev();
    let layer = *layer;

    egui::SidePanel::left("incident_dock")
        .resizable(open)
        .default_width(340.0)
        .width_range(if open { 280.0..=560.0 } else { 26.0..=26.0 })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                open = collapse_button(ui, open, "⏴", "⏵");
                if open {
                    ui.heading("Incident");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if is_dev {
                            ui.colored_label(egui::Color32::YELLOW, "🔧 DEV")
                                .on_hover_text("A synthetic test scenario, not real data.");
                        }
                        ui.label(egui::RichText::new(&scenario_name).weak());
                    });
                }
            });

            if !open {
                return;
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // The legend belongs next to the numbers, not next to the
                    // layer buttons in the menu bar: it explains what the map is
                    // currently coloured by, which is something you read while
                    // looking at the map rather than while switching layers.
                    ui.label(egui::RichText::new(format!("Map: {}", layer.label())).strong());
                    ui.small(layer.legend());

                    ui.add_space(6.0);
                    section(ui, "Fire");
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

                    ui.add_space(6.0);
                    section(ui, "Evacuation");
                    // The one number that answers "how is this going", above the
                    // breakdown that explains it. A bar rather than a ratio
                    // because the useful reading is at a glance, mid-incident.
                    let done = evac.safe as f32 / households.max(1) as f32;
                    ui.add(
                        egui::ProgressBar::new(done)
                            .desired_height(14.0)
                            .text(format!("{} of {households} households out", evac.safe)),
                    );
                    if evac.casualties > 0 || evac.cutoff > 0 {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 110, 90),
                            format!(
                                "⚠ {} cut off · {} casualties",
                                evac.cutoff, evac.casualties
                            ),
                        );
                    }
                    ui.add_space(4.0);
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
                        ui.label(format!(
                            "{} households · {} people",
                            evac.safe, evac.people_safe
                        ));
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
                 i place ignition · r restart · b behaviour composer · \
                 click a house/person/car to inspect it · drag orbit · \
                 right-drag pan · scroll zoom",
                    );
                });
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
    panels.incident = open;

    focus.0 = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
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
    focus.0 |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

fn help_english(ui: &mut egui::Ui) {
    ui.heading("What is this?");
    ui.label("You are the incident commander for a wildfire near Spotorno. The fire, weather, roads, households and response crews are simulated in real time. Your job is to watch the situation, protect people, and use the available crews where they can make a difference.");
    ui.add_space(8.0);
    ui.heading("A simple first run");
    ui.label("1. Look at the fire and choose a fire layer in the Incident panel.");
    ui.label("2. Order an evacuation when people may be at risk.");
    ui.label("3. Select a unit in Resources, choose an order, then click the map to place it.");
    ui.label("4. Press Play and adjust time acceleration as the incident develops.");
    ui.small("There is no score: use the information in the panels to see the consequences of each decision.");
    ui.add_space(8.0);
    ui.heading("Reading the panels");
    ui.label("Incident shows the clock, fire size, intensity, risk, structure losses and evacuation progress.");
    ui.label("Wildfire lets you change wind, moisture and ignitions; use Restart to run a revised scenario from T+0.");
    ui.label("Resources is where you select crews and assign direct attack, a fire line or a water drop. Request air support early: it takes time to arrive.");
    ui.label("Entities helps you find households, people, vehicles and units. Click something on the map or in the list to inspect it.");
    controls_guide(
        ui,
        [
            (
                "Move the view",
                "left-drag orbit · right-drag pan · scroll zoom",
            ),
            ("Run time", "Space play/pause · [ and ] change speed"),
            ("Evacuate", "E orders everyone out"),
            ("Fire layers", "1–4 switch the map overlay"),
            ("Ignition", "I, then click the map · R restarts"),
            (
                "Crew orders",
                "Tab next unit · A attack · L line · D drop · X stand down",
            ),
            (
                "Behaviours",
                "B opens the Agent Behaviour Composer, where the civilian decision model can be edited",
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
    ui.label("1. Osserva l'incendio e scegli un livello nella sezione Incident.");
    ui.label("2. Ordina l'evacuazione quando le persone potrebbero essere in pericolo.");
    ui.label("3. Seleziona una squadra in Resources, scegli un ordine e poi clicca sulla mappa per assegnarlo.");
    ui.label("4. Premi Play e regola l'accelerazione del tempo mentre l'emergenza evolve.");
    ui.small("Non c'è un punteggio: usa le informazioni nei pannelli per capire le conseguenze delle decisioni.");
    ui.add_space(8.0);
    ui.heading("Come leggere i pannelli");
    ui.label("Incident mostra l'orologio, l'estensione e l'intensità dell'incendio, il rischio, le strutture perse e l'avanzamento dell'evacuazione.");
    ui.label("Wildfire permette di cambiare vento, umidità e inneschi; usa Restart per ripartire da T+0 con uno scenario modificato.");
    ui.label("Resources serve per selezionare le squadre e assegnare un attacco diretto, una linea tagliafuoco o un lancio d'acqua. Richiedi presto il supporto aereo: serve tempo perché arrivi.");
    ui.label("Entities aiuta a trovare famiglie, persone, veicoli e squadre. Clicca un elemento sulla mappa o nell'elenco per ispezionarlo.");
    controls_guide(
        ui,
        [
            (
                "Muovere la visuale",
                "trascina a sinistra per ruotare · a destra per spostare · rotella per zoom",
            ),
            ("Tempo", "Spazio avvia/pausa · [ e ] cambiano velocità"),
            ("Evacuazione", "E ordina l'evacuazione generale"),
            (
                "Livelli incendio",
                "1–4 cambiano la sovrapposizione sulla mappa",
            ),
            ("Innesco", "I, poi clicca la mappa · R riavvia"),
            (
                "Ordini alle squadre",
                "Tab squadra successiva · A attacco · L linea · D lancio · X rientro",
            ),
            (
                "Comportamenti",
                "B apre l'Agent Behaviour Composer, dove si può modificare il modello decisionale dei civili",
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

/// The wildfire controls: weather, ignitions, restart.
///
/// Weather is staged rather than applied per-pixel-of-drag. Every change is a
/// scheduled boundary condition in the core, so applying on each frame of a
/// slider drag would push a hundred events for one gesture — the fire would
/// still be right, but the event heap would carry the whole drag. It commits on
/// release, and the Apply button covers keyboard entry.
pub fn wildfire_panel(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    mut tool: ResMut<IgnitionTool>,
    mut focus: ResMut<UiFocus>,
    mut panels: ResMut<PanelState>,
    mut restarted: EventWriter<SimRestarted>,
) {
    let ctx = contexts.ctx_mut();
    let mut open = panels.wildfire;

    let mut weather = sim.weather;
    let mut commit_weather = false;
    let mut do_restart = false;
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

    egui::SidePanel::right("wildfire_dock")
        .resizable(open)
        .default_width(300.0)
        .width_range(if open { 260.0..=520.0 } else { 26.0..=26.0 })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                open = collapse_button(ui, open, "⏵", "⏴");
                if open {
                    ui.heading("Wildfire");
                }
            });
            if !open {
                return;
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label("Wind");
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
                                egui::RichText::new(format!(
                                    "drives fire {}",
                                    cardinal((from + 180.0) % 360.0)
                                ))
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

                    ui.add_space(4.0);
                    ui.label("Fuel moisture");
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

                    ui.separator();
                    ui.label("Ignition");
                    let placing = mode == EditMode::Place;
                    if ui
                        .selectable_label(
                            placing,
                            if placing {
                                "◉ Click the map to light a fire"
                            } else {
                                "Place ignition  (i)"
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
                        egui::Slider::new(
                            &mut radius,
                            MIN_IGNITION_RADIUS_M..=MAX_IGNITION_RADIUS_M,
                        )
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

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Seed");
                        ui.add(egui::DragValue::new(&mut seed).speed(1.0));
                    });
                    let restart = ui
                        .add_sized(
                            [ui.available_width(), 28.0],
                            egui::Button::new("⟲  Restart simulation  (r)")
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
                });
        });

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
    panels.wildfire = open;

    focus.0 |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
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
