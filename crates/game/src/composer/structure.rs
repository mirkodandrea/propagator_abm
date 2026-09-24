//! A dependency tree projected from actual wires. No separately authored rules.
use std::collections::BTreeSet;

use behavior::{BehaviorGraph, NodeId};
use bevy_egui::egui;

use super::live::Frame;

/// Include disconnected nodes and cycles as well as output-rooted components.
fn roots(graph: &BehaviorGraph) -> Vec<NodeId> {
    let mut roots: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| !graph.wires.iter().any(|w| w.from_node == n.id))
        .map(|n| n.id)
        .collect();
    let mut reached = BTreeSet::new();
    fn visit(graph: &BehaviorGraph, id: NodeId, reached: &mut BTreeSet<NodeId>) {
        if !reached.insert(id) {
            return;
        }
        for wire in graph.wires.iter().filter(|w| w.to_node == id) {
            visit(graph, wire.from_node, reached);
        }
    }
    for id in &roots {
        visit(graph, *id, &mut reached);
    }
    for node in &graph.nodes {
        if !reached.contains(&node.id) {
            roots.push(node.id);
            visit(graph, node.id, &mut reached);
        }
    }
    roots
}

pub fn panel(
    ui: &mut egui::Ui,
    graph: &BehaviorGraph,
    frame: Option<&Frame>,
    selected: &mut Option<NodeId>,
) {
    ui.weak("Expand a branch to inspect its inputs · click a node for details");
    ui.add_space(12.0);
    egui::ScrollArea::both()
        .id_source(("behavior-structure", &graph.id, frame.is_some()))
        .show(ui, |ui| {
            let roots = roots(graph);
            let columns = ((ui.available_width() / 340.0) as usize).clamp(1, 3);
            for chunk in roots.chunks(columns) {
                ui.columns(columns, |columns| {
                    for (column, id) in columns.iter_mut().zip(chunk) {
                        column.push_id((&graph.id, id, frame.is_some()), |ui| {
                            ui.group(|ui| {
                                ui.set_min_width((ui.available_width() - 12.0).max(180.0));
                                row(ui, graph, *id, frame, selected, &mut BTreeSet::new(), 0);
                            });
                            ui.add_space(8.0);
                        });
                    }
                });
            }
            if graph.nodes.is_empty() {
                ui.weak("Add a node to begin.");
            }
        });
}

fn row(
    ui: &mut egui::Ui,
    graph: &BehaviorGraph,
    id: NodeId,
    frame: Option<&Frame>,
    selected: &mut Option<NodeId>,
    path: &mut BTreeSet<NodeId>,
    depth: usize,
) {
    let Some(node) = graph.node(id) else {
        ui.colored_label(egui::Color32::LIGHT_RED, format!("Missing node #{id}"));
        return;
    };
    if !path.insert(id) {
        ui.colored_label(
            egui::Color32::LIGHT_RED,
            "Cycle — fix this connection in Wiring",
        );
        return;
    }
    let mut incoming: Vec<_> = graph.wires.iter().filter(|w| w.to_node == id).collect();
    incoming.sort_by_key(|w| (w.to_port, w.from_node, w.from_port));
    let state_id = ui.make_persistent_id("expanded");
    let mut expanded = ui.data_mut(|d| *d.get_temp_mut_or_insert_with(state_id, || depth < 1));
    ui.horizontal(|ui| {
        if incoming.is_empty() {
            ui.add_space(24.0);
        } else if ui.small_button(if expanded { "−" } else { "+" }).clicked() {
            expanded = !expanded;
            ui.data_mut(|d| d.insert_temp(state_id, expanded));
        }
        let name = node.spec().map(|s| s.name).unwrap_or(&node.type_id);
        let mut text = egui::RichText::new(name).size(16.0);
        if let Some(frame) = frame {
            text = text.color(frame.role(id).colour());
        }
        if ui.selectable_label(*selected == Some(id), text).clicked() {
            *selected = Some(id);
        }
        if let Some(frame) = frame {
            if frame.winner == Some(id) {
                ui.strong("▶ winner");
            }
            if let Some(values) = frame.values.get(&id) {
                ui.weak(
                    values
                        .iter()
                        .map(behavior::Value::display)
                        .collect::<Vec<_>>()
                        .join(" · "),
                );
            }
        }
    });
    if expanded && depth < 64 {
        ui.indent(id, |ui| {
            for (index, wire) in incoming.iter().enumerate() {
                ui.push_id(index, |ui| {
                    let source = graph
                        .node(wire.from_node)
                        .and_then(|n| n.spec())
                        .and_then(|s| s.outputs.get(wire.from_port as usize))
                        .map(|p| p.name)
                        .unwrap_or("?");
                    let input = node
                        .spec()
                        .and_then(|s| s.inputs.get(wire.to_port as usize))
                        .map(|p| p.name)
                        .unwrap_or("?");
                    ui.add_space(6.0);
                    ui.scope(|ui| row(ui, graph, wire.from_node, frame, selected, path, depth + 1))
                        .response
                        .on_hover_text(format!("{source} feeds {input}"));
                });
            }
        });
    }
    path.remove(&id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use behavior::Wire;

    #[test]
    fn structure_includes_outputs_disconnected_nodes_and_cycles() {
        let mut graph = BehaviorGraph::new("test", "test");
        let input = graph.add("param.bool", [0.0, 0.0]).unwrap();
        let output = graph.add("logic.not", [0.0, 0.0]).unwrap();
        let orphan = graph.add("param.bool", [0.0, 0.0]).unwrap();
        let cycle = graph.add("logic.not", [0.0, 0.0]).unwrap();
        graph.wires.push(Wire {
            from_node: input,
            from_port: 0,
            to_node: output,
            to_port: 0,
        });
        graph.wires.push(Wire {
            from_node: cycle,
            from_port: 0,
            to_node: cycle,
            to_port: 0,
        });
        assert_eq!(roots(&graph), vec![output, orphan, cycle]);
        graph.wires.clear();
        assert_eq!(roots(&graph), vec![input, output, orphan, cycle]);
    }
}
