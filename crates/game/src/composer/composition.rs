//! Building and reusing connected rules from the compact editor.
use std::collections::{BTreeMap, BTreeSet};

use behavior::{BehaviorGraph, Category, NodeId, ParamValue, Wire};
use bevy_egui::egui;

use super::{viewer, Composer, RightTab};

#[derive(Clone, Copy)]
pub(super) enum Rule {
    Single,
    All,
    Any,
    Quorum,
    Threshold,
}

pub fn rule_menu(ui: &mut egui::Ui, c: &mut Composer) {
    ui.menu_button("Add rule", |ui| {
        for action in behavior::registry().in_category_and_domain(Category::Action, c.domain()) {
            ui.menu_button(action.name, |ui| {
                for (label, rule) in [
                    ("When a condition holds", Rule::Single),
                    ("When all conditions hold", Rule::All),
                    ("When any condition holds", Rule::Any),
                    ("When at least N hold", Rule::Quorum),
                    ("When a signal exceeds a threshold", Rule::Threshold),
                ] {
                    if ui.button(label).clicked() {
                        c.add_rule(action.id, rule);
                        ui.close_menu();
                    }
                }
            });
        }
    });
}

/// Test cycles before changing anything, including before replacing a wire.
pub(super) fn connection_allowed(graph: &BehaviorGraph, wire: Wire) -> bool {
    let mut candidate = graph.clone();
    if !candidate.connect(wire) { return false; }
    let mut seen = BTreeSet::new();
    let mut stack = vec![wire.to_node];
    while let Some(id) = stack.pop() {
        if id == wire.from_node { return false; }
        if seen.insert(id) {
            stack.extend(candidate.wires.iter().filter(|w| w.from_node == id).map(|w| w.to_node));
        }
    }
    true
}

impl Composer {
    fn install_composition(&mut self, graph: BehaviorGraph, selected: Option<NodeId>) {
        self.snarl = viewer::snarl_from_graph(&graph);
        self.selected = selected.and_then(|id| self.snarl_id_of(id));
        self.sync();
        self.dirty = true;
    }

    pub(super) fn add_rule(&mut self, action: &str, rule: Rule) {
        let Some(spec) = behavior::registry().get(action)
            .filter(|s| s.category == Category::Action && s.allowed_in(self.domain())) else { return };
        let mut graph = self.to_graph();
        let sinks: Vec<_> = graph.nodes.iter().filter(|n| n.type_id == self.domain().decision_output()).map(|n| n.id).collect();
        if sinks.len() > 1 { self.set_error("Keep one decision output before adding a rule.".into()); return; }
        let y = graph.nodes.iter().map(|n| n.pos[1]).fold(0.0, f32::max) + 180.0;
        let sink = sinks.first().copied().unwrap_or_else(|| graph.add(self.domain().decision_output(), [900.0, y]).unwrap());
        let seed = graph.add(if matches!(rule, Rule::Threshold) { "param.number" } else { "param.bool" }, [0.0, y]).unwrap();
        // Starters are deliberately inactive until configured.
        graph.node_mut(seed).unwrap().params.insert("value".into(), if matches!(rule, Rule::Threshold) { ParamValue::Number(0.0) } else { ParamValue::Bool(false) });
        let condition = match rule {
            Rule::Single => seed,
            _ => {
                let kind = match rule { Rule::All => "logic.all", Rule::Any => "logic.any", Rule::Quorum => "logic.at_least", _ => "cmp.above" };
                let gate = graph.add(kind, [280.0, y]).unwrap();
                graph.connect(Wire { from_node: seed, from_port: 0, to_node: gate, to_port: 0 });
                gate
            }
        };
        let action = graph.add(spec.id, [560.0, y]).unwrap();
        graph.connect(Wire { from_node: condition, from_port: 0, to_node: action, to_port: 0 });
        graph.connect(Wire { from_node: action, from_port: 0, to_node: sink, to_port: 0 });
        self.install_composition(graph, Some(action));
        self.right = RightTab::Inspector;
        self.set_status("Rule connected. Select its condition to configure it; it starts inactive.".into());
    }

    pub(super) fn connect_input(&mut self, wire: Wire) -> bool {
        let mut graph = self.to_graph();
        if !connection_allowed(&graph, wire) { return false; }
        graph.connect(wire);
        let selected = self.selected.and_then(|sid| self.snarl.get_node(sid)).map(|n| n.id);
        self.install_composition(graph, selected);
        true
    }

    pub(super) fn disconnect_input(&mut self, wire: Wire) {
        let mut graph = self.to_graph();
        graph.disconnect(wire);
        let selected = self.selected.and_then(|sid| self.snarl.get_node(sid)).map(|n| n.id);
        self.install_composition(graph, selected);
    }

    /// Copy the upstream dependency closure once, even when dependencies are shared.
    /// Existing multi-input consumers receive the new branch; single inputs stay intact.
    pub(super) fn duplicate_branch(&mut self, root: NodeId) {
        let mut graph = self.to_graph();
        if graph.node(root).and_then(|n| n.spec()).is_none_or(|s| s.category == Category::Output) { return; }
        let mut members = BTreeSet::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if members.insert(id) {
                stack.extend(graph.wires.iter().filter(|w| w.to_node == id).map(|w| w.from_node));
            }
        }
        if graph.nodes.iter().any(|n| members.contains(&n.id) && n.spec().is_some_and(|s| s.category == Category::Output)) {
            self.set_error("This branch depends on an output. Copy its inputs individually.".into());
            return;
        }
        let original = graph.clone();
        let mut remap = BTreeMap::new();
        for node in original.nodes.iter().filter(|n| members.contains(&n.id)) {
            let mut copy = node.clone();
            copy.id = graph.next_id();
            copy.pos[0] += 50.0;
            copy.pos[1] += 160.0;
            remap.insert(node.id, copy.id);
            graph.nodes.push(copy);
        }
        for wire in &original.wires {
            if let (Some(&from), Some(&to)) = (remap.get(&wire.from_node), remap.get(&wire.to_node)) {
                graph.wires.push(Wire { from_node: from, to_node: to, ..*wire });
            } else if wire.from_node == root && original.node(wire.to_node).and_then(|n| n.spec())
                .and_then(|s| s.inputs.get(wire.to_port as usize)).is_some_and(|p| p.multi) {
                graph.wires.push(Wire { from_node: remap[&root], ..*wire });
            }
        }
        for profile in self.lib.subtypes.values_mut().filter(|s| s.graph == graph.id) {
            let copied: Vec<_> = profile.overrides.iter().filter_map(|(key, value)| {
                let (id, param) = behavior::subtype::split_key(key)?;
                Some((BehaviorGraph::override_key(*remap.get(&id)?, param), value.clone()))
            }).collect();
            profile.overrides.extend(copied);
        }
        self.install_composition(graph, Some(remap[&root]));
        self.set_status("Branch copied with independent nodes and profile settings.".into());
    }
}

/// Every input can be wired here, so composition does not require canvas gestures.
pub fn inputs(ui: &mut egui::Ui, c: &mut Composer, node: NodeId) {
    let graph = c.to_graph();
    let Some(spec) = graph.node(node).and_then(|n| n.spec()) else { return };
    if spec.inputs.is_empty() { return; }
    ui.separator();
    ui.strong("Inputs").on_hover_text("Connections change the shared behavior for all profiles.");
    for (port, input) in spec.inputs.iter().enumerate() {
        ui.push_id((node, port), |ui| {
            ui.label(input.name).on_hover_text(input.doc);
            for wire in graph.wires.iter().filter(|w| w.to_node == node && w.to_port == port as u16) {
                ui.horizontal(|ui| {
                    let name = graph.node(wire.from_node).and_then(|n| n.spec()).map(|s| s.name).unwrap_or("Unknown node");
                    let output = graph.node(wire.from_node).and_then(|n| n.spec()).and_then(|s| s.outputs.get(wire.from_port as usize)).map(|p| p.name).unwrap_or("?");
                    if ui.small_button(format!("{name} · {output}")).clicked() { c.selected = c.snarl_id_of(wire.from_node); }
                    if ui.small_button("×").on_hover_text("Disconnect").clicked() { c.disconnect_input(*wire); }
                });
            }
            egui::ComboBox::from_id_source("source").selected_text(if input.multi { "Add input…" } else { "Connect / replace…" }).show_ui(ui, |ui| {
                for source in &graph.nodes {
                    let Some(source_spec) = source.spec() else { continue };
                    for (output, p) in source_spec.outputs.iter().enumerate().filter(|(_, p)| p.ty == input.ty) {
                        let wire = Wire { from_node: source.id, from_port: output as u16, to_node: node, to_port: port as u16 };
                        if !connection_allowed(&graph, wire) { continue; }
                        if ui.button(format!("{} · {} (#{} )", source_spec.name, p.name, source.id)).clicked() {
                            c.connect_input(wire);
                            ui.close_menu();
                        }
                    }
                }
            });
        });
    }
}
