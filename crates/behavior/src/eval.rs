//! Compiling a graph once, then running it a thousand times a tick.
//!
//! The decision layer in `abm` evaluates one graph per household every five
//! simulated seconds. At 750 households and 512x that is a few hundred
//! thousand evaluations a minute, so the split here is deliberate: everything
//! that can be resolved from the graph alone — topological order, port
//! lookups, parameter values after subtype overrides, whether a port is
//! multi-connection — is resolved once at [`CompiledGraph::compile`], and the
//! evaluation itself does nothing but index into vectors.
//!
//! Tracing is a separate entry point rather than a flag, so the hot path
//! carries no branch for it and the test bench gets the full picture.

use std::collections::BTreeMap;

use crate::domain::Domain;
use crate::graph::{BehaviorGraph, NodeId};
use crate::node::{registry, EvalCtx, Inputs, NodeSpec, ParamValue, Params};
use crate::nodes;
use crate::observation::Observation;
use crate::validate::{validate, Report};
use crate::value::{ActionKind, Value};

/// Parameter overrides, keyed `"<node id>.<param name>"`.
///
/// This is how one graph serves several subtypes. See
/// [`BehaviorGraph::override_key`].
pub type Overrides = BTreeMap<String, ParamValue>;

struct CompiledNode {
    id: NodeId,
    spec: &'static NodeSpec,
    params: Vec<ParamValue>,
    /// Per input port, the compiled-index/port pairs feeding it, in wire order.
    sources: Vec<Vec<(u32, u16)>>,
}

/// A graph resolved against the registry and a set of overrides, ready to run.
pub struct CompiledGraph {
    pub graph_id: String,
    pub domain: Domain,
    nodes: Vec<CompiledNode>,
    /// Compiled index of the single `out.decision` node.
    decision: usize,
    prep_scale: Option<usize>,
    urgency: Option<usize>,
    /// Widest output count, so the scratch buffers are sized once.
    max_out: usize,
}

/// What the model reads back out of one evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decision {
    pub action: ActionKind,
    /// Priority of the winning proposal. Surfaced so the inspector can say
    /// *how strongly* a household wants to go, not just that it does.
    pub priority: f32,
    /// Multiplier on the household's baked milling time. 1.0 leaves it alone;
    /// below 1 is a household that drops what it is doing.
    pub prep_scale: f32,
    /// 0–1, for the HUD and the debrief. Has no mechanical effect.
    pub urgency: f32,
}

impl Default for Decision {
    fn default() -> Self {
        Decision { action: ActionKind::Stay, priority: 0.0, prep_scale: 1.0, urgency: 0.0 }
    }
}

/// One node's values from a traced evaluation.
#[derive(Debug, Clone)]
pub struct NodeTrace {
    pub node: NodeId,
    pub type_id: &'static str,
    pub name: &'static str,
    pub inputs: Vec<Vec<Value>>,
    pub outputs: Vec<Value>,
}

/// A full record of one evaluation, for the test bench.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    /// In evaluation order, which is also the order to read them in.
    pub nodes: Vec<NodeTrace>,
    /// Every action proposal that fired, strongest first.
    pub proposals: Vec<(NodeId, ActionKind, f32)>,
    pub decision: Decision,
}

impl Trace {
    pub fn node(&self, id: NodeId) -> Option<&NodeTrace> {
        self.nodes.iter().find(|n| n.node == id)
    }
}

impl CompiledGraph {
    /// Resolve `graph` against the registry, baking in `overrides`.
    ///
    /// Fails with the same [`Report`] the editor shows, so a graph that will
    /// not compile has already told the author why before they try to run it.
    pub fn compile(graph: &BehaviorGraph, overrides: &Overrides) -> Result<CompiledGraph, Report> {
        let report = validate(graph);
        if !report.ok() {
            return Err(report);
        }

        // `report.order` indexes `graph.nodes`; the compiled node list is in
        // that order, so a node's sources always sit before it.
        let mut pos_of: BTreeMap<NodeId, u32> = BTreeMap::new();
        for (compiled_i, gi) in report.order.iter().enumerate() {
            pos_of.insert(graph.nodes[*gi].id, compiled_i as u32);
        }

        let reg = registry();
        let mut nodes = Vec::with_capacity(report.order.len());
        let mut max_out = 1;

        for gi in &report.order {
            let gn = &graph.nodes[*gi];
            let spec = reg.get(&gn.type_id).expect("validated");
            max_out = max_out.max(spec.outputs.len());

            let params = spec
                .params
                .iter()
                .map(|p| {
                    let key = BehaviorGraph::override_key(gn.id, p.name);
                    overrides
                        .get(&key)
                        .cloned()
                        .or_else(|| gn.params.get(p.name).cloned())
                        .filter(|v| v.same_kind(&p.default_value()))
                        .unwrap_or_else(|| p.default_value())
                })
                .collect();

            let mut sources = vec![Vec::new(); spec.inputs.len()];
            for w in graph.wires.iter().filter(|w| w.to_node == gn.id) {
                let Some(&src) = pos_of.get(&w.from_node) else { continue };
                if let Some(slot) = sources.get_mut(w.to_port as usize) {
                    slot.push((src, w.from_port));
                }
            }

            nodes.push(CompiledNode { id: gn.id, spec, params, sources });
        }

        let find = |type_id: &str| {
            nodes.iter().position(|n| n.spec.id == type_id)
        };
        let decision = find(graph.domain.decision_output()).expect("validated");

        Ok(CompiledGraph {
            graph_id: graph.id.clone(),
            domain: graph.domain,
            decision,
            prep_scale: find(nodes::PREP_SCALE_OUTPUT),
            urgency: find(nodes::URGENCY_OUTPUT),
            nodes,
            max_out,
        })
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Run the graph. Allocates one buffer per call; use
    /// [`CompiledGraph::eval_with`] on the model's hot path.
    pub fn eval(&self, obs: &Observation) -> Decision {
        let mut scratch = Scratch::new(self);
        self.eval_with(obs, &mut scratch)
    }

    /// Run the graph reusing caller-owned scratch space.
    pub fn eval_with(&self, obs: &Observation, scratch: &mut Scratch) -> Decision {
        self.run(obs, scratch, None)
    }

    /// Run the graph and record every intermediate value.
    pub fn eval_traced(&self, obs: &Observation) -> (Decision, Trace) {
        let mut scratch = Scratch::new(self);
        let mut trace = Trace::default();
        let d = self.run(obs, &mut scratch, Some(&mut trace));
        trace.decision = d;
        trace.proposals.sort_by(|a, b| b.2.total_cmp(&a.2));
        (d, trace)
    }

    fn run(&self, obs: &Observation, s: &mut Scratch, mut trace: Option<&mut Trace>) -> Decision {
        let ctx = EvalCtx { obs };
        s.reset(self.nodes.len());

        for (i, n) in self.nodes.iter().enumerate() {
            // Gather this node's inputs. `slots` is cleared and refilled per
            // node rather than held per edge: the graphs are tens of nodes,
            // and one contiguous buffer beats a map every time.
            s.slots.clear();
            for src in &n.sources {
                let mut slot = Vec::with_capacity(src.len());
                for (from, port) in src {
                    if let Some(v) = s.outputs[*from as usize].get(*port as usize) {
                        slot.push(*v);
                    }
                }
                s.slots.push(slot);
            }

            let inputs = Inputs { slots: &s.slots, ports: n.spec.inputs };
            let params = Params { values: &n.params };
            let out = &mut s.outputs[i];
            out.clear();
            (n.spec.eval)(&ctx, &params, &inputs, out);
            // A node that under-produces would silently shift every downstream
            // port index, so pad rather than trust.
            while out.len() < n.spec.outputs.len() {
                let p = n.spec.outputs[out.len()];
                out.push(p.default.unwrap_or(Value::Number(0.0)));
            }

            if let Some(t) = trace.as_deref_mut() {
                if n.spec.category == crate::node::Category::Action {
                    if let Some(Value::Action(a)) = out.first() {
                        if a.fired {
                            t.proposals.push((n.id, a.kind, a.priority));
                        }
                    }
                }
                t.nodes.push(NodeTrace {
                    node: n.id,
                    type_id: n.spec.id,
                    name: n.spec.name,
                    inputs: s.slots.clone(),
                    outputs: out.clone(),
                });
            }
        }

        let chosen = s.outputs[self.decision]
            .first()
            .map(|v| v.as_action())
            .unwrap_or_else(|| crate::value::ActionProposal {
                kind: self.domain.default_action(),
                priority: 0.0,
                fired: false,
            });
        Decision {
            action: chosen.kind,
            priority: chosen.priority,
            prep_scale: self
                .prep_scale
                .and_then(|i| s.outputs[i].first().copied())
                .map(|v| v.as_number())
                .unwrap_or(1.0),
            urgency: self
                .urgency
                .and_then(|i| s.outputs[i].first().copied())
                .map(|v| v.as_number())
                .unwrap_or(0.0),
        }
    }
}

/// Reusable buffers for [`CompiledGraph::eval_with`].
pub struct Scratch {
    outputs: Vec<Vec<Value>>,
    slots: Vec<Vec<Value>>,
}

impl Scratch {
    pub fn new(g: &CompiledGraph) -> Scratch {
        Scratch {
            outputs: vec![Vec::with_capacity(g.max_out); g.nodes.len()],
            slots: Vec::with_capacity(8),
        }
    }

    fn reset(&mut self, n: usize) {
        if self.outputs.len() < n {
            self.outputs.resize_with(n, Vec::new);
        }
        for o in &mut self.outputs[..n] {
            o.clear();
        }
    }
}

