//! Somewhere to go that is not a refuge.
//!
//! A [`Refuge`](crate::refuge::Refuge) is where the evacuation is organised: a
//! drivable node in a clear neighbourhood, spaced 600 m apart, twelve of them in
//! the window. That is the right object for "where is everybody going" and the
//! wrong one for "where do I go *now*", and the difference between the two is
//! the gap people on foot die in — a car park a hundred metres away, walked past
//! on the way to an assembly point two kilometres off.
//!
//! A **haven** is that car park. Measured the same way refuges are, for the same
//! reason (finding 9: a plausible-looking safe spot in continuous macchia is a
//! death trap the model will route people into), but on three criteria rather
//! than two:
//!
//! - **not in the fuel**: under [`MAX_BURNABLE_FRAC`] of the ground within
//!   [`CLEAR_RADIUS_M`] is burnable;
//! - **not built up**: under [`MAX_BUILDINGS`] buildings within
//!   [`BUILDING_RADIUS_M`]. This is the criterion refuges do not have and it is
//!   what separates a beach from a street. Non-vegetated fuel does not
//!   distinguish a car park from the old town, and standing in a lane with
//!   houses alight on both sides is not open ground;
//! - **reachable on foot**, not by vehicle. Nobody drives to these.
//!
//! A haven adjacent to water is a [`HavenKind::Water`] one, and it is a
//! different object again: in the water a person is out of the fire's reach for
//! as long as they can stand it. Mati's fatalities include people who died in
//! lanes trying to reach the shoreline and its survivors include people who
//! swam clear; Rhodes moved thousands off beaches by boat. Neither was
//! expressible before this.
//!
//! ### Where the water comes from
//!
//! Not from OSM. `natural=water` is inland ponds and the sea is
//! `natural=coastline`, which the bake does not carry. The sea is derived from
//! the two rasters the fire model already needs: a **non-burnable cell at or
//! below [`SEA_M`]**. On the shipped windows that is 41% of Spotorno and 22% of
//! Rhodes, and 0% of Pedrógão Grande and Mati — both of which are entirely
//! inland, the second one despite being a scenario about a coastal disaster.
//! A behaviour that leans on the shore has to read sensibly where there is
//! none, which is why every block that offers it is gated on the distance being
//! finite.

use scenario::{Cell, Pos, Scenario};

use crate::network::{NodeId, RoadNetwork};

/// Radius searched for burnable fuel around a candidate, metres. Half the
/// refuge's, because a haven is a local answer.
const CLEAR_RADIUS_M: f32 = 150.0;
/// Fraction of that neighbourhood allowed to be burnable.
const MAX_BURNABLE_FRAC: f32 = 0.10;
/// Radius searched for buildings, metres.
const BUILDING_RADIUS_M: f32 = 60.0;
/// Buildings allowed within it. Two, not zero: a beach with a bar on it is
/// still a beach.
const MAX_BUILDINGS: usize = 2;
/// Minimum spacing between havens, metres. Much tighter than the refuges' 600 m
/// — the whole point is that one is near wherever somebody is caught.
const MIN_SPACING_M: f32 = 150.0;
/// Elevation at or below which a non-burnable cell is water, metres.
const SEA_M: f64 = 2.0;
/// How close a haven has to be to water to be one you can get into, metres.
const SHORE_M: f32 = 80.0;
/// And how far above it. Proximity alone is not the water's edge on a coast
/// like this one: the first version of this selected a node 27 m up, eighty
/// metres from the sea and separated from it by the cliff the Aurelia is cut
/// into. "Near the sea" and "able to get into the sea" are different
/// predicates, and only the second one is a refuge.
const SHORE_ELEV_M: f32 = 12.0;

/// How many havens a window gets. Enough that the nearest one is genuinely
/// near; small enough that the per-agent straight-line scan is free.
pub const MAX_HAVENS: usize = 160;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HavenKind {
    /// A clearing: a car park, a sports ground, a field edge, the port.
    OpenGround,
    /// The water's edge. Survivable indefinitely, and the only place a boat
    /// lift can take anybody off.
    Water,
}

#[derive(Debug, Clone, Copy)]
pub struct Haven {
    pub node: NodeId,
    pub pos: Pos,
    pub kind: HavenKind,
    /// Burnable fraction of the surrounding neighbourhood, 0-1.
    pub burnable_frac: f32,
}

impl Haven {
    pub fn is_water(&self) -> bool {
        self.kind == HavenKind::Water
    }
}

/// Choose havens for a scenario. Deterministic: the same scenario always yields
/// the same set, in the same order.
pub fn choose(scn: &Scenario, net: &RoadNetwork, max: usize) -> Vec<Haven> {
    let w = scn.world;
    // Summed-area tables, because this is a screening test over every node in
    // the network -- 61 k of them on the shipped window -- and the per-node
    // disc scan `refuge::choose` runs over its thousands of drivable nodes
    // would be seventeen million cell reads here.
    let burnable = Sat::of(w.fire_rows, w.fire_cols, |c| scn.is_burnable(c));
    let water = Sat::of(w.fire_rows, w.fire_cols, |c| is_water(scn, c));
    let buildings = BuildingGrid::build(scn);

    let mut candidates: Vec<Haven> = Vec::new();
    for (i, &p) in net.nodes.iter().enumerate() {
        let n = i as NodeId;
        // Foot access only. A haven a vehicle cannot reach is exactly the kind
        // that is still there when the road is gone.
        if net.neighbours(n).is_empty() {
            continue;
        }
        let c = w.cell_of(p);
        let frac = burnable.fraction(c, radius_cells(CLEAR_RADIUS_M, w.cellsize));
        if frac > MAX_BURNABLE_FRAC {
            continue;
        }
        if buildings.count_near(p, BUILDING_RADIUS_M) > MAX_BUILDINGS {
            continue;
        }
        let wet = water.any(c, radius_cells(SHORE_M, w.cellsize))
            && net.elev[i] <= SHORE_ELEV_M;
        candidates.push(Haven {
            node: n,
            pos: p,
            kind: if wet { HavenKind::Water } else { HavenKind::OpenGround },
            burnable_frac: frac,
        });
    }

    // Water first, then cleanest, so the greedy spacing keeps the shoreline
    // rather than the car park behind it -- the shore is the one with a way out
    // of the incident at the end of it.
    candidates.sort_by(|a, b| {
        b.is_water()
            .cmp(&a.is_water())
            .then(
                a.burnable_frac
                    .partial_cmp(&b.burnable_frac)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.node.cmp(&b.node))
    });

    let mut chosen: Vec<Haven> = Vec::new();
    for c in candidates {
        if chosen.len() >= max {
            break;
        }
        let clash = chosen.iter().any(|h| {
            (h.pos.x - c.pos.x).powi(2) + (h.pos.y - c.pos.y).powi(2)
                < MIN_SPACING_M * MIN_SPACING_M
        });
        if !clash {
            chosen.push(c);
        }
    }
    chosen
}

/// A non-burnable cell at or below sea level: the sea, a lake, a reservoir.
pub fn is_water(scn: &Scenario, c: Cell) -> bool {
    if scn.is_burnable(c) {
        return false;
    }
    let i = c.row * scn.world.fire_cols + c.col;
    scn.dem.get(i).map(|e| *e <= SEA_M).unwrap_or(false)
}

fn radius_cells(m: f32, cellsize: f32) -> i64 {
    (m / cellsize).ceil() as i64
}

/// Summed-area table over the fire grid, for O(1) window queries.
///
/// The window is a square rather than the disc `refuge::burnable_fraction`
/// uses. That is a deliberate difference and not a shortcut being hidden: this
/// is a screening test at 150 m over every node in the network, and a square
/// that includes a little more of the corners selects marginally more
/// conservatively — which is the right direction for a test whose failure mode
/// is sending somebody to stand in the fuel.
struct Sat {
    rows: usize,
    cols: usize,
    /// `(rows + 1) * (cols + 1)`, inclusive prefix sums.
    sum: Vec<u32>,
}

impl Sat {
    fn of(rows: usize, cols: usize, f: impl Fn(Cell) -> bool) -> Sat {
        let mut sum = vec![0u32; (rows + 1) * (cols + 1)];
        for r in 0..rows {
            for c in 0..cols {
                let v = u32::from(f(Cell { row: r, col: c }));
                sum[(r + 1) * (cols + 1) + c + 1] =
                    v + sum[r * (cols + 1) + c + 1] + sum[(r + 1) * (cols + 1) + c]
                        - sum[r * (cols + 1) + c];
            }
        }
        Sat { rows, cols, sum }
    }

    /// Count and area of the clamped square window of half-width `span`.
    fn window(&self, c: Cell, span: i64) -> (u32, u32) {
        let r0 = (c.row as i64 - span).max(0) as usize;
        let c0 = (c.col as i64 - span).max(0) as usize;
        let r1 = ((c.row as i64 + span + 1).max(0) as usize).min(self.rows);
        let c1 = ((c.col as i64 + span + 1).max(0) as usize).min(self.cols);
        if r1 <= r0 || c1 <= c0 {
            return (0, 0);
        }
        let w = self.cols + 1;
        let n = self.sum[r1 * w + c1] + self.sum[r0 * w + c0]
            - self.sum[r0 * w + c1]
            - self.sum[r1 * w + c0];
        (n, ((r1 - r0) * (c1 - c0)) as u32)
    }

    fn fraction(&self, c: Cell, span: i64) -> f32 {
        let (n, area) = self.window(c, span);
        if area == 0 {
            0.0
        } else {
            n as f32 / area as f32
        }
    }

    fn any(&self, c: Cell, span: i64) -> bool {
        self.window(c, span).0 > 0
    }
}

/// Building centroids on a coarse grid, so "how built up is it here" is a
/// bucket lookup rather than a scan of 42,000 footprints per node.
struct BuildingGrid {
    cell_m: f32,
    cols: usize,
    rows: usize,
    cells: Vec<Vec<Pos>>,
}

impl BuildingGrid {
    fn build(scn: &Scenario) -> BuildingGrid {
        let cell_m = 100.0;
        let cols = (scn.world.width_m / cell_m).ceil() as usize + 1;
        let rows = (scn.world.height_m / cell_m).ceil() as usize + 1;
        let mut cells = vec![Vec::new(); rows * cols];
        for b in &scn.vectors.buildings {
            let p = Pos { x: b.centroid[0], y: b.centroid[1] };
            let (gx, gy) = ((p.x / cell_m) as usize, (p.y / cell_m) as usize);
            if gx < cols && gy < rows {
                cells[gy * cols + gx].push(p);
            }
        }
        BuildingGrid { cell_m, cols, rows, cells }
    }

    fn count_near(&self, p: Pos, radius_m: f32) -> usize {
        let span = (radius_m / self.cell_m).ceil() as i64;
        let (gx, gy) = ((p.x / self.cell_m) as i64, (p.y / self.cell_m) as i64);
        let r2 = radius_m * radius_m;
        let mut n = 0;
        for dy in -span..=span {
            for dx in -span..=span {
                let (x, y) = (gx + dx, gy + dy);
                if x < 0 || y < 0 || x as usize >= self.cols || y as usize >= self.rows {
                    continue;
                }
                for q in &self.cells[y as usize * self.cols + x as usize] {
                    if (q.x - p.x).powi(2) + (q.y - p.y).powi(2) <= r2 {
                        n += 1;
                        // Nothing needs the true count, only whether it is over
                        // the limit, and a dense old town has hundreds.
                        if n > MAX_BUILDINGS {
                            return n;
                        }
                    }
                }
            }
        }
        n
    }
}
