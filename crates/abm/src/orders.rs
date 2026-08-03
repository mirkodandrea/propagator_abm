//! The commander's levers that are not an evacuation order and not a unit task.
//!
//! Two, and they come from opposite ends of the same problem — the road network
//! being the only way out.
//!
//! **A road closure** takes capacity away on purpose. Investigators found the
//! police failed to close the N236 in time at Pedrógão Grande and gave it as a
//! specific reason the toll was as high as it was: traffic kept moving onto a
//! road that was about to be cut, because nobody with the authority to close it
//! did. It is a distinct lever from a fireline, which acts on fuel and not on
//! traffic, and from an evacuation order, which says leave and not which way.
//! The model closed a road only when the fire had already cut it, which is to
//! say it could reproduce the disaster and not the thing that would have
//! prevented it.
//!
//! **A boat lift** adds capacity the road network does not have. Rhodes moved
//! thousands of people off beaches by coastguard and private boat when the roads
//! could not clear the area in time — the largest evacuation in the country's
//! history and the reason that scenario is in the set as the success case. It
//! is asked for and it arrives late, exactly as air support does, because a
//! resource that is instantly available is not a decision.

use scenario::Pos;

/// A stretch of road closed to civilian traffic by order.
///
/// Circular rather than per-way, and that is the honest shape for the decision:
/// a commander closes *an area* to traffic, and which OSM ways that turns out to
/// be is the model's problem rather than theirs.
#[derive(Debug, Clone, Copy)]
pub struct Closure {
    pub centre: Pos,
    pub radius_m: f32,
    /// Simulated time it was ordered.
    pub from_s: f32,
    /// Simulated time it lifts. Infinite for a closure with no end.
    pub until_s: f32,
}

impl Closure {
    pub fn active_at(&self, t: f32) -> bool {
        t >= self.from_s && t < self.until_s
    }

    pub fn contains(&self, p: Pos) -> bool {
        (p.x - self.centre.x).powi(2) + (p.y - self.centre.y).powi(2)
            <= self.radius_m * self.radius_m
    }
}

/// A maritime pickup at the shore.
///
/// One lift, not a fleet: the quantity that matters is how many people per
/// minute leave the beach, and modelling individual hulls would be detail
/// underneath the decision rather than in it.
#[derive(Debug, Clone, Copy)]
pub struct BoatLift {
    pub requested_s: f32,
    /// Simulated time the first boats are on station.
    pub on_station_s: f32,
    /// People taken off per simulated minute once it is working.
    pub rate_per_min: f32,
    /// Carried fraction of a person, so the rate is integrated over simulated
    /// time rather than accumulated per call. Anything accumulated per update
    /// call is a bug (finding 5).
    pub credit: f32,
    /// People taken off so far.
    pub lifted: usize,
}

/// Minutes before requested boats are on station. The same shape as air
/// support's twenty-five, and a little longer: harbours are further away than
/// airfields and a coastguard cutter is slower than a Canadair.
pub const LIFT_DELAY_S: f32 = 30.0 * 60.0;

/// People per simulated minute a lift takes off the beach, at the default
/// request. Two hundred an hour, which is a couple of small vessels working a
/// beach rather than a ferry alongside a quay.
pub const LIFT_RATE_PER_MIN: f32 = 3.5;

impl BoatLift {
    pub fn requested(now_s: f32, delay_s: f32, rate_per_min: f32) -> BoatLift {
        BoatLift {
            requested_s: now_s,
            on_station_s: now_s + delay_s.max(0.0),
            rate_per_min: rate_per_min.max(0.0),
            credit: 0.0,
            lifted: 0,
        }
    }

    pub fn on_station(&self, now_s: f32) -> bool {
        now_s >= self.on_station_s
    }

    /// Minutes until it is working, zero once it is.
    pub fn minutes_out(&self, now_s: f32) -> f32 {
        ((self.on_station_s - now_s) / 60.0).max(0.0)
    }

    /// How many people it can take off over `dt_s`, banking the remainder.
    pub fn capacity(&mut self, now_s: f32, dt_s: f32) -> usize {
        if !self.on_station(now_s) {
            return 0;
        }
        self.credit += self.rate_per_min * dt_s / 60.0;
        let n = self.credit.floor().max(0.0);
        self.credit -= n;
        n as usize
    }
}
