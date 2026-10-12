//! How a ground unit gets somewhere on the road network: the suppression units
//! ([`crate::suppression`]) and `rocca`'s messenger teams drive by this, so
//! there is one rule for both.
//!
//! - [`network::route`] around the fire, re-planned when the goal moves, every
//!   [`REROUTE_S`], and at once when the fire cuts the link ahead: the vehicle
//!   then turns back to the last node it passed and plans from there, rather
//!   than driving on into the flames until the next periodic re-plan.
//! - The civilian queue on each link costs time
//!   ([`Traffic::emergency_factor`]) without the vehicle taking a place in it.
//! - A barricade by order does not stop it ([`crate::Abm::close_road`]).
//!
//! The vehicle's position stays with its owner, who also moves it by other
//! means (an aircraft arriving, a crew walking the last metres).

use fire::threat::IMPASSABLE;
use fire::ThreatField;
use scenario::Pos;

use crate::network::{self, NodeId, RoadNetwork, NO_NODE};
use crate::traffic::Traffic;

/// A route is kept when the goal moves less than this, metres.
const GOAL_MOVED_M: f32 = 15.0;

/// Ground units re-plan at most this often, so a cut road is noticed within a
/// minute without re-running A* every step — unless it is the link ahead.
pub const REROUTE_S: f32 = 60.0;

/// What a vehicle may drive and how fast.
#[derive(Debug, Clone, Copy)]
pub struct Vehicle {
    /// Carriageways only; otherwise footpaths and tracks too.
    pub drivable_only: bool,
    /// On a carriageway, m/s.
    pub road_speed: f32,
    /// On a footpath or track, m/s.
    pub path_speed: f32,
}

/// Where a vehicle stands after a step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Leg {
    Moving,
    /// At the end of the route, with `spare_s` of the step left.
    Arrived { spare_s: f32 },
    /// Every road to the goal is cut. The vehicle waits where it is, or at
    /// the last node it passed if the fire cut the link it was on.
    NoRoad,
}

#[derive(Debug, Clone)]
pub struct Drive {
    /// The last node passed; [`NO_NODE`] before the first.
    at_node: NodeId,
    /// Nodes still to reach, in order.
    route: Vec<NodeId>,
    /// The goal the route was planned to, and the node it resolved to.
    goal: Option<(Pos, NodeId)>,
    planned_at_s: f32,
    /// Re-plans forced by the fire cutting the link ahead.
    pub cut_ahead: u32,
    /// Metres travelled on the network.
    pub driven_m: f32,
    /// The last plan found no open road.
    stuck: bool,
}

impl Default for Drive {
    fn default() -> Drive {
        Drive { at_node: NO_NODE, route: vec![], goal: None, planned_at_s: f32::NEG_INFINITY, cut_ahead: 0, driven_m: 0.0, stuck: false }
    }
}

impl Drive {
    /// Nodes still to reach.
    pub fn route(&self) -> &[NodeId] {
        &self.route
    }

    /// The last node passed, if any.
    pub fn last_node(&self) -> Option<NodeId> {
        (self.at_node != NO_NODE).then_some(self.at_node)
    }

    /// Forget the route: the next step plans afresh, from the nearest node (a
    /// new order, or the vehicle moved by other means).
    pub fn reset(&mut self) {
        self.at_node = NO_NODE;
        self.route.clear();
        self.goal = None;
        self.planned_at_s = f32::NEG_INFINITY;
    }

    /// Move `pos` for `dt` seconds toward `goal`.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        pos: &mut Pos,
        heading: &mut f32,
        goal: Pos,
        now_s: f32,
        dt: f32,
        how: Vehicle,
        net: &RoadNetwork,
        traffic: &Traffic,
        threat: &ThreatField,
    ) -> Leg {
        // The goal itself may be hot -- that is often where the work is -- so,
        // as in `network::route`, only the way there is refused.
        let cut = self.route.len() > 1 && {
            let np = net.pos(self.route[0]);
            threat.at(np).max(threat.at(mid(*pos, np))) >= IMPASSABLE
        };
        if cut {
            self.cut_ahead += 1;
        }
        let moved = self.goal.is_none_or(|(g, _)| dist(g, goal) > GOAL_MOVED_M);
        let replan = cut || moved || now_s - self.planned_at_s >= REROUTE_S;
        if replan {
            self.stuck = !self.plan(*pos, goal, cut, now_s, how, net, threat);
            if self.stuck && cut && self.at_node != NO_NODE {
                // Nowhere to go, but not on into the fire: back to the last
                // node, and wait there.
                self.route = vec![self.at_node];
            }
        }
        let mut time = dt;
        while time > 0.0 {
            let Some(&next) = self.route.first() else { break };
            let edge = (self.at_node != NO_NODE).then(|| net.neighbours(self.at_node).iter().find(|e| e.to == next)).flatten();
            let jam = edge.map_or(1.0, |e| traffic.emergency_factor(Traffic::link_id(e.id, self.at_node, next)));
            let speed = jam * if edge.is_none_or(|e| e.drivable) { how.road_speed } else { how.path_speed };
            let np = net.pos(next);
            let d = dist(*pos, np);
            if d > speed * time {
                let f = speed * time / d.max(1e-3);
                *heading = (np.y - pos.y).atan2(np.x - pos.x);
                *pos = Pos { x: pos.x + (np.x - pos.x) * f, y: pos.y + (np.y - pos.y) * f };
                self.driven_m += speed * time;
                time = 0.0;
            } else {
                *pos = np;
                self.at_node = next;
                self.route.remove(0);
                self.driven_m += d;
                time -= d / speed.max(1e-3);
            }
        }
        match (self.stuck, self.route.is_empty()) {
            (true, _) => Leg::NoRoad,
            (false, true) => Leg::Arrived { spare_s: time.max(0.0) },
            (false, false) => Leg::Moving,
        }
    }

    /// Whether an open road leads from `pos` to `goal`, without changing course.
    pub fn can_reach(&self, pos: Pos, goal: Pos, how: Vehicle, net: &RoadNetwork, threat: &ThreatField) -> bool {
        let Some(a) = self.start(pos, false, how, net) else { return false };
        net.nearest_reachable(goal, how.drivable_only, a).is_some_and(|b| network::route(net, a, b, threat, how.drivable_only).is_some())
    }

    /// The node a new route starts from: the one ahead, or the one passed when
    /// the way ahead is cut, or the nearest for a vehicle with no route.
    fn start(&self, pos: Pos, back: bool, how: Vehicle, net: &RoadNetwork) -> Option<NodeId> {
        match self.route.first() {
            Some(_) if back && self.at_node != NO_NODE => Some(self.at_node),
            Some(&ahead) if !back => Some(ahead),
            _ => net.nearest(pos, how.drivable_only),
        }
    }

    /// Plan a route to `goal`. False, with no route, if no open road leads
    /// there.
    #[allow(clippy::too_many_arguments)]
    fn plan(&mut self, pos: Pos, goal: Pos, back: bool, now_s: f32, how: Vehicle, net: &RoadNetwork, threat: &ThreatField) -> bool {
        self.planned_at_s = now_s;
        let Some(a) = self.start(pos, back, how, net) else { return false };
        let b = match self.goal {
            Some((g, b)) if dist(g, goal) <= GOAL_MOVED_M => Some(b),
            _ => net.nearest_reachable(goal, how.drivable_only, a),
        };
        let Some(b) = b else { return false };
        self.goal = Some((goal, b));
        let Some(path) = network::route(net, a, b, threat, how.drivable_only) else {
            self.route.clear();
            return false;
        };
        // `route(a, a)` is empty, which reads as "arrived": but a vehicle
        // planning mid-edge is near the node, not at it. Finish the edge --
        // unless it had arrived and left the road on foot, which is not
        // undone by the next re-plan.
        let arrived = path.is_empty() && self.route.is_empty();
        self.route = if dist(pos, net.pos(a)) > 0.5 && !arrived { std::iter::once(a).chain(path).collect() } else { path };
        true
    }
}

fn mid(a: Pos, b: Pos) -> Pos {
    Pos { x: (a.x + b.x) * 0.5, y: (a.y + b.y) * 0.5 }
}

fn dist(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}
