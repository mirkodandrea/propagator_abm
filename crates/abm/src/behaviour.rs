//! Running an authored behaviour instead of the hand-written decision layer.
//!
//! The composer replaces exactly one thing: the block in [`Abm::decide`] that
//! decides whether a household departs, defends or shelters. Perception,
//! preparation, movement, congestion, rerouting and the lethality model are
//! untouched — an authored graph changes *when* people act, not what acting
//! costs them. That boundary is the reason a graph a scientist wrote can be
//! run in a real scenario without review.
//!
//! Three properties are preserved deliberately, because they are what the rest
//! of the model's tests rest on:
//!
//! **Step-size invariance.** A graph maps one observation to one decision and
//! accumulates nothing, so the decision layer's answer at a 2 s step and a
//! 60 s step is the same. There is no way to author around this.
//!
//! **Per-agent determinism.** The graph's only source of variation is
//! `Observation::jitter`, which is hashed from the household id — the same
//! draw the hand-written layer uses. An authored behaviour cannot become
//! order-dependent or step-dependent.
//!
//! **A closed read surface.** A graph sees a [`behavior::Observation`] and
//! nothing else, and hands back an action from a fixed set. It cannot reach
//! the fire model, the road network, or another household.

use std::collections::BTreeMap;

pub use behavior::subtype::{Capability, TraitKey};
use behavior::{ActionKind, CompiledGraph, Decision, Library, Observation, Scratch};
use scenario::population::Intent;

/// One compiled subtype, ready to run.
pub struct SubtypeRuntime {
    pub id: String,
    pub name: String,
    pub graph: CompiledGraph,
    pub traits: BTreeMap<TraitKey, f32>,
    pub capabilities: BTreeMap<Capability, bool>,
    scratch: Scratch,
}

/// Everything the agent model needs to run authored behaviour.
///
/// Built once, from a [`Library`], before `Abm::new`: the subtype a household
/// belongs to sets its starting traits, so the assignment has to happen while
/// the households are being constructed rather than after.
pub struct BehaviorRuntime {
    subtypes: Vec<SubtypeRuntime>,
    /// Cumulative shares, for assignment. Same length as `subtypes`.
    cumulative: Vec<f32>,
}

/// What went wrong building a runtime, in the terms the panel reports.
#[derive(Debug)]
pub struct RuntimeErrors(pub Vec<String>);

impl std::fmt::Display for RuntimeErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join("; "))
    }
}

impl std::error::Error for RuntimeErrors {}

impl BehaviorRuntime {
    /// Compile every subtype with a non-zero share.
    ///
    /// Fails as a whole rather than silently dropping a subtype that will not
    /// compile: a run with three of four profiles in it is a different
    /// experiment, and finding that out from the household counts is not
    /// finding it out.
    pub fn build(lib: &Library) -> Result<Option<BehaviorRuntime>, RuntimeErrors> {
        let assignment = lib.assignment();
        if assignment.is_empty() {
            return Ok(None);
        }

        let mut subtypes = Vec::with_capacity(assignment.len());
        let mut cumulative = Vec::with_capacity(assignment.len());
        let mut errors = Vec::new();
        let mut running = 0.0;

        for (id, share) in assignment {
            let Some(s) = lib.subtypes.get(&id) else { continue };
            match lib.compile(&id) {
                Ok(graph) => {
                    let scratch = Scratch::new(&graph);
                    running += share;
                    cumulative.push(running);
                    subtypes.push(SubtypeRuntime {
                        id: id.clone(),
                        name: s.name.clone(),
                        graph,
                        traits: s.traits.clone(),
                        capabilities: s.capabilities.clone(),
                        scratch,
                    });
                }
                Err(e) => errors.push(e.to_string()),
            }
        }

        if !errors.is_empty() {
            return Err(RuntimeErrors(errors));
        }
        if subtypes.is_empty() {
            return Ok(None);
        }
        // Guard against float drift leaving the last bucket unreachable.
        if let Some(last) = cumulative.last_mut() {
            *last = 1.0;
        }
        Ok(Some(BehaviorRuntime { subtypes, cumulative }))
    }

    pub fn len(&self) -> usize {
        self.subtypes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.subtypes.is_empty()
    }

    pub fn subtype(&self, i: usize) -> Option<&SubtypeRuntime> {
        self.subtypes.get(i)
    }

    pub fn names(&self) -> impl Iterator<Item = (&str, &str)> {
        self.subtypes.iter().map(|s| (s.id.as_str(), s.name.as_str()))
    }

    /// Which subtype a household belongs to.
    ///
    /// Hashed from the household id rather than drawn from an RNG, for the
    /// same reason every other per-agent quantity is: the assignment has to
    /// survive a restart, a different step size, and any change to the order
    /// households are built in.
    pub fn assign(&self, household_id: usize) -> usize {
        let r = crate::hash01(household_id as u64, 0x5B7A);
        self.cumulative.iter().position(|c| r < *c).unwrap_or(self.subtypes.len() - 1)
    }

    /// Evaluate a household's behaviour.
    pub fn decide(&mut self, subtype: usize, obs: &Observation) -> Decision {
        let Some(s) = self.subtypes.get_mut(subtype) else { return Decision::default() };
        s.graph.eval_with(obs, &mut s.scratch)
    }

    /// Evaluate with a full trace, for the household inspector. Slower, and
    /// only ever called for the one household the player clicked.
    pub fn explain(&self, subtype: usize, obs: &Observation) -> Option<(Decision, behavior::Trace)> {
        let s = self.subtypes.get(subtype)?;
        Some(s.graph.eval_traced(obs))
    }
}

/// A trait value the subtype forces, or `None` to leave the bake alone.
pub fn trait_override(rt: &BehaviorRuntime, subtype: usize, key: TraitKey) -> Option<f32> {
    rt.subtypes.get(subtype)?.traits.get(&key).copied()
}

pub fn capability_override(
    rt: &BehaviorRuntime,
    subtype: usize,
    key: Capability,
) -> Option<bool> {
    rt.subtypes.get(subtype)?.capabilities.get(&key).copied()
}

/// `scenario`'s intent, as the composer's.
///
/// The two enums are deliberately separate — `behavior` is a leaf crate that
/// knows nothing about scenarios — and this is the one place they meet.
pub fn intent_of(i: Intent) -> behavior::IntentValue {
    match i {
        Intent::LeaveEarly => behavior::IntentValue::LeaveEarly,
        Intent::WaitAndSee => behavior::IntentValue::WaitAndSee,
        Intent::StayDefend => behavior::IntentValue::StayDefend,
    }
}

/// What the movement layer does with each action.
///
/// Written out here rather than inline in `decide` because it is the whole
/// semantics of the composer's output, and it should be readable in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing changes this tick.
    Hold,
    /// Start milling, then leave.
    Prepare,
    /// Leave immediately: whatever preparation is left is abandoned.
    Go,
    /// Commit to defending the property.
    Defend,
    /// Stop trying to move and shelter where they are.
    Shelter,
}

pub fn outcome_of(a: ActionKind) -> Outcome {
    match a {
        ActionKind::Stay => Outcome::Hold,
        ActionKind::Prepare => Outcome::Prepare,
        ActionKind::EvacuateNow => Outcome::Go,
        ActionKind::Defend => Outcome::Defend,
        ActionKind::Shelter => Outcome::Shelter,
    }
}
