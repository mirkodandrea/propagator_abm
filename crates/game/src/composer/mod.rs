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

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_snarl::{ui::SnarlStyle, Snarl};

use behavior::{BehaviorGraph, Domain, Library, Observation, ParamValue, Report, Wire};

pub mod bench;
mod help;
mod history;
mod composition;
mod inspector;
pub mod live;
mod palette;
mod subtypes;
mod structure;
mod viewer;

pub use viewer::EditorNode;

/// Which panel the right-hand column is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightTab {
    #[default]
    Inspector,
    Subtypes,
    Bench,
    /// What the selected agent's behaviour is doing right now.
    Live,
    Help,
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
    /// Which kind of agent the graph in `snarl` is about.
    ///
    /// Held here rather than read off `self.graph`, because `self.graph` is the
    /// *output* of [`Composer::to_graph`] and `to_graph` needs the domain to
    /// build it. Without this the projection would default every graph back to
    /// a household one on the first sync — which validates, and silently makes
    /// a unit policy into a broken civilian behaviour.
    pub graph_domain: Domain,

    /// Live validation of `snarl`, recomputed every frame it is drawn.
    pub report: Report,
    /// The projection of `snarl`, kept alongside the report it produced.
    pub graph: BehaviorGraph,

    pub right: RightTab,
    pub palette_query: String,
    /// Where a palette click drops a node, in graph space.
    pub drop_at: egui::Pos2,
    pub selected: Option<egui_snarl::NodeId>,
    pub view_revision: u64,
    pub wiring: bool,

    /// Subtype being edited, and the one it is being compared with.
    pub subtype: Option<String>,
    pub compare_with: Option<String>,

    pub bench: bench::Bench,
    /// Watching one agent's behaviour run. See [`live`].
    pub live: live::Live,

    /// What the last read of the library directory found, file by file.
    ///
    /// Kept so the Help tab can list what is on disk *and what would not load*.
    /// A malformed file that is silently skipped is the worst of both: the
    /// author's edit is gone and nothing says where.
    pub load_report: Vec<behavior::FileReport>,
    /// Path typed into the import/export field.
    pub transfer_path: String,

    /// One line under the toolbar: what just happened.
    pub status: String,
    pub status_is_error: bool,
    /// Set when the library has changed since the model was last rebuilt.
    pub dirty: bool,
    applied: Library,
    saved: Library,
    history: history::History,
}

/// Ask the game to rebuild the agent model on the composer's library.
#[derive(Event)]
pub struct ApplyBehaviour;

impl Composer {
    fn new(root: PathBuf) -> Composer {
        // The reported form, so the Help tab can show which files loaded and
        // which did not. The library comes only from disk: an empty or invalid
        // directory is surfaced when the simulation tries to start, rather
        // than silently swapping in a compiled behavior.
        let (lib, load_report) = read_library(&root);
        // Open on a household behaviour when there is one: it is the domain the
        // composer is mostly used for, and opening on whichever id sorts first
        // would make that a property of the alphabet.
        let graph_id = lib
            .graphs
            .values()
            .find(|g| g.domain == Domain::Household)
            .or_else(|| lib.graphs.values().next())
            .map(|g| g.id.clone())
            .unwrap_or_else(|| behavior::defaults::DEFAULT_GRAPH_ID.to_string());
        let mut c = Composer {
            // `SPOTORNO_COMPOSER` opens it with the scenario, so an
            // unattended screenshot run can capture the editor; its value
            // picks the right-hand tab. In play it is opened with `b`.
            open: std::env::var("SPOTORNO_COMPOSER").is_ok(),
            root,
            applied: history::normalized(&lib),
            saved: history::normalized(&lib),
            history: history::History::default(),
            lib,
            snarl: Snarl::new(),
            graph_id: graph_id.clone(),
            graph_name: String::new(),
            graph_description: String::new(),
            graph_domain: Domain::Household,
            report: Report::default(),
            graph: BehaviorGraph::new(&graph_id, ""),
            right: match std::env::var("SPOTORNO_COMPOSER").as_deref() {
                Ok("subtypes") => RightTab::Subtypes,
                Ok("test") => RightTab::Bench,
                Ok("live") => RightTab::Live,
                Ok("help") => RightTab::Help,
                _ => RightTab::Inspector,
            },
            palette_query: String::new(),
            drop_at: egui::Pos2::ZERO,
            selected: None,
            view_revision: 0,
            wiring: false,
            subtype: None,
            compare_with: None,
            bench: bench::Bench::default(),
            live: live::Live::default(),
            load_report,
            transfer_path: String::new(),
            status: String::new(),
            status_is_error: false,
            dirty: false,
        };
        c.load_graph(&graph_id);
        c.subtype = c.first_subtype_in(c.domain());
        // `SPOTORNO_COMPOSER_DOMAIN` opens on the other kind of agent. The
        // editor shows one domain at a time and the switch is a click, so this
        // is the only way to screenshot the suppression side unattended — the
        // same reason `SPOTORNO_COMPOSER` picks the right-hand tab.
        if let Ok(key) = std::env::var("SPOTORNO_COMPOSER_DOMAIN") {
            match Domain::from_key(&key).or(match key.as_str() {
                "units" | "unit" | "suppression" => Some(Domain::SuppressionUnit),
                "civilians" | "households" => Some(Domain::Household),
                "people" | "persons" | "separated" => Some(Domain::Person),
                _ => None,
            }) {
                Some(d) => c.switch_domain(d),
                None => c.set_error(format!("no such agent kind: {key:?}")),
            }
        }
        c.checkpoint(false);
        c
    }

    /// The snarl handle for a node the model, the validator or a trace names.
    ///
    /// Those all speak [`behavior::NodeId`]; snarl speaks its own. The two used
    /// to be the same number, which is exactly what made them impossible to keep
    /// in step — see [`EditorNode`].
    pub fn snarl_id_of(&self, id: behavior::NodeId) -> Option<egui_snarl::NodeId> {
        self.snarl
            .node_ids()
            .find(|(_, n)| n.id == id)
            .map(|(sid, _)| sid)
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
        self.graph_domain = g.domain;
        self.snarl = viewer::snarl_from_graph(&g);
        self.selected = None;
        if self.subtype.as_ref().and_then(|id| self.lib.subtypes.get(id))
            .is_none_or(|s| s.graph != self.graph_id) {
            self.subtype = None;
        }
        self.sync();
    }

    /// Project the canvas back into a [`BehaviorGraph`] and revalidate.
    pub fn sync(&mut self) {
        self.graph = self.to_graph();
        self.report = behavior::validate(&self.graph);
    }

    pub fn to_graph(&self) -> BehaviorGraph {
        let mut g = BehaviorGraph::new_in(self.graph_domain, &self.graph_id, &self.graph_name);
        g.description = self.graph_description.clone();
        for (_, pos, node) in self.snarl.nodes_pos_ids() {
            g.nodes.push(behavior::GraphNode {
                id: node.id,
                type_id: node.type_id.clone(),
                pos: [pos.x, pos.y],
                params: node.params.clone(),
                comment: node.comment.clone(),
            });
        }
        g.nodes.sort_by_key(|n| n.id);
        let id_of = |sid: egui_snarl::NodeId| self.snarl.get_node(sid).map(|n| n.id);
        for (out, inp) in self.snarl.wires() {
            let (Some(from_node), Some(to_node)) = (id_of(out.node), id_of(inp.node)) else {
                continue;
            };
            g.wires.push(Wire {
                from_node,
                from_port: out.output as u16,
                to_node,
                to_port: inp.input as u16,
            });
        }
        g
    }

    /// Write the canvas into the library. Does not touch disk.
    pub fn commit(&mut self) {
        self.sync();
        // Remove overrides at the same time as their nodes. Otherwise a new
        // node reusing an id could inherit settings meant for the deleted one.
        let removed: Vec<_> = self.lib.graphs.get(&self.graph_id).into_iter()
            .flat_map(|g| &g.nodes).filter(|n| self.graph.node(n.id).is_none())
            .map(|n| format!("{}.", n.id)).collect();
        for profile in self.lib.subtypes.values_mut().filter(|s| s.graph == self.graph_id) {
            profile.overrides.retain(|key, _| !removed.iter().any(|prefix| key.starts_with(prefix)));
        }
        self.lib.graphs.insert(self.graph_id.clone(), self.graph.clone());
        self.dirty = history::normalized(&self.lib) != self.applied;
    }

    fn checkpoint(&mut self, grouping: bool) {
        self.commit();
        self.history.record(history::Snapshot {
            lib: history::normalized(&self.lib), graph_id: self.graph_id.clone(), subtype: self.subtype.clone(),
        }, grouping);
    }

    fn restore(&mut self, snapshot: history::Snapshot) {
        self.lib = snapshot.lib;
        self.subtype = snapshot.subtype;
        self.load_graph(&snapshot.graph_id);
        self.compare_with = None;
        self.dirty = history::normalized(&self.lib) != self.applied;
        self.view_revision += 1;
    }

    pub fn undo(&mut self) {
        self.checkpoint(false);
        if let Some(snapshot) = self.history.undo() { self.restore(snapshot); }
    }

    pub fn redo(&mut self) {
        self.checkpoint(false);
        if let Some(snapshot) = self.history.redo() { self.restore(snapshot); }
    }

    pub fn mark_applied(&mut self, applied: Library) {
        self.applied = history::normalized(&applied);
        self.commit();
    }

    fn unsaved(&self) -> bool { history::normalized(&self.lib) != self.saved }

    fn select_profile(&mut self, id: &str) {
        let Some(graph) = self.lib.subtypes.get(id).map(|s| s.graph.clone()) else { return };
        if !self.lib.graphs.contains_key(&graph) {
            self.set_error(format!("Profile references missing behavior: {graph}"));
            return;
        }
        self.commit();
        if graph != self.graph_id { self.load_graph(&graph); }
        self.subtype = Some(id.to_string());
    }

    fn delete_profile(&mut self, id: &str) {
        self.lib.subtypes.remove(id);
        if self.subtype.as_deref() == Some(id) { self.subtype = None; }
        self.dirty = true;
        self.set_status("Profile removed from draft. Save to keep this change.".into());
    }

    pub fn save(&mut self) {
        self.commit();
        let result = self.lib.save_dir(&self.root).and_then(|()| {
            let deleted: Vec<_> = self.saved.subtypes.keys().filter(|id| !self.lib.subtypes.contains_key(*id)).cloned().collect();
            let mut previous = self.saved.clone();
            for id in deleted { previous.delete_subtype(&self.root, &id)?; }
            Ok(())
        });
        match result {
            Ok(()) => {
                self.saved = history::normalized(&self.lib);
                let (graphs, subtypes) = (self.lib.graphs.len(), self.lib.subtypes.len());
                self.set_status(format!(
                    "saved {graphs} behaviour(s) and {subtypes} profile(s) to {}",
                    self.root.display()
                ));
                // The listing the Help tab shows is now out of date by exactly
                // the files just written.
                self.refresh_load_report();
            }
            Err(e) => self.set_error(format!("{e:#}")),
        }
    }

    pub fn reload(&mut self) {
        let (lib, report) = read_library(&self.root);
        self.load_report = report;
        if self.load_report.iter().any(|f| !f.ok()) {
            self.set_error("Reload cancelled: some files could not load. Your edits are kept. See Help → Files on disk.".into());
            return;
        }
        if lib.graphs.is_empty() {
            self.set_error("nothing saved there yet".into());
            return;
        }
        self.saved = history::normalized(&lib);
        self.lib = lib;
        let id = self.graph_id.clone();
        let first = self.lib.graphs.keys().next().cloned().unwrap_or_default();
        self.load_graph(if self.lib.graphs.contains_key(&id) {
            &id
        } else {
            &first
        });
        self.dirty = true;

        self.set_status(format!(
            "Reloaded {} behaviour(s) and {} profile(s). Apply and restart to run them.",
            self.lib.graphs.len(),
            self.lib.subtypes.len()
        ));
    }

    fn refresh_load_report(&mut self) {
        self.load_report = read_library(&self.root).1;
    }

    /// Read one behaviour or profile from an arbitrary path into the library.
    ///
    /// Does not overwrite: an id already in the library gets a free one, and the
    /// status says so. Silently replacing a graph someone is editing with one
    /// from a file they were only looking at is the kind of thing an editor gets
    /// exactly one chance to do.
    pub fn import(&mut self, path: &str) {
        let path = std::path::Path::new(path.trim());
        if path.as_os_str().is_empty() {
            self.set_error("give a path to a .json file".into());
            return;
        }
        match Library::import_file(path) {
            Ok(behavior::Imported::Graph(mut g)) => {
                let wanted = g.id.clone();
                let id = self.lib.free_id(&g.id, true);
                let renamed = id != wanted;
                g.id = id.clone();
                let domain = g.domain;
                self.lib.graphs.insert(id.clone(), g);
                self.commit();
                self.graph_domain = domain;
                self.load_graph(&id);
                self.subtype = self.first_subtype_in(domain);
                self.dirty = true;
                self.set_status(if renamed {
                    format!("imported behaviour as \"{id}\" — \"{wanted}\" was taken")
                } else {
                    format!("imported behaviour \"{id}\"")
                });
            }
            Ok(behavior::Imported::Subtype(mut s)) => {
                let wanted = s.id.clone();
                let id = self.lib.free_id(&s.id, false);
                let renamed = id != wanted;
                s.id = id.clone();
                let missing = !self.lib.graphs.contains_key(&s.graph);
                let graph = s.graph.clone();
                self.lib.subtypes.insert(id.clone(), s);
                self.subtype = Some(id.clone());
                self.right = RightTab::Subtypes;
                self.dirty = true;
                if missing {
                    // It loaded; it just cannot run. Reported rather than
                    // refused, because importing the profile and then its graph
                    // is a perfectly ordinary order to do it in.
                    self.set_error(format!(
                        "imported profile \"{id}\", but its behaviour \"{graph}\" is not loaded"
                    ));
                } else {
                    self.set_status(if renamed {
                        format!("imported profile as \"{id}\" — \"{wanted}\" was taken")
                    } else {
                        format!("imported profile \"{id}\"")
                    });
                }
            }
            Err(e) => self.set_error(format!("{e:#}")),
        }
    }

    /// Write the open behaviour, or the selected profile, to an arbitrary path.
    pub fn export(&mut self, path: &str, graph: bool) {
        let path = std::path::Path::new(path.trim());
        if path.as_os_str().is_empty() {
            self.set_error("give a path to write to".into());
            return;
        }
        let r = if graph {
            self.commit();
            Library::export_graph(&self.graph, path)
        } else {
            match self
                .subtype
                .as_ref()
                .and_then(|id| self.lib.subtypes.get(id))
            {
                Some(s) => Library::export_subtype(s, path),
                None => {
                    self.set_error("no profile selected".into());
                    return;
                }
            }
        };
        match r {
            Ok(()) => self.set_status(format!("wrote {}", path.display())),
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

    /// Which kind of agent the editor is currently working on.
    ///
    /// Read off the loaded graph rather than held as its own field: two places
    /// for the answer is two places that can disagree, and the one that would
    /// win is whichever the palette happened to consult.
    pub fn domain(&self) -> Domain {
        self.graph_domain
    }

    /// Switch the whole editor to another kind of agent.
    ///
    /// Loads that domain's first graph, or starts one if it has none. The
    /// editor deliberately shows one domain at a time: the palettes are
    /// disjoint, the test bench situations are disjoint, and a canvas holding
    /// both would be a canvas where half the boxes silently do nothing.
    pub fn switch_domain(&mut self, d: Domain) {
        if self.domain() == d {
            return;
        }
        self.commit();
        let first = self
            .lib
            .graphs
            .values()
            .find(|g| g.domain == d)
            .map(|g| g.id.clone());
        match first {
            Some(id) => self.load_graph(&id),
            None => self.new_graph(d),
        }
        // The selected profile almost certainly belonged to the old domain.
        self.subtype = self.first_subtype_in(d);
        self.compare_with = None;
        self.selected = None;
    }

    /// Start an empty behaviour in `domain`, and open it.
    pub fn new_graph(&mut self, domain: Domain) {
        let id = self.lib.free_id(
            match domain {
                Domain::Household => "new-behaviour",
                Domain::SuppressionUnit => "new-unit-policy",
                Domain::Person => "new-person-behaviour",
            },
            true,
        );
        let mut g = BehaviorGraph::new_in(domain, &id, "New behaviour");
        // An empty canvas cannot validate — every graph needs its domain's one
        // sink — so it opens with it already placed rather than opening on an
        // error.
        g.add(domain.decision_output(), [600.0, 200.0]);
        self.lib.graphs.insert(id.clone(), g);
        self.dirty = true;
        self.load_graph(&id);
        self.set_status(format!("new {} behaviour", domain.label().to_lowercase()));
    }

    /// The first profile belonging to `domain`, for when the selection has to
    /// be replaced.
    ///
    /// Prefers one that is actually in play. Ids sort alphabetically, so
    /// without this the editor opens on whichever profile happens to sort
    /// first — which is quite likely to be a disabled one, and a bench showing
    /// the answers of a profile no agent is running is a bench that misleads.
    pub fn first_subtype_in(&self, domain: Domain) -> Option<String> {
        let mine = || {
            self.lib
                .subtypes
                .values()
                .filter(move |s| self.lib.domain_of(s) == Some(domain))
        };
        let live = |s: &&behavior::AgentSubtype| match domain {
            Domain::Household | Domain::Person => s.share > 0.0,
            Domain::SuppressionUnit => s.enabled,
        };
        mine()
            .find(live)
            .or_else(|| mine().next())
            .map(|s| s.id.clone())
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

/// Read the library directory, keeping the per-file report.
///
/// Lenient about individual files by design: one graph with a stray comma costs
/// that graph and not the other nine, and the report is what turns a skipped
/// file from a silent loss into a line in the Help tab.
#[cfg(not(target_arch = "wasm32"))]
fn read_library(root: &PathBuf) -> (Library, Vec<behavior::FileReport>) {
    match Library::load_dir_reported(root) {
        Ok(r) => (r.library, r.files),
        Err(e) => {
            eprintln!("behaviour library: {e:#}");
            (Library::default(), Vec::new())
        }
    }
}

/// Browser builds have no filesystem from which to discover the authored
/// library. The built-in form is generated from the same shipped graphs and
/// profiles and keeps the self-contained WASM bundle launchable.
#[cfg(target_arch = "wasm32")]
fn read_library(_root: &PathBuf) -> (Library, Vec<behavior::FileReport>) {
    (behavior::defaults::default_library(), Vec::new())
}

/// Editor chrome. Built per frame rather than stored: `SnarlStyle` carries a
/// boxed closure for its background pattern, so it is neither `Send` nor
/// `Sync` and cannot live in a Bevy resource. It is a handful of `Option`s;
/// constructing it costs nothing.
pub(crate) fn editor_style() -> SnarlStyle {
    SnarlStyle {
        bg_pattern: Some(egui_snarl::ui::BackgroundPattern::NoPattern),
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
                // `capture` runs first so the canvas draws the state the last
                // step produced; `transport_requests` runs last so a play or a
                // step asked for in the panel lands before the next frame's
                // `step_fire`.
                (live::capture, toggle, live::transport_requests)
                    .chain()
                    .run_if(in_state(crate::AppState::Playing)),
            );
    }
}

/// `G` opens and closes the composer.
///
/// Deliberately not a left-click tool: the composer is a modal workbench, not
/// a fourth thing contending for the pointer over the map (see the
/// three-tools rule in CLAUDE.md).
///
/// It was `b`, which `crate::browser` also claimed — both systems fired on the
/// same press, so opening the composer also toggled the Entities panel behind
/// it. `g` for graph; the browser kept `b`.
///
/// Gated on the keyboard, not the pointer: the old pointer gate meant the
/// composer could not be closed from the keyboard while the cursor happened to
/// rest over it, which is where the cursor always is.
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut composer: ResMut<Composer>,
    mut panels: ResMut<crate::ui::PanelState>,
    focus: Res<crate::ui::UiFocus>,
) {
    if focus.typing() || crate::camera::modified(&keys) || crate::camera::shift(&keys) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        if composer.open && panels.bottom_tab == crate::ui::BottomTab::Behaviour {
            composer.open = false;
            panels.incident = crate::ui::PanelPlacement::Hidden;
        } else {
            composer.open = true;
            panels.focus_bottom(crate::ui::BottomTab::Behaviour);
        }
    }
}

/// Full-screen workspace body. The structure and wiring views share the draft;
/// debugging projects the applied snapshot through the same renderers.
pub fn panel_body(ui: &mut egui::Ui, c: &mut Composer, apply: &mut EventWriter<ApplyBehaviour>) {
    let grouping = ui.ctx().wants_keyboard_input() || ui.input(|i| i.pointer.any_down());
    c.checkpoint(grouping);
    // Text fields keep their own undo; workspace shortcuts act only outside them.
    if !ui.ctx().wants_keyboard_input() {
        let redo = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z));
        let undo = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z));
        if redo { c.redo(); } else if undo { c.undo(); }
    }
    ui.horizontal(|ui| {
        ui.selectable_value(&mut c.right, RightTab::Inspector, "Edit");
        ui.selectable_value(&mut c.right, RightTab::Live, "Debug");
        ui.selectable_value(&mut c.right, RightTab::Bench, "Test");
        ui.selectable_value(&mut c.right, RightTab::Subtypes, "Profiles");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Help & files").clicked() { c.right = RightTab::Help; }
            ui.weak(match (c.unsaved(), c.dirty) {
                (true, true) => "Unsaved · not applied",
                (true, false) => "Applied · unsaved",
                (false, true) => "Saved · not applied",
                (false, false) => "Saved · applied",
            });
        });
    });
    ui.separator();
    if c.right == RightTab::Live {
        live::debugger_panel(ui, c);
        return;
    }
    toolbar(ui, c, apply);
    ui.separator();
    egui::SidePanel::right("composer-inspector")
        .resizable(true).default_width(320.0).width_range(260.0..=460.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match c.right {
                RightTab::Inspector => inspector::panel(ui, c),
                RightTab::Subtypes => subtypes::panel(ui, c),
                RightTab::Bench => bench::panel(ui, c),
                RightTab::Help => help::panel(ui, c),
                RightTab::Live => unreachable!(),
            });
        });
    egui::TopBottomPanel::bottom("composer-issues")
        .resizable(false).show_inside(ui, |ui| issues(ui, c));
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            ui.strong(&c.graph_name);
            ui.selectable_value(&mut c.wiring, false, "Structure");
            ui.selectable_value(&mut c.wiring, true, "Wiring");
            if c.wiring && ui.small_button("Fit graph").clicked() { c.view_revision += 1; }
        });
        if c.wiring {
            viewer::canvas(ui, c);
        } else {
            let mut selected = c.selected.and_then(|id| c.snarl.get_node(id)).map(|n| n.id);
            structure::panel(ui, &c.graph, None, &mut selected);
            if let Some(id) = selected { c.selected = c.snarl_id_of(id); }
        }
    });
    c.checkpoint(grouping);
}

fn toolbar(ui: &mut egui::Ui, c: &mut Composer, apply: &mut EventWriter<ApplyBehaviour>) {
    ui.horizontal(|ui| {
        let current = c.domain();
        for d in Domain::ALL {
            if ui.selectable_label(d == current, d.label()).clicked() { c.switch_domain(d); }
        }
        ui.separator();
        let selected = c.subtype.as_ref().and_then(|id| c.lib.subtypes.get(id))
            .filter(|s| s.graph == c.graph_id).map(|s| s.name.as_str()).unwrap_or("Shared defaults");
        egui::ComboBox::from_id_source("edit-profile").selected_text(selected).show_ui(ui, |ui| {
            ui.selectable_value(&mut c.subtype, None, "Shared defaults");
            for profile in c.lib.subtypes.values().filter(|s| s.graph == c.graph_id) {
                ui.selectable_value(&mut c.subtype, Some(profile.id.clone()), &profile.name);
            }
        });
    });

    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(!c.history.undo.is_empty(), egui::Button::new("Undo")).clicked() { c.undo(); }
        if ui.add_enabled(!c.history.redo.is_empty(), egui::Button::new("Redo")).clicked() { c.redo(); }
        ui.separator();
        let current = c.graph_id.clone();
        let domain = c.domain();
        egui::ComboBox::from_id_source("composer-graph")
            .selected_text(c.graph_name.clone())
            .show_ui(ui, |ui| {
                let ids: Vec<(String, String)> = c
                    .lib
                    .graphs
                    .values()
                    .filter(|g| g.domain == domain)
                    .map(|g| (g.id.clone(), g.name.clone()))
                    .collect();
                for (id, name) in ids {
                    if ui.selectable_label(id == current, name).clicked() && id != current {
                        c.commit();
                        c.load_graph(&id);
                    }
                }
            });

        composition::rule_menu(ui, c);
        ui.menu_button("Add node", |ui| { palette::panel(ui, c); });
        ui.menu_button("Manage", |ui| {
        if ui
            .button("New")
            .on_hover_text("Start an empty behaviour")
            .clicked()
        {
            c.commit();
            c.new_graph(domain);
        }

        if ui.button("Duplicate").clicked() {
            c.commit();
            let id = c.lib.free_id(&format!("{}-copy", c.graph_id), true);
            let mut g = c.graph.clone();
            g.id = id.clone();
            g.name = format!("{} (copy)", g.name);
            c.lib.graphs.insert(id.clone(), g);
            c.load_graph(&id);
            c.dirty = true;
            c.set_status("Duplicated. Assign a profile to this copy before applying it.".into());
        }

        ui.separator();
        if ui.button("Save to disk").clicked() {
            c.save();
        }
        if ui
            .button("Discard edits and reload")
            .on_hover_text("Discard edits and re-read the files")
            .clicked()
        {
            c.reload();
        }

        ui.separator();
        ui.label("Name");
        if ui.text_edit_singleline(&mut c.graph_name).changed() { c.dirty = true; }
        });
        ui.separator();
        c.commit();
        let runtime_error = c.lib.validate_runtime().err().map(|e| format!("{e:#}"));
        let runnable = c.runnable() && runtime_error.is_none();
        let btn = egui::Button::new(if c.dirty {
            "Apply and restart *"
        } else {
            "Apply and restart"
        });
        let resp = ui.add_enabled(runnable, btn).on_disabled_hover_text(runtime_error.as_deref().unwrap_or("Fix the graph errors before applying")).on_hover_text(
            "Rebuild the agent model on this library and replay the incident from the start. \
             The fire, the weather and the ignition list are unchanged, so this is a like-for-like \
             comparison.",
        );
        if resp.clicked() {
            c.commit();
            match c.lib.validate_runtime() {
                Ok(()) => { apply.send(ApplyBehaviour); }
                Err(e) => c.set_error(format!("Cannot run yet: {e:#}")),
            }
        }
        if !runnable {
            ui.colored_label(
                egui::Color32::from_rgb(0xe0, 0x6c, 0x5f),
                if c.report.error_count() > 0 { format!("{} error(s)", c.report.error_count()) } else { "Library cannot be applied".into() },
            );
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
    egui::ScrollArea::vertical()
        .max_height(110.0)
        .show(ui, |ui| {
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
                            c.selected = c.snarl_id_of(n);
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
                    ui.add(egui::DragValue::new(n).speed(0.5).range(min..=max).suffix(unit))
                        .changed()
                } else {
                    ui.add(egui::Slider::new(n, min..=max).suffix(unit))
                        .changed()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> Composer {
        Composer::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/behaviours"))
    }

    #[test]
    fn undo_groups_gestures_and_tracks_the_applied_version() {
        let mut c = editor();
        let original = c.graph_name.clone();
        for name in ["E", "Ed", "Edited"] {
            c.graph_name = name.into();
            c.checkpoint(true);
        }
        c.checkpoint(false);
        assert_eq!(c.history.undo.len(), 1);
        c.mark_applied(c.lib.clone());
        assert!(!c.dirty);
        c.undo();
        assert_eq!(c.graph_name, original);
        assert!(c.dirty);
        c.redo();
        assert_eq!(c.graph_name, "Edited");
        assert!(!c.dirty);
        c.undo();
        c.graph_name = "Another edit".into();
        c.checkpoint(false);
        assert!(c.history.redo.is_empty());
    }

    #[test]
    fn deleting_nodes_cleans_profile_overrides_and_undo_restores_them() {
        let mut c = editor();
        let profile = c.subtype.clone().unwrap();
        let id = viewer::free_id(&c.snarl);
        let sid = c.snarl.insert_node(egui::Pos2::ZERO, EditorNode::new(id, "param.number"));
        let key = BehaviorGraph::override_key(id, "value");
        c.lib.subtypes.get_mut(&profile).unwrap().overrides.insert(key.clone(), ParamValue::Number(42.0));
        c.checkpoint(false);
        c.snarl.remove_node(sid);
        c.checkpoint(false);
        assert!(!c.lib.subtypes[&profile].overrides.contains_key(&key));
        c.undo();
        assert!(c.graph.node(id).is_some());
        assert_eq!(c.lib.subtypes[&profile].overrides[&key], ParamValue::Number(42.0));
        c.redo();
        assert!(c.graph.node(id).is_none());
        let reused = viewer::free_id(&c.snarl);
        assert_eq!(reused, id);
        c.snarl.insert_node(egui::Pos2::ZERO, EditorNode::new(reused, "param.number"));
        c.commit();
        assert!(!c.lib.subtypes[&profile].overrides.contains_key(&key));
    }

    #[test]
    fn selecting_a_profile_opens_its_graph_and_browsing_is_not_an_edit() {
        let mut c = editor();
        let profile = c.lib.subtypes.values().find(|s| s.graph != c.graph_id).unwrap().clone();
        c.select_profile(&profile.id);
        c.checkpoint(false);
        assert_eq!(c.graph_id, profile.graph);
        assert_eq!(c.subtype.as_deref(), Some(profile.id.as_str()));
        assert!(c.history.undo.is_empty());
        assert!(!c.dirty);
        c.load_graph(behavior::defaults::DEFAULT_GRAPH_ID);
        assert!(c.subtype.is_none());
    }

    #[test]
    fn profile_deletion_stays_in_draft_until_save_and_is_undoable() {
        let root = std::env::temp_dir().join(format!("behavior-editor-test-{}", uuid::Uuid::new_v4()));
        let library = editor().lib;
        library.save_dir(&root).unwrap();
        let mut c = Composer::new(root.clone());
        let id = c.subtype.clone().unwrap();
        c.delete_profile(&id);
        c.checkpoint(false);
        assert!(Library::load_dir_reported(&root).unwrap().library.subtypes.contains_key(&id));
        assert!(c.unsaved());
        c.save();
        assert!(!c.status_is_error, "{}", c.status);
        assert!(!Library::load_dir_reported(&root).unwrap().library.subtypes.contains_key(&id));
        assert!(!c.unsaved());
        c.undo();
        assert!(c.lib.subtypes.contains_key(&id));
        assert!(c.unsaved());
        assert!(!c.dirty);
        c.save();
        assert!(Library::load_dir_reported(&root).unwrap().library.subtypes.contains_key(&id));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn debug_views_render_applied_graph_without_changing_the_draft() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/behaviours");
        let mut c = Composer::new(root);
        let applied = c.graph.clone();
        let compiled = behavior::CompiledGraph::compile(&applied, &Default::default()).unwrap();
        let (decision, trace) = compiled.eval_traced(&behavior::HouseholdObs::default().into());
        c.live.frame = Some(live::Frame {
            graph_id: applied.id.clone(), graph: applied.clone(),
            subtype_id: "test".into(), subtype_name: "Test profile".into(), agent: "Test agent".into(),
            decision, winner: trace.winner(), active: trace.active(), withheld: trace.withheld(),
            seen: Default::default(),
            values: trace.nodes.iter().map(|n| (n.node, n.outputs.clone())).collect(),
            inputs: Default::default(), trace, stale: true,
        });
        c.graph_name = "Draft only".into();
        c.snarl = Snarl::new();
        c.commit();
        let draft = c.to_graph();
        for wiring in [false, true] {
            c.wiring = wiring;
            let ctx = egui::Context::default();
            let output = ctx.run(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
                ..Default::default()
            }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| live::debugger_panel(ui, &mut c));
            });
            assert!(!output.shapes.is_empty());
            assert_eq!(c.live.frame.as_ref().unwrap().graph, applied);
            assert_eq!(c.to_graph(), draft);
            assert!(c.dirty);
        }
    }

    #[test]
    fn browsing_does_not_mark_the_library_as_changed() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/behaviours");
        let mut c = Composer::new(root);
        c.dirty = false;
        c.commit();
        assert!(!c.dirty);
        c.switch_domain(Domain::Person);
        assert!(!c.dirty);
        c.graph_name.push_str(" edited");
        c.commit();
        assert!(c.dirty);
        assert_eq!(c.lib.graphs[&c.graph_id].name, c.graph_name);
    }
}
