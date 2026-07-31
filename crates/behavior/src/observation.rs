//! What an agent can know.
//!
//! This struct is the whole contract between the model and an authored graph.
//! A graph can only read fields that appear here, which is what makes an
//! authored behaviour safe to run: there is no way to write a node that
//! reaches into the fire model, the road network, or another household.
//!
//! Every field is a *perceived* quantity, not ground truth, with one
//! exception: [`Observation::threat`] is the real survivability at the house,
//! because the movement layer needs it to decide who dies and it would be
//! dishonest to let a graph author that away. Everything a household acts on
//! before that point — `cue`, `fire_distance_m`, `warning_received` — is
//! filtered through the household's own attention and warning channel by
//! `abm` before it reaches here.

use crate::value::IntentValue;

use serde::{Deserialize, Serialize};

#[cfg(feature = "reflect")]
use bevy_reflect::Reflect;

/// One household's view of the incident at one decision tick.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
#[serde(default)]
pub struct Observation {
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

impl Default for Observation {
    fn default() -> Self {
        Observation {
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
