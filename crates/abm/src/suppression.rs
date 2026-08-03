//! Suppression resources as agents: hand crews, engines, and air tankers.
//!
//! The commander's other lever. Evacuation moves the people; this moves the
//! fire — or fails to, which is most of what an initial attack teaches. A unit
//! here is an agent in the same sense a household is: it has a position on the
//! real network, a state, a task it was given, consumables it runs out of, and
//! a safety rule that will override the order it was handed.
//!
//! ## What each kind can actually do, and why they are not interchangeable
//!
//! | | Moves on | Acts by | Runs out of | Held back by |
//! |---|---|---|---|---|
//! | **Hand crew** (`squadra AIB`, 5) | roads *and* tracks, on foot beyond them | cutting line — permanent fuel removal | daylight, not supplies | [`LINE_M_PER_H`]: 120 m/h in macchia |
//! | **Engine** (`autobotte`, 2,500 L) | drivable roads only | water, from the road | its tank, in 6 minutes of pumping | [`ENGINE_REACH_M`]: 60 m of hose |
//! | **Air tanker** (CL-415) | straight lines, over everything | 6,137 L in one swath | nothing, but each cycle costs minutes | arriving at all: [`AIR_RESPONSE_S`] |
//!
//! Those three constraints are the whole game. The engine is fast and useless
//! away from a road; the crew can reach anywhere and cuts line slower than the
//! fire spreads; the aircraft can hit anything but takes 25 minutes to show up
//! and its water wears off. Every one of the numbers is sourced in the
//! constant's own doc comment, because a serious game that invents its
//! production rates is just a game.
//!
//! ## Three rules inherited from the civilian model
//!
//! **Nothing accumulates per call.** Water is a flow rate in litres per second
//! and line is metres per second, integrated over `dt`. The game steps at 2 s
//! and the tests at 60 s; `water_is_independent_of_step_size` pins that they
//! agree. This is the same trap `fire::exposure` fell into.
//!
//! **Work goes on the fuel, not the flames.** A cell the core has lit stays
//! lit, so wetting the front changes nothing (`fire::intervention`). Units
//! target [`fire::FireSim::is_suppressible`] cells — unburnt, burnable, not
//! already cut — which is also what direct attack means in the field: you wet
//! what is about to burn.
//!
//! **Safety overrides orders.** A unit ordered into a place the threat field
//! says is lethal withdraws and says so ([`Unit::note`]), rather than dying
//! quietly to satisfy the player. It can still be caught — [`UnitState::Lost`]
//! is reachable — but only by the fire moving onto it, never by obedience.
//!
//! ## What a unit decides for itself, through the behavior graph
//!
//! Almost everything a unit does is the commander's decision. What is left —
//! and it is the whole of the unit's own agency — is *when to stop*: pull back
//! because the ground is not survivable, break off because the tank is empty,
//! hold or go home because the order was a bad one. That block, and only that
//! block, is what the applied graph controls; see
//! [`Suppression::unit_outcome`]. Where a unit is sent, how fast it gets there
//! and what its work does to the fire are all untouched, which is why a policy
//! a scientist wrote can be run without review.
//!
//! A policy is mandatory. The reference graph encodes [`WORK_LIMIT`] and the
//! dry-tank rule, while custom graphs can change them without creating a
//! second decision path in this module.

use anyhow::Result;
use behavior::{Observation, UnitObs};
use fire::{FireSim, Intervention};
use scenario::{Cell, Pos, Scenario};

use crate::behaviour::{unit_kind_of, unit_outcome_of, UnitOutcome, UnitRuntime};
use crate::network::{self, NodeId, RoadNetwork, NO_NODE};
use crate::traffic::Traffic;

// --- rates and capacities ---------------------------------------------------

/// Hand-line production for one 5-person squadra in Mediterranean shrub,
/// metres per hour. Italian AIB and US fireline handbook figures for hand
/// tools in heavy brush both land near 100-150 m/h per squad; the low number is
/// the honest one for macchia on a slope. Two hours of work is therefore ~240 m
/// of line, which is *less than the flank of this fire* — that is not a
/// modelling failure, it is why aircraft exist.
pub const LINE_M_PER_H: f32 = 120.0;
/// Width of a cut hand line, metres. Two to three times fuel height is the
/// standard rule; macchia is 2-3 m, so a 6 m line is a realistic hand line and
/// covers less than half a fire cell.
pub const LINE_WIDTH_M: f32 = 6.0;

/// Engine tank, litres. A Ligurian `autobotte` on a hill road is a 2,500 L
/// class vehicle, not a 10,000 L tanker.
pub const ENGINE_TANK_L: f32 = 2_500.0;
/// Engine pump rate on an attack line, litres per minute.
pub const ENGINE_PUMP_LPM: f32 = 400.0;
/// How far from the road an engine can work: one standard hose lay.
pub const ENGINE_REACH_M: f32 = 60.0;
/// Refill rate at a hydrant, litres per minute. A full tank in ~2.5 minutes.
pub const HYDRANT_LPM: f32 = 1_000.0;

/// CL-415 load, litres.
pub const TANKER_LOAD_L: f32 = 6_137.0;
/// Drop footprint: length along the run and width across it, metres. A single
/// pass at coverage level 1 over ~220 x 60 m.
pub const DROP_LENGTH_M: f32 = 220.0;
pub const DROP_WIDTH_M: f32 = 60.0;
/// Cruise speed, m/s (~290 km/h).
pub const TANKER_SPEED: f32 = 80.0;
/// Scoop run: touch down, fill, climb out.
pub const SCOOP_S: f32 = 90.0;
/// How long after the request the first aircraft is overhead. Ligurian air
/// support comes from the national fleet, not from Spotorno: 25 minutes is the
/// optimistic end of a real dispatch.
pub const AIR_RESPONSE_S: f32 = 25.0 * 60.0;

/// Engine road speed, m/s (~45 km/h): blue lights on a coast road with hairpins.
const ENGINE_SPEED: f32 = 12.0;
/// Hand crew network speed, m/s. A blend, deliberately: they ride a light
/// vehicle where there is a track and walk where there is not, and modelling
/// the transfer explicitly would add a state nobody would ever look at.
const CREW_SPEED: f32 = 3.0;
/// Crew speed off the network, m/s, before the slope correction.
const CREW_WALK_SPEED: f32 = 1.1;

// --- safety -----------------------------------------------------------------

/// Threat above which a unit will not work. Below `fire::threat::IMPASSABLE`
/// (0.55) on purpose: firefighters are not civilians who happen to be braver,
/// they are people with a stated safety margin, and they disengage while they
/// still can.
pub const WORK_LIMIT: f32 = 0.35;
/// Seconds of direct flame exposure a unit survives. Shorter than a civilian's
/// shelter time: they are in the open with PPE, not in a house.
const BURNOVER_S: f32 = 90.0;
/// Recovery of accumulated heat once clear, per second of clear air.
const HEAT_RECOVERY: f32 = 0.5;

/// How close a unit has to get to count as arrived, metres.
const ARRIVE_M: f32 = 15.0;
/// Longest sub-step the unit model integrates in. Matches the civilian model's
/// `MAX_SUBSTEP_S`; see [`Suppression::step`] for why it is needed at all.
const SUBSTEP_S: f32 = 4.0;
/// Ground units re-plan at most this often, so a cut road is noticed within a
/// minute without re-running A* every step.
const REROUTE_S: f32 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    HandCrew,
    Engine,
    AirTanker,
}

impl UnitKind {
    pub fn label(self) -> &'static str {
        match self {
            UnitKind::HandCrew => "hand crew",
            UnitKind::Engine => "engine",
            UnitKind::AirTanker => "air tanker",
        }
    }

    /// Can this unit only use roads a vehicle can drive?
    pub fn drivable_only(self) -> bool {
        matches!(self, UnitKind::Engine)
    }

    pub fn is_air(self) -> bool {
        matches!(self, UnitKind::AirTanker)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitState {
    /// Air support that has not been requested yet. Not on the incident, not
    /// drawable, not assignable.
    Unavailable,
    /// Requested and inbound, but not here yet: see [`AIR_RESPONSE_S`].
    Inbound,
    /// On the incident with nothing to do.
    Staged,
    /// Travelling to its task.
    Moving,
    /// Working: cutting line, pumping, or lined up on a drop.
    Working,
    /// Going for water, or scooping it.
    Refilling,
    /// Pulling back because where it was sent is not survivable.
    Withdrawing,
    /// Burnt over. Terminal, and the one outcome the player should never cause.
    Lost,
}

impl UnitState {
    pub fn label(self) -> &'static str {
        match self {
            UnitState::Unavailable => "not requested",
            UnitState::Inbound => "inbound",
            UnitState::Staged => "staged",
            UnitState::Moving => "responding",
            UnitState::Working => "working",
            UnitState::Refilling => "refilling",
            UnitState::Withdrawing => "withdrawing",
            UnitState::Lost => "lost",
        }
    }
}

/// What a unit has been told to do. The commander's whole vocabulary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Task {
    /// Stand by where you are.
    Hold,
    /// Direct attack: work the fire edge nearest this point. For an engine that
    /// means wetting the fuel ahead of the front within hose reach of the road;
    /// for a crew it means cutting line across the front there.
    Attack { at: Pos },
    /// Cut a fuel break along this alignment. Hand crews only — the one order
    /// whose effect is permanent.
    Line { from: Pos, to: Pos },
    /// Put one load on this point, then come back for another. Air only.
    Drop { at: Pos },
    /// Return to staging and stand by.
    Return,
}

impl Task {
    /// Where the order points, for the map marker and the route.
    pub fn focus(&self) -> Option<Pos> {
        match self {
            Task::Hold | Task::Return => None,
            Task::Attack { at } | Task::Drop { at } => Some(*at),
            Task::Line { from, .. } => Some(*from),
        }
    }
}

/// One suppression resource.
#[derive(Debug, Clone)]
pub struct Unit {
    pub id: usize,
    pub kind: UnitKind,
    /// What the radio calls it. Real Italian conventions: engines are
    /// `Autobotte 1`, crews `Squadra A`, aircraft `Canadair 1`.
    pub callsign: String,
    pub pos: Pos,
    /// Bearing of travel, radians in the world frame, for rendering.
    pub heading: f32,
    pub state: UnitState,
    pub task: Task,
    /// Where it stages, and returns to.
    pub base: Pos,
    pub water_l: f32,
    pub tank_l: f32,
    /// Simulated time this unit becomes usable ([`UnitState::Inbound`] only).
    pub arrives_at_s: f32,
    /// Metres of the current [`Task::Line`] already cut, and the alignment it
    /// is cutting. Kept as its own field rather than derived from position so a
    /// crew that has to withdraw and come back does not start over.
    pub line_done_m: f32,
    /// Accumulated flame exposure, seconds. Same currency as a civilian's.
    pub heat_s: f32,
    /// Cumulative work, for the readout and the debrief.
    pub water_used_l: f32,
    pub line_cut_m: f32,
    pub drops: u32,
    /// Why this unit is not doing what it was told, in words the UI can show.
    /// Empty when it is simply getting on with it.
    pub note: &'static str,

    /// Nodes still to visit. Ground units only; air flies straight.
    route: Vec<NodeId>,
    at_node: NodeId,
    /// Simulated time the route was planned, for [`REROUTE_S`].
    planned_at_s: f32,
    /// What the current route was planned *toward*.
    ///
    /// The re-plan trigger, together with [`REROUTE_S`]. An empty route is
    /// deliberately not a trigger: a unit that has arrived has an empty route
    /// for the rest of the incident, and re-planning on that ran a 61 k-node A*
    /// per unit per sub-step, which is how the model went from milliseconds to
    /// minutes.
    route_to: Option<Pos>,
    /// Task to resume once refilling is done.
    resume: Option<Task>,
    /// Simulated time this unit's current order was given, for
    /// `UnitObs::minutes_on_task`.
    tasked_at_s: f32,
    /// Which editable policy governs this unit. Resolved once at build from its
    /// kind; construction fails when the library leaves a kind uncovered.
    policy: usize,
    /// Where an air tanker is heading right now: the target, or the water.
    air_leg: AirLeg,
    air_timer_s: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AirLeg {
    ToTarget,
    ToWater,
    Scooping,
}

impl Unit {
    /// Fraction of the tank remaining, 0-1. Meaningless for a hand crew, which
    /// carries hand tools and no water.
    pub fn water_frac(&self) -> f32 {
        if self.tank_l <= 0.0 {
            0.0
        } else {
            (self.water_l / self.tank_l).clamp(0.0, 1.0)
        }
    }

    /// Able to take an order.
    ///
    /// Includes [`UnitState::Inbound`]: an aircraft on its way can be briefed
    /// before it arrives, which is both what happens on a real incident and the
    /// difference between air support that starts working the moment it is
    /// overhead and air support that circles waiting to be noticed. What cannot
    /// be tasked is a unit that has not been *asked for* — that is what
    /// [`Suppression::request_air`] is for.
    pub fn assignable(&self) -> bool {
        !matches!(self.state, UnitState::Unavailable | UnitState::Lost)
    }

    /// On the incident: drawable, and countable as a resource in hand.
    pub fn on_scene(&self) -> bool {
        !matches!(
            self.state,
            UnitState::Unavailable | UnitState::Inbound | UnitState::Lost
        )
    }
}

/// Aggregate readout, like [`crate::Stats`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SuppressionStats {
    pub staged: usize,
    pub responding: usize,
    pub working: usize,
    pub refilling: usize,
    pub withdrawing: usize,
    pub inbound: usize,
    pub lost: usize,
    /// Air tankers not yet requested.
    pub unrequested: usize,
    pub water_l: f64,
    pub line_m: f32,
    pub drops: u32,
}

pub struct Suppression {
    pub units: Vec<Unit>,
    /// Hydrants and open water, split because they mean different things: an
    /// engine fills from a hydrant, an aircraft scoops from open water.
    hydrants: Vec<Pos>,
    open_water: Vec<Pos>,
    time_s: f32,
    /// The editable unit policy. There is no alternate hand-written policy.
    policy: UnitRuntime,
    /// Bumped whenever something a view would draw has changed.
    pub generation: u64,
}

/// The default roster. Deliberately thin: a Ligurian initial attack is two or
/// three engines and a couple of volunteer squads, with air support requested
/// and waited for. A commander who can solve the scenario with what is already
/// on scene is not being asked anything.
pub const DEFAULT_ENGINES: usize = 3;
pub const DEFAULT_CREWS: usize = 3;
pub const DEFAULT_TANKERS: usize = 2;

impl Suppression {
    /// Build the roster, staged at `bases`.
    ///
    /// `bases` are the measured refuges (`crate::refuge`) — road nodes with
    /// almost no burnable fuel around them, reachable by vehicle. That is the
    /// same set of properties a staging area needs, and reusing them means the
    /// engines start somewhere defensible rather than somewhere authored.
    pub fn new(scn: &Scenario, bases: &[Pos]) -> Result<Suppression> {
        let lib = behavior::defaults::default_library();
        let policy = UnitRuntime::build(&lib)
            .map_err(|e| anyhow::anyhow!(e))?
            .ok_or_else(|| anyhow::anyhow!("no active suppression-unit behaviour profiles"))?;
        Suppression::with_policy(scn, bases, policy)
    }

    /// The same, running an authored unit policy.
    ///
    /// The runtime arrives here rather than being attached afterwards so which
    /// policy governs each unit is resolved once, at build, from its kind —
    /// which is what makes the answer to "why did this engine do that" a lookup
    /// in one file rather than a re-derivation.
    pub fn with_policy(
        scn: &Scenario,
        bases: &[Pos],
        policy: UnitRuntime,
    ) -> Result<Suppression> {
        anyhow::ensure!(!bases.is_empty(), "no staging area for suppression units");

        let hydrants: Vec<Pos> = scn
            .vectors
            .water
            .iter()
            .filter(|w| w.kind == "hydrant")
            .map(|w| Pos { x: w.pos[0], y: w.pos[1] })
            .collect();
        let open_water: Vec<Pos> = scn
            .vectors
            .water
            .iter()
            .filter(|w| w.kind != "hydrant")
            .map(|w| Pos { x: w.pos[0], y: w.pos[1] })
            .collect();

        let mut units = Vec::new();
        let policy_for = |kind: UnitKind| policy.assign(unit_kind_of(kind));
        let push = |kind: UnitKind, n: usize, units: &mut Vec<Unit>| -> Result<()> {
            let policy = policy_for(kind).ok_or_else(|| {
                anyhow::anyhow!("no active behaviour profile covers {} units", kind.label())
            })?;
            for i in 0..n {
                // Round-robin the staging areas so the roster is spread across
                // the town rather than parked in one car park.
                let base = bases[units.len() % bases.len()];
                let (tank, state) = match kind {
                    UnitKind::Engine => (ENGINE_TANK_L, UnitState::Staged),
                    UnitKind::HandCrew => (0.0, UnitState::Staged),
                    // Air support has to be asked for.
                    UnitKind::AirTanker => (TANKER_LOAD_L, UnitState::Unavailable),
                };
                let callsign = match kind {
                    UnitKind::Engine => format!("Autobotte {}", i + 1),
                    UnitKind::HandCrew => {
                        format!("Squadra {}", (b'A' + i as u8) as char)
                    }
                    UnitKind::AirTanker => format!("Canadair {}", i + 1),
                };
                units.push(Unit {
                    id: units.len(),
                    kind,
                    callsign,
                    pos: base,
                    heading: 0.0,
                    state,
                    task: Task::Hold,
                    base,
                    water_l: tank,
                    tank_l: tank,
                    arrives_at_s: 0.0,
                    line_done_m: 0.0,
                    heat_s: 0.0,
                    water_used_l: 0.0,
                    line_cut_m: 0.0,
                    drops: 0,
                    note: "",
                    route: Vec::new(),
                    at_node: NO_NODE,
                    // Negative infinity, not zero: a unit that has never been
                    // given an order must plan on the first step it needs to
                    // move, and `stale` is what makes that happen.
                    planned_at_s: f32::NEG_INFINITY,
                    route_to: None,
                    resume: None,
                    tasked_at_s: 0.0,
                    policy,
                    air_leg: AirLeg::ToTarget,
                    air_timer_s: 0.0,
                });
            }
            Ok(())
        };
        push(UnitKind::Engine, DEFAULT_ENGINES, &mut units)?;
        push(UnitKind::HandCrew, DEFAULT_CREWS, &mut units)?;
        push(UnitKind::AirTanker, DEFAULT_TANKERS, &mut units)?;

        Ok(Suppression {
            units,
            hydrants,
            open_water,
            time_s: 0.0,
            policy,
            generation: 0,
        })
    }

    pub fn time_s(&self) -> f32 {
        self.time_s
    }

    /// The editable unit policy in force.
    pub fn policy(&self) -> &UnitRuntime {
        &self.policy
    }

    /// Which policy governs a unit, for the inspector.
    pub fn policy_of(&self, unit: usize) -> Option<(&str, &str)> {
        let p = self.policy.policy(self.units.get(unit)?.policy)?;
        Some((&p.id, &p.name))
    }

    /// Re-run one unit's policy with a full trace, for the inspector.
    ///
    /// Deliberately not cached, for the same reason `Abm::explain` is not: the
    /// answer has to be the one the current fire state produces rather than the
    /// one from whenever the sub-step last ran.
    pub fn explain(
        &self,
        unit: usize,
        net: &RoadNetwork,
        fire: &FireSim,
        scn: &Scenario,
    ) -> Option<(behavior::Decision, behavior::Trace)> {
        let policy = self.units.get(unit)?.policy;
        let danger = fire.threat().at(self.units[unit].pos);
        let obs = self.observe(unit, danger, Some(net), fire, scn);
        self.policy.explain(policy, &obs)
    }

    /// Give a unit an order.
    ///
    /// Returns why it cannot be taken rather than failing silently: "that crew
    /// is 4 km away" and "engines cannot cut line" are things the player has to
    /// be told, and the UI has nowhere else to learn them.
    pub fn assign(&mut self, id: usize, task: Task) -> Result<(), &'static str> {
        let unit = self.units.get_mut(id).ok_or("no such unit")?;
        if !unit.assignable() {
            return Err(match unit.state {
                UnitState::Unavailable => "not on the incident: request air support first",
                _ => "unit is lost",
            });
        }
        match (unit.kind, &task) {
            (UnitKind::AirTanker, Task::Attack { .. } | Task::Line { .. }) => {
                return Err("aircraft drop water; they cannot work a line")
            }
            (UnitKind::Engine, Task::Line { .. }) => {
                return Err("an engine cannot cut line -- send a hand crew")
            }
            (UnitKind::HandCrew, Task::Drop { .. })
            | (UnitKind::Engine, Task::Drop { .. }) => {
                return Err("only aircraft drop")
            }
            _ => {}
        }
        unit.task = task;
        unit.line_done_m = 0.0;
        unit.note = "";
        unit.resume = None;
        unit.route.clear();
        unit.planned_at_s = f32::NEG_INFINITY;
        unit.tasked_at_s = self.time_s;
        // An inbound aircraft keeps its state: the order is a briefing, and
        // `arrive_if_due` picks it up the moment it is on station.
        if unit.state != UnitState::Inbound {
            unit.state = match task {
                Task::Hold => UnitState::Staged,
                _ => UnitState::Moving,
            };
        } else {
            unit.note = "tasked on arrival";
        }
        if unit.kind.is_air() {
            unit.air_leg = if unit.water_l > 0.0 { AirLeg::ToTarget } else { AirLeg::ToWater };
        }
        self.generation += 1;
        Ok(())
    }

    /// Call for air support. Returns how many aircraft started inbound.
    ///
    /// The delay is the mechanic: a commander who waits until the fire is in
    /// the houses to ask for aircraft gets them 25 minutes after that.
    pub fn request_air(&mut self) -> usize {
        let now = self.time_s;
        let mut n = 0;
        for u in &mut self.units {
            if u.kind.is_air() && u.state == UnitState::Unavailable {
                u.state = UnitState::Inbound;
                u.arrives_at_s = now + AIR_RESPONSE_S;
                // They come in over the water they will be scooping from.
                if let Some(w) = nearest(&self.open_water, u.base) {
                    u.pos = w;
                }
                n += 1;
            }
        }
        if n > 0 {
            self.generation += 1;
        }
        n
    }

    /// Seconds until the next aircraft is overhead, if any is inbound.
    pub fn air_eta_s(&self) -> Option<f32> {
        self.units
            .iter()
            .filter(|u| u.state == UnitState::Inbound)
            .map(|u| u.arrives_at_s - self.time_s)
            .fold(None, |acc: Option<f32>, v| Some(acc.map_or(v, |a| a.min(v))))
    }

    /// The unit of `kind` nearest `p` that could take a new order, for
    /// click-to-assign.
    pub fn nearest_available(&self, p: Pos, kind: UnitKind) -> Option<usize> {
        self.units
            .iter()
            .filter(|u| u.kind == kind && u.assignable())
            .min_by(|a, b| {
                dist2(a.pos, p)
                    .partial_cmp(&dist2(b.pos, p))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|u| u.id)
    }

    pub fn stats(&self) -> SuppressionStats {
        let mut s = SuppressionStats::default();
        for u in &self.units {
            match u.state {
                UnitState::Unavailable => s.unrequested += 1,
                UnitState::Inbound => s.inbound += 1,
                UnitState::Staged => s.staged += 1,
                UnitState::Moving => s.responding += 1,
                UnitState::Working => s.working += 1,
                UnitState::Refilling => s.refilling += 1,
                UnitState::Withdrawing => s.withdrawing += 1,
                UnitState::Lost => s.lost += 1,
            }
            s.water_l += u.water_used_l as f64;
            s.line_m += u.line_cut_m;
            s.drops += u.drops;
        }
        s
    }

    /// Advance every unit by `dt_s` and return the interventions their work
    /// amounts to, for the caller to hand to [`FireSim::queue`].
    ///
    /// Returned rather than applied because `fire` is borrowed immutably here —
    /// the units are reading the threat field and the fire state to decide what
    /// to do — and because it keeps this model testable without a `FireSim` to
    /// mutate. The caller queues them, and the core applies them on its next
    /// advance, which is one step of latency and is documented at the call site.
    /// `traffic` is the civilian vehicle queue. Units are **not** in it — an
    /// engine on blue lights uses the oncoming lane and traffic pulls over for
    /// it — but they do not get through a solid line of cars at road speed
    /// either, so it is read for a slowdown on whichever link they are on.
    pub fn step(
        &mut self,
        dt_s: f32,
        net: &RoadNetwork,
        traffic: &Traffic,
        fire: &FireSim,
        scn: &Scenario,
    ) -> Vec<Intervention> {
        if dt_s <= 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        // Sub-stepped for the same reason `Abm::move_travellers` is, and it
        // matters more here: a unit's duty cycle is a chain of transitions —
        // arrive, pump dry, drive to a hydrant, fill, drive back — and each one
        // is only noticed at a step boundary. Every rate below is already per
        // second, so the *rates* are step-size invariant on their own; what is
        // not is the time lost rounding six transitions up to a 60 s step,
        // which measured as a 20% difference in water delivered before this
        // loop existed.
        let mut remaining = dt_s;
        while remaining > 0.0 {
            let dt = remaining.min(SUBSTEP_S);
            remaining -= dt;
            self.time_s += dt;

            for i in 0..self.units.len() {
                if self.units[i].state == UnitState::Lost {
                    continue;
                }
                self.arrive_if_due(i);
                if matches!(
                    self.units[i].state,
                    UnitState::Unavailable | UnitState::Inbound
                ) {
                    continue;
                }
                if self.units[i].kind.is_air() {
                    self.step_air(i, dt, fire, scn, &mut out);
                } else {
                    self.step_ground(i, dt, net, traffic, fire, scn, &mut out);
                }
            }
        }

        self.generation += 1;
        out
    }

    // --- the unit's own decision --------------------------------------------

    /// Assemble what unit `i` can know, for an authored policy.
    ///
    /// Everything here is either already to hand or one cheap scan; the only
    /// one worth a note is `work_available`, which for an engine means running
    /// [`Suppression::reachable_targets`] over a 60 m radius — twenty-five cells,
    /// three engines, once a sub-step. That is nothing next to the routing
    /// refresh, and it is the observation that makes "there is nothing left to
    /// wet here" authorable rather than a note in the panel.
    fn observe(&self, i: usize, danger: f32, net: Option<&RoadNetwork>, fire: &FireSim, scn: &Scenario) -> Observation {
        let u = &self.units[i];
        let kind = unit_kind_of(u.kind);

        let distance_to_fire_m = fire
            .active_cells()
            .iter()
            .map(|c| dist2(scn.world.centre_of(*c), u.pos))
            .fold(f32::INFINITY, f32::min)
            .sqrt();

        let focus = u.task.focus();
        let distance_to_task_m = focus.map(|f| dist(u.pos, f)).unwrap_or(1.0e6);

        // "Reachable" means what `nearest_reachable` means: the road network
        // gets there from where this unit is. Air is always reachable, which is
        // the entire point of aircraft.
        let task_reachable = match (focus, net) {
            (None, _) => true,
            (Some(_), _) if u.kind.is_air() => true,
            (Some(f), Some(net)) => {
                let drivable = u.kind.drivable_only();
                net.nearest(u.pos, drivable)
                    .and_then(|a| net.nearest_reachable(f, drivable, a))
                    .is_some()
            }
            // No network to ask, which is only the case in a unit test.
            (Some(_), None) => true,
        };

        let work_available = match u.kind {
            UnitKind::Engine => !self.reachable_targets(u.pos, ENGINE_REACH_M, fire, scn).is_empty(),
            // A crew's work is the line it was given, so "something to do" is
            // "the line is not finished".
            UnitKind::HandCrew => match u.task {
                Task::Line { from, to } => u.line_done_m < dist(from, to),
                Task::Attack { .. } => true,
                _ => false,
            },
            // An aircraft with a load and somewhere to put it.
            UnitKind::AirTanker => u.water_l > 0.0 && matches!(u.task, Task::Drop { .. }),
        };

        UnitObs {
            time_min: self.time_s / 60.0,
            kind,
            threat_here: danger,
            heat_fraction: (u.heat_s / BURNOVER_S).clamp(0.0, 1.0),
            distance_to_fire_m: if distance_to_fire_m.is_finite() {
                distance_to_fire_m
            } else {
                1.0e6
            },
            water_fraction: u.water_frac(),
            carries_water: u.tank_l > 0.0,
            has_task: !matches!(u.task, Task::Hold),
            distance_to_task_m,
            task_reachable,
            line_progress: match u.task {
                Task::Line { from, to } => {
                    let total = dist(from, to);
                    if total > 1.0 {
                        (u.line_done_m / total).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            },
            minutes_on_task: (self.time_s - u.tasked_at_s).max(0.0) / 60.0,
            work_available,
            is_working: u.state == UnitState::Working,
            is_moving: u.state == UnitState::Moving,
            distance_to_base_m: dist(u.pos, u.base),
            jitter: crate::hash01(u.id as u64, 0x9C4D),
        }
        .into()
    }

    /// What unit `i` decides to do about its own situation.
    ///
    /// The graph is the only policy: it receives the unit observation and its
    /// winning action is applied to the fixed movement and work mechanics.
    ///
    /// Only consulted for a unit that is doing something. A unit already
    /// withdrawing or refilling is mid-manoeuvre, and asking again every
    /// sub-step would let a policy restart the manoeuvre forever — the refill
    /// case in particular would reset the task it was going to resume.
    fn unit_outcome(
        &mut self,
        i: usize,
        danger: f32,
        net: Option<&RoadNetwork>,
        fire: &FireSim,
        scn: &Scenario,
    ) -> UnitOutcome {
        if !matches!(
            self.units[i].state,
            UnitState::Staged | UnitState::Moving | UnitState::Working
        ) {
            return UnitOutcome::Carry;
        }
        let policy = self.units[i].policy;
        let obs = self.observe(i, danger, net, fire, scn);
        let d = self.policy.decide(policy, &obs);
        unit_outcome_of(d.action)
    }

    /// Act on an authored outcome. Returns whether the unit's normal duty cycle
    /// should be skipped this sub-step.
    fn apply_outcome(&mut self, i: usize, outcome: UnitOutcome) -> bool {
        match outcome {
            UnitOutcome::Carry => false,
            UnitOutcome::Withdraw => {
                // An aircraft has no retreat to drive: it breaks off the run
                // and orbits. Giving it `Withdrawing` instead would leave it in
                // a state `step_air` never clears, which is a unit that
                // silently stops existing.
                if self.units[i].kind.is_air() {
                    let u = &mut self.units[i];
                    u.state = UnitState::Staged;
                    u.task = Task::Hold;
                    u.note = "broke off: not survivable there";
                    return true;
                }
                // Idempotent: a unit already withdrawing is not sent back to
                // the start of its retreat every sub-step.
                if self.units[i].state != UnitState::Withdrawing {
                    let u = &mut self.units[i];
                    u.state = UnitState::Withdrawing;
                    u.note = "pulled back: not survivable here";
                    u.route.clear();
                    u.planned_at_s = f32::NEG_INFINITY;
                }
                false
            }
            UnitOutcome::Refill => {
                // A hand crew has nothing to fill. Saying so beats silently
                // parking it at a hydrant.
                if self.units[i].tank_l <= 0.0 {
                    self.units[i].note = "a hand crew carries no water";
                    return false;
                }
                if self.units[i].kind.is_air() {
                    self.units[i].air_leg = AirLeg::ToWater;
                    return false;
                }
                let resume = self.units[i].task;
                self.begin_refill(i, resume);
                true
            }
            UnitOutcome::Hold => {
                let u = &mut self.units[i];
                if u.state != UnitState::Staged {
                    u.state = UnitState::Staged;
                    u.task = Task::Hold;
                    u.note = "holding: nothing useful to do here";
                    u.route.clear();
                }
                true
            }
            UnitOutcome::Return => {
                // Same for air: `step_air` only understands `Drop`, and
                // anything else is "orbit at base", which is what returning
                // means for an aircraft.
                if self.units[i].kind.is_air() {
                    let u = &mut self.units[i];
                    u.state = UnitState::Staged;
                    u.task = Task::Hold;
                    u.note = "standing down";
                    return true;
                }
                if !matches!(self.units[i].task, Task::Return) {
                    let u = &mut self.units[i];
                    u.task = Task::Return;
                    u.state = UnitState::Moving;
                    u.note = "returning to staging";
                    u.route.clear();
                    u.planned_at_s = f32::NEG_INFINITY;
                    u.tasked_at_s = self.time_s;
                }
                false
            }
        }
    }

    /// An inbound aircraft becoming available.
    fn arrive_if_due(&mut self, i: usize) {
        let u = &mut self.units[i];
        if u.state == UnitState::Inbound && self.time_s >= u.arrives_at_s {
            // Straight to work if it was briefed on the way in.
            u.state = match u.task {
                Task::Hold => UnitState::Staged,
                _ => UnitState::Moving,
            };
            u.note = if u.state == UnitState::Staged { "on station" } else { "" };
        }
    }

    // --- ground units -------------------------------------------------------

    fn step_ground(
        &mut self,
        i: usize,
        dt: f32,
        net: &RoadNetwork,
        traffic: &Traffic,
        fire: &FireSim,
        scn: &Scenario,
        out: &mut Vec<Intervention>,
    ) {
        // --- physics, before anything else ---------------------------------
        // Burning over is not a decision and is not authorable: a unit standing
        // in flame accumulates exposure and is lost, whatever any policy says.
        let danger = fire.threat().at(self.units[i].pos);
        {
            let u = &mut self.units[i];
            if danger >= fire::threat::IMPASSABLE {
                u.heat_s += dt * danger;
                if u.heat_s >= BURNOVER_S {
                    u.state = UnitState::Lost;
                    u.note = "burnt over";
                    return;
                }
            } else {
                u.heat_s = (u.heat_s - dt * HEAT_RECOVERY).max(0.0);
            }
        }

        // --- the unit's own decision ---------------------------------------
        let outcome = self.unit_outcome(i, danger, Some(net), fire, scn);
        if self.apply_outcome(i, outcome) {
            return;
        }

        match self.units[i].state {
            UnitState::Withdrawing => {
                let base = self.units[i].base;
                self.drive_toward(i, base, dt, net, traffic, fire, scn);
                if dist(self.units[i].pos, base) < ARRIVE_M {
                    let u = &mut self.units[i];
                    u.state = UnitState::Staged;
                    u.task = Task::Hold;
                    u.note = "back at staging, awaiting orders";
                }
            }
            UnitState::Refilling => self.step_refill(i, dt, net, traffic, fire, scn),
            UnitState::Staged => {}
            UnitState::Moving | UnitState::Working => {
                let task = self.units[i].task;
                match task {
                    Task::Hold => self.units[i].state = UnitState::Staged,
                    Task::Return => {
                        let base = self.units[i].base;
                        self.drive_toward(i, base, dt, net, traffic, fire, scn);
                        if dist(self.units[i].pos, base) < ARRIVE_M {
                            let u = &mut self.units[i];
                            u.state = UnitState::Staged;
                            u.task = Task::Hold;
                            u.note = "";
                        }
                    }
                    Task::Attack { at } => match self.units[i].kind {
                        UnitKind::Engine => self.engine_attack(i, at, dt, net, traffic, fire, scn, out),
                        UnitKind::HandCrew => {
                            // Direct attack by a hand crew *is* cutting line at
                            // the fire's edge, so it becomes an alignment across
                            // the front and runs through the same code as an
                            // explicit one. Anything else would be two models
                            // of the same activity.
                            let line = self.crew_alignment(at, fire, scn);
                            self.units[i].task = Task::Line { from: line.0, to: line.1 };
                            self.crew_line(i, line.0, line.1, dt, net, traffic, fire, scn, out);
                        }
                        UnitKind::AirTanker => unreachable!("air handled elsewhere"),
                    },
                    Task::Line { from, to } => {
                        self.crew_line(i, from, to, dt, net, traffic, fire, scn, out)
                    }
                    Task::Drop { .. } => {
                        self.units[i].note = "only aircraft drop";
                        self.units[i].state = UnitState::Staged;
                    }
                }
            }
            UnitState::Unavailable | UnitState::Inbound | UnitState::Lost => {}
        }
    }

    /// An engine works from the road: drive as close to the target as the
    /// drivable network gets, then wet the suppressible fuel within hose reach
    /// of *where it ended up*.
    ///
    /// Not of where it was sent. An engine parked 600 m short of the ordered
    /// point because that is where the tarmac ends is still doing the most
    /// useful thing available to it — wetting the fuel beside the road it is
    /// standing on, which is road-side asset protection and exactly what
    /// engines are for. Refusing to work instead would be tidier and wrong.
    /// [`Unit::note`] tells the player the ordered point was out of reach, so
    /// the shortfall is visible rather than silent.
    fn engine_attack(
        &mut self,
        i: usize,
        at: Pos,
        dt: f32,
        net: &RoadNetwork,
        traffic: &Traffic,
        fire: &FireSim,
        scn: &Scenario,
        out: &mut Vec<Intervention>,
    ) {
        self.drive_toward(i, at, dt, net, traffic, fire, scn);
        let pos = self.units[i].pos;
        if !self.units[i].route.is_empty() {
            self.units[i].state = UnitState::Moving;
            return;
        }
        if self.units[i].water_l <= 0.0 {
            self.begin_refill(i, Task::Attack { at });
            return;
        }

        let short_by = dist(pos, at);
        let cells = self.reachable_targets(pos, ENGINE_REACH_M, fire, scn);
        if cells.is_empty() {
            let u = &mut self.units[i];
            u.state = UnitState::Working;
            u.note = if short_by > ENGINE_REACH_M {
                "no road within hose reach: nothing to work from here"
            } else {
                "nothing left to wet here"
            };
            return;
        }

        let litres = (ENGINE_PUMP_LPM / 60.0 * dt).min(self.units[i].water_l);
        let cell_m2 = scn.world.cellsize * scn.world.cellsize;
        let lpm2 = litres / (cells.len() as f32 * cell_m2);
        {
            let u = &mut self.units[i];
            u.water_l -= litres;
            u.water_used_l += litres;
            u.state = UnitState::Working;
            u.note = if short_by > ENGINE_REACH_M {
                "working the roadside: the ordered point is beyond the hose"
            } else {
                ""
            };
        }
        out.push(Intervention::water(cells, lpm2 as f64));
        if self.units[i].water_l <= 0.0 {
            self.begin_refill(i, Task::Attack { at });
        }
    }

    /// A hand crew walks to the head of its alignment and cuts along it.
    fn crew_line(
        &mut self,
        i: usize,
        from: Pos,
        to: Pos,
        dt: f32,
        net: &RoadNetwork,
        traffic: &Traffic,
        fire: &FireSim,
        scn: &Scenario,
        out: &mut Vec<Intervention>,
    ) {
        let total = dist(from, to);
        if total < 1.0 {
            self.units[i].note = "that line is too short to cut";
            self.units[i].state = UnitState::Staged;
            return;
        }
        // Where along the alignment the crew is working now.
        let head = lerp(from, to, (self.units[i].line_done_m / total).min(1.0));
        if dist(self.units[i].pos, head) > ARRIVE_M {
            self.drive_toward(i, head, dt, net, traffic, fire, scn);
            self.units[i].state = UnitState::Moving;
            return;
        }

        let before = self.units[i].line_done_m;
        let cut = LINE_M_PER_H / 3600.0 * dt;
        let after = (before + cut).min(total);
        {
            let u = &mut self.units[i];
            u.line_done_m = after;
            u.line_cut_m += after - before;
            u.pos = lerp(from, to, after / total);
            u.heading = (to.y - from.y).atan2(to.x - from.x);
            u.state = UnitState::Working;
            u.note = "";
        }

        // Only the newly cut stretch, and only the cells worth cutting. A crew
        // creeping through a cell for twelve minutes emits the same cell
        // repeatedly, which the core merges -- harmless, and cheaper than
        // tracking which cells this crew has already finished.
        let seg = fire::cells_along(
            &scn.world,
            lerp(from, to, before / total),
            lerp(from, to, after / total),
            LINE_WIDTH_M * 0.5,
        );
        let cells = Intervention::useful_cells(
            seg,
            |c| scn.is_burnable(c),
            |c| fire.is_suppressible(c, scn),
        );
        if !cells.is_empty() {
            out.push(Intervention::fireline(cells));
        }

        if after >= total {
            let u = &mut self.units[i];
            u.state = UnitState::Staged;
            u.task = Task::Hold;
            u.note = "line complete";
        }
    }

    /// Pick the alignment a crew ordered to "attack here" should cut: across
    /// the fire's line of advance, on the far side of the point from the front.
    ///
    /// Perpendicular to the direction the nearest burning cell lies in, which
    /// is a local read of where the fire is coming *from* — the same reasoning a
    /// crew boss does by eye, and it needs no wind input to get right.
    fn crew_alignment(&self, at: Pos, fire: &FireSim, scn: &Scenario) -> (Pos, Pos) {
        // Ninety minutes of production, centred on the point: as much line as
        // this crew can plausibly *finish* inside an initial attack. Asking for
        // more would leave a gap at the end, which holds nothing at all.
        let half = LINE_M_PER_H * 1.5 / 2.0;
        let toward_fire = fire
            .active_cells()
            .iter()
            .map(|c| scn.world.centre_of(*c))
            .min_by(|a, b| {
                dist2(*a, at)
                    .partial_cmp(&dist2(*b, at))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            // Degenerate when the ordered point *is* a burning cell, which is
            // the common case rather than an edge one: "attack the head of the
            // fire" puts the click straight onto the front. Without the length
            // guard the alignment collapses to a zero-length line and the crew
            // reports "that line is too short to cut" instead of working.
            .filter(|f| dist2(*f, at) > 1.0)
            .map(|f| {
                let (dx, dy) = (f.x - at.x, f.y - at.y);
                let len = (dx * dx + dy * dy).sqrt();
                (dx / len, dy / len)
            })
            // No fire, or standing in it: cut across the slope, east-west, and
            // let the player place a better alignment by hand.
            .unwrap_or((0.0, 1.0));
        // Perpendicular, so the line lies across the approach.
        let (px, py) = (-toward_fire.1, toward_fire.0);
        (
            Pos { x: at.x - px * half, y: at.y - py * half },
            Pos { x: at.x + px * half, y: at.y + py * half },
        )
    }

    fn begin_refill(&mut self, i: usize, resume: Task) {
        let u = &mut self.units[i];
        u.state = UnitState::Refilling;
        u.resume = Some(resume);
        u.note = "out of water, going for more";
        u.route.clear();
        u.planned_at_s = f32::NEG_INFINITY;
    }

    fn step_refill(
        &mut self,
        i: usize,
        dt: f32,
        net: &RoadNetwork,
        traffic: &Traffic,
        fire: &FireSim,
        scn: &Scenario,
    ) {
        let pos = self.units[i].pos;
        let Some(source) = nearest(&self.hydrants, pos) else {
            let u = &mut self.units[i];
            u.note = "no water source on the map";
            u.state = UnitState::Staged;
            return;
        };
        if dist(pos, source) > ARRIVE_M * 2.0 {
            self.drive_toward(i, source, dt, net, traffic, fire, scn);
            return;
        }
        let u = &mut self.units[i];
        u.water_l = (u.water_l + HYDRANT_LPM / 60.0 * dt).min(u.tank_l);
        if u.water_l >= u.tank_l {
            u.note = "";
            match u.resume.take() {
                Some(task) => {
                    u.task = task;
                    u.state = UnitState::Moving;
                    u.route.clear();
                    u.planned_at_s = f32::NEG_INFINITY;
                }
                None => u.state = UnitState::Staged,
            }
        }
    }

    /// Move a ground unit toward `target` along the network, then across open
    /// ground for the last stretch if its kind can.
    ///
    /// Engines never leave the road: their route ends at the nearest drivable
    /// node and the remaining metres are simply not travelled, which is what
    /// [`ENGINE_REACH_M`] then has to bridge. Crews walk the rest, slowed by
    /// slope exactly as civilians on foot are.
    fn drive_toward(
        &mut self,
        i: usize,
        target: Pos,
        dt: f32,
        net: &RoadNetwork,
        traffic: &Traffic,
        fire: &FireSim,
        scn: &Scenario,
    ) {
        let (kind, pos) = (self.units[i].kind, self.units[i].pos);
        let drivable_only = kind.drivable_only();

        // Re-plan periodically, not every step: a road the fire has cut has to
        // be noticed, but A* on 61 k nodes every 2 s for every unit would not
        // be free.
        let stale = self.time_s - self.units[i].planned_at_s >= REROUTE_S;
        let moved_goal = self.units[i]
            .route_to
            .map_or(true, |old| dist(old, target) > ARRIVE_M);
        if stale || moved_goal {
            self.units[i].route_to = Some(target);
            let from = net.nearest(pos, drivable_only);
            // Reachable, not merely nearest: see `nearest_reachable`.
            let to = from.and_then(|a| net.nearest_reachable(target, drivable_only, a));
            if let (Some(a), Some(b)) = (from, to) {
                match network::route(net, a, b, fire.threat(), drivable_only) {
                    Some(path) => {
                        self.units[i].route = path;
                        self.units[i].note = "";
                    }
                    None => {
                        self.units[i].note = "no road to there that is still open";
                    }
                }
            }
            self.units[i].planned_at_s = self.time_s;
        }

        let mut budget = match kind {
            UnitKind::Engine => ENGINE_SPEED,
            UnitKind::HandCrew => CREW_SPEED,
            UnitKind::AirTanker => TANKER_SPEED,
        } * dt;

        // Civilian traffic on the link ahead. A unit is never *in* the queue —
        // it is not subject to storage, it does not take a place in the line
        // and it cannot be spilled back into — but a jammed street still costs
        // it time, which is the whole reason an engine dispatched into a mass
        // departure arrives late. Sampled on the link it is about to travel,
        // because that is the traffic it is about to be in.
        if !self.units[i].route.is_empty() {
            let next = self.units[i].route[0];
            let from = self.units[i].at_node;
            if let Some(edge) = net.edge_between(from, next) {
                budget *= traffic.emergency_factor(Traffic::link_id(edge, from, next));
            }
        }

        // Follow the network.
        while budget > 0.0 {
            let Some(&next) = self.units[i].route.first() else {
                break;
            };
            let np = net.pos(next);
            let d = dist(self.units[i].pos, np);
            let u = &mut self.units[i];
            if d > budget {
                let f = budget / d.max(1e-3);
                u.heading = (np.y - u.pos.y).atan2(np.x - u.pos.x);
                u.pos.x += (np.x - u.pos.x) * f;
                u.pos.y += (np.y - u.pos.y) * f;
                budget = 0.0;
            } else {
                u.pos = np;
                u.at_node = next;
                u.route.remove(0);
                budget -= d;
            }
        }

        // Off-network walk-in, crews only. Deliberately *not* a loop: closing
        // the last metres by repeatedly taking `on_foot / d` of the remaining
        // gap never quite reaches zero in f32, and the residue is small enough
        // that the loop runs until the heat death of the process. One straight
        // move per sub-step covers at most 4.4 m, which needs no iteration.
        if budget > 0.0 && self.units[i].route.is_empty() && kind == UnitKind::HandCrew {
            let slope = scn.terrain.slope_deg_at(self.units[i].pos);
            let walk = CREW_WALK_SPEED * (1.0 - slope / 45.0).clamp(0.35, 1.0);
            // The remaining budget was spent at network speed; convert it.
            let on_foot = budget / CREW_SPEED * walk;
            let u = &mut self.units[i];
            let d = dist(u.pos, target);
            if d > 0.1 {
                let f = (on_foot / d).min(1.0);
                u.heading = (target.y - u.pos.y).atan2(target.x - u.pos.x);
                u.pos.x += (target.x - u.pos.x) * f;
                u.pos.y += (target.y - u.pos.y) * f;
            }
        }
    }

    // --- air ----------------------------------------------------------------

    /// Air tankers fly straight lines and cycle: target, water, target. The
    /// cycle time is the constraint, not the flying.
    fn step_air(
        &mut self,
        i: usize,
        dt: f32,
        fire: &FireSim,
        scn: &Scenario,
        out: &mut Vec<Intervention>,
    ) {
        // Aircraft consult their graph just as ground units do. The baseline
        // normally answers "carry on", while other policies can author "come
        // home when the fire is out" or "scoop before you are empty".
        let danger = fire.threat().at(self.units[i].pos);
        let outcome = self.unit_outcome(i, danger, None, fire, scn);
        if self.apply_outcome(i, outcome) {
            return;
        }

        let Task::Drop { at } = self.units[i].task else {
            // Nothing to do: orbit at base rather than pretending to work.
            if self.units[i].state != UnitState::Staged {
                self.units[i].state = UnitState::Staged;
            }
            return;
        };

        if self.units[i].air_leg == AirLeg::Scooping {
            let u = &mut self.units[i];
            u.state = UnitState::Refilling;
            u.air_timer_s += dt;
            if u.air_timer_s >= SCOOP_S {
                u.air_timer_s = 0.0;
                u.water_l = u.tank_l;
                u.air_leg = AirLeg::ToTarget;
            }
            return;
        }

        let leg_target = match self.units[i].air_leg {
            AirLeg::ToTarget => at,
            AirLeg::ToWater => nearest(&self.open_water, self.units[i].pos).unwrap_or(at),
            AirLeg::Scooping => unreachable!(),
        };

        // Straight-line flight. No terrain, no network: this is the one agent
        // in the project that is genuinely free of both.
        {
            let u = &mut self.units[i];
            let d = dist(u.pos, leg_target);
            let travel = TANKER_SPEED * dt;
            u.state = if u.air_leg == AirLeg::ToTarget {
                UnitState::Moving
            } else {
                UnitState::Refilling
            };
            if d > travel {
                let f = travel / d.max(1e-3);
                u.heading = (leg_target.y - u.pos.y).atan2(leg_target.x - u.pos.x);
                u.pos.x += (leg_target.x - u.pos.x) * f;
                u.pos.y += (leg_target.y - u.pos.y) * f;
                return;
            }
            u.pos = leg_target;
        }

        // Arrived at the leg's end.
        if self.units[i].air_leg == AirLeg::ToWater {
            self.units[i].air_leg = AirLeg::Scooping;
            self.units[i].note = "scooping";
            return;
        }

        // Over the target with a load: drop it, along the fire's line of
        // advance so the swath lies across the front rather than along it.
        let (a, b) = self.drop_run(at, fire, scn);
        let swath = fire::cells_along(&scn.world, a, b, DROP_WIDTH_M * 0.5);
        let cells = Intervention::useful_cells(
            swath,
            |c| scn.is_burnable(c),
            |c| fire.is_suppressible(c, scn),
        );
        let load = self.units[i].water_l;
        if cells.is_empty() {
            self.units[i].note = "nothing worth dropping on: hold or re-target";
            self.units[i].state = UnitState::Staged;
            return;
        }
        let cell_m2 = scn.world.cellsize * scn.world.cellsize;
        let lpm2 = load / (cells.len() as f32 * cell_m2);
        out.push(Intervention::water(cells, lpm2 as f64));
        let u = &mut self.units[i];
        u.water_used_l += load;
        u.water_l = 0.0;
        u.drops += 1;
        u.note = "";
        u.air_leg = AirLeg::ToWater;
        u.state = UnitState::Working;
    }

    /// The line the drop is laid along: across the direction the fire is
    /// approaching from, same reasoning as [`Suppression::crew_alignment`].
    fn drop_run(&self, at: Pos, fire: &FireSim, scn: &Scenario) -> (Pos, Pos) {
        let half = DROP_LENGTH_M * 0.5;
        let dir = fire
            .active_cells()
            .iter()
            .map(|c| scn.world.centre_of(*c))
            .min_by(|a, b| {
                dist2(*a, at)
                    .partial_cmp(&dist2(*b, at))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            // Same degeneracy guard as `crew_alignment`: a drop is usually
            // called *onto* the front, so the run direction cannot be derived
            // from a zero-length vector.
            .filter(|f| dist2(*f, at) > 1.0)
            .map(|f| {
                let (dx, dy) = (f.x - at.x, f.y - at.y);
                let len = (dx * dx + dy * dy).sqrt();
                (-dy / len, dx / len)
            })
            .unwrap_or((1.0, 0.0));
        (
            Pos { x: at.x - dir.0 * half, y: at.y - dir.1 * half },
            Pos { x: at.x + dir.0 * half, y: at.y + dir.1 * half },
        )
    }

    /// Suppressible cells within `reach` of a working position, nearest first.
    ///
    /// Nearest-first matters: an engine with six minutes of water should spend
    /// it on the fuel closest to the fire it can actually hit, not spread it
    /// evenly over every cell in range.
    fn reachable_targets(
        &self,
        pos: Pos,
        reach: f32,
        fire: &FireSim,
        scn: &Scenario,
    ) -> Vec<Cell> {
        let mut cells: Vec<Cell> = fire::cells_in_radius(&scn.world, pos, reach)
            .into_iter()
            .filter(|c| fire.is_suppressible(*c, scn))
            .collect();
        // Hottest first. The threat field is already a distance-to-flame
        // measure, so sorting on it descending puts the water on the fuel the
        // front is about to reach -- without scanning the whole active front
        // once per candidate cell.
        cells.sort_by_key(|c| {
            let p = scn.world.centre_of(*c);
            -(fire.threat().at(p) * 1000.0) as i32
        });
        // Four cells, 1,600 m². A full 2,500 L tank spread over that is
        // 1.5 L/m², or ~45 points of moisture: past the core's extinction
        // threshold, so an engine can genuinely hold a couple of cells and
        // nothing wider. Spreading the same tank over everything in reach would
        // put it below extinction everywhere and hold nothing.
        cells.truncate(4);
        cells
    }
}

fn nearest(points: &[Pos], from: Pos) -> Option<Pos> {
    points
        .iter()
        .copied()
        .min_by(|a, b| {
            dist2(*a, from)
                .partial_cmp(&dist2(*b, from))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn dist(a: Pos, b: Pos) -> f32 {
    dist2(a, b).sqrt()
}

fn dist2(a: Pos, b: Pos) -> f32 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}

fn lerp(a: Pos, b: Pos, t: f32) -> Pos {
    let t = t.clamp(0.0, 1.0);
    Pos { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t }
}
