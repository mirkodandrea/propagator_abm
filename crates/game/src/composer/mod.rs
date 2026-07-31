//! **Agent Behaviour Composer** — the editor.
//!
//! A node canvas, a searchable palette, an inspector, a validation strip and a
//! test bench, in one window over the running incident. It edits the
//! [`behavior`] crate's data structures directly; nothing about the model
//! lives here.
//!
//! ### The one design decision worth knowing
//!
//! [`egui_snarl`] owns the live graph while the window is open, and
//! [`behavior::BehaviorGraph`] is derived from it. The alternative — keeping
//! the `BehaviorGraph` authoritative and pushing it into snarl each frame —
//! loses the editor's own state on every rebuild: which node is on top, what
//! is collapsed, where the view is scrolled. So snarl is the editing state,
//! `to_graph` is the projection, and the projection runs whenever anything
//! needs the model's view of the graph — which is every frame, because
//! validation is live. It costs a few microseconds on a graph of forty nodes,
//! and it means the canvas can never disagree with the validator.
//!
//! ### Applying an edit
//!
//! Editing does **not** touch the running simulation. The composer writes to
//! its own library, and `Apply` rebuilds the agent model through
//! [`crate::sim::Sim::apply_behaviour`] — which replays the ignition
//! list, so "same fire, different behaviour" is a genuine comparison rather
//! than a new roll of the dice. Changing behaviour mid-run without a restart
//! would give households a decision layer that disagrees with the one that
//! produced the state they are in.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui_snarl::{ui::SnarlStyle, Snarl};

use behavior::{
    BehaviorGraph, Library, Observation, ParamValue, Report, Wire,
};

mod bench;
mod inspector;
mod palette;
mod subtypes;
mod viewer;

pub use viewer::EditorNode;

/// Which panel the right-hand column is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightTab {
    #[default]
    Inspector,
    Subtypes,
    Bench,
}

/// The composer's whole state.
#[derive(Resource)]
pub struct Composer {
    pub open: bool,
    /// Where the library is read from and written to.
    pub root: PathBuf,
    pub lib: Library,

    /// The graph being edited, as snarl sees it.
    pub snarl: Snarl<EditorNode>,
    /// Id of the graph in `snarl`.
    pub graph_id: String,
    pub graph_name: String,
    pub graph_description: String,

    /// Live validation of `snarl`, recomputed every frame it is drawn.
    pub report: Report,
    /// The projection of `snarl`, kept alongside the report it produced.
    pub graph: BehaviorGraph,

    pub right: RightTab,
    pub palette_query: String,
    /// Where a palette click drops a node, in graph space.
    pub drop_at: egui::Pos2,
    pub selected: Option<egui_snarl::NodeId>,

    /// Subtype being edited, and the one it is being compared with.
    pub subtype: Option<String>,
    pub compare_with: Option<String>,

    pub bench: bench::Bench,

    /// One line under the toolbar: what just happened.
    pub status: String,
    pub status_is_error: bool,
    /// Set when the library has changed since the model was last rebuilt.
    pub dirty: bool,
}

/// Ask the game to rebuild the agent model on the composer's library.
#[derive(Event)]
pub struct ApplyBehaviour;

impl Composer {
    fn new(root: PathBuf) -> Composer {
        let lib = Library::load_or_default(&root);
        let graph_id = lib
            .graphs
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| behavior::defaults::DEFAULT_GRAPH_ID.to_string());
        let mut c = Composer {
            // `SPOTORNO_COMPOSER` opens it with the scenario, so an
            // unattended screenshot run can capture the editor; its value
            // picks the right-hand tab. In play it is opened with `b`.
            open: std::env::var("SPOTORNO_COMPOSER").is_ok(),
            root,
            lib,
            snarl: Snarl::new(),
            graph_id: graph_id.clone(),
            graph_name: String::new(),
            graph_description: String::new(),
            report: Report::default(),
            graph: BehaviorGraph::new(&graph_id, ""),
            right: match std::env::var("SPOTORNO_COMPOSER").as_deref() {
                Ok("subtypes") => RightTab::Subtypes,
                Ok("test") => RightTab::Bench,
                _ => RightTab::Inspector,
            },
            palette_query: String::new(),
            drop_at: egui::Pos2::ZERO,
            selected: None,
            subtype: None,
            compare_with: None,
            bench: bench::Bench::default(),
            status: String::new(),
            status_is_error: false,
            dirty: false,
        };
        c.load_graph(&graph_id);
        c.subtype = c.lib.subtypes.keys().next().cloned();
        c
    }

    /// Pull a graph out of the library and into the canvas.
    pub fn load_graph(&mut self, id: &str) {
        let Some(g) = self.lib.graphs.get(id).cloned() else {
            self.set_error(format!("no graph \"{id}\""));
            return;
        };
        self.graph_id = g.id.clone();
        self.graph_name = g.name.clone();
        self.graph_description = g.description.clone();
        self.snarl = Snarl::new();
        self.selected = None;

        // Snarl hands out its own ids on insert, so the graph's ids are
        // translated rather than preserved. That is fine going in; coming back
        // out, `to_graph` uses snarl's ids, which is what subtype overrides are
        // then keyed on. The two agree because a load is immediately followed
        // by a `sync`, and `prune_overrides` remaps anything that moved.
        let mut map: BTreeMap<behavior::NodeId, egui_snarl::NodeId> = BTreeMap::new();
        for n in &g.nodes {
            let sid = self.snarl.insert_node(
                egui::pos2(n.pos[0], n.pos[1]),
                EditorNode {
                    type_id: n.type_id.clone(),
                    params: n.params.clone(),
                    comment: n.comment.clone(),
                },
            );
            map.insert(n.id, sid);
        }
        for w in &g.wires {
            let (Some(&from), Some(&to)) = (map.get(&w.from_node), map.get(&w.to_node)) else {
                continue;
            };
            self.snarl.connect(
                egui_snarl::OutPinId { node: from, output: w.from_port as usize },
                egui_snarl::InPinId { node: to, input: w.to_port as usize },
            );
        }

        // Overrides were keyed on the *file's* node ids; after the translation
        // they have to be keyed on snarl's. Doing it here rather than lazily
        // means a subtype never silently stops overriding anything.
        self.remap_overrides(id, &map);
        self.sync();
    }

    fn remap_overrides(&mut self, graph_id: &str, map: &BTreeMap<behavior::NodeId, egui_snarl::NodeId>) {
        for s in self.lib.subtypes.values_mut() {
            if s.graph != graph_id {
                continue;
            }
            let remapped = s
                .overrides
                .iter()
                .filter_map(|(k, v)| {
                    let (node, param) = behavior::subtype::split_key(k)?;
                    let sid = map.get(&node)?;
                    Some((BehaviorGraph::override_key(sid.0 as u32, param), v.clone()))
                })
                .collect();
            s.overrides = remapped;
        }
    }

    /// Project the canvas back into a [`BehaviorGraph`] and revalidate.
    pub fn sync(&mut self) {
        self.graph = self.to_graph();
        self.report = behavior::validate(&self.graph);
    }

    pub fn to_graph(&self) -> BehaviorGraph {
        let mut g = BehaviorGraph::new(&self.graph_id, &self.graph_name);
        g.description = self.graph_description.clone();
        for (id, pos, node) in self.snarl.nodes_pos_ids() {
            g.nodes.push(behavior::GraphNode {
                id: id.0 as behavior::NodeId,
                type_id: node.type_id.clone(),
                pos: [pos.x, pos.y],
                params: node.params.clone(),
                comment: node.comment.clone(),
            });
        }
        g.nodes.sort_by_key(|n| n.id);
        for (out, inp) in self.snarl.wires() {
            g.wires.push(Wire {
                from_node: out.node.0 as behavior::NodeId,
                from_port: out.output as u16,
                to_node: inp.node.0 as behavior::NodeId,
                to_port: inp.input as u16,
            });
        }
        g
    }

    /// Write the canvas into the library. Does not touch disk.
    pub fn commit(&mut self) {
        self.sync();
        self.lib.graphs.insert(self.graph_id.clone(), self.graph.clone());
        self.dirty = true;
    }

    pub fn save(&mut self) {
        self.commit();
        match self.lib.save_dir(&self.root) {
            Ok(()) => self.set_status(format!("saved to {}", self.root.display())),
            Err(e) => self.set_error(format!("{e:#}")),
        }
    }

    pub fn reload(&mut self) {
        match Library::load_dir(&self.root) {
            Ok(lib) if !lib.graphs.is_empty() => {
                self.lib = lib;
                let id = self.graph_id.clone();
                let first = self.lib.graphs.keys().next().cloned().unwrap_or_default();
                self.load_graph(if self.lib.graphs.contains_key(&id) { &id } else { &first });
                self.set_status("reloaded from disk".into());
            }
            Ok(_) => self.set_error("nothing saved there yet".into()),
            Err(e) => self.set_error(format!("{e:#}")),
        }
    }

    pub fn set_status(&mut self, s: String) {
        self.status = s;
        self.status_is_error = false;
    }

    pub fn set_error(&mut self, s: String) {
        self.status = s;
        self.status_is_error = true;
    }

    /// Whether the current graph could be run right now.
    pub fn runnable(&self) -> bool {
        self.report.ok()
    }

    /// The subtype currently selected, if it is on this graph.
    pub fn active_overrides(&self) -> behavior::Overrides {
        self.subtype
            .as_ref()
            .and_then(|id| self.lib.subtypes.get(id))
            .filter(|s| s.graph == self.graph_id)
            .map(|s| s.overrides.clone())
            .unwrap_or_default()
    }
}

/// Editor chrome. Built per frame rather than stored: `SnarlStyle` carries a
/// boxed closure for its background pattern, so it is neither `Send` nor
/// `Sync` and cannot live in a Bevy resource. It is a handful of `Option`s;
/// constructing it costs nothing.
pub(crate) fn editor_style() -> SnarlStyle {
    SnarlStyle {
        pin_size: Some(7.0),
        wire_width: Some(2.5),
        wire_frame_size: Some(24.0),
        collapsible: Some(true),
        ..SnarlStyle::new()
    }
}

pub struct ComposerPlugin;

impl Plugin for ComposerPlugin {
    fn build(&self, app: &mut App) {
        // Read from the environment rather than from `DataPath`: the resource
        // is inserted after the plugins are added, so it is not there yet, and
        // a plugin that silently defaulted to `data/` under `SPOTORNO_DATA`
        // would edit one library and run another.
        let root = PathBuf::from(std::env::var("SPOTORNO_DATA").unwrap_or_else(|_| "data".into()))
            .join(behavior::library::DEFAULT_DIR);

        app.insert_resource(Composer::new(root))
            .add_event::<ApplyBehaviour>()
            // Bevy's reflection registry, so the composer's value types are
            // visible to anything that walks the type registry — the entity
            // inspector, a future save-game, `bevy_remote`. It costs nothing
            // and it is the difference between these being Bevy types and
            // being opaque blobs Bevy happens to store.
            .register_type::<behavior::Value>()
            .register_type::<behavior::ValueType>()
            .register_type::<behavior::ActionKind>()
            .register_type::<behavior::ActionProposal>()
            .register_type::<behavior::IntentValue>()
            .register_type::<behavior::ParamValue>()
            .register_type::<Observation>()
            .add_systems(
                Update,
                (toggle, window)
                    .chain()
                    .run_if(in_state(crate::AppState::Playing)),
            );
    }
}

/// `b` opens and closes the composer.
///
/// Deliberately not a left-click tool: the composer is a modal workbench, not
/// a fourth thing contending for the pointer over the map (see the
/// three-tools rule in CLAUDE.md).
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut composer: ResMut<Composer>,
    focus: Res<crate::ui::UiFocus>,
) {
    if focus.0 {
        return;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        composer.open = !composer.open;
    }
}

fn window(
    mut contexts: EguiContexts,
    mut composer: ResMut<Composer>,
    mut focus: ResMut<crate::ui::UiFocus>,
    mut apply: EventWriter<ApplyBehaviour>,
) {
    if !composer.open {
        return;
    }
    let ctx = contexts.ctx_mut();
    let c = &mut *composer;

    let mut open = true;
    egui::Window::new("Agent Behaviour Composer")
        .open(&mut open)
        // Sized for the three columns it has to hold at once: palette,
        // canvas, inspector. Below the minimum the canvas stops being a canvas
        // and the window is better dragged bigger than laid out differently.
        .default_size([1460.0, 900.0])
        .min_size([1040.0, 640.0])
        .vscroll(false)
        .show(ctx, |ui| {
            toolbar(ui, c, &mut apply);
            ui.separator();
            egui::SidePanel::left("composer-palette")
                .resizable(true)
                .default_width(260.0)
                .show_inside(ui, |ui| palette::panel(ui, c));
            egui::SidePanel::right("composer-inspector")
                .resizable(true)
                .default_width(360.0)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut c.right, RightTab::Inspector, "Inspector");
                        ui.selectable_value(&mut c.right, RightTab::Subtypes, "Subtypes");
                        ui.selectable_value(&mut c.right, RightTab::Bench, "Test");
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| match c.right {
                        RightTab::Inspector => inspector::panel(ui, c),
                        RightTab::Subtypes => subtypes::panel(ui, c),
                        RightTab::Bench => bench::panel(ui, c),
                    });
                });
            egui::TopBottomPanel::bottom("composer-issues")
                .resizable(false)
                .show_inside(ui, |ui| issues(ui, c));
            egui::CentralPanel::default().show_inside(ui, |ui| viewer::canvas(ui, c));
        });

    // The canvas eats drags and the text fields eat keys; without this the
    // camera orbits behind the window and `space` pauses the sim mid-rename.
    focus.0 |= ctx.is_pointer_over_area() || ctx.wants_keyboard_input();

    composer.open &= open;
}

fn toolbar(ui: &mut egui::Ui, c: &mut Composer, apply: &mut EventWriter<ApplyBehaviour>) {
    ui.horizontal_wrapped(|ui| {
        let current = c.graph_id.clone();
        egui::ComboBox::from_id_source("composer-graph")
            .selected_text(c.graph_name.clone())
            .show_ui(ui, |ui| {
                let ids: Vec<(String, String)> =
                    c.lib.graphs.values().map(|g| (g.id.clone(), g.name.clone())).collect();
                for (id, name) in ids {
                    if ui.selectable_label(id == current, name).clicked() && id != current {
                        c.commit();
                        c.load_graph(&id);
                    }
                }
            });

        if ui.button("New").on_hover_text("Start an empty behaviour").clicked() {
            c.commit();
            let id = c.lib.free_id("new-behaviour", true);
            let mut g = BehaviorGraph::new(&id, "New behaviour");
            // An empty canvas cannot validate — every graph needs the one
            // sink — so it opens with it already placed rather than opening
            // on an error.
            g.add("out.decision", [600.0, 200.0]);
            c.lib.graphs.insert(id.clone(), g);
            c.load_graph(&id);
            c.set_status("new behaviour".into());
        }

        if ui.button("Duplicate").clicked() {
            c.commit();
            let id = c.lib.free_id(&format!("{}-copy", c.graph_id), true);
            let mut g = c.graph.clone();
            g.id = id.clone();
            g.name = format!("{} (copy)", g.name);
            c.lib.graphs.insert(id.clone(), g);
            c.load_graph(&id);
            c.set_status("duplicated".into());
        }

        ui.separator();
        if ui.button("Save").clicked() {
            c.save();
        }
        if ui.button("Reload").on_hover_text("Discard edits and re-read the files").clicked() {
            c.reload();
        }

        ui.separator();
        let runnable = c.runnable();
        let btn = egui::Button::new(if c.dirty { "Apply and restart *" } else { "Apply and restart" });
        let resp = ui.add_enabled(runnable, btn).on_hover_text(
            "Rebuild the agent model on this library and replay the incident from the start. \
             The fire, the weather and the ignition list are unchanged, so this is a like-for-like \
             comparison.",
        );
        if resp.clicked() {
            c.commit();
            apply.send(ApplyBehaviour);
        }
        if !runnable {
            ui.colored_label(
                egui::Color32::from_rgb(0xe0, 0x6c, 0x5f),
                format!("{} error(s)", c.report.error_count()),
            );
        }
    });

    ui.horizontal(|ui| {
        ui.label("Name");
        if ui.text_edit_singleline(&mut c.graph_name).changed() {
            c.dirty = true;
        }
    });
    if !c.status.is_empty() {
        let colour = if c.status_is_error {
            egui::Color32::from_rgb(0xe0, 0x6c, 0x5f)
        } else {
            egui::Color32::from_rgb(0x7a, 0xb2, 0x8a)
        };
        ui.colored_label(colour, &c.status);
    }
}

/// The validation strip. Errors first, then warnings; clicking one selects the
/// node it is about, which is the only way to find it on a canvas of forty.
fn issues(ui: &mut egui::Ui, c: &mut Composer) {
    let errors = c.report.error_count();
    let warnings = c.report.warning_count();
    ui.horizontal(|ui| {
        if errors == 0 {
            ui.colored_label(egui::Color32::from_rgb(0x7a, 0xb2, 0x8a), "✔ valid");
        } else {
            ui.colored_label(
                egui::Color32::from_rgb(0xe0, 0x6c, 0x5f),
                format!("✖ {errors} error(s)"),
            );
        }
        if warnings > 0 {
            ui.colored_label(
                egui::Color32::from_rgb(0xd8, 0xa6, 0x4b),
                format!("⚠ {warnings} warning(s)"),
            );
        }
    });

    if errors + warnings == 0 {
        return;
    }
    egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
        let mut issues: Vec<behavior::Issue> = c.report.issues.clone();
        issues.sort_by_key(|i| i.severity);
        for issue in issues {
            let (glyph, colour) = match issue.severity {
                behavior::Severity::Error => ("✖", egui::Color32::from_rgb(0xe0, 0x6c, 0x5f)),
                behavior::Severity::Warning => ("⚠", egui::Color32::from_rgb(0xd8, 0xa6, 0x4b)),
            };
            ui.horizontal(|ui| {
                ui.colored_label(colour, glyph);
                let r = ui.label(&issue.message);
                if let Some(n) = issue.node {
                    if r.interact(egui::Sense::click()).clicked() {
                        c.selected = Some(egui_snarl::NodeId(n as usize));
                        c.right = RightTab::Inspector;
                    }
                }
            });
        }
    });
}

/// Render a parameter, returning whether it changed. Shared by the node
/// inspector and the subtype override list, which is why it is here and not in
/// either.
pub(crate) fn param_widget(
    ui: &mut egui::Ui,
    spec: &behavior::ParamSpec,
    value: &mut ParamValue,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(spec.label).on_hover_text(spec.doc);
        match (spec.kind, value) {
            (behavior::ParamKind::Number { min, max, unit, .. }, ParamValue::Number(n)) => {
                // A wide slider on an unbounded range is useless, so anything
                // whose declared range is huge gets a drag value instead.
                let wide = (max - min) > 1000.0;
                changed |= if wide {
                    ui.add(egui::DragValue::new(n).speed(0.5).suffix(unit)).changed()
                } else {
                    ui.add(egui::Slider::new(n, min..=max).suffix(unit)).changed()
                };
            }
            (behavior::ParamKind::Bool { .. }, ParamValue::Bool(b)) => {
                changed |= ui.checkbox(b, "").changed();
            }
            (behavior::ParamKind::Choice { options, .. }, ParamValue::Choice(s)) => {
                egui::ComboBox::from_id_source(spec.name)
                    .selected_text(pretty(s))
                    .show_ui(ui, |ui| {
                        for opt in options {
                            if ui.selectable_label(s == opt, pretty(opt)).clicked() {
                                *s = (*opt).to_string();
                                changed = true;
                            }
                        }
                    });
            }
            (behavior::ParamKind::Text { .. }, ParamValue::Text(s)) => {
                changed |= ui.text_edit_singleline(s).changed();
            }
            // A saved file whose parameter changed kind. The validator has
            // already reported it; showing the raw value beats showing nothing.
            (_, v) => {
                ui.colored_label(egui::Color32::from_rgb(0xe0, 0x6c, 0x5f), v.display());
            }
        }
    });
    changed
}

/// `wait_and_see` -> `Wait and see`.
pub(crate) fn pretty(key: &str) -> String {
    let mut s = key.replace('_', " ");
    if let Some(first) = s.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    s
}
