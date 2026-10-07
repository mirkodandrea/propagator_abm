//! A headless run of one mission: the thing the COMPARE screen replays with no
//! orders, and the thing the beat tests measure. No Bevy.
//!
//! Steps with the same fire-then-agents order `game::Sim::advance` uses, at the
//! 6 s decision interval, so a figure taken here is the figure the kiosk shows.

use abm::suppression::{Suppression, Task, UnitKind, UnitState, DROP_WIDTH_M};
use abm::Abm;
use anyhow::Result;
use fire::FireSim;
use scenario::{Pos, Scenario};

use crate::cost;
use crate::district::{self, District};
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
    /// Warn one district by name (index into `Referee::districts`).
    EvacuateDistrict(usize),
    /// Send the best free unit of this kind to defend a district: posted at
    /// its edge facing the fire (spec §4 option B -- homes, not the front).
    Defend { kind: UnitKind, district: usize },
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
    /// When the fire first reached each household while it was still at home.
    caught_at: Vec<Option<i64>>,
    /// When the fire first reached somebody who was still at home.
    pub first_caught_s: Option<i64>,
    /// Home defence (spec §4 option B). `None` until [`Tally::enable_defence`]:
    /// with it off, `homes_lost` is the end-state rule above, bit for bit.
    defence: Option<Defence>,
}

/// A home an engine is posted to defend, or one a retardant drop has just
/// wetted, is lost only to flame contact ([`DEFENDED_RADIUS_M`]); an undefended
/// one is lost when burnt ground reaches [`LOST_RADIUS_M`]. Loss is latched at
/// the moment of contact, so *when* the engine got there is the whole mechanic:
/// one that arrives after the front protects nobody.
///
/// An engine defends from the moment it first starts work at its post until
/// it is re-tasked or lost -- **not** only while it is pumping. The model
/// rightly pulls a crew back out of lethal heat as the front passes, which is
/// exactly when the houses need it; structure protection is the crew being
/// there before and after, putting out what the front leaves on the roofs.
/// Measured: tied to `Working` alone, three engines on a district changed
/// homes lost by nothing (`district_sweep::defend_sweep`).
#[derive(Clone)]
struct Defence {
    /// Simulated second until which each household counts as defended.
    until_s: Vec<i64>,
    lost: Vec<bool>,
    /// Per unit: where it took up its post, and the task that sent it there.
    station: Vec<Option<(Pos, Task)>>,
    /// Per household, the fire-grid indices within [`DEFENDED_RADIUS_M`] and
    /// [`LOST_RADIUS_M`] of the home. Homes do not move, so this is computed
    /// once on the first step rather than re-derived for 250 homes every 6 s
    /// (it was most of a session's cost). Same cells, same answer.
    near_cells: Vec<(Vec<u32>, Vec<u32>)>,
}

/// An engine posted within this distance of a home defends it: about a
/// street's length either side, which is what one crew with two lines out
/// covers. Was 80 m, under which three engines on a district of a hundred homes
/// defended seven (`district_probe::engines_after_a_defend_order`).
pub const DEFEND_REACH_M: f32 = 120.0;
/// How long a retardant drop keeps the homes under it defended.
pub const DROP_DEFENCE_S: i64 = 15 * 60;
/// A defended home is lost only if burnt ground gets this close.
pub const DEFENDED_RADIUS_M: f32 = 25.0;

impl Tally {
    pub fn new(households: usize) -> Tally {
        Tally { caught_at: vec![None; households], first_caught_s: None, defence: None }
    }

    /// Turn on home defence. Inert until called; the kiosk does not call it.
    pub fn enable_defence(&mut self) {
        let n = self.caught_at.len();
        self.defence.get_or_insert_with(|| Defence { until_s: vec![0; n], lost: vec![false; n], station: vec![], near_cells: vec![] });
    }

    pub fn defence_enabled(&self) -> bool {
        self.defence.is_some()
    }

    /// Whether household `i` is being defended at `now_s`.
    pub fn defended(&self, i: usize, now_s: i64) -> bool {
        self.defence.as_ref().is_some_and(|d| d.until_s.get(i).is_some_and(|&u| u > now_s))
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
        use fire::CellFire;
        let Some(d) = self.defence.as_mut() else { return };
        let now = fire.time_s();
        let near = |a: Pos, b: Pos, r: f32| (a.x - b.x).powi(2) + (a.y - b.y).powi(2) <= r * r;
        if d.station.len() != crews.units.len() {
            d.station = vec![None; crews.units.len()];
        }
        for (k, u) in crews.units.iter().enumerate() {
            if u.kind != UnitKind::Engine || u.state == UnitState::Lost {
                d.station[k] = None;
                continue;
            }
            match d.station[k] {
                Some((_, task)) if task != u.task => d.station[k] = None,
                None if u.state == UnitState::Working && u.water_l > 0.0 => d.station[k] = Some((u.pos, u.task)),
                _ => {}
            }
        }
        for (i, h) in agents.households.iter().enumerate() {
            let on_scene = d.station.iter().flatten().any(|(p, _)| near(*p, h.home, DEFEND_REACH_M));
            let dropped = dropped_at.iter().any(|&p| near(p, h.home, DROP_WIDTH_M));
            if on_scene {
                d.until_s[i] = d.until_s[i].max(now + STEP_S);
            }
            if dropped {
                d.until_s[i] = d.until_s[i].max(now + DROP_DEFENCE_S);
            }
        }
        if d.near_cells.len() != agents.households.len() {
            let idx = |p: Pos, r: f32| -> Vec<u32> {
                fire::cells_in_radius(world, p, r).iter().map(|c| (c.row * world.fire_cols + c.col) as u32).collect()
            };
            d.near_cells = agents.households.iter().map(|h| (idx(h.home, DEFENDED_RADIUS_M), idx(h.home, LOST_RADIUS_M))).collect();
        }
        let state = fire.state();
        for i in 0..agents.households.len() {
            if d.lost[i] {
                continue;
            }
            let (near, far) = &d.near_cells[i];
            let cells = if d.until_s[i] > now { near } else { far };
            if cells.iter().any(|&c| state[c as usize] != CellFire::Unburnt) {
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
        for i in 0..self.caught_at.len() {
            if self.caught_at[i].is_none() && Self::endangered(i, agents, fire) {
                self.caught_at[i] = Some(fire.time_s());
                self.first_caught_s.get_or_insert(fire.time_s());
            }
        }
    }

    /// Households the fire has caught at home, by position -- lets a test ask
    /// about one hamlet rather than the whole town.
    pub fn caught_where(&self, agents: &Abm, f: impl Fn(scenario::Pos) -> bool) -> usize {
        agents.households.iter().zip(&self.caught_at).filter(|(h, c)| c.is_some() && f(h.home)).count()
    }

    /// When the fire reached household `i` at home, if it ever did.
    pub fn caught_at(&self, i: usize) -> Option<i64> {
        self.caught_at.get(i).copied().flatten()
    }

    /// Whether household `i`'s home has been reached (only with defence on).
    pub fn home_lost(&self, i: usize) -> Option<bool> {
        self.defence.as_ref().map(|d| d.lost[i])
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
            caught: self.caught_at.iter().filter(|c| c.is_some()).count(),
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
    /// The turn game (gameplay spec §2): every unit starts at one fire station
    /// ([`station`]) instead of round-robin over the refuges, so a token's ETA
    /// is a drive from one place. Off = the published staging.
    pub station: bool,
}

/// What the commander can be told and judged by, kept apart from who steps the
/// model: the headless [`Run`] and the kiosk's live `Sim` each own their fire,
/// agents and units, and each hands them to one `Referee` before and after
/// every step. The COMPARE screen, the cost, the trust meter, the districts'
/// stories and the events are therefore computed by the same code on both
/// sides, which is the only way the end card can be honest.
pub struct Referee {
    pub spec: Spec,
    pub tally: Tally,
    /// What the commander did, for [`Referee::ledger`] (spec 5.1).
    pub log: cost::Log,
    wolf: Option<trust::CryWolf>,
    /// Everything notable that has happened, oldest first (spec 5.4/5.6).
    pub events: Vec<Event>,
    pub districts: Vec<District>,
    /// One per district, same order.
    pub reports: Vec<district::Report>,
    watch: Watch,
    shift_pending: bool,
    drops_before: Vec<u32>,
    /// Where engines sent to each district were posted, so the next one goes
    /// somewhere else along the edge.
    posts: Vec<Vec<Pos>>,
}

/// Borrowed model parts, so the same [`Referee`] methods serve the headless
/// [`Run`] and the kiosk's `Sim`, which hold them under different names.
pub struct Parts<'a> {
    pub scn: &'a Scenario,
    pub fire: &'a mut FireSim,
    pub agents: &'a mut Abm,
    pub crews: &'a mut Suppression,
}

impl Referee {
    pub fn new(spec: Spec, scn: &Scenario, agents: &Abm, variant: Variant) -> Referee {
        let mut tally = Tally::new(agents.households.len());
        if variant.defend_homes {
            tally.enable_defence();
        }
        let districts = district::of(scn, agents);
        let reports = district::reports(&districts);
        Referee {
            spec,
            tally,
            log: cost::Log::default(),
            wolf: variant.cry_wolf.map(|d| trust::CryWolf::new(agents, d)),
            events: vec![],
            districts,
            reports,
            watch: Watch::default(),
            shift_pending: spec.shift.is_some(),
            drops_before: vec![],
            posts: vec![],
        }
    }

    /// Call before stepping the model: applies the scripted or drawn wind
    /// shift when it is due. Returns whether the weather changed.
    pub fn before_step(&mut self, fire: &mut FireSim, crews: &Suppression) -> Result<bool> {
        self.drops_before = crews.units.iter().map(|u| u.drops).collect();
        if let (true, Some(s)) = (self.shift_pending, self.spec.shift) {
            if fire.time_s() >= s.at_s {
                fire.set_weather(s.weather)?;
                self.shift_pending = false;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Call after stepping the model: outcome, cost, trust, events, districts.
    pub fn after_step(&mut self, m: Parts) {
        let Parts { scn, fire, agents, crews } = m;
        self.tally.note(agents, fire);
        if let Some(w) = self.wolf.as_mut() {
            w.step(agents, fire, &scn.world);
        }
        let dropped: Vec<Pos> = crews
            .units
            .iter()
            .enumerate()
            .filter(|(k, u)| u.drops > self.drops_before.get(*k).copied().unwrap_or(u.drops))
            .map(|(_, u)| u.pos)
            .collect();
        for _ in &dropped {
            self.log.push(cost::Entry::AirLoad { at_s: fire.time_s() });
        }
        if self.tally.defence_enabled() {
            self.tally.note_defence(crews, agents, fire, &scn.world, &dropped);
            let now = fire.time_s();
            for (d, r) in self.districts.iter().zip(self.reports.iter_mut()) {
                if r.defended_at_s.is_none() && d.households.iter().any(|&i| self.tally.defended(i, now)) {
                    r.defended_at_s = Some(now);
                }
            }
        }
        district::note(&self.districts, &mut self.reports, &self.tally, agents, fire, scn);
        self.collect_events(fire, agents, crews, scn);
    }

    /// Give an order. Everything the player can do goes through here.
    pub fn order(&mut self, o: Order, m: Parts) {
        let Parts { scn, fire, agents, crews } = m;
        let now = fire.time_s();
        let before: Vec<bool> = agents.households.iter().map(|h| h.ordered).collect();
        match o {
            Order::EvacuateAll => {
                agents.order_evacuation_all();
            }
            Order::EvacuateZone { centre, radius_m } => {
                agents.order_evacuation(centre, radius_m);
            }
            Order::EvacuateDistrict(d) => {
                if let Some(d) = self.districts.get(d) {
                    agents.order_evacuation_of(&d.households);
                }
            }
            Order::Attack { kind, at } => {
                if let Some(id) = best_unit(crews, kind) {
                    if crews.assign(id, Task::Attack { at }).is_ok() {
                        self.log.task(now, id, kind);
                    }
                }
            }
            Order::Defend { kind, district } => {
                if let Some(d) = self.districts.get(district) {
                    if self.posts.len() < self.districts.len() {
                        self.posts.resize(self.districts.len(), vec![]);
                    }
                    let at = d.post_facing(agents, head_of(fire, scn, self.spec.ignition), &self.posts[district], 2.0 * DEFEND_REACH_M);
                    if let Some(id) = best_unit(crews, kind) {
                        if crews.assign(id, Task::Attack { at }).is_ok() {
                            self.log.task(now, id, kind);
                            self.posts[district].push(at);
                        }
                    }
                }
            }
            Order::Drop { at } => {
                crews.request_air();
                if let Some(id) = best_unit(crews, UnitKind::AirTanker) {
                    if crews.assign(id, Task::Drop { at }).is_ok() {
                        self.log.task(now, id, UnitKind::AirTanker);
                    }
                }
            }
        }
        if matches!(o, Order::EvacuateAll | Order::EvacuateZone { .. } | Order::EvacuateDistrict(_)) {
            let moved: Vec<usize> = (0..before.len()).filter(|&i| !before[i] && agents.households[i].ordered).collect();
            if !moved.is_empty() {
                for (d, r) in self.districts.iter().zip(self.reports.iter_mut()) {
                    if r.warned_at_s.is_none() && d.households.iter().any(|i| moved.contains(i)) {
                        r.warned_at_s = Some(now);
                    }
                }
                self.log.push(cost::Entry::Evacuation { at_s: now, households: moved.len() });
                if let Some(w) = self.wolf.as_mut() {
                    w.note_order(now, moved);
                }
            }
        }
    }

    /// Trust and anger for the HUD (spec 5.2). Constant while cry-wolf is off.
    pub fn trust(&self, agents: &Abm) -> trust::Trust {
        match &self.wolf {
            Some(w) => w.trust(agents),
            None => trust::CryWolf::new(agents, 0.0).trust(agents),
        }
    }

    /// Engines posted to defend district `d` so far.
    pub fn posted(&self, d: usize) -> usize {
        self.posts.get(d).map_or(0, |p| p.len())
    }

    /// Where those engines were posted (each defends [`DEFEND_REACH_M`] around it).
    pub fn posts(&self, d: usize) -> &[Pos] {
        self.posts.get(d).map_or(&[], |p| p.as_slice())
    }

    /// Orders the town has judged false alarms so far (cry-wolf on only).
    pub fn needless_orders(&self) -> usize {
        self.wolf.as_ref().map_or(0, |w| w.needless_orders)
    }

    /// The bill so far: a pure function of [`Referee::log`].
    pub fn ledger(&self, now_s: i64) -> cost::Ledger {
        self.log.price(now_s)
    }

    pub fn outcome(&self, m: &Parts) -> Outcome {
        self.tally.outcome(m.agents, m.fire, &m.scn.world)
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

    /// Derive this step's events from the model's state (one pass, no text).
    fn collect_events(&mut self, fire: &FireSim, agents: &Abm, crews: &Suppression, scn: &Scenario) {
        use abm::suppression::UnitState as S;
        let now = fire.time_s();
        let w = &mut self.watch;
        let mut out: Vec<Event> = vec![];
        let seen_to = w.last_spot_s;
        for s in agents.spot_fires().spots().filter(|s| s.at_s > seen_to) {
            out.push(Event { at_s: s.at_s as i64, kind: EventKind::SpotFire, pos: Some(s.pos) });
            w.last_spot_s = w.last_spot_s.max(s.at_s);
        }
        let wind = fire.weather().wind_dir_deg;
        if let Some(prev) = w.wind_from {
            if (prev - wind).abs() > 1.0 {
                out.push(Event { at_s: now, kind: EventKind::WindShifted { from_deg: prev as f32, to_deg: wind as f32 }, pos: None });
            }
        }
        w.wind_from = Some(wind);
        if w.unit_states.len() != crews.units.len() {
            w.unit_states = crews.units.iter().map(|u| u.state).collect();
        }
        for (k, u) in crews.units.iter().enumerate() {
            if u.state != w.unit_states[k] {
                match u.state {
                    S::Withdrawing => out.push(Event { at_s: now, kind: EventKind::UnitWithdrew { unit: u.id, kind: u.kind }, pos: Some(u.pos) }),
                    S::Lost => out.push(Event { at_s: now, kind: EventKind::UnitLost { unit: u.id, kind: u.kind }, pos: Some(u.pos) }),
                    _ => {}
                }
                w.unit_states[k] = u.state;
            }
        }
        let down = agents.comms().down();
        if down > w.masts_down {
            out.push(Event { at_s: now, kind: EventKind::MastDown, pos: None });
        }
        w.masts_down = down;
        // District transitions: the moments an advisor speaks about.
        if w.levels.len() != self.reports.len() {
            w.levels = vec![district::Level::Calm; self.reports.len()];
        }
        for (k, r) in self.reports.iter().enumerate() {
            let level = r.level();
            if level > w.levels[k] {
                if level >= district::Level::Threatened {
                    let kind = if level == district::Level::Reached { EventKind::DistrictReached { district: k } } else { EventKind::DistrictThreatened { district: k } };
                    out.push(Event { at_s: now, kind, pos: Some(self.districts[k].centre) });
                }
                w.levels[k] = level;
            }
        }
        if let Some(wolf) = &self.wolf {
            if wolf.needless_orders > w.needless_seen {
                w.needless_seen = wolf.needless_orders;
                out.push(Event { at_s: now, kind: EventKind::FalseAlarm, pos: None });
            }
        }
        self.events.extend(out);
        let head = head_of(fire, scn, self.spec.ignition);
        let near = agents
            .households
            .iter()
            .map(|x| ((x.home.x - head.x).powi(2) + (x.home.y - head.y).powi(2)).sqrt())
            .fold(f32::INFINITY, f32::min);
        if !self.watch.near_town && near <= event::NEAR_TOWN_M {
            self.watch.near_town = true;
            self.events.push(Event { at_s: now, kind: EventKind::FireNearTown, pos: Some(head) });
        }
    }
}

/// The unit the kiosk's button would send: a free one of this kind, preferring
/// whichever is closest to being ready.
pub fn best_unit(crews: &Suppression, kind: UnitKind) -> Option<usize> {
    use abm::suppression::UnitState as S;
    crews.units.iter().filter(|u| u.kind == kind && u.assignable()).min_by_key(|u| match u.state {
        S::Staged => 0,
        S::Inbound => 1,
        S::Moving | S::Working | S::Refilling => 2,
        _ => 3,
    }).map(|u| u.id)
}

/// Where the fire is going: the burning cell furthest downwind of the opening
/// ignition (current wind, so it follows a shift).
pub fn head_of(fire: &FireSim, scn: &Scenario, ignition: Pos) -> Pos {
    let (ux, uy) = downwind(fire);
    fire.active_cells()
        .iter()
        .map(|c| scn.world.centre_of(*c))
        .max_by(|a, b| {
            let pa = (a.x - ignition.x) * ux + (a.y - ignition.y) * uy;
            let pb = (b.x - ignition.x) * ux + (b.y - ignition.y) * uy;
            pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(ignition)
}

/// Unit vector the wind blows *toward*.
fn downwind(fire: &FireSim) -> (f32, f32) {
    // `wind_dir_deg` is where the wind blows FROM (finding 1).
    let to = (fire.weather().wind_dir_deg as f32 + 180.0).to_radians();
    (to.sin(), to.cos())
}

pub struct Run {
    pub scn: Scenario,
    pub fire: FireSim,
    pub agents: Abm,
    pub crews: Suppression,
    pub spec: Spec,
    pub referee: Referee,
}

/// What [`Referee::after_step`] compared against last time, to turn state into events.
#[derive(Default)]
struct Watch {
    last_spot_s: f32,
    wind_from: Option<f64>,
    unit_states: Vec<abm::suppression::UnitState>,
    masts_down: usize,
    near_town: bool,
    levels: Vec<district::Level>,
    needless_seen: usize,
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
        let bases = if variant.station { vec![station(&agents, scn.world.centre_of(centre))] } else { staging(&agents, scn.world.centre_of(centre)) };
        let mut crews = Suppression::new(&scn, &bases)?;
        crews.effect = variant.unit_effect;
        let referee = Referee::new(spec, &scn, &agents, variant);
        Ok(Run { scn, fire, agents, crews, spec, referee })
    }

    pub fn time_s(&self) -> i64 {
        self.fire.time_s()
    }

    pub fn first_caught_s(&self) -> Option<i64> {
        self.referee.tally.first_caught_s
    }

    pub fn order(&mut self, o: Order) {
        let Run { scn, fire, agents, crews, referee, .. } = self;
        referee.order(o, Parts { scn, fire, agents, crews });
    }

    /// Trust and anger for the HUD (spec 5.2). Constant while cry-wolf is off.
    pub fn trust(&self) -> trust::Trust {
        self.referee.trust(&self.agents)
    }

    /// One step: scripted weather, then fire, then agents, then units, then the books.
    pub fn step(&mut self) -> Result<()> {
        self.referee.before_step(&mut self.fire, &self.crews)?;
        self.fire.advance(STEP_S)?;
        self.agents.step(STEP_S as f32, &self.fire, &self.scn);
        for a in self.crews.step(STEP_S as f32, &self.agents.network, &self.agents.traffic, &self.fire, &self.scn) {
            self.fire.queue(a);
        }
        let Run { scn, fire, agents, crews, referee, .. } = self;
        referee.after_step(Parts { scn, fire, agents, crews });
        Ok(())
    }

    /// `d` metres downwind of [`Run::head`], clamped to the world.
    pub fn ahead(&self, d: f32) -> Pos {
        let h = self.head();
        if d == 0.0 {
            return h;
        }
        let (ux, uy) = downwind(&self.fire);
        let w = &self.scn.world;
        Pos { x: (h.x + ux * d).clamp(0.0, w.width_m), y: (h.y + uy * d).clamp(0.0, w.height_m) }
    }

    /// Up to `n` homes nearest the head of the fire that are at least
    /// [`DEFEND_REACH_M`]`* 2` apart: where "protect the town" posts its engines,
    /// one to a cluster rather than three on the same doorstep.
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
        self.referee.events.iter().rev().find(|e| e.kind == EventKind::SpotFire).and_then(|e| e.pos)
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

    /// The district lying most nearly along `bearing_to_deg` (the way the wind
    /// blows *toward*) from the opening ignition.
    pub fn district_toward(&self, bearing_to_deg: f32) -> Option<usize> {
        let a = bearing_to_deg.to_radians();
        let (ux, uy) = (a.sin(), a.cos());
        let o = self.spec.ignition;
        self.referee
            .districts
            .iter()
            .enumerate()
            .map(|(k, d)| {
                let (dx, dy) = (d.centre.x - o.x, d.centre.y - o.y);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                (k, (dx * ux + dy * uy) / len)
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(k, _)| k)
    }

    pub fn head(&self) -> Pos {
        head_of(&self.fire, &self.scn, self.spec.ignition)
    }

    pub fn caught_where(&self, f: impl Fn(scenario::Pos) -> bool) -> usize {
        self.referee.tally.caught_where(&self.agents, f)
    }

    pub fn facts(&self) -> crate::why::Facts {
        self.referee.facts()
    }

    /// Why it went the way it did: the end card's one sentence.
    pub fn why(&self) -> crate::why::Why {
        crate::why::why(&self.outcome(), self.facts())
    }

    /// The bill so far: a pure function of the referee's log.
    pub fn ledger(&self) -> cost::Ledger {
        self.referee.ledger(self.time_s())
    }

    pub fn outcome(&self) -> Outcome {
        self.referee.tally.outcome(&self.agents, &self.fire, &self.scn.world)
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

/// The turn game's fire station: the map-edge exit nearest the fire, where
/// the road comes in from the next town. Measured in `tests/sweeps.rs`
/// (`station_drive_times`): from here the patrol reaches Il Borgo's
/// fire-facing edge in a few minutes, which is the delay the patrol token is
/// about (gameplay spec §7.4). Falls back to the nearest refuge of any kind.
pub fn station(agents: &Abm, ignition: Pos) -> Pos {
    let d = |p: &Pos| (p.x - ignition.x).powi(2) + (p.y - ignition.y).powi(2);
    let pick = |exit: bool| {
        agents.refuges.iter().filter(|r| r.is_exit == exit).map(|r| r.pos).min_by(|a, b| d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal))
    };
    pick(true).or_else(|| pick(false)).unwrap_or(ignition)
}

/// Where suppression units stage: the refuges, closest to the fire first --
/// out of the fuel and reachable by road, which is what a staging area needs.
/// The kiosk's `Sim` uses the same order, so unit ids match the twin's.
pub fn staging(agents: &Abm, ignition: Pos) -> Vec<Pos> {
    let mut v: Vec<Pos> = agents.refuges.iter().map(|r| r.pos).collect();
    let d = |p: &Pos| (p.x - ignition.x).powi(2) + (p.y - ignition.y).powi(2);
    v.sort_by(|a, b| d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal));
    v
}
