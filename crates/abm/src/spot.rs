//! Fires that start where there was no fire.
//!
//! The core genuinely models spotting: embers are drawn per burning cell, a
//! landing distance is computed from wind and intensity, and the landing cell
//! actually ignites. That is a real, disconnected new fire — and until this
//! module nothing an agent could read said so, because every quantity the
//! civilian model derives from the fire is a field over the *whole* burning
//! mask. A new ignition 400 m behind a household reads exactly like the front
//! creeping 400 m closer, and the two are not the same event: one is the fire
//! doing what it was doing, and the other is the road you were relying on being
//! cut behind you. Pedrógão Grande and Mati are both accounts of the second.
//!
//! What counts as a spot fire here is **non-contiguity**, not provenance: a
//! newly burning cell with nothing that was already alight within [`GAP_M`] of
//! it. That deliberately also catches a second ignition the player lit
//! somewhere else, because from a resident's point of view the two are the same
//! event, and distinguishing them would need the core to tell us which cells it
//! spotted — which it does not.

use std::collections::VecDeque;

use fire::FireSim;
use scenario::{Cell, Pos, Scenario};

/// Metres of unburnt ground between a new ignition and anything already
/// burning, past which it is a separate fire rather than the front moving.
///
/// Six cells on the 20 m grid. The front advances at most one or two cells per
/// refresh at any calibration this model runs at, so this is well clear of it.
const GAP_M: f32 = 120.0;

/// Spot fires remembered. They are only ever read nearest-first, and a town's
/// worth of separate ignitions is a different scenario from the one this
/// supports.
const MAX_SPOTS: usize = 64;

/// One fire that started away from the front.
#[derive(Debug, Clone, Copy)]
pub struct Spot {
    pub pos: Pos,
    /// Simulated time it was first seen burning.
    pub at_s: f32,
}

/// The spot fires seen so far, and the mask needed to recognise the next one.
pub struct SpotFires {
    /// Cells that have ever been active. Not the core's own burnt mask: this
    /// one has to be *ours*, because the question is what was alight the last
    /// time we looked.
    seen: Vec<bool>,
    spots: VecDeque<Spot>,
    cols: usize,
    /// Search radius in cells, from [`GAP_M`].
    span: i64,
    /// Nothing has burnt yet, so the first fire to appear is the incident
    /// rather than a spot from it.
    primed: bool,
}

impl SpotFires {
    pub fn new(scn: &Scenario) -> SpotFires {
        let w = scn.world;
        SpotFires {
            seen: vec![false; w.fire_rows * w.fire_cols],
            spots: VecDeque::new(),
            cols: w.fire_cols,
            span: (GAP_M / w.cellsize).ceil() as i64,
            primed: false,
        }
    }

    /// Fold in whatever is burning now. Returns the number of new spot fires.
    ///
    /// Called on the routing refresh rather than every step, which is also the
    /// resolution the answer is honest at: a spot fire is news for the minutes
    /// after it starts, not for the seconds.
    pub fn update(&mut self, fire: &FireSim, scn: &Scenario, now_s: f32) -> usize {
        let w = scn.world;
        let rows = w.fire_rows as i64;
        let cols = w.fire_cols as i64;

        // Two passes, because a candidate has to be judged against the mask as
        // it was *before* this batch. Judging it against a mask the batch is
        // being written into would make the answer depend on cell order: the
        // first cell of a new fire would be a spot and the second, one cell
        // away, would not.
        let mut fresh: Vec<Cell> = Vec::new();
        for c in fire.active_cells() {
            let i = c.row * self.cols + c.col;
            if !self.seen[i] {
                fresh.push(*c);
            }
        }

        // Cells with nothing already alight near them. Each is *part of* a new
        // fire rather than one of its own: a 200 m patch is four hundred metres
        // across and its far edges are further apart than the gap, so counting
        // cells here reported one ignition as eleven separate fires.
        let mut detached: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for c in &fresh {
            let mut contiguous = false;
            'search: for dr in -self.span..=self.span {
                for dc in -self.span..=self.span {
                    if (dr * dr + dc * dc) as f32 > (self.span * self.span) as f32 {
                        continue;
                    }
                    let (r, col) = (c.row as i64 + dr, c.col as i64 + dc);
                    if r < 0 || col < 0 || r >= rows || col >= cols {
                        continue;
                    }
                    if self.seen[r as usize * self.cols + col as usize] {
                        contiguous = true;
                        break 'search;
                    }
                }
            }
            if !contiguous {
                detached.insert((c.row, c.col));
            }
        }

        // One spot per connected blob, at its centroid. Flood-filled on the
        // 8-neighbourhood of the detached set itself, which is the definition
        // that matches what the thing *is*: a new fire, however many cells of
        // it appeared between one look and the next.
        let mut found = 0;
        let mut pending: Vec<(usize, usize)> = detached.iter().copied().collect();
        pending.sort_unstable();
        for start in pending {
            if !detached.remove(&start) {
                continue;
            }
            let mut stack = vec![start];
            let (mut sx, mut sy, mut n) = (0.0f32, 0.0f32, 0.0f32);
            while let Some((r, c)) = stack.pop() {
                let p = w.centre_of(Cell { row: r, col: c });
                sx += p.x;
                sy += p.y;
                n += 1.0;
                for dr in -1i64..=1 {
                    for dc in -1i64..=1 {
                        let (nr, nc) = (r as i64 + dr, c as i64 + dc);
                        if nr < 0 || nc < 0 || nr >= rows || nc >= cols {
                            continue;
                        }
                        let k = (nr as usize, nc as usize);
                        if detached.remove(&k) {
                            stack.push(k);
                        }
                    }
                }
            }
            if !self.primed {
                continue;
            }
            if self.spots.len() == MAX_SPOTS {
                self.spots.pop_front();
            }
            self.spots.push_back(Spot { pos: Pos { x: sx / n, y: sy / n }, at_s: now_s });
            found += 1;
        }

        for c in &fresh {
            self.seen[c.row * self.cols + c.col] = true;
        }
        if !fresh.is_empty() {
            self.primed = true;
        }
        found
    }

    /// Distance in metres to the nearest spot fire and its age in minutes, as
    /// someone standing at `p` would judge them.
    ///
    /// `(f32::INFINITY, f32::MAX)` when there has been none, which is what the
    /// observation's own defaults mean and what every block reading it is
    /// written to fall through on.
    pub fn nearest(&self, p: Pos, now_s: f32) -> (f32, f32) {
        let mut best = f32::INFINITY;
        let mut age = f32::MAX;
        for s in &self.spots {
            let d = dist(s.pos, p);
            if d < best {
                best = d;
                age = ((now_s - s.at_s) / 60.0).max(0.0);
            }
        }
        (best, age)
    }

    /// Every spot fire so far, oldest first. For the panel and the tests.
    pub fn spots(&self) -> impl Iterator<Item = &Spot> {
        self.spots.iter()
    }

    pub fn len(&self) -> usize {
        self.spots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spots.is_empty()
    }
}

fn dist(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}
