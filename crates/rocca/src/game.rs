//! One game: the single simulation and gameplay authority. The headless
//! runner and (from phase 5) the kiosk step this and nothing else.
//!
//! Order of a step, every [`STEP_S`] simulated seconds: fire, civilians,
//! coordinator review (every [`REVIEW_S`]), units, structure protection,
//! bookkeeping. The active plan changes only through [`Game::commit`]; the
//! proposed plan is [`Game::preview`], which reads the state and changes
//! nothing.

use std::path::Path;

use abm::suppression::{Task, UnitKind, UnitState};
use abm::{Abm, Suppression};
use anyhow::{Context, Result};
use fire::FireSim;
use scenario::population::Status;
use scenario::{Pos, Scenario};

use crate::case::{Case, Territory};
use crate::coordinator::{self, Post, Proposal, View, DEFEND_REACH_M, ON_POST_M, REVIEW_S};
use crate::crisis::{Crisis, Detector};
use crate::district::{self, dist, District};
use crate::plan::{Civil, Plan};

/// Simulated seconds per step (the units' and civilians' decision interval).
pub const STEP_S: i64 = 6;
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
    /// Per unit: where it last moved and when, to tell a unit stuck on a
    /// closed road from one on its way.
    moved: Vec<(Pos, i64)>,
    /// Per unit: the state last logged, to log a withdrawal or a loss once.
    logged: Vec<UnitState>,
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
        let agents = Abm::with_behaviours(&scn, seed, households, persons)?;
        let roster: Vec<(UnitKind, Pos)> =
            territory.roster.iter().map(|s| (s.kind.unit_kind(), Pos::from(territory.stations[s.station].pos))).collect();
        let crews = Suppression::with_roster(&scn, &roster, policy)?;
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
            moved: crews.units.iter().map(|u| (u.pos, 0)).collect(),
            logged: crews.units.iter().map(|u| u.state).collect(),
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
        let now = self.time_s();
        for (d, (&new, &old)) in plan.civil.iter().zip(&self.active.civil).enumerate() {
            if new == old {
                continue;
            }
            let ids = self.districts[d].households.clone();
            let n = match new {
                Civil::Preallerta => self.agents.prealert_of(&ids),
                Civil::Evacua => self.agents.order_evacuation_of(&ids),
                Civil::Nessuno => 0,
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
                self.log.push(LogEntry { at_s: now, text: format!("senza mezzi: {why}") });
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
        if let Some(c) = self.detector.check(&v, &self.active, &self.posts, now, forecast) {
            self.log.push(LogEntry { at_s: now, text: format!("CRISI: {}", c.text) });
            self.crisis = Some(c);
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
        if self.time_s() >= self.next_review_s {
            self.review();
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
                UnitState::Lost => self.log.push(LogEntry { at_s: now, text: format!("{} raggiunto dal fuoco vicino a {near} mentre {}: fuori servizio", u.callsign, if was == UnitState::Withdrawing { "si ritirava" } else { "lavorava" }) }),
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
    /// event for `quiet_s` simulated seconds and no unit on the move. The
    /// kiosk runs faster then; it is pacing, not a rule of the game.
    pub fn is_quiet(&self, quiet_s: i64) -> bool {
        let now = self.time_s();
        let calm_log = self.log.last().is_none_or(|e| now - e.at_s >= quiet_s);
        let parked = self.crews.units.iter().all(|u| !matches!(u.state, UnitState::Moving | UnitState::Withdrawing));
        calm_log && parked
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
        let clock = |s: i64| format!("T+{}:{:02}", s / 3600, (s / 60) % 60);
        let fam = |k: usize| if k == 1 { "1 famiglia".to_string() } else { format!("{k} famiglie") };
        let mut out = vec![];
        // the order, and how long families took to leave after it
        let (pre, evac) = self.ordered_at[d];
        let left: Vec<i64> = hh.iter().filter_map(|&i| self.left_at[i]).collect();
        let order = match (pre, evac) {
            (Some(p), Some(e)) => Some((format!("Preallerta a {}, evacuazione a {}", clock(p), clock(e)), p)),
            (None, Some(e)) => Some((format!("Evacuazione a {}", clock(e)), e)),
            (Some(p), None) => Some((format!("Preallerta a {}, nessun ordine di evacuazione", clock(p)), p)),
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
            parts.push(if home == 1 { "1 rimasta a casa".into() } else { format!("{home} rimaste a casa") });
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
                format!("Il fuoco ha raggiunto 1 famiglia ancora in casa, a {}.", clock(first))
            } else {
                format!("Il fuoco ha raggiunto {} ancora in casa, la prima a {}.", fam(caught.len()), clock(first))
            });
        }
        out
    }
}
