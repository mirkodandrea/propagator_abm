//! Running the graph-backed decision layers.
//!
//! Three of them, one per [`behavior::Domain`], and each controls exactly one
//! block:
//!
//! | | Replaces | Leaves alone |
//! |---|---|---|
//! | Households | the departure decision in [`Abm::decide`](crate::Abm) | perception, milling, movement, congestion, lethality |
//! | Separated people | where one person who is not with their household is trying to get to | how fast they walk, what the smoke does to them, whether they make it |
//! | Suppression units | the self-preservation and resupply policy in [`Suppression::step`](crate::Suppression) | where units are sent, how fast they move, what their work does to the fire |
//!
//! An authored graph changes *when* an agent acts, not what acting costs it.
//! That boundary is the reason a graph a scientist wrote can be run in a real
//! scenario without review, and it is the same boundary in both domains.
//!
//! Three properties are preserved deliberately, because they are what the rest
//! of the model's tests rest on:
//!
//! **Step-size invariance.** A graph maps one observation to one decision and
//! accumulates nothing, so the decision layer's answer at a 2 s step and a
//! 60 s step is the same. There is no way to author around this.
//!
//! **Per-agent determinism.** The graph's only source of variation is
//! `Observation::jitter`, which is hashed from the household id. A behavior
//! graph cannot become
//! order-dependent or step-dependent.
//!
//! **A closed read surface.** A graph sees a [`behavior::Observation`] and
//! nothing else, and hands back an action from a fixed set. It cannot reach
//! the fire model, the road network, or another household.

use std::collections::BTreeMap;

pub use behavior::subtype::{Capability, TraitKey};
use behavior::{
    ActionKind, CompiledGraph, Decision, Domain, Library, Observation, Scratch, UnitKindKey,
};
use scenario::population::Intent;

use crate::suppression::UnitKind;

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
        BehaviorRuntime::build_for(lib, Domain::Household)
    }

    /// The same, for any domain whose profiles are assigned by share.
    ///
    /// Households and separated people both are, because both are populations
    /// of anonymous agents; suppression units are not, and [`UnitRuntime`] says
    /// why.
    pub fn build_for(
        lib: &Library,
        domain: Domain,
    ) -> Result<Option<BehaviorRuntime>, RuntimeErrors> {
        let assignment = lib.share_assignment(domain);
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
        self.assign_with(household_id, 0x5B7A)
    }

    /// The same, with the caller's own salt.
    ///
    /// A person and their household are different agents in different domains,
    /// and sharing a salt would correlate their profiles: every member of a
    /// household with a low hash would land in the same person profile, which
    /// looks like a finding and is an artefact.
    pub fn assign_with(&self, id: usize, salt: u64) -> usize {
        let r = crate::hash01(id as u64, salt);
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
        ActionKind::Prepare => Outcome::Prepare,
        ActionKind::EvacuateNow => Outcome::Go,
        ActionKind::Defend => Outcome::Defend,
        ActionKind::Shelter => Outcome::Shelter,
        // `Stay`, plus the suppression half of the enum, which a validated
        // household graph cannot produce.
        _ => Outcome::Hold,
    }
}

// ---------------------------------------------------------------------------
// Separated people
// ---------------------------------------------------------------------------

/// Everything the agent model needs to run authored behaviour for the people
/// who are not with their household.
///
/// A thin newtype over [`BehaviorRuntime`] rather than a copy of it: the
/// assignment mechanism is identical — hundreds of anonymous agents, profiles
/// with shares — and the only reason it is a distinct type is that handing a
/// person runtime to the household decision layer must not compile.
pub struct PersonRuntime(BehaviorRuntime);

impl PersonRuntime {
    pub fn build(lib: &Library) -> Result<Option<PersonRuntime>, RuntimeErrors> {
        Ok(BehaviorRuntime::build_for(lib, Domain::Person)?.map(PersonRuntime))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn subtype(&self, i: usize) -> Option<&SubtypeRuntime> {
        self.0.subtype(i)
    }

    pub fn names(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.names()
    }

    /// Which profile a person runs, hashed from the person id.
    pub fn assign(&self, person_id: usize) -> usize {
        self.0.assign_with(person_id, 0x2C91)
    }

    pub fn decide(&mut self, subtype: usize, obs: &Observation) -> Decision {
        self.0.decide(subtype, obs)
    }

    pub fn explain(
        &self,
        subtype: usize,
        obs: &Observation,
    ) -> Option<(Decision, behavior::Trace)> {
        self.0.explain(subtype, obs)
    }
}

/// What the movement layer does with each separated-person action.
///
/// Narrower than the household's, and every one of these is about a
/// *destination* rather than about timing. That is the difference between the
/// two civilian domains: a household decides when to go, and a person who is
/// already out decides where to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonOutcome {
    /// Carry on doing whatever they were doing.
    Carry,
    /// Make for the nearest refuge on foot.
    WalkOut,
    /// Stop moving and shelter where they are.
    Shelter,
    /// Turn round and walk back to the household's home.
    HeadHome,
}

pub fn person_outcome_of(a: ActionKind) -> PersonOutcome {
    match a {
        ActionKind::WalkOut => PersonOutcome::WalkOut,
        ActionKind::TakeShelter => PersonOutcome::Shelter,
        ActionKind::HeadHome => PersonOutcome::HeadHome,
        // `Remain`, plus the other two domains' halves of the enum, which a
        // validated person graph cannot produce.
        _ => PersonOutcome::Carry,
    }
}

// ---------------------------------------------------------------------------
// Suppression units
// ---------------------------------------------------------------------------

/// What the suppression layer does with each unit action.
///
/// Written out here rather than inline in `step` for the same reason
/// [`Outcome`] is: it is the whole semantics of an authored unit policy, and it
/// should be readable in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitOutcome {
    /// Get on with the order. The overwhelmingly common answer.
    Carry,
    /// Break off and pull back to staging. The safety action.
    Withdraw,
    /// Break off for water and resume the same order afterwards.
    Refill,
    /// Stop where you are and stand by.
    Hold,
    /// Drive back to staging and await orders.
    Return,
}

pub fn unit_outcome_of(a: ActionKind) -> UnitOutcome {
    match a {
        ActionKind::Withdraw => UnitOutcome::Withdraw,
        ActionKind::Refill => UnitOutcome::Refill,
        ActionKind::HoldPosition => UnitOutcome::Hold,
        ActionKind::ReturnToBase => UnitOutcome::Return,
        // `Continue`, plus the household half of the enum, which a validated
        // unit graph cannot produce.
        _ => UnitOutcome::Carry,
    }
}

/// `suppression`'s unit kind, as the composer's.
///
/// The two enums are deliberately separate — `behavior` is a leaf crate that
/// knows nothing about the road network or the fire model — and this is the one
/// place they meet, exactly as [`intent_of`] is for households.
pub fn unit_kind_of(k: UnitKind) -> UnitKindKey {
    match k {
        UnitKind::HandCrew => UnitKindKey::HandCrew,
        UnitKind::Engine => UnitKindKey::Engine,
        UnitKind::AirTanker => UnitKindKey::AirTanker,
    }
}

/// One compiled unit profile, ready to run.
pub struct UnitPolicy {
    pub id: String,
    pub name: String,
    /// Which kinds this profile governs. Empty means every kind.
    pub kinds: Vec<UnitKindKey>,
    graph: CompiledGraph,
    scratch: Scratch,
}

/// Everything the suppression model needs to run an authored policy.
///
/// The shape differs from [`BehaviorRuntime`] in one way, and it is the
/// interesting difference: households are assigned a profile by a hash of their
/// id because there are 750 anonymous families, while a unit takes the first
/// profile that names its kind because there are eight named individuals. A
/// hash would make "why did Autobotte 2 do that" unanswerable from the files.
pub struct UnitRuntime {
    policies: Vec<UnitPolicy>,
}

impl UnitRuntime {
    /// Compile every enabled suppression profile.
    ///
    /// Fails as a whole rather than dropping one that will not compile, for the
    /// same reason [`BehaviorRuntime::build`] does: a run with two of three
    /// policies in it is a different experiment.
    pub fn build(lib: &Library) -> Result<Option<UnitRuntime>, RuntimeErrors> {
        let ids = lib.unit_assignment();
        if ids.is_empty() {
            return Ok(None);
        }

        let mut policies = Vec::with_capacity(ids.len());
        let mut errors = Vec::new();
        for id in ids {
            let Some(s) = lib.subtypes.get(&id) else { continue };
            match lib.compile(&id) {
                Ok(graph) => {
                    let scratch = Scratch::new(&graph);
                    policies.push(UnitPolicy {
                        id: id.clone(),
                        name: s.name.clone(),
                        kinds: s.unit_kinds.clone(),
                        graph,
                        scratch,
                    });
                }
                Err(e) => errors.push(e.to_string()),
            }
        }

        if !errors.is_empty() {
            return Err(RuntimeErrors(errors));
        }
        if policies.is_empty() {
            return Ok(None);
        }
        Ok(Some(UnitRuntime { policies }))
    }

    pub fn len(&self) -> usize {
        self.policies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.policies.is_empty()
    }

    pub fn policy(&self, i: usize) -> Option<&UnitPolicy> {
        self.policies.get(i)
    }

    pub fn names(&self) -> impl Iterator<Item = (&str, &str)> {
        self.policies.iter().map(|p| (p.id.as_str(), p.name.as_str()))
    }

    /// Which policy governs a unit of `kind`: the first that names it, or the
    /// first that names none. `None` means this kind is not covered; simulation
    /// construction rejects that incomplete library.
    pub fn assign(&self, kind: UnitKindKey) -> Option<usize> {
        self.policies
            .iter()
            .position(|p| p.kinds.is_empty() || p.kinds.contains(&kind))
    }

    /// Evaluate one unit's policy.
    pub fn decide(&mut self, policy: usize, obs: &Observation) -> Decision {
        let Some(p) = self.policies.get_mut(policy) else { return Decision::default() };
        p.graph.eval_with(obs, &mut p.scratch)
    }

    /// Evaluate with a full trace, for the unit inspector. Slower, and only ever
    /// called for the one unit the player has selected.
    pub fn explain(
        &self,
        policy: usize,
        obs: &Observation,
    ) -> Option<(Decision, behavior::Trace)> {
        let p = self.policies.get(policy)?;
        Some(p.graph.eval_traced(obs))
    }
}
