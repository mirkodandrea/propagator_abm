//! The operations coordinator (01-SPEC-GIOCO §1, 03-PIANO «Coordinatore
//! MVP»): turns the player's ranked places into posts for the units.
//!
//! Deterministic and **pure**: [`propose`] reads the current state and writes
//! nothing, so the same function is the active plan's review and the proposed
//! plan's preview. It never orders civilians and never reorders priorities.
//!
//! The heuristic, in order:
//! 1. A district *needs cover* when the known fire is within [`ENGAGE_M`] of a
//!    home, or within [`ENGAGE_DOWNWIND_M`] with the wind blowing it there;
//!    the first-ranked district, within [`PREEMPT_M`] whatever the wind.
//!    Ranked districts that do not need cover get nobody: a priority is a
//!    claim on units, not a guarantee and not a waste.
//! 2. Districts needing cover share the units by rank: the first takes two,
//!    each later one takes one, any left over go to the first.
//! 3. Inertia: a unit keeps its post while its district still has the place
//!    and the post is still workable. Re-posting happens only on a review.
//! 4. A free unit gets a post on the fire-facing side of its district: homes
//!    nearest the fire first, posts at least two defence radii apart, threat
//!    there below the units' working limit, and a road to it now.
//! 5. Every post carries a reason; every district left uncovered too.

use abm::network::{self, NodeId};
use abm::suppression::{UnitKind, UnitState, CREW_SPEED, ENGINE_SPEED, WORK_LIMIT};
use abm::{Abm, Suppression};
use fire::FireSim;
use scenario::{Pos, Scenario};

use crate::district::{self, dist, District};
use crate::plan::Plan;

/// Fire this close to a home: the district needs cover whatever the wind.
pub const ENGAGE_M: f32 = 1500.0;
/// Fire this close with the wind blowing it at the district.
pub const ENGAGE_DOWNWIND_M: f32 = 3000.0;
/// The player's first priority is covered in advance, whatever the wind,
/// once the fire is this close: pre-positioning is the player's call to make.
pub const PREEMPT_M: f32 = 3500.0;
/// `Exposure::downwind` above this counts as blowing at the district (±60°).
pub const DOWNWIND_COS: f32 = 0.5;
/// Homes within this distance of a crew on its post are protected.
pub const DEFEND_REACH_M: f32 = 120.0;
/// A unit counts as on its post within this distance of it.
pub const ON_POST_M: f32 = 150.0;
/// How often the coordinator reviews the active plan, simulated seconds.
pub const REVIEW_S: i64 = 120;
/// No new post with the burning front closer than this: crew safety comes
/// before the priorities (01-SPEC-GIOCO §1).
pub const SAFE_M: f32 = 150.0;
/// A held post is kept while its home is no more than this many times later
/// to be reached than the district's most threatened home; beyond it the fire
/// has moved on and the unit is re-posted at the next review.
pub const KEEP_RATIO: f32 = 2.5;
/// Candidate homes tried per district before giving up on it.
const CANDIDATES: usize = 12;

#[derive(Debug, Clone, PartialEq)]
pub struct Post {
    pub unit: usize,
    pub district: usize,
    pub at: Pos,
    /// Drive (or walk) time from where the unit is now, by the route open now.
    pub eta_s: f32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Proposal {
    /// One per unit.
    pub posts: Vec<Option<Post>>,
    /// Ranked districts that need cover and did not get all they should, with why.
    pub uncovered: Vec<(usize, String)>,
    /// Ranked districts that do not need cover now.
    pub quiet: Vec<usize>,
}

impl Proposal {
    pub fn units_on(&self, d: usize) -> usize {
        self.posts.iter().flatten().filter(|p| p.district == d).count()
    }
}

/// Borrowed state the coordinator reads.
pub struct View<'a> {
    pub scn: &'a Scenario,
    pub agents: &'a Abm,
    pub fire: &'a FireSim,
    pub crews: &'a Suppression,
    pub districts: &'a [District],
}

/// Travel time along a route: crews ride their vehicle on carriageways and go
/// at [`CREW_SPEED`] on tracks, as `abm::suppression` moves them.
fn route_time(v: &View, kind: UnitKind, nodes: &[NodeId]) -> f32 {
    let net = &v.agents.network;
    nodes
        .windows(2)
        .map(|w| {
            let len = dist(net.pos(w[0]), net.pos(w[1]));
            let drivable = net.neighbours(w[0]).iter().find(|e| e.to == w[1]).is_none_or(|e| e.drivable);
            let speed = if kind == UnitKind::Engine || drivable { ENGINE_SPEED } else { CREW_SPEED };
            len / speed
        })
        .sum()
}

/// Where this unit would stand to cover `home`, and how long it takes to get
/// there by the roads open now. `None` if there is no way.
fn reach(v: &View, unit: usize, home: Pos) -> Option<(Pos, f32)> {
    let u = &v.crews.units[unit];
    let drivable = u.kind.drivable_only();
    let net = &v.agents.network;
    let from = net.nearest(u.pos, drivable)?;
    let to = net.nearest_reachable(home, drivable, from)?;
    let post = net.pos(to);
    if dist(post, home) > DEFEND_REACH_M {
        return None;
    }
    let path = network::route(net, from, to, v.fire.threat(), drivable)?;
    let walk = dist(u.pos, net.pos(from)) / CREW_SPEED;
    Some((post, route_time(v, u.kind, &path) + walk))
}

fn workable(v: &View, p: Pos) -> bool {
    v.fire.threat().at(p) < WORK_LIMIT
}

/// Safe enough to send a unit to now.
fn safe(v: &View, p: Pos) -> bool {
    workable(v, p) && v.fire.active_cells().iter().all(|c| dist(v.scn.world.centre_of(*c), p) >= SAFE_M)
}

/// How soon `unit` could take a workable post in district `d`, by the roads
/// open now: the quickest of the homes the front reaches first.
pub fn eta_to_district(v: &View, unit: usize, d: usize) -> Option<f32> {
    district::homes_by_threat(&v.districts[d], v.agents, v.fire, v.scn)
        .into_iter()
        .filter(|(h, _)| safe(v, *h))
        .take(CANDIDATES)
        .filter_map(|(h, _)| reach(v, unit, h).map(|(_, eta)| eta))
        .min_by(|a, b| a.total_cmp(b))
}

/// The coordinator's proposal for `plan`, given the posts held now.
pub fn propose(v: &View, plan: &Plan, current: &[Option<Post>]) -> Proposal {
    let n_units = v.crews.units.len();
    let usable: Vec<usize> = (0..n_units)
        .filter(|&k| {
            let u = &v.crews.units[k];
            matches!(u.kind, UnitKind::Engine | UnitKind::HandCrew) && u.state != UnitState::Lost
        })
        .collect();

    // 1. who needs cover, in the player's order
    let mut engaged: Vec<(usize, district::Exposure)> = vec![];
    let mut quiet = vec![];
    for (rank, &d) in plan.priorities.iter().enumerate() {
        match district::exposure(&v.districts[d], v.agents, v.fire, v.scn) {
            Some(e)
                if e.distance_m < ENGAGE_M
                    || (e.distance_m < ENGAGE_DOWNWIND_M && e.downwind > DOWNWIND_COS)
                    || (rank == 0 && e.distance_m < PREEMPT_M) =>
            {
                engaged.push((d, e))
            }
            _ => quiet.push(d),
        }
    }

    // 2. quotas by rank
    let mut quota: Vec<usize> = (0..engaged.len()).map(|k| if k == 0 { 2 } else { 1 }).collect();
    let mut total: usize = quota.iter().sum();
    while total > usable.len() {
        let last = quota.iter().rposition(|&q| q > 0).unwrap();
        quota[last] -= 1;
        total -= 1;
    }
    if let Some(first) = quota.first_mut() {
        *first += usable.len() - total;
    }

    let ranked: Vec<Vec<(Pos, f32)>> =
        engaged.iter().map(|(d, _)| district::homes_by_threat(&v.districts[*d], v.agents, v.fire, v.scn)).collect();
    let mut posts: Vec<Option<Post>> = vec![None; n_units];
    let mut taken = vec![0usize; v.districts.len()];
    let quota_of = |d: usize| engaged.iter().position(|(x, _)| *x == d).map_or(0, |k| quota[k]);

    // 3. inertia
    for &k in &usable {
        if let Some(Some(p)) = current.get(k) {
            // still among the homes the front reaches soonest?
            let still_threatened = engaged.iter().position(|(x, _)| *x == p.district).is_some_and(|r| {
                let best = ranked[r].first().map_or(f32::INFINITY, |x| x.1);
                ranked[r].iter().filter(|(h, _)| dist(*h, p.at) <= DEFEND_REACH_M).any(|(_, s)| *s <= KEEP_RATIO * best.max(100.0))
            });
            if taken[p.district] < quota_of(p.district)
                && still_threatened
                && (workable(v, p.at) || v.crews.units[k].state == UnitState::Withdrawing)
            {
                taken[p.district] += 1;
                let mut kept = p.clone();
                kept.eta_s = reach(v, k, p.at).map_or(p.eta_s, |(_, eta)| eta);
                posts[k] = Some(kept);
            }
        }
    }

    // 4. free units, engines first (roster order), to districts by rank
    let mut uncovered: Vec<(usize, String)> = vec![];
    let mut no_access: Vec<usize> = vec![];
    // Districts whose every home is already within reach of a post.
    let mut full: Vec<usize> = vec![];
    for &k in &usable {
        if posts[k].is_some() || v.crews.units[k].state == UnitState::Withdrawing {
            continue;
        }
        let u = &v.crews.units[k];
        'districts: for (rank, (d, e)) in engaged.iter().enumerate() {
            if taken[*d] >= quota[rank] {
                continue;
            }
            let held: Vec<Pos> = posts.iter().flatten().filter(|p| p.district == *d).map(|p| p.at).collect();
            let homes: Vec<Pos> = ranked[rank].iter().map(|x| x.0).collect();
            let mut tried = 0;
            if homes.iter().all(|h| held.iter().any(|q| dist(*q, *h) <= DEFEND_REACH_M)) && !held.is_empty() {
                if !full.contains(d) {
                    full.push(*d);
                }
                continue;
            }
            for h in homes {
                if held.iter().any(|q| dist(*q, h) < 2.0 * DEFEND_REACH_M) || !safe(v, h) {
                    continue;
                }
                tried += 1;
                if tried > CANDIDATES {
                    break;
                }
                if let Some((at, eta)) = reach(v, k, h) {
                    if held.iter().any(|q| dist(*q, at) < 2.0 * DEFEND_REACH_M) {
                        continue;
                    }
                    let name = &v.districts[*d].name;
                    let reason = format!(
                        "{} va a {} (priorità {}): case sottovento al fronte, fuoco a {:.1} km, arrivo in {:.0} min",
                        u.callsign,
                        name,
                        plan.rank(*d).map_or(0, |r| r + 1),
                        e.distance_m / 1000.0,
                        (eta / 60.0).ceil()
                    );
                    posts[k] = Some(Post { unit: k, district: *d, at, eta_s: eta, reason });
                    taken[*d] += 1;
                    break 'districts;
                }
            }
            if !no_access.contains(d) {
                no_access.push(*d);
            }
        }
    }

    // 5. what is left uncovered, and why
    for (rank, (d, _)) in engaged.iter().enumerate() {
        if taken[*d] < quota[rank] && !full.contains(d) {
            let name = &v.districts[*d].name;
            let why = if no_access.contains(d) {
                format!("{name}: nessuna postazione raggiungibile e lavorabile ora")
            } else {
                format!("{name}: mezzi insufficienti ({} di {})", taken[*d], quota[rank])
            };
            uncovered.push((*d, why));
        }
    }
    for (rank, (d, _)) in engaged.iter().enumerate() {
        if quota[rank] == 0 && !uncovered.iter().any(|(x, _)| x == d) {
            uncovered.push((*d, format!("{}: nessun mezzo rimasto dopo le priorità più alte", v.districts[*d].name)));
        }
    }
    Proposal { posts, uncovered, quiet }
}
