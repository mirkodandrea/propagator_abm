//! Evacuation, simplified (docs/01-GIOCO.md §4): no model of single people.
//!
//! An evacuation empties one district along one route to a refuge. Families
//! leave in turn, nearest the departure point first, at most
//! [`EvacRules::per_min`] people a minute, after [`EvacRules::prep_min`]
//! of preparation; each family then drives the route at [`CONVOY_MPS`]. Only
//! one district at a time is being emptied: the next can start once the last
//! family of the current one is on the road.
//!
//! The fire can cut the route. Every [`CHECK_S`] the route ahead is checked:
//! if a road is cut, a new route is searched; if there is none, the
//! evacuation is **interrupted** and whoever is still at home goes into
//! **self-defence**. A family on the road whose way ahead is cut looks for
//! another way; without one it stops where it is.
//!
//! When the fire reaches people (the same rule for the screen and the score):
//! - at home with no order: the threat at the door reaches
//!   [`fire::threat::ALARMING`], or the house is hit;
//! - in self-defence (prepared, sheltering): the house is hit, or the threat
//!   at the door reaches [`fire::threat::IMPASSABLE`];
//! - on the road: the threat where they are reaches `IMPASSABLE`.

use std::collections::VecDeque;
use std::sync::Arc;

use abm::network::{self, NodeId, RoadNetwork};
use fire::threat::{ALARMING, IMPASSABLE};
use fire::FireSim;
use scenario::{Pos, Scenario};
use serde::Deserialize;

use crate::district::{dist, District};

/// Speed of a family's car on the evacuation route, m/s (~30 km/h: a column
/// of cars on hill roads).
pub const CONVOY_MPS: f32 = 8.0;
/// How often the routes ahead are checked against the fire, simulated s.
pub const CHECK_S: i64 = 60;
/// Spacing of the samples along a route checked against the fire, metres
/// (the fire grid is 20 m).
const SAMPLE_M: f32 = 20.0;
/// A refuge this close to a home of the district is not a way out of it.
const REFUGE_CLEAR_M: f32 = 400.0;
/// Refuges considered (`abm::refuge::choose`).
const MAX_REFUGES: usize = 10;

/// From `game.json` (`evacuation`).
#[derive(Debug, Clone, Deserialize)]
pub struct EvacRules {
    /// Hard limit: people leaving a district per minute.
    pub per_min: f32,
    /// Minutes between the order and the first departure.
    pub prep_min: f32,
    /// Gathering areas chosen by hand, one per district at most; a district
    /// without one (or whose one is cut) uses the automatic refuges.
    #[serde(default)]
    pub refuges: Vec<AuthoredRefuge>,
}

/// An authored gathering area (`evacuation.refuges` in `game.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct AuthoredRefuge {
    pub district: String,
    /// As shown to the player after «verso», article included.
    pub name: String,
    pub pos: [f32; 2],
}

impl Default for EvacRules {
    fn default() -> Self {
        EvacRules { per_min: 20.0, prep_min: 5.0, refuges: vec![] }
    }
}

/// Where a family is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// At home, no order.
    Home,
    /// Ordered out, waiting for its turn (at home).
    Queued,
    /// Driving to the refuge.
    Road,
    /// On the road, every way ahead cut: stopped.
    Stuck,
    /// Reached the refuge.
    Safe,
    /// The evacuation was interrupted before it left: sheltering at home.
    SelfDefence,
    /// The fire reached it.
    Caught,
}

#[derive(Debug, Clone)]
pub struct Family {
    pub district: usize,
    pub home: Pos,
    pub people: u32,
    pub state: State,
    pub caught_at: Option<i64>,
    /// The way it is driving, and how far along it, metres.
    path: Option<Arc<Path>>,
    along_m: f32,
}

impl Family {
    /// Where it is now.
    pub fn pos(&self) -> Pos {
        match &self.path {
            Some(p) if matches!(self.state, State::Road | State::Stuck | State::Safe) => p.at(self.along_m),
            _ => self.home,
        }
    }
}

/// A route as a polyline with its cumulative length.
#[derive(Debug, Clone)]
pub struct Path {
    pub points: Vec<Pos>,
    cum: Vec<f32>,
    /// The refuge's name for the screen.
    pub to: String,
}

impl Path {
    fn new(points: Vec<Pos>, to: String) -> Path {
        let mut cum = Vec::with_capacity(points.len());
        let mut s = 0.0;
        for (k, p) in points.iter().enumerate() {
            if k > 0 {
                s += dist(points[k - 1], *p);
            }
            cum.push(s);
        }
        Path { points, cum, to }
    }

    pub fn length_m(&self) -> f32 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    pub fn at(&self, m: f32) -> Pos {
        let k = self.cum.partition_point(|&c| c <= m);
        if k == 0 {
            return self.points[0];
        }
        if k >= self.points.len() {
            return *self.points.last().unwrap();
        }
        let (a, b) = (self.points[k - 1], self.points[k]);
        let seg = self.cum[k] - self.cum[k - 1];
        let t = if seg > 0.0 { (m - self.cum[k - 1]) / seg } else { 0.0 };
        Pos { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t }
    }

    /// Whether the fire cuts the route beyond `from_m`.
    fn cut_after(&self, from_m: f32, fire: &FireSim) -> bool {
        let mut m = from_m;
        let end = self.length_m();
        while m < end {
            if fire.threat().at(self.at(m)) >= IMPASSABLE {
                return true;
            }
            m += SAMPLE_M;
        }
        fire.threat().at(self.at(end)) >= IMPASSABLE
    }

    /// The rest of the route from `from_m`.
    pub fn rest(&self, from_m: f32) -> Vec<Pos> {
        let k = self.cum.partition_point(|&c| c <= from_m);
        std::iter::once(self.at(from_m)).chain(self.points[k.min(self.points.len())..].iter().copied()).collect()
    }
}

/// What an evacuation of a district would be, from the state now
/// ([`Evacuations::plan`]): shown before the player confirms.
#[derive(Debug, Clone)]
pub struct EvacPlan {
    pub district: usize,
    pub path: Arc<Path>,
    pub people: u32,
    pub families: usize,
    /// Preparation, departures at the limited rate, and the mean drive.
    pub prep_min: f32,
    pub leaving_min: f32,
    pub drive_min: f32,
    /// Until the last family reaches the refuge: what the player is told,
    /// and where the timer starts ([`Evacuations::remaining_min`]).
    pub finish_min: f32,
}

impl EvacPlan {
    pub fn total_min(&self) -> f32 {
        self.finish_min
    }
}

/// Minutes until the last of `families` reaches the end of `path`: the ones
/// on the road drive the rest of their way; the queued ones, in turn, leave
/// after `prep_left_min` when the departures (`per_min`, with `credit`
/// people already allowed) reach them, then drive from home along `path`.
/// The rule of [`Evacuations::step`], in closed form.
fn finish_min<'a>(families: impl Iterator<Item = &'a Family>, path: &Path, prep_left_min: f32, per_min: f32, credit: f32) -> f32 {
    let (mut out, mut people) = (0.0f32, 0.0f32);
    let start = path.points.first().copied();
    for f in families {
        let t = match (f.state, &f.path) {
            (State::Road, Some(p)) => (p.length_m() - f.along_m).max(0.0) / CONVOY_MPS / 60.0,
            (State::Home | State::SelfDefence | State::Queued, _) => {
                people += f.people as f32;
                let leg = start.map_or(0.0, |s| dist(f.home, s)) + path.length_m();
                prep_left_min + (people - credit).max(0.0) / per_min.max(0.1) + leg / CONVOY_MPS / 60.0
            }
            _ => continue,
        };
        out = out.max(t);
    }
    out
}

/// An evacuation in progress or over.
#[derive(Debug, Clone)]
pub struct Evacuation {
    pub district: usize,
    pub started_s: i64,
    pub path: Arc<Path>,
    /// Families still to leave, in turn.
    queue: VecDeque<usize>,
    credit: f32,
    /// When the last family left, or the evacuation was interrupted.
    pub ended_s: Option<i64>,
    pub interrupted: bool,
    /// Times the route was changed because the fire cut it.
    pub rerouted: u32,
    /// The families it queued and their state before: what [`Evacuations::cancel`] restores.
    before: Vec<(usize, State)>,
}

impl Evacuation {
    pub fn waiting(&self) -> usize {
        self.queue.len()
    }
}

/// Something the player should read in the log.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Rerouted { district: usize },
    Interrupted { district: usize, families: usize },
    Done { district: usize },
}

pub struct Evacuations {
    pub rules: EvacRules,
    pub families: Vec<Family>,
    /// The current evacuation and the past ones, in order.
    pub runs: Vec<Evacuation>,
    refuges: Vec<(NodeId, Pos, String)>,
    /// Authored gathering areas: district, node, position, name.
    authored: Vec<(usize, NodeId, Pos, String)>,
    next_check_s: i64,
}

impl Evacuations {
    pub fn new(scn: &Scenario, net: &RoadNetwork, districts: &[District], rules: EvacRules) -> Evacuations {
        let mut families: Vec<Family> = scn
            .population
            .households
            .iter()
            .map(|h| Family { district: usize::MAX, home: Pos::from(h.pos), people: h.size as u32, state: State::Home, caught_at: None, path: None, along_m: 0.0 })
            .collect();
        for (d, dd) in districts.iter().enumerate() {
            for &i in &dd.households {
                families[i].district = d;
            }
        }
        let refuges = abm::refuge::choose(scn, net, MAX_REFUGES)
            .into_iter()
            .map(|r| {
                let name = refuge_name(r.pos, districts, r.is_exit);
                (r.node, r.pos, name)
            })
            .collect();
        // authored areas, snapped to the network the district's road is on
        let authored = rules
            .refuges
            .iter()
            .filter_map(|r| {
                let d = districts.iter().position(|d| d.name == r.district)?;
                let from = net.nearest(districts[d].departure, true)?;
                let node = net.nearest_reachable(Pos::from(r.pos), true, from)?;
                Some((d, node, net.pos(node), r.name.clone()))
            })
            .collect();
        Evacuations { rules, families, runs: vec![], refuges, authored, next_check_s: 0 }
    }

    /// The evacuation being carried out (families still leaving).
    pub fn active(&self) -> Option<&Evacuation> {
        self.runs.last().filter(|e| e.ended_s.is_none())
    }

    pub fn of(&self, d: usize) -> Option<&Evacuation> {
        self.runs.iter().rev().find(|e| e.district == d)
    }

    /// The shortest open route from district `d` to a refuge out of it: its
    /// authored gathering area if there is one and the way is open, else the
    /// nearest automatic refuge.
    fn route(&self, d: usize, from: Pos, districts: &[District], net: &RoadNetwork, fire: &FireSim) -> Option<Arc<Path>> {
        let homes: Vec<Pos> = districts.get(d).map(|dd| dd.households.iter().map(|&i| self.families[i].home).collect()).unwrap_or_default();
        let own: Vec<(NodeId, Pos, String)> = self.authored.iter().filter(|a| a.0 == d).map(|a| (a.1, a.2, a.3.clone())).collect();
        if !own.is_empty() {
            if let Some(p) = self.route_in(&own, from, &homes, net, fire) {
                return Some(p);
            }
        }
        self.route_in(&self.refuges, from, &homes, net, fire)
    }

    /// The shortest open route to one of `refuges`.
    fn route_in(&self, refuges: &[(NodeId, Pos, String)], from: Pos, homes: &[Pos], net: &RoadNetwork, fire: &FireSim) -> Option<Arc<Path>> {
        let a = net.nearest(from, true)?;
        let mut best: Option<Path> = None;
        for (node, pos, name) in refuges {
            if homes.iter().any(|&h| dist(h, *pos) < REFUGE_CLEAR_M) || net.component(*node, true) != net.component(a, true) {
                continue;
            }
            let Some(nodes) = network::route(net, a, *node, fire.threat(), true) else { continue };
            let points: Vec<Pos> = std::iter::once(from).chain(std::iter::once(net.pos(a))).chain(nodes.iter().map(|&n| net.pos(n))).collect();
            let p = Path::new(points, name.clone());
            if p.cut_after(0.0, fire) {
                continue;
            }
            if best.as_ref().is_none_or(|b| p.length_m() < b.length_m()) {
                best = Some(p);
            }
        }
        best.map(Arc::new)
    }

    /// What evacuating district `d` now would take, or `None` if no open
    /// road leads out. Reads the state, changes nothing.
    pub fn plan(&self, d: usize, districts: &[District], net: &RoadNetwork, fire: &FireSim) -> Option<EvacPlan> {
        let dd = districts.get(d)?;
        let path = self.route(d, dd.departure, districts, net, fire)?;
        let left: Vec<usize> = dd.households.iter().copied().filter(|&i| matches!(self.families[i].state, State::Home | State::SelfDefence)).collect();
        let people: u32 = left.iter().map(|&i| self.families[i].people).sum();
        let mut order = left.clone();
        order.sort_by(|&a, &b| dist(self.families[a].home, dd.departure).total_cmp(&dist(self.families[b].home, dd.departure)).then(a.cmp(&b)));
        let finish = finish_min(order.iter().map(|&i| &self.families[i]), &path, self.rules.prep_min, self.rules.per_min, 0.0);
        let mean_home_leg = if left.is_empty() { 0.0 } else { left.iter().map(|&i| dist(self.families[i].home, dd.departure)).sum::<f32>() / left.len() as f32 };
        Some(EvacPlan {
            district: d,
            people,
            families: left.len(),
            prep_min: self.rules.prep_min,
            leaving_min: people as f32 / self.rules.per_min.max(0.1),
            drive_min: (path.length_m() + mean_home_leg) / CONVOY_MPS / 60.0,
            finish_min: finish,
            path,
        })
    }

    /// Order district `d` out along the plan's route.
    pub fn start(&mut self, plan: &EvacPlan, districts: &[District], now: i64) -> Result<(), String> {
        if let Some(e) = self.active() {
            return Err(format!("si sta già evacuando {}", districts[e.district].name));
        }
        let dd = &districts[plan.district];
        let mut queue: Vec<usize> = dd.households.iter().copied().filter(|&i| matches!(self.families[i].state, State::Home | State::SelfDefence)).collect();
        if queue.is_empty() {
            return Err(format!("a {} non c'è più nessuno da evacuare", dd.name));
        }
        queue.sort_by(|&a, &b| dist(self.families[a].home, dd.departure).total_cmp(&dist(self.families[b].home, dd.departure)).then(a.cmp(&b)));
        let before = queue.iter().map(|&i| (i, self.families[i].state)).collect();
        for &i in &queue {
            self.families[i].state = State::Queued;
        }
        self.runs.push(Evacuation {
            district: plan.district,
            started_s: now,
            path: plan.path.clone(),
            queue: queue.into(),
            credit: 0.0,
            ended_s: None,
            interrupted: false,
            rerouted: 0,
            before,
        });
        Ok(())
    }

    /// Take back the evacuation in course if it was ordered at `since` or
    /// later and nothing has happened yet: nobody has left. Returns its
    /// district.
    pub fn cancel(&mut self, since: i64) -> Option<usize> {
        let e = self.runs.last().filter(|e| e.ended_s.is_none() && e.started_s >= since && e.queue.len() == e.before.len())?;
        let d = e.district;
        let e = self.runs.pop()?;
        for (i, s) in e.before {
            self.families[i].state = s;
        }
        Some(d)
    }

    /// Minutes until the last family of district `d`'s evacuation reaches
    /// the refuge, `None` once nobody is left waiting or driving. With the
    /// same rule as the plan, so the timer starts at what was announced.
    pub fn remaining_min(&self, d: usize, now: i64) -> Option<f32> {
        let e = self.of(d)?;
        let waiting = e.queue.iter().map(|&i| &self.families[i]).filter(|f| f.state == State::Queued);
        let road = e.before.iter().map(|&(i, _)| &self.families[i]).filter(|f| f.state == State::Road);
        if waiting.clone().next().is_none() && road.clone().next().is_none() {
            return None;
        }
        let prep_left = (self.rules.prep_min - (now - e.started_s) as f32 / 60.0).max(0.0);
        Some(finish_min(road.chain(waiting), &e.path, prep_left, self.rules.per_min, e.credit))
    }

    /// One step of `dt` simulated seconds ending at `now`.
    pub fn step(&mut self, dt: f32, now: i64, districts: &[District], net: &RoadNetwork, fire: &FireSim) -> Vec<Event> {
        let mut events = vec![];
        let check = now >= self.next_check_s;
        if check {
            self.next_check_s = now + CHECK_S;
        }
        // the route out of the district being emptied
        if let Some(k) = self.runs.iter().position(|e| e.ended_s.is_none()) {
            let d = self.runs[k].district;
            if check && self.runs[k].path.cut_after(0.0, fire) {
                match self.route(d, districts[d].departure, districts, net, fire) {
                    Some(p) => {
                        self.runs[k].path = p;
                        self.runs[k].rerouted += 1;
                        events.push(Event::Rerouted { district: d });
                    }
                    None => {
                        let e = &mut self.runs[k];
                        e.interrupted = true;
                        e.ended_s = Some(now);
                        let n = e.queue.len();
                        for i in e.queue.drain(..) {
                            self.families[i].state = State::SelfDefence;
                        }
                        events.push(Event::Interrupted { district: d, families: n });
                    }
                }
            }
            let e = &mut self.runs[k];
            if e.ended_s.is_none() && now as f32 >= e.started_s as f32 + self.rules.prep_min * 60.0 {
                e.credit += self.rules.per_min / 60.0 * dt;
                while let Some(&i) = e.queue.front() {
                    let f = &mut self.families[i];
                    if f.state != State::Queued {
                        e.queue.pop_front();
                        continue;
                    }
                    if e.credit < f.people as f32 {
                        break;
                    }
                    e.credit -= f.people as f32;
                    e.queue.pop_front();
                    // home, then the district's route
                    let mut pts = vec![f.home];
                    pts.extend(e.path.points.iter().copied());
                    f.path = Some(Arc::new(Path::new(pts, e.path.to.clone())));
                    f.along_m = 0.0;
                    f.state = State::Road;
                }
                if e.queue.is_empty() {
                    e.ended_s = Some(now);
                    events.push(Event::Done { district: d });
                }
                // credit does not pile up while the next family is bigger
                e.credit = e.credit.min(self.rules.per_min.max(10.0));
            }
        }
        // families on the road
        for i in 0..self.families.len() {
            let f = &mut self.families[i];
            if !matches!(f.state, State::Road | State::Stuck) {
                continue;
            }
            let Some(path) = f.path.clone() else { continue };
            if check && path.cut_after(f.along_m, fire) {
                let here = path.at(f.along_m);
                let to = path.to.clone();
                let d = f.district;
                match self.reroute(d, here, districts, net, fire) {
                    Some(p) => {
                        let f = &mut self.families[i];
                        f.path = Some(p);
                        f.along_m = 0.0;
                        f.state = State::Road;
                    }
                    None => {
                        let _ = to;
                        self.families[i].state = State::Stuck;
                    }
                }
            }
            let f = &mut self.families[i];
            if f.state == State::Road {
                let path = f.path.as_ref().unwrap();
                f.along_m += CONVOY_MPS * dt;
                if f.along_m >= path.length_m() {
                    f.along_m = path.length_m();
                    f.state = State::Safe;
                }
            }
        }
        // the fire reaching people
        let ex = fire.exposure().fields();
        for (i, f) in self.families.iter_mut().enumerate() {
            let threat = fire.threat().at(f.pos());
            let hit = ex.get(i).is_some_and(|e| e.alight);
            let caught = match f.state {
                State::Home | State::Queued => hit || threat >= ALARMING,
                State::SelfDefence => hit || threat >= IMPASSABLE,
                State::Road | State::Stuck => threat >= IMPASSABLE,
                State::Safe | State::Caught => false,
            };
            if caught {
                f.state = State::Caught;
                f.caught_at = Some(now);
            }
        }
        events
    }

    fn reroute(&self, d: usize, from: Pos, districts: &[District], net: &RoadNetwork, fire: &FireSim) -> Option<Arc<Path>> {
        self.route(d, from, districts, net, fire)
    }

    /// People per state in district `d` (all districts with `None`).
    pub fn count(&self, d: Option<usize>, s: State) -> u32 {
        self.families.iter().filter(|f| f.state == s && d.is_none_or(|d| f.district == d)).map(|f| f.people).sum()
    }
}

/// «l'area di raccolta di X» for a refuge in a district, «il porto» for one
/// by the sea at the east, else the nearest district's name.
fn refuge_name(p: Pos, districts: &[District], is_exit: bool) -> String {
    let near = districts.iter().min_by(|a, b| dist(a.centre, p).total_cmp(&dist(b.centre, p)));
    match near {
        _ if is_exit => "l'uscita".into(),
        Some(d) if dist(d.centre, p) < 900.0 => format!("l'area di raccolta di {}", d.name),
        Some(d) => format!("l'area sicura vicino a {}", d.name),
        None => "l'area sicura".into(),
    }
}
