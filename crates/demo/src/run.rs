//! A headless run of one mission: the thing the COMPARE screen replays with no
//! orders, and the thing the beat tests measure. No Bevy.
//!
//! Steps with the same fire-then-agents order `game::Sim::advance` uses, at the
//! 6 s decision interval, so a figure taken here is the figure the kiosk shows.

use abm::suppression::{Suppression, Task, UnitKind, DROP_WIDTH_M};
use abm::Abm;
use anyhow::Result;
use fire::FireSim;
use scenario::{Pos, Scenario};

use crate::cost;
use crate::event::{self, Event, EventKind};
use crate::mission::Spec;
use crate::trust;

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
    /// Households within `radius_m` of `centre` (spec 5.5): warns fewer, spares
    /// the rest a needless flight.
    EvacuateZone { centre: Pos, radius_m: f32 },
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
    /// Home defence (spec §4 option B). `None` until [`Tally::enable_defence`]:
    /// with it off, `homes_lost` is the end-state rule above, bit for bit.
    defence: Option<Defence>,
}

/// A home an engine is hosing down, or one a retardant drop has just wetted, is
/// lost only to flame contact ([`DEFENDED_RADIUS_M`]); an undefended one is lost
/// when burnt ground reaches [`LOST_RADIUS_M`]. Loss is latched at the moment of
/// contact, so *when* the unit is on station is the whole mechanic: an engine
/// that has withdrawn, run dry or never arrived protects nobody.
#[derive(Clone)]
struct Defence {
    /// Simulated second until which each household counts as defended.
    until_s: Vec<i64>,
    lost: Vec<bool>,
    /// Per unit: seconds spent on station since its last refill, and the tank
    /// level it had then (a rise means it refilled).
    on_station_s: Vec<i64>,
    last_water_l: Vec<f32>,
}

/// How long an engine's tank sprinkles homes before it must go and refill: the
/// reason sending an engine *too early* wastes it (2,500 L at ~100 L/min of sprinkler = 25 min).
pub const SPRINKLE_BUDGET_S: i64 = 25 * 60;

/// An engine within this distance of a home (working, with water) defends it.
pub const DEFEND_REACH_M: f32 = 80.0;
/// How long a retardant drop keeps the homes under it defended.
pub const DROP_DEFENCE_S: i64 = 15 * 60;
/// A defended home is lost only if burnt ground gets this close.
pub const DEFENDED_RADIUS_M: f32 = 25.0;

impl Tally {
    pub fn new(households: usize) -> Tally {
        Tally { caught: vec![false; households], first_caught_s: None, defence: None }
    }

    /// Turn on home defence. Inert until called; the kiosk does not call it.
    pub fn enable_defence(&mut self) {
        let n = self.caught.len();
        self.defence.get_or_insert_with(|| Defence {
            until_s: vec![0; n],
            lost: vec![false; n],
            on_station_s: vec![],
            last_water_l: vec![],
        });
    }

    pub fn defence_enabled(&self) -> bool {
        self.defence.is_some()
    }

    /// Households currently defended (for a map marker or a test).
    pub fn defended_now(&self, now_s: i64) -> usize {
        self.defence.as_ref().map_or(0, |d| d.until_s.iter().filter(|&&u| u > now_s).count())
    }

    /// Call once after each step, after [`Tally::note`], when defence is on.
    pub fn note_defence(
        &mut self,
        crews: &abm::suppression::Suppression,
        agents: &Abm,
        fire: &FireSim,
        world: &scenario::World,
        dropped_at: &[Pos],
    ) {
        use abm::suppression::{UnitKind, UnitState};
        use fire::CellFire;
        let Some(d) = self.defence.as_mut() else { return };
        let now = fire.time_s();
        let near = |a: Pos, b: Pos, r: f32| (a.x - b.x).powi(2) + (a.y - b.y).powi(2) <= r * r;
        if d.on_station_s.len() != crews.units.len() {
            d.on_station_s = vec![0; crews.units.len()];
            d.last_water_l = crews.units.iter().map(|u| u.water_l).collect();
        }
        // An engine sprinkles while it is working with water, for a tank's worth
        // of time; a refill (its level rising) starts the budget again.
        let mut sprinkling = vec![false; crews.units.len()];
        for (k, u) in crews.units.iter().enumerate() {
            if u.water_l > d.last_water_l[k] + 1.0 {
                d.on_station_s[k] = 0;
            }
            d.last_water_l[k] = u.water_l;
            if u.kind == UnitKind::Engine && u.state == UnitState::Working && u.water_l > 0.0 {
                d.on_station_s[k] += STEP_S;
                sprinkling[k] = d.on_station_s[k] <= SPRINKLE_BUDGET_S;
            }
        }
        for (i, h) in agents.households.iter().enumerate() {
            let on_scene = crews.units.iter().enumerate().any(|(k, u)| sprinkling[k] && near(u.pos, h.home, DEFEND_REACH_M));
            let dropped = dropped_at.iter().any(|&p| near(p, h.home, DROP_WIDTH_M));
            if on_scene {
                d.until_s[i] = d.until_s[i].max(now + STEP_S);
            }
            if dropped {
                d.until_s[i] = d.until_s[i].max(now + DROP_DEFENCE_S);
            }
        }
        let state = fire.state();
        for (i, h) in agents.households.iter().enumerate() {
            if d.lost[i] {
                continue;
            }
            let r = if d.until_s[i] > now { DEFENDED_RADIUS_M } else { LOST_RADIUS_M };
            if fire::cells_in_radius(world, h.home, r).iter().any(|c| state[c.row * world.fire_cols + c.col] != CellFire::Unburnt) {
                d.lost[i] = true;
            }
        }
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
            homes_lost: match &self.defence {
                Some(d) => d.lost.iter().filter(|&&l| l).count(),
                None => Self::lost(agents, fire, world),
            },
            hectares: burnt as f32 * 0.04,
        }
    }
}

/// Model options a sweep can switch, **inert by default** (finding 34): the
/// default `Variant` reproduces every published figure exactly, and the kiosk
/// never sets anything else. Passed to [`Run::with_variant`], never global.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Variant {
    /// Scales what a unit's work is worth (spec §4 option A). `ONE` = published.
    pub unit_effect: abm::suppression::UnitEffect,
    /// Home defence (spec §4 option B): engines on station and drops over homes
    /// protect them from all but flame contact. Off = published rule.
    pub defend_homes: bool,
    /// Replace the town's odds of a wind shift (spec 5.3). Applied by the sweep
    /// when it draws the session's weather (`weather::draw_with`); `Run` itself
    /// only sees the resulting spec. `None` = shipped climate.
    pub shift_p: Option<(f32, f32)>,
    /// Cry-wolf (spec 5.2): the most trust a fully needless order costs the
    /// households still to be ordered. `None` = off, nothing runs.
    pub cry_wolf: Option<f32>,
}

pub struct Run {
    pub scn: Scenario,
    pub fire: FireSim,
    pub agents: Abm,
    pub crews: Suppression,
    pub spec: Spec,
    pub tally: Tally,
    /// What the commander did, for [`Run::ledger`] (spec 5.1).
    pub log: cost::Log,
    wolf: Option<trust::CryWolf>,
    /// Everything notable that has happened, oldest first (spec 5.4/5.6).
    pub events: Vec<Event>,
    watch: Watch,
    shift_pending: bool,
}

/// What [`Run::step`] compared against last time, to turn state into events.
#[derive(Default)]
struct Watch {
    last_spot_s: f32,
    wind_from: Option<f64>,
    unit_states: Vec<abm::suppression::UnitState>,
    masts_down: usize,
    near_town: bool,
}

impl Run {
    pub fn new(data_dir: &std::path::Path, spec: Spec, seed: u64) -> Result<Run> {
        Run::with_variant(data_dir, spec, seed, Variant::default())
    }

    pub fn with_variant(data_dir: &std::path::Path, spec: Spec, seed: u64, variant: Variant) -> Result<Run> {
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
        let wolf = variant.cry_wolf.map(|d| trust::CryWolf::new(&agents, d));
        let mut crews = Suppression::new(&scn, &staging)?;
        crews.effect = variant.unit_effect;
        let mut tally = tally;
        if variant.defend_homes {
            tally.enable_defence();
        }
        Ok(Run { scn, fire, agents, crews, spec, tally, log: cost::Log::default(), wolf, events: vec![], watch: Watch::default(), shift_pending: spec.shift.is_some() })
    }

    pub fn time_s(&self) -> i64 {
        self.fire.time_s()
    }

    pub fn first_caught_s(&self) -> Option<i64> {
        self.tally.first_caught_s
    }

    pub fn order(&mut self, o: Order) {
        let before: Vec<bool> = self.agents.households.iter().map(|h| h.ordered).collect();
        self.order_inner(o);
        if matches!(o, Order::EvacuateAll | Order::EvacuateZone { .. }) {
            let moved: Vec<usize> = (0..before.len()).filter(|&i| !before[i] && self.agents.households[i].ordered).collect();
            if !moved.is_empty() {
                self.log.push(cost::Entry::Evacuation { at_s: self.fire.time_s(), households: moved.len() });
                if let Some(w) = self.wolf.as_mut() {
                    w.note_order(self.fire.time_s(), moved);
                }
            }
        }
    }

    /// Trust and anger for the HUD (spec 5.2). Constant while cry-wolf is off.
    pub fn trust(&self) -> trust::Trust {
        match &self.wolf {
            Some(w) => w.trust(&self.agents),
            None => trust::CryWolf::new(&self.agents, 0.0).trust(&self.agents),
        }
    }

    fn order_inner(&mut self, o: Order) {
        match o {
            Order::EvacuateAll => {
                self.agents.order_evacuation_all();
            }
            Order::EvacuateZone { centre, radius_m } => {
                self.agents.order_evacuation(centre, radius_m);
            }
            Order::Attack { kind, at } => {
                if let Some(id) = self.best_unit(kind) {
                    if self.crews.assign(id, Task::Attack { at }).is_ok() {
                        self.log.task(self.fire.time_s(), id, kind);
                    }
                }
            }
            Order::Drop { at } => {
                self.crews.request_air();
                if let Some(id) = self.best_unit(UnitKind::AirTanker) {
                    if self.crews.assign(id, Task::Drop { at }).is_ok() {
                        self.log.task(self.fire.time_s(), id, UnitKind::AirTanker);
                    }
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
        let drops_before: Vec<u32> = self.crews.units.iter().map(|u| u.drops).collect();
        for a in self.crews.step(STEP_S as f32, &self.agents.network, &self.agents.traffic, &self.fire, &self.scn) {
            self.fire.queue(a);
        }
        self.tally.note(&self.agents, &self.fire);
        if let Some(w) = self.wolf.as_mut() {
            w.step(&mut self.agents, &self.fire, &self.scn.world);
        }
        self.collect_events();
        let dropped: Vec<Pos> =
            self.crews.units.iter().zip(&drops_before).filter(|(u, &b)| u.drops > b).map(|(u, _)| u.pos).collect();
        for _ in &dropped {
            self.log.push(cost::Entry::AirLoad { at_s: self.fire.time_s() });
        }
        if self.tally.defence_enabled() {
            self.tally.note_defence(&self.crews, &self.agents, &self.fire, &self.scn.world, &dropped);
        }
        Ok(())
    }

    /// Where the fire is going: the burning cell furthest downwind of the
    /// opening ignition (current wind, so it follows a shift). This is where a
    /// commander sends units, and it moves, which is why policies resolve it at
    /// the moment they act rather than carrying a fixed point.
    fn downwind(&self) -> (f32, f32) {
        let w = self.fire.weather();
        // `wind_dir_deg` is where the wind blows FROM (finding 1).
        let to = (w.wind_dir_deg as f32 + 180.0).to_radians();
        (to.sin(), to.cos())
    }

    /// `d` metres downwind of [`Run::head`], clamped to the world.
    pub fn ahead(&self, d: f32) -> Pos {
        let h = self.head();
        if d == 0.0 {
            return h;
        }
        let (ux, uy) = self.downwind();
        let w = &self.scn.world;
        Pos { x: (h.x + ux * d).clamp(0.0, w.width_m), y: (h.y + uy * d).clamp(0.0, w.height_m) }
    }

    /// Up to `n` homes nearest the head of the fire that are at least
    /// [`DEFEND_REACH_M`]`* 2` apart: where "protect the town" posts its engines,
    /// one to a cluster rather than three on the same doorstep. Homes the fire has
    /// already burnt up to are skipped (nothing left to save there).
    pub fn clusters_near_head(&self, n: usize) -> Vec<Pos> {
        let h = self.head();
        let d2 = |a: Pos| (a.x - h.x).powi(2) + (a.y - h.y).powi(2);
        let mut homes: Vec<Pos> = self.agents.households.iter().map(|x| x.home).collect();
        homes.sort_by(|a, b| d2(*a).partial_cmp(&d2(*b)).unwrap_or(std::cmp::Ordering::Equal));
        let mut out: Vec<Pos> = vec![];
        for p in homes {
            if out.len() == n {
                break;
            }
            if out.iter().all(|q| (q.x - p.x).powi(2) + (q.y - p.y).powi(2) > (2.0 * DEFEND_REACH_M).powi(2)) {
                out.push(p);
            }
        }
        out
    }

    /// Where the most recent spot fire started, if there has been one.
    pub fn latest_spot(&self) -> Option<Pos> {
        self.events.iter().rev().find(|e| e.kind == EventKind::SpotFire).and_then(|e| e.pos)
    }

    /// Distance from the head of the fire to the nearest home, metres.
    pub fn head_to_town_m(&self) -> f32 {
        let h = self.head();
        self.agents
            .households
            .iter()
            .map(|x| ((x.home.x - h.x).powi(2) + (x.home.y - h.y).powi(2)).sqrt())
            .fold(f32::INFINITY, f32::min)
    }

    pub fn head(&self) -> Pos {
        let (ux, uy) = self.downwind();
        let o = self.spec.ignition;
        self.fire
            .active_cells()
            .iter()
            .map(|c| self.scn.world.centre_of(*c))
            .max_by(|a, b| {
                let pa = (a.x - o.x) * ux + (a.y - o.y) * uy;
                let pb = (b.x - o.x) * ux + (b.y - o.y) * uy;
                pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(o)
    }

    pub fn caught_where(&self, f: impl Fn(scenario::Pos) -> bool) -> usize {
        self.tally.caught_where(&self.agents, f)
    }

    /// Derive this step's events from the model's state (one pass, no text).
    fn collect_events(&mut self) {
        use abm::suppression::UnitState as S;
        let now = self.fire.time_s();
        let w = &mut self.watch;
        let mut out: Vec<Event> = vec![];
        let seen_to = w.last_spot_s;
        for s in self.agents.spot_fires().spots().filter(|s| s.at_s > seen_to) {
            out.push(Event { at_s: s.at_s as i64, kind: EventKind::SpotFire, pos: Some(s.pos) });
            w.last_spot_s = w.last_spot_s.max(s.at_s);
        }
        let wind = self.fire.weather().wind_dir_deg;
        if let Some(prev) = w.wind_from {
            if (prev - wind).abs() > 1.0 {
                out.push(Event { at_s: now, kind: EventKind::WindShifted { from_deg: prev as f32, to_deg: wind as f32 }, pos: None });
            }
        }
        w.wind_from = Some(wind);
        if w.unit_states.len() != self.crews.units.len() {
            w.unit_states = self.crews.units.iter().map(|u| u.state).collect();
        }
        for (k, u) in self.crews.units.iter().enumerate() {
            if u.state != w.unit_states[k] {
                match u.state {
                    S::Withdrawing => out.push(Event { at_s: now, kind: EventKind::UnitWithdrew { unit: u.id, kind: u.kind }, pos: Some(u.pos) }),
                    S::Lost => out.push(Event { at_s: now, kind: EventKind::UnitLost { unit: u.id, kind: u.kind }, pos: Some(u.pos) }),
                    _ => {}
                }
                w.unit_states[k] = u.state;
            }
        }
        let down = self.agents.comms().down();
        if down > w.masts_down {
            out.push(Event { at_s: now, kind: EventKind::MastDown, pos: None });
        }
        w.masts_down = down;
        self.events.extend(out);
        if !self.watch.near_town && self.head_to_town_m() <= event::NEAR_TOWN_M {
            self.watch.near_town = true;
            let pos = self.head();
            self.events.push(Event { at_s: now, kind: EventKind::FireNearTown, pos: Some(pos) });
        }
    }

    /// The moments `why` is built from, as this run saw them.
    pub fn facts(&self) -> crate::why::Facts {
        let first = |f: &dyn Fn(&EventKind) -> bool| self.events.iter().find(|e| f(&e.kind)).map(|e| e.at_s);
        crate::why::Facts {
            order_at_s: self
                .log
                .entries
                .iter()
                .find_map(|e| if let cost::Entry::Evacuation { at_s, .. } = e { Some(*at_s) } else { None }),
            shift_at_s: first(&|k| matches!(k, EventKind::WindShifted { .. })),
            near_town_s: first(&|k| *k == EventKind::FireNearTown),
            first_caught_s: self.tally.first_caught_s,
        }
    }

    /// Why it went the way it did: the end card's one sentence.
    pub fn why(&self) -> crate::why::Why {
        crate::why::why(&self.outcome(), self.facts())
    }

    /// The bill so far: a pure function of [`Run::log`].
    pub fn ledger(&self) -> cost::Ledger {
        self.log.price(self.time_s())
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
