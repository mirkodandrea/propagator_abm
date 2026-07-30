//! Where people are trying to get to.
//!
//! Refuges are **measured, not authored**. Picking them by eye off a map would
//! have been quicker, but the same mistake was made four times over with
//! ignition placement (`fire::ignition`) before measurement settled it, and
//! the failure mode here is identical: a plausible-looking assembly point that
//! sits in continuous macchia is a death trap the model would happily route
//! everyone into.
//!
//! So a refuge is a road node with two properties, both read off the data:
//!
//! - **it is not in the fuel.** Fewer than [`MAX_BURNABLE_FRAC`] of the cells
//!   within [`CLEAR_RADIUS_M`] are burnable. On this window that selects the
//!   waterfront, the dense town core, the port and the larger car parks --
//!   which is exactly where Ligurian coastal evacuations actually assemble.
//! - **it is reachable by vehicle**, since the assembly points that matter are
//!   the ones an engine or a bus can also reach.
//!
//! Leaving the scenario window on a drivable road counts as evacuated too:
//! the A10 and the Aurelia both run through it, and someone who drives out of
//! a 10 km window in an initial attack has left the incident.

use scenario::{Cell, Pos, Scenario};

use crate::network::{NodeId, RoadNetwork};

/// Radius searched for burnable fuel around a candidate, metres.
const CLEAR_RADIUS_M: f32 = 300.0;
/// Fraction of that neighbourhood allowed to be burnable.
const MAX_BURNABLE_FRAC: f32 = 0.12;
/// Minimum spacing between chosen refuges, metres -- otherwise the whole set
/// lands in one car park.
const MIN_SPACING_M: f32 = 600.0;
/// Nodes this close to the window edge are exits.
const EDGE_M: f32 = 120.0;

#[derive(Debug, Clone, Copy)]
pub struct Refuge {
    pub node: NodeId,
    pub pos: Pos,
    /// True for a map-edge exit rather than an in-town assembly point.
    pub is_exit: bool,
    /// Burnable fraction of the surrounding neighbourhood, 0-1.
    pub burnable_frac: f32,
}

/// Choose refuges for a scenario. Deterministic: the same scenario always
/// yields the same set, in the same order.
pub fn choose(scn: &Scenario, net: &RoadNetwork, max: usize) -> Vec<Refuge> {
    let w = scn.world;
    let mut candidates: Vec<Refuge> = Vec::new();

    for (i, &p) in net.nodes.iter().enumerate() {
        let n = i as NodeId;
        if !net.is_drivable_node(n) {
            continue;
        }
        let is_exit = p.x < EDGE_M
            || p.y < EDGE_M
            || p.x > w.width_m - EDGE_M
            || p.y > w.height_m - EDGE_M;
        let frac = burnable_fraction(scn, p, CLEAR_RADIUS_M);
        if is_exit || frac <= MAX_BURNABLE_FRAC {
            candidates.push(Refuge { node: n, pos: p, is_exit, burnable_frac: frac });
        }
    }

    // Cleanest first, so the greedy spacing keeps the best of each cluster.
    candidates.sort_by(|a, b| {
        a.burnable_frac
            .partial_cmp(&b.burnable_frac)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.node.cmp(&b.node))
    });

    let mut chosen: Vec<Refuge> = Vec::new();
    for c in candidates {
        if chosen.len() >= max {
            break;
        }
        let clash = chosen.iter().any(|r| {
            (r.pos.x - c.pos.x).powi(2) + (r.pos.y - c.pos.y).powi(2)
                < MIN_SPACING_M * MIN_SPACING_M
        });
        if !clash {
            chosen.push(c);
        }
    }
    chosen
}

/// Fraction of fire cells within `radius` of `p` that carry burnable fuel.
pub fn burnable_fraction(scn: &Scenario, p: Pos, radius: f32) -> f32 {
    let w = scn.world;
    let c = w.cell_of(p);
    let span = (radius / w.cellsize).ceil() as i64;
    let (mut burnable, mut total) = (0u32, 0u32);
    for dr in -span..=span {
        for dc in -span..=span {
            if (dr * dr + dc * dc) as f32 > (span * span) as f32 {
                continue;
            }
            let (r, col) = (c.row as i64 + dr, c.col as i64 + dc);
            if r < 0 || col < 0 || r >= w.fire_rows as i64 || col >= w.fire_cols as i64 {
                continue;
            }
            total += 1;
            if scn.is_burnable(Cell { row: r as usize, col: col as usize }) {
                burnable += 1;
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        burnable as f32 / total as f32
    }
}
