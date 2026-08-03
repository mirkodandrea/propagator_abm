//! Vehicles as a **spatial queue** on the road graph.
//!
//! The thing an evacuation traffic model has to produce is a *queue*: demand
//! arrives at a bottleneck faster than the bottleneck discharges, a line of
//! stationary cars forms behind it, and that line grows backwards until it
//! blocks the junctions feeding it. Everything a commander can see about
//! traffic — the exit that saturates, the street that stops moving, the family
//! that left too late — is a consequence of those three sentences.
//!
//! The model this replaced had none of them. It scaled each car's own speed by
//! how many cars shared its link (`speed *= 1/(1 + 0.06·(n-1))`) and left them
//! otherwise independent: cars interpenetrated, nobody blocked anybody, and the
//! discharge rate of a road was unbounded. It also counted *vehicles per link*
//! on a graph whose links are OSM polyline segments — **8 m at the median on
//! Spotorno** — so three cars nose to tail on one segment, which is a standstill,
//! read as an 11% slowdown, while the coefficient needed 18 cars on one link to
//! halve speed and 95 to reach its floor. On the real window the term was inert
//! by construction; on the synthetic labs, whose links are 320–633 m, it fired
//! where the road was empty. See the module tests for what each of those is now.
//!
//! # The model
//!
//! Each **directed link** — one per direction of travel over an undirected
//! graph edge, because a northbound queue is not a southbound one — carries
//! three constraints:
//!
//! | | | |
//! |---|---|---|
//! | **Free-flow time** | `length / speed` | a car cannot reach the stop line early |
//! | **Flow capacity** | veh/s, from the road class | a link cannot *discharge* faster than this |
//! | **Storage capacity** | `length · lanes / [`JAM_SPACING_M`]` | a link cannot *hold* more than this |
//!
//! A vehicle leaves a link only when it is at the head of that link's queue,
//! has physically reached the end of it, the link's discharge gate has come
//! round, **and the link it wants next has room**. That last clause is the whole
//! point: it is what makes a queue spill back through a junction instead of
//! evaporating, and it is what a per-car speed multiplier cannot express at any
//! coefficient.
//!
//! Capacities in series do not compound — a chain of links all rated 800 veh/h
//! carries 800 veh/h, not less — which is why applying this to 8 m OSM segments
//! is sound rather than absurd. Storage on those segments is one vehicle, which
//! is the physically right answer: 8 m of one lane holds one stopped car. The
//! graph is, by coincidence of OSM's vertex density, already close to the 7.5 m
//! cell of a Nagel–Schreckenberg lattice.
//!
//! # Two properties this has to keep
//!
//! **Step-size invariance.** Nothing here is accumulated per call (finding 5).
//! A vehicle's position on its link is a pure function of the clock, the time it
//! entered, and how many cars are ahead of it — not a running total — and a
//! link's discharge is event-timed (`next_release_s`) rather than credited per
//! step, so `capacity · elapsed` vehicles leave over a given interval whatever
//! the caller's step size. This is also why the model is *not* car-following:
//! an IDM or Gipps integration is only stable at sub-second steps and is not
//! step-size invariant at any of them, and the queue is the phenomenon we are
//! after, not the acceleration profile.
//!
//! **Determinism.** Vehicles are processed in `(link, ticket)` order, tickets
//! are issued in arrival order, and there is no randomness anywhere.

use crate::network::{NodeId, RoadNetwork};

/// A vehicle is not on any link: crossing open ground to the road, waiting at a
/// junction for room, or no longer travelling.
pub const NO_LINK: u32 = u32::MAX;

/// Metres of a single lane one stationary vehicle occupies, bumper to bumper.
/// A car is ~4.5 m and drivers leave ~2.5 m at a standstill.
pub const JAM_SPACING_M: f32 = 7.0;

/// How much of their speed emergency vehicles keep on a jammed link.
///
/// They are deliberately **not** in the queue: an engine on blue lights uses
/// the oncoming lane, the shoulder and the pavement, and civilian traffic pulls
/// over for it. But it does not get through a solid line of cars at road speed
/// either, so a fully jammed link costs it this fraction of its pace and an
/// empty one costs it nothing. One number, because anything finer would be
/// invented rather than measured.
const EMERGENCY_JAM_FLOOR: f32 = 0.35;

/// How a road is classified for traffic purposes. Derived from the OSM
/// `highway` tag the bake already carries and, until now, threw away at
/// [`RoadNetwork::build`] — the graph knew only whether a way was drivable, so
/// the A10 and a farm service track had the same capacity and the same speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RoadClass {
    Motorway = 0,
    Trunk = 1,
    Primary = 2,
    Secondary = 3,
    Tertiary = 4,
    Residential = 5,
    Unclassified = 6,
    Service = 7,
    LivingStreet = 8,
    /// A slip road off any of the above. Short, single lane, and the thing that
    /// actually meters a motorway junction.
    Ramp = 9,
    /// Anything not drivable, or a tag we do not model. Never used by a
    /// vehicle; present so every edge has an entry.
    Other = 10,
}

impl RoadClass {
    /// Classify an OSM `highway` value. Unknown drivable tags fall to
    /// [`RoadClass::Unclassified`] rather than to `Other`, because a way the
    /// bake marked drivable is one vehicles will be routed onto and it needs a
    /// capacity that is merely approximate, not zero.
    pub fn from_osm(tag: &str, drivable: bool) -> RoadClass {
        if tag.ends_with("_link") {
            return if drivable { RoadClass::Ramp } else { RoadClass::Other };
        }
        match tag {
            "motorway" => RoadClass::Motorway,
            "trunk" => RoadClass::Trunk,
            "primary" => RoadClass::Primary,
            "secondary" => RoadClass::Secondary,
            "tertiary" => RoadClass::Tertiary,
            "residential" => RoadClass::Residential,
            "unclassified" => RoadClass::Unclassified,
            "service" => RoadClass::Service,
            "living_street" => RoadClass::LivingStreet,
            _ if drivable => RoadClass::Unclassified,
            _ => RoadClass::Other,
        }
    }

    /// Free-flow speed (m/s), lanes **per direction**, and saturation flow
    /// (veh/h per lane).
    ///
    /// The speeds are for a Ligurian coast road rather than for a design manual:
    /// this is a hill town of hairpins and parked cars, and the 40 km/h the old
    /// single global constant used is about right for its residential streets.
    /// What the old constant could not say is that the A10 is not one of them.
    /// Saturation flows are the standard highway-capacity range — ~1,900 veh/h
    /// per lane on a motorway, falling to a few hundred on a service road where
    /// a single reversing car stops everything.
    pub fn params(self) -> (f32, f32, f32) {
        match self {
            RoadClass::Motorway => (27.8, 2.0, 1900.0),
            RoadClass::Trunk => (22.2, 2.0, 1800.0),
            RoadClass::Primary => (16.7, 1.0, 1600.0),
            RoadClass::Secondary => (13.9, 1.0, 1400.0),
            RoadClass::Tertiary => (12.5, 1.0, 1200.0),
            RoadClass::Residential => (8.3, 1.0, 800.0),
            RoadClass::Unclassified => (9.7, 1.0, 800.0),
            RoadClass::Service => (5.6, 1.0, 400.0),
            RoadClass::LivingStreet => (4.2, 1.0, 300.0),
            RoadClass::Ramp => (13.9, 1.0, 1400.0),
            // Never driven; give it something finite so no arithmetic divides
            // by zero if a caller asks anyway.
            RoadClass::Other => (1.4, 1.0, 100.0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RoadClass::Motorway => "motorway",
            RoadClass::Trunk => "trunk",
            RoadClass::Primary => "primary",
            RoadClass::Secondary => "secondary",
            RoadClass::Tertiary => "tertiary",
            RoadClass::Residential => "residential",
            RoadClass::Unclassified => "unclassified",
            RoadClass::Service => "service",
            RoadClass::LivingStreet => "living street",
            RoadClass::Ramp => "slip road",
            RoadClass::Other => "not drivable",
        }
    }

    pub fn from_u8(v: u8) -> RoadClass {
        match v {
            0 => RoadClass::Motorway,
            1 => RoadClass::Trunk,
            2 => RoadClass::Primary,
            3 => RoadClass::Secondary,
            4 => RoadClass::Tertiary,
            5 => RoadClass::Residential,
            6 => RoadClass::Unclassified,
            7 => RoadClass::Service,
            8 => RoadClass::LivingStreet,
            9 => RoadClass::Ramp,
            _ => RoadClass::Other,
        }
    }
}

/// The live state of every directed link, plus the static capacities it is
/// measured against.
///
/// Static parameters are per **undirected** edge (both directions of a street
/// have the same class and the same width); live state is per **directed**
/// link, `2·edge + dir`.
pub struct Traffic {
    // --- static, per undirected edge ---
    free_speed: Vec<f32>,
    /// Vehicles per second this link can discharge, per direction.
    capacity: Vec<f32>,
    /// Vehicles this link can hold, per direction. Never zero: a link shorter
    /// than one vehicle still has to be able to have that vehicle on it, or a
    /// car reaching it would have nowhere to be and the network would deadlock.
    storage: Vec<u16>,
    /// Metres a queued vehicle takes off the link, i.e. [`JAM_SPACING_M`] over
    /// the lane count. Precomputed because the position of every stationary car
    /// is derived from it every sub-step.
    spacing: Vec<f32>,

    // --- live, per directed link ---
    count: Vec<u16>,
    /// Monotonic per link: the order vehicles joined its queue.
    ///
    /// A vehicle's *rank* is deliberately **not** derived from this by
    /// subtracting a served counter, which is the obvious O(1) trick and is
    /// wrong here: a car can leave a queue from the middle — burnt over in it,
    /// or turned round by a `last_resort` branch — and a served counter can
    /// only ever describe departures from the front. Rank is recomputed from
    /// the FIFO order every sub-step instead, which is one pass over the
    /// vehicles and cannot be desynchronised by a removal.
    issued: Vec<u32>,
    /// Simulated time this link may next discharge a vehicle.
    next_release_s: Vec<f32>,
}

impl Traffic {
    pub fn new(net: &RoadNetwork) -> Traffic {
        let n = net.edge_count;
        let mut t = Traffic {
            free_speed: Vec::with_capacity(n),
            capacity: Vec::with_capacity(n),
            storage: Vec::with_capacity(n),
            spacing: Vec::with_capacity(n),
            count: vec![0; n * 2],
            issued: vec![0; n * 2],
            next_release_s: vec![f32::NEG_INFINITY; n * 2],
        };
        for e in 0..n as u32 {
            let (speed, lanes, sat) = net.edge_class(e).params();
            let len = net.edge_len(e);
            t.free_speed.push(speed);
            t.capacity.push(lanes * sat / 3600.0);
            t.storage.push(((len * lanes / JAM_SPACING_M).floor() as u32).max(1).min(u16::MAX as u32) as u16);
            t.spacing.push(JAM_SPACING_M / lanes);
        }
        t
    }

    /// The directed link a vehicle occupies when travelling `from` -> `to` over
    /// undirected edge `edge`. Direction 0 is the low node id to the high one,
    /// which is arbitrary and only has to be consistent.
    pub fn link_id(edge: u32, from: NodeId, to: NodeId) -> u32 {
        edge * 2 + u32::from(from > to)
    }

    pub fn edge_of(link: u32) -> u32 {
        link / 2
    }

    /// Free-flow speed on this link, m/s.
    pub fn speed(&self, link: u32) -> f32 {
        self.free_speed[(link / 2) as usize]
    }

    /// Metres of link one queued vehicle consumes.
    pub fn spacing(&self, link: u32) -> f32 {
        self.spacing[(link / 2) as usize]
    }

    pub fn storage(&self, link: u32) -> u16 {
        self.storage[(link / 2) as usize]
    }

    pub fn capacity(&self, link: u32) -> f32 {
        self.capacity[(link / 2) as usize]
    }

    pub fn count(&self, link: u32) -> u16 {
        self.count[link as usize]
    }

    /// Is there room for one more vehicle on this link?
    pub fn has_room(&self, link: u32) -> bool {
        self.count[link as usize] < self.storage(link)
    }

    /// Put a vehicle on the link and hand it its place in the arrival order.
    /// Returns the ticket and how many vehicles are now on the link, so the
    /// caller can seat the newcomer at the back without a second lookup.
    pub fn enter(&mut self, link: u32) -> (u32, u16) {
        let n = self.count[link as usize].saturating_add(1);
        self.count[link as usize] = n;
        let ticket = self.issued[link as usize];
        self.issued[link as usize] = ticket.wrapping_add(1);
        (ticket, n)
    }

    /// Take a vehicle off the link. Valid from anywhere in the queue, not only
    /// the head — see the note on `issued`.
    pub fn leave(&mut self, link: u32) {
        self.count[link as usize] = self.count[link as usize].saturating_sub(1);
    }

    /// The earliest time this link may discharge its next vehicle.
    pub fn release_gate(&self, link: u32) -> f32 {
        self.next_release_s[link as usize]
    }

    /// Record that the link discharged a vehicle at `at_s`, and set when it may
    /// discharge the next one.
    ///
    /// Anchored on the *later* of the release and the gate it had, so a link
    /// that has stood empty for an hour does not bank an hour of credit and
    /// then release a whole queue at once — a mistake that would make the
    /// bottleneck's throughput depend on how long it had been idle.
    pub fn took_slot(&mut self, link: u32, at_s: f32) {
        let cap = self.capacity(link).max(1e-4);
        let base = self.next_release_s[link as usize].max(at_s);
        self.next_release_s[link as usize] = base + 1.0 / cap;
    }

    /// Fraction of this link's storage in use, 0-1.
    pub fn saturation(&self, link: u32) -> f32 {
        self.count[link as usize] as f32 / self.storage(link).max(1) as f32
    }

    /// What a jammed link does to a vehicle that is **not** in the queue —
    /// an engine or a crew truck on blue lights. See [`EMERGENCY_JAM_FLOOR`].
    pub fn emergency_factor(&self, link: u32) -> f32 {
        let s = self.saturation(link).clamp(0.0, 1.0);
        1.0 - (1.0 - EMERGENCY_JAM_FLOOR) * s
    }

    /// Reset every live counter, keeping the static capacities. Nothing calls
    /// this today — a restart rebuilds the whole [`crate::Abm`] — but a queue
    /// left populated by vehicles that no longer exist is the exact shape of
    /// finding 21, so the way to clear it exists and is one call.
    pub fn clear(&mut self) {
        self.count.iter_mut().for_each(|c| *c = 0);
        self.issued.iter_mut().for_each(|c| *c = 0);
        self.next_release_s.iter_mut().for_each(|c| *c = f32::NEG_INFINITY);
    }

    /// Directed links carrying at least one vehicle, for reporting. Linear in
    /// the network, so this is a diagnostic rather than something to call per
    /// frame.
    pub fn occupied_links(&self) -> impl Iterator<Item = (u32, u16)> + '_ {
        self.count
            .iter()
            .enumerate()
            .filter(|(_, &c)| c > 0)
            .map(|(i, &c)| (i as u32, c))
    }

    /// The worst queue anywhere: the directed link with the most vehicles, and
    /// how full it is. The headline number for "is there congestion".
    pub fn busiest(&self) -> Option<(u32, u16, f32)> {
        self.occupied_links()
            .max_by_key(|&(_, c)| c)
            .map(|(l, c)| (l, c, self.saturation(l)))
    }

    /// Vehicles sitting on links that are full. This is *queueing* rather than
    /// *driving*, and it is the quantity the old model could not produce at all.
    pub fn queued_vehicles(&self) -> usize {
        self.occupied_links()
            .filter(|&(l, c)| c >= self.storage(l))
            .map(|(_, c)| c as usize)
            .sum()
    }
}
