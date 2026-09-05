//! A readable entry point to the same graph and parameter editor as the canvas.
use behavior::Domain;
use bevy_egui::egui;

use super::{Composer, RightTab};

pub fn panel(ui: &mut egui::Ui, c: &mut Composer) {
    egui::ScrollArea::vertical().id_source("guided-behaviour").show(ui, |ui| {
        ui.heading(&c.graph_name);
        if !c.graph_description.is_empty() {
            ui.label(&c.graph_description);
        }
        ui.add_space(8.0);
        ui.strong("Who uses this behaviour?");
        let profiles: Vec<_> = c.lib.subtypes.values()
            .filter(|s| s.graph == c.graph_id)
            .map(|s| (s.id.clone(), s.name.clone(), match c.domain() {
                Domain::SuppressionUnit => s.enabled,
                _ => s.share > 0.0,
            })).collect();
        let assigned = profiles.iter().any(|(_, _, active)| *active);
        if !assigned {
            ui.colored_label(egui::Color32::YELLOW,
                "No active profile uses this behaviour. It will not affect agents until you assign one in Profiles.");
        }
        ui.small("Choose a profile to change only its settings, or choose shared settings to change the defaults.");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut c.subtype, None, "Shared settings");
            for (id, name, active) in profiles {
                ui.selectable_value(&mut c.subtype, Some(id),
                    format!("{name}{}", if active { "" } else { " (inactive)" }));
            }
            if ui.button("Manage profiles").clicked() { c.right = RightTab::Subtypes; }
            if ui.button("Try a situation").clicked() { c.right = RightTab::Bench; }
        });
        ui.separator();
        ui.strong("Behaviour rules");
        ui.label("Choose a rule to read what it does and adjust its settings on the right. Use Advanced wiring to add rules or change their connections.");
        let nodes: Vec<_> = c.snarl.node_ids().filter_map(|(sid, node)| {
            node.spec().map(|spec| (sid, spec.name, spec.doc, !spec.params.is_empty()))
        }).collect();
        for (sid, name, doc, configurable) in nodes {
            ui.push_id(sid, |ui| {
                ui.group(|ui| {
                    if ui.selectable_label(c.selected == Some(sid), name).clicked() {
                        c.selected = Some(sid);
                        c.right = RightTab::Inspector;
                    }
                    ui.small(doc.lines().next().unwrap_or(doc));
                    if configurable && ui.small_button("Adjust settings").clicked() {
                        c.selected = Some(sid);
                        c.right = RightTab::Inspector;
                    }
                });
            });
        }
    });
}
