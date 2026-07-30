//! The road network as a graph, and the evacuation routing on top of it.
//!
//! Agents do not move across open ground toward a bearing; they move along the
//! real OSM network, which is the whole reason the drivable/track split was
//! kept in [`scenario::vectors`]. An engine and a family car are confined to
//! drivable ways; a person on foot can also use tracks, paths and steps.
//!
//! Routing is deliberately **not** per-agent pathfinding. Every agent wants
//! the same thing -- the nearest reachable refuge -- so one multi-source
//! Dijkstra from all refuges gives every node in the network its distance to
//! safety and its next hop toward it. That is a single graph search per
//! refresh instead of 750, and it makes rerouting around a road the fire has
//! cut a property of the field rather than something each agent has to
//! discover. Two fields are kept, because the two travel modes see different
//! graphs.

use std::collections::BinaryHeap;

use scenario::{Pos, Scenario};

/// Snap tolerance when welding way vertices into shared nodes, in metres.
/// OSM ways that meet at a junction share a node exactly, so this only has to
/// absorb the two-decimal rounding in the baked file.
const SNAP_M: f32 = 0.05;

/// Bucket edge for the nearest-node lookup, metres.
const BUCKET_M: f32 = 100.0;

pub type NodeId = u32;
pub const NO_NODE: NodeId = u32::MAX;

#[derive(Debug, Clone, Copy)]
pub struct Edge {
    pub to: NodeId,
    pub length_m: f32,
    /// A vehicle can use this.
    pub drivable: bool,
    /// Index into the network's edge list, for congestion accounting.
    pub id: u32,
}

pub struct RoadNetwork {
    pub nodes: Vec<Pos>,
    /// Ground elevation at each node, so agents follow the terrain.
    pub elev: Vec<f32>,
    adjacency: Vec<Vec<Edge>>,
    pub edge_count: usize,
    buckets: Vec<Vec<NodeId>>,
    bcols: usize,
    brows: usize,
}

impl RoadNetwork {
    /// Build from the scenario's OSM ways. Footways, steps and tracks are
    /// included: they are how people leave when the road is blocked.
    pub fn build(scn: &Scenario) -> RoadNetwork {
        let w = scn.world;
        let bcols = (w.width_m / BUCKET_M).ceil() as usize + 1;
        let brows = (w.height_m / BUCKET_M).ceil() as usize + 1;

        let mut net = RoadNetwork {
            nodes: Vec::new(),
            elev: Vec::new(),
            adjacency: Vec::new(),
            edge_count: 0,
            buckets: vec![Vec::new(); brows * bcols],
            bcols,
            brows,
        };

        // Weld coincident vertices by exact key. Ways that cross without a
        // shared node are genuinely not connected in OSM either -- that is a
        // bridge or an underpass, and pretending otherwise would route people
        // through a viaduct pier.
        let mut index: std::collections::HashMap<(i64, i64), NodeId> =
            std::collections::HashMap::new();
        let key = |p: [f32; 2]| {
            (
                (p[0] / SNAP_M).round() as i64,
                (p[1] / SNAP_M).round() as i64,
            )
        };

        for road in &scn.vectors.roads {
            if road.line.len() < 2 {
                continue;
            }
            let drivable = road.drivable;
            let mut prev: Option<NodeId> = None;
            for v in &road.line {
                let k = key(*v);
                let id = *index.entry(k).or_insert_with(|| {
                    let p = Pos { x: v[0], y: v[1] };
                    net.nodes.push(p);
                    net.elev.push(scn.terrain.height_at(p));
                    net.adjacency.push(Vec::new());
                    (net.nodes.len() - 1) as NodeId
                });
                if let Some(a) = prev {
                    if a != id {
                        net.add_edge(a, id, drivable);
                    }
                }
                prev = Some(id);
            }
        }

        for (i, p) in net.nodes.iter().enumerate() {
            let bx = (p.x / BUCKET_M).max(0.0) as usize;
            let by = (p.y / BUCKET_M).max(0.0) as usize;
            if bx < bcols && by < brows {
                net.buckets[by * bcols + bx].push(i as NodeId);
            }
        }
        net
    }

    fn add_edge(&mut self, a: NodeId, b: NodeId, drivable: bool) {
        let pa = self.nodes[a as usize];
        let pb = self.nodes[b as usize];
        let len = ((pb.x - pa.x).powi(2) + (pb.y - pa.y).powi(2)).sqrt().max(0.1);
        let id = self.edge_count as u32;
        self.edge_count += 1;
        self.adjacency[a as usize].push(Edge { to: b, length_m: len, drivable, id });
        self.adjacency[b as usize].push(Edge { to: a, length_m: len, drivable, id });
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn pos(&self, n: NodeId) -> Pos {
        self.nodes[n as usize]
    }

    pub fn neighbours(&self, n: NodeId) -> &[Edge] {
        &self.adjacency[n as usize]
    }

    /// True if any way at this node can carry a vehicle.
    pub fn is_drivable_node(&self, n: NodeId) -> bool {
        self.adjacency[n as usize].iter().any(|e| e.drivable)
    }

    /// Nearest node to a position, optionally restricted to drivable ways.
    /// Searches outward by bucket ring so a house up a footpath still finds
    /// the network it is actually on.
    pub fn nearest(&self, p: Pos, drivable_only: bool) -> Option<NodeId> {
        let bx = (p.x / BUCKET_M).max(0.0) as isize;
        let by = (p.y / BUCKET_M).max(0.0) as isize;
        let mut best = None;
        let mut best_d = f32::MAX;

        for ring in 0..=25isize {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    // Only the shell of the ring is new.
                    if ring > 0 && dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    let (nx, ny) = (bx + dx, by + dy);
                    if nx < 0 || ny < 0 || nx as usize >= self.bcols || ny as usize >= self.brows
                    {
                        continue;
                    }
                    for &n in &self.buckets[ny as usize * self.bcols + nx as usize] {
                        if drivable_only && !self.is_drivable_node(n) {
                            continue;
                        }
                        let q = self.nodes[n as usize];
                        let d = (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
                        if d < best_d {
                            best_d = d;
                            best = Some(n);
                        }
                    }
                }
            }
            // One extra ring after the first hit, since a nearer node can sit
            // in a diagonal bucket.
            if best.is_some() && (ring as f32 * BUCKET_M).powi(2) > best_d {
                break;
            }
        }
        best
    }
}

/// Where safety is, and how to get there from anywhere on the network.
///
/// `cost` is minutes-equivalent travel time to the nearest refuge, with fire
/// danger folded in as a penalty rather than as a hard block wherever
/// possible, so a route that is merely unpleasant still beats no route at all.
pub struct RouteField {
    /// Travel cost to the nearest refuge, in seconds. `f32::INFINITY` where
    /// no route exists.
    pub cost: Vec<f32>,
    /// Next node on the way to that refuge, or [`NO_NODE`].
    pub next: Vec<NodeId>,
}

impl RouteField {
    fn empty(n: usize) -> RouteField {
        RouteField { cost: vec![f32::INFINITY; n], next: vec![NO_NODE; n] }
    }

    pub fn reachable(&self, n: NodeId) -> bool {
        self.cost[n as usize].is_finite()
    }
}

/// Free-travel speeds, m/s. Foot speed is per-person; these are the network
/// speeds used for *routing* cost, which only needs to rank routes.
const CAR_SPEED: f32 = 11.0;
const FOOT_SPEED: f32 = 1.3;

/// How much a dangerous edge is inflated in routing cost. High enough that a
/// long way round beats driving past the flaming front, low enough that a
/// slightly smoky road is still used.
const DANGER_PENALTY: f32 = 40.0;

/// Danger above which an edge is dropped from routing entirely.
const DANGER_CUT: f32 = fire::threat::IMPASSABLE;

/// Multi-source Dijkstra from every refuge, once per mode.
pub fn solve(
    net: &RoadNetwork,
    refuges: &[NodeId],
    threat: &fire::ThreatField,
    drivable_only: bool,
) -> RouteField {
    let n = net.len();
    let mut field = RouteField::empty(n);
    let mut heap: BinaryHeap<Entry> = BinaryHeap::new();

    for &r in refuges {
        if drivable_only && !net.is_drivable_node(r) {
            continue;
        }
        // A refuge the fire has already reached is not a refuge.
        if threat.at(net.pos(r)) >= DANGER_CUT {
            continue;
        }
        field.cost[r as usize] = 0.0;
        heap.push(Entry { cost: 0.0, node: r });
    }

    let speed = if drivable_only { CAR_SPEED } else { FOOT_SPEED };

    while let Some(Entry { cost, node }) = heap.pop() {
        if cost > field.cost[node as usize] {
            continue;
        }
        for e in net.neighbours(node) {
            if drivable_only && !e.drivable {
                continue;
            }
            let mid = midpoint(net.pos(node), net.pos(e.to));
            let danger = threat.at(mid).max(threat.at(net.pos(e.to)));
            if danger >= DANGER_CUT {
                continue;
            }
            let step = e.length_m / speed * (1.0 + DANGER_PENALTY * danger);
            let next = cost + step;
            if next < field.cost[e.to as usize] {
                field.cost[e.to as usize] = next;
                // Edges are undirected, so the predecessor in the search *is*
                // the next hop toward the refuge.
                field.next[e.to as usize] = node;
                heap.push(Entry { cost: next, node: e.to });
            }
        }
    }
    field
}

fn midpoint(a: Pos, b: Pos) -> Pos {
    Pos { x: (a.x + b.x) * 0.5, y: (a.y + b.y) * 0.5 }
}

/// Min-heap entry. `BinaryHeap` is a max-heap and `f32` is not `Ord`, so the
/// ordering is reversed and NaN-free costs are compared by `partial_cmp`.
struct Entry {
    cost: f32,
    node: NodeId,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}
impl Eq for Entry {}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
