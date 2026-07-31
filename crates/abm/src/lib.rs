//! The civilian agent model: who notices the fire, who leaves, and who gets
//! caught.
//!
//! This is wildfire-specific and built from scratch. The structure follows the
//! evacuation literature's own decomposition -- **perception**, **decision**,
//! **preparation**, **movement** -- because that is where the real variance
//! sits. In post-fire studies of Black Saturday, Camp Fire and Mati, what
//! separated survivors from casualties was almost never the fire's behaviour
//! at the property; it was how long people took to decide, and whether the
//! road was still open when they finally moved.
//!
//! The four stages, and what each one is driven by:
//!
//! | Stage | Driven by |
//! |---|---|
//! | Perception | distance to the fire, [`fire::ThreatField`] at the house, structure exposure, and the commander's order arriving over the household's own warning channel |
//! | Decision | `intent` (leave early / wait and see / stay and defend), `risk_perception`, `trust_authority` |
//! | Preparation | `prep_time_min` -- the single biggest lever, and the reason "we told them to go" is not the same as "they went" |
//! | Movement | the real road graph, at car or foot speed, rerouted around fire, with congestion |
//!
//! Three rules the implementation holds to:
//!
//! **Nothing accumulates per call.** Every rate is per simulated second, so
//! stepping at 2 s from the game loop and at 60 s from a headless test give
//! the same outcome. This is the same trap the structure-damage model fell
//! into (see `fire::exposure`), and `step_size_invariance` in the tests pins
//! it.
//!
//! **Randomness is per-agent, not per-call.** Thresholds are jittered from a
//! hash of the agent id, never from a sequential RNG, so an agent's behaviour
//! cannot change because someone else's did or because the step size changed.
//! The RNG is used once, at construction.
//!
//! **Houses do not burn in the CA.** People are on the same non-burnable cells
//! their houses are, so "is this agent in the fire mask" is always false.
//! Danger to an agent comes from [`fire::ThreatField`], which is a proximity
//! field, for exactly the reason documented in `fire::exposure`.

use anyhow::Result;
use fire::FireSim;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use scenario::population::{Intent, Status, WarningChannel};
use scenario::{Pos, Scenario};

pub mod behaviour;
pub mod network;
pub mod refuge;
pub mod suppression;

use network::{NodeId, RoadNetwork, RouteField, NO_NODE};
use refuge::Refuge;

pub use behaviour::{BehaviorRuntime, Outcome, UnitOutcome, UnitRuntime};
pub use suppression::{Suppression, SuppressionStats, Task, Unit, UnitKind, UnitState};

/// How often the household decision layer runs, in simulated seconds. People
/// do not re-evaluate continuously, and this keeps the cost off the movement
/// path.
const DECISION_S: f32 = 5.0;

/// How often the evacuation routing is re-solved against the fire.
const ROUTE_REFRESH_S: f32 = 60.0;

/// Movement is integrated in sub-steps no longer than this, so a 30 s frame
/// step cannot shoot a car straight past a junction.
const MAX_SUBSTEP_S: f32 = 4.0;

/// Seconds of direct flame contact a person survives.
const LETHAL_EXPOSURE_S: f32 = 90.0;

/// How fast accumulated heat load recovers once out of the danger, per second.
const HEAT_RECOVERY: f32 = 0.5;

/// Seconds a household survives sheltering inside a house the front has
/// reached, before defensible space is taken into account. Much longer than
/// [`LETHAL_EXPOSURE_S`] in the open: walls are the reason shelter-in-place is
/// a real option rather than a euphemism.
const SHELTER_SURVIVAL_S: f32 = 900.0;

/// Free-running vehicle speed on the network, m/s (~40 km/h: this is a hill
/// town with hairpins, not a motorway).
const CAR_SPEED: f32 = 11.0;

/// Speed while crossing open ground between the house and the network.
const APPROACH_SPEED: f32 = 1.6;

/// A traveller within this distance of its target node has reached it.
const ARRIVE_M: f32 = 4.0;

/// How much each additional vehicle on the same road link slows everyone on
/// it. Crude, but it reproduces the effect that matters: a late mass
/// departure down one road is slower than the same cars leaving early.
const CONGESTION: f32 = 0.06;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Foot,
    Car,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TravelState {
    /// Crossing open ground to the nearest usable road node.
    Approaching,
    /// Following the route field along the network.
    OnNetwork,
    /// At a refuge, or off the map.
    Safe,
    /// No route out: cut off, sheltering where they stand.
    Cutoff,
    Dead,
}

/// A moving group: one household in its car or on foot, or a single person
/// who was away from home when it started.
#[derive(Debug, Clone)]
pub struct Traveller {
    pub household: usize,
    /// Person ids travelling together.
    pub members: Vec<usize>,
    pub pos: Pos,
    /// Bearing of travel in radians, world frame, for rendering.
    pub heading: f32,
    pub mode: Mode,
    /// Free-running speed for this group, m/s: the slowest member on foot, or
    /// the vehicle speed.
    pub free_speed: f32,
    pub state: TravelState,
    /// Node being driven toward, or [`NO_NODE`] while approaching.
    pub target: NodeId,
    /// Node most recently reached.
    pub at_node: NodeId,
    /// Link currently occupied, for congestion accounting.
    edge: u32,
    /// Accumulated flame exposure, seconds.
    pub heat_s: f32,
    /// Metres travelled, for the debrief.
    pub distance_m: f32,
    /// Simulated time this group started moving.
    pub departed_s: f32,
    /// A single person who was away from home, rather than a household.
    pub solo: bool,
}

/// One person. Individually inspectable, which is the point: the scenario is
/// about people, not about a population count.
#[derive(Debug, Clone)]
pub struct PersonAgent {
    pub id: usize,
    pub household: usize,
    pub age: u8,
    pub walk_speed: f32,
    pub needs_assistance: bool,
    pub pos: Pos,
    pub status: Status,
    /// Traveller this person is riding with, if moving.
    pub traveller: Option<usize>,
    /// Small fixed offset so a group does not render as one point.
    offset: [f32; 2],
}

/// One household: the decision-making unit. People evacuate as families, not
/// as individuals -- which is also why a household with one slow member is
/// slow as a whole.
#[derive(Debug, Clone)]
pub struct HouseholdAgent {
    pub id: usize,
    pub home: Pos,
    pub status: Status,
    /// Perceived threat, 0-1: the agent's own reading of the situation, not
    /// the true danger.
    pub cue: f32,
    /// The commander's order has been issued for this household's area.
    pub ordered: bool,
    /// The order has actually arrived, after the channel's own delay.
    pub warning_received: bool,
    /// Simulated time the household first became aware.
    pub aware_at_s: f32,
    /// Simulated seconds of preparation still to do.
    pub prep_remaining_s: f32,
    pub traveller: Option<usize>,

    pub intent: Intent,
    pub risk_perception: f32,
    pub trust_authority: f32,
    pub channel: WarningChannel,
    pub vehicles: u8,
    pub defensible_space: f32,
    /// Baked trait: minutes of milling before this household actually moves.
    pub prep_time_min: f32,
    /// Seconds spent sheltering in the house with the fire on it.
    pub shelter_s: f32,
    pub members: Vec<usize>,
    /// Time the order was issued to this household, or `f32::INFINITY`.
    ordered_at_s: f32,
}

/// Aggregate readout for the HUD and the debrief.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    pub aware: usize,
    pub preparing: usize,
    pub moving: usize,
    pub safe: usize,
    pub defending: usize,
    pub cutoff: usize,
    pub casualties: usize,
    pub people_safe: usize,
    pub people_moving: usize,
    pub people_at_risk: usize,
    pub cars_moving: usize,
    pub on_foot: usize,
}

pub struct Abm {
    pub households: Vec<HouseholdAgent>,
    pub people: Vec<PersonAgent>,
    pub travellers: Vec<Traveller>,
    pub network: RoadNetwork,
    pub refuges: Vec<Refuge>,
    /// Nearest road node to each house, by mode.
    entry_foot: Vec<NodeId>,
    entry_car: Vec<NodeId>,
    routes_foot: RouteField,
    routes_car: RouteField,
    /// Coarse distance-to-fire field, metres, on a 200 m grid: what a person
    /// can *see*, as opposed to what is close enough to hurt them.
    fire_dist: Vec<f32>,
    fd_cols: usize,
    fd_rows: usize,
    /// Vehicles currently on each link, from the previous step.
    occupancy: Vec<u16>,
    /// Authored behaviour, when the scenario is running one. `None` runs the
    /// hand-written decision layer below, which stays the default: a composer
    /// that silently replaced the shipped model would make every existing
    /// measurement in `crates/fire/tests` un-reproducible.
    behavior: Option<BehaviorRuntime>,
    /// Which subtype each household belongs to. Parallel to `households`, and
    /// empty when no behaviour is loaded.
    subtype: Vec<u16>,
    /// The last decision each household's graph produced, for the inspector.
    /// Parallel to `households`.
    last_decision: Vec<behavior::Decision>,
    time_s: f32,
    next_decision_s: f32,
    next_route_s: f32,
    /// Bumped whenever anything an observer would draw has changed.
    pub generation: u64,
}

/// Coarse grid spacing for the visible-fire field, metres.
const FD_M: f32 = 200.0;
/// Beyond this the fire is not a personal cue, however big it is.
const SEE_RANGE_M: f32 = 2500.0;

impl Abm {
    pub fn new(scn: &Scenario, seed: u64) -> Result<Abm> {
        Abm::with_behavior(scn, seed, None)
    }

    /// Build the agent model, optionally running an authored behaviour.
    ///
    /// The runtime has to arrive here rather than being attached afterwards: a
    /// subtype can set starting traits, and a trait applied after the
    /// households were constructed would be a trait that did not affect the
    /// preparation time drawn from it.
    pub fn with_behavior(
        scn: &Scenario,
        seed: u64,
        behavior: Option<BehaviorRuntime>,
    ) -> Result<Abm> {
        let net = RoadNetwork::build(scn);
        anyhow::ensure!(!net.is_empty(), "road network is empty");
        let refuges = refuge::choose(scn, &net, 12);
        anyhow::ensure!(
            !refuges.is_empty(),
            "no refuge found: every drivable node is in the fuel"
        );

        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0x9E37_79B9);

        let mut households = Vec::with_capacity(scn.population.households.len());
        let mut entry_foot = Vec::with_capacity(households.capacity());
        let mut entry_car = Vec::with_capacity(households.capacity());

        let mut subtype = Vec::with_capacity(households.capacity());
        for h in &scn.population.households {
            let home = Pos { x: h.pos[0], y: h.pos[1] };
            entry_foot.push(net.nearest(home, false).unwrap_or(NO_NODE));
            entry_car.push(net.nearest(home, true).unwrap_or(NO_NODE));

            // A subtype's traits and capabilities replace what the population
            // bake drew; anything the subtype does not mention stays as baked,
            // which is what keeps a profile a small, readable file rather than
            // a full population spec.
            let st = behavior.as_ref().map(|b| b.assign(h.id));
            let force = |key: behaviour::TraitKey, baked: f32| {
                st.and_then(|s| behaviour::trait_override(behavior.as_ref().unwrap(), s, key))
                    .unwrap_or(baked)
            };
            let cap = |key: behaviour::Capability, baked: bool| {
                st.and_then(|s| behaviour::capability_override(behavior.as_ref().unwrap(), s, key))
                    .unwrap_or(baked)
            };
            let vehicles = if cap(behaviour::Capability::Vehicle, h.vehicles > 0) {
                h.vehicles.max(1)
            } else {
                0
            };

            subtype.push(st.unwrap_or(0) as u16);
            households.push(HouseholdAgent {
                id: h.id,
                home,
                status: Status::Normal,
                cue: 0.0,
                ordered: false,
                warning_received: false,
                aware_at_s: f32::INFINITY,
                prep_remaining_s: f32::INFINITY,
                traveller: None,
                intent: h.intent,
                risk_perception: force(behaviour::TraitKey::RiskPerception, h.risk_perception),
                trust_authority: force(behaviour::TraitKey::TrustAuthority, h.trust_authority),
                channel: h.warning_channel,
                vehicles,
                defensible_space: force(
                    behaviour::TraitKey::DefensibleSpace,
                    h.defensible_space,
                ),
                prep_time_min: force(behaviour::TraitKey::PrepTimeMin, h.prep_time_min),
                shelter_s: 0.0,
                members: h.members.clone(),
                ordered_at_s: f32::INFINITY,
            });
        }

        let mut people = Vec::with_capacity(scn.population.people.len());
        for p in &scn.population.people {
            let home = households
                .get(p.household)
                .map(|h| h.home)
                .unwrap_or(Pos { x: 0.0, y: 0.0 });
            // People who were out when it started are somewhere else on the
            // network. They evacuate on their own rather than driving home
            // into it, which is what the guidance says and, unhelpfully, not
            // always what people do -- worth revisiting once the model has a
            // reunification behaviour.
            let pos = if p.at_home {
                home
            } else {
                let a: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
                let r: f32 = rng.gen_range(200.0..1500.0);
                let cand = Pos { x: home.x + r * a.cos(), y: home.y + r * a.sin() };
                let cand = clamp_to_world(cand, scn);
                net.nearest(cand, false).map(|n| net.pos(n)).unwrap_or(home)
            };
            // The assistance capability is a property of a person, but it is
            // the *household* that has a subtype, so it lands here.
            let needs_assistance = behavior
                .as_ref()
                .and_then(|b| {
                    let s = *subtype.get(p.household)? as usize;
                    behaviour::capability_override(b, s, behaviour::Capability::NeedsAssistance)
                })
                .unwrap_or(p.needs_assistance);

            people.push(PersonAgent {
                id: p.id,
                household: p.household,
                age: p.age,
                walk_speed: p.walk_speed,
                needs_assistance,
                pos,
                status: Status::Normal,
                traveller: None,
                offset: [rng.gen_range(-3.0..3.0), rng.gen_range(-3.0..3.0)],
            });
        }

        let n = net.len();
        let fd_cols = (scn.world.width_m / FD_M).ceil() as usize + 1;
        let fd_rows = (scn.world.height_m / FD_M).ceil() as usize + 1;

        let last_decision = vec![behavior::Decision::default(); households.len()];
        let mut abm = Abm {
            households,
            people,
            travellers: Vec::new(),
            network: net,
            refuges,
            entry_foot,
            entry_car,
            routes_foot: RouteField { cost: vec![f32::INFINITY; n], next: vec![NO_NODE; n] },
            routes_car: RouteField { cost: vec![f32::INFINITY; n], next: vec![NO_NODE; n] },
            fire_dist: vec![f32::MAX; fd_rows * fd_cols],
            fd_cols,
            fd_rows,
            occupancy: Vec::new(),
            behavior,
            subtype,
            last_decision,
            time_s: 0.0,
            next_decision_s: 0.0,
            next_route_s: 0.0,
            generation: 0,
        };
        abm.occupancy = vec![0; abm.network.edge_count];

        // People who are away start moving immediately: they are already out,
        // and the model has them make their own way to a refuge on foot.
        let away: Vec<usize> = scn
            .population
            .people
            .iter()
            .filter(|p| !p.at_home)
            .map(|p| p.id)
            .collect();
        for id in away {
            let p = &abm.people[id];
            let (pos, hh, speed) = (p.pos, p.household, p.walk_speed);
            let at = abm.network.nearest(pos, false).unwrap_or(NO_NODE);
            let t = Traveller {
                household: hh,
                members: vec![id],
                pos,
                heading: 0.0,
                mode: Mode::Foot,
                free_speed: speed,
                state: TravelState::Approaching,
                target: at,
                at_node: NO_NODE,
                edge: u32::MAX,
                heat_s: 0.0,
                distance_m: 0.0,
                departed_s: 0.0,
                solo: true,
            };
            abm.travellers.push(t);
            let ti = abm.travellers.len() - 1;
            abm.people[id].traveller = Some(ti);
            abm.people[id].status = Status::Evacuating;
        }

        Ok(abm)
    }

    pub fn time_s(&self) -> f32 {
        self.time_s
    }

    /// Issue an evacuation order to every household within `radius_m` of
    /// `centre`. This is the commander's main lever, and it is deliberately
    /// not instantaneous: each household still hears about it over its own
    /// warning channel, and still has to decide.
    pub fn order_evacuation(&mut self, centre: Pos, radius_m: f32) -> usize {
        let r2 = radius_m * radius_m;
        let mut n = 0;
        let now = self.time_s;
        for h in &mut self.households {
            if h.ordered {
                continue;
            }
            let d2 = (h.home.x - centre.x).powi(2) + (h.home.y - centre.y).powi(2);
            if d2 <= r2 {
                h.ordered = true;
                h.ordered_at_s = now;
                n += 1;
            }
        }
        self.generation += 1;
        n
    }

    /// Order everyone out, whatever the distance.
    pub fn order_evacuation_all(&mut self) -> usize {
        let now = self.time_s;
        let mut n = 0;
        for h in &mut self.households {
            if !h.ordered {
                h.ordered = true;
                h.ordered_at_s = now;
                n += 1;
            }
        }
        self.generation += 1;
        n
    }

    /// Advance the agent model by `dt_s` of simulated time against the current
    /// fire state.
    pub fn step(&mut self, dt_s: f32, fire: &FireSim, scn: &Scenario) {
        if dt_s <= 0.0 {
            return;
        }
        self.time_s += dt_s;

        if self.time_s >= self.next_route_s {
            self.refresh_fire_distance(fire, scn);
            self.routes_foot = network::solve(&self.network, &self.refuge_nodes(), fire.threat(), false);
            self.routes_car = network::solve(&self.network, &self.refuge_nodes(), fire.threat(), true);
            self.next_route_s = self.time_s + ROUTE_REFRESH_S;
        }

        if self.time_s >= self.next_decision_s {
            // Decisions are integrated over the interval that actually
            // elapsed, not over the nominal one, so a coarse caller does not
            // get a slower evacuation than a fine one.
            let elapsed = DECISION_S.max(dt_s);
            self.decide(elapsed, fire, scn);
            self.next_decision_s = self.time_s + DECISION_S;
        }

        let mut remaining = dt_s;
        while remaining > 0.0 {
            let sub = remaining.min(MAX_SUBSTEP_S);
            self.move_travellers(sub, fire, scn);
            remaining -= sub;
        }
        self.sync_people();
        self.generation += 1;
    }

    fn refuge_nodes(&self) -> Vec<NodeId> {
        self.refuges.iter().map(|r| r.node).collect()
    }

    /// Rebuild the coarse "how far away is the fire" field with a two-pass
    /// chamfer transform. 51x51 cells, so this is free next to everything
    /// else, and it is what lets a household react to a fire it can see on the
    /// ridge rather than only to one that is already scorching the wall.
    fn refresh_fire_distance(&mut self, fire: &FireSim, scn: &Scenario) {
        let big = f32::MAX / 4.0;
        for v in &mut self.fire_dist {
            *v = big;
        }
        for c in fire.active_cells() {
            let p = scn.world.centre_of(*c);
            let (gx, gy) = ((p.x / FD_M) as usize, (p.y / FD_M) as usize);
            if gx < self.fd_cols && gy < self.fd_rows {
                self.fire_dist[gy * self.fd_cols + gx] = 0.0;
            }
        }
        let (d1, d2) = (FD_M, FD_M * std::f32::consts::SQRT_2);
        let cols = self.fd_cols;
        for y in 0..self.fd_rows {
            for x in 0..cols {
                let i = y * cols + x;
                let mut v = self.fire_dist[i];
                if x > 0 {
                    v = v.min(self.fire_dist[i - 1] + d1);
                }
                if y > 0 {
                    v = v.min(self.fire_dist[i - cols] + d1);
                    if x > 0 {
                        v = v.min(self.fire_dist[i - cols - 1] + d2);
                    }
                    if x + 1 < cols {
                        v = v.min(self.fire_dist[i - cols + 1] + d2);
                    }
                }
                self.fire_dist[i] = v;
            }
        }
        for y in (0..self.fd_rows).rev() {
            for x in (0..cols).rev() {
                let i = y * cols + x;
                let mut v = self.fire_dist[i];
                if x + 1 < cols {
                    v = v.min(self.fire_dist[i + 1] + d1);
                }
                if y + 1 < self.fd_rows {
                    v = v.min(self.fire_dist[i + cols] + d1);
                    if x + 1 < cols {
                        v = v.min(self.fire_dist[i + cols + 1] + d2);
                    }
                    if x > 0 {
                        v = v.min(self.fire_dist[i + cols - 1] + d2);
                    }
                }
                self.fire_dist[i] = v;
            }
        }
    }

    /// Distance to the nearest burning cell, metres, as an agent would judge
    /// it: coarse, and capped at the range it stops mattering.
    pub fn fire_distance(&self, p: Pos) -> f32 {
        let (gx, gy) = ((p.x / FD_M) as usize, (p.y / FD_M) as usize);
        if gx >= self.fd_cols || gy >= self.fd_rows {
            return f32::MAX;
        }
        self.fire_dist[gy * self.fd_cols + gx]
    }

    /// Assemble what household `i` can know, for an authored behaviour.
    ///
    /// This is the whole read surface a graph gets. Everything in it is
    /// already computed for the hand-written path, so running a behaviour
    /// costs a struct copy rather than any extra model work.
    fn observe(
        &self,
        i: usize,
        jitter: f32,
        danger: f32,
        ex: &fire::exposure::ExposureField,
    ) -> behavior::Observation {
        let h = &self.households[i];
        // The route the household would actually take: their car if they have
        // one and it is reachable, otherwise on foot.
        let (field, entry) = if h.vehicles > 0 && self.entry_car[i] != NO_NODE {
            (&self.routes_car, self.entry_car[i])
        } else {
            (&self.routes_foot, self.entry_foot[i])
        };
        let refuge_distance_m = if entry == NO_NODE {
            f32::INFINITY
        } else {
            field.cost.get(entry as usize).copied().unwrap_or(f32::INFINITY)
        };

        let needs_assistance = h.members.iter().any(|p| self.people[*p].needs_assistance);

        behavior::HouseholdObs {
            time_min: self.time_s / 60.0,
            threat: danger,
            radiant: ex.radiant,
            ember: ex.ember,
            structure_alight: ex.alight,
            fire_distance_m: self.fire_distance(h.home).min(SEE_RANGE_M),
            cue: h.cue,
            order_issued: h.ordered,
            warning_received: h.warning_received,
            minutes_since_order: if h.ordered_at_s.is_finite() {
                (self.time_s - h.ordered_at_s) / 60.0
            } else {
                f32::MAX
            },
            intent: behaviour::intent_of(h.intent),
            risk_perception: h.risk_perception,
            trust_authority: h.trust_authority,
            prep_time_min: h.prep_time_min,
            defensible_space: h.defensible_space,
            household_size: h.members.len() as f32,
            has_vehicle: h.vehicles > 0,
            needs_assistance,
            is_preparing: h.status == Status::Preparing,
            is_moving: h.traveller.is_some(),
            is_defending: h.status == Status::Defending,
            route_blocked: !refuge_distance_m.is_finite(),
            refuge_distance_m,
            jitter,
        }
        .into()
    }

    /// Which subtype a household is running, and the last decision its graph
    /// produced. `None` when no authored behaviour is loaded.
    pub fn behaviour_of(&self, household: usize) -> Option<(&str, &str, behavior::Decision)> {
        let rt = self.behavior.as_ref()?;
        let s = rt.subtype(*self.subtype.get(household)? as usize)?;
        Some((&s.id, &s.name, self.last_decision[household]))
    }

    /// Re-run one household's behaviour with a full trace, for the inspector.
    ///
    /// Deliberately not cached: it is called for the single household the
    /// player has selected, at UI rate, and the answer has to be the one the
    /// current fire state produces rather than the one from whenever the
    /// decision layer last ran.
    pub fn explain(
        &self,
        household: usize,
        fire: &FireSim,
    ) -> Option<(behavior::Decision, behavior::Trace)> {
        let rt = self.behavior.as_ref()?;
        let h = self.households.get(household)?;
        let ex = fire.exposure().get(h.id);
        let danger = fire.threat().at(h.home);
        let jitter = hash01(h.id as u64, 0x51);
        let obs = self.observe(household, jitter, danger, &ex);
        rt.explain(*self.subtype.get(household)? as usize, &obs)
    }

    /// The authored behaviour in force, if any.
    pub fn behavior(&self) -> Option<&BehaviorRuntime> {
        self.behavior.as_ref()
    }

    /// Perception and decision, run every [`DECISION_S`].
    fn decide(&mut self, dt: f32, fire: &FireSim, _scn: &Scenario) {
        let threat = fire.threat();
        let exposure = fire.exposure();
        let now = self.time_s;

        for i in 0..self.households.len() {
            let h = &self.households[i];
            if matches!(h.status, Status::Evacuated | Status::Casualty) {
                continue;
            }

            // --- perception -------------------------------------------------
            let danger = threat.at(h.home);
            let ex = exposure.get(h.id);
            let seen = {
                let d = self.fire_distance(h.home);
                (1.0 - (d / SEE_RANGE_M)).clamp(0.0, 1.0)
            };
            // Attention is not neutral: a risk-aware household reads the same
            // smoke column as a stronger cue than a dismissive one does.
            let attention = 0.6 + 0.8 * h.risk_perception;
            let raw = danger
                .max(ex.radiant + ex.ember)
                .max(0.75 * seen * seen)
                .min(1.0);
            let cue = (raw * attention).min(1.0);

            let h = &mut self.households[i];
            // Cue rises fast and falls slowly: once alarmed, people stay
            // alarmed.
            h.cue = if cue > h.cue { cue } else { h.cue * 0.98 + cue * 0.02 };

            // --- the order arriving over the household's own channel --------
            if h.ordered && !h.warning_received && now - h.ordered_at_s >= channel_delay_s(h.channel)
            {
                h.warning_received = true;
            }

            let jitter = hash01(h.id as u64, 0x51);
            let awake = h.warning_received || h.cue > 0.10 + 0.20 * (1.0 - h.risk_perception) + 0.05 * jitter;

            if awake && h.status == Status::Normal {
                h.status = Status::Warned;
                h.aware_at_s = now;
                // Preparation starts when the household decides to go, not
                // when it is warned -- see the departure test below.
            }

            // --- decision ----------------------------------------------------
            // Either the hand-written rules below, or an authored graph. This
            // is the *only* place the composer takes over: everything above is
            // perception and everything after is consequence.
            let (depart, run_now, prep_scale, defend) = if self.behavior.is_some() {
                let obs = self.observe(i, jitter, danger, &ex);
                let st = self.subtype[i] as usize;
                let d = self.behavior.as_mut().expect("checked").decide(st, &obs);
                self.last_decision[i] = d;
                match behaviour::outcome_of(d.action) {
                    behaviour::Outcome::Hold => (false, false, 1.0, false),
                    behaviour::Outcome::Prepare => (true, false, d.prep_scale, false),
                    behaviour::Outcome::Go => (true, true, d.prep_scale, false),
                    behaviour::Outcome::Defend => (false, false, 1.0, true),
                    // Sheltering is not a departure. The household stays where
                    // it is and the survival model below takes over, which is
                    // the same path a household that never left would take.
                    behaviour::Outcome::Shelter => (false, false, 1.0, false),
                }
            } else {
                let h = &self.households[i];
                let credible_order = h.warning_received && h.trust_authority > 0.35;
                let depart = match h.intent {
                    // Leaves on the first credible signal, order or not.
                    Intent::LeaveEarly => h.status != Status::Normal,
                    // The dangerous majority: needs a direct cue, or an order
                    // it trusts.
                    Intent::WaitAndSee => {
                        credible_order || h.cue > 0.22 + 0.15 * (1.0 - h.risk_perception)
                    }
                    // Stays to defend, and leaves late or not at all -- which
                    // is exactly the population that dies in the road.
                    Intent::StayDefend => h.cue > 0.55 + 0.25 * jitter || ex.alight,
                };
                let scale = match h.intent {
                    Intent::LeaveEarly => 0.8,
                    Intent::WaitAndSee => 1.0,
                    // Already outside with the hoses out: less to pack, but
                    // they left it far too late to matter.
                    Intent::StayDefend => 0.5,
                };
                (depart, false, scale, h.intent == Intent::StayDefend && !depart)
            };

            let h = &mut self.households[i];
            if defend && h.status == Status::Warned {
                h.status = Status::Defending;
            }

            if depart
                && matches!(h.status, Status::Warned | Status::Defending)
                && h.traveller.is_none()
            {
                // "Evacuate now" means the packing is abandoned, not that the
                // household teleports: the minimum in `prep_seconds` still
                // applies, because getting out of a house takes a minute even
                // when you take nothing.
                h.prep_remaining_s = if run_now {
                    60.0
                } else {
                    prep_seconds(h.id, h.prep_time_min, prep_scale)
                };
                h.status = Status::Preparing;
            } else if run_now && h.status == Status::Preparing {
                // A household already milling that the graph has now told to
                // run drops what it is doing.
                h.prep_remaining_s = h.prep_remaining_s.min(60.0);
            }

            if h.status == Status::Preparing {
                h.prep_remaining_s -= dt;
                if h.prep_remaining_s <= 0.0 {
                    self.launch(i);
                }
            }

            // --- caught at home ---------------------------------------------
            let h = &mut self.households[i];
            if h.traveller.is_none() && !matches!(h.status, Status::Evacuated | Status::Casualty)
            {
                if danger >= fire::threat::IMPASSABLE || ex.alight {
                    // The front is on the house and they are still in it.
                    // Sheltering inside is survivable for a while -- it is
                    // sometimes the right call -- but not indefinitely, and
                    // defensible space is what buys the time.
                    h.status = Status::Trapped;
                    h.shelter_s += dt;
                    let survivable = SHELTER_SURVIVAL_S * (0.7 + 0.8 * h.defensible_space);
                    if h.shelter_s > survivable {
                        h.status = Status::Casualty;
                        let members = h.members.clone();
                        for pid in members {
                            self.people[pid].status = Status::Casualty;
                        }
                    } else {
                        let members = h.members.clone();
                        for pid in members {
                            if self.people[pid].traveller.is_none() {
                                self.people[pid].status = Status::Trapped;
                            }
                        }
                    }
                } else {
                    h.shelter_s = (h.shelter_s - dt * HEAT_RECOVERY).max(0.0);
                }
            }
        }
    }

    /// Put a household on the road.
    fn launch(&mut self, i: usize) {
        let (home, vehicles, members) = {
            let h = &self.households[i];
            (h.home, h.vehicles, h.members.clone())
        };
        // Only members who are actually at home leave with the household.
        let riding: Vec<usize> = members
            .iter()
            .copied()
            .filter(|p| self.people[*p].traveller.is_none())
            .filter(|p| {
                !matches!(self.people[*p].status, Status::Evacuated | Status::Casualty)
            })
            .collect();
        if riding.is_empty() {
            self.households[i].status = Status::Evacuated;
            return;
        }

        let car_node = self.entry_car[i];
        // A car is only useful if there is a drivable node near enough to walk
        // to and a route out of it. Otherwise the household walks.
        let car_ok = vehicles > 0
            && car_node != NO_NODE
            && dist(self.network.pos(car_node), home) < 400.0
            && self.routes_car.reachable(car_node);
        let (mode, entry) = if car_ok {
            (Mode::Car, car_node)
        } else {
            (Mode::Foot, self.entry_foot[i])
        };

        // A group moves at the pace of its slowest member, and someone who
        // needs assistance slows the group further.
        let free_speed = match mode {
            Mode::Car => CAR_SPEED,
            Mode::Foot => riding
                .iter()
                .map(|p| {
                    let per = &self.people[*p];
                    if per.needs_assistance {
                        per.walk_speed * 0.55
                    } else {
                        per.walk_speed
                    }
                })
                .fold(f32::MAX, f32::min)
                .min(2.0),
        };

        let t = Traveller {
            household: i,
            members: riding.clone(),
            pos: home,
            heading: 0.0,
            mode,
            free_speed,
            state: TravelState::Approaching,
            target: entry,
            at_node: NO_NODE,
            edge: u32::MAX,
            heat_s: 0.0,
            distance_m: 0.0,
            departed_s: self.time_s,
            solo: false,
        };
        self.travellers.push(t);
        let ti = self.travellers.len() - 1;
        for p in riding {
            self.people[p].traveller = Some(ti);
            self.people[p].status = Status::Evacuating;
        }
        self.households[i].traveller = Some(ti);
        self.households[i].status = Status::Evacuating;
    }

    fn move_travellers(&mut self, dt: f32, fire: &FireSim, scn: &Scenario) {
        let threat = fire.threat();

        // Congestion is read from the previous sub-step's occupancy, then
        // rebuilt: reading and writing the same counters inside the loop would
        // make the result depend on agent order.
        let mut next_occ = vec![0u16; self.network.edge_count];

        for ti in 0..self.travellers.len() {
            let t = &self.travellers[ti];
            if matches!(t.state, TravelState::Safe | TravelState::Dead) {
                continue;
            }

            // --- heat load ---------------------------------------------------
            let danger = threat.at(t.pos);
            let t = &mut self.travellers[ti];
            if danger >= fire::threat::IMPASSABLE {
                t.heat_s += dt * danger;
            } else {
                t.heat_s = (t.heat_s - dt * HEAT_RECOVERY).max(0.0);
            }
            if t.heat_s >= LETHAL_EXPOSURE_S {
                t.state = TravelState::Dead;
                let (hh, members) = (t.household, t.members.clone());
                for p in members {
                    self.people[p].status = Status::Casualty;
                }
                if let Some(h) = self.households.get_mut(hh) {
                    if h.traveller == Some(ti) {
                        h.status = Status::Casualty;
                    }
                }
                continue;
            }

            // --- speed --------------------------------------------------------
            let t = &self.travellers[ti];
            let mut speed = match t.state {
                TravelState::Approaching => t.free_speed.min(APPROACH_SPEED),
                _ => t.free_speed,
            };
            if t.mode == Mode::Foot && t.state != TravelState::OnNetwork {
                // Off the network, slope matters: a 20 degree hillside roughly
                // halves walking speed.
                let slope = scn.terrain.slope_deg_at(t.pos);
                speed *= (1.0 - slope / 45.0).clamp(0.35, 1.0);
            }
            if t.mode == Mode::Car && t.edge != u32::MAX {
                let n = self.occupancy[t.edge as usize] as f32;
                speed *= (1.0 / (1.0 + CONGESTION * (n - 1.0).max(0.0))).max(0.15);
            }
            // Smoke and heat slow everyone down well before they stop them.
            speed *= (1.0 - danger * 0.6).clamp(0.3, 1.0);

            // --- advance -------------------------------------------------------
            let mut budget = speed * dt;
            while budget > 0.0 {
                let t = &self.travellers[ti];
                if t.target == NO_NODE {
                    // No target: either arrived, or nowhere to go.
                    break;
                }
                let target_pos = self.network.pos(t.target);
                let d = dist(t.pos, target_pos);
                if d > budget {
                    let f = budget / d.max(1e-3);
                    let t = &mut self.travellers[ti];
                    t.heading = (target_pos.y - t.pos.y).atan2(target_pos.x - t.pos.x);
                    t.pos.x += (target_pos.x - t.pos.x) * f;
                    t.pos.y += (target_pos.y - t.pos.y) * f;
                    t.distance_m += budget;
                    budget = 0.0;
                } else {
                    let t = &mut self.travellers[ti];
                    t.pos = target_pos;
                    t.at_node = t.target;
                    t.distance_m += d;
                    t.state = TravelState::OnNetwork;
                    budget -= d;
                    self.pick_next_hop(ti);
                    if matches!(
                        self.travellers[ti].state,
                        TravelState::Safe | TravelState::Cutoff
                    ) {
                        break;
                    }
                }
                if d <= ARRIVE_M && budget <= 0.0 {
                    break;
                }
            }

            // OSM ways run a little past the window edge, and the A10 and
            // the Aurelia both leave it. Driving off the map is leaving the
            // incident, so it ends the same way reaching a refuge does.
            if !scn.world.contains(self.travellers[ti].pos) {
                self.mark_safe(ti);
                continue;
            }

            let t = &mut self.travellers[ti];
            if t.mode == Mode::Car && t.edge != u32::MAX {
                if let Some(slot) = next_occ.get_mut(t.edge as usize) {
                    *slot = slot.saturating_add(1);
                }
            }
        }

        self.occupancy = next_occ;
    }

    /// This group is out of the incident: at a refuge, or off the map.
    fn mark_safe(&mut self, ti: usize) {
        let t = &mut self.travellers[ti];
        t.state = TravelState::Safe;
        t.target = NO_NODE;
        t.edge = u32::MAX;
        let (hh, members) = (t.household, t.members.clone());
        for p in members {
            self.people[p].status = Status::Evacuated;
        }
        if let Some(h) = self.households.get_mut(hh) {
            if h.traveller == Some(ti) {
                h.status = Status::Evacuated;
            }
        }
    }

    /// Follow the route field one hop, or conclude there is nowhere to go.
    fn pick_next_hop(&mut self, ti: usize) {
        let (mode, at) = {
            let t = &self.travellers[ti];
            (t.mode, t.at_node)
        };
        if at == NO_NODE {
            return;
        }
        let routes = match mode {
            Mode::Car => &self.routes_car,
            Mode::Foot => &self.routes_foot,
        };

        if routes.cost[at as usize] == 0.0 {
            // Standing on a refuge.
            self.mark_safe(ti);
            return;
        }

        let next = routes.next[at as usize];
        if next == NO_NODE {
            // Cut off. A car with no drivable route abandons the vehicle and
            // continues on foot, which is what people do and what kills them
            // when they do it too late.
            if mode == Mode::Car && self.routes_foot.reachable(at) {
                let t = &mut self.travellers[ti];
                t.mode = Mode::Foot;
                t.free_speed = 1.1;
                t.edge = u32::MAX;
                self.pick_next_hop(ti);
                return;
            }
            let t = &mut self.travellers[ti];
            t.state = TravelState::Cutoff;
            t.target = NO_NODE;
            let members = t.members.clone();
            let hh = t.household;
            for p in members {
                self.people[p].status = Status::Trapped;
            }
            if let Some(h) = self.households.get_mut(hh) {
                if h.traveller == Some(ti) && h.status != Status::Casualty {
                    h.status = Status::Trapped;
                }
            }
            return;
        }

        let edge = self
            .network
            .neighbours(at)
            .iter()
            .find(|e| e.to == next)
            .map(|e| e.id)
            .unwrap_or(u32::MAX);
        let t = &mut self.travellers[ti];
        t.target = next;
        t.edge = edge;
        t.state = TravelState::OnNetwork;
    }

    /// Carry traveller positions onto the individual people.
    fn sync_people(&mut self) {
        for p in &mut self.people {
            if let Some(ti) = p.traveller {
                let t = &self.travellers[ti];
                // In a car everyone is at the vehicle; on foot the group
                // spreads out along the road.
                let (ox, oy) = if t.mode == Mode::Foot {
                    (p.offset[0], p.offset[1])
                } else {
                    (0.0, 0.0)
                };
                p.pos = Pos { x: t.pos.x + ox, y: t.pos.y + oy };
            }
        }
    }

    pub fn stats(&self) -> Stats {
        let mut s = Stats::default();
        for h in &self.households {
            match h.status {
                Status::Normal => {}
                Status::Warned => s.aware += 1,
                Status::Preparing => s.preparing += 1,
                Status::Evacuating => s.moving += 1,
                Status::Evacuated => s.safe += 1,
                Status::Defending => s.defending += 1,
                Status::Trapped => s.cutoff += 1,
                Status::Casualty => s.casualties += 1,
            }
        }
        for p in &self.people {
            match p.status {
                Status::Evacuated => s.people_safe += 1,
                Status::Evacuating => s.people_moving += 1,
                Status::Trapped | Status::Casualty => s.people_at_risk += 1,
                _ => {}
            }
        }
        for t in &self.travellers {
            if matches!(t.state, TravelState::Approaching | TravelState::OnNetwork) {
                match t.mode {
                    Mode::Car => s.cars_moving += 1,
                    Mode::Foot => s.on_foot += 1,
                }
            }
        }
        s
    }

    /// Median seconds from departure to reaching a refuge, over households
    /// that made it. The number a debrief actually wants.
    ///
    /// Households only: the people who were already out when it started have
    /// been walking since T+0 from wherever they happened to be, and mixing
    /// them in measures the map, not the evacuation.
    pub fn median_evacuation_s(&self) -> Option<f32> {
        let mut v: Vec<f32> = self
            .travellers
            .iter()
            .filter(|t| t.state == TravelState::Safe && !t.solo)
            .map(|t| self.time_s - t.departed_s)
            .collect();
        if v.is_empty() {
            return None;
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Some(v[v.len() / 2])
    }
}

fn clamp_to_world(p: Pos, scn: &Scenario) -> Pos {
    Pos {
        x: p.x.clamp(1.0, scn.world.width_m - 1.0),
        y: p.y.clamp(1.0, scn.world.height_m - 1.0),
    }
}

fn dist(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// How long the commander's order takes to reach a household, by the channel
/// it is on. A mobile alert is nearly instant; being told by a neighbour is
/// not; someone with no channel at all only finds out by looking outside.
fn channel_delay_s(c: WarningChannel) -> f32 {
    match c {
        WarningChannel::MobileAlert => 90.0,
        WarningChannel::Siren => 180.0,
        WarningChannel::Neighbour => 420.0,
        WarningChannel::SelfObserved => 600.0,
        WarningChannel::None => 1200.0,
    }
}

/// Preparation time for a household, in seconds.
///
/// The baked `prep_time_min` is the household's own trait -- the milling time
/// that dominates whether an evacuation succeeds -- and `scale` is how its
/// intent modifies it. Jittered per household from its id, never from a
/// call-ordered RNG, so this is stable under any step size.
fn prep_seconds(id: usize, prep_time_min: f32, scale: f32) -> f32 {
    let j = 0.85 + 0.3 * hash01(id as u64, 0x2B);
    (prep_time_min * 60.0 * scale * j).max(60.0)
}

/// Deterministic 0-1 noise from an id and a salt.
pub(crate) fn hash01(id: u64, salt: u64) -> f32 {
    let mut h = id.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h >> 40) as f32 / (1u32 << 24) as f32
}
