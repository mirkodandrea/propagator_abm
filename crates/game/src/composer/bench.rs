//! Behaviour testing mode.
//!
//! Put a made-up household in a situation and read what the graph does, node
//! by node. Three views, because they answer three different questions:
//!
//! - **Evaluate** — one situation, the full trace. "Why did it decide that?"
//! - **Sweep** — one field varied across its range. "*Where* does it change?"
//! - **Profiles** — every subtype against the same situation. "Do these
//!   profiles actually differ?"
//!
//! The sweep is the one that earns its place. A threshold in this model is
//! never a single number — it is a number plus a risk term plus a jitter term
//! — so "what alarm level does this household leave at" is not readable off
//! the canvas, and guessing at it is how the hand-written version acquired
//! four wrong ignition placements.

use bevy_egui::egui;

use behavior::testbench::{self, SweepField};
use behavior::{ActionKind, CompiledGraph, Observation, Trace};

use super::Composer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Evaluate,
    Sweep,
    Profiles,
}

pub struct Bench {
    pub view: View,
    pub obs: Observation,
    pub situation: usize,
    pub field: SweepField,
    /// Show every node's value, not just the ones that decided it.
    pub verbose: bool,
}

impl Default for Bench {
    fn default() -> Self {
        let s = testbench::situations();
        Bench {
            view: View::default(),
            obs: s.get(2).map(|s| s.obs).unwrap_or_default(),
            situation: 2,
            field: SweepField::Cue,
            verbose: false,
        }
    }
}

pub fn panel(ui: &mut egui::Ui, c: &mut Composer) {
    ui.heading("Test bench");
    if !c.runnable() {
        ui.colored_label(
            egui::Color32::from_rgb(0xe0, 0x6c, 0x5f),
            "This behaviour has errors and cannot be run. Fix them below the canvas.",
        );
        return;
    }

    ui.horizontal(|ui| {
        ui.selectable_value(&mut c.bench.view, View::Evaluate, "Evaluate");
        ui.selectable_value(&mut c.bench.view, View::Sweep, "Sweep");
        ui.selectable_value(&mut c.bench.view, View::Profiles, "Profiles");
    });
    ui.separator();

    situation_picker(ui, c);
    ui.separator();

    match c.bench.view {
        View::Evaluate => evaluate(ui, c),
        View::Sweep => sweep(ui, c),
        View::Profiles => profiles(ui, c),
    }
}

/// The situation: a preset, then every field of it editable.
fn situation_picker(ui: &mut egui::Ui, c: &mut Composer) {
    let situations = testbench::situations();
    ui.horizontal(|ui| {
        ui.label("Situation");
        let name = situations.get(c.bench.situation).map(|s| s.name).unwrap_or("custom");
        egui::ComboBox::from_id_source("bench-situation")
            .selected_text(name)
            .show_ui(ui, |ui| {
                for (i, s) in situations.iter().enumerate() {
                    if ui
                        .selectable_label(c.bench.situation == i, s.name)
                        .on_hover_text(s.note)
                        .clicked()
                    {
                        c.bench.situation = i;
                        c.bench.obs = s.obs;
                    }
                }
            });
    });
    if let Some(s) = situations.get(c.bench.situation) {
        ui.small(s.note);
    }

    egui::CollapsingHeader::new("Agent inputs").default_open(false).show(ui, |ui| {
        let o = &mut c.bench.obs;
        egui::Grid::new("bench-inputs").num_columns(2).striped(true).show(ui, |ui| {
            let num = |ui: &mut egui::Ui, label: &str, v: &mut f32, lo: f32, hi: f32| {
                ui.label(label);
                ui.add(egui::Slider::new(v, lo..=hi));
                ui.end_row();
            };
            num(ui, "Time (min)", &mut o.time_min, 0.0, 180.0);
            num(ui, "Threat at home", &mut o.threat, 0.0, 1.0);
            num(ui, "Radiant", &mut o.radiant, 0.0, 1.0);
            num(ui, "Ember", &mut o.ember, 0.0, 1.0);
            num(ui, "Distance to fire (m)", &mut o.fire_distance_m, 0.0, 2500.0);
            num(ui, "Perceived alarm", &mut o.cue, 0.0, 1.0);
            num(ui, "Minutes since order", &mut o.minutes_since_order, 0.0, 180.0);
            num(ui, "Risk perception", &mut o.risk_perception, 0.0, 1.0);
            num(ui, "Trust in authority", &mut o.trust_authority, 0.0, 1.0);
            num(ui, "Preparation time (min)", &mut o.prep_time_min, 0.0, 120.0);
            num(ui, "Defensible space", &mut o.defensible_space, 0.0, 1.0);
            num(ui, "Household size", &mut o.household_size, 1.0, 8.0);
            num(ui, "Distance to refuge (m)", &mut o.refuge_distance_m, 0.0, 5000.0);
            num(ui, "Individual variation", &mut o.jitter, 0.0, 1.0);

            ui.label("Stated intent");
            egui::ComboBox::from_id_source("bench-intent")
                .selected_text(o.intent.label())
                .show_ui(ui, |ui| {
                    for i in behavior::IntentValue::ALL {
                        if ui.selectable_label(o.intent == i, i.label()).clicked() {
                            o.intent = i;
                        }
                    }
                });
            ui.end_row();

            for (label, flag) in [
                ("House is alight", &mut o.structure_alight),
                ("Order issued", &mut o.order_issued),
                ("Warning received", &mut o.warning_received),
                ("Has a vehicle", &mut o.has_vehicle),
                ("Needs assistance", &mut o.needs_assistance),
                ("Already preparing", &mut o.is_preparing),
                ("Already moving", &mut o.is_moving),
                ("Already defending", &mut o.is_defending),
                ("Route blocked", &mut o.route_blocked),
            ] {
                ui.label(label);
                ui.checkbox(flag, "");
                ui.end_row();
            }
        });
        // Editing anything makes this a situation of the author's own, and
        // labelling it as one of the presets afterwards would be a lie.
        if ui.button("Reset to preset").clicked() {
            if let Some(s) = testbench::situations().get(c.bench.situation) {
                c.bench.obs = s.obs;
            }
        }
    });
}

/// Compile the canvas with the selected profile's overrides applied — the same
/// thing the model would run.
fn compiled(c: &Composer) -> Option<CompiledGraph> {
    CompiledGraph::compile(&c.graph, &c.active_overrides()).ok()
}

fn evaluate(ui: &mut egui::Ui, c: &mut Composer) {
    let Some(g) = compiled(c) else {
        ui.label("This behaviour will not compile.");
        return;
    };
    let (decision, trace) = g.eval_traced(&c.bench.obs);

    if let Some(id) = &c.subtype {
        let name = c.lib.subtypes.get(id).map(|s| s.name.as_str()).unwrap_or(id);
        ui.small(format!("with profile: {name}"));
    } else {
        ui.small("with the graph's own values");
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.strong("Decision");
        ui.colored_label(action_colour(decision.action), decision.action.label());
    });
    ui.small(format!(
        "priority {:.2} · preparation ×{:.2} · urgency {:.2}",
        decision.priority, decision.prep_scale, decision.urgency
    ));

    ui.separator();
    ui.label("Proposals");
    if trace.proposals.is_empty() {
        ui.small("Nothing fired — the household stays put.");
    } else {
        for (i, (node, kind, prio)) in trace.proposals.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.colored_label(action_colour(*kind), if i == 0 { "▶" } else { " " });
                ui.small(format!("{} @ {prio:.2}  (#{node})", kind.label()));
            });
        }
        if trace.proposals.len() > 1 {
            ui.small("The strongest wins; the rest are shown so a losing branch is visible.");
        }
    }

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Evaluation");
        ui.checkbox(&mut c.bench.verbose, "every node");
    });
    trace_list(ui, c, &trace);
}

/// The trace, in evaluation order. Clicking a row selects the node, which is
/// how an author gets from "this number is wrong" to the box that produced it.
fn trace_list(ui: &mut egui::Ui, c: &mut Composer, trace: &Trace) {
    let verbose = c.bench.verbose;
    let mut select = None;
    egui::ScrollArea::vertical().max_height(320.0).id_source("trace").show(ui, |ui| {
        for n in &trace.nodes {
            let interesting = verbose
                || behavior::registry()
                    .get(n.type_id)
                    .map(|s| s.category != behavior::Category::Observation)
                    .unwrap_or(true);
            if !interesting {
                continue;
            }
            let values = n
                .outputs
                .iter()
                .map(behavior::Value::display)
                .collect::<Vec<_>>()
                .join(", ");
            let r = ui.selectable_label(
                c.selected.map(|s| s.0 as u32) == Some(n.node),
                format!("{}  =  {values}", n.name),
            );
            if r.clicked() {
                select = Some(egui_snarl::NodeId(n.node as usize));
            }
        }
    });
    if let Some(n) = select {
        c.selected = Some(n);
    }
}

fn sweep(ui: &mut egui::Ui, c: &mut Composer) {
    let Some(g) = compiled(c) else {
        ui.label("This behaviour will not compile.");
        return;
    };

    ui.horizontal(|ui| {
        ui.label("Vary");
        egui::ComboBox::from_id_source("sweep-field")
            .selected_text(c.bench.field.label())
            .show_ui(ui, |ui| {
                for f in SweepField::ALL {
                    if ui.selectable_label(c.bench.field == f, f.label()).clicked() {
                        c.bench.field = f;
                    }
                }
            });
    });
    ui.small("Everything else is held at the situation above.");

    let points = testbench::sweep(&g, &c.bench.obs, c.bench.field, 121);
    let (lo, hi) = c.bench.field.range();

    // A strip of colour: one band per action, across the range. It is a
    // one-dimensional picture and that is exactly what the question is.
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 26.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    let n = points.len().max(1);
    for (i, p) in points.iter().enumerate() {
        let x0 = rect.left() + rect.width() * i as f32 / n as f32;
        let x1 = rect.left() + rect.width() * (i + 1) as f32 / n as f32;
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1, rect.bottom())),
            0.0,
            action_colour(p.action),
        );
    }
    // The current situation's own value, so the strip is anchored to the case
    // being examined rather than floating free.
    let here = c.bench.field.get(&c.bench.obs);
    if (lo..=hi).contains(&here) && hi > lo {
        let x = rect.left() + rect.width() * (here - lo) / (hi - lo);
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(2.0, egui::Color32::WHITE),
        );
    }
    ui.horizontal(|ui| {
        ui.small(format!("{lo:.0}"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.small(format!("{hi:.0}"));
        });
    });

    ui.separator();
    let changes = testbench::transitions(&points);
    if changes.is_empty() {
        ui.label(format!(
            "{} never changes the decision here — it comes out {} throughout.",
            c.bench.field.label(),
            points.first().map(|p| p.action.label()).unwrap_or("?")
        ));
    } else {
        ui.label("Thresholds");
        for (at, from, to) in changes {
            ui.horizontal(|ui| {
                ui.colored_label(action_colour(from), from.label());
                ui.small("→");
                ui.colored_label(action_colour(to), to.label());
                ui.small(format!("at {at:.3}"));
            });
        }
    }

    ui.separator();
    ui.label("Legend");
    ui.horizontal_wrapped(|ui| {
        for a in ActionKind::ALL {
            ui.colored_label(action_colour(a), "■");
            ui.small(a.label());
        }
    });
}

fn profiles(ui: &mut egui::Ui, c: &mut Composer) {
    ui.small("Every profile on this behaviour, against the situation above.");
    let ids: Vec<String> = c
        .lib
        .subtypes
        .values()
        .filter(|s| s.graph == c.graph_id)
        .map(|s| s.id.clone())
        .collect();
    if ids.is_empty() {
        ui.label("No profile uses this behaviour yet.");
        return;
    }

    let obs = c.bench.obs;
    egui::Grid::new("bench-profiles").striped(true).num_columns(4).show(ui, |ui| {
        ui.strong("Profile");
        ui.strong("Decision");
        ui.strong("Priority");
        ui.strong("Prep ×");
        ui.end_row();
        for id in &ids {
            let Some(s) = c.lib.subtypes.get(id) else { continue };
            // Compiled against the *canvas*, not the library's copy of the
            // graph, so an unsaved edit is included — the bench has to test
            // what is on screen.
            let Ok(g) = CompiledGraph::compile(&c.graph, &s.overrides) else {
                ui.label(&s.name);
                ui.colored_label(egui::Color32::from_rgb(0xe0, 0x6c, 0x5f), "will not compile");
                ui.label("");
                ui.label("");
                ui.end_row();
                continue;
            };
            let d = g.eval(&obs);
            ui.label(&s.name);
            ui.colored_label(action_colour(d.action), d.action.label());
            ui.label(format!("{:.2}", d.priority));
            ui.label(format!("{:.2}", d.prep_scale));
            ui.end_row();
        }
    });

    ui.add_space(4.0);
    ui.small(
        "Profiles that all answer the same in every situation are not profiles — they are one \
         profile with four names.",
    );
}

fn action_colour(a: ActionKind) -> egui::Color32 {
    match a {
        ActionKind::Stay => egui::Color32::from_rgb(0x5a, 0x60, 0x68),
        ActionKind::Prepare => egui::Color32::from_rgb(0xd8, 0xa6, 0x4b),
        ActionKind::EvacuateNow => egui::Color32::from_rgb(0x6f, 0xb1, 0xe8),
        ActionKind::Defend => egui::Color32::from_rgb(0x7a, 0xb2, 0x8a),
        ActionKind::Shelter => egui::Color32::from_rgb(0xe0, 0x6c, 0x5f),
    }
}
