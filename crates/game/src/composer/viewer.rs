//! The canvas: how a node draws, and what a connection is allowed to be.
//!
//! Two things here are load-bearing rather than cosmetic.
//!
//! **A wire is refused at the point of dragging, not reported afterwards.**
//! [`SnarlViewer::connect`] is the only place that decides, and it asks
//! [`behavior`] rather than reimplementing the rule. A scientist dragging a
//! number onto a condition gets nothing — no wire, no error list to go and
//! read — which is the whole benefit of a typed graph over a text file.
//!
//! **A pin's colour is its type.** Four colours, defined once in
//! `behavior::ValueType::colour`, used for the pin, the wire and the palette
//! entry. Nothing else on the canvas is coloured, so colour means exactly one
//! thing.

use std::collections::BTreeMap;

use bevy_egui::egui::{self, Color32};
use egui_snarl::{
    ui::{AnyPins, PinInfo, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};

use behavior::{registry, BehaviorGraph, Category, Domain, NodeSpec, ParamValue, ValueType};

use super::Composer;

/// A node as the editor holds it.
///
/// Only what the author can change: the identity, the type, the parameter
/// values, and their note. Ports, docs and evaluation all come from the
/// registry, so a node cannot get out of step with the code that runs it.
///
/// ### Why the id lives here
///
/// [`egui_snarl`] hands out its own `NodeId` on insert and cannot be asked to
/// keep the one a file carried. The first version of this editor used snarl's
/// id as the graph's id, which meant every load renumbered the graph — and
/// subtype overrides, which are keyed `<node id>.<param>`, silently stopped
/// matching anything. That was patched by remapping the overrides at load; it
/// is now fixed instead, by giving a node an identity of its own that a load, a
/// save and a rebuild all preserve.
///
/// The general shape, worth keeping in mind for anything else keyed on a node:
/// **an identity the editor is free to reassign is not an identity.** The live
/// execution view depends on this too — it matches trace node ids against
/// canvas nodes, and could not if the two renumbered independently.
#[derive(Debug, Clone)]
pub struct EditorNode {
    /// Stable across load, save and rebuild. What override keys, validator
    /// issues and execution traces all refer to.
    pub id: behavior::NodeId,
    pub type_id: String,
    pub params: BTreeMap<String, ParamValue>,
    pub comment: String,
}

impl EditorNode {
    pub fn new(id: behavior::NodeId, type_id: &str) -> EditorNode {
        let params = registry()
            .get(type_id)
            .map(|s| s.params.iter().map(|p| (p.name.to_string(), p.default_value())).collect())
            .unwrap_or_default();
        EditorNode { id, type_id: type_id.to_string(), params, comment: String::new() }
    }

    pub fn spec(&self) -> Option<&'static NodeSpec> {
        registry().get(&self.type_id)
    }
}

fn colour(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn pin_for(ty: ValueType) -> PinInfo {
    let base = match ty {
        // The shapes carry the type too, for anyone who cannot separate the
        // four hues. This is not a token gesture: two of them are a blue and a
        // purple.
        ValueType::Number => PinInfo::circle(),
        ValueType::Bool => PinInfo::square(),
        ValueType::Intent => PinInfo::triangle(),
        ValueType::Action => PinInfo::star(),
    };
    base.with_fill(colour(ty.colour()))
}

/// Everything the canvas needs that is not in the snarl itself.
pub struct Viewer<'a> {
    /// Nodes the validator has something to say about, and what.
    pub issues: &'a behavior::Report,
    /// Which kind of agent the open graph is about. The add-node menus filter
    /// on it for the same reason the palette does: a node from another domain
    /// cannot be placed, so offering it offers an error.
    pub domain: Domain,
    /// What the agent being watched is actually doing, when one is. `None`
    /// leaves the canvas as a plain editor.
    pub live: Option<&'a super::live::Frame>,
    /// Filled in by the canvas when the author asks for something the
    /// `Composer` has to act on — snarl owns `&mut Snarl` for the duration of
    /// `show`, so the composer cannot be borrowed at the same time.
    pub selected: Option<NodeId>,
    pub added: bool,
    /// Whether graph mutations are available. The Debug tab uses the same
    /// renderer for the applied graph, but it must never edit the composer's
    /// working copy (or even pretend that it can).
    pub editable: bool,
}

/// How a node relates to the decision the watched agent just took.
///
/// Four states, and they are not shades of one thing — a node that ran and said
/// "no" is a completely different report from a node nothing reached, and
/// drawing them the same way is the failure mode this whole view exists to
/// avoid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveRole {
    /// Fed the decision that was taken.
    Active,
    /// Ran and declined: a rule that was checked and did not apply.
    Withheld,
    /// Was on the path in a recent tick, but not this one.
    Recent,
    /// Ran, as every node does, but has never been on the path.
    Cold,
}

impl LiveRole {
    /// Tint for the node's header swatch, its pins and the panel's legend. One
    /// palette, so the legend cannot come to describe something else.
    pub fn colour(self) -> Color32 {
        match self {
            LiveRole::Active => Color32::from_rgb(0x7f, 0xd4, 0x92),
            LiveRole::Withheld => Color32::from_rgb(0x8a, 0x6a, 0x5c),
            LiveRole::Recent => Color32::from_rgb(0x54, 0x7c, 0x6a),
            LiveRole::Cold => Color32::from_rgb(0x55, 0x5a, 0x62),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LiveRole::Active => "on the path taken",
            LiveRole::Withheld => "checked, did not apply",
            LiveRole::Recent => "was on the path recently",
            LiveRole::Cold => "not on the path",
        }
    }

    /// How strongly the node reads. The whole point of the view is that the
    /// live path is the thing your eye lands on.
    fn dim(self) -> f32 {
        match self {
            LiveRole::Active => 1.0,
            LiveRole::Withheld => 0.75,
            LiveRole::Recent => 0.65,
            LiveRole::Cold => 0.45,
        }
    }

    /// Current-tick states. Both deserve their parameters on the canvas: the
    /// active path explains what won, while a withheld proposal explains the
    /// nearby rule that was checked and said no.
    fn is_current(self) -> bool {
        matches!(self, LiveRole::Active | LiveRole::Withheld)
    }
}

impl<'a> SnarlViewer<EditorNode> for Viewer<'a> {
    fn title(&mut self, node: &EditorNode) -> String {
        node.spec().map(|s| s.name.to_string()).unwrap_or_else(|| format!("? {}", node.type_id))
    }

    fn inputs(&mut self, node: &EditorNode) -> usize {
        node.spec().map(|s| s.inputs.len()).unwrap_or(0)
    }

    fn outputs(&mut self, node: &EditorNode) -> usize {
        node.spec().map(|s| s.outputs.len()).unwrap_or(0)
    }

    fn show_header(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) {
        let n = &snarl[node];
        let spec = n.spec();
        let title = spec.map(|s| s.name).unwrap_or("unknown node");
        let cat = spec.map(|s| s.category).unwrap_or(Category::Logic);
        let id = n.id;
        let role = self.live.map(|l| l.role(id));

        ui.horizontal(|ui| {
            // The category tint sits on a swatch rather than on the text, so
            // the label keeps the theme's contrast. While an agent is being
            // watched the swatch carries the execution role instead: colour
            // means one thing at a time, and in that mode the thing it means is
            // "did this box produce the answer".
            let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 14.0), egui::Sense::hover());
            let swatch = match role {
                Some(r) => r.colour(),
                None => colour(cat.colour()),
            };
            ui.painter().rect_filled(rect, 2.0, swatch);

            // A parameter node's own label is what the author named it; that
            // is far more use on the canvas than "Number".
            let label = n
                .params
                .get("label")
                .map(ParamValue::display)
                .filter(|s| !s.is_empty() && cat == Category::Parameter)
                .unwrap_or_else(|| title.to_string());
            let mut text = egui::RichText::new(label).strong();
            if let Some(r) = role {
                text = text.color(
                    ui.visuals().text_color().gamma_multiply(r.dim()),
                );
            }
            let r = ui.label(text);
            if let Some(s) = spec {
                let hover = match role {
                    Some(role) => format!("{}\n\n▸ {}", s.doc, role.label()),
                    None => s.doc.to_string(),
                };
                r.on_hover_text(hover);
            }

            let worst = self
                .issues
                .for_node(id)
                .map(|i| i.severity)
                .min();
            match worst {
                Some(behavior::Severity::Error) => {
                    ui.colored_label(Color32::from_rgb(0xe0, 0x6c, 0x5f), "✖")
                        .on_hover_text(node_issues(self.issues, id));
                }
                Some(behavior::Severity::Warning) => {
                    ui.colored_label(Color32::from_rgb(0xd8, 0xa6, 0x4b), "⚠")
                        .on_hover_text(node_issues(self.issues, id));
                }
                None => {}
            }

            // The winning proposal, marked on the box that made it. Without
            // this the slice tells you which branches contributed and not which
            // one of them was the answer.
            if self.live.map(|l| l.winner == Some(id)).unwrap_or(false) {
                ui.colored_label(LiveRole::Active.colour(), "▶").on_hover_text(
                    "This proposal won: it is what the agent is doing.",
                );
            }
        });

        if !snarl[node].comment.is_empty() {
            ui.small(snarl[node].comment.clone());
        }

        // What this node actually produced on the tick being watched, on the
        // box rather than in a list off to one side. Reading a graph and
        // reading its values in two places is most of why a dataflow graph is
        // hard to debug.
        if let Some(l) = self.live {
            if let Some(values) = l.values.get(&id) {
                let text = values.iter().map(behavior::Value::display).collect::<Vec<_>>().join("  ·  ");
                if !text.is_empty() {
                    ui.small(
                        egui::RichText::new(text)
                            .monospace()
                            .color(l.role(id).colour()),
                    );
                }
            }
        }
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) -> PinInfo {
        let Some(spec) = snarl[pin.id.node].spec() else {
            ui.label("?");
            return PinInfo::circle();
        };
        let Some(port) = spec.inputs.get(pin.id.input) else {
            ui.label("?");
            return PinInfo::circle();
        };
        // What is arriving on this port right now, next to its name. On an
        // input this is the value the node actually consumed, including the
        // port default when nothing is wired in — which is the case a reader
        // most often gets wrong.
        let live_text = self.live.and_then(|l| {
            let id = snarl[pin.id.node].id;
            l.inputs.get(&(id, pin.id.input as u16)).map(|v| {
                v.iter().map(behavior::Value::display).collect::<Vec<_>>().join(", ")
            })
        });
        let r = match &live_text {
            Some(v) if !v.is_empty() => {
                ui.label(format!("{}  {v}", port.name))
            }
            _ => ui.label(port.name),
        };
        r.on_hover_text(format!("{} ({})", port.doc, port.ty.label()));

        // An unconnected port that has a default is drawn hollow: the node
        // still evaluates, and the author should be able to tell at a glance
        // which inputs are actually carrying something.
        //
        // The shape always carries the type; only the fill is repurposed while
        // an agent is being watched, and that is the whole reason the shapes
        // exist rather than being a token gesture.
        let base = self
            .live
            .map(|l| l.role(snarl[pin.id.node].id).colour())
            .unwrap_or_else(|| colour(port.ty.colour()));
        let info = pin_for(port.ty).with_fill(base);
        if pin.remotes.is_empty() && port.default.is_some() {
            info.with_fill(base.gamma_multiply(0.35))
        } else {
            info
        }
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) -> PinInfo {
        let Some(spec) = snarl[pin.id.node].spec() else {
            ui.label("?");
            return PinInfo::circle();
        };
        let Some(port) = spec.outputs.get(pin.id.output) else {
            ui.label("?");
            return PinInfo::circle();
        };
        ui.label(port.name).on_hover_text(format!("{} ({})", port.doc, port.ty.label()));
        // A wire's colour is the mix of its two pins' fills, so colouring the
        // pins by their nodes' roles colours every wire by the pair it joins:
        // active to active is bright, and a live value arriving somewhere that
        // did not matter is visibly half of one.
        match self.live {
            Some(l) => pin_for(port.ty).with_fill(l.role(snarl[pin.id.node].id).colour()),
            None => pin_for(port.ty),
        }
    }

    fn has_footer(&mut self, node: &EditorNode) -> bool {
        let Some(live) = self.live else { return false };
        let Some(spec) = node.spec() else { return false };
        live.role(node.id).is_current()
            && !spec.params.is_empty()
            && live
                .trace
                .node(node.id)
                .is_some_and(|trace| !trace.params_read.is_empty())
    }

    fn show_footer(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) {
        let Some(live) = self.live else { return };
        let editor_node = &snarl[node];
        let Some(spec) = editor_node.spec() else { return };
        let Some(trace) = live.trace.node(editor_node.id) else { return };
        let role = live.role(editor_node.id);

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("USED PARAMETERS")
                    .small()
                    .color(role.colour()),
            );
            for index in &trace.params_read {
                let Some(param) = spec.params.get(*index as usize) else { continue };
                let value = live
                    .graph
                    .param(editor_node.id, param.name)
                    .unwrap_or_else(|| param.default_value());
                parameter_badge(ui, role, param, &value);
            }
        });
    }

    fn final_node_rect(
        &mut self,
        node: NodeId,
        ui_rect: egui::Rect,
        _graph_rect: egui::Rect,
        ui: &mut egui::Ui,
        scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) {
        let Some(live) = self.live else { return };
        let id = snarl[node].id;
        if live.role(id) != LiveRole::Active {
            return;
        }

        // A full outline is deliberately stronger than the header swatch. It
        // makes the current backward slice readable while zoomed out, where
        // labels and parameter badges no longer are.
        let winning = live.winner == Some(id);
        let width = if winning { 3.5 } else { 2.25 } * scale.clamp(0.75, 1.5);
        ui.painter().rect_stroke(
            ui_rect.expand(2.0 * scale.clamp(0.75, 1.5)),
            5.0 * scale,
            egui::Stroke::new(width, LiveRole::Active.colour()),
        );
    }

    /// The rule that makes the canvas trustworthy.
    ///
    /// Delegated to `behavior` rather than duplicated: the editor and the
    /// validator must agree, and the only way to guarantee that is for there
    /// to be one implementation.
    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<EditorNode>) {
        if !self.editable {
            return;
        }
        let (Some(fs), Some(ts)) = (snarl[from.id.node].spec(), snarl[to.id.node].spec()) else {
            return;
        };
        let (Some(op), Some(ip)) = (fs.outputs.get(from.id.output), ts.inputs.get(to.id.input))
        else {
            return;
        };
        if op.ty != ip.ty {
            return;
        }
        // A node feeding itself is the smallest possible cycle and the easiest
        // one to draw by accident.
        if from.id.node == to.id.node {
            return;
        }
        if !ip.multi {
            for remote in &to.remotes {
                snarl.disconnect(*remote, to.id);
            }
        }
        snarl.connect(from.id, to.id);
    }

    fn disconnect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<EditorNode>) {
        if self.editable {
            snarl.disconnect(from.id, to.id);
        }
    }

    fn drop_outputs(&mut self, pin: &OutPin, snarl: &mut Snarl<EditorNode>) {
        if self.editable {
            snarl.drop_outputs(pin.id);
        }
    }

    fn drop_inputs(&mut self, pin: &InPin, snarl: &mut Snarl<EditorNode>) {
        if self.editable {
            snarl.drop_inputs(pin.id);
        }
    }

    fn has_body(&mut self, node: &EditorNode) -> bool {
        // Only the nodes whose *whole content* is a constant get an inline
        // editor. Everything else is edited in the inspector, so a canvas of
        // forty nodes stays readable.
        self.editable
            && matches!(node.type_id.as_str(), "param.number" | "param.bool" | "param.intent")
    }

    fn show_body(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) {
        let Some(spec) = snarl[node].spec() else { return };
        let param = spec.params.iter().find(|p| p.name == "value" || p.name == "plan");
        let Some(param) = param.copied() else { return };
        let n = &mut snarl[node];
        let mut value = n.params.get(param.name).cloned().unwrap_or_else(|| param.default_value());
        if super::param_widget(ui, &param, &mut value) {
            n.params.insert(param.name.to_string(), value);
        }
    }

    fn has_node_menu(&mut self, _node: &EditorNode) -> bool {
        self.editable
    }

    fn show_node_menu(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) {
        if ui.button("Inspect").clicked() {
            self.selected = Some(node);
            ui.close_menu();
        }
        if ui.button("Duplicate").clicked() {
            let mut copy = snarl[node].clone();
            // A fresh identity, not the original's: two nodes sharing an id
            // would share every subtype override keyed on it, and the copy
            // would look like a second knob that moved the first one.
            copy.id = free_id(snarl);
            let pos = snarl
                .get_node_info(node)
                .map(|i| i.pos + egui::vec2(30.0, 30.0))
                .unwrap_or_default();
            let new = snarl.insert_node(pos, copy);
            self.selected = Some(new);
            self.added = true;
            ui.close_menu();
        }
        if ui.button("Delete").clicked() {
            snarl.remove_node(node);
            self.selected = None;
            self.added = true;
            ui.close_menu();
        }
    }

    fn has_on_hover_popup(&mut self, node: &EditorNode) -> bool {
        node.spec().is_some()
    }

    fn show_on_hover_popup(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) {
        let Some(spec) = snarl[node].spec() else { return };
        let id = snarl[node].id;
        ui.set_max_width(320.0);
        ui.strong(spec.name);
        ui.label(spec.doc);
        ui.small(format!("{}  ·  #{id}", spec.id));

        if !spec.params.is_empty() {
            ui.separator();
            ui.small(if self.live.is_some() {
                "Applied parameters · ● read this tick"
            } else {
                "Effective parameters"
            });
            let trace = self.live.and_then(|live| live.trace.node(id));
            for (index, param) in spec.params.iter().enumerate() {
                let value = self
                    .live
                    .and_then(|live| live.graph.param(id, param.name))
                    .or_else(|| snarl[node].params.get(param.name).cloned())
                    .unwrap_or_else(|| param.default_value());
                let text = format!("{} = {}", param.label, parameter_value(param, &value));
                let read = trace.is_some_and(|trace| trace.params_read.contains(&(index as u16)));
                if read {
                    let role = self.live.map(|live| live.role(id)).unwrap_or(LiveRole::Active);
                    ui.colored_label(role.colour(), format!("● {text}"));
                } else {
                    ui.weak(format!("  {text}"));
                }
            }
        }

        // While an agent is being watched, the popup is where the whole of what
        // this node just did lives: what came in, what went out, and whether it
        // mattered. The canvas can only afford a summary line.
        let Some(l) = self.live else { return };
        ui.separator();
        ui.colored_label(l.role(id).colour(), l.role(id).label());
        for (i, port) in spec.inputs.iter().enumerate() {
            let v = l
                .inputs
                .get(&(id, i as u16))
                .map(|v| v.iter().map(behavior::Value::display).collect::<Vec<_>>().join(", "))
                .unwrap_or_default();
            ui.small(format!("in  {} = {}", port.name, if v.is_empty() { "—" } else { &v }));
        }
        if let Some(values) = l.values.get(&id) {
            for (port, v) in spec.outputs.iter().zip(values) {
                ui.small(format!("out {} = {}", port.name, v.display()));
            }
        }
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<EditorNode>) -> bool {
        self.editable
    }

    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) {
        ui.label("Add node");
        ui.separator();
        for cat in Category::ALL {
            ui.menu_button(cat.label(), |ui| {
                for spec in registry().in_category_and_domain(cat, self.domain) {
                    if ui.button(spec.name).on_hover_text(spec.doc).clicked() {
                        let node = EditorNode::new(free_id(snarl), spec.id);
                        let id = snarl.insert_node(pos, node);
                        self.selected = Some(id);
                        self.added = true;
                        ui.close_menu();
                    }
                }
            });
        }
    }

    /// Dropping a wire on empty space offers only the nodes that could accept
    /// it. This is the fastest way to build a graph, and filtering it by type
    /// is what stops it becoming a second unfiltered palette.
    fn has_dropped_wire_menu(&mut self, _src: AnyPins, _snarl: &mut Snarl<EditorNode>) -> bool {
        self.editable
    }

    fn show_dropped_wire_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        _scale: f32,
        src_pins: AnyPins,
        snarl: &mut Snarl<EditorNode>,
    ) {
        let (want, from_output) = match src_pins {
            AnyPins::Out(pins) => (
                pins.first().and_then(|p| port_type(snarl, *p, true)),
                true,
            ),
            AnyPins::In(pins) => (
                pins.first().and_then(|p| in_port_type(snarl, *p)),
                false,
            ),
        };
        let Some(want) = want else { return };

        ui.label(format!("Connect this {} to…", want.label()));
        ui.separator();
        for spec in registry().in_domain(self.domain) {
            let ports = if from_output { spec.inputs } else { spec.outputs };
            let Some(port) = ports.iter().position(|p| p.ty == want) else { continue };
            if ui.button(spec.name).on_hover_text(spec.doc).clicked() {
                let node = EditorNode::new(free_id(snarl), spec.id);
                let new = snarl.insert_node(pos, node);
                match src_pins {
                    AnyPins::Out(pins) => {
                        for p in pins {
                            snarl.connect(*p, InPinId { node: new, input: port });
                        }
                    }
                    AnyPins::In(pins) => {
                        for p in pins {
                            snarl.connect(OutPinId { node: new, output: port }, *p);
                        }
                    }
                }
                self.selected = Some(new);
                self.added = true;
                ui.close_menu();
            }
        }
    }
}

/// Compact, unit-aware parameter value for the canvas. The inspector keeps
/// the full editing widget; this is a readout meant to survive a busy graph.
fn parameter_value(spec: &behavior::ParamSpec, value: &ParamValue) -> String {
    match (spec.kind, value) {
        (behavior::ParamKind::Number { unit, .. }, ParamValue::Number(n)) => {
            let mut number = format!("{n:.3}");
            while number.contains('.') && number.ends_with('0') {
                number.pop();
            }
            if number.ends_with('.') {
                number.pop();
            }
            format!("{number}{unit}")
        }
        (behavior::ParamKind::Bool { .. }, ParamValue::Bool(value)) => {
            if *value { "on".into() } else { "off".into() }
        }
        (behavior::ParamKind::Choice { .. }, ParamValue::Choice(value)) => super::pretty(value),
        (behavior::ParamKind::Text { .. }, ParamValue::Text(value)) => value.clone(),
        (_, value) => value.display(),
    }
}

fn parameter_badge(
    ui: &mut egui::Ui,
    role: LiveRole,
    spec: &behavior::ParamSpec,
    value: &ParamValue,
) {
    let colour = role.colour();
    egui::Frame::none()
        .fill(colour.gamma_multiply(0.12))
        .stroke(egui::Stroke::new(0.8, colour.gamma_multiply(0.8)))
        .rounding(3.0)
        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "{}  =  {}",
                        spec.label,
                        parameter_value(spec, value)
                    ))
                    .small(),
                )
                .wrap(false),
            )
            .on_hover_text(spec.doc);
        });
}

/// Materialise a behavior graph in the canvas representation.
///
/// Kept as a single conversion for both the editor and the live debugger so
/// stable behavior node ids, positions, and wires cannot drift between the
/// graph people author and the graph they inspect while it runs.
pub fn snarl_from_graph(graph: &BehaviorGraph) -> Snarl<EditorNode> {
    let mut snarl = Snarl::new();
    let mut map: BTreeMap<behavior::NodeId, NodeId> = BTreeMap::new();
    for node in &graph.nodes {
        let sid = snarl.insert_node(
            egui::pos2(node.pos[0], node.pos[1]),
            EditorNode {
                id: node.id,
                type_id: node.type_id.clone(),
                params: node.params.clone(),
                comment: node.comment.clone(),
            },
        );
        map.insert(node.id, sid);
    }
    for wire in &graph.wires {
        let (Some(&from), Some(&to)) = (map.get(&wire.from_node), map.get(&wire.to_node)) else {
            continue;
        };
        snarl.connect(
            OutPinId { node: from, output: wire.from_port as usize },
            InPinId { node: to, input: wire.to_port as usize },
        );
    }
    snarl
}

fn port_type(snarl: &Snarl<EditorNode>, pin: OutPinId, _out: bool) -> Option<ValueType> {
    snarl.get_node(pin.node)?.spec()?.outputs.get(pin.output).map(|p| p.ty)
}

fn in_port_type(snarl: &Snarl<EditorNode>, pin: InPinId) -> Option<ValueType> {
    snarl.get_node(pin.node)?.spec()?.inputs.get(pin.input).map(|p| p.ty)
}

fn node_issues(report: &behavior::Report, node: behavior::NodeId) -> String {
    report.for_node(node).map(|i| i.message.clone()).collect::<Vec<_>>().join("\n")
}

/// The next unused node identity on this canvas.
///
/// Linear over the nodes, which is nothing at the scale these graphs are, and
/// correct after a delete: reusing the id of a node that was removed would hand
/// the new node every subtype override the old one had.
pub fn free_id(snarl: &Snarl<EditorNode>) -> behavior::NodeId {
    snarl.nodes().map(|n| n.id).max().map_or(1, |m| m.saturating_add(1))
}

/// Draw the canvas and fold whatever it did back into the composer.
pub fn canvas(ui: &mut egui::Ui, c: &mut Composer) {
    // The viewer borrows the report and the live frame, so both have to be
    // taken out of the composer before snarl takes `&mut`.
    let report = std::mem::take(&mut c.report);
    let frame = c.live.frame.take();
    let mut viewer = Viewer {
        issues: &report,
        domain: c.domain(),
        live: frame.as_ref().filter(|f| f.graph_id == c.graph_id),
        selected: None,
        added: false,
        editable: true,
    };
    let style = super::editor_style();
    c.snarl.show(&mut viewer, &style, "behaviour-canvas", ui);
    let (selected, added) = (viewer.selected, viewer.added);
    c.report = report;
    c.live.frame = frame;

    if let Some(n) = selected {
        c.selected = Some(n);
        c.right = super::RightTab::Inspector;
    }
    if added {
        c.dirty = true;
    }

    // Validation is live, so the strip below and the marks on the nodes are
    // never a frame behind what the author just did. The graphs are tens of
    // nodes; this is microseconds.
    c.sync();
}

/// Draw the exact graph evaluated for the selected entity.
///
/// A fresh snarl is projected from the captured graph every frame. View state
/// (pan/zoom/collapse) belongs to egui's persistent id, while any accidental
/// drag is discarded immediately. Together with the disabled menus and pin
/// editing this makes the debug canvas observational only.
pub fn debug_canvas(ui: &mut egui::Ui, c: &mut Composer) {
    let frame = c.live.frame.take();
    let Some(frame) = frame else {
        ui.centered_and_justified(|ui| {
            ui.weak("Select a person, household, or unit to see its applied behavior graph.");
        });
        return;
    };

    ui.horizontal(|ui| {
        ui.strong(format!("Applied graph: {}", frame.graph.name));
        ui.weak("scroll to zoom · drag background to pan");
    });
    ui.separator();

    let report = behavior::validate(&frame.graph);
    let mut snarl = snarl_from_graph(&frame.graph);
    let mut viewer = Viewer {
        issues: &report,
        domain: frame.graph.domain,
        live: Some(&frame),
        selected: None,
        added: false,
        editable: false,
    };
    let style = super::editor_style();
    snarl.show(&mut viewer, &style, "live-debug-canvas", ui);
    c.live.frame = Some(frame);
}
