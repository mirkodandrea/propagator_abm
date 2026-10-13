//! The operations coordinator (docs/01-GIOCO.md, 03-PIANO «Coordinatore
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
//!
//! With [`FrontAttack`] on, a unit can also be posted *on the front*: on the
//! burning edge nearest a district, between it and the homes, at the nearest
//! safe stand-off. An engine wets the fuel ahead of the edge from there, a crew
//! cuts line across it (`abm::suppression`, `Task::Attack`). When the edge is
//! already closer to the homes than the shortest stand-off, the unit defends
//! the homes instead.

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
/// before the priorities (docs/01-GIOCO.md).
pub const SAFE_M: f32 = 150.0;
/// A held post is kept while its home is no more than this many times later
/// to be reached than the district's most threatened home; beyond it the fire
/// has moved on and the unit is re-posted at the next review.
pub const KEEP_RATIO: f32 = 2.5;
/// Candidate homes tried per district before giving up on it.
const CANDIDATES: usize = 12;
/// Distances from the burning edge, nearest first, at which a unit attacking
/// the front tries to stand: the first safe one with a way there.
pub const STANDOFF_M: [f32; 4] = [200.0, 300.0, 450.0, 650.0];
/// How far from the chosen stand-off point the unit's road may end.
const FRONT_SLACK_M: f32 = 200.0;
/// A post on the front is kept while the burning edge is no farther from it
/// than this; beyond it the fire has moved on.
pub const FRONT_KEEP_M: f32 = 900.0;

/// Whether the units also attack the front (TODO B, richiesta dell'utente
/// 2026-10-13), not only defend the homes. Off in the kiosk until the user
/// confirms it on the measurements (`examples/spegnimento.rs --attacco`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrontAttack {
    /// Units only defend the homes of the districts that need cover.
    #[default]
    Off,
    /// As `Off`, and the units left without a post attack the front heading
    /// for the first district that needs cover (or the nearest to the fire).
    Spare,
    /// Every unit attacks the front heading for its district; it defends the
    /// homes only once the edge is too close to them to stand in between.
    All,
}

impl FrontAttack {
    pub fn parse(s: &str) -> Option<FrontAttack> {
        match s {
            "off" | "no" => Some(FrontAttack::Off),
            "liberi" | "spare" => Some(FrontAttack::Spare),
            "tutti" | "all" => Some(FrontAttack::All),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FrontAttack::Off => "off",
            FrontAttack::Spare => "liberi",
            FrontAttack::All => "tutti",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Post {
    pub unit: usize,
    pub district: usize,
    pub at: Pos,
    /// On the front heading for `district`, not in front of its homes.
    pub front: bool,
    /// Drive (or walk) time from where the unit is now, by the route open now.
    pub eta_s: f32,
    /// The way there, from the unit, by the roads open now: for the map.
    pub route: Vec<Pos>,
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
    /// Units left without a post, with why: a unit sent home is a choice the
    /// player should see, not a silence.
    pub idle: Vec<(usize, String)>,
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
    pub attack: FrontAttack,
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
/// there by the roads open now, and the way. `None` if there is no way.
fn reach(v: &View, unit: usize, home: Pos) -> Option<(Pos, f32, Vec<Pos>)> {
    reach_near(v, unit, home, DEFEND_REACH_M)
}

/// The same for any point: the road end nearest `target`, within `slack` of it.
fn reach_near(v: &View, unit: usize, target: Pos, slack: f32) -> Option<(Pos, f32, Vec<Pos>)> {
    let u = &v.crews.units[unit];
    let drivable = u.kind.drivable_only();
    let net = &v.agents.network;
    let from = net.nearest(u.pos, drivable)?;
    let to = net.nearest_reachable(target, drivable, from)?;
    let post = net.pos(to);
    if dist(post, target) > slack {
        return None;
    }
    let path = network::route(net, from, to, v.fire.threat(), drivable)?;
    let walk = dist(u.pos, net.pos(from)) / CREW_SPEED;
    let way = std::iter::once(u.pos).chain(std::iter::once(net.pos(from))).chain(path.iter().map(|&n| net.pos(n))).collect();
    Some((post, route_time(v, u.kind, &path) + walk, way))
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
        .filter_map(|(h, _)| reach(v, unit, h).map(|(_, eta, _)| eta))
        .min_by(|a, b| a.total_cmp(b))
}

/// Where `unit` would stand to attack the front heading for district `d`: on
/// the line from the burning cell nearest its homes to the nearest home, at the
/// first [`STANDOFF_M`] that is safe and has a way there. `None` when the edge
/// is already within the shortest stand-off of the homes (that is defence) or
/// nothing is reachable.
fn front_post(v: &View, unit: usize, d: usize) -> Option<(Pos, f32, Vec<Pos>)> {
    let e = district::exposure(&v.districts[d], v.agents, v.fire, v.scn)?;
    let home = v.districts[d]
        .households
        .iter()
        .map(|&i| v.agents.households[i].home)
        .min_by(|a, b| dist(*a, e.fire_at).total_cmp(&dist(*b, e.fire_at)))?;
    let len = dist(home, e.fire_at);
    let (ux, uy) = ((home.x - e.fire_at.x) / len.max(1.0), (home.y - e.fire_at.y) / len.max(1.0));
    for s in STANDOFF_M {
        if s >= len {
            break;
        }
        let p = Pos { x: e.fire_at.x + ux * s, y: e.fire_at.y + uy * s };
        if let Some(r) = reach_near(v, unit, p, FRONT_SLACK_M).filter(|r| safe(v, r.0)) {
            return Some(r);
        }
    }
    None
}

/// A held post on the front still makes sense: safe, the edge still near,
/// and not a crew that has finished its line (it is re-posted where the edge is now).
fn front_kept(v: &View, unit: usize, p: &Post) -> bool {
    let u = &v.crews.units[unit];
    let near = v.fire.active_cells().iter().any(|c| dist(v.scn.world.centre_of(*c), p.at) <= FRONT_KEEP_M);
    let done = u.kind == UnitKind::HandCrew && u.state == UnitState::Staged;
    (workable(v, p.at) || u.state == UnitState::Withdrawing) && near && !done
}

fn front_reason(v: &View, plan: &Plan, unit: usize, d: usize, eta: f32) -> String {
    let fire = district::exposure(&v.districts[d], v.agents, v.fire, v.scn).map_or(String::new(), |e| format!("; fuoco a {}", crate::words::km(e.distance_m)));
    let rank = plan.rank(d).map_or(String::new(), |r| format!(" (priorità {})", r + 1));
    format!("{} attacca il fuoco che avanza verso {}{rank}{fire}, arrivo in {:.0} min", v.crews.units[unit].callsign, v.districts[d].name, (eta / 60.0).ceil())
}

/// A held post, with the drive time and route from where the unit is now.
fn refreshed(v: &View, unit: usize, p: &Post) -> Post {
    let mut kept = p.clone();
    let slack = if p.front { FRONT_SLACK_M } else { DEFEND_REACH_M };
    if let Some((_, eta, way)) = reach_near(v, unit, p.at, slack) {
        kept.eta_s = eta;
        kept.route = way;
    }
    kept
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
            if p.front {
                // with `All` a post on the front holds its district's share;
                // a spare one is reconsidered once the homes are served (7.)
                if v.attack == FrontAttack::All && engaged.iter().any(|(x, _)| *x == p.district) && taken[p.district] < quota_of(p.district) && front_kept(v, k, p) {
                    taken[p.district] += 1;
                    posts[k] = Some(refreshed(v, k, p));
                }
                continue;
            }
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
                posts[k] = Some(refreshed(v, k, p));
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
            if v.attack == FrontAttack::All {
                if let Some((at, eta, route)) = front_post(v, k, *d) {
                    let reason = front_reason(v, plan, k, *d, eta);
                    posts[k] = Some(Post { unit: k, district: *d, at, front: true, eta_s: eta, route, reason });
                    taken[*d] += 1;
                    break 'districts;
                }
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
                if let Some((at, eta, route)) = reach(v, k, h) {
                    if held.iter().any(|q| dist(*q, at) < 2.0 * DEFEND_REACH_M) {
                        continue;
                    }
                    let name = &v.districts[*d].name;
                    let reason = format!(
                        "{} va a {} (priorità {}) a difendere le case verso cui il vento spinge il fuoco; fuoco a {}, arrivo in {:.0} min",
                        u.callsign,
                        name,
                        plan.rank(*d).map_or(0, |r| r + 1),
                        crate::words::km(e.distance_m),
                        (eta / 60.0).ceil()
                    );
                    posts[k] = Some(Post { unit: k, district: *d, at, front: false, eta_s: eta, route, reason });
                    taken[*d] += 1;
                    break 'districts;
                }
            }
            if !no_access.contains(d) {
                no_access.push(*d);
            }
        }
    }

    // 4b. with the attack on, the units the homes do not need go to the front:
    // heading for the first district that needs cover, else for the ranked
    // (then any) district the fire is nearest
    if v.attack != FrontAttack::Off {
        let nearest_of = |ds: &mut dyn Iterator<Item = usize>| {
            ds.filter_map(|d| district::exposure(&v.districts[d], v.agents, v.fire, v.scn).map(|e| (d, e.distance_m)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|x| x.0)
        };
        let mut target = None;
        for &k in &usable {
            if posts[k].is_some() || v.crews.units[k].state == UnitState::Withdrawing {
                continue;
            }
            if let Some(Some(p)) = current.get(k).filter(|p| p.as_ref().is_some_and(|p| p.front && front_kept(v, k, p))) {
                posts[k] = Some(refreshed(v, k, p));
                continue;
            }
            let d = *target.get_or_insert_with(|| {
                engaged.first().map(|(d, _)| *d).or_else(|| nearest_of(&mut plan.priorities.iter().copied())).or_else(|| nearest_of(&mut (0..v.districts.len())))
            });
            let Some(d) = d else { break };
            if let Some((at, eta, route)) = front_post(v, k, d) {
                let reason = front_reason(v, plan, k, d, eta);
                posts[k] = Some(Post { unit: k, district: d, at, front: true, eta_s: eta, route, reason });
            }
        }
    }

    // 5. what is left uncovered, and why
    for (rank, (d, _)) in engaged.iter().enumerate() {
        if taken[*d] < quota[rank] && !full.contains(d) {
            let name = &v.districts[*d].name;
            let why = if no_access.contains(d) && taken[*d] > 0 {
                format!("{name}: {} {} su {}; gli altri non possono avvicinarsi senza rischi ora", taken[*d], if taken[*d] == 1 { "mezzo" } else { "mezzi" }, quota[rank])
            } else if no_access.contains(d) {
                format!("{name}: i mezzi non possono avvicinarsi senza rischi ora")
            } else {
                format!("{name}: mezzi insufficienti ({} di {})", taken[*d], quota[rank])
            };
            uncovered.push((*d, why));
        }
    }
    for (rank, (d, _)) in engaged.iter().enumerate() {
        if quota[rank] == 0 && !uncovered.iter().any(|(x, _)| x == d) {
            uncovered.push((*d, format!("{}: nessun mezzo libero, sono già tutti dove la priorità è più alta", v.districts[*d].name)));
        }
    }
    // 6. who is left without a post, and why
    // a ranked district short of its share because no unit could reach it
    let blocked = engaged.iter().enumerate().find(|(r, (d, _))| no_access.contains(d) && taken[*d] < quota[*r]).map(|(_, (d, _))| *d);
    let mut idle = vec![];
    for &k in &usable {
        if posts[k].is_some() || v.crews.units[k].state == UnitState::Withdrawing {
            continue;
        }
        let u = &v.crews.units[k];
        let what = match current.get(k).and_then(|p| p.as_ref()) {
            Some(p) => format!("{} lascia {} e rientra alla base", u.callsign, v.districts[p.district].name),
            None => format!("{} resta in attesa", u.callsign),
        };
        let why = match blocked {
            Some(d) => format!("{} (priorità {}) non è raggiungibile ora", v.districts[d].name, plan.rank(d).map_or(0, |r| r + 1)),
            None if plan.priorities.is_empty() => "nessun luogo scelto da difendere".to_string(),
            None if engaged.is_empty() => "nessun luogo in priorità è minacciato ora".to_string(),
            None => "i luoghi in priorità hanno già i mezzi che servono".to_string(),
        };
        idle.push((k, format!("{what}: {why}")));
    }
    Proposal { posts, uncovered, quiet, idle }
}
