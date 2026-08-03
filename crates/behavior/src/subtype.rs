//! Named behavioural profiles.
//!
//! A subtype is a graph plus a set of parameter overrides plus some starting
//! traits. That is the whole model, and the omission is deliberate: there is
//! **no inheritance**. A subtype does not extend another subtype, cannot
//! partially override a parent's overrides, and has no resolution order to
//! reason about. What you read in the file is what the agent gets.
//!
//! The cost of that is duplication between two similar profiles. The benefit
//! is that "why did this household do that" is answered by reading one file,
//! which is the question a scientist actually asks. Variation without
//! duplication is available where it belongs — several subtypes pointing at
//! the *same* graph with different numbers.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::eval::Overrides;
use crate::graph::BehaviorGraph;
use crate::node::ParamValue;
use crate::value::UnitKindKey;

/// A trait value a subtype starts its households with, replacing whatever the
/// population bake drew.
///
/// Only the fields a behaviour can meaningfully be authored against are
/// settable; everything else about a household still comes from the bake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraitKey {
    RiskPerception,
    TrustAuthority,
    PrepTimeMin,
    DefensibleSpace,
}

impl TraitKey {
    pub const ALL: [TraitKey; 4] = [
        TraitKey::RiskPerception,
        TraitKey::TrustAuthority,
        TraitKey::PrepTimeMin,
        TraitKey::DefensibleSpace,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TraitKey::RiskPerception => "Risk perception",
            TraitKey::TrustAuthority => "Trust in authority",
            TraitKey::PrepTimeMin => "Preparation time (min)",
            TraitKey::DefensibleSpace => "Defensible space",
        }
    }

    /// Sensible editing range, and whether it is a 0–1 fraction.
    pub fn range(self) -> (f32, f32) {
        match self {
            TraitKey::PrepTimeMin => (0.0, 120.0),
            _ => (0.0, 1.0),
        }
    }
}

/// A capability a subtype's households have or lack.
///
/// Capabilities are flags the observation exposes, so a behaviour can be
/// authored against them; they are not a permission system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// The household has a car. Without it they walk — slower, but immune to
    /// a road being cut.
    Vehicle,
    /// Someone cannot move unaided.
    NeedsAssistance,
    /// Nobody in this household is at home: a hotel party, a let, a campsite.
    ///
    /// A capability rather than a field on the population bake, and that is the
    /// interesting part: a transient population is *assigned* exactly the way
    /// every other behavioural profile is — a share of households, hashed — so
    /// asking "what does this town look like in August" is a share on a profile
    /// rather than a re-bake. What it changes in the model is small and
    /// specific: no vehicle of their own, a warning that arrives through
    /// whoever is running the place instead of over a resident's channel, and
    /// no local knowledge, which is a threshold their behaviour graph reads.
    Transient,
}

impl Capability {
    pub const ALL: [Capability; 3] =
        [Capability::Vehicle, Capability::NeedsAssistance, Capability::Transient];

    pub fn label(self) -> &'static str {
        match self {
            Capability::Vehicle => "Has a vehicle",
            Capability::NeedsAssistance => "Needs assistance",
            Capability::Transient => "Visitors, not residents",
        }
    }
}

/// A named behavioural profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSubtype {
    /// Stable id, referenced by an assignment. Kebab-case by convention.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,

    /// The graph this subtype runs. Several subtypes sharing one graph is the
    /// intended shape.
    pub graph: String,

    /// Parameter overrides, keyed `"<node id>.<param name>"`. See
    /// [`BehaviorGraph::override_key`].
    #[serde(default)]
    pub overrides: BTreeMap<String, ParamValue>,

    /// Starting traits that replace the population bake's draw.
    #[serde(default)]
    pub traits: BTreeMap<TraitKey, f32>,

    /// Capabilities forced on or off. Absent means "leave it as baked".
    ///
    /// Households only.
    #[serde(default)]
    pub capabilities: BTreeMap<Capability, bool>,

    /// Share of the population assigned this subtype, before normalisation.
    /// Zero means the subtype exists but is not in play.
    ///
    /// **Households only.** A share is how you assign a profile to 750
    /// anonymous families; there are eight suppression units and they are named
    /// individuals, so a unit profile says which *kinds* it governs instead —
    /// see [`AgentSubtype::unit_kinds`].
    #[serde(default)]
    pub share: f32,

    /// Which unit kinds this profile governs. **Suppression units only.**
    ///
    /// Empty means every kind, which is the usual case: a safety policy that
    /// differs between a crew and an engine is a safety policy someone has to
    /// remember to keep in step, and the per-kind parameters on
    /// `block.unit_safety` already express the difference where it is real.
    ///
    /// The first profile matching a unit's kind wins, in id order. Deliberately
    /// not a share: assigning a named engine a policy by a hash of its id would
    /// make "why did Autobotte 2 do that" unanswerable from the files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unit_kinds: Vec<UnitKindKey>,

    /// Whether this profile is in play at all. **Suppression units only** —
    /// households use `share` for the same purpose, because a share of zero
    /// already says it.
    #[serde(default)]
    pub enabled: bool,
}

impl AgentSubtype {
    pub fn new(id: impl Into<String>, name: impl Into<String>, graph: impl Into<String>) -> Self {
        AgentSubtype {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            author: String::new(),
            tags: Vec::new(),
            graph: graph.into(),
            overrides: BTreeMap::new(),
            traits: BTreeMap::new(),
            capabilities: BTreeMap::new(),
            share: 1.0,
            unit_kinds: Vec::new(),
            enabled: true,
        }
    }

    /// Whether this profile governs a unit of `kind`.
    pub fn governs(&self, kind: UnitKindKey) -> bool {
        self.enabled && (self.unit_kinds.is_empty() || self.unit_kinds.contains(&kind))
    }

    /// A copy under a new id, with the name marked so two of them are
    /// distinguishable in a list.
    pub fn duplicate(&self, new_id: impl Into<String>) -> AgentSubtype {
        let mut c = self.clone();
        c.id = new_id.into();
        c.name = format!("{} (copy)", self.name);
        c
    }

    pub fn overrides(&self) -> &Overrides {
        &self.overrides
    }

    /// Drop overrides that no longer name a real node or parameter.
    ///
    /// Run after the graph is edited: an override whose node was deleted is
    /// dead weight that will otherwise sit in the file forever, and one whose
    /// parameter changed kind is worse — it is silently ignored at compile.
    /// Returns the keys removed.
    pub fn prune(&mut self, graph: &BehaviorGraph) -> Vec<String> {
        let mut dead = Vec::new();
        self.overrides.retain(|key, value| {
            let live = split_key(key)
                .and_then(|(node, param)| {
                    let spec = graph.node(node)?.spec()?;
                    let p = spec.params.iter().find(|p| p.name == param)?;
                    Some(p.default_value().same_kind(value))
                })
                .unwrap_or(false);
            if !live {
                dead.push(key.clone());
            }
            live
        });
        dead
    }
}

/// `"12.threshold"` -> `(12, "threshold")`.
pub fn split_key(key: &str) -> Option<(crate::graph::NodeId, &str)> {
    let (node, param) = key.split_once('.')?;
    Some((node.parse().ok()?, param))
}

/// One line of difference between two subtypes.
#[derive(Debug, Clone, PartialEq)]
pub struct Difference {
    /// What differs, in the author's terms — "Threshold on Above threshold",
    /// not "17.threshold".
    pub what: String,
    pub left: String,
    pub right: String,
}

/// Compare two subtypes field by field.
///
/// `graph` is used only to render override keys as readable names; pass the
/// graph they share, or `None` if they do not share one.
pub fn compare(
    a: &AgentSubtype,
    b: &AgentSubtype,
    graph: Option<&BehaviorGraph>,
) -> Vec<Difference> {
    let mut out = Vec::new();
    let mut push = |what: &str, l: String, r: String| {
        if l != r {
            out.push(Difference { what: what.to_string(), left: l, right: r });
        }
    };

    push("Behaviour graph", a.graph.clone(), b.graph.clone());
    push("Share", format!("{:.2}", a.share), format!("{:.2}", b.share));
    push("Tags", a.tags.join(", "), b.tags.join(", "));
    let kinds = |s: &AgentSubtype| {
        if s.unit_kinds.is_empty() {
            "every kind".to_string()
        } else {
            s.unit_kinds.iter().map(|k| k.label()).collect::<Vec<_>>().join(", ")
        }
    };
    push("Applies to", kinds(a), kinds(b));

    let keys: BTreeSet<&String> = a.overrides.keys().chain(b.overrides.keys()).collect();
    for k in keys {
        let l = a.overrides.get(k);
        let r = b.overrides.get(k);
        if l == r {
            continue;
        }
        let label = describe_key(k, graph);
        // "(graph default)" rather than "(none)": an absent override is not a
        // missing value, it is the graph's own number, and conflating the two
        // is how a comparison misleads.
        out.push(Difference {
            what: label,
            left: l.map(ParamValue::display).unwrap_or_else(|| "(graph default)".into()),
            right: r.map(ParamValue::display).unwrap_or_else(|| "(graph default)".into()),
        });
    }

    for t in TraitKey::ALL {
        let (l, r) = (a.traits.get(&t), b.traits.get(&t));
        if l == r {
            continue;
        }
        out.push(Difference {
            what: format!("Trait: {}", t.label()),
            left: l.map(|v| format!("{v:.2}")).unwrap_or_else(|| "(as baked)".into()),
            right: r.map(|v| format!("{v:.2}")).unwrap_or_else(|| "(as baked)".into()),
        });
    }

    for c in Capability::ALL {
        let (l, r) = (a.capabilities.get(&c), b.capabilities.get(&c));
        if l == r {
            continue;
        }
        let show = |v: Option<&bool>| match v {
            Some(true) => "yes".to_string(),
            Some(false) => "no".to_string(),
            None => "(as baked)".to_string(),
        };
        out.push(Difference {
            what: format!("Capability: {}", c.label()),
            left: show(l),
            right: show(r),
        });
    }

    out
}

/// Render an override key as something readable, given the graph it refers to.
pub fn describe_key(key: &str, graph: Option<&BehaviorGraph>) -> String {
    let Some((node_id, param)) = split_key(key) else {
        return key.to_string();
    };
    let named = graph.and_then(|g| {
        let n = g.node(node_id)?;
        let spec = n.spec()?;
        let label = spec
            .params
            .iter()
            .find(|p| p.name == param)
            .map(|p| p.label)
            .unwrap_or(param);
        // A parameter node's own label is far more use than "Number", so
        // prefer it when the author bothered to set one.
        let title = n
            .params
            .get("label")
            .map(ParamValue::display)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| spec.name.to_string());
        Some(format!("{label} on \"{title}\" (#{node_id})"))
    });
    named.unwrap_or_else(|| key.to_string())
}
