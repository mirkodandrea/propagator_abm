//! What an agent can know.
//!
//! One struct per [`Domain`], and they are the whole contract between the model
//! and an authored graph. A graph can only read fields that appear in its own
//! domain's struct, which is what makes an authored behaviour safe to run:
//! there is no way to write a node that reaches into the fire model, the road
//! network, or another agent.
//!
//! [`Observation`] wraps the two so one evaluator, one registry and one editor
//! serve both. The wrapper is an enum rather than a trait because the node
//! table is static: an eval function is a plain `fn` pointer with no generics,
//! and that is what keeps a decision tick free of allocation and dynamic
//! dispatch. A node asks for the domain it declared, and the validator has
//! already guaranteed it will get it.

use crate::domain::Domain;
use crate::value::{IntentValue, UnitKindKey};

use serde::{Deserialize, Serialize};

#[cfg(feature = "reflect")]
use bevy_reflect::Reflect;

/// One household's view of the incident at one decision tick.
///
/// Every field is a *perceived* quantity, not ground truth, with one exception:
/// [`HouseholdObs::threat`] is the real survivability at the house, because the
/// movement layer needs it to decide who dies and it would be dishonest to let
/// a graph author that away. Everything a household acts on before that point —
/// `cue`, `fire_distance_m`, `warning_received` — is filtered through the
/// household's own attention and warning channel by `abm` before it reaches
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(default)]
pub struct HouseholdObs {
    // --- the incident ----------------------------------------------------
    /// Simulated minutes since the incident started.
    pub time_min: f32,
    /// Survivability at the house, 0–1, from `fire::ThreatField`. 1.0 is "you
    /// cannot stand here". Instantaneous, not integrated.
    pub threat: f32,
    /// Radiant component of the structure's exposure, 0–1.
    pub radiant: f32,
    /// Ember component of the structure's exposure, 0–1. Reaches much further
    /// than the radiant term, and is what destroys houses hours later.
    pub ember: f32,
    /// The house itself has ignited.
    pub structure_alight: bool,
    /// Distance to the nearest burning cell, metres, as coarsely as a person
    /// standing at the house could judge it. Saturates at 2500 m.
    pub fire_distance_m: f32,
    /// The household's accumulated alarm, 0–1: rises fast, decays slowly.
    /// Already scaled by their own risk perception.
    pub cue: f32,

    // --- the commander ----------------------------------------------------
    /// An evacuation order covers this household.
    pub order_issued: bool,
    /// The order has actually arrived over this household's warning channel.
    /// The gap between this and `order_issued` is 90 s to 20 min, and is the
    /// single most under-modelled quantity in evacuation planning.
    pub warning_received: bool,
    /// Minutes since the order was issued, or a large number if none has been.
    pub minutes_since_order: f32,

    // --- the household ----------------------------------------------------
    /// Their stated pre-fire plan.
    pub intent: IntentValue,
    /// How seriously they take wildfire risk, 0–1.
    pub risk_perception: f32,
    /// How much weight they give an official instruction, 0–1.
    pub trust_authority: f32,
    /// Baked milling time, minutes: how long they take to actually get out of
    /// the door once they have decided to.
    pub prep_time_min: f32,
    /// Cleared ground around the property, 0–1. What makes defending survivable.
    pub defensible_space: f32,
    /// People in the household.
    pub household_size: f32,
    /// They have a vehicle available.
    pub has_vehicle: bool,
    /// Someone in the household cannot move unaided.
    pub needs_assistance: bool,

    // --- where they are in the process ------------------------------------
    /// Already milling.
    pub is_preparing: bool,
    /// Already on the road.
    pub is_moving: bool,
    /// Already committed to defending.
    pub is_defending: bool,
    /// No route to any refuge survives the fire.
    pub route_blocked: bool,
    /// Network distance to the nearest reachable refuge, metres. Infinite when
    /// `route_blocked`.
    pub refuge_distance_m: f32,

    /// Per-agent deterministic jitter, 0–1, stable for the life of the run.
    ///
    /// Exposed as an observation rather than left to a random node so an
    /// authored graph cannot become non-reproducible: `abm` hashes the
    /// household id, so the same household gets the same draw whatever the
    /// step size and whoever else moved first.
    pub jitter: f32,
}

impl Default for HouseholdObs {
    fn default() -> Self {
        HouseholdObs {
            time_min: 0.0,
            threat: 0.0,
            radiant: 0.0,
            ember: 0.0,
            structure_alight: false,
            fire_distance_m: 2500.0,
            cue: 0.0,
            order_issued: false,
            warning_received: false,
            minutes_since_order: 1.0e6,
            intent: IntentValue::WaitAndSee,
            risk_perception: 0.5,
            trust_authority: 0.5,
            prep_time_min: 20.0,
            defensible_space: 0.3,
            household_size: 2.0,
            has_vehicle: true,
            needs_assistance: false,
            is_preparing: false,
            is_moving: false,
            is_defending: false,
            route_blocked: false,
            refuge_distance_m: 800.0,
            jitter: 0.5,
        }
    }
}

/// One suppression unit's view of its own situation at one sub-step.
///
/// Deliberately egocentric. A unit knows what it can see from where it is
/// standing, what it is carrying, and what it was told to do — not the shape of
/// the fire, not what the other units are doing, and not where the good targets
/// are. That is the same restriction the household observation carries, and for
/// the same reason: a graph that could read the whole incident state would be a
/// planner, and the planner in this game is the player.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(default)]
pub struct UnitObs {
    /// Simulated minutes since the incident started.
    pub time_min: f32,
    /// What kind of resource this is. The one observation that is a *category*
    /// rather than a quantity, and it is here so one graph can serve all three
    /// kinds — which is what makes a shared safety policy expressible.
    pub kind: UnitKindKey,

    // --- the fire, from where the unit is standing -------------------------
    /// Survivability where the unit is, 0–1, from `fire::ThreatField`. The same
    /// field the civilians read, and the same scale: 0.35 is the firefighters'
    /// stated working limit and 0.55 is impassable.
    pub threat_here: f32,
    /// Accumulated flame exposure as a fraction of what this unit survives.
    /// Reaching 1.0 is being burnt over, which is terminal and is not something
    /// a graph can author away.
    pub heat_fraction: f32,
    /// Metres to the nearest burning cell. Large when there is no fire.
    pub distance_to_fire_m: f32,

    // --- what it is carrying -----------------------------------------------
    /// Tank remaining, 0–1. Always zero for a hand crew.
    pub water_fraction: f32,
    /// This unit carries water at all. False for a hand crew, whose consumable
    /// is daylight.
    pub carries_water: bool,

    // --- the order ---------------------------------------------------------
    /// It has been given something to do beyond standing by.
    pub has_task: bool,
    /// Metres to where the order points. Large when there is no order.
    pub distance_to_task_m: f32,
    /// The ordered point is on a stretch of network this unit can actually
    /// reach. False is the engine parked at the end of the tarmac.
    pub task_reachable: bool,
    /// Fraction of an ordered fuel break already cut, 0–1. Zero for anything
    /// that is not cutting line.
    pub line_progress: f32,
    /// Minutes since this unit was given its current order.
    pub minutes_on_task: f32,
    /// There is something worth working on within this unit's reach: unburnt
    /// burnable fuel for an engine or an aircraft, unfinished line for a crew.
    pub work_available: bool,

    // --- where it is in its own cycle --------------------------------------
    /// Actually working, as opposed to travelling to the job.
    pub is_working: bool,
    /// Travelling.
    pub is_moving: bool,
    /// Metres back to staging.
    pub distance_to_base_m: f32,

    /// Per-agent deterministic jitter, 0–1, stable for the life of the run.
    /// Hashed from the unit id, for the same reason the household's is.
    pub jitter: f32,
}

impl Default for UnitObs {
    fn default() -> Self {
        UnitObs {
            time_min: 0.0,
            kind: UnitKindKey::HandCrew,
            threat_here: 0.0,
            heat_fraction: 0.0,
            distance_to_fire_m: 5000.0,
            water_fraction: 0.0,
            carries_water: false,
            has_task: false,
            distance_to_task_m: 1.0e6,
            task_reachable: true,
            line_progress: 0.0,
            minutes_on_task: 0.0,
            work_available: false,
            is_working: false,
            is_moving: false,
            distance_to_base_m: 0.0,
            jitter: 0.5,
        }
    }
}

/// What one agent knows, whichever kind of agent it is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum Observation {
    Household(HouseholdObs),
    SuppressionUnit(UnitObs),
}

impl Default for Observation {
    fn default() -> Self {
        Observation::Household(HouseholdObs::default())
    }
}

impl Observation {
    pub fn domain(&self) -> Domain {
        match self {
            Observation::Household(_) => Domain::Household,
            Observation::SuppressionUnit(_) => Domain::SuppressionUnit,
        }
    }

    /// An empty observation of `domain`, for the test bench and for a node that
    /// has nothing to read yet.
    pub fn empty(domain: Domain) -> Observation {
        match domain {
            Domain::Household => Observation::Household(HouseholdObs::default()),
            Domain::SuppressionUnit => Observation::SuppressionUnit(UnitObs::default()),
        }
    }

    /// The household view.
    ///
    /// Returns the default when this is not a household observation, which the
    /// validator guarantees cannot happen in a graph that compiled: a node
    /// declaring `domain: Household` is an error in any other graph. Returning
    /// a default rather than panicking keeps a half-built graph in the editor
    /// evaluable, which is the same choice [`crate::Inputs`] makes for a
    /// dangling port.
    pub fn household(&self) -> HouseholdObs {
        match self {
            Observation::Household(h) => *h,
            _ => HouseholdObs::default(),
        }
    }

    pub fn unit(&self) -> UnitObs {
        match self {
            Observation::SuppressionUnit(u) => *u,
            _ => UnitObs::default(),
        }
    }

    pub fn household_mut(&mut self) -> Option<&mut HouseholdObs> {
        match self {
            Observation::Household(h) => Some(h),
            _ => None,
        }
    }

    pub fn unit_mut(&mut self) -> Option<&mut UnitObs> {
        match self {
            Observation::SuppressionUnit(u) => Some(u),
            _ => None,
        }
    }
}

impl From<HouseholdObs> for Observation {
    fn from(h: HouseholdObs) -> Observation {
        Observation::Household(h)
    }
}

impl From<UnitObs> for Observation {
    fn from(u: UnitObs) -> Observation {
        Observation::SuppressionUnit(u)
    }
}
