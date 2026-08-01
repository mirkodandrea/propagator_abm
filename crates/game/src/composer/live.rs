//! Watching one agent's behaviour run.
//!
//! Select a household, a person or a unit on the map and this points the editor
//! at the graph that agent is actually running, then keeps it in step with the
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

use behavior::{ActionKind, Decision, NodeId, Trace, Value};

use super::viewer::LiveRole;
use super::{Composer, RightTab};
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
    /// The agent being watched, if any.
    pub subject: Option<Subject>,
    /// The most recent capture. Taken out of the composer while the canvas
    /// draws, and put back after.
    pub frame: Option<Frame>,
    /// Newest last, capped at [`HISTORY`].
    pub history: VecDeque<Recorded>,
    /// Follow the selection: switch the canvas to whatever the selected agent
    /// is running. On by default, because a live view of a graph you are not
    /// looking at is not a live view.
    pub follow: bool,
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
    /// The default the editor opens with: a live view of a graph you are not
    /// looking at is not a live view, so following the selection is on.
    pub fn following() -> Live {
        Live { follow: true, ..Live::default() }
    }

    pub fn watching(&self) -> bool {
        self.subject.is_some() && self.frame.is_some()
    }

    /// Drop everything about the agent that was being watched.
    ///
    /// Called whenever the subject changes. Keeping the history across a switch
    /// would shade the new agent's graph with the old one's path, which reads
    /// exactly like the new agent having been somewhere it has never been.
    fn forget(&mut self) {
        self.frame = None;
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
    subtype_id: String,
    subtype_name: String,
    agent: String,
    decision: Decision,
    trace: Trace,
}

/// Ask the model to explain one agent, whichever kind it is.
///
/// Returns `None` for an agent running a hand-written layer, which is not a
/// failure: it is the default, and the panel says so rather than showing an
/// empty graph.
fn explain(sim: &Sim, target: Target) -> Option<Capture> {
    let lib = sim.behaviour.as_ref()?;
    let graph_of = |subtype: &str| lib.subtypes.get(subtype).map(|s| s.graph.clone());

    match target {
        Target::Household(id) => {
            let (sid, name, _) = sim.agents.behaviour_of(id)?;
            let (sid, name) = (sid.to_string(), name.to_string());
            let (decision, trace) = sim.agents.explain(id, &sim.fire)?;
            Some(Capture {
                graph_id: graph_of(&sid)?,
                subtype_id: sid,
                subtype_name: name,
                agent: format!("Household #{id}"),
                decision,
                trace,
            })
        }
        Target::Person(id) => {
            let (sid, name, _) = sim.agents.person_behaviour_of(id)?;
            let (sid, name) = (sid.to_string(), name.to_string());
            // A person who is at home has no behaviour of their own — the
            // household is the agent — and saying so is more use than an empty
            // graph.
            if !sim.agents.people.get(id)?.away {
                return None;
            }
            let (decision, trace) = sim.agents.explain_person(id, &sim.fire)?;
            Some(Capture {
                graph_id: graph_of(&sid)?,
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
            Some(Capture {
                graph_id: graph_of(&sid)?,
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

    let graph_id = cap.graph_id.clone();
    let subtype_id = cap.subtype_id.clone();
    c.live.frame = Some(Frame {
        graph_id: cap.graph_id,
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

    // Point the editor at what this agent is running. Committed first, so an
    // edit in progress goes into the library rather than being thrown away by
    // the load — but the dirty flag is put back afterwards, because following
    // the selection is not an edit and marking the library unsaved for clicking
    // around the map would make the warning meaningless.
    if c.live.follow && c.open && c.graph_id != graph_id && c.lib.graphs.contains_key(&graph_id) {
        let was_dirty = c.dirty;
        c.commit();
        c.load_graph(&graph_id);
        c.dirty = was_dirty;
        c.subtype = Some(subtype_id);
    }
}

// ---------------------------------------------------------------------------
// The panel
// ---------------------------------------------------------------------------

pub fn panel(ui: &mut egui::Ui, c: &mut Composer) {
    ui.horizontal(|ui| {
        ui.heading("Live");
        ui.checkbox(&mut c.live.follow, "follow selection").on_hover_text(
            "Switch the canvas to whatever the selected agent is running. Off keeps the \
             behaviour you are editing on screen while the highlight follows the agent — \
             which shows nothing at all unless they happen to be the same graph.",
        );
    });

    let Some(subject) = c.live.subject else {
        ui.separator();
        ui.label("Nothing is selected.");
        ui.small(
            "Click a person, a household or a unit on the map. Their behaviour opens here and \
             stays in step with the incident: what every node produced, and which of them \
             produced the decision.",
        );
        return;
    };

    // Taken out for the body and put back at the end: the panel reads the frame
    // and writes to the composer at the same time, and the frame is the one
    // thing here that is genuinely large enough to be worth not cloning.
    let frame = c.live.frame.take();
    let Some(frame) = frame else {
        ui.separator();
        ui.label(match subject.target {
            Target::Unit(_) => "This unit is running the hand-written policy.",
            Target::Person(_) => "This person is not running an authored behaviour.",
            _ => "This agent is running the hand-written decision layer.",
        });
        ui.small(
            "Give a profile a share in the Profiles tab and press \"Apply and restart\", and \
             its graph appears here as the incident runs.",
        );
        return;
    };

    ui.separator();
    ui.horizontal(|ui| {
        ui.strong(&frame.agent);
        ui.small(&frame.subtype_name).on_hover_text(&frame.subtype_id);
    });

    if frame.graph_id != c.graph_id {
        ui.colored_label(
            egui::Color32::from_rgb(0xd8, 0xa6, 0x4b),
            "The canvas is showing a different behaviour.",
        );
        let want = frame.graph_id.clone();
        if ui.button(format!("Open \"{want}\"")).clicked() {
            c.commit();
            c.load_graph(&want);
        }
        c.live.frame = Some(frame);
        return;
    }

    if frame.stale {
        ui.colored_label(
            egui::Color32::from_rgb(0xd8, 0xa6, 0x4b),
            "Edited since the incident was started.",
        )
        .on_hover_text(
            "The highlight describes the behaviour as applied. Anything you have changed \
             since is not what is running — press \"Apply and restart\" to make it so.",
        );
    }

    // --- what it decided ----------------------------------------------------
    ui.add_space(4.0);
    let d = frame.decision;
    ui.horizontal(|ui| {
        ui.colored_label(super::bench::action_colour(d.action), d.action.label());
        ui.small(format!("priority {:.2}", d.priority));
    });
    if d.prep_scale != 1.0 {
        ui.small(format!("preparation ×{:.2}", d.prep_scale));
    }
    if d.urgency > 0.0 {
        ui.small(format!("urgency readout {:.2}", d.urgency));
    }

    transport(ui, c);

    // --- the legend ---------------------------------------------------------
    // Not decoration: four states in two greens is unreadable without it, and
    // the difference between "checked and declined" and "never reached" is the
    // single most useful thing on the canvas.
    ui.separator();
    egui::CollapsingHeader::new("What the colours mean").default_open(false).show(ui, |ui| {
        for role in [LiveRole::Active, LiveRole::Withheld, LiveRole::Recent, LiveRole::Cold] {
            ui.horizontal(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, role_colour(role));
                ui.small(role.label());
            });
        }
        ui.small(
            "Every node runs on every tick — this is a dataflow graph, not a flowchart. \
             \"On the path taken\" means the node fed a value into the decision that won.",
        );
    });

    // --- the proposals ------------------------------------------------------
    ui.separator();
    ui.label("Proposals");
    if frame.trace.proposals.is_empty() {
        ui.small("Nothing fired: the agent is doing its default.");
    }
    let mut jump: Option<NodeId> = None;
    for (i, (node, kind, prio)) in frame.trace.proposals.iter().enumerate() {
        let text = format!("{} {} @ {prio:.2}", if i == 0 { "▶" } else { " " }, kind.label());
        if ui
            .add(egui::Label::new(egui::RichText::new(text).color(super::bench::action_colour(*kind))).sense(egui::Sense::click()))
            .on_hover_text("Select the node that made this proposal")
            .clicked()
        {
            jump = Some(*node);
        }
    }

    // --- every node, in evaluation order ------------------------------------
    ui.separator();
    ui.label("Node by node");
    egui::ScrollArea::vertical().max_height(240.0).id_source("live-trace").show(ui, |ui| {
        for n in &frame.trace.nodes {
            let role = frame.role(n.node);
            let values =
                n.outputs.iter().map(Value::display).collect::<Vec<_>>().join(", ");
            let r = ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("{} = {values}", n.name))
                        .color(role_colour(role)),
                )
                .sense(egui::Sense::click()),
            );
            if r.on_hover_text(role.label()).clicked() {
                jump = Some(n.node);
            }
        }
    });

    // --- what it has been doing ---------------------------------------------
    ui.separator();
    egui::CollapsingHeader::new(format!("History ({})", c.live.history.len()))
        .default_open(false)
        .show(ui, |ui| {
            if c.live.history.len() < 2 {
                ui.small("Nothing yet. Step or run the incident and the decisions land here.");
            }
            // Newest first: what it is doing now is the question, and what it
            // was doing forty ticks ago is the follow-up.
            let mut last: Option<ActionKind> = None;
            for r in c.live.history.iter().rev() {
                // Only the changes. A list of two hundred identical rows hides
                // the three moments that matter.
                if last == Some(r.action) {
                    continue;
                }
                last = Some(r.action);
                ui.small(
                    egui::RichText::new(format!(
                        "T+{:02}:{:02}  {} @ {:.2}",
                        r.time_s / 60,
                        r.time_s % 60,
                        r.action.label(),
                        r.priority
                    ))
                    .color(super::bench::action_colour(r.action)),
                );
            }
        });

    if let Some(node) = jump {
        if let Some(sid) = c.snarl_id_of(node) {
            c.selected = Some(sid);
            c.right = RightTab::Inspector;
        }
    }
    c.live.frame = Some(frame);
}

/// Play, pause and step, in the panel that needs them.
///
/// Duplicated from the menu bar's status strip on purpose: someone reading a
/// behaviour tick by tick should not have to go to the other end of the screen
/// and back for every step, and the composer window covers the strip anyway.
fn transport(ui: &mut egui::Ui, c: &mut Composer) {
    ui.horizontal(|ui| {
        let (glyph, hint) = if c.live.playing {
            ("⏸", "Pause the incident")
        } else {
            ("▶", "Run the incident")
        };
        if ui.button(glyph).on_hover_text(hint).clicked() {
            c.live.toggle_play = true;
        }
        if ui
            .button("⏭")
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

fn role_colour(role: LiveRole) -> egui::Color32 {
    // The viewer owns the palette; the panel must not invent a second one, or
    // the legend stops describing the canvas.
    role.colour()
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
