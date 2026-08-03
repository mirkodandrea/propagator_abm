//! The warning infrastructure, and the fire's ability to take it out.
//!
//! Every household draws a warning channel at generation time and its own delay
//! follows from that: 90 s on a mobile alert, twenty minutes for a household
//! with none. Independent draws, so in the shipped model everybody's warning is
//! late for a private reason.
//!
//! Real failures are not private. Reporting on Pedrógão Grande cites the fire
//! knocking out communications as a contributing cause of the death toll — one
//! mast, everybody under it, at the moment it mattered most. That is a
//! *correlated* failure and there is no per-household draw that produces one:
//! it needs a shared thing the incident itself can break, which is what this
//! module is.
//!
//! Two decisions worth stating:
//!
//! **Sites cover people, and stand on the highest ground that does.** The first
//! version placed them on the six highest road nodes, which is where masts
//! visibly are, and it was wrong for the reason a plausible authored fact is
//! always wrong (finding 33): checked against the data it covered 745 of 750
//! households on Spotorno, 316 on `mati` and 139 on `pedrogao` — so on two of
//! the four real scenarios the model would have started with most of the town
//! already out of signal, silently changing the baseline every shipped figure
//! was measured against. An operator sites a mast to cover subscribers, so the
//! derivation does: greedy maximum coverage over the households, breaking ties
//! on elevation.
//!
//! **What is modelled is the *loss* of service, not service.** A household that
//! no site ever covered is not affected by one going down: its baked warning
//! channel already says what reception it has. That is what makes this
//! mechanism strictly inert until the fire breaks something, which is the
//! property every measurement taken before it existed depends on.
//!
//! **A site goes down on threat, not on burning.** A mast stands on cleared
//! ground and cleared ground is non-vegetated, so it is never in the fire mask —
//! the same always-negative that makes houses never burn (finding 2). It goes
//! down when the fire gets close enough to make standing there impossible,
//! which is the same field the structure exposure model uses and for the same
//! reason.

use fire::FireSim;
use scenario::Pos;

use crate::network::RoadNetwork;

/// How far one site covers, metres. Generous: this is a rural macrocell, and
/// the point of the model is the outage rather than the fringe.
const COVERAGE_M: f32 = 3500.0;

/// Minimum spacing between sites, metres. Without it the whole set lands on one
/// ridge and the first fire there takes out the window.
const SPACING_M: f32 = 2000.0;

/// How many sites a window gets. Enough to cover the shipped windows, which is
/// checked rather than assumed.
const MAX_SITES: usize = 10;

/// Candidate nodes are subsampled: 61 k of them, and two masts 20 m apart are
/// the same mast.
const CANDIDATE_STRIDE: usize = 8;

/// Threat at the site past which it stops working. Below the civilians'
/// impassable and at the firefighters' working limit: the equipment fails well
/// before the ground does.
const KILL_THREAT: f32 = 0.35;

/// One mast or repeater.
#[derive(Debug, Clone, Copy)]
pub struct Site {
    pub pos: Pos,
    pub elev_m: f32,
    pub radius_m: f32,
    /// Simulated time it went down, or `None` while it is up. There is no
    /// repair: a two-hour initial attack does not get an engineer up the hill.
    pub down_at_s: Option<f32>,
}

/// Every site in the window, and which of them the fire has taken out.
pub struct CommsNet {
    sites: Vec<Site>,
}

impl CommsNet {
    /// Derive the sites from the road network, the elevation and where the
    /// people are.
    ///
    /// Deterministic: candidates are walked in index order and every tie is
    /// broken on a total order, so the same scenario always yields the same
    /// masts in the same order.
    pub fn build(net: &RoadNetwork, homes: &[Pos]) -> CommsNet {
        let candidates: Vec<usize> = (0..net.len()).step_by(CANDIDATE_STRIDE).collect();
        let mut uncovered: Vec<bool> = vec![true; homes.len()];
        let mut sites: Vec<Site> = Vec::new();
        let r2 = COVERAGE_M * COVERAGE_M;

        while sites.len() < MAX_SITES {
            let mut best: Option<(usize, usize, f32)> = None; // node, gain, elevation
            for &i in &candidates {
                let p = net.nodes[i];
                if sites.iter().any(|s| {
                    (s.pos.x - p.x).powi(2) + (s.pos.y - p.y).powi(2) < SPACING_M * SPACING_M
                }) {
                    continue;
                }
                let gain = homes
                    .iter()
                    .zip(&uncovered)
                    .filter(|(h, u)| {
                        **u && (h.x - p.x).powi(2) + (h.y - p.y).powi(2) <= r2
                    })
                    .count();
                let elev = net.elev[i];
                // Coverage first, then height, then index: an operator picks
                // the hill that reaches the most subscribers, and the tie-break
                // is what keeps this reproducible.
                let better = match best {
                    None => gain > 0,
                    Some((_, g, e)) => gain > g || (gain == g && elev > e),
                };
                if better {
                    best = Some((i, gain, elev));
                }
            }
            let Some((i, gain, _)) = best else { break };
            if gain == 0 {
                break;
            }
            let p = net.nodes[i];
            for (h, u) in homes.iter().zip(uncovered.iter_mut()) {
                if (h.x - p.x).powi(2) + (h.y - p.y).powi(2) <= r2 {
                    *u = false;
                }
            }
            sites.push(Site { pos: p, elev_m: net.elev[i], radius_m: COVERAGE_M, down_at_s: None });
        }
        CommsNet { sites }
    }

    /// Take out whatever the fire has reached. Returns the number that went
    /// down on this call, which is what the panel reports.
    pub fn update(&mut self, fire: &FireSim, now_s: f32) -> usize {
        let threat = fire.threat();
        let mut lost = 0;
        for s in &mut self.sites {
            if s.down_at_s.is_some() {
                continue;
            }
            if threat.at(s.pos) >= KILL_THREAT {
                s.down_at_s = Some(now_s);
                lost += 1;
            }
        }
        lost
    }

    /// Whether this point still has whatever service it started with.
    ///
    /// Deliberately not "is there a live site over it". What this models is the
    /// *loss* of a channel, so somewhere no site ever reached is unaffected by
    /// one going down: its households' baked warning channel already says what
    /// reception they have, and declaring them out of signal on top of that
    /// would double-count it — and, worse, would change the run before the fire
    /// had done anything.
    ///
    /// A window whose road network produced no sites at all is covered
    /// everywhere for the same reason: a model that announced a total blackout
    /// because a derivation found nothing would be the worst kind of
    /// always-negative.
    pub fn covered(&self, p: Pos) -> bool {
        let mut ever = false;
        for s in &self.sites {
            let inside =
                (s.pos.x - p.x).powi(2) + (s.pos.y - p.y).powi(2) <= s.radius_m * s.radius_m;
            if !inside {
                continue;
            }
            ever = true;
            if s.down_at_s.is_none() {
                return true;
            }
        }
        !ever
    }

    pub fn sites(&self) -> &[Site] {
        &self.sites
    }

    /// Sites the fire has taken out so far.
    pub fn down(&self) -> usize {
        self.sites.iter().filter(|s| s.down_at_s.is_some()).count()
    }
}
