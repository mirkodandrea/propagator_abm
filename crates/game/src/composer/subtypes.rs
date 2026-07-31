//! Agent subtypes: create, duplicate, edit, save, load, compare.
//!
//! A subtype is a name, a graph, a flat set of parameter overrides, some
//! starting traits and two capability flags. There is no inheritance, and this
//! panel is where that shows: everything a profile does to an agent is on one
//! screen, and there is no parent to go and read.
//!
//! The share column is the other thing worth understanding. Shares are
//! *relative* — 3 and 1 means three to one — and are normalised when the
//! runtime is built, so a scientist can type whole numbers without having to
//! make them add up. A subtype with a share of zero still exists and is still
//! editable; it is simply not in the population.

use bevy_egui::egui;

use behavior::subtype::{Capability, TraitKey};
use behavior::AgentSubtype;

use super::Composer;

pub fn panel(ui: &mut egui::Ui, c: &mut Composer) {
    roster(ui, c);
    ui.separator();

    let Some(id) = c.subtype.clone() else {
        ui.label("No profile selected. The inspector is editing the graph's own values.");
        return;
    };
    let Some(mut s) = c.lib.subtypes.get(&id).cloned() else {
        c.subtype = None;
        return;
    };

    let mut changed = false;
    ui.heading(&s.name);
    ui.small(format!("id: {}", s.id));

    ui.horizontal(|ui| {
        ui.label("Name");
        changed |= ui.text_edit_singleline(&mut s.name).changed();
    });
    ui.label("Description");
    changed |= ui
        .add(egui::TextEdit::multiline(&mut s.description).desired_rows(3))
        .changed();
    ui.horizontal(|ui| {
        ui.label("Author");
        changed |= ui.text_edit_singleline(&mut s.author).changed();
    });

    ui.horizontal(|ui| {
        ui.label("Behaviour");
        let ids: Vec<(String, String)> =
            c.lib.graphs.values().map(|g| (g.id.clone(), g.name.clone())).collect();
        let current = ids
            .iter()
            .find(|(gid, _)| *gid == s.graph)
            .map(|(_, n)| n.clone())
            .unwrap_or_else(|| format!("missing: {}", s.graph));
        egui::ComboBox::from_id_source("subtype-graph")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (gid, name) in ids {
                    if ui.selectable_label(gid == s.graph, name).clicked() && gid != s.graph {
                        // Overrides are keyed on node ids in the old graph, so
                        // they mean nothing in the new one. Dropping them is
                        // the honest move; silently keeping keys that resolve
                        // to unrelated nodes is not.
                        s.graph = gid;
                        s.overrides.clear();
                        changed = true;
                    }
                }
            });
    });

    ui.horizontal(|ui| {
        ui.label("Share").on_hover_text(
            "Relative weight in the population. These are normalised, so 3 and 1 means three \
             to one. Zero keeps the profile but takes it out of play.",
        );
        changed |= ui.add(egui::DragValue::new(&mut s.share).speed(0.01).range(0.0..=100.0)).changed();
    });

    ui.horizontal_wrapped(|ui| {
        ui.label("Tags");
        let mut tags = s.tags.join(", ");
        if ui.text_edit_singleline(&mut tags).changed() {
            s.tags = tags
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            changed = true;
        }
    });

    // --- traits ------------------------------------------------------------
    ui.separator();
    ui.label("Starting traits").on_hover_text(
        "Replace what the population bake drew for this profile's households. Anything left \
         off keeps the baked value, which is what keeps a profile a short file.",
    );
    for key in TraitKey::ALL {
        ui.horizontal(|ui| {
            let mut on = s.traits.contains_key(&key);
            if ui.checkbox(&mut on, "").changed() {
                if on {
                    let (lo, hi) = key.range();
                    s.traits.insert(key, (lo + hi) * 0.5);
                } else {
                    s.traits.remove(&key);
                }
                changed = true;
            }
            ui.label(key.label());
            if let Some(v) = s.traits.get_mut(&key) {
                let (lo, hi) = key.range();
                changed |= ui.add(egui::Slider::new(v, lo..=hi)).changed();
            } else {
                ui.small("as baked");
            }
        });
    }

    // --- capabilities ------------------------------------------------------
    ui.separator();
    ui.label("Capabilities");
    for key in Capability::ALL {
        ui.horizontal(|ui| {
            let current = s.capabilities.get(&key).copied();
            let mut state = match current {
                None => 0,
                Some(true) => 1,
                Some(false) => 2,
            };
            egui::ComboBox::from_id_source(key.label())
                .selected_text(["as baked", "yes", "no"][state])
                .show_ui(ui, |ui| {
                    for (i, label) in ["as baked", "yes", "no"].iter().enumerate() {
                        if ui.selectable_label(state == i, *label).clicked() {
                            state = i;
                        }
                    }
                });
            let want = match state {
                0 => None,
                1 => Some(true),
                _ => Some(false),
            };
            if want != current {
                match want {
                    None => {
                        s.capabilities.remove(&key);
                    }
                    Some(v) => {
                        s.capabilities.insert(key, v);
                    }
                }
                changed = true;
            }
            ui.label(key.label());
        });
    }

    // --- overrides ---------------------------------------------------------
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!("{} parameter override(s)", s.overrides.len()));
        if !s.overrides.is_empty() && ui.small_button("clear all").clicked() {
            s.overrides.clear();
            changed = true;
        }
    });
    let mut drop = None;
    for (key, value) in &s.overrides {
        ui.horizontal(|ui| {
            if ui.small_button("✖").on_hover_text("Back to the graph's value").clicked() {
                drop = Some(key.clone());
            }
            ui.small(format!(
                "{} = {}",
                behavior::subtype::describe_key(key, Some(&c.graph)),
                value.display()
            ));
        });
    }
    if let Some(k) = drop {
        s.overrides.remove(&k);
        changed = true;
    }
    if s.graph == c.graph_id {
        ui.small("Select a node on the canvas to add or change an override.");
        if ui
            .small_button("Prune dead overrides")
            .on_hover_text("Drop overrides whose node or parameter no longer exists")
            .clicked()
        {
            let dead = s.prune(&c.graph);
            changed = true;
            c.set_status(if dead.is_empty() {
                "nothing to prune".into()
            } else {
                format!("pruned {} override(s)", dead.len())
            });
        }
    } else {
        ui.small("This profile is on a different behaviour; open it to edit its overrides.");
    }

    if changed {
        c.lib.subtypes.insert(id.clone(), s);
        c.dirty = true;
    }

    ui.separator();
    compare(ui, c, &id);
}

fn roster(ui: &mut egui::Ui, c: &mut Composer) {
    ui.heading("Profiles");
    ui.horizontal_wrapped(|ui| {
        if ui.button("New").clicked() {
            let id = c.lib.free_id("new-profile", false);
            let mut s = AgentSubtype::new(&id, "New profile", &c.graph_id);
            s.share = 0.0;
            c.lib.subtypes.insert(id.clone(), s);
            c.subtype = Some(id);
            c.dirty = true;
        }
        let selected = c.subtype.clone();
        if ui.add_enabled(selected.is_some(), egui::Button::new("Duplicate")).clicked() {
            if let Some(src) = selected.as_ref().and_then(|id| c.lib.subtypes.get(id)).cloned() {
                let id = c.lib.free_id(&format!("{}-copy", src.id), false);
                let copy = src.duplicate(&id);
                c.lib.subtypes.insert(id.clone(), copy);
                c.subtype = Some(id);
                c.dirty = true;
            }
        }
        if ui.add_enabled(selected.is_some(), egui::Button::new("Delete")).clicked() {
            if let Some(id) = selected {
                match c.lib.delete_subtype(&c.root.clone(), &id) {
                    Ok(()) => {
                        c.subtype = c.lib.subtypes.keys().next().cloned();
                        c.dirty = true;
                        c.set_status(format!("deleted {id}"));
                    }
                    Err(e) => c.set_error(format!("{e:#}")),
                }
            }
        }
    });

    let rows: Vec<(String, String, f32, bool)> = c
        .lib
        .subtypes
        .values()
        .map(|s| (s.id.clone(), s.name.clone(), s.share, s.graph == c.graph_id))
        .collect();
    let total: f32 = rows.iter().map(|r| r.2.max(0.0)).sum();

    egui::ScrollArea::vertical().max_height(180.0).id_source("roster").show(ui, |ui| {
        for (id, name, share, on_this_graph) in rows {
            ui.horizontal(|ui| {
                let selected = c.subtype.as_deref() == Some(id.as_str());
                if ui.selectable_label(selected, &name).clicked() {
                    c.subtype = Some(id.clone());
                }
                if share <= 0.0 {
                    ui.small("—");
                } else if total > 0.0 {
                    ui.small(format!("{:.0}%", 100.0 * share / total));
                }
                if !on_this_graph {
                    ui.small("(other behaviour)");
                }
            });
        }
    });
    if total <= 0.0 {
        ui.colored_label(
            egui::Color32::from_rgb(0xd8, 0xa6, 0x4b),
            "No profile has a share, so applying this would run the shipped model.",
        );
    }
    ui.horizontal(|ui| {
        if ui.selectable_label(c.subtype.is_none(), "none (edit the graph)").clicked() {
            c.subtype = None;
        }
    });
}

/// Two profiles, side by side, in the terms they were authored in.
fn compare(ui: &mut egui::Ui, c: &mut Composer, left: &str) {
    ui.heading("Compare");
    let others: Vec<(String, String)> = c
        .lib
        .subtypes
        .values()
        .filter(|s| s.id != left)
        .map(|s| (s.id.clone(), s.name.clone()))
        .collect();
    if others.is_empty() {
        ui.small("Nothing to compare with yet.");
        return;
    }

    let current = c
        .compare_with
        .as_ref()
        .and_then(|id| c.lib.subtypes.get(id))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "choose…".into());
    egui::ComboBox::from_id_source("compare-with")
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (id, name) in others {
                let sel = c.compare_with.as_deref() == Some(id.as_str());
                if ui.selectable_label(sel, name).clicked() {
                    c.compare_with = Some(id);
                }
            }
        });

    let Some(right) = c.compare_with.clone() else { return };
    let (Some(a), Some(b)) = (c.lib.subtypes.get(left), c.lib.subtypes.get(&right)) else {
        return;
    };
    let shared_graph = (a.graph == b.graph).then(|| &c.graph);
    let diffs = behavior::subtype::compare(a, b, shared_graph.filter(|_| a.graph == c.graph_id));

    if diffs.is_empty() {
        ui.small("Identical, apart from the name.");
        return;
    }
    egui::Grid::new("subtype-diff").striped(true).num_columns(3).show(ui, |ui| {
        ui.strong("");
        ui.strong(&a.name);
        ui.strong(&b.name);
        ui.end_row();
        for d in &diffs {
            ui.label(&d.what);
            ui.label(&d.left);
            ui.label(&d.right);
            ui.end_row();
        }
    });
}
