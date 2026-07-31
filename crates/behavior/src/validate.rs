//! Telling the author what is wrong, in the terms they built it in.
//!
//! Validation runs on every edit, not on save. The distinction that matters
//! for a scientist is between an **error** — this graph cannot run, and the
//! model will refuse it — and a **warning** — this runs, but it is very
//! probably not what you meant. Both name the node they are about, so the
//! editor can highlight it rather than printing a message next to a canvas of
//! forty boxes.

use std::collections::BTreeMap;

use crate::graph::{BehaviorGraph, NodeId};
use crate::node::{registry, Category, ParamKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub severity: Severity,
    /// The node this is about, when it is about one.
    pub node: Option<NodeId>,
    pub message: String,
}

impl Issue {
    fn error(node: Option<NodeId>, message: String) -> Issue {
        Issue { severity: Severity::Error, node, message }
    }
    fn warn(node: Option<NodeId>, message: String) -> Issue {
        Issue { severity: Severity::Warning, node, message }
    }
}

/// The result of checking a graph.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub issues: Vec<Issue>,
    /// Evaluation order, when the graph is acyclic. Indices into
    /// `graph.nodes`, not node ids.
    pub order: Vec<usize>,
}

impl Report {
    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error)
    }
    pub fn warnings(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.severity == Severity::Warning)
    }
    pub fn ok(&self) -> bool {
        self.errors().next().is_none()
    }
    pub fn error_count(&self) -> usize {
        self.errors().count()
    }
    pub fn warning_count(&self) -> usize {
        self.warnings().count()
    }
    /// Issues attached to one node, for the highlight in the editor.
    pub fn for_node(&self, id: NodeId) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(move |i| i.node == Some(id))
    }
}

/// Check a graph and, if it is runnable, produce its evaluation order.
pub fn validate(g: &BehaviorGraph) -> Report {
    let reg = registry();
    let mut issues = Vec::new();

    // --- nodes ------------------------------------------------------------
    let mut index: BTreeMap<NodeId, usize> = BTreeMap::new();
    for (i, n) in g.nodes.iter().enumerate() {
        if index.insert(n.id, i).is_some() {
            issues.push(Issue::error(Some(n.id), format!("duplicate node id {}", n.id)));
        }
        let Some(spec) = reg.get(&n.type_id) else {
            issues.push(Issue::error(
                Some(n.id),
                format!(
                    "unknown node type \"{}\" — saved by a build that had it, or a typo",
                    n.type_id
                ),
            ));
            continue;
        };
        for (name, value) in &n.params {
            let Some(p) = spec.params.iter().find(|p| p.name == name) else {
                issues.push(Issue::warn(
                    Some(n.id),
                    format!("\"{}\" has no parameter \"{name}\"; it will be ignored", spec.name),
                ));
                continue;
            };
            if !p.default_value().same_kind(value) {
                issues.push(Issue::error(
                    Some(n.id),
                    format!("parameter \"{}\" has the wrong kind of value", p.label),
                ));
                continue;
            }
            match p.kind {
                ParamKind::Number { min, max, .. } => {
                    let v = value.as_number();
                    if !v.is_finite() {
                        issues.push(Issue::error(
                            Some(n.id),
                            format!("parameter \"{}\" is not a finite number", p.label),
                        ));
                    } else if v < min || v > max {
                        issues.push(Issue::warn(
                            Some(n.id),
                            format!("\"{}\" is {v}, outside the usual {min}–{max}", p.label),
                        ));
                    }
                }
                ParamKind::Choice { options, .. } => {
                    if !options.contains(&value.as_str()) {
                        issues.push(Issue::error(
                            Some(n.id),
                            format!(
                                "\"{}\" is set to \"{}\", which is not one of {}",
                                p.label,
                                value.as_str(),
                                options.join(", ")
                            ),
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    // --- wires ------------------------------------------------------------
    // Counted per destination port so a second wire into a single-connection
    // input is reported once, where the author can see it.
    let mut in_count: BTreeMap<(NodeId, u16), usize> = BTreeMap::new();
    for w in &g.wires {
        let (Some(&fi), Some(&ti)) = (index.get(&w.from_node), index.get(&w.to_node)) else {
            issues.push(Issue::error(None, "a wire refers to a node that is gone".into()));
            continue;
        };
        let (Some(fs), Some(ts)) = (g.nodes[fi].spec(), g.nodes[ti].spec()) else { continue };
        let (Some(op), Some(ip)) = (
            fs.outputs.get(w.from_port as usize),
            ts.inputs.get(w.to_port as usize),
        ) else {
            issues.push(Issue::error(
                Some(w.to_node),
                "a wire refers to a port that no longer exists".into(),
            ));
            continue;
        };
        if op.ty != ip.ty {
            issues.push(Issue::error(
                Some(w.to_node),
                format!(
                    "{}.{} is a {}, but {}.{} takes a {}",
                    fs.name,
                    op.name,
                    op.ty.label(),
                    ts.name,
                    ip.name,
                    ip.ty.label()
                ),
            ));
        }
        *in_count.entry((w.to_node, w.to_port)).or_default() += 1;
    }
    for ((node, port), n) in &in_count {
        if *n > 1 {
            let Some(&i) = index.get(node) else { continue };
            let Some(spec) = g.nodes[i].spec() else { continue };
            let Some(ip) = spec.inputs.get(*port as usize) else { continue };
            if !ip.multi {
                issues.push(Issue::error(
                    Some(*node),
                    format!("{n} wires into {}.{}, which takes one", spec.name, ip.name),
                ));
            }
        }
    }

    // --- required inputs ---------------------------------------------------
    for n in &g.nodes {
        let Some(spec) = n.spec() else { continue };
        for (pi, p) in spec.inputs.iter().enumerate() {
            let connected = in_count.contains_key(&(n.id, pi as u16));
            if !connected && p.default.is_none() && !p.multi {
                issues.push(Issue::error(
                    Some(n.id),
                    format!("{}.{} needs a connection", spec.name, p.name),
                ));
            }
        }
    }

    // --- outputs -----------------------------------------------------------
    let decisions = g
        .nodes
        .iter()
        .filter(|n| n.type_id == crate::nodes::DECISION_OUTPUT)
        .count();
    match decisions {
        0 => issues.push(Issue::error(
            None,
            "no \"Decision\" output node — the model has nothing to read".into(),
        )),
        1 => {}
        n => issues.push(Issue::error(
            None,
            format!("{n} \"Decision\" output nodes; a behaviour has exactly one"),
        )),
    }

    for n in &g.nodes {
        let Some(spec) = n.spec() else { continue };
        if spec.category == Category::Output {
            continue;
        }
        let feeds = g.wires.iter().any(|w| w.from_node == n.id);
        if !feeds && !spec.outputs.is_empty() {
            issues.push(Issue::warn(
                Some(n.id),
                format!("\"{}\" is not wired into anything", spec.name),
            ));
        }
    }

    // --- cycles ------------------------------------------------------------
    // Kahn's algorithm, which gives the evaluation order as a by-product; the
    // nodes left over when it stalls are exactly the ones in a cycle, so the
    // author gets told which boxes to look at rather than "cycle detected".
    let n = g.nodes.len();
    let mut indeg = vec![0usize; n];
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    for w in &g.wires {
        let (Some(&fi), Some(&ti)) = (index.get(&w.from_node), index.get(&w.to_node)) else {
            continue;
        };
        out[fi].push(ti);
        indeg[ti] += 1;
    }
    let mut queue: Vec<usize> = (0..n).filter(|i| indeg[*i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(i) = queue.pop() {
        order.push(i);
        for &j in &out[i] {
            indeg[j] -= 1;
            if indeg[j] == 0 {
                queue.push(j);
            }
        }
    }
    if order.len() != n {
        let stuck: Vec<String> = (0..n)
            .filter(|i| indeg[*i] > 0)
            .map(|i| {
                g.nodes[i]
                    .spec()
                    .map(|s| s.name.to_string())
                    .unwrap_or_else(|| g.nodes[i].type_id.clone())
            })
            .collect();
        for i in (0..n).filter(|i| indeg[*i] > 0) {
            issues.push(Issue::error(Some(g.nodes[i].id), "part of a feedback loop".into()));
        }
        issues.push(Issue::error(
            None,
            format!("the graph feeds back into itself through: {}", stuck.join(", ")),
        ));
        order.clear();
    }

    Report { issues, order }
}
