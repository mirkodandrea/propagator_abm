//! A headless run of one mission: the thing the COMPARE screen replays with no
//! orders, and the thing the beat tests measure. No Bevy.
//!
//! Steps with the same fire-then-agents order `game::Sim::advance` uses, at the
//! 6 s decision interval, so a figure taken here is the figure the kiosk shows.

use abm::suppression::{Suppression, Task, UnitKind};
use abm::Abm;
use anyhow::Result;
use fire::FireSim;
use scenario::{Pos, Scenario};

use crate::mission::Spec;

/// Simulated seconds per step: `DECISION_S` rounded up to the fire's quantum.
pub const STEP_S: i64 = 6;

/// How close burnt ground has to be for a house to count as reached by the fire.
/// Measured (tests/lost.rs, seed 42, no orders): 30-60 m reaches nobody, 150 m
/// gives 54 / 15 / 47 houses on borgo / valle / porto -- graded, not saturated.
pub const LOST_RADIUS_M: f32 = 150.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Order {
    /// Every household, whatever the distance.
    EvacuateAll,
    /// Send the best free unit of this (ground) kind at the fire edge nearest `at`.
    Attack { kind: UnitKind, at: Pos },
    /// Request the aircraft if not yet asked, and send a load at `at`.
    Drop { at: Pos },
}

/// The four facts the outcome card shows (spec §6.5). Facts, not a score.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Outcome {
    pub households: usize,
    pub safe: usize,
    pub moving: usize,
    /// Not safe, and the fire is on them: threatened, trapped or caught.
    pub in_danger: usize,
    /// Households the fire reached while they were still at home, ever. Latched:
    /// the number that separates a early warning from a late one, because by
    /// the end of a long incident nearly everyone has got out one way or another.
    pub caught: usize,
    pub homes_lost: usize,
    pub hectares: f32,
}

impl Outcome {
    /// Families not in danger: the ones who reached a refuge *and* the ones the
    /// fire never came near. Counting evacuees alone made doing nothing in a
    /// harmless session read as a disaster (playtest §16 #5).
    pub fn secure(&self) -> usize {
        self.households.saturating_sub(self.in_danger)
    }
}

/// The outcome bookkeeping, separate from who steps the model so the headless
/// [`Run`] and the kiosk's live `Sim` count identically -- the COMPARE screen is
/// only honest if both sides of it are counted by the same code.
#[derive(Clone)]
pub struct Tally {
    caught: Vec<bool>,
    /// When the fire first reached somebody who was still at home.
    pub first_caught_s: Option<i64>,
}

impl Tally {
    pub fn new(households: usize) -> Tally {
        Tally { caught: vec![false; households], first_caught_s: None }
    }

    /// Whether the fire is on this household and they are not yet away.
    fn endangered(i: usize, agents: &Abm, fire: &FireSim) -> bool {
        use scenario::population::Status;
        let h = &agents.households[i];
        // Evacuating is *on the road*, not at home: leaving is exactly what an
        // early warning buys, and counting it as endangered erased the beat.
        if matches!(h.status, Status::Evacuated | Status::Evacuating) {
            return false;
        }
        let f = fire.exposure().fields()[i];
        f.alight
            || matches!(h.status, Status::Trapped | Status::Casualty)
            || fire.threat().at(h.home) >= fire::threat::ALARMING
    }

    /// Call once after each step.
    pub fn note(&mut self, agents: &Abm, fire: &FireSim) {
        for i in 0..self.caught.len() {
            if !self.caught[i] && Self::endangered(i, agents, fire) {
                self.caught[i] = true;
                self.first_caught_s.get_or_insert(fire.time_s());
            }
        }
    }

    /// Households the fire has caught at home, by position -- lets a test ask
    /// about one hamlet rather than the whole town.
    pub fn caught_where(&self, agents: &Abm, f: impl Fn(scenario::Pos) -> bool) -> usize {
        agents.households.iter().zip(&self.caught).filter(|(h, &c)| c && f(h.home)).count()
    }

    /// Households with burnt ground at their door: within [`LOST_RADIUS_M`] of a
    /// cell the fire has been in. Not `ExposureField::alight`, which saturates --
    /// firebrands out to 2.5 km set every house in a 4 km town alight whatever the
    /// player does (measured: 250/250 on borgo), so the card could not respond.
    fn lost(agents: &Abm, fire: &FireSim, world: &scenario::World) -> usize {
        use fire::CellFire;
        let state = fire.state();
        agents
            .households
            .iter()
            .filter(|h| {
                fire::cells_in_radius(world, h.home, LOST_RADIUS_M)
                    .iter()
                    .any(|c| state[c.row * world.fire_cols + c.col] != CellFire::Unburnt)
            })
            .count()
    }

    pub fn outcome(&self, agents: &Abm, fire: &FireSim, world: &scenario::World) -> Outcome {
        use fire::CellFire;
        let s = agents.stats();
        let in_danger = (0..agents.households.len()).filter(|&i| Self::endangered(i, agents, fire)).count();
        let burnt = fire.state().iter().filter(|c| **c != CellFire::Unburnt).count();
        Outcome {
            households: agents.households.len(),
            safe: s.safe,
            moving: s.moving,
            in_danger,
            caught: self.caught.iter().filter(|&&c| c).count(),
            homes_lost: Self::lost(agents, fire, world),
            hectares: burnt as f32 * 0.04,
        }
    }
}

pub struct Run {
    pub scn: Scenario,
    pub fire: FireSim,
    pub agents: Abm,
    pub crews: Suppression,
    pub spec: Spec,
    pub tally: Tally,
    shift_pending: bool,
}

impl Run {
    pub fn new(data_dir: &std::path::Path, spec: Spec, seed: u64) -> Result<Run> {
        let scn = Scenario::load_by_id(data_dir, spec.id)?;
        let mut fire = FireSim::new(&scn, spec.weather, seed)?;
        let centre = scn.world.cell_of(spec.ignition);
        fire.ignite_patch(centre, spec.radius_m, &scn)?;
        let agents = Abm::new(&scn, seed)?;
        let tally = Tally::new(agents.households.len());
        let ig = scn.world.centre_of(centre);
        let mut staging: Vec<Pos> = agents.refuges.iter().map(|r| r.pos).collect();
        let d = |p: &Pos| (p.x - ig.x).powi(2) + (p.y - ig.y).powi(2);
        staging.sort_by(|a, b| d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal));
        let crews = Suppression::new(&scn, &staging)?;
        Ok(Run { scn, fire, agents, crews, spec, tally, shift_pending: spec.shift.is_some() })
    }

    pub fn time_s(&self) -> i64 {
        self.fire.time_s()
    }

    pub fn first_caught_s(&self) -> Option<i64> {
        self.tally.first_caught_s
    }

    pub fn order(&mut self, o: Order) {
        match o {
            Order::EvacuateAll => {
                self.agents.order_evacuation_all();
            }
            Order::Attack { kind, at } => {
                if let Some(id) = self.best_unit(kind) {
                    let _ = self.crews.assign(id, Task::Attack { at });
                }
            }
            Order::Drop { at } => {
                self.crews.request_air();
                if let Some(id) = self.best_unit(UnitKind::AirTanker) {
                    let _ = self.crews.assign(id, Task::Drop { at });
                }
            }
        }
    }

    /// The unit the kiosk's button would send: a free one of this kind, preferring
    /// whichever is closest to being ready.
    fn best_unit(&self, kind: UnitKind) -> Option<usize> {
        use abm::suppression::UnitState as S;
        self.crews.units.iter().filter(|u| u.kind == kind && u.assignable()).min_by_key(|u| match u.state {
            S::Staged => 0,
            S::Inbound => 1,
            S::Moving | S::Working | S::Refilling => 2,
            _ => 3,
        }).map(|u| u.id)
    }

    /// One step: scripted weather, then fire, then agents.
    pub fn step(&mut self) -> Result<()> {
        if let (true, Some(s)) = (self.shift_pending, self.spec.shift) {
            if self.fire.time_s() >= s.at_s {
                self.fire.set_weather(s.weather)?;
                self.shift_pending = false;
            }
        }
        self.fire.advance(STEP_S)?;
        self.agents.step(STEP_S as f32, &self.fire, &self.scn);
        for a in self.crews.step(STEP_S as f32, &self.agents.network, &self.agents.traffic, &self.fire, &self.scn) {
            self.fire.queue(a);
        }
        self.tally.note(&self.agents, &self.fire);
        Ok(())
    }

    pub fn caught_where(&self, f: impl Fn(scenario::Pos) -> bool) -> usize {
        self.tally.caught_where(&self.agents, f)
    }

    pub fn outcome(&self) -> Outcome {
        self.tally.outcome(&self.agents, &self.fire, &self.scn.world)
    }

    /// Run to the end of the mission, giving each order at its time.
    pub fn play(&mut self, orders: &[(i64, Order)]) -> Result<Outcome> {
        let mut next = 0;
        while self.time_s() < self.spec.duration_s {
            while next < orders.len() && orders[next].0 <= self.time_s() {
                self.order(orders[next].1);
                next += 1;
            }
            self.step()?;
        }
        Ok(self.outcome())
    }
}
