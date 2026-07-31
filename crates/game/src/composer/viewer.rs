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

use behavior::{registry, Category, NodeSpec, ParamValue, ValueType};

use super::Composer;

/// A node as the editor holds it.
///
/// Only what the author can change: the type, the parameter values, and their
/// note. Ports, docs and evaluation all come from the registry, so a node
/// cannot get out of step with the code that runs it.
#[derive(Debug, Clone)]
pub struct EditorNode {
    pub type_id: String,
    pub params: BTreeMap<String, ParamValue>,
    pub comment: String,
}

impl EditorNode {
    pub fn new(type_id: &str) -> EditorNode {
        let params = registry()
            .get(type_id)
            .map(|s| s.params.iter().map(|p| (p.name.to_string(), p.default_value())).collect())
            .unwrap_or_default();
        EditorNode { type_id: type_id.to_string(), params, comment: String::new() }
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
    /// Filled in by the canvas when the author asks for something the
    /// `Composer` has to act on — snarl owns `&mut Snarl` for the duration of
    /// `show`, so the composer cannot be borrowed at the same time.
    pub selected: Option<NodeId>,
    pub added: bool,
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

        ui.horizontal(|ui| {
            // The category tint sits on a swatch rather than on the text, so
            // the label keeps the theme's contrast.
            let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, colour(cat.colour()));

            // A parameter node's own label is what the author named it; that
            // is far more use on the canvas than "Number".
            let label = n
                .params
                .get("label")
                .map(ParamValue::display)
                .filter(|s| !s.is_empty() && cat == Category::Parameter)
                .unwrap_or_else(|| title.to_string());
            let r = ui.label(egui::RichText::new(label).strong());
            if let Some(s) = spec {
                r.on_hover_text(s.doc);
            }

            let id = node.0 as behavior::NodeId;
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
        });

        if !snarl[node].comment.is_empty() {
            ui.small(snarl[node].comment.clone());
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
        let r = ui.label(port.name);
        r.on_hover_text(format!("{} ({})", port.doc, port.ty.label()));
        // An unconnected port that has a default is drawn hollow: the node
        // still evaluates, and the author should be able to tell at a glance
        // which inputs are actually carrying something.
        let info = pin_for(port.ty);
        if pin.remotes.is_empty() && port.default.is_some() {
            info.with_fill(colour(port.ty.colour()).gamma_multiply(0.35))
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
        pin_for(port.ty)
    }

    /// The rule that makes the canvas trustworthy.
    ///
    /// Delegated to `behavior` rather than duplicated: the editor and the
    /// validator must agree, and the only way to guarantee that is for there
    /// to be one implementation.
    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<EditorNode>) {
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

    fn has_body(&mut self, node: &EditorNode) -> bool {
        // Only the nodes whose *whole content* is a constant get an inline
        // editor. Everything else is edited in the inspector, so a canvas of
        // forty nodes stays readable.
        matches!(node.type_id.as_str(), "param.number" | "param.bool" | "param.intent")
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
        true
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
            let copy = snarl[node].clone();
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
        ui.set_max_width(320.0);
        ui.strong(spec.name);
        ui.label(spec.doc);
        ui.small(format!("{}  ·  #{}", spec.id, node.0));
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<EditorNode>) -> bool {
        true
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
                for spec in registry().in_category(cat) {
                    if ui.button(spec.name).on_hover_text(spec.doc).clicked() {
                        let id = snarl.insert_node(pos, EditorNode::new(spec.id));
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
        true
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
        for spec in registry().all() {
            let ports = if from_output { spec.inputs } else { spec.outputs };
            let Some(port) = ports.iter().position(|p| p.ty == want) else { continue };
            if ui.button(spec.name).on_hover_text(spec.doc).clicked() {
                let new = snarl.insert_node(pos, EditorNode::new(spec.id));
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

fn port_type(snarl: &Snarl<EditorNode>, pin: OutPinId, _out: bool) -> Option<ValueType> {
    snarl.get_node(pin.node)?.spec()?.outputs.get(pin.output).map(|p| p.ty)
}

fn in_port_type(snarl: &Snarl<EditorNode>, pin: InPinId) -> Option<ValueType> {
    snarl.get_node(pin.node)?.spec()?.inputs.get(pin.input).map(|p| p.ty)
}

fn node_issues(report: &behavior::Report, node: behavior::NodeId) -> String {
    report.for_node(node).map(|i| i.message.clone()).collect::<Vec<_>>().join("\n")
}

/// Draw the canvas and fold whatever it did back into the composer.
pub fn canvas(ui: &mut egui::Ui, c: &mut Composer) {
    // The viewer borrows the report, so the projection has to be taken out of
    // the composer before snarl takes `&mut`.
    let report = std::mem::take(&mut c.report);
    let mut viewer = Viewer { issues: &report, selected: None, added: false };
    let style = super::editor_style();
    c.snarl.show(&mut viewer, &style, "behaviour-canvas", ui);
    let (selected, added) = (viewer.selected, viewer.added);
    c.report = report;

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
