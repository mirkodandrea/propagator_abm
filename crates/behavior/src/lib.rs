//! **Agent Behaviour Composer** — authored agent behaviour, without Rust.
//!
//! A scientist working on this model does not want to write Rust, and should
//! not have to in order to ask "what if households acted on smoke rather than
//! on the order?". A developer, conversely, wants to expose a new thing an
//! agent can know or do without also writing an editor for it. This crate is
//! the join between those two:
//!
//! - a developer adds one [`behavior_node!`] invocation, and the node is in
//!   the palette, the validator, the inspector and the test bench;
//! - a scientist wires nodes into a [`BehaviorGraph`], names the result as an
//!   [`AgentSubtype`], and the model runs it.
//!
//! ### The shape of it
//!
//! | | |
//! |---|---|
//! | [`Observation`] | everything an agent may know. The whole contract with the model — a graph cannot reach past it |
//! | [`NodeSpec`] | one kind of box: its ports, its parameters, one `fn` |
//! | [`BehaviorGraph`] | boxes and wires, as saved |
//! | [`CompiledGraph`] | a graph resolved against the registry with a subtype's overrides baked in |
//! | [`AgentSubtype`] | a named profile: one graph, its overrides, starting traits |
//!
//! ### Three rules it holds to
//!
//! **Composition, not inheritance.** A subtype is a graph plus a flat map of
//! overrides. There is no parent, no resolution order, and no way for a change
//! in one profile to alter another. Variation without duplication comes from
//! several subtypes pointing at the same graph, which is a thing you can see
//! in a directory listing.
//!
//! **An authored graph cannot break determinism.** There is no random node.
//! Per-agent variation comes from [`Observation::jitter`], which `abm` hashes
//! from the household id — so the same household gets the same draw whatever
//! the step size and whoever else moved first. This is the same rule the
//! hand-written decision layer already follows, and it is why a replay is a
//! replay.
//!
//! **Evaluation is pure and stateless.** A graph maps one observation to one
//! decision. Nothing accumulates between calls, which means an authored
//! behaviour inherits the step-size invariance the rest of the model is tested
//! for rather than quietly breaking it.

pub mod defaults;
pub mod eval;
pub mod graph;
pub mod library;
pub mod node;
pub mod nodes;
pub mod observation;
pub mod subtype;
pub mod testbench;
pub mod validate;
pub mod value;

// Re-exported for the registration macro, which expands in the calling crate
// and must not require it to depend on `inventory` itself.
#[doc(hidden)]
pub use inventory;

pub use eval::{CompiledGraph, Decision, NodeTrace, Overrides, Scratch, Trace};
pub use graph::{BehaviorGraph, GraphNode, NodeId, Wire};
pub use library::{CompileError, Library};
pub use node::{
    registry, Category, EvalCtx, EvalFn, Inputs, NodeSpec, ParamKind, ParamSpec, ParamValue,
    Params, PortSpec, Registry,
};
pub use observation::Observation;
pub use subtype::{AgentSubtype, Capability, Difference, TraitKey};
pub use validate::{validate, Issue, Report, Severity};
pub use value::{ActionKind, ActionProposal, IntentValue, Value, ValueType};
