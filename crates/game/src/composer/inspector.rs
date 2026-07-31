//! Configuring one node.
//!
//! The one thing worth calling out: when a subtype is selected, this panel
//! edits **the subtype's override**, not the graph. That distinction is the
//! whole reason overrides exist, so the panel says which of the two it is
//! writing to and offers a one-click way back to the graph's own value. A
//! scientist who edits a threshold while a profile is selected and finds they
//! have changed the number for every profile has been badly served.

use bevy_egui::egui;

use behavior::BehaviorGraph;

use super::Composer;

pub fn panel(ui: &mut egui::Ui, c: &mut Composer) {
    let Some(node_id) = c.selected else {
        ui.label("Select a node on the canvas, or add one from the palette.");
        ui.separator();
        ui.small(
            "Right-click the canvas to add a node where you clicked. Drag a wire into empty \
             space to add a node that can accept it.",
        );
        graph_notes(ui, c);
        return;
    };
    let Some(node) = c.snarl.get_node(node_id) else {
        c.selected = None;
        return;
    };
    let Some(spec) = node.spec() else {
        ui.colored_label(
            egui::Color32::from_rgb(0xe0, 0x6c, 0x5f),
            format!("This build has no node type \"{}\".", node.type_id),
        );
        ui.small("It was saved by a build that had it. Delete it, or open the file in a build that does.");
        return;
    };

    ui.horizontal(|ui| {
        ui.heading(spec.name);
        ui.small(format!("#{}", node_id.0));
    });
    ui.label(spec.doc);
    ui.small(spec.id);
    ui.separator();

    // Which of the two things the sliders below are writing to.
    let editing_subtype = c
        .subtype
        .clone()
        .filter(|id| c.lib.subtypes.get(id).map(|s| s.graph == c.graph_id).unwrap_or(false));

    if spec.params.is_empty() {
        ui.small("This node has nothing to configure.");
    } else {
        match &editing_subtype {
            Some(id) => {
                let name = c.lib.subtypes.get(id).map(|s| s.name.clone()).unwrap_or_default();
                ui.colored_label(
                    egui::Color32::from_rgb(0x9a, 0x8c, 0xe0),
                    format!("Editing overrides for \"{name}\""),
                );
                ui.small("The graph's own values are unchanged. Switch the subtype to \"none\" to edit those.");
            }
            None => {
                ui.small("Editing the graph's own values, which every subtype inherits.");
            }
        }
        ui.add_space(4.0);
    }

    for param in spec.params {
        let key = BehaviorGraph::override_key(node_id.0 as behavior::NodeId, param.name);
        let graph_value = c
            .snarl
            .get_node(node_id)
            .and_then(|n| n.params.get(param.name).cloned())
            .unwrap_or_else(|| param.default_value());

        match &editing_subtype {
            Some(sub_id) => {
                let overridden =
                    c.lib.subtypes.get(sub_id).map(|s| s.overrides.contains_key(&key)).unwrap_or(false);
                let mut value = if overridden {
                    c.lib.subtypes[sub_id].overrides[&key].clone()
                } else {
                    graph_value.clone()
                };
                let changed = super::param_widget(ui, param, &mut value);
                ui.horizontal(|ui| {
                    if overridden {
                        ui.colored_label(egui::Color32::from_rgb(0x9a, 0x8c, 0xe0), "overridden");
                        if ui.small_button("reset").on_hover_text("Use the graph's value").clicked() {
                            if let Some(s) = c.lib.subtypes.get_mut(sub_id) {
                                s.overrides.remove(&key);
                            }
                            c.dirty = true;
                        }
                    } else {
                        ui.small(format!("graph: {}", graph_value.display()));
                    }
                });
                if changed {
                    if let Some(s) = c.lib.subtypes.get_mut(sub_id) {
                        s.overrides.insert(key, value);
                    }
                    c.dirty = true;
                }
            }
            None => {
                let mut value = graph_value;
                if super::param_widget(ui, param, &mut value) {
                    if let Some(n) = c.snarl.get_node_mut(node_id) {
                        n.params.insert(param.name.to_string(), value);
                    }
                    c.dirty = true;
                }
                // Which profiles have moved this number off the graph's value.
                let holders: Vec<String> = c
                    .lib
                    .subtypes
                    .values()
                    .filter(|s| s.graph == c.graph_id && s.overrides.contains_key(&key))
                    .map(|s| s.name.clone())
                    .collect();
                if !holders.is_empty() {
                    ui.small(format!("overridden by: {}", holders.join(", ")));
                }
            }
        }
    }

    ui.separator();
    ui.label("Note");
    let mut comment = c.snarl.get_node(node_id).map(|n| n.comment.clone()).unwrap_or_default();
    if ui
        .add(egui::TextEdit::multiline(&mut comment).desired_rows(2).hint_text("Why this is here"))
        .changed()
    {
        if let Some(n) = c.snarl.get_node_mut(node_id) {
            n.comment = comment;
        }
        c.dirty = true;
    }

    let issues: Vec<String> = c
        .report
        .for_node(node_id.0 as behavior::NodeId)
        .map(|i| format!("{:?}: {}", i.severity, i.message))
        .collect();
    if !issues.is_empty() {
        ui.separator();
        for i in issues {
            ui.colored_label(egui::Color32::from_rgb(0xd8, 0xa6, 0x4b), i);
        }
    }

    ui.separator();
    if ui.button("Delete node").clicked() {
        c.snarl.remove_node(node_id);
        c.selected = None;
        c.dirty = true;
    }
}

fn graph_notes(ui: &mut egui::Ui, c: &mut Composer) {
    ui.separator();
    ui.heading("This behaviour");
    ui.small(format!("id: {}", c.graph_id));
    ui.label("Description");
    if ui
        .add(egui::TextEdit::multiline(&mut c.graph_description).desired_rows(4))
        .changed()
    {
        c.dirty = true;
    }
    ui.small(format!(
        "{} nodes, {} wires",
        c.graph.nodes.len(),
        c.graph.wires.len()
    ));

    // Every parameter any profile has moved, in one list — the answer to "what
    // do the subtypes actually change", which is otherwise a hunt across the
    // canvas.
    let graph_id = c.graph_id.clone();
    let mut rows: Vec<(String, String, String)> = Vec::new();
    for s in c.lib.subtypes.values().filter(|s| s.graph == graph_id) {
        for (k, v) in &s.overrides {
            rows.push((
                s.name.clone(),
                behavior::subtype::describe_key(k, Some(&c.graph)),
                v.display(),
            ));
        }
    }
    if !rows.is_empty() {
        ui.separator();
        ui.label(format!("{} override(s) across all profiles", rows.len()));
        egui::ScrollArea::vertical().max_height(200.0).id_source("all-overrides").show(ui, |ui| {
            for (who, what, value) in rows {
                ui.small(format!("{who} · {what} = {value}"));
            }
        });
    }
}
