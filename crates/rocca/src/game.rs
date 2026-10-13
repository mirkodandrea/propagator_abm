//! One game: the single simulation and gameplay authority. The headless
//! runner and (from phase 5) the kiosk step this and nothing else.
//!
//! Order of a step, every [`STEP_S`] simulated seconds: fire, civilians,
//! coordinator review (every [`REVIEW_S`]), units, structure protection,
//! bookkeeping. The active plan changes only through [`Game::commit`]; the
//! proposed plan is [`Game::preview`], which reads the state and changes
//! nothing.

use std::path::Path;

use abm::suppression::{Task, UnitEffect, UnitKind, UnitState};
use abm::{Abm, Suppression};
use anyhow::{Context, Result};
use fire::FireSim;
use scenario::population::Status;
use scenario::{Pos, Scenario};

use crate::case::{Case, Territory};
use crate::coordinator::{self, Post, Proposal, View, DEFEND_REACH_M, ON_POST_M, REVIEW_S};
use crate::crisis::{Closing, Crisis, Detector};
use crate::district::{self, dist, District};
use crate::plan::{Civil, Plan};

/// Simulated seconds per step (the units' and civilians' decision interval).
pub const STEP_S: i64 = 6;
/// Capacità di spegnimento rispetto al modello calibrato (2026-10-13, richiesta
/// dell'utente): la stessa acqua bagna 30 volte più celle, i getti arrivano 10
/// volte più lontano, la linea tagliata dalle squadre è 3 volte (30 era
/// eccessiva: 3900 m a partita contro 205).
pub const SUPPRESSION: UnitEffect = UnitEffect { line_x: 3.0, water_x: 30.0, reach_x: 10.0, nerve_x: 1.0 };
/// A unit on the move that has not moved for this long is stuck: its road is
/// cut and no other is open.
pub const STUCK_S: i64 = 5 * 60;

/// How much a unit on its post protects the homes around it (0-1), by kind:
/// an engine with water does the full job, an empty one or a hand crew (no
/// water, beaters and hand tools against embers) part of it.
pub fn protection_of(kind: UnitKind, water_l: f32) -> f32 {
    match kind {
        UnitKind::Engine if water_l > 0.0 => 1.0,
        UnitKind::Engine => 0.5,
        UnitKind::HandCrew => 0.6,
        UnitKind::AirTanker => 0.0,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    pub at_s: i64,
    pub text: String,
}

/// Simulated facts per district. "Colpite" are homes the structure exposure
/// model ignited (`ExposureField::alight`): a model fact, not a proxy.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DistrictOutcome {
    pub households: usize,
    pub homes_hit: usize,
    /// Households the fire reached while still at home, ever.
    pub caught: usize,
    pub evacuated: usize,
    pub on_the_road: usize,
    pub at_home: usize,
    pub casualties: usize,
}

/// Where a unit posted to a district stands ([`Game::arrivals`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Arrival {
    OnPost,
    /// On its way, minutes left by the route it is driving.
    InMin(u32),
    /// Moving but stopped for [`STUCK_S`]: the road is cut.
    Blocked,
    /// Withdrawing, refilling or otherwise not heading there, in words.
    Away(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    pub at_s: i64,
    pub districts: Vec<DistrictOutcome>,
    pub hectares: f32,
    pub units_lost: usize,
}

impl Outcome {
    pub fn homes_hit(&self) -> usize {
        self.districts.iter().map(|d| d.homes_hit).sum()
    }
    pub fn caught(&self) -> usize {
        self.districts.iter().map(|d| d.caught).sum()
    }
}

/// How far the fire must be from families still at home for the kiosk to
/// count the game as quiet ([`Game::is_quiet`]).
pub const QUIET_FIRE_M: f32 = 1500.0;

/// An order this long before the fire reached the doors earns full
/// Tempismo ([`Game::timing`]).
pub const TIMELY_S: i64 = 30 * 60;

/// How often the debrief's "nearest the fire came" is sampled.
const NEAREST_EVERY_S: i64 = 60;

/// How the debrief judges an order ([`Game::order_review`]): it helped, it may not
/// have been needed, or it was missing or late.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Bene,
    Forse,
    Male,
}

/// How close the fire came to a district: farther than [`QUIET_FIRE_M`],
/// nearer but never at its doors, or at its doors from a time.
enum Reach {
    Far,
    Near,
    Doors(i64),
}

/// One line of «Come sono andati i tuoi ordini», for one district.
#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    pub verdict: Verdict,
    pub text: String,
}

/// How many separate ways out [`Game::evacuation_routes`] draws.
pub const ROUTES_SHOWN: usize = 12;

/// District's ways out ([`Game::evacuation_routes`]).
#[derive(Debug, Clone, Default)]
pub struct EvacRoutes {
    /// Home, then road nodes to the refuge, metres.
    pub paths: Vec<Vec<Pos>>,
    /// Where they end, in Italian («l'area di raccolta di X», «il porto»).
    pub to: Vec<String>,
    /// Typical and longest journey of the families at home, minutes.
    pub minutes: Option<(u32, u32)>,
    /// Families at home with no way out.
    pub no_way: usize,
}

pub struct Game {
    pub scn: Scenario,
    pub fire: FireSim,
    pub agents: Abm,
    pub crews: Suppression,
    pub territory: Territory,
    pub case: Case,
    pub seed: u64,
    pub districts: Vec<District>,
    /// The plan in force.
    pub active: Plan,
    /// The coordinator's posts for the active plan, one per unit.
    pub posts: Vec<Option<Post>>,
    pub uncovered: Vec<(usize, String)>,
    pub log: Vec<LogEntry>,
    /// A crisis raised and not yet taken by the caller ([`Game::take_crisis`]).
    pub crisis: Option<Crisis>,
    detector: Detector,
    /// The player has committed a first plan.
    planned: bool,
    next_review_s: i64,
    caught_at: Vec<Option<i64>>,
    /// Per household: when it left home (on the road or already safe).
    left_at: Vec<Option<i64>>,
    /// Per district: when the pre-alert and the evacuation order went out.
    ordered_at: Vec<(Option<i64>, Option<i64>)>,
    /// Per district: simulated seconds with at least one unit on its post.
    defended_s: Vec<i64>,
    /// Per district: when the threat at one of its doors first reached
    /// [`fire::threat::ALARMING`], the level that counts a family caught.
    threatened_at: Vec<Option<i64>>,
    /// Per district: the nearest the burning front came to one of its homes,
    /// sampled every [`NEAREST_EVERY_S`].
    nearest_m: Vec<f32>,
    /// Per unit: where it last moved and when, to tell a unit stuck on a
    /// closed road from one on its way.
    moved: Vec<(Pos, i64)>,
    /// Per unit: the state last logged, to log a withdrawal or a loss once.
    logged: Vec<UnitState>,
    /// Teams carrying orders to the door, from `game.json`; `None` sends
    /// every order over the households' own channels ([`crate::messengers`]).
    pub messengers: Option<crate::Messengers>,
    rounds: Vec<crate::messengers::Round>,
    /// The teams, built on the first round.
    pub teams: Vec<crate::messengers::Team>,
    /// Evacuation needs the mayor's authorisation ([`crate::mayor`]), from
    /// `game.json`; `None` lets the player evacuate any district at any time
    /// (the rules before 2026-10-13).
    pub mayor: Option<crate::Mayor>,
    /// Per district: when the mayor authorised its evacuation.
    authorised_at: Vec<Option<i64>>,
    /// Authorisations not yet taken by the caller ([`Game::take_authorisations`]).
    fresh: Vec<usize>,
    closing: Closing,
}

impl Game {
    /// A new game on the territory in `data_dir` (`data`), fire `case`.
    pub fn new(data_dir: &Path, case: &str, seed: u64) -> Result<Game> {
        let territory = Territory::load(data_dir)?;
        let case = territory.case(case).with_context(|| format!("caso {case:?} assente in game.json"))?.clone();
        let scn = Scenario::load(data_dir)?;
        let mut fire = FireSim::new(&scn, territory.weather(&case), seed)?;
        fire.ignite_patch(scn.world.cell_of(case.ignition()), case.radius_m, &scn)?;
        // The behaviour graphs the civilians and units decide by: the shipped
        // library in `data/behaviours`, the same for every caller.
        let lib = behavior::Library::load_dir_reported(&data_dir.join(behavior::library::DEFAULT_DIR))?.library;
        lib.validate_runtime().map_err(|e| anyhow::anyhow!("behaviour library: {e}"))?;
        let households = abm::BehaviorRuntime::build(&lib)
            .map_err(|e| anyhow::anyhow!("household behaviour: {e}"))?
            .context("household behaviour: no profile has a positive share")?;
        let persons = abm::PersonRuntime::build(&lib)
            .map_err(|e| anyhow::anyhow!("person behaviour: {e}"))?
            .context("person behaviour: no profile has a positive share")?;
        let policy = abm::UnitRuntime::build(&lib)
            .map_err(|e| anyhow::anyhow!("unit behaviour: {e}"))?
            .context("unit behaviour: no profile is enabled")?;
        let mut agents = Abm::with_behaviours(&scn, seed, households, persons)?;
        agents.set_exit_capacity(territory.exit_per_min);
        let roster: Vec<(UnitKind, Pos)> =
            territory.roster.iter().map(|s| (s.kind.unit_kind(), Pos::from(territory.stations[s.station].pos))).collect();
        let mut crews = Suppression::with_roster(&scn, &roster, policy)?;
        crews.effect = SUPPRESSION;
        let districts = district::of(&scn, &agents);
        let n = agents.households.len();
        let units = crews.units.len();
        let mut game = Game {
            active: Plan::new(districts.len()),
            posts: vec![None; units],
            uncovered: vec![],
            log: vec![],
            crisis: None,
            detector: Detector::default(),
            planned: false,
            next_review_s: 0,
            caught_at: vec![None; n],
            left_at: vec![None; n],
            ordered_at: vec![(None, None); districts.len()],
            defended_s: vec![0; districts.len()],
            threatened_at: vec![None; districts.len()],
            nearest_m: vec![f32::INFINITY; districts.len()],
            moved: crews.units.iter().map(|u| (u.pos, 0)).collect(),
            logged: crews.units.iter().map(|u| u.state).collect(),
            messengers: territory.messengers,
            rounds: vec![],
            teams: vec![],
            mayor: territory.mayor,
            authorised_at: vec![None; districts.len()],
            fresh: vec![],
            closing: Closing::default(),
            scn,
            fire,
            agents,
            crews,
            territory,
            case,
            seed,
            districts,
        };
        // The ignition is a boundary condition the core applies on its next
        // advance: one step puts the opening fire into the state, so the
        // player plans (×0) on a fire that is actually there.
        game.step()?;
        Ok(game)
    }

    pub fn time_s(&self) -> i64 {
        self.fire.time_s()
    }

    /// How far the active civil order of district `d` has got: the households
    /// that have heard it (over their channel or from a team at the door) and
    /// all of them. `None` while the district has no order.
    pub fn alerted(&self, d: usize) -> Option<(usize, usize)> {
        let hh = &self.districts.get(d)?.households;
        let heard = |i: usize| {
            let h = &self.agents.households[i];
            match self.active.civil[d] {
                Civil::Evacua => h.warning_received,
                Civil::Preallerta => h.prealert_received || h.warning_received,
                Civil::Nessuno => false,
            }
        };
        (self.active.civil[d] != Civil::Nessuno).then(|| (hh.iter().filter(|&&i| heard(i)).count(), hh.len()))
    }

    pub fn district_index(&self, name: &str) -> Option<usize> {
        self.districts.iter().position(|d| d.name.eq_ignore_ascii_case(name) || d.name.to_lowercase().contains(&name.to_lowercase()))
    }

    fn view(&self) -> View<'_> {
        View { scn: &self.scn, agents: &self.agents, fire: &self.fire, crews: &self.crews, districts: &self.districts }
    }

    /// The proposed plan: what the coordinator would do with `plan` from the
    /// state now. Reads, never writes -- no unit moves until [`Game::commit`].
    pub fn preview(&self, plan: &Plan) -> Result<Proposal> {
        plan.validate(self.districts.len())?;
        Ok(coordinator::propose(&self.view(), plan, &self.posts))
    }

    /// Make `plan` the active plan: civil orders go out (escalations only),
    /// and the coordinator re-plans from the state now, revalidating the
    /// posts rather than trusting an older preview.
    pub fn commit(&mut self, plan: Plan) -> Result<Proposal> {
        plan.validate(self.districts.len())?;
        let plan = plan.escalated_from(&self.active);
        for (d, (&new, &old)) in plan.civil.iter().zip(&self.active.civil).enumerate() {
            anyhow::ensure!(
                new != Civil::Evacua || old == Civil::Evacua || self.may_evacuate(d),
                "evacuazione di {}: serve l'autorizzazione della sindaca",
                self.districts[d].name
            );
        }
        let now = self.time_s();
        let before = self.active.civil.clone();
        for (d, (&new, &old)) in plan.civil.iter().zip(&before).enumerate() {
            if new == old {
                continue;
            }
            let ids = self.districts[d].households.clone();
            let n = match (new, self.messengers) {
                (Civil::Nessuno, _) => 0,
                (Civil::Preallerta, None) => self.agents.prealert_of(&ids),
                (Civil::Evacua, None) => self.agents.order_evacuation_of(&ids),
                (_, Some(m)) => self.send_round(d, new == Civil::Evacua, m),
            };
            let at = &mut self.ordered_at[d];
            match new {
                Civil::Preallerta => at.0 = at.0.or(Some(now)),
                Civil::Evacua => at.1 = at.1.or(Some(now)),
                Civil::Nessuno => {}
            }
            let what = if new == Civil::Evacua { "evacuazione" } else { "preallerta" };
            self.log.push(LogEntry { at_s: now, text: format!("{what}: {} ({n} famiglie)", self.districts[d].name) });
        }
        if plan.priorities != self.active.priorities {
            let names: Vec<&str> = plan.priorities.iter().map(|&d| self.districts[d].name.as_str()).collect();
            self.log.push(LogEntry { at_s: now, text: format!("priorità: {}", names.join(" > ")) });
        }
        self.active = plan;
        self.planned = true;
        Ok(self.review_inner(false))
    }

    /// Open a messengers' round for district `d`: the households that will
    /// hear over their channel are told that way now, the rest queue for a
    /// team, nearest the centre first. Returns the households ordered.
    fn send_round(&mut self, d: usize, evacuate: bool, m: crate::Messengers) -> usize {
        let centre = self.districts[d].centre;
        let mut by_channel = vec![];
        let mut queue = vec![];
        for &i in &self.districts[d].households {
            let h = &self.agents.households[i];
            let listening = evacuate && h.prealert_received;
            let alert = m.broadcast && h.channel == scenario::population::WarningChannel::MobileAlert;
            if listening || alert { by_channel.push(i) } else { queue.push(i) }
        }
        if evacuate {
            self.agents.order_evacuation_of(&by_channel);
        } else {
            self.agents.prealert_of(&by_channel);
        }
        let homes = &self.agents.households;
        queue.sort_by(|&a, &b| dist(homes[a].home, centre).total_cmp(&dist(homes[b].home, centre)).then(a.cmp(&b)));
        self.rounds.retain(|r| r.district != d);
        let n = by_channel.len() + queue.len();
        self.rounds.push(crate::messengers::Round { district: d, evacuate, queue: queue.into() });
        n
    }

    /// One step of the messengers: the fire's heat on the teams, then they
    /// knock, pick a district, drive (and are diverted on the road when the
    /// rounds change), or pull back from the fire.
    fn knock(&mut self) {
        use crate::messengers::{Task as T, VEHICLE};
        use abm::drive::Leg;
        use abm::suppression::{BURNOVER_S, HEAT_RECOVERY, WORK_LIMIT};
        let Some(m) = self.messengers else { return };
        if self.teams.len() != m.teams {
            let st = &self.territory.stations;
            self.teams = (0..m.teams).map(|k| crate::messengers::Team::new(Pos::from(st[k % st.len()].pos))).collect();
        }
        self.rounds.retain(|r| !r.queue.is_empty());
        let now = self.time_s();
        let dt = STEP_S as f32;
        let open = |rounds: &[crate::messengers::Round], d: usize| rounds.iter().position(|r| r.district == d);
        for k in 0..self.teams.len() {
            if self.teams[k].task == T::Lost {
                continue;
            }
            self.teams[k].unreachable.retain(|&(_, until)| now < until);
            // As for the units: standing in flame is not a decision.
            let danger = self.fire.threat().at(self.teams[k].pos);
            let t = &mut self.teams[k];
            if danger >= fire::threat::IMPASSABLE {
                t.heat_s += dt * danger;
            } else {
                t.heat_s = (t.heat_s - dt * HEAT_RECOVERY).max(0.0);
            }
            if t.heat_s >= BURNOVER_S {
                t.task = T::Lost;
                let text = format!("una pattuglia che portava gli avvisi è stata raggiunta dal fuoco vicino a {}", self.near(self.teams[k].pos));
                self.log.push(LogEntry { at_s: now, text });
                continue;
            }
            if let T::Door { district, carry } = self.teams[k].task {
                if let Some(r) = open(&self.rounds, district) {
                    // Door to door: the team stands at each door it knocks
                    // on, and does not walk up to one the fire is at.
                    let hot = |g: &Game, i: usize| g.fire.threat().at(g.agents.households[i].home) >= WORK_LIMIT;
                    let mut carry = carry + dt / 60.0 * m.per_min;
                    let (mut told, mut skipped) = (vec![], 0);
                    while carry >= 1.0 && skipped < self.rounds[r].queue.len() {
                        let i = self.rounds[r].queue.pop_front().unwrap();
                        if hot(self, i) {
                            self.rounds[r].queue.push_back(i);
                            skipped += 1;
                            continue;
                        }
                        told.push(i);
                        carry -= 1.0;
                    }
                    if let Some(&i) = told.last() {
                        self.teams[k].pos = self.agents.households[i].home;
                        self.teams[k].drive.reset();
                    }
                    self.agents.tell_in_person(&told, self.rounds[r].evacuate);
                    self.teams[k].told += told.len();
                    let all_hot = !self.rounds[r].queue.is_empty() && self.rounds[r].queue.iter().all(|&i| hot(self, i));
                    if danger >= WORK_LIMIT || all_hot {
                        // Out, as the units disengage: the district is
                        // skipped for a while and its households left untold.
                        let t = &mut self.teams[k];
                        t.left += 1;
                        t.unreachable.push((district, now + abm::drive::REROUTE_S as i64));
                        t.task = T::Idle { retry_s: now };
                        self.say_once(now, format!("il fuoco è troppo vicino: le pattuglie lasciano {} senza finire gli avvisi", self.districts[district].name));
                    } else {
                        self.teams[k].task = T::Door { district, carry };
                    }
                } else {
                    self.teams[k].task = T::Idle { retry_s: now };
                }
            }
            if let T::Idle { retry_s } = self.teams[k].task {
                if let Some(d) = (now >= retry_s).then(|| self.choose(k, m, now)).flatten() {
                    self.teams[k].task = T::Driving { district: d };
                } else {
                    // Nothing to do: back to the station, as the units go
                    // back to staging, rather than waiting at the last door.
                    let t = &mut self.teams[k];
                    let base = t.base;
                    t.drive.step(&mut t.pos, &mut t.heading, base, now as f32, dt, VEHICLE, &self.agents.network, &self.agents.traffic, self.fire.threat());
                }
            }
            if let T::Driving { district } = self.teams[k].task {
                let Some(best) = self.choose(k, m, now) else {
                    self.teams[k].task = T::Idle { retry_s: now };
                    continue;
                };
                let closed = open(&self.rounds, district).is_none();
                let better = m.by_priority && self.team_rank(best, m) < self.team_rank(district, m);
                // Only for a district a road still leads to: otherwise the
                // team would turn round every minute for one it cannot reach.
                let reach = best != district && !closed && better && {
                    let t = &self.teams[k];
                    let ok = t.drive.can_reach(t.pos, self.districts[best].centre, VEHICLE, &self.agents.network, self.fire.threat());
                    if !ok {
                        self.teams[k].unreachable.push((best, now + 60));
                    }
                    ok
                };
                let to = if best != district && (closed || reach) {
                    self.teams[k].diverted += 1;
                    let text = format!("una pattuglia diretta a {} va invece a {}", self.districts[district].name, self.districts[best].name);
                    self.log.push(LogEntry { at_s: now, text });
                    best
                } else {
                    district
                };
                let goal = self.districts[to].centre;
                let t = &mut self.teams[k];
                let leg = t.drive.step(&mut t.pos, &mut t.heading, goal, now as f32, dt, VEHICLE, &self.agents.network, &self.agents.traffic, self.fire.threat());
                self.teams[k].task = match leg {
                    Leg::Moving => T::Driving { district: to },
                    Leg::Arrived { .. } => {
                        if !self.teams.iter().any(|t| matches!(t.task, T::Door { district, .. } if district == to)) {
                            let text = format!("una pattuglia arriva a {} per avvisare le famiglie", self.districts[to].name);
                            self.log.push(LogEntry { at_s: now, text });
                        }
                        T::Door { district: to, carry: 0.0 }
                    }
                    Leg::NoRoad => {
                        // Said once: the teams try again every minute.
                        self.say_once(now, format!("nessuna strada aperta per portare l'ordine a {}", self.districts[to].name));
                        self.teams[k].unreachable.push((to, now + 60));
                        T::Idle { retry_s: now }
                    }
                };
            }
        }
    }

    fn say_once(&mut self, at_s: i64, text: String) {
        if !self.log.iter().any(|e| e.text == text) {
            self.log.push(LogEntry { at_s, text });
        }
    }

    /// The name of the district nearest `p`.
    fn near(&self, p: Pos) -> &str {
        self.districts.iter().min_by(|a, b| dist(a.centre, p).total_cmp(&dist(b.centre, p))).map_or("", |d| d.name.as_str())
    }

    /// Where team `k` should go now: an open round it can reach, one without
    /// a team per household left, by priority or by fewest teams.
    fn choose(&self, k: usize, m: crate::Messengers, now: i64) -> Option<usize> {
        let on = |d: usize| self.teams.iter().enumerate().filter(|&(j, t)| j != k && t.district() == Some(d)).count();
        self.rounds
            .iter()
            .filter(|r| !self.teams[k].cut_off(r.district, now))
            .map(|r| {
                let (d, n) = (r.district, on(r.district));
                let full = n >= r.queue.len();
                let key = if m.by_priority { (full, self.team_rank(d, m), n) } else { (full, n, self.team_rank(d, m)) };
                (key, d)
            })
            .min()
            .map(|(_, d)| d)
    }

    /// District `d`'s place in the priorities as the teams read them.
    fn team_rank(&self, d: usize, m: crate::Messengers) -> usize {
        let r = self.active.priorities.iter().position(|&p| p == d).unwrap_or(usize::MAX);
        if m.reversed { usize::MAX - r } else { r }
    }

    /// Where district `d`'s families would go if ordered out now: the ways
    /// out of up to [`ROUTES_SHOWN`] households spread over the district
    /// (distinct first road nodes), where they end (the gathering area's
    /// district, or «il porto» for the map-edge exit), and the typical and
    /// longest journey in minutes. What «Evacua» shows before it is
    /// confirmed (user, 2026-10-13); the same routes the agents drive.
    pub fn evacuation_routes(&self, d: usize) -> EvacRoutes {
        let mut out = EvacRoutes::default();
        let mut seen = std::collections::HashSet::new();
        let hh = &self.districts[d].households;
        let mut minutes: Vec<u32> = vec![];
        let mut ends: Vec<(Pos, bool)> = vec![];
        for &i in hh {
            if !Self::at_home(self.agents.households[i].status) {
                continue;
            }
            match self.agents.evacuation_journey_s(i) {
                Some(s) => minutes.push((s / 60.0).ceil().max(1.0) as u32),
                None => out.no_way += 1,
            }
            let Some((path, exit)) = self.agents.evacuation_path(i) else { continue };
            let Some(&end) = path.last() else { continue };
            if !ends.iter().any(|(p, _)| *p == end) {
                ends.push((end, exit));
            }
            // one line per first road node: the rest share it
            if out.paths.len() < ROUTES_SHOWN && seen.insert((path[1].x as i64, path[1].y as i64)) {
                out.paths.push(path);
            }
        }
        minutes.sort_unstable();
        out.minutes = minutes.last().map(|&b| (minutes[minutes.len() / 2], b));
        out.to = ends
            .iter()
            .map(|&(p, exit)| {
                if exit {
                    "il porto".to_string()
                } else {
                    let near = (0..self.districts.len()).min_by(|&a, &b| dist(self.districts[a].centre, p).total_cmp(&dist(self.districts[b].centre, p)));
                    near.map_or_else(|| "l'area di raccolta".to_string(), |n| format!("l'area di raccolta di {}", self.districts[n].name))
                }
            })
            .collect();
        out.to.dedup();
        out
    }

    /// Whether the player may order district `d` to evacuate now.
    pub fn may_evacuate(&self, d: usize) -> bool {
        self.mayor.is_none() || self.authorised_at[d].is_some()
    }

    /// When the mayor authorised district `d`'s evacuation.
    pub fn authorised_at(&self, d: usize) -> Option<i64> {
        self.authorised_at[d]
    }

    /// When the fire first reached district `d`'s doors (the level that
    /// counts a family caught), at home or not.
    pub fn at_doors_at(&self, d: usize) -> Option<i64> {
        self.threatened_at[d]
    }

    /// Districts the mayor has authorised since the last call, in the order
    /// she did. The kiosk opens a decision moment at ×1 on them; a headless
    /// strategy decides on them.
    pub fn take_authorisations(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.fresh)
    }

    /// The mayor looks at every district not yet authorised: the front's
    /// distance and how fast it has been closing, or the fire at the doors.
    fn authorise(&mut self) {
        let Some(mayor) = self.mayor else { return };
        let now = self.time_s();
        let n = self.districts.len();
        for d in 0..n {
            let Some(e) = district::exposure(&self.districts[d], &self.agents, &self.fire, &self.scn) else { continue };
            let speed = self.closing.observe(n, d, now, e.distance_m);
            if self.authorised_at[d].is_some() {
                continue;
            }
            let eta_s = e.distance_m / speed.max(0.05);
            if self.threatened_at[d].is_none() && eta_s > mayor.lead_s as f32 {
                continue;
            }
            self.authorised_at[d] = Some(now);
            self.fresh.push(d);
            let name = &self.districts[d].name;
            let when = if self.threatened_at[d].is_some() {
                "il fuoco è alle case".to_string()
            } else {
                format!("il fuoco può arrivare in circa {} min", ((eta_s / 60.0).ceil() as i64).max(1))
            };
            self.log.push(LogEntry { at_s: now, text: format!("la sindaca autorizza l'evacuazione di {name}: {when}") });
            // the decision it opens: risk, time and the cost, never the answer
            let home = self.districts[d].households.iter().filter(|&&i| Self::at_home(self.agents.households[i].status)).count();
            let text = format!(
                "A {name} {when}, e {} ancora in casa. Ho firmato: ora puoi ordinare l'evacuazione. Se il fuoco poi non arriva, le famiglie saranno partite per niente.",
                if home == 1 { "c'è 1 famiglia".to_string() } else { format!("ci sono {home} famiglie") }
            );
            if self.crisis.is_none() {
                let title = format!("La sindaca autorizza l'evacuazione di {name}");
                self.crisis = Some(Crisis { at_s: now, kind: crate::crisis::Kind::Autorizzazione { district: d }, text, title });
            }
        }
    }

    /// Tempismo, 0-1 (01-SPEC-GIOCO §5): over the families of the districts
    /// the fire reached at the doors, how early the first order (pre-alert
    /// or evacuation) came before it, full marks from [`TIMELY_S`]; nothing
    /// for a family caught, at home or on the road. `None` when the fire
    /// reached no district's doors.
    pub fn timing(&self) -> Option<f64> {
        let (mut sum, mut n) = (0.0f64, 0usize);
        for (d, district) in self.districts.iter().enumerate() {
            let Some(t) = self.threatened_at[d] else { continue };
            let (pre, evac) = self.ordered_at[d];
            let first = match (pre, evac) {
                (Some(p), Some(e)) => Some(p.min(e)),
                (p, e) => p.or(e),
            };
            for &i in &district.households {
                n += 1;
                let caught = self.caught_at[i].is_some() || matches!(self.agents.households[i].status, Status::Trapped | Status::Casualty);
                if let (false, Some(o)) = (caught, first) {
                    sum += ((t - o) as f64 / TIMELY_S as f64).clamp(0.0, 1.0);
                }
            }
        }
        (n > 0).then(|| sum / n as f64)
    }

    /// Misura, 0-1 (01-SPEC-GIOCO §5): 1 less the share of the families of
    /// districts the fire never reached at the doors that left on an
    /// evacuation order ([`Game::needless_evacuations`]). 1 when the fire
    /// reached every district.
    pub fn measure(&self) -> f64 {
        let n: usize = (0..self.districts.len()).filter(|&d| self.threatened_at[d].is_none()).map(|d| self.districts[d].households.len()).sum();
        if n == 0 { 1.0 } else { 1.0 - self.needless_evacuations() as f64 / n as f64 }
    }

    /// Re-plan the active plan and hand the units their tasks; a periodic
    /// review also looks for a crisis.
    fn review(&mut self) -> Proposal {
        self.review_inner(true)
    }

    fn review_inner(&mut self, look_for_crisis: bool) -> Proposal {
        let p = coordinator::propose(&self.view(), &self.active, &self.posts);
        let now = self.time_s();
        for (k, post) in p.posts.iter().enumerate() {
            let changed = post.as_ref().map(|x| (x.district, x.at)) != self.posts[k].as_ref().map(|x| (x.district, x.at));
            let (state, task, callsign) = {
                let u = &self.crews.units[k];
                (u.state, u.task, u.callsign.clone())
            };
            if matches!(state, UnitState::Lost | UnitState::Withdrawing) {
                continue;
            }
            match post {
                Some(x) => {
                    if task != (Task::Attack { at: x.at }) && self.crews.assign(k, Task::Attack { at: x.at }).is_ok() && changed {
                        self.log.push(LogEntry { at_s: now, text: x.reason.clone() });
                    }
                }
                None => {
                    if matches!(task, Task::Attack { .. }) {
                        let _ = self.crews.assign(k, Task::Return);
                        let why = p.idle.iter().find(|(u, _)| *u == k).map_or_else(|| format!("{callsign} rientra alla base"), |(_, w)| w.clone());
                        self.log.push(LogEntry { at_s: now, text: why });
                    }
                }
            }
        }
        for (d, why) in &p.uncovered {
            if !self.uncovered.iter().any(|(x, w)| x == d && w == why) {
                self.log.push(LogEntry { at_s: now, text: why.clone() });
            }
        }
        self.posts = p.posts.clone();
        self.uncovered = p.uncovered.clone();
        self.next_review_s = now + REVIEW_S;
        // No crisis before the player's first plan (that is the planning
        // phase), nor out of a commit: only the world moving raises one.
        let v = View { scn: &self.scn, agents: &self.agents, fire: &self.fire, crews: &self.crews, districts: &self.districts };
        if !look_for_crisis {
            self.detector.acknowledge(&v, &self.posts);
        }
        if !(look_for_crisis && self.planned) || now > self.case.duration_s() - crate::crisis::LAST_CALL_S {
            return p;
        }
        let forecast = self.territory.shifted(&self.case).map(|(at, w)| (at, w.wind_dir_deg));
        let unwarned: Vec<bool> = self.active.civil.iter().map(|c| *c == Civil::Nessuno).collect();
        if let Some(c) = self.detector.check(&v, &self.posts, now, forecast, &unwarned) {
            // with the mayor's rule, her authorisations, the wind and the
            // fire near homes nobody warned slow the game; the rest is news
            if self.mayor.is_some() && (!c.kind.stops_with_mayor() || self.crisis.is_some()) {
                self.log.push(LogEntry { at_s: now, text: c.text });
            } else if self.mayor.is_some() {
                self.log.push(LogEntry { at_s: now, text: c.text.clone() });
                self.crisis = Some(c);
            } else {
                self.log.push(LogEntry { at_s: now, text: format!("CRISI: {}", c.text) });
                self.crisis = Some(c);
            }
        }
        p
    }

    /// The crisis raised since the last call, if any. The kiosk slows to ×1
    /// on it; a headless strategy decides on it.
    pub fn take_crisis(&mut self) -> Option<Crisis> {
        self.crisis.take()
    }

    /// Homes protected by units on their posts, 0-1 each.
    fn protection(&self) -> Vec<f32> {
        let mut out = vec![0.0f32; self.agents.households.len()];
        for post in self.posts.iter().flatten() {
            let u = &self.crews.units[post.unit];
            if !matches!(u.state, UnitState::Working | UnitState::Staged | UnitState::Moving) || dist(u.pos, post.at) > ON_POST_M {
                continue;
            }
            let p = protection_of(u.kind, u.water_l);
            for (i, h) in self.agents.households.iter().enumerate() {
                if dist(h.home, post.at) <= DEFEND_REACH_M {
                    out[i] = out[i].max(p);
                }
            }
        }
        out
    }

    pub fn step(&mut self) -> Result<()> {
        if let Some((at, w)) = self.territory.shifted(&self.case) {
            if self.time_s() >= at && self.fire.weather().wind_dir_deg != w.wind_dir_deg {
                self.fire.set_weather(w)?;
                self.log.push(LogEntry { at_s: self.time_s(), text: format!("il vento gira: ora soffia da {}", crate::words::compass(w.wind_dir_deg)) });
            }
        }
        self.fire.advance(STEP_S)?;
        self.agents.step(STEP_S as f32, &self.fire, &self.scn);
        self.knock();
        if self.time_s() >= self.next_review_s {
            self.review();
            self.authorise();
        }
        for a in self.crews.step(STEP_S as f32, &self.agents.network, &self.agents.traffic, &self.fire, &self.scn) {
            self.fire.queue(a);
        }
        let prot = self.protection();
        self.fire.set_structure_protection(&prot);
        self.note();
        let now = self.time_s();
        for (k, u) in self.crews.units.iter().enumerate() {
            let was = std::mem::replace(&mut self.logged[k], u.state);
            if was == u.state {
                continue;
            }
            let near = self.districts.iter().min_by(|a, b| dist(a.centre, u.pos).total_cmp(&dist(b.centre, u.pos))).map_or("", |d| d.name.as_str());
            match u.state {
                UnitState::Withdrawing => self.log.push(LogEntry { at_s: now, text: format!("{} si ritira vicino a {near}: il fuoco è troppo vicino", u.callsign) }),
                UnitState::Lost => self.log.push(LogEntry { at_s: now, text: format!("Il fuoco ha colpito {} vicino a {near} mentre {}: fuori servizio", u.callsign, if was == UnitState::Withdrawing { "si ritirava" } else { "lavorava" }) }),
                _ => {}
            }
        }
        for (u, m) in self.crews.units.iter().zip(&mut self.moved) {
            if dist(u.pos, m.0) > 1.0 {
                *m = (u.pos, now);
            }
        }
        Ok(())
    }

    /// What unit `k` is doing, in words for the map and the log.
    pub fn unit_status(&self, k: usize) -> String {
        let u = &self.crews.units[k];
        let post = self.posts.get(k).and_then(|p| p.as_ref());
        let place = |p: &Post| self.districts[p.district].name.clone();
        match u.state {
            UnitState::Lost => "fuori servizio".into(),
            UnitState::Withdrawing => "si ritira: troppo pericoloso".into(),
            UnitState::Refilling => "va a rifornirsi d'acqua".into(),
            UnitState::Working => {
                let what = match post {
                    Some(p) => format!("difende {}", place(p)),
                    None => "finisce il lavoro in corso, poi rientra".into(),
                };
                // how close the fire is, so a withdrawal does not come out of nowhere
                let near = self.fire.active_cells().iter().map(|c| dist(self.scn.world.centre_of(*c), u.pos)).fold(f32::INFINITY, f32::min);
                let fire = if near < 250.0 { format!(", fuoco a {:.0} m", (near / 10.0).round() * 10.0) } else { String::new() };
                let water = if u.kind == UnitKind::Engine { format!(", acqua {:.0}%", u.water_frac() * 100.0) } else { String::new() };
                format!("{what}{fire}{water}")
            }
            UnitState::Moving if self.time_s() - self.moved[k].1 >= STUCK_S => "bloccato: strada tagliata dal fuoco".into(),
            UnitState::Moving => match post {
                Some(p) => {
                    let min = (self.crews.route_remaining_m(k, &self.agents.network) / abm::suppression::ENGINE_SPEED / 60.0).ceil().max(1.0);
                    format!("verso {}, {min:.0} min", place(p))
                }
                None => "rientra alla base".into(),
            },
            UnitState::Staged => match post {
                Some(p) if dist(u.pos, p.at) <= ON_POST_M => format!("in postazione a {}", place(p)),
                _ => "in attesa".into(),
            },
            UnitState::Inbound | UnitState::Unavailable => "non disponibile".into(),
        }
    }

    /// The units posted to district `d` by the plan in force, and where each
    /// stands now: the card's defence line while the plan runs, from the
    /// units' real state rather than from the preview's drive times.
    pub fn arrivals(&self, d: usize) -> Vec<(usize, Arrival)> {
        let mut out = vec![];
        for (k, post) in self.posts.iter().enumerate() {
            let Some(p) = post.as_ref().filter(|p| p.district == d) else { continue };
            let u = &self.crews.units[k];
            let a = match u.state {
                UnitState::Working => Arrival::OnPost,
                UnitState::Staged if dist(u.pos, p.at) <= ON_POST_M => Arrival::OnPost,
                UnitState::Moving if self.time_s() - self.moved[k].1 >= STUCK_S => Arrival::Blocked,
                UnitState::Moving => Arrival::InMin((self.crews.route_remaining_m(k, &self.agents.network) / abm::suppression::ENGINE_SPEED / 60.0).ceil().max(1.0) as u32),
                _ => Arrival::Away(self.unit_status(k)),
            };
            out.push((k, a));
        }
        out
    }

    /// Nothing is changing that the player would want to watch closely: no
    /// event for `quiet_s` simulated seconds, no unit on the move and no
    /// family packing or on the road (playtest 4: «niente di nuovo» showed
    /// while a whole village was leaving). The kiosk runs faster then; it is
    /// pacing, not a rule of the game.
    pub fn is_quiet(&self, quiet_s: i64) -> bool {
        let now = self.time_s();
        let calm_log = self.log.last().is_none_or(|e| now - e.at_s >= quiet_s);
        let parked = self.crews.units.iter().all(|u| !matches!(u.state, UnitState::Moving | UnitState::Withdrawing));
        let settled = self.agents.households.iter().all(|h| !matches!(h.status, Status::Preparing | Status::Evacuating));
        // nor while the fire closes on families still at home (playtest:
        // «tutto fermo, ×120» with the front 400 m from Castelvento)
        // (the costliest test, so last)
        let safe = || self.districts.iter().all(|d| {
            let home = d.households.iter().any(|&i| matches!(self.agents.households[i].status, Status::Normal | Status::Warned | Status::Defending));
            !home || district::exposure(d, &self.agents, &self.fire, &self.scn).is_none_or(|e| e.distance_m > QUIET_FIRE_M)
        });
        calm_log && parked && settled && safe()
    }

    /// The same fire with no orders at all, to the end of the case: what the
    /// debrief compares the player's game with.
    pub fn without_orders(data_dir: &Path, case: &str, seed: u64) -> Result<Outcome> {
        let mut g = Game::new(data_dir, case, seed)?;
        let end = g.case.duration_s();
        g.run_until(end)?;
        Ok(g.outcome())
    }

    pub fn run_until(&mut self, t_s: i64) -> Result<()> {
        while self.time_s() < t_s {
            self.step()?;
        }
        Ok(())
    }

    fn at_home(s: Status) -> bool {
        matches!(s, Status::Normal | Status::Warned | Status::Preparing | Status::Defending | Status::Trapped)
    }

    /// Caught at home: still there, and the fire is on them (the house is
    /// alight or the threat at the door is alarming), or trapped. Also the
    /// debrief's bookkeeping: when each household left, and how long each
    /// district had a unit on its post.
    fn note(&mut self) {
        let now = self.time_s();
        for (i, h) in self.agents.households.iter().enumerate() {
            if self.left_at[i].is_none() && matches!(h.status, Status::Evacuating | Status::Evacuated) {
                self.left_at[i] = Some(now);
            }
        }
        let mut held = vec![false; self.districts.len()];
        for post in self.posts.iter().flatten() {
            let u = &self.crews.units[post.unit];
            if matches!(u.state, UnitState::Working | UnitState::Staged) && dist(u.pos, post.at) <= ON_POST_M {
                held[post.district] = true;
            }
        }
        for (d, h) in held.into_iter().enumerate() {
            self.defended_s[d] += STEP_S * h as i64;
        }
        // whether the fire reached each district's doors, at home or not:
        // the same level that counts a family caught
        for (d, district) in self.districts.iter().enumerate() {
            if self.threatened_at[d].is_none()
                && district.households.iter().any(|&i| self.fire.threat().at(self.agents.households[i].home) >= fire::threat::ALARMING)
            {
                self.threatened_at[d] = Some(now);
            }
        }
        if now % NEAREST_EVERY_S == 0 {
            for (d, district) in self.districts.iter().enumerate() {
                if let Some(e) = district::exposure(district, &self.agents, &self.fire, &self.scn) {
                    self.nearest_m[d] = self.nearest_m[d].min(e.distance_m);
                }
            }
        }
        let ex = self.fire.exposure().fields();
        for (i, h) in self.agents.households.iter().enumerate() {
            if self.caught_at[i].is_some() || !Self::at_home(h.status) {
                continue;
            }
            if ex[i].alight || h.status == Status::Trapped || self.fire.threat().at(h.home) >= fire::threat::ALARMING {
                self.caught_at[i] = Some(now);
            }
        }
    }

    pub fn outcome(&self) -> Outcome {
        let ex = self.fire.exposure().fields();
        let districts = self
            .districts
            .iter()
            .map(|d| {
                let mut o = DistrictOutcome { households: d.households.len(), ..Default::default() };
                for &i in &d.households {
                    let h = &self.agents.households[i];
                    o.homes_hit += ex[i].alight as usize;
                    o.caught += self.caught_at[i].is_some() as usize;
                    match h.status {
                        Status::Evacuated => o.evacuated += 1,
                        Status::Evacuating => o.on_the_road += 1,
                        Status::Casualty => o.casualties += 1,
                        _ => o.at_home += 1,
                    }
                }
                o
            })
            .collect();
        let burnt = self.fire.state().iter().filter(|c| **c != fire::CellFire::Unburnt).count();
        Outcome {
            at_s: self.time_s(),
            districts,
            hectares: burnt as f32 * self.scn.world.cellsize * self.scn.world.cellsize / 10_000.0,
            units_lost: self.crews.units.iter().filter(|u| u.state == UnitState::Lost).count(),
        }
    }

    pub fn caught_at(&self, household: usize) -> Option<i64> {
        self.caught_at[household]
    }

    /// Two or three lines on why district `d` ended as it did, for the
    /// debrief: the civil order and how fast families left, the defence, and
    /// when the fire reached families still at home. Only facts the game
    /// recorded; no rule and no advice.
    pub fn story(&self, d: usize) -> Vec<String> {
        let hh = &self.districts[d].households;
        let n = hh.len();
        let clock = crate::words::clock;
        let fam = |k: usize| if k == 1 { "1 famiglia".to_string() } else { format!("{k} famiglie") };
        let mut out = vec![];
        // the order, and how long families took to leave after it
        let (pre, evac) = self.ordered_at[d];
        let left: Vec<i64> = hh.iter().filter_map(|&i| self.left_at[i]).collect();
        let order = match (pre, evac) {
            (Some(p), Some(e)) => Some((format!("Preallerta alle {}, evacuazione alle {}", clock(p), clock(e)), p)),
            (None, Some(e)) => Some((format!("Evacuazione alle {}", clock(e)), e)),
            (Some(p), None) => Some((format!("Preallerta alle {}, nessun ordine di evacuazione", clock(p)), p)),
            (None, None) => None,
        };
        out.push(match order {
            Some((what, from)) => {
                let mut after: Vec<i64> = left.iter().filter(|&&t| t >= from).map(|&t| t - from).collect();
                after.sort_unstable();
                match after.len() {
                    0 => format!("{what}: dopo l'ordine nessuna famiglia è partita."),
                    1 => format!("{what}: dopo l'ordine è partita 1 famiglia, in {} min.", (after[0] + 59) / 60),
                    k => format!("{what}: dopo l'ordine sono partite {k} famiglie, metà entro {} min.", (after[(k - 1) / 2] + 59) / 60),
                }
            }
            None if left.is_empty() => "Nessun ordine alla popolazione: nessuna famiglia è partita.".into(),
            None if left.len() == 1 => format!("Nessun ordine alla popolazione: 1 famiglia su {n} è partita da sola."),
            None => format!("Nessun ordine alla popolazione: {} famiglie su {n} sono partite da sole.", left.len()),
        });
        // where every family ended, so the counts add up to the district
        let (mut safe, mut road, mut home, mut dead) = (0, 0, 0, 0);
        for &i in hh {
            match self.agents.households[i].status {
                Status::Evacuated => safe += 1,
                Status::Evacuating => road += 1,
                Status::Casualty => dead += 1,
                _ => home += 1,
            }
        }
        let mut parts = vec![format!("{safe} in salvo")];
        if road > 0 {
            parts.push(format!("{road} ancora in viaggio"));
        }
        if home > 0 {
            parts.push(if home == 1 { "1 non è partita".into() } else { format!("{home} non sono partite") });
        }
        if dead > 0 {
            parts.push(if dead == 1 { "1 vittima".into() } else { format!("{dead} vittime") });
        }
        out.push(format!("Alla fine, su {n} famiglie: {}.", parts.join(", ")));
        // the defence
        let rank = self.active.rank(d);
        out.push(match (self.defended_s[d], rank) {
            (0, None) => "Nessun mezzo: non era tra le priorità.".into(),
            (0, Some(r)) => format!("Priorità {}, ma nessun mezzo è mai arrivato in postazione.", r + 1),
            (s, _) if s >= 3600 => format!("Mezzi in postazione per {} h {:02} min.", s / 3600, (s / 60) % 60),
            (s, _) => format!("Mezzi in postazione per {} min.", (s + 59) / 60),
        });
        // the fire at the door
        let caught: Vec<i64> = hh.iter().filter_map(|&i| self.caught_at[i]).collect();
        if let Some(&first) = caught.iter().min() {
            out.push(if caught.len() == 1 {
                format!("Il fuoco ha raggiunto 1 famiglia ancora in casa, alle {}.", clock(first))
            } else {
                format!("Il fuoco ha raggiunto {} ancora in casa, la prima alle {}.", fam(caught.len()), clock(first))
            });
        }
        out
    }

    /// «Come sono andati i tuoi ordini»: the civil order given to district
    /// `d`, judged by how close the fire came in this game. An order cannot
    /// change where the fire burns, so this is the player's own fire. An
    /// evacuation where the fire never reached the doors is the cost Misura
    /// counts (decision 2026-10-13): said as a fact, not as a mistake, when
    /// the fire came within [`QUIET_FIRE_M`]; farther, it suggests the
    /// pre-alert would have done.
    pub fn order_review(&self, d: usize) -> Review {
        let hh = &self.districts[d].households;
        let name = &self.districts[d].name;
        let clock = crate::words::clock;
        let fam = |k: usize| if k == 1 { "1 famiglia".to_string() } else { format!("{k} famiglie") };
        let caught = hh.iter().filter(|&&i| self.caught_at[i].is_some()).count();
        let nearest = match self.nearest_m[d] {
            m if m.is_finite() => format!("a {} dalle case", crate::words::km(m)),
            _ => "lontano".to_string(),
        };
        let (pre, evac) = self.ordered_at[d];
        let (verdict, text) = match (evac, pre, self.reach(d)) {
            (Some(e), _, Reach::Far) => {
                let left = hh.iter().filter(|&&i| self.left_at[i].is_some_and(|t| t >= e)).count();
                (Verdict::Forse, format!("{name}: hai fatto evacuare {}, ma il fuoco è rimasto {nearest}. Forse bastava la preallerta.", fam(left)))
            }
            (Some(e), _, Reach::Near) => {
                let left = hh.iter().filter(|&&i| self.left_at[i].is_some_and(|t| t >= e)).count();
                (Verdict::Forse, format!("{name}: evacuazione alle {}, {} in strada. Il fuoco è arrivato {nearest}, non alle case.", clock(e), fam(left)))
            }
            (Some(e), _, Reach::Doors(t)) if caught == 0 => (
                Verdict::Bene,
                if e <= t {
                    format!("{name}: evacuazione alle {}, {} min prima che il fuoco arrivasse alle case. Nessuna famiglia colta in casa.", clock(e), (t - e) / 60)
                } else {
                    format!("{name}: evacuazione alle {}, con il fuoco già alle case. Nessuna famiglia colta in casa.", clock(e))
                },
            ),
            (Some(e), _, Reach::Doors(t)) => (
                Verdict::Male,
                if e <= t {
                    format!("{name}: evacuazione alle {}, il fuoco alle case alle {}: troppo poco tempo per {}.", clock(e), clock(t), fam(caught))
                } else {
                    format!("{name}: evacuazione alle {}, ma il fuoco era alle case dalle {}. Colte in casa: {}.", clock(e), clock(t), fam(caught))
                },
            ),
            (None, Some(_), Reach::Far | Reach::Near) => (Verdict::Bene, format!("{name}: solo preallerta, ed è bastata: il fuoco è rimasto {nearest}.")),
            (None, Some(_), Reach::Doors(_)) if caught == 0 => {
                (Verdict::Bene, format!("{name}: solo preallerta, e il fuoco è arrivato alle case. Nessuna famiglia colta in casa."))
            }
            (None, Some(p), Reach::Doors(t)) => (
                Verdict::Male,
                format!("{name}: preallerta alle {} ma nessuna evacuazione. Il fuoco è arrivato alle case alle {}: colte in casa {}.", clock(p), clock(t), fam(caught)),
            ),
            (None, None, Reach::Far) => (Verdict::Bene, format!("{name}: nessun ordine, e non serviva: il fuoco è rimasto {nearest}.")),
            (None, None, Reach::Near) => {
                (Verdict::Forse, format!("{name}: nessun ordine. Il fuoco è arrivato {nearest}: una preallerta sarebbe stata prudente."))
            }
            (None, None, Reach::Doors(_)) if caught == 0 => {
                (Verdict::Forse, format!("{name}: nessun ordine. Il fuoco è arrivato alle case, ma le famiglie erano già partite da sole."))
            }
            (None, None, Reach::Doors(t)) => {
                (Verdict::Male, format!("{name}: nessun ordine. Il fuoco è arrivato alle case alle {}: colte in casa {}.", clock(t), fam(caught)))
            }
        };
        Review { verdict, text }
    }

    /// How close the fire came to district `d` in this game.
    fn reach(&self, d: usize) -> Reach {
        match self.threatened_at[d] {
            Some(t) => Reach::Doors(t),
            None if self.nearest_m[d] > QUIET_FIRE_M => Reach::Far,
            None => Reach::Near,
        }
    }

    /// Families that left on an evacuation order from a district the fire
    /// never reached at the doors: the sum of the review's «Forse» lines for
    /// an evacuation, and what Misura counts.
    pub fn needless_evacuations(&self) -> usize {
        (0..self.districts.len())
            .filter_map(|d| match (self.ordered_at[d].1, self.reach(d)) {
                (Some(e), Reach::Far | Reach::Near) => {
                    Some(self.districts[d].households.iter().filter(|&&i| self.left_at[i].is_some_and(|t| t >= e)).count())
                }
                _ => None,
            })
            .sum()
    }
}
