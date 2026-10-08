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
use crate::district::{self, dist, District};
use crate::plan::{Civil, Plan};

/// Simulated seconds per step (the units' and civilians' decision interval).
pub const STEP_S: i64 = 6;

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
    next_review_s: i64,
    caught_at: Vec<Option<i64>>,
}

impl Game {
    pub fn new(data_dir: &Path, scenario: &str, case: &str, seed: u64) -> Result<Game> {
        let territory = Territory::load(data_dir, scenario)?;
        let case = territory.case(case).with_context(|| format!("caso {case:?} assente in game.json"))?.clone();
        let scn = Scenario::load_by_id(data_dir, scenario)?;
        let mut fire = FireSim::new(&scn, territory.weather(&case), seed)?;
        fire.ignite_patch(scn.world.cell_of(case.ignition()), case.radius_m, &scn)?;
        let agents = Abm::new(&scn, seed)?;
        let roster: Vec<(UnitKind, Pos)> =
            territory.roster.iter().map(|s| (s.kind.unit_kind(), Pos::from(territory.stations[s.station].pos))).collect();
        let lib = behavior::defaults::default_library();
        let policy = abm::behaviour::UnitRuntime::build(&lib)
            .map_err(|e| anyhow::anyhow!(e))?
            .context("nessun profilo attivo per i mezzi")?;
        let crews = Suppression::with_roster(&scn, &roster, policy)?;
        let districts = district::of(&scn, &agents);
        let n = agents.households.len();
        let units = crews.units.len();
        Ok(Game {
            active: Plan::new(districts.len()),
            posts: vec![None; units],
            uncovered: vec![],
            log: vec![],
            next_review_s: 0,
            caught_at: vec![None; n],
            scn,
            fire,
            agents,
            crews,
            territory,
            case,
            seed,
            districts,
        })
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
            let what = if new == Civil::Evacua { "evacuazione" } else { "preallerta" };
            self.log.push(LogEntry { at_s: now, text: format!("{what}: {} ({n} famiglie)", self.districts[d].name) });
        }
        if plan.priorities != self.active.priorities {
            let names: Vec<&str> = plan.priorities.iter().map(|&d| self.districts[d].name.as_str()).collect();
            self.log.push(LogEntry { at_s: now, text: format!("priorità: {}", names.join(" > ")) });
        }
        self.active = plan;
        Ok(self.review())
    }

    /// Re-plan the active plan and hand the units their tasks.
    fn review(&mut self) -> Proposal {
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
                        self.log.push(LogEntry { at_s: now, text: format!("{}: rientra, nessun quartiere in priorità da coprire", callsign) });
                    }
                }
            }
        }
        for (d, why) in &p.uncovered {
            if !self.uncovered.iter().any(|(x, w)| x == d && w == why) {
                self.log.push(LogEntry { at_s: now, text: format!("scoperto: {why}") });
            }
        }
        self.posts = p.posts.clone();
        self.uncovered = p.uncovered.clone();
        self.next_review_s = now + REVIEW_S;
        p
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
        self.fire.advance(STEP_S)?;
        self.agents.step(STEP_S as f32, &self.fire, &self.scn);
        // The opening fire is in the state only after the first advance, so
        // the plan committed at T+0 is reviewed again straight away.
        if self.time_s() >= self.next_review_s || self.time_s() == STEP_S {
            self.review();
        }
        for a in self.crews.step(STEP_S as f32, &self.agents.network, &self.agents.traffic, &self.fire, &self.scn) {
            self.fire.queue(a);
        }
        let prot = self.protection();
        self.fire.set_structure_protection(&prot);
        self.note();
        Ok(())
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
    /// alight or the threat at the door is alarming), or trapped.
    fn note(&mut self) {
        let now = self.time_s();
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
}
