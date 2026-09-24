//! Watching one agent's behaviour run.
//!
//! Select a household, a person or a unit on the map to inspect its applied
//! graph without changing the open draft. The debugger stays in step with the
//! incident: what every node produced this tick, which of them fed the decision
//! that was taken, which branches were checked and declined, and which parts of
//! the graph have not mattered in the last few minutes.
//!
//! ### There is no execution cursor, and pretending otherwise would lie
//!
//! A [`behavior::BehaviorGraph`] is **dataflow**, not a behaviour tree. Every
//! node is evaluated on every decision tick, in topological order, and the whole
//! evaluation happens between two instants of simulated time. So "the node that
//! is currently executing" has no referent in a running incident — asking for it
//! is asking about a machine this one is not.
//!
//! What does have a referent, and is what this draws:
//!
//! | Asked for | Drawn as |
//! |---|---|
//! | the active path | the backward slice from the winning proposal to the observations that produced it ([`behavior::Trace::active`]) |
//! | inactive branches | action nodes that ran and withheld their proposal |
//! | previously traversed | nodes that were in the slice on a recent tick and are not on this one |
//! | unvisited | nodes that have not been on the slice since watching started |
//!
//! Every one of those is a fact about the trace rather than an inference, which
//! is the property that makes the highlight worth looking at: a colour that
//! guesses is worse than no colour.
//!
//! ### Stepping
//!
//! `Sim::request_step` advances exactly one decision interval, whether or not
//! the clock is running, and a capture happens on the far side of it. That is
//! the granularity a behaviour is authored at; the fire's own 2 s quantum would
//! mean pressing the key three times to see one change.
//!
//! ### Switching agents
//!
//! The history is per subject and is dropped the moment the subject changes, so
//! one agent's traversed path can never be shown over another's graph. That is
//! the whole of the "do not confuse their runtime states" requirement, and it is
//! cheap because there is only ever one subject.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use bevy::prelude::*;
use bevy_egui::egui;

use behavior::{ActionKind, BehaviorGraph, Decision, NodeId, Trace, Value};

use super::viewer::LiveRole;
use super::Composer;
use crate::inspect::{Selected, Target};
use crate::sim::Sim;

/// How many decision ticks of history the "was recently on the path" shading
/// looks back over.
///
/// Sixty ticks is five simulated minutes, which is roughly how long a household
/// takes to get from noticing something to leaving. Shorter and the fading
/// tells you nothing; much longer and everything is shaded, which also tells
/// you nothing.
const HISTORY: usize = 60;

/// Which agent is being watched.
///
/// Held as the inspector's own [`Target`] rather than an id, so "the selection
/// changed" is one comparison and there is no second notion of what is
/// selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subject {
    pub target: Target,
}

/// One decision, recorded.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub time_s: i64,
    pub action: ActionKind,
    pub priority: f32,
    pub active: BTreeSet<NodeId>,
}

/// Everything the canvas needs to draw one tick of one agent's behaviour.
///
/// Pre-chewed into lookups rather than handing the canvas a [`Trace`]: the
/// viewer touches this once per node and once per pin, and walking a vector to
/// find a node id each time would be the slowest thing in the editor.
pub struct Frame {
    /// The graph this trace came out of. The canvas draws the live state only
    /// when it is showing the same graph — a slice from one behaviour laid over
    /// another is exactly the class of confusion this view exists to remove.
    pub graph_id: String,
    /// The exact graph that was compiled for this profile, with all profile
    /// parameter overrides materialised. This deliberately comes from the
    /// simulation's applied library, not the editor's possibly-dirty copy.
    pub graph: BehaviorGraph,
    pub subtype_id: String,
    pub subtype_name: String,
    pub agent: String,
    pub decision: Decision,
    pub trace: Trace,
    pub winner: Option<NodeId>,
    pub active: BTreeSet<NodeId>,
    pub withheld: BTreeSet<NodeId>,
    /// Nodes that have been on the path at some point in the recent history.
    pub seen: BTreeSet<NodeId>,
    pub values: BTreeMap<NodeId, Vec<Value>>,
    pub inputs: BTreeMap<(NodeId, u16), Vec<Value>>,
    /// The editor has edits the running model has not been given. Said out
    /// loud, because a slice drawn over a node the incident has never seen is
    /// otherwise indistinguishable from a node the incident ignored.
    pub stale: bool,
}

impl Frame {
    pub fn role(&self, id: NodeId) -> LiveRole {
        if self.active.contains(&id) {
            LiveRole::Active
        } else if self.withheld.contains(&id) {
            LiveRole::Withheld
        } else if self.seen.contains(&id) {
            LiveRole::Recent
        } else {
            LiveRole::Cold
        }
    }
}

/// The composer's live-inspection state.
#[derive(Default)]
pub struct Live {
    /// Keep the explanation readable without requiring graph literacy.
    pub selected_node: Option<NodeId>,
    /// The agent being watched, if any.
    pub subject: Option<Subject>,
    /// The most recent capture. Taken out of the composer while the canvas
    /// draws, and put back after.
    pub frame: Option<Frame>,
    /// Newest last, capped at [`HISTORY`].
    pub history: VecDeque<Recorded>,
    /// Union of the active sets across `history`.
    seen: BTreeSet<NodeId>,
    /// The sim generation the last capture was taken at, so a paused sim is
    /// not re-explained every frame.
    last_generation: u64,

    // --- the transport ------------------------------------------------------
    //
    // Mirrored off `Sim` by `capture` and back onto it by `transport`, because
    // an egui panel here is handed `&mut Composer` and nothing else. Two flags
    // and two readouts is a smaller price than threading the world into every
    // panel function, and it keeps the panels testable as plain functions.
    /// Whether the incident is running, as of the last capture.
    pub playing: bool,
    /// `T+HH:MM:SS`, as of the last capture.
    pub clock: String,
    /// The panel asked for play/pause.
    pub toggle_play: bool,
    /// The panel asked for one decision tick.
    pub step: bool,
}

impl Live {
    /// Drop everything about the agent that was being watched.
    ///
    /// Called whenever the subject changes. Keeping the history across a switch
    /// would shade the new agent's graph with the old one's path, which reads
    /// exactly like the new agent having been somewhere it has never been.
    fn forget(&mut self) {
        self.frame = None;
        self.selected_node = None;
        self.history.clear();
        self.seen.clear();
        self.last_generation = u64::MAX;
    }

    fn record(&mut self, time_s: i64, decision: Decision, active: &BTreeSet<NodeId>) {
        if self.history.back().map(|r| r.time_s) == Some(time_s) {
            return;
        }
        self.history.push_back(Recorded {
            time_s,
            action: decision.action,
            priority: decision.priority,
            active: active.clone(),
        });
        while self.history.len() > HISTORY {
            self.history.pop_front();
        }
        self.seen = self.history.iter().flat_map(|r| r.active.iter().copied()).collect();
    }
}

/// One traced evaluation of whatever the selected agent is running.
struct Capture {
    graph_id: String,
    graph: BehaviorGraph,
    subtype_id: String,
    subtype_name: String,
    agent: String,
    decision: Decision,
    trace: Trace,
}

/// Ask the model to explain one agent, whichever kind it is.
///
/// Returns `None` only when the target no longer exists or its required graph
/// cannot be resolved. Every live decision layer is graph-backed.
fn explain(sim: &Sim, target: Target) -> Option<Capture> {
    let lib = &sim.behaviour;
    let graph_of = |subtype: &str| effective_graph(lib, subtype);

    match target {
        Target::Household(id) => {
            let (sid, name, _) = sim.agents.behaviour_of(id)?;
            let (sid, name) = (sid.to_string(), name.to_string());
            let (decision, trace) = sim.agents.explain(id, &sim.fire)?;
            let graph = graph_of(&sid)?;
            Some(Capture {
                graph_id: graph.id.clone(),
                graph,
                subtype_id: sid,
                subtype_name: name,
                agent: format!("Household #{id}"),
                decision,
                trace,
            })
        }
        Target::Person(id) => {
            let person = sim.agents.people.get(id)?;
            // At home the household makes the decision for this person. Show
            // that real governing graph instead of an empty person panel.
            if !person.away {
                let household = person.household;
                let mut cap = explain(sim, Target::Household(household))?;
                cap.agent = format!("Person #{id} · governed by Household #{household}");
                return Some(cap);
            }
            let (sid, name, _) = sim.agents.person_behaviour_of(id)?;
            let (sid, name) = (sid.to_string(), name.to_string());
            let (decision, trace) = sim.agents.explain_person(id, &sim.fire)?;
            let graph = graph_of(&sid)?;
            Some(Capture {
                graph_id: graph.id.clone(),
                graph,
                subtype_id: sid,
                subtype_name: name,
                agent: format!("Person #{id}"),
                decision,
                trace,
            })
        }
        Target::Unit(id) => {
            let (sid, name) = sim.crews.policy_of(id)?;
            let (sid, name) = (sid.to_string(), name.to_string());
            let (decision, trace) =
                sim.crews.explain(id, &sim.agents.network, &sim.fire, &sim.scenario)?;
            let call = sim.crews.units.get(id).map(|u| u.callsign.clone()).unwrap_or_default();
            let graph = graph_of(&sid)?;
            Some(Capture {
                graph_id: graph.id.clone(),
                graph,
                subtype_id: sid,
                subtype_name: name,
                agent: call,
                decision,
                trace,
            })
        }
        // A group on the move is a household or one person walking alone. Both
        // have a behaviour; the traveller itself is a vehicle, not an agent.
        Target::Traveller(i) => {
            let t = sim.agents.travellers.get(i)?;
            if t.solo {
                explain(sim, Target::Person(*t.members.first()?))
            } else {
                explain(sim, Target::Household(t.household))
            }
        }
    }
}

/// Resolve the graph as this profile actually runs it. Overrides are normally
/// folded into the compiled evaluator; materialising them here lets the Debug
/// tab show those effective values on the nodes themselves.
fn effective_graph(lib: &behavior::Library, subtype_id: &str) -> Option<BehaviorGraph> {
    let subtype = lib.subtypes.get(subtype_id)?;
    let mut graph = lib.graphs.get(&subtype.graph)?.clone();
    for node in &mut graph.nodes {
        let Some(spec) = node.spec() else { continue };
        for param in spec.params {
            let key = BehaviorGraph::override_key(node.id, param.name);
            let value = param.resolve_value(node.params.get(param.name), subtype.overrides.get(&key));
            node.params.insert(param.name.to_string(), value);
        }
    }
    Some(graph)
}

/// Keep the composer's live view in step with the incident.
///
/// Scheduled after `sim::step_fire` so a captured trace describes the state the
/// last step produced rather than the one before it, and after the selection
/// systems so a click and its capture land on the same frame.
pub fn capture(sim: Res<Sim>, selected: Res<Selected>, mut composer: ResMut<Composer>) {
    let c = &mut *composer;
    c.live.playing = sim.playing;
    c.live.clock = format!("T+{}", sim.clock());

    // The composer being shut does not stop the capture: the bottom inspector
    // shows the decision too, and a "show behaviour" button that opened onto an
    // empty panel would be a worse answer than one that opens onto the graph.
    let subject = selected.target.map(|target| Subject { target });
    if c.live.subject != subject {
        c.live.subject = subject;
        c.live.forget();
    }
    let Some(subject) = subject else {
        c.live.frame = None;
        return;
    };

    // Draft changes must be visible even while the simulation is paused.
    if let Some(frame) = &mut c.live.frame { frame.stale = c.dirty; }

    // Nothing has moved and the agent has not changed, so the last capture is
    // still the answer. `explain` is one graph evaluation, but it is one per
    // frame at 60 Hz for as long as something is selected.
    if c.live.frame.is_some() && c.live.last_generation == sim.generation {
        return;
    }
    c.live.last_generation = sim.generation;

    let Some(cap) = explain(&sim, subject.target) else {
        c.live.frame = None;
        return;
    };

    let active = cap.trace.active();
    c.live.record(sim.time_s(), cap.decision, &active);

    let mut values = BTreeMap::new();
    let mut inputs = BTreeMap::new();
    for n in &cap.trace.nodes {
        values.insert(n.node, n.outputs.clone());
        for (port, slot) in n.inputs.iter().enumerate() {
            if !slot.is_empty() {
                inputs.insert((n.node, port as u16), slot.clone());
            }
        }
    }

    c.live.frame = Some(Frame {
        graph_id: cap.graph_id,
        graph: cap.graph,
        subtype_id: cap.subtype_id,
        subtype_name: cap.subtype_name,
        agent: cap.agent,
        decision: cap.decision,
        winner: cap.trace.winner(),
        withheld: cap.trace.withheld(),
        trace: cap.trace,
        active,
        seen: c.live.seen.clone(),
        values,
        inputs,
        stale: c.dirty,
    });


}

// ---------------------------------------------------------------------------
// The panel
// ---------------------------------------------------------------------------

/// The applied graph and its trace share one canvas; details come from the
/// selected runtime node, never from a draft with a matching graph id.
pub fn debugger_panel(ui: &mut egui::Ui, c: &mut Composer) {
    transport(ui, c);
    if let Some(frame) = &c.live.frame {
        ui.horizontal_wrapped(|ui| {
            ui.strong(&frame.agent);
            ui.weak(&frame.subtype_name).on_hover_text(&frame.subtype_id);
            ui.colored_label(super::bench::action_colour(frame.decision.action), frame.decision.action.label());
            ui.weak("Applied behavior");
        });
    } else {
        ui.centered_and_justified(|ui| { ui.label("Select a household, person or unit on the map, then open Debug."); });
        return;
    }
    ui.separator();
    egui::SidePanel::right("live-debug-details")
        .resizable(true).default_width(320.0).width_range(260.0..=460.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let Some(frame) = &c.live.frame else { return };
                if c.dirty { ui.weak("Draft edits are not running."); }
                ui.strong("Actions considered");
                if frame.trace.proposals.is_empty() { ui.weak("No proposal · default action"); }
                for (node, kind, priority) in &frame.trace.proposals {
                    let winner = frame.winner == Some(*node);
                    let label = format!("{}{}  ·  {:.2}", if winner { "▶ " } else { "" }, kind.label(), priority);
                    if ui.selectable_label(c.live.selected_node == Some(*node), label).clicked() {
                        c.live.selected_node = Some(*node);
                    }
                }
                ui.separator();
                if let Some(node) = c.live.selected_node.and_then(|id| frame.graph.node(id)) {
                    if let Some(spec) = node.spec() {
                        ui.heading(spec.name);
                        ui.colored_label(frame.role(node.id).colour(), frame.role(node.id).label());
                        egui::CollapsingHeader::new("About this node").show(ui, |ui| { ui.label(spec.doc); });
                        if let Some(trace) = frame.trace.node(node.id) {
                            ui.strong("Outputs");
                            for (port, value) in spec.outputs.iter().zip(&trace.outputs) {
                                ui.label(format!("{}: {}", port.name, value.display()));
                            }
                            if !trace.params_read.is_empty() {
                                ui.separator();
                                ui.strong("Parameters used");
                                for index in &trace.params_read {
                                    if let Some(param) = spec.params.get(*index as usize) {
                                        let value = frame.graph.param(node.id, param.name).unwrap_or_else(|| param.default_value());
                                        ui.label(format!("{}: {}", param.label, value.display()));
                                    }
                                }
                            }
                        }
                    } else { ui.label(&node.type_id); }
                } else { ui.weak("Select a node to inspect its values."); }
                ui.separator();
                egui::CollapsingHeader::new("Decision history").show(ui, |ui| {
                    let mut last = None;
                    for r in c.live.history.iter().rev() {
                        if last == Some(r.action) { continue; }
                        last = Some(r.action);
                        ui.small(format!("T+{:02}:{:02}  {} · {:.2}", r.time_s / 60, r.time_s % 60, r.action.label(), r.priority));
                    }
                });
                egui::CollapsingHeader::new("Legend").show(ui, |ui| {
                    for role in [LiveRole::Active, LiveRole::Withheld, LiveRole::Recent, LiveRole::Cold] {
                        ui.colored_label(role.colour(), role.label());
                    }
                    ui.small("Every node evaluates. The highlighted connections contribute to the winning decision.");
                });
            });
        });
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut c.wiring, false, "Structure");
            ui.selectable_value(&mut c.wiring, true, "Wiring");
        });
        if c.wiring { super::viewer::debug_canvas(ui, c); }
        else if let Some(frame) = &c.live.frame {
            super::structure::panel(ui, &frame.graph, Some(frame), &mut c.live.selected_node);
        }
    });
}

/// Play, pause and step, in the panel that needs them.
///
/// Duplicated from the menu bar's status strip on purpose: someone reading a
/// behaviour tick by tick should not have to go to the other end of the screen
/// and back for every step, and the composer window covers the strip anyway.
fn transport(ui: &mut egui::Ui, c: &mut Composer) {
    ui.horizontal(|ui| {
        let (glyph, hint) = if c.live.playing {
            ("Pause", "Pause the incident")
        } else {
            ("Run", "Run the incident")
        };
        if ui.button(glyph).on_hover_text(hint).clicked() {
            c.live.toggle_play = true;
        }
        if ui
            .button("Next decision")
            .on_hover_text(
                "One decision tick. Every agent decides exactly once, and the highlight \
                 moves to what they decided.",
            )
            .clicked()
        {
            c.live.step = true;
        }
        ui.small(&c.live.clock);
    });
}

/// Carry the panel's transport requests onto the simulation.
///
/// A separate system rather than the panel touching `Sim` directly, because the
/// composer's panels are plain `fn(&mut Ui, &mut Composer)` and keeping them
/// that way is what lets them be read — and eventually tested — without a
/// world.
pub fn transport_requests(mut sim: ResMut<Sim>, mut composer: ResMut<Composer>) {
    if std::mem::take(&mut composer.live.toggle_play) {
        sim.playing = !sim.playing;
    }
    if std::mem::take(&mut composer.live.step) {
        sim.request_step();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applied_parameter_display_matches_runtime_even_for_malformed_overrides() {
        let mut lib = behavior::defaults::default_library();
        let profile = lib.subtypes.values().find(|s| lib.domain_of(s) == Some(behavior::Domain::Household)).unwrap().id.clone();
        let graph_id = lib.subtypes[&profile].graph.clone();
        let graph = lib.graphs.get_mut(&graph_id).unwrap();
        let id = graph.add("param.number", [0.0, 0.0]).unwrap();
        graph.node_mut(id).unwrap().params.insert("value".into(), behavior::ParamValue::Number(42.0));
        let key = BehaviorGraph::override_key(id, "value");
        for value in [behavior::ParamValue::Bool(true), behavior::ParamValue::Number(9.0)] {
            lib.subtypes.get_mut(&profile).unwrap().overrides.insert(key.clone(), value);
            let (_, trace) = lib.compile(&profile).unwrap().eval_traced(&behavior::HouseholdObs::default().into());
            let displayed = effective_graph(&lib, &profile).unwrap().param(id, "value").unwrap().as_number();
            assert_eq!(trace.node(id).unwrap().outputs[0], Value::Number(displayed));
        }
    }
}
