//! The hexagonal grid the teams are moved on (docs/01-GIOCO.md §3): pointy-top
//! hexes [`HEX_M`] apart, in axial coordinates. A hex is a destination only if
//! a drivable road of the main network crosses it; the team then drives to
//! the road node of the hex nearest its centre ([`Hex::target`]).

use abm::network::{NodeId, RoadNetwork};
use scenario::{Pos, Scenario};

/// Distance between the centres of two neighbouring hexes (across the flats),
/// metres. User, 2026-10-14: 500 m.
pub const HEX_M: f32 = 500.0;

/// Centre to corner, metres.
pub fn radius_m() -> f32 {
    HEX_M / 3f32.sqrt()
}

/// Axial coordinates of a hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Axial {
    pub q: i32,
    pub r: i32,
}

impl Axial {
    pub fn centre(self) -> Pos {
        Pos { x: HEX_M * (self.q as f32 + self.r as f32 * 0.5), y: HEX_M * 3f32.sqrt() / 2.0 * self.r as f32 }
    }

    /// The hex containing `p`.
    pub fn of(p: Pos) -> Axial {
        let r = p.y / (HEX_M * 3f32.sqrt() / 2.0);
        let q = p.x / HEX_M - r * 0.5;
        // cube rounding
        let (x, z) = (q, r);
        let y = -x - z;
        let (mut rx, ry, mut rz) = (x.round(), y.round(), z.round());
        let (dx, dy, dz) = ((rx - x).abs(), (ry - y).abs(), (rz - z).abs());
        if dx > dy && dx > dz {
            rx = -ry - rz;
        } else if dy <= dz {
            rz = -rx - ry;
        }
        Axial { q: rx as i32, r: rz as i32 }
    }

    /// The six corners, for drawing.
    pub fn corners(self) -> [Pos; 6] {
        let c = self.centre();
        let s = radius_m();
        std::array::from_fn(|k| {
            let a = std::f32::consts::PI / 180.0 * (60.0 * k as f32 - 30.0);
            Pos { x: c.x + s * a.cos(), y: c.y + s * a.sin() }
        })
    }

    pub fn distance(self, o: Axial) -> i32 {
        let (dq, dr) = (self.q - o.q, self.r - o.r);
        (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
    }
}

/// A hex a team can be sent to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hex {
    pub at: Axial,
    /// The road node the team drives to.
    pub node: NodeId,
    pub target: Pos,
}

/// Every hex of the island a team can be sent to.
#[derive(Debug, Clone, Default)]
pub struct HexGrid {
    pub hexes: Vec<Hex>,
}

impl HexGrid {
    /// The hexes crossed by a drivable road of the network component `home`
    /// is on (the one the stations are on).
    pub fn build(scn: &Scenario, net: &RoadNetwork, home: NodeId) -> HexGrid {
        let want = net.component(home, true);
        let mut best: std::collections::BTreeMap<Axial, (f32, NodeId)> = Default::default();
        for (i, &p) in net.nodes.iter().enumerate() {
            let n = i as NodeId;
            if !net.is_drivable_node(n) || net.component(n, true) != want || !scn.world.contains(p) {
                continue;
            }
            let at = Axial::of(p);
            let d = dist(p, at.centre());
            let e = best.entry(at).or_insert((f32::INFINITY, n));
            if d < e.0 {
                *e = (d, n);
            }
        }
        HexGrid { hexes: best.into_iter().map(|(at, (_, node))| Hex { at, node, target: net.pos(node) }).collect() }
    }

    pub fn get(&self, at: Axial) -> Option<&Hex> {
        self.hexes.binary_search_by(|h| h.at.cmp(&at)).ok().map(|i| &self.hexes[i])
    }

    /// The usable hex whose road point is nearest `p`.
    pub fn nearest(&self, p: Pos) -> Option<&Hex> {
        self.hexes.iter().min_by(|a, b| dist(a.target, p).total_cmp(&dist(b.target, p)))
    }

    /// The usable hex containing `p`, if any.
    pub fn at(&self, p: Pos) -> Option<&Hex> {
        self.get(Axial::of(p))
    }
}

pub fn dist(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_falls_in_the_hex_whose_centre_is_nearest() {
        for k in 0..2000 {
            let p = Pos { x: (k * 37 % 3000) as f32 + 0.3, y: (k * 91 % 2800) as f32 + 0.7 };
            let a = Axial::of(p);
            let d = dist(p, a.centre());
            for (dq, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)] {
                let n = Axial { q: a.q + dq, r: a.r + dr };
                assert!(d <= dist(p, n.centre()) + 1e-3, "{p:?} in {a:?}");
                assert_eq!(a.distance(n), 1);
                assert!((dist(a.centre(), n.centre()) - HEX_M).abs() < 1e-2);
            }
        }
    }
}
