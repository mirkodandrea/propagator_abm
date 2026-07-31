//! The saved form of a behaviour: nodes, wires, and where the boxes sit.
//!
//! This is a file format. It is plain JSON with no version-specific magic in
//! it, node types are referred to by their dotted string id, and node ids are
//! small integers the author never sees. A graph that references a node type
//! this build does not have still loads — it fails *validation*, with the
//! missing type named, rather than failing to parse.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::Domain;
use crate::node::{registry, NodeSpec, ParamValue};

/// Identifies a node inside one graph. Small and dense, so the compiler can
/// index by it.
pub type NodeId = u32;

/// One placed node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: NodeId,
    /// Registry id, e.g. `"obs.threat"`.
    #[serde(rename = "type")]
    pub type_id: String,
    /// Editor position. Round-tripped so a saved graph opens laid out the way
    /// it was left; it has no effect on evaluation.
    #[serde(default)]
    pub pos: [f32; 2],
    /// Parameter values, by name. Sparse: anything absent takes the spec's
    /// default, so adding a parameter to an existing node type does not
    /// invalidate saved graphs.
    #[serde(default)]
    pub params: BTreeMap<String, ParamValue>,
    /// Author's note, shown on hover in the editor.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
}

impl GraphNode {
    pub fn spec(&self) -> Option<&'static NodeSpec> {
        registry().get(&self.type_id)
    }
}

/// One connection, from an output port to an input port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wire {
    pub from_node: NodeId,
    pub from_port: u16,
    pub to_node: NodeId,
    pub to_port: u16,
}

/// A whole authored behaviour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BehaviorGraph {
    /// Stable identifier, referenced by subtypes. Kebab-case by convention.
    pub id: String,
    pub name: String,
    /// Which kind of agent this behaviour is about.
    ///
    /// Defaulted rather than required so every graph written before domains
    /// existed loads as what it was: a household behaviour.
    #[serde(default)]
    pub domain: Domain,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub wires: Vec<Wire>,
}

impl BehaviorGraph {
    /// A new household behaviour. Use [`BehaviorGraph::new_in`] for any other
    /// domain.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> BehaviorGraph {
        BehaviorGraph::new_in(Domain::Household, id, name)
    }

    pub fn new_in(
        domain: Domain,
        id: impl Into<String>,
        name: impl Into<String>,
    ) -> BehaviorGraph {
        BehaviorGraph {
            id: id.into(),
            name: name.into(),
            domain,
            description: String::new(),
            nodes: Vec::new(),
            wires: Vec::new(),
        }
    }

    pub fn node(&self, id: NodeId) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut GraphNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Smallest unused node id. Ids are never reused within a session, which
    /// keeps subtype overrides keyed on them stable across an edit.
    pub fn next_id(&self) -> NodeId {
        self.nodes.iter().map(|n| n.id).max().map(|m| m + 1).unwrap_or(0)
    }

    /// Add a node of `type_id` at `pos`, with the spec's default parameters.
    ///
    /// Refuses a node belonging to another domain. The editor's palette already
    /// filters those out, so this is the guard for a graph built in code and
    /// the reason the default library cannot be assembled wrong.
    pub fn add(&mut self, type_id: &str, pos: [f32; 2]) -> Option<NodeId> {
        let spec = registry().get(type_id)?;
        if !spec.allowed_in(self.domain) {
            return None;
        }
        let id = self.next_id();
        let params = spec
            .params
            .iter()
            .map(|p| (p.name.to_string(), p.default_value()))
            .collect();
        self.nodes.push(GraphNode {
            id,
            type_id: type_id.to_string(),
            pos,
            params,
            comment: String::new(),
        });
        Some(id)
    }

    pub fn remove(&mut self, id: NodeId) {
        self.nodes.retain(|n| n.id != id);
        self.wires.retain(|w| w.from_node != id && w.to_node != id);
    }

    /// Connect two ports, replacing whatever was on the destination unless it
    /// is a multi-connection input. Returns false if the connection is not
    /// type-compatible or would not refer to real ports.
    pub fn connect(&mut self, w: Wire) -> bool {
        let Some(from) = self.node(w.from_node).and_then(GraphNode::spec) else { return false };
        let Some(to) = self.node(w.to_node).and_then(GraphNode::spec) else { return false };
        let Some(op) = from.outputs.get(w.from_port as usize) else { return false };
        let Some(ip) = to.inputs.get(w.to_port as usize) else { return false };
        if op.ty != ip.ty {
            return false;
        }
        if !ip.multi {
            self.wires.retain(|e| !(e.to_node == w.to_node && e.to_port == w.to_port));
        } else if self.wires.contains(&w) {
            return false;
        }
        self.wires.push(w);
        true
    }

    pub fn disconnect(&mut self, w: Wire) {
        self.wires.retain(|e| *e != w);
    }

    /// The parameter value actually in force for a node, taking the graph's
    /// own value and then the spec default.
    pub fn param(&self, node: NodeId, name: &str) -> Option<ParamValue> {
        let n = self.node(node)?;
        if let Some(v) = n.params.get(name) {
            return Some(v.clone());
        }
        let spec = n.spec()?;
        spec.params.iter().find(|p| p.name == name).map(|p| p.default_value())
    }

    /// The override key a subtype uses to address one parameter.
    ///
    /// Deliberately `<node id>.<param>` and nothing cleverer: an override is
    /// meant to be readable in the JSON and greppable in a diff.
    pub fn override_key(node: NodeId, param: &str) -> String {
        format!("{node}.{param}")
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(s: &str) -> anyhow::Result<BehaviorGraph> {
        Ok(serde_json::from_str(s)?)
    }
}
