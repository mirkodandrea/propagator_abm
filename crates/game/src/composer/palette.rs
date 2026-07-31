//! The palette: every registered node, searchable.
//!
//! Grouped by category and filtered by a single query box that matches the id,
//! the name, the documentation and a set of keywords the node declares — so
//! "smoke" finds the fire-distance observation and "threshold" finds the two
//! comparisons, which is what a scientist actually types. The list is built
//! from the registry every frame, which means a node added anywhere in the
//! codebase is here with no further work.

use bevy_egui::egui;

use behavior::{registry, Category};

use super::{viewer::EditorNode, Composer};

pub fn panel(ui: &mut egui::Ui, c: &mut Composer) {
    ui.heading("Palette");
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(
            egui::TextEdit::singleline(&mut c.palette_query)
                .hint_text("threshold, smoke, evacuate…")
                .desired_width(f32::INFINITY),
        );
    });
    let query = c.palette_query.to_lowercase();

    let total = registry().len();
    let matching = registry().all().filter(|s| s.matches(&query)).count();
    ui.small(if query.is_empty() {
        format!("{total} nodes")
    } else {
        format!("{matching} of {total}")
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        for cat in Category::ALL {
            let hits: Vec<_> =
                registry().in_category(cat).filter(|s| s.matches(&query)).collect();
            if hits.is_empty() {
                continue;
            }
            // Searching opens every group that has a hit; browsing keeps them
            // collapsed, because the observation list alone is two dozen long.
            egui::CollapsingHeader::new(format!("{} ({})", cat.label(), hits.len()))
                .default_open(!query.is_empty() || cat != Category::Observation)
                .show(ui, |ui| {
                    for spec in hits {
                        let r = ui.add(
                            egui::Button::new(spec.name)
                                .fill(egui::Color32::TRANSPARENT)
                                .min_size(egui::vec2(ui.available_width(), 0.0)),
                        );
                        let r = r.on_hover_ui(|ui| {
                            ui.set_max_width(300.0);
                            ui.strong(spec.name);
                            ui.label(spec.doc);
                            ports(ui, spec);
                            ui.small(spec.id);
                        });
                        if r.clicked() {
                            // Dropped in a cascade near the top-left of the
                            // view rather than at the pointer: the pointer is
                            // over the palette, not over the canvas.
                            let at = c.drop_at;
                            c.drop_at += egui::vec2(24.0, 24.0);
                            let id = c.snarl.insert_node(at, EditorNode::new(spec.id));
                            c.selected = Some(id);
                            c.right = super::RightTab::Inspector;
                            c.dirty = true;
                            c.set_status(format!("added {}", spec.name));
                        }
                    }
                });
        }
    });
}

fn ports(ui: &mut egui::Ui, spec: &behavior::NodeSpec) {
    if !spec.inputs.is_empty() {
        ui.small(format!(
            "in: {}",
            spec.inputs
                .iter()
                .map(|p| format!("{} ({})", p.name, p.ty.label()))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !spec.outputs.is_empty() {
        ui.small(format!(
            "out: {}",
            spec.outputs
                .iter()
                .map(|p| format!("{} ({})", p.name, p.ty.label()))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}
