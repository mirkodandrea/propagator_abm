//! Behaviour testing mode.
//!
//! Put a made-up agent in a situation and read what the graph does, node by
//! node. Which kind of agent, which situations it can be put in and which
//! fields are editable all follow the open graph's [`Domain`] — an engine and a
//! household have no observation field in common, so there is nothing to share
//! but the layout.
//!
//! Three views, because they answer three different questions:
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
use behavior::{ActionKind, CompiledGraph, Domain, Observation, Trace};

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
        let mut b = Bench {
            view: View::default(),
            obs: Observation::default(),
            situation: 0,
            field: SweepField::Cue,
            verbose: false,
        };
        b.reset_to(Domain::Household);
        b
    }
}

impl Bench {
    /// Point the bench at a domain: its situations, its fields, its sweeps.
    ///
    /// Opens on the third situation rather than the first, in both domains. The
    /// first is deliberately the quiet one — nothing has happened, nothing
    /// fires — which is the right thing to have in the list and the wrong thing
    /// to open on, because it shows a behaviour doing nothing.
    pub fn reset_to(&mut self, d: Domain) {
        let s = testbench::situations_for(d);
        self.situation = 2.min(s.len().saturating_sub(1));
        self.obs = s
            .get(self.situation)
            .map(|s| s.obs)
            .unwrap_or_else(|| Observation::empty(d));
        if self.field.domain() != d {
            self.field = SweepField::for_domain(d)[0];
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

    // The bench holds an observation of whatever domain was last open. Switching
    // the editor to the other kind of agent leaves it stale, and evaluating a
    // unit policy against a household would answer with every field at its
    // default — an entirely plausible-looking wrong answer.
    let domain = c.domain();
    if c.bench.obs.domain() != domain {
        c.bench.reset_to(domain);
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
    let domain = c.domain();
    let situations = testbench::situations_for(domain);
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
        egui::Grid::new("bench-inputs").num_columns(2).striped(true).show(ui, |ui| {
            match domain {
                Domain::Household => household_inputs(ui, &mut c.bench.obs),
                Domain::SuppressionUnit => unit_inputs(ui, &mut c.bench.obs),
                Domain::Person => person_inputs(ui, &mut c.bench.obs),
            }
        });
        // Editing anything makes this a situation of the author's own, and
        // labelling it as one of the presets afterwards would be a lie.
        if ui.button("Reset to preset").clicked() {
            if let Some(s) = testbench::situations_for(domain).get(c.bench.situation) {
                c.bench.obs = s.obs;
            }
        }
    });
}

fn slider(ui: &mut egui::Ui, label: &str, v: &mut f32, lo: f32, hi: f32) {
    ui.label(label);
    ui.add(egui::Slider::new(v, lo..=hi));
    ui.end_row();
}

fn flags(ui: &mut egui::Ui, rows: &mut [(&str, &mut bool)]) {
    for (label, flag) in rows {
        ui.label(*label);
        ui.checkbox(flag, "");
        ui.end_row();
    }
}

fn household_inputs(ui: &mut egui::Ui, obs: &mut Observation) {
    let Some(o) = obs.household_mut() else { return };
    slider(ui, "Time (min)", &mut o.time_min, 0.0, 180.0);
    slider(ui, "Threat at home", &mut o.threat, 0.0, 1.0);
    slider(ui, "Radiant", &mut o.radiant, 0.0, 1.0);
    slider(ui, "Ember", &mut o.ember, 0.0, 1.0);
    slider(ui, "Distance to fire (m)", &mut o.fire_distance_m, 0.0, 2500.0);
    slider(ui, "Perceived alarm", &mut o.cue, 0.0, 1.0);
    slider(ui, "Minutes since order", &mut o.minutes_since_order, 0.0, 180.0);
    slider(ui, "Risk perception", &mut o.risk_perception, 0.0, 1.0);
    slider(ui, "Trust in authority", &mut o.trust_authority, 0.0, 1.0);
    slider(ui, "Preparation time (min)", &mut o.prep_time_min, 0.0, 120.0);
    slider(ui, "Defensible space", &mut o.defensible_space, 0.0, 1.0);
    slider(ui, "Household size", &mut o.household_size, 1.0, 8.0);
    slider(ui, "Distance to refuge (m)", &mut o.refuge_distance_m, 0.0, 5000.0);
    slider(ui, "Individual variation", &mut o.jitter, 0.0, 1.0);

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

    flags(
        ui,
        &mut [
            ("House is alight", &mut o.structure_alight),
            ("Order issued", &mut o.order_issued),
            ("Warning received", &mut o.warning_received),
            ("Has a vehicle", &mut o.has_vehicle),
            ("Needs assistance", &mut o.needs_assistance),
            ("Already preparing", &mut o.is_preparing),
            ("Already moving", &mut o.is_moving),
            ("Already defending", &mut o.is_defending),
            ("Route blocked", &mut o.route_blocked),
        ],
    );
}

fn unit_inputs(ui: &mut egui::Ui, obs: &mut Observation) {
    let Some(o) = obs.unit_mut() else { return };
    ui.label("Kind");
    egui::ComboBox::from_id_source("bench-unit-kind")
        .selected_text(o.kind.label())
        .show_ui(ui, |ui| {
            for k in behavior::UnitKindKey::ALL {
                if ui.selectable_label(o.kind == k, k.label()).clicked() {
                    o.kind = k;
                    // The tank follows the kind, because a hand crew with water
                    // in it is a situation the model cannot produce and a
                    // policy tested against it is tested against nothing.
                    o.carries_water = k != behavior::UnitKindKey::HandCrew;
                    if !o.carries_water {
                        o.water_fraction = 0.0;
                    }
                }
            }
        });
    ui.end_row();

    slider(ui, "Time (min)", &mut o.time_min, 0.0, 180.0);
    slider(ui, "Threat at the unit", &mut o.threat_here, 0.0, 1.0);
    slider(ui, "Accumulated heat", &mut o.heat_fraction, 0.0, 1.0);
    slider(ui, "Distance to fire (m)", &mut o.distance_to_fire_m, 0.0, 3000.0);
    slider(ui, "Water remaining", &mut o.water_fraction, 0.0, 1.0);
    slider(ui, "Distance to the order (m)", &mut o.distance_to_task_m, 0.0, 5000.0);
    slider(ui, "Distance to staging (m)", &mut o.distance_to_base_m, 0.0, 8000.0);
    slider(ui, "Line cut so far", &mut o.line_progress, 0.0, 1.0);
    slider(ui, "Minutes on this order", &mut o.minutes_on_task, 0.0, 180.0);
    slider(ui, "Individual variation", &mut o.jitter, 0.0, 1.0);

    flags(
        ui,
        &mut [
            ("Carries water", &mut o.carries_water),
            ("Has an order", &mut o.has_task),
            ("Order is reachable", &mut o.task_reachable),
            ("Something to work on", &mut o.work_available),
            ("Already working", &mut o.is_working),
            ("Already travelling", &mut o.is_moving),
        ],
    );
}

fn person_inputs(ui: &mut egui::Ui, obs: &mut Observation) {
    let Some(o) = obs.person_mut() else { return };

    slider(ui, "Time (min)", &mut o.time_min, 0.0, 180.0);
    slider(ui, "Threat where they stand", &mut o.threat, 0.0, 1.0);
    slider(ui, "Accumulated heat", &mut o.heat_fraction, 0.0, 1.0);
    slider(ui, "Distance to fire (m)", &mut o.fire_distance_m, 0.0, 2500.0);
    slider(ui, "Perceived alarm", &mut o.cue, 0.0, 1.0);
    slider(ui, "Minutes since order", &mut o.minutes_since_order, 0.0, 180.0);
    slider(ui, "Age", &mut o.age, 0.0, 100.0);
    slider(ui, "Walking speed (m/s)", &mut o.walk_speed, 0.2, 2.0);
    slider(ui, "Distance home (m)", &mut o.home_distance_m, 0.0, 3000.0);
    slider(ui, "Distance to refuge (m)", &mut o.refuge_distance_m, 0.0, 3000.0);
    slider(ui, "Individual variation", &mut o.jitter, 0.0, 1.0);

    flags(
        ui,
        &mut [
            ("Order issued", &mut o.order_issued),
            ("Needs assistance", &mut o.needs_assistance),
            ("Family still at home", &mut o.household_at_home),
            ("Family already safe", &mut o.household_safe),
            ("Home is alight", &mut o.home_alight),
            ("Route blocked", &mut o.route_blocked),
            ("Already walking", &mut o.is_moving),
            ("Already heading home", &mut o.is_heading_home),
            ("Already sheltering", &mut o.is_sheltering),
        ],
    );
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
        // What "nothing fired" *means* differs by domain, and it is the whole
        // difference between the two decision sinks: a household that has been
        // told nothing stays put, while a unit already has its orders.
        ui.small(match c.domain() {
            Domain::Household => "Nothing fired — the household stays put.",
            Domain::SuppressionUnit => {
                "Nothing fired — the unit gets on with the order it was given, which is \
                 the common case."
            }
            Domain::Person => {
                "Nothing fired — they carry on doing whatever they were doing, which for \
                 almost everyone in this domain means walking out."
            }
        });
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
            let here = c.selected.and_then(|s| c.snarl.get_node(s)).map(|e| e.id);
            let r = ui.selectable_label(
                here == Some(n.node),
                format!("{}  =  {values}", n.name),
            );
            if r.clicked() {
                select = c.snarl_id_of(n.node);
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
                for f in SweepField::for_domain(c.domain()).iter().copied() {
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
                ui.small("»");
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

/// One colour per action, shared by the proposal list, the trace and the sweep
/// strip.
///
/// The three domains reuse the same five hues on purpose, matched by what the
/// action *means* rather than by position: grey is "nothing changed", amber is
/// "getting ready to move", blue is "moving", green is "committing to the job",
/// red is "this is the dangerous one". Only one domain's actions are ever on
/// screen at a time, so there is nothing to tell apart.
pub fn action_colour(a: ActionKind) -> egui::Color32 {
    match a {
        ActionKind::Stay | ActionKind::Continue | ActionKind::Remain => {
            egui::Color32::from_rgb(0x5a, 0x60, 0x68)
        }
        ActionKind::Prepare | ActionKind::HoldPosition => {
            egui::Color32::from_rgb(0xd8, 0xa6, 0x4b)
        }
        ActionKind::EvacuateNow | ActionKind::ReturnToBase | ActionKind::WalkOut => {
            egui::Color32::from_rgb(0x6f, 0xb1, 0xe8)
        }
        // Going home is a commitment to a destination, the same shape of choice
        // defending and refilling are.
        ActionKind::Defend | ActionKind::Refill | ActionKind::HeadHome => {
            egui::Color32::from_rgb(0x7a, 0xb2, 0x8a)
        }
        ActionKind::Shelter | ActionKind::Withdraw | ActionKind::TakeShelter => {
            egui::Color32::from_rgb(0xe0, 0x6c, 0x5f)
        }
    }
}
