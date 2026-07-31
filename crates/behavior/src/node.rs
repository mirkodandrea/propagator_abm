//! The node abstraction, and the registry that makes new ones appear in the
//! editor by themselves.
//!
//! A node is described entirely by data — its ports, its parameters and one
//! `fn` pointer — so the whole registry is a static table with no allocation
//! and no trait objects. That matters for the two things this has to support
//! at once: the editor enumerating everything that exists, and the evaluator
//! running a graph a thousand times per decision tick without touching the
//! heap.
//!
//! Registration is distributed. [`inventory`] collects every
//! [`behavior_node!`](crate::behavior_node) invocation in every linked crate
//! at link time, so adding a node is one macro call in one file and nothing
//! else — no central list to update, and no way to add a node the palette does
//! not know about.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::observation::Observation;
use crate::value::{ActionKind, ActionProposal, IntentValue, Value, ValueType};

#[cfg(feature = "reflect")]
use bevy_reflect::Reflect;

/// Where a node sits in the palette, and what it is allowed to do.
///
/// The categories are not cosmetic: an `Observation` is the only kind that may
/// read the world, and an `Output` is the only kind the evaluator collects
/// results from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Reads one field of the [`Observation`]. No inputs.
    Observation,
    /// A constant the scientist sets, and the thing a subtype overrides.
    Parameter,
    /// Arithmetic, comparison, boolean algebra, shaping curves. Pure.
    Logic,
    /// Proposes an action with a priority. Pure, but its output only means
    /// something to an `Output` node.
    Action,
    /// A sink. What the model actually reads back out of the graph.
    Output,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Observation,
        Category::Parameter,
        Category::Logic,
        Category::Action,
        Category::Output,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Observation => "Observations",
            Category::Parameter => "Parameters",
            Category::Logic => "Logic",
            Category::Action => "Actions",
            Category::Output => "Outputs",
        }
    }

    /// Header tint for the editor, sRGB bytes.
    pub fn colour(self) -> [u8; 3] {
        match self {
            Category::Observation => [0x3a, 0x62, 0x7a],
            Category::Parameter => [0x5c, 0x52, 0x76],
            Category::Logic => [0x3f, 0x4a, 0x5a],
            Category::Action => [0x76, 0x44, 0x3c],
            Category::Output => [0x3f, 0x63, 0x45],
        }
    }
}

/// One input or output port.
#[derive(Debug, Clone, Copy)]
pub struct PortSpec {
    pub name: &'static str,
    pub ty: ValueType,
    pub doc: &'static str,
    /// Value used when nothing is wired in. `None` makes the input required,
    /// and the validator reports a graph that leaves it dangling.
    pub default: Option<Value>,
    /// The port accepts more than one wire. Only `out.decision` does; every
    /// other multi-connection is a mistake worth reporting.
    pub multi: bool,
}

/// How a parameter is edited, and what it is allowed to be.
#[derive(Debug, Clone, Copy)]
pub enum ParamKind {
    Number { default: f32, min: f32, max: f32, unit: &'static str },
    Bool { default: bool },
    /// A fixed set of keys. Used for intent and action selection, so a subtype
    /// override cannot name something that does not exist.
    Choice { default: &'static str, options: &'static [&'static str] },
    Text { default: &'static str },
}

#[derive(Debug, Clone, Copy)]
pub struct ParamSpec {
    pub name: &'static str,
    pub label: &'static str,
    pub doc: &'static str,
    pub kind: ParamKind,
}

impl ParamSpec {
    pub fn default_value(&self) -> ParamValue {
        match self.kind {
            ParamKind::Number { default, .. } => ParamValue::Number(default),
            ParamKind::Bool { default } => ParamValue::Bool(default),
            ParamKind::Choice { default, .. } => ParamValue::Choice(default.to_string()),
            ParamKind::Text { default } => ParamValue::Text(default.to_string()),
        }
    }
}

/// A parameter's value in a saved graph or a subtype override.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(tag = "t", content = "v", rename_all = "snake_case")]
pub enum ParamValue {
    Number(f32),
    Bool(bool),
    Choice(String),
    Text(String),
}

impl ParamValue {
    pub fn as_number(&self) -> f32 {
        match self {
            ParamValue::Number(n) => *n,
            ParamValue::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            ParamValue::Bool(b) => *b,
            ParamValue::Number(n) => *n > 0.5,
            _ => false,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            ParamValue::Choice(s) | ParamValue::Text(s) => s,
            _ => "",
        }
    }

    pub fn display(&self) -> String {
        match self {
            ParamValue::Number(n) => format!("{n:.3}"),
            ParamValue::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
            ParamValue::Choice(s) | ParamValue::Text(s) => s.clone(),
        }
    }

    /// Whether two values are the same *kind*. A subtype override that changes
    /// a number into a string is a broken subtype, not a clever one.
    pub fn same_kind(&self, other: &ParamValue) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// The parameter values a node is evaluated with, after subtype overrides.
///
/// Indexed positionally against [`NodeSpec::params`]: the compiler resolves
/// names once, so the hot path never does a map lookup.
pub struct Params<'a> {
    pub(crate) values: &'a [ParamValue],
}

impl<'a> Params<'a> {
    pub fn num(&self, i: usize) -> f32 {
        self.values.get(i).map(ParamValue::as_number).unwrap_or(0.0)
    }
    pub fn boolean(&self, i: usize) -> bool {
        self.values.get(i).map(ParamValue::as_bool).unwrap_or(false)
    }
    pub fn key(&self, i: usize) -> &str {
        self.values.get(i).map(ParamValue::as_str).unwrap_or("")
    }
    pub fn intent(&self, i: usize) -> IntentValue {
        IntentValue::from_key(self.key(i)).unwrap_or(IntentValue::WaitAndSee)
    }
    pub fn action(&self, i: usize) -> ActionKind {
        ActionKind::from_key(self.key(i)).unwrap_or(ActionKind::Stay)
    }
}

/// The values arriving on a node's inputs.
///
/// A slot is empty when nothing is wired in, in which case the accessors fall
/// back to the port's declared default — which is why a half-built graph still
/// evaluates in the test bench instead of refusing to run.
pub struct Inputs<'a> {
    pub(crate) slots: &'a [Vec<Value>],
    pub(crate) ports: &'static [PortSpec],
}

impl<'a> Inputs<'a> {
    fn first(&self, i: usize) -> Option<Value> {
        self.slots.get(i).and_then(|s| s.first().copied()).or_else(|| {
            self.ports.get(i).and_then(|p| p.default)
        })
    }

    pub fn num(&self, i: usize) -> f32 {
        self.first(i).map(|v| v.as_number()).unwrap_or(0.0)
    }
    pub fn boolean(&self, i: usize) -> bool {
        self.first(i).map(|v| v.as_bool()).unwrap_or(false)
    }
    pub fn intent(&self, i: usize) -> IntentValue {
        self.first(i).map(|v| v.as_intent()).unwrap_or(IntentValue::WaitAndSee)
    }
    /// Every value on a multi-connection input, in wire order.
    pub fn all(&self, i: usize) -> &[Value] {
        self.slots.get(i).map(|s| s.as_slice()).unwrap_or(&[])
    }
    /// Whether anything is actually wired into this input.
    pub fn connected(&self, i: usize) -> bool {
        self.slots.get(i).map(|s| !s.is_empty()).unwrap_or(false)
    }
}

/// Everything a node may read that is not on a wire.
pub struct EvalCtx<'a> {
    pub obs: &'a Observation,
}

/// The evaluation function. Writes one value per declared output, in order.
pub type EvalFn = fn(&EvalCtx, &Params, &Inputs, &mut Vec<Value>);

/// Everything the editor and the evaluator know about one kind of node.
pub struct NodeSpec {
    /// Stable, dotted, and the key in the saved graph. Renaming one breaks
    /// every graph that used it, so treat it as a file format.
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub doc: &'static str,
    /// Extra words the palette search matches on, for the terms a scientist
    /// would actually type: "smoke" finds the fire-distance observation.
    pub keywords: &'static [&'static str],
    pub inputs: &'static [PortSpec],
    pub outputs: &'static [PortSpec],
    pub params: &'static [ParamSpec],
    pub eval: EvalFn,
}

impl NodeSpec {
    pub fn param_index(&self, name: &str) -> Option<usize> {
        self.params.iter().position(|p| p.name == name)
    }

    pub fn default_params(&self) -> Vec<ParamValue> {
        self.params.iter().map(ParamSpec::default_value).collect()
    }

    /// Whether the palette search `query` (already lowercased) matches.
    pub fn matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        query.split_whitespace().all(|term| {
            self.id.contains(term)
                || self.name.to_lowercase().contains(term)
                || self.doc.to_lowercase().contains(term)
                || self.keywords.iter().any(|k| k.contains(term))
        })
    }
}

impl std::fmt::Debug for NodeSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeSpec").field("id", &self.id).finish()
    }
}

inventory::collect!(NodeSpec);

/// Every registered node, indexed by id.
pub struct Registry {
    by_id: BTreeMap<&'static str, &'static NodeSpec>,
}

impl Registry {
    pub fn get(&self, id: &str) -> Option<&'static NodeSpec> {
        self.by_id.get(id).copied()
    }

    pub fn all(&self) -> impl Iterator<Item = &'static NodeSpec> + '_ {
        self.by_id.values().copied()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Everything in one category, in palette order (id, which sorts the
    /// dotted namespaces together).
    pub fn in_category(&self, c: Category) -> impl Iterator<Item = &'static NodeSpec> + '_ {
        self.all().filter(move |s| s.category == c)
    }
}

/// The process-wide node registry.
///
/// Built once from the [`inventory`] collection. A duplicate id is a
/// programming error that would silently shadow a node, so it panics: better
/// at first use than as a graph that quietly loads the wrong behaviour.
pub fn registry() -> &'static Registry {
    static REG: OnceLock<Registry> = OnceLock::new();
    REG.get_or_init(|| {
        let mut by_id = BTreeMap::new();
        for spec in inventory::iter::<NodeSpec> {
            if by_id.insert(spec.id, spec).is_some() {
                panic!("duplicate behaviour node id {:?}", spec.id);
            }
        }
        Registry { by_id }
    })
}

/// Convenience for action nodes: the standard "withheld" proposal.
pub fn withheld(kind: ActionKind) -> Value {
    Value::Action(ActionProposal { kind, priority: 0.0, fired: false })
}

// ---------------------------------------------------------------------------
// Registration macros
// ---------------------------------------------------------------------------

/// Declare a port. Not called directly — see [`behavior_node!`].
#[doc(hidden)]
#[macro_export]
macro_rules! __port {
    (number $name:literal, $doc:literal) => {
        $crate::PortSpec { name: $name, ty: $crate::ValueType::Number, doc: $doc, default: None, multi: false }
    };
    (number $name:literal, $doc:literal, $d:expr) => {
        $crate::PortSpec { name: $name, ty: $crate::ValueType::Number, doc: $doc, default: Some($crate::Value::Number($d)), multi: false }
    };
    (bool $name:literal, $doc:literal) => {
        $crate::PortSpec { name: $name, ty: $crate::ValueType::Bool, doc: $doc, default: None, multi: false }
    };
    (bool $name:literal, $doc:literal, $d:expr) => {
        $crate::PortSpec { name: $name, ty: $crate::ValueType::Bool, doc: $doc, default: Some($crate::Value::Bool($d)), multi: false }
    };
    (intent $name:literal, $doc:literal) => {
        $crate::PortSpec { name: $name, ty: $crate::ValueType::Intent, doc: $doc, default: None, multi: false }
    };
    (action $name:literal, $doc:literal) => {
        $crate::PortSpec { name: $name, ty: $crate::ValueType::Action, doc: $doc, default: None, multi: false }
    };
    (actions $name:literal, $doc:literal) => {
        $crate::PortSpec { name: $name, ty: $crate::ValueType::Action, doc: $doc, default: None, multi: true }
    };
}

/// Declare a parameter. Not called directly — see [`behavior_node!`].
#[doc(hidden)]
#[macro_export]
macro_rules! __param {
    (number $name:literal, $label:literal, $doc:literal, $d:expr, $min:expr, $max:expr, $unit:literal) => {
        $crate::ParamSpec {
            name: $name,
            label: $label,
            doc: $doc,
            kind: $crate::ParamKind::Number { default: $d, min: $min, max: $max, unit: $unit },
        }
    };
    (bool $name:literal, $label:literal, $doc:literal, $d:expr) => {
        $crate::ParamSpec { name: $name, label: $label, doc: $doc, kind: $crate::ParamKind::Bool { default: $d } }
    };
    (choice $name:literal, $label:literal, $doc:literal, $d:literal, [$($opt:literal),* $(,)?]) => {
        $crate::ParamSpec {
            name: $name,
            label: $label,
            doc: $doc,
            kind: $crate::ParamKind::Choice { default: $d, options: &[$($opt),*] },
        }
    };
    (text $name:literal, $label:literal, $doc:literal, $d:literal) => {
        $crate::ParamSpec { name: $name, label: $label, doc: $doc, kind: $crate::ParamKind::Text { default: $d } }
    };
}

/// Register one behaviour node.
///
/// This is the whole developer-facing surface for exposing a new agent
/// capability: one invocation, anywhere in any linked crate, and the node is in
/// the palette, the validator, the inspector and the test bench.
///
/// Ports and parameters are each parenthesised, which keeps the grammar
/// unambiguous however many optional pieces a declaration uses:
///
/// ```ignore
/// behavior_node! {
///     id: "logic.and",
///     name: "And",
///     category: Logic,
///     doc: "True when both inputs are true.",
///     keywords: ["both", "&&"],
///     inputs: [(bool "a", "First condition", false), (bool "b", "Second condition", false)],
///     outputs: [(bool "out", "a and b")],
///     params: [],
///     eval: |_ctx, _p, i, out| out.push(Value::Bool(i.boolean(0) && i.boolean(1))),
/// }
/// ```
#[macro_export]
macro_rules! behavior_node {
    (
        id: $id:literal,
        name: $name:literal,
        category: $cat:ident,
        doc: $doc:literal,
        $(keywords: [$($kw:literal),* $(,)?],)?
        inputs: [ $(( $($itok:tt)* )),* $(,)? ],
        outputs: [ $(( $($otok:tt)* )),* $(,)? ],
        params: [ $(( $($ptok:tt)* )),* $(,)? ],
        eval: $eval:expr $(,)?
    ) => {
        $crate::inventory::submit! {
            $crate::NodeSpec {
                id: $id,
                name: $name,
                category: $crate::Category::$cat,
                doc: $doc,
                keywords: &[$($($kw),*)?],
                inputs: &[ $($crate::__port!($($itok)*)),* ],
                outputs: &[ $($crate::__port!($($otok)*)),* ],
                params: &[ $($crate::__param!($($ptok)*)),* ],
                eval: {
                    // Coerced to a plain fn pointer so the whole spec stays a
                    // `static` with no allocation behind it.
                    const F: $crate::EvalFn = $eval;
                    F
                },
            }
        }
    };
}
