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
pub mod comms;
pub mod haven;
pub mod network;
pub mod orders;
pub mod refuge;
pub mod spot;
pub mod suppression;
pub mod traffic;

use comms::CommsNet;
use haven::{Haven, HavenKind};
use network::{NodeId, RoadNetwork, RouteField, NO_NODE};
use orders::{BoatLift, Closure};
use refuge::Refuge;
use spot::SpotFires;
use traffic::{Traffic, NO_LINK};

pub use behaviour::{
    BehaviorRuntime, Outcome, PersonOutcome, PersonRuntime, UnitOutcome, UnitRuntime,
};
pub use suppression::{Suppression, SuppressionStats, Task, Unit, UnitKind, UnitState};

/// How often the household decision layer runs, in simulated seconds. People
/// do not re-evaluate continuously, and this keeps the cost off the movement
/// path.
pub const DECISION_S: f32 = 5.0;

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


/// How long the commander's order takes to reach a party staying in a hotel or
/// a let: through whoever is running the place, over a PA or a knock on every
/// door. Between a mobile alert and a siren, and — the point — it does not go
/// over the mobile network, so [`CommsNet`] taking the mast out does not touch
/// it. A managed population is the one that gets *more* reliable when the
/// infrastructure fails.
const MANAGED_DELAY_S: f32 = 300.0;

/// Fraction of the flame exposure a haven takes off somebody sheltering at it.
///
/// One at the water's edge, because a person in the sea is out of the fire's
/// reach for as long as they can stand it, and that is what the Mati survivor
/// accounts describe. Not one on open ground: a car park in a firestorm is
/// survivable and is not safe, and the difference between those two is the
/// whole reason the two kinds of haven are distinguished at all.
const WATER_RELIEF: f32 = 1.0;
const OPEN_GROUND_RELIEF: f32 = 0.6;

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
    /// Reached a destination that is not safety — the only one being a
    /// household's own home, walked back to by a separated person. The person
    /// is put back into the household and this record retires; it is kept
    /// rather than removed because `travellers` is append-only and every view
    /// indexes into it.
    Arrived,
    /// Stopped at a haven — a clearing, a car park, the water's edge — having
    /// gone there on purpose.
    ///
    /// Deliberately not [`TravelState::Safe`]: nobody at a haven has left the
    /// incident. They are alive, out of the house, off the road and not
    /// counted as evacuated, which is exactly what the shoreline at Mati was.
    /// It is also not terminal — a behaviour can send them on again once the
    /// front has passed, and a boat lift takes them off it.
    Sheltering,
    Dead,
}

/// Where a traveller is trying to get to.
///
/// Almost everyone wants the same thing — the nearest refuge — which is why
/// routing is a field rather than a search (see `network::solve`). The one
/// exception is a person who has decided to go back for their family, and they
/// want somewhere nobody else does, so they carry a path of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goal {
    /// Follow the multi-source route field to the nearest refuge.
    Refuge,
    /// Follow a pre-planned path back to the household's home.
    Home,
    /// Follow the haven field to the nearest survivable open ground. A second
    /// field rather than a per-agent search, for the same reason the refuge
    /// field is one: everybody who wants this wants the *nearest* one.
    Haven,
    /// The same, restricted to the water's edge. A third field, and it only
    /// exists in a window that has a coast — which two of the four shipped real
    /// scenarios do not.
    Shore,
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
    /// Directed link this vehicle is queued on, or [`NO_LINK`] when it is not
    /// in traffic: on foot, crossing open ground, or waiting at a junction for
    /// room on the link it wants next.
    link: u32,
    /// Place in that link's queue, as issued by [`Traffic::enter`].
    ticket: u32,
    /// Simulated time it entered the link. Its position along the link is
    /// derived from this and from how many vehicles are ahead of it, never
    /// accumulated — see [`traffic`] on why that is load-bearing.
    link_entered_s: f32,
    /// Free-flow speed on the current link, m/s, fixed when it entered:
    /// the road class's own speed, less what the smoke and heat there take
    /// off it.
    link_speed: f32,
    /// Vehicles ahead of it on that link, 0 being the one at the stop line.
    /// Recomputed from the queue order each sub-step rather than stored
    /// authoritatively — see [`traffic`].
    queue_rank: u32,
    /// Accumulated flame exposure, seconds.
    pub heat_s: f32,
    /// Metres travelled, for the debrief.
    pub distance_m: f32,
    /// Simulated time this group started moving.
    pub departed_s: f32,
    /// A single person who was away from home, rather than a household.
    pub solo: bool,
    /// What this traveller is trying to reach.
    pub goal: Goal,
    /// Remaining nodes of a [`Goal::Home`] path, nearest first. Empty for
    /// anything following the route field, which is everyone else.
    home_path: Vec<NodeId>,
    /// Fraction of the flame exposure their surroundings take off them, 0 for
    /// anyone in the open and 1 for anyone standing in the sea. Set on arrival
    /// at a haven and zero everywhere else.
    pub shelter_relief: f32,
}

impl Traveller {
    /// The directed link this vehicle is queued on, if it is in traffic at all.
    /// `None` for anyone on foot, for a car still crossing open ground from the
    /// house, and for one waiting at a junction for room.
    pub fn link(&self) -> Option<u32> {
        (self.link != NO_LINK).then_some(self.link)
    }

    /// Vehicles between it and the stop line of its link.
    pub fn queue_rank(&self) -> u32 {
        self.queue_rank
    }
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
    /// Not with their household: out when it started, or separated since.
    ///
    /// The flag that decides whether this person makes decisions of their own.
    /// Everyone else evacuates with the family, and the household is the agent.
    pub away: bool,
    /// This person's own accumulated alarm, 0-1. Only maintained while `away` —
    /// a person at home reads the household's.
    pub cue: f32,
    /// A visitor rather than a resident: inherited from the household, because
    /// a hotel party is transient as a party.
    pub visitor: bool,
    /// Which authored profile this person runs, when one is loaded.
    pub subtype: u16,
    /// The last decision this person's graph produced, for the inspector.
    pub last_decision: behavior::Decision,
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
    /// Nobody here is at home: a hotel party, a let, a campsite. Set by the
    /// [`Capability::Transient`](behavior::subtype::Capability::Transient)
    /// capability on their profile, which is how a transient population is
    /// assigned without re-baking one.
    pub transient: bool,
    pub members: Vec<usize>,
    /// Time the order was issued to this household, or `f32::INFINITY`.
    ordered_at_s: f32,
    /// Time the order actually *arrived* over this household's channel, or
    /// `f32::INFINITY`. Behind `ordered_at_s` by 90 s to 20 minutes, and it is
    /// the clock anything a household does *about* the order has to run on.
    warned_at_s: f32,
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
    /// Groups stopped at a haven: alive, out of the house, off the road, and
    /// not evacuated. Counted off the travellers rather than off a household
    /// status, because "sheltering at the beach" is a *place*, and the status
    /// that describes it is still Evacuating — they have not finished.
    pub sheltering: usize,
    /// Of those, the ones at the water's edge. The only ones a boat lift can
    /// take off.
    pub at_the_shore: usize,
    /// People a boat lift has taken off the beach.
    pub lifted: usize,
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
    /// Somewhere to go that is not a refuge: clearings, car parks, the water's
    /// edge. Much denser than the refuges and reachable on foot only.
    pub havens: Vec<Haven>,
    /// Nearest road node to each house, by mode.
    entry_foot: Vec<NodeId>,
    entry_car: Vec<NodeId>,
    routes_foot: RouteField,
    routes_car: RouteField,
    /// Route fields to the havens and to the water's edge, both on foot.
    routes_haven: RouteField,
    routes_shore: RouteField,
    /// Straight-line metres from each house to the nearest haven and to the
    /// nearest water haven, or infinity. Precomputed: neither moves, and this
    /// is the quantity a household *judges* rather than the one it travels.
    haven_dist: Vec<f32>,
    shore_dist: Vec<f32>,
    /// Which haven each haven node is, for the arrival that sets the relief.
    haven_at: std::collections::HashMap<NodeId, HavenKind>,
    /// Fires that started away from the front, and the warning infrastructure
    /// the fire can take out. Both exist because a correlated failure is not
    /// expressible as a per-agent draw.
    spots: SpotFires,
    comms: CommsNet,
    /// Roads closed to civilian traffic by order, and the per-link mask derived
    /// from them.
    closures: Vec<Closure>,
    closed_edges: Vec<bool>,
    /// The maritime pickup, once somebody asks for one.
    boat_lift: Option<BoatLift>,
    /// Coarse distance-to-fire field, metres, on a 200 m grid: what a person
    /// can *see*, as opposed to what is close enough to hurt them.
    fire_dist: Vec<f32>,
    fd_cols: usize,
    fd_rows: usize,
    /// Vehicle queues on the road graph: capacity, storage and spillback.
    /// Cars are the only travellers in it — pedestrians do not queue, and
    /// suppression units read it for a slowdown without joining it.
    pub traffic: Traffic,
    /// The editable household decision layer. Every household has one: there is
    /// no second, hand-written policy for a run to fall back to.
    behavior: BehaviorRuntime,
    /// The editable decision layer for people away from their household.
    person_behavior: PersonRuntime,
    /// Which subtype each household belongs to. Parallel to `households`.
    subtype: Vec<u16>,
    /// The last decision each household's graph produced, for the inspector.
    /// Parallel to `households`.
    last_decision: Vec<behavior::Decision>,
    /// Simulated time the first evacuation order was issued to anyone, or
    /// infinity. What a person out in the street hears, as opposed to what
    /// arrives at their own front door over their household's channel.
    order_at_s: f32,
    time_s: f32,
    next_decision_s: f32,
    /// When the decision layer last ran, so the interval it integrates over is
    /// the one that actually elapsed rather than the one that was intended.
    last_decision_s: f32,
    next_route_s: f32,
    /// Bumped whenever anything an observer would draw has changed.
    pub generation: u64,
}

/// Coarse grid spacing for the visible-fire field, metres.
const FD_M: f32 = 200.0;
/// Beyond this the fire is not a personal cue, however big it is.
const SEE_RANGE_M: f32 = 2500.0;

impl Abm {
    /// Build on the shipped graph library.
    ///
    /// This is primarily the model-test convenience constructor. The game loads
    /// the editable library from disk and passes its compiled runtimes through
    /// [`Abm::with_behaviours`]. Both paths execute graphs.
    pub fn new(scn: &Scenario, seed: u64) -> Result<Abm> {
        let lib = behavior::defaults::default_library();
        let household = BehaviorRuntime::build(&lib)
            .map_err(|e| anyhow::anyhow!(e))?
            .ok_or_else(|| anyhow::anyhow!("no active household behaviour profiles"))?;
        let person = PersonRuntime::build(&lib)
            .map_err(|e| anyhow::anyhow!(e))?
            .ok_or_else(|| anyhow::anyhow!("no active separated-person behaviour profiles"))?;
        Abm::with_behaviours(scn, seed, household, person)
    }

    /// Build the agent model with one household runtime and the shipped person
    /// graph. Kept as a convenience for focused household-behaviour tests.
    ///
    /// The runtime has to arrive here rather than being attached afterwards: a
    /// subtype can set starting traits, and a trait applied after the
    /// households were constructed would be a trait that did not affect the
    /// preparation time drawn from it.
    pub fn with_behavior(
        scn: &Scenario,
        seed: u64,
        behavior: BehaviorRuntime,
    ) -> Result<Abm> {
        let lib = behavior::defaults::default_library();
        let person = PersonRuntime::build(&lib)
            .map_err(|e| anyhow::anyhow!(e))?
            .ok_or_else(|| anyhow::anyhow!("no active separated-person behaviour profiles"))?;
        Abm::with_behaviours(scn, seed, behavior, person)
    }

    /// Build the agent model with both civilian runtimes.
    ///
    /// Two, because households and separated people are different agents in
    /// different domains: a scenario can run an authored household behaviour
    /// with the shipped person one, or the other way round, and the composer
    /// applies whichever the library actually assigns.
    pub fn with_behaviours(
        scn: &Scenario,
        seed: u64,
        behavior: BehaviorRuntime,
        person_behavior: PersonRuntime,
    ) -> Result<Abm> {
        let net = RoadNetwork::build(scn);
        anyhow::ensure!(!net.is_empty(), "road network is empty");
        let refuges = refuge::choose(scn, &net, 12);
        anyhow::ensure!(
            !refuges.is_empty(),
            "no refuge found: every drivable node is in the fuel"
        );
        // Havens are allowed to be empty. A window with nowhere open in it is a
        // finding about that window, not a broken scenario, and every behaviour
        // that offers one is gated on the distance being finite.
        let havens = haven::choose(scn, &net, haven::MAX_HAVENS);

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
            let st = behavior.assign(h.id);
            let force = |key: behaviour::TraitKey, baked: f32| {
                behaviour::trait_override(&behavior, st, key)
                    .unwrap_or(baked)
            };
            let cap = |key: behaviour::Capability, baked: bool| {
                behaviour::capability_override(&behavior, st, key)
                    .unwrap_or(baked)
            };
            let vehicles = if cap(behaviour::Capability::Vehicle, h.vehicles > 0) {
                h.vehicles.max(1)
            } else {
                0
            };

            let transient = cap(behaviour::Capability::Transient, false);
            subtype.push(st as u16);
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
                transient,
                members: h.members.clone(),
                ordered_at_s: f32::INFINITY,
                warned_at_s: f32::INFINITY,
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
            let needs_assistance = subtype
                .get(p.household)
                .and_then(|s| {
                    behaviour::capability_override(
                        &behavior,
                        *s as usize,
                        behaviour::Capability::NeedsAssistance,
                    )
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
                away: !p.at_home,
                cue: 0.0,
                visitor: households.get(p.household).map(|h| h.transient).unwrap_or(false),
                subtype: person_behavior.assign(p.id) as u16,
                last_decision: behavior::Decision::default(),
                offset: [rng.gen_range(-3.0..3.0), rng.gen_range(-3.0..3.0)],
            });
        }

        let n = net.len();
        let fd_cols = (scn.world.width_m / FD_M).ceil() as usize + 1;
        let fd_rows = (scn.world.height_m / FD_M).ceil() as usize + 1;

        let last_decision = vec![behavior::Decision::default(); households.len()];

        // Straight-line, and the same choice `PersonObs::home_distance_m`
        // documents: this number exists to be *compared* with how far away
        // safety looks, and that is what somebody standing at the door judges.
        // Whether the network gets them there is the route field's answer.
        let nearest = |p: Pos, water_only: bool| {
            havens
                .iter()
                .filter(|h| !water_only || h.is_water())
                .map(|h| dist(p, h.pos))
                .fold(f32::INFINITY, f32::min)
        };
        let haven_dist: Vec<f32> = households.iter().map(|h| nearest(h.home, false)).collect();
        let shore_dist: Vec<f32> = households.iter().map(|h| nearest(h.home, true)).collect();
        let haven_at = havens.iter().map(|h| (h.node, h.kind)).collect();
        let homes: Vec<Pos> = households.iter().map(|h| h.home).collect();
        let comms = CommsNet::build(&net, &homes);
        let traffic = Traffic::new(&net);

        let mut abm = Abm {
            households,
            people,
            travellers: Vec::new(),
            comms,
            spots: SpotFires::new(scn),
            network: net,
            refuges,
            havens,
            haven_dist,
            shore_dist,
            haven_at,
            closures: Vec::new(),
            closed_edges: Vec::new(),
            boat_lift: None,
            entry_foot,
            entry_car,
            routes_foot: RouteField { cost: vec![f32::INFINITY; n], next: vec![NO_NODE; n] },
            routes_car: RouteField { cost: vec![f32::INFINITY; n], next: vec![NO_NODE; n] },
            routes_haven: RouteField { cost: vec![f32::INFINITY; n], next: vec![NO_NODE; n] },
            routes_shore: RouteField { cost: vec![f32::INFINITY; n], next: vec![NO_NODE; n] },
            fire_dist: vec![f32::MAX; fd_rows * fd_cols],
            fd_cols,
            fd_rows,
            traffic,
            behavior,
            person_behavior,
            subtype,
            last_decision,
            order_at_s: f32::INFINITY,
            time_s: 0.0,
            next_decision_s: 0.0,
            last_decision_s: 0.0,
            next_route_s: 0.0,
            generation: 0,
        };
        abm.closed_edges = vec![false; abm.network.edge_count];

        // People who are away start moving immediately: they are already out,
        // and the model has them make their own way to a refuge on foot.
        //
        // This stays here, at T+0, rather than falling out of the person
        // decision layer's first tick. Moving it would shift every one of these
        // departures by one decision interval and quietly invalidate every
        // evacuation figure measured before the layer existed — and it is what
        // the shipped person behaviour transcribes anyway.
        let away: Vec<usize> = abm.people.iter().filter(|p| p.away).map(|p| p.id).collect();
        for id in away {
            abm.walk_out(id);
        }

        Ok(abm)
    }

    /// Put one separated person on the road toward the nearest refuge.
    ///
    /// Idempotent for someone already walking out, which matters: the shipped
    /// person behaviour proposes this on every tick, and re-launching them each
    /// time would reset their progress.
    fn walk_out(&mut self, id: usize) {
        if let Some(ti) = self.people[id].traveller {
            let t = &mut self.travellers[ti];
            // Already on their way out: nothing to do. `Cutoff` is deliberately
            // not in that set — someone who stopped because the road was gone
            // has to be able to set off again when the routing says it is back,
            // and treating "has a traveller heading for a refuge" as "is
            // walking" would strand them for the rest of the incident.
            let walking = matches!(
                t.state,
                TravelState::Approaching | TravelState::OnNetwork
            );
            if t.goal == Goal::Refuge && walking {
                return;
            }
            if matches!(t.state, TravelState::Safe | TravelState::Dead) {
                return;
            }
            // Turning round: back onto the route field from wherever they are.
            let nearest_walk =
                self.network.nearest(self.travellers[ti].pos, false).unwrap_or(NO_NODE);
            self.detach(ti);
            let t = &mut self.travellers[ti];
            t.goal = Goal::Refuge;
            t.home_path.clear();
            t.state = TravelState::Approaching;
            t.target = nearest_walk;
            self.people[id].status = Status::Evacuating;
            return;
        }
        if matches!(self.people[id].status, Status::Evacuated | Status::Casualty) {
            return;
        }
        let p = &self.people[id];
        // Their bare walking speed, with none of the adjustments `launch`
        // applies to a household group: no assistance penalty and no 2 m/s cap.
        // That is what the model has always done with a solo walker and it is
        // left alone here, because changing it would move every evacuation
        // figure taken before this layer existed. Worth revisiting on purpose.
        let (pos, hh, speed) = (p.pos, p.household, p.walk_speed);
        let at = self.network.nearest(pos, false).unwrap_or(NO_NODE);
        self.travellers.push(Traveller {
            household: hh,
            members: vec![id],
            pos,
            heading: 0.0,
            mode: Mode::Foot,
            free_speed: speed,
            state: TravelState::Approaching,
            target: at,
            at_node: NO_NODE,
            link: NO_LINK,
            ticket: 0,
            link_entered_s: 0.0,
            link_speed: 0.0,
            queue_rank: 0,
            heat_s: 0.0,
            distance_m: 0.0,
            departed_s: self.time_s,
            solo: true,
            goal: Goal::Refuge,
            home_path: Vec::new(),
            shelter_relief: 0.0,
        });
        let ti = self.travellers.len() - 1;
        self.people[id].traveller = Some(ti);
        self.people[id].status = Status::Evacuating;
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
        if n > 0 {
            self.order_at_s = self.order_at_s.min(now);
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
        if n > 0 {
            self.order_at_s = self.order_at_s.min(now);
        }
        self.generation += 1;
        n
    }

    // -----------------------------------------------------------------------
    // The commander's other two levers
    // -----------------------------------------------------------------------

    /// Close every drivable link within `radius_m` of `centre` to civilian
    /// traffic for `duration_s`.
    ///
    /// Returns the number of links closed, which is zero for a point with no
    /// road near it — and saying so is the point, because a closure that
    /// silently covered nothing looks exactly like one that worked.
    ///
    /// This is the lever investigators found missing at Pedrógão Grande, and it
    /// is not free: it takes a road away from everyone who was going to use it,
    /// and the households it strands are visible in `block.road_closed`. Units
    /// and pedestrians are unaffected — a barricade stops civilian traffic, not
    /// the engine that asked for it.
    pub fn close_road(&mut self, centre: Pos, radius_m: f32, duration_s: f32) -> usize {
        let c = Closure {
            centre,
            radius_m,
            from_s: self.time_s,
            until_s: if duration_s.is_finite() {
                self.time_s + duration_s
            } else {
                f32::INFINITY
            },
        };
        self.closures.push(c);
        self.refresh_closures();
        self.generation += 1;
        self.closed_edges.iter().filter(|c| **c).count()
    }

    /// Lift every closure. What the commander does when the fire has gone past.
    pub fn reopen_roads(&mut self) {
        self.closures.clear();
        self.refresh_closures();
        self.generation += 1;
    }

    pub fn closures(&self) -> &[Closure] {
        &self.closures
    }

    /// Rebuild the per-link closed mask from the closures still in force.
    ///
    /// Recomputed from the closure list rather than toggled per link, so a
    /// closure expiring cannot leave a link closed that another closure was
    /// also covering — the bug that shape of bookkeeping always eventually has.
    fn refresh_closures(&mut self) {
        let now = self.time_s;
        self.closures.retain(|c| now < c.until_s);
        for slot in &mut self.closed_edges {
            *slot = false;
        }
        if self.closures.is_empty() {
            return;
        }
        for n in 0..self.network.len() as NodeId {
            let a = self.network.pos(n);
            for e in self.network.neighbours(n) {
                if !e.drivable {
                    continue;
                }
                let b = self.network.pos(e.to);
                let mid = Pos { x: (a.x + b.x) * 0.5, y: (a.y + b.y) * 0.5 };
                if self.closures.iter().any(|c| c.active_at(now) && c.contains(mid)) {
                    if let Some(slot) = self.closed_edges.get_mut(e.id as usize) {
                        *slot = true;
                    }
                }
            }
        }
    }

    /// Ask for a maritime pickup at the shore.
    ///
    /// Fails when the window has no water haven in it, which is the honest
    /// answer for an inland scenario and is a sentence in the panel rather than
    /// a silent no-op — the same rule every suppression refusal follows.
    /// Requesting a second one while the first is on the way replaces it.
    pub fn request_boat_lift(
        &mut self,
        delay_s: f32,
        rate_per_min: f32,
    ) -> Result<(), &'static str> {
        if !self.havens.iter().any(|h| h.is_water()) {
            return Err("there is no shore in this window to lift anyone off");
        }
        self.boat_lift = Some(BoatLift::requested(self.time_s, delay_s, rate_per_min));
        self.generation += 1;
        Ok(())
    }

    pub fn boat_lift(&self) -> Option<&BoatLift> {
        self.boat_lift.as_ref()
    }

    /// Minutes until the pickup is on station, or a very large number when none
    /// has been asked for. The quantity the behaviours read.
    fn boat_lift_min(&self) -> f32 {
        self.boat_lift.map(|b| b.minutes_out(self.time_s)).unwrap_or(1.0e6)
    }

    /// Take people off the beach, oldest arrival first.
    ///
    /// Integrated over simulated time by [`BoatLift::capacity`], so how many
    /// leave in an hour does not depend on how often the caller steps —
    /// finding 5, which the first version of the structure damage model got
    /// wrong in exactly this way.
    fn run_boat_lift(&mut self, dt_s: f32) {
        let Some(mut lift) = self.boat_lift else { return };
        let mut budget = lift.capacity(self.time_s, dt_s);
        if budget > 0 {
            for ti in 0..self.travellers.len() {
                if budget == 0 {
                    break;
                }
                let t = &self.travellers[ti];
                if t.state != TravelState::Sheltering || t.shelter_relief < WATER_RELIEF {
                    continue;
                }
                let n = t.members.len();
                if n > budget {
                    // A group boards together or waits: splitting a family
                    // across two boats is a different model and not a better
                    // one.
                    continue;
                }
                budget -= n;
                lift.lifted += n;
                self.mark_safe(ti);
            }
        }
        self.boat_lift = Some(lift);
    }

    /// Spot fires seen so far: fires that started away from the mapped front.
    pub fn spot_fires(&self) -> &SpotFires {
        &self.spots
    }

    /// The warning infrastructure and what the fire has done to it.
    pub fn comms(&self) -> &CommsNet {
        &self.comms
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
            self.spots.update(fire, scn, self.time_s);
            self.comms.update(fire, self.time_s);
            self.refresh_closures();
            let refuges = self.refuge_nodes();
            let threat = fire.threat();
            self.routes_foot = network::solve(&self.network, &refuges, threat, false);
            self.routes_car =
                network::solve_with(&self.network, &refuges, threat, true, &self.closed_edges);
            // Two more fields, on foot, and only where there is anything to
            // solve for: a landlocked window has no water havens, and a shore
            // field over no sources is 61 k nodes of infinity nobody reads.
            let havens: Vec<NodeId> = self.havens.iter().map(|h| h.node).collect();
            let shore: Vec<NodeId> =
                self.havens.iter().filter(|h| h.is_water()).map(|h| h.node).collect();
            if !havens.is_empty() {
                self.routes_haven = network::solve(&self.network, &havens, threat, false);
            }
            if !shore.is_empty() {
                self.routes_shore = network::solve(&self.network, &shore, threat, false);
            }
            self.next_route_s = self.time_s + ROUTE_REFRESH_S;
        }

        self.run_boat_lift(dt_s);

        if self.time_s >= self.next_decision_s {
            // Decisions are integrated over the interval that actually
            // elapsed, not over the nominal one, so neither a coarse caller nor
            // a fine one gets a different evacuation from the same model.
            //
            // This was `DECISION_S.max(dt_s)`, which is right for a caller
            // coarser than `DECISION_S` and silently wrong for a finer one: at
            // a 2 s step the decision fires every *6* s — the first step at or
            // past the deadline — and charged 5, so preparation ran at 83% of
            // real time; at 4 s it fired every 8 and charged 5, and ran at 63%.
            // The game steps at ~2 s and every batch measurement in
            // `crates/fire/tests` uses 10 s or more, so the shipped game was
            // preparing households slower than the numbers taken on it, and
            // nothing anywhere reported a discrepancy. Measuring the interval
            // rather than assuming it is the whole fix.
            let elapsed = (self.time_s - self.last_decision_s).max(dt_s);
            self.last_decision_s = self.time_s;
            self.decide(elapsed, fire, scn);
            self.decide_people(elapsed, fire);
            self.next_decision_s = self.time_s + DECISION_S;
        }

        // Sub-steps carry the absolute clock, not just their length: the
        // traffic queue times a link's discharge against simulated time so that
        // the same number of vehicles gets through however the caller chops the
        // interval up.
        let mut remaining = dt_s;
        while remaining > 0.0 {
            let sub = remaining.min(MAX_SUBSTEP_S);
            remaining -= sub;
            self.move_travellers(sub, self.time_s - remaining, fire, scn);
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

    /// Assemble what household `i` can know for its behavior graph.
    ///
    /// This is the whole read surface a graph gets. Everything in it is
    /// already computed by perception and routing, so evaluating a graph costs
    /// a struct copy rather than extra spatial model work.
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
        let (spot_m, spot_age) = self.spots.nearest(h.home, self.time_s);
        // What the barricade looks like from the front door: their own way onto
        // the drivable network is inside a closed area. Deliberately not "the
        // route field got longer", which is also true and is indistinguishable
        // from the fire having done it.
        let road_closed = self.entry_car[i] != NO_NODE
            && !self.closures.is_empty()
            && self
                .closures
                .iter()
                .any(|c| c.active_at(self.time_s) && c.contains(self.network.pos(self.entry_car[i])));

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
            minutes_since_warning: if h.warned_at_s.is_finite() {
                (self.time_s - h.warned_at_s) / 60.0
            } else {
                f32::MAX
            },
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
            spot_fire_distance_m: spot_m.min(SEE_RANGE_M),
            spot_fire_age_min: spot_age,
            road_closed,
            // A mast being down is not a fact about one household's channel:
            // everybody under it knows there is no signal, whether or not they
            // were the ones waiting for a message.
            comms_down: !self.comms.covered(h.home),
            is_visitor: h.transient,
            open_ground_distance_m: self.haven_dist.get(i).copied().unwrap_or(f32::INFINITY),
            shore_distance_m: self.shore_dist.get(i).copied().unwrap_or(f32::INFINITY),
            boat_lift_min: self.boat_lift_min(),
            jitter,
        }
        .into()
    }

    /// Straight-line metres from `p` to the nearest haven, optionally only the
    /// ones at the water's edge. Infinite when there are none.
    fn haven_distance(&self, p: Pos, water_only: bool) -> f32 {
        self.havens
            .iter()
            .filter(|h| !water_only || h.is_water())
            .map(|h| dist(p, h.pos))
            .fold(f32::INFINITY, f32::min)
    }

    /// Which subtype a household is running, and the last decision its graph
    /// produced.
    pub fn behaviour_of(&self, household: usize) -> Option<(&str, &str, behavior::Decision)> {
        let s = self.behavior.subtype(*self.subtype.get(household)? as usize)?;
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
        let h = self.households.get(household)?;
        let ex = fire.exposure().get(h.id);
        let danger = fire.threat().at(h.home);
        let jitter = hash01(h.id as u64, 0x51);
        let obs = self.observe(household, jitter, danger, &ex);
        self.behavior.explain(*self.subtype.get(household)? as usize, &obs)
    }

    /// The editable household behaviour in force.
    pub fn behavior(&self) -> &BehaviorRuntime {
        &self.behavior
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

            // Whether the network that was going to warn them is still up. A
            // shared piece of infrastructure, so this is *correlated* across
            // every household under the same mast — which is what the reporting
            // on Pedrógão Grande describes and what no per-household channel
            // draw can produce.
            let signal = self.comms.covered(h.home);

            let h = &mut self.households[i];
            // Cue rises fast and falls slowly: once alarmed, people stay
            // alarmed.
            h.cue = if cue > h.cue { cue } else { h.cue * 0.98 + cue * 0.02 };

            // --- the order arriving over the household's own channel --------
            if h.ordered && !h.warning_received && now - h.ordered_at_s >= warning_delay_s(h, signal)
            {
                h.warning_received = true;
                h.warned_at_s = now;
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
            // The graph owns the decision. Everything above is perception and
            // everything after is consequence; routing, movement and survival
            // remain fixed simulator mechanics.
            let obs = self.observe(i, jitter, danger, &ex);
            let st = self.subtype[i] as usize;
            let d = self.behavior.decide(st, &obs);
            self.last_decision[i] = d;
            let outcome = behaviour::outcome_of(d.action);
            let (depart, run_now, prep_scale, defend) = match outcome {
                behaviour::Outcome::Hold => (false, false, 1.0, false),
                behaviour::Outcome::Prepare => (true, false, d.prep_scale, false),
                behaviour::Outcome::Go => (true, true, d.prep_scale, false),
                behaviour::Outcome::Defend => (false, false, 1.0, true),
                // Sheltering is not a departure. The household stays where it
                // is and the survival model below takes over.
                behaviour::Outcome::Shelter => (false, false, 1.0, false),
                // Nor are these: a household making for open ground leaves the
                // house on foot immediately and has no preparation to do — the
                // fire is on them, which is the only condition under which the
                // block that proposes this fires at all.
                behaviour::Outcome::OpenGround | behaviour::Outcome::Shore => {
                    (false, false, 1.0, false)
                }
            };
            match outcome {
                behaviour::Outcome::OpenGround => self.flee_to_haven(i, Goal::Haven),
                behaviour::Outcome::Shore => self.flee_to_haven(i, Goal::Shore),
                _ => {}
            }

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
                    self.launch(i, Goal::Refuge);
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

    // -----------------------------------------------------------------------
    // Separated people
    // -----------------------------------------------------------------------

    /// Assemble what one person away from their household can know.
    ///
    /// The whole read surface a person graph gets, and much narrower than the
    /// household's: this agent is out in the street with no more information
    /// than their own eyes give them.
    fn observe_person(&self, id: usize, danger: f32, home_alight: bool) -> behavior::Observation {
        let p = &self.people[id];
        let home = self.households.get(p.household).map(|h| h.home).unwrap_or(p.pos);
        let hh = self.households.get(p.household);

        let travelling = p.traveller.map(|ti| &self.travellers[ti]);
        let heat_fraction =
            travelling.map(|t| (t.heat_s / LETHAL_EXPOSURE_S).min(1.0)).unwrap_or(0.0);
        let is_moving = travelling
            .map(|t| matches!(t.state, TravelState::Approaching | TravelState::OnNetwork))
            .unwrap_or(false);
        let is_heading_home = travelling.map(|t| t.goal == Goal::Home).unwrap_or(false);
        let is_sheltering = travelling.map(|t| t.state == TravelState::Cutoff).unwrap_or(false)
            || p.status == Status::Trapped;

        // Straight-line to both, so the two are on one scale and "is home
        // nearer than safety" means what it says. Whether the road survives is
        // `route_blocked`, which does come off the network.
        let refuge_distance_m = self
            .refuges
            .iter()
            .map(|r| dist(p.pos, r.pos))
            .fold(f32::INFINITY, f32::min);
        let entry = self.network.nearest(p.pos, false).unwrap_or(NO_NODE);
        let route_blocked = entry == NO_NODE || !self.routes_foot.reachable(entry);
        let (spot_m, spot_age) = self.spots.nearest(p.pos, self.time_s);

        behavior::PersonObs {
            time_min: self.time_s / 60.0,
            threat: danger,
            heat_fraction,
            fire_distance_m: self.fire_distance(p.pos).min(SEE_RANGE_M),
            cue: p.cue,
            order_issued: self.order_at_s.is_finite(),
            minutes_since_order: if self.order_at_s.is_finite() {
                (self.time_s - self.order_at_s) / 60.0
            } else {
                f32::MAX
            },
            age: p.age as f32,
            walk_speed: p.walk_speed,
            needs_assistance: p.needs_assistance,
            home_distance_m: dist(p.pos, home),
            household_at_home: hh
                .map(|h| {
                    h.traveller.is_none()
                        && !matches!(h.status, Status::Evacuated | Status::Casualty)
                })
                .unwrap_or(false),
            household_safe: hh.map(|h| h.status == Status::Evacuated).unwrap_or(false),
            home_alight,
            refuge_distance_m,
            route_blocked,
            is_moving,
            is_heading_home,
            is_sheltering,
            spot_fire_distance_m: spot_m.min(SEE_RANGE_M),
            spot_fire_age_min: spot_age,
            is_visitor: p.visitor,
            open_ground_distance_m: self.haven_distance(p.pos, false),
            shore_distance_m: self.haven_distance(p.pos, true),
            boat_lift_min: self.boat_lift_min(),
            jitter: hash01(p.id as u64, 0xA37),
        }
        .into()
    }

    /// Which profile a separated person is running, and the last decision their
    /// graph produced.
    pub fn person_behaviour_of(&self, person: usize) -> Option<(&str, &str, behavior::Decision)> {
        let p = self.people.get(person)?;
        let s = self.person_behavior.subtype(p.subtype as usize)?;
        Some((&s.id, &s.name, p.last_decision))
    }

    /// Re-run one person's behaviour with a full trace, for the inspector.
    ///
    /// Not cached, for the same reason [`Abm::explain`] is not: the answer has
    /// to be the one the current fire produces, not the one from whenever the
    /// decision layer last ran.
    pub fn explain_person(
        &self,
        person: usize,
        fire: &FireSim,
    ) -> Option<(behavior::Decision, behavior::Trace)> {
        let p = self.people.get(person)?;
        let danger = fire.threat().at(p.pos);
        let alight = fire.exposure().get(p.household).alight;
        let obs = self.observe_person(person, danger, alight);
        self.person_behavior.explain(p.subtype as usize, &obs)
    }

    /// The editable separated-person behaviour in force.
    pub fn person_behavior(&self) -> &PersonRuntime {
        &self.person_behavior
    }

    /// Perception and decision for everyone who is not with their household.
    ///
    /// Only people who are actually away evaluate it; people at home are governed
    /// by their household's graph.
    fn decide_people(&mut self, dt: f32, fire: &FireSim) {
        let threat = fire.threat();
        let exposure = fire.exposure();

        for i in 0..self.people.len() {
            let p = &self.people[i];
            if !p.away || matches!(p.status, Status::Evacuated | Status::Casualty) {
                continue;
            }
            let (pos, household) = (p.pos, p.household);

            // --- perception -------------------------------------------------
            // The same shape as the household's, read from where the person is
            // standing rather than from the house, and without the attention
            // term: that is a household trait and a person out in the street
            // does not carry one.
            let danger = threat.at(pos);
            let seen = {
                let d = self.fire_distance(pos);
                (1.0 - (d / SEE_RANGE_M)).clamp(0.0, 1.0)
            };
            let raw = danger.max(0.75 * seen * seen).min(1.0);
            let p = &mut self.people[i];
            p.cue = if raw > p.cue { raw } else { p.cue * 0.98 + raw * 0.02 };

            // --- decision ---------------------------------------------------
            let alight = exposure.get(household).alight;
            let obs = self.observe_person(i, danger, alight);
            let st = self.people[i].subtype as usize;
            let d = self.person_behavior.decide(st, &obs);
            self.people[i].last_decision = d;

            match behaviour::person_outcome_of(d.action) {
                behaviour::PersonOutcome::Carry => {}
                behaviour::PersonOutcome::WalkOut => self.walk_out(i),
                behaviour::PersonOutcome::Shelter => self.shelter_in_place(i),
                behaviour::PersonOutcome::HeadHome => self.head_home(i, fire),
                behaviour::PersonOutcome::OpenGround => self.walk_to_haven(i, Goal::Haven),
                behaviour::PersonOutcome::Shore => self.walk_to_haven(i, Goal::Shore),
            }
        }
        // Every branch above is a change of destination, never a change of
        // pace: `dt` is the model's, not the graph's, which is what keeps an
        // authored person behaviour step-size invariant.
        let _ = dt;
    }

    /// Stop this person where they are and let them take what cover there is.
    fn shelter_in_place(&mut self, id: usize) {
        let Some(ti) = self.people[id].traveller else {
            self.people[id].status = Status::Trapped;
            return;
        };
        if matches!(
            self.travellers[ti].state,
            TravelState::Safe | TravelState::Dead | TravelState::Arrived
        ) {
            return;
        }
        self.detach(ti);
        let t = &mut self.travellers[ti];
        t.state = TravelState::Cutoff;
        t.target = NO_NODE;
        t.home_path.clear();
        self.people[id].status = Status::Trapped;
    }

    /// Send this person to the nearest haven, or to the water's edge.
    ///
    /// Idempotent for someone already going there, for the same reason
    /// [`Abm::walk_out`] is: the block proposes it on every tick while the
    /// condition holds. Someone with no reachable haven keeps doing whatever
    /// they were doing rather than being stopped — an unreachable destination
    /// is not a reason to stand still, and the shelter branch is what says
    /// otherwise.
    fn walk_to_haven(&mut self, id: usize, goal: Goal) {
        let reachable = |net: &RoadNetwork, field: &RouteField, p: Pos| {
            net.nearest(p, false).map(|n| field.reachable(n)).unwrap_or(false)
        };
        let field = match goal {
            Goal::Shore => &self.routes_shore,
            _ => &self.routes_haven,
        };
        if !reachable(&self.network, field, self.people[id].pos) {
            return;
        }
        if matches!(self.people[id].status, Status::Evacuated | Status::Casualty) {
            return;
        }
        // They may not be walking at all yet, so put them on the road first and
        // then redirect: `walk_out` is the one place a traveller is created for
        // a person, and having two would be two places to keep in step.
        if self.people[id].traveller.is_none() {
            self.walk_out(id);
        }
        let Some(ti) = self.people[id].traveller else { return };
        let t = &self.travellers[ti];
        if t.goal == goal
            || matches!(t.state, TravelState::Safe | TravelState::Dead | TravelState::Arrived)
        {
            return;
        }
        let here = self.travellers[ti].pos;
        let entry = self.network.nearest(here, false).unwrap_or(NO_NODE);
        self.detach(ti);
        let t = &mut self.travellers[ti];
        t.goal = goal;
        t.home_path.clear();
        t.state = TravelState::Approaching;
        t.shelter_relief = 0.0;
        t.target = entry;
        self.people[id].status = Status::Evacuating;
    }

    /// Turn this person round and walk them back to the household's home.
    ///
    /// The one place the civilian model runs a per-agent search rather than
    /// reading the route field, and it is the case the field cannot answer:
    /// every other civilian wants the nearest refuge and this one wants a
    /// specific house. Planned once, on the tick they decide, and not
    /// re-planned — an A* per person per tick is the mistake the suppression
    /// layer already made once.
    fn head_home(&mut self, id: usize, fire: &FireSim) {
        if self.people[id].traveller.map(|ti| self.travellers[ti].goal) == Some(Goal::Home) {
            return;
        }
        let home = match self.households.get(self.people[id].household) {
            Some(h) => h.home,
            None => return,
        };
        let from = self.network.nearest(self.people[id].pos, false).unwrap_or(NO_NODE);
        let to = self.network.nearest(home, false).unwrap_or(NO_NODE);
        let Some(mut path) = network::route(&self.network, from, to, fire.threat(), false) else {
            // No way back through the fire. They keep walking out, which is the
            // answer the model would have given anyway.
            self.walk_out(id);
            return;
        };
        path.push(to);

        if self.people[id].traveller.is_none() {
            self.walk_out(id);
        }
        let Some(ti) = self.people[id].traveller else { return };
        self.detach(ti);
        let t = &mut self.travellers[ti];
        t.goal = Goal::Home;
        t.state = TravelState::Approaching;
        t.target = from;
        t.home_path = path;
        self.people[id].status = Status::Evacuating;
    }

    /// This person has walked back to the house and is part of the household
    /// again — which is what makes reunification a behaviour rather than a
    /// detour: the family's own decision layer now covers them.
    fn arrive_home(&mut self, ti: usize) {
        self.detach(ti);
        let (members, hh) = {
            let t = &mut self.travellers[ti];
            t.state = TravelState::Arrived;
            t.target = NO_NODE;
            t.home_path.clear();
            (std::mem::take(&mut t.members), t.household)
        };
        let home = self.households.get(hh).map(|h| h.home);
        for p in members {
            let person = &mut self.people[p];
            person.traveller = None;
            person.away = false;
            person.status = Status::Normal;
            if let Some(home) = home {
                person.pos = home;
            }
        }
        // A household that had already left is not there to be rejoined, and a
        // person who walks into an empty house is on their own again — which is
        // the outcome the reunification profile exists to make visible.
        if let Some(h) = self.households.get(hh) {
            if matches!(h.status, Status::Evacuated | Status::Casualty) || h.traveller.is_some() {
                for p in &self.households[hh].members.clone() {
                    if self.people[*p].away || self.people[*p].traveller.is_some() {
                        continue;
                    }
                    self.people[*p].away = true;
                }
            }
        }
    }

    /// Put a household on the road toward `goal`.
    fn launch(&mut self, i: usize, goal: Goal) {
        let (home, vehicles, members) = {
            let h = &self.households[i];
            (h.home, h.vehicles, h.members.clone())
        };
        // Only members who are actually at home leave with the household.
        // `away` is the explicit half of that: someone out in the street has no
        // traveller of their own only in the moment between arriving home and
        // being folded back into the family, and they should not be swept up
        // from the far side of town.
        let riding: Vec<usize> = members
            .iter()
            .copied()
            .filter(|p| self.people[*p].traveller.is_none() && !self.people[*p].away)
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
        //
        // A haven is on foot whatever they own. These are the places two
        // streets away that the road does not go to, chosen for exactly that,
        // and a household that has decided the road is not the answer does not
        // then take it.
        let car_ok = goal == Goal::Refuge
            && vehicles > 0
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
            link: NO_LINK,
            ticket: 0,
            link_entered_s: 0.0,
            link_speed: 0.0,
            queue_rank: 0,
            heat_s: 0.0,
            distance_m: 0.0,
            departed_s: self.time_s,
            solo: false,
            goal,
            home_path: Vec::new(),
            shelter_relief: 0.0,
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

    /// Send a household out of the house on foot for a haven, or turn one that
    /// is already moving toward one.
    ///
    /// Idempotent, because the graph proposes this on every tick while the
    /// condition holds and re-launching them each time would reset their
    /// progress — the same property [`Abm::walk_out`] has and for the same
    /// reason. A household with no reachable haven is left exactly as it was:
    /// the block that proposed this is gated on the distance, but distance is
    /// not reachability, and inventing a destination for them would be worse
    /// than the shelter-in-place branch they fall back on.
    fn flee_to_haven(&mut self, i: usize, goal: Goal) {
        let field = match goal {
            Goal::Shore => &self.routes_shore,
            _ => &self.routes_haven,
        };
        let entry = self.entry_foot[i];
        if entry == NO_NODE || !field.reachable(entry) {
            return;
        }
        if matches!(self.households[i].status, Status::Evacuated | Status::Casualty) {
            return;
        }
        match self.households[i].traveller {
            None => {
                self.launch(i, goal);
                // `launch` retires a household with nobody left to ride, and
                // that is not a state to overwrite.
                if let Some(ti) = self.households[i].traveller {
                    self.travellers[ti].goal = goal;
                }
            }
            Some(ti) => {
                let t = &self.travellers[ti];
                if t.goal == goal
                    || matches!(
                        t.state,
                        TravelState::Safe | TravelState::Dead | TravelState::Arrived
                    )
                {
                    return;
                }
                // Turning round mid-route: back onto the network on foot from
                // wherever they are. A car is abandoned, which is what happens
                // and is the same thing `pick_next_hop` does to a driver whose
                // road is gone.
                let here = self.travellers[ti].pos;
                let entry = self.network.nearest(here, false).unwrap_or(NO_NODE);
                self.detach(ti);
                let t = &mut self.travellers[ti];
                t.goal = goal;
                t.mode = Mode::Foot;
                t.free_speed = t.free_speed.min(1.3);
                t.home_path.clear();
                t.state = TravelState::Approaching;
                t.shelter_relief = 0.0;
                t.target = entry;
            }
        }
    }

    /// This group has reached a haven and stops there.
    fn arrive_haven(&mut self, ti: usize, node: NodeId) {
        let water = self.haven_at.get(&node).copied() == Some(HavenKind::Water);
        self.detach(ti);
        let t = &mut self.travellers[ti];
        t.state = TravelState::Sheltering;
        t.target = NO_NODE;
        t.shelter_relief = if water { WATER_RELIEF } else { OPEN_GROUND_RELIEF };
        let members = t.members.clone();
        for p in members {
            // Not `Evacuated`: nobody at a haven has left the incident, and a
            // model that counted them as safe would say Mati was a success.
            self.people[p].status = Status::Evacuating;
        }
    }

    /// Move everybody one sub-step. `now` is the simulated time at the *end* of
    /// it, which the traffic queue needs as an absolute clock rather than as an
    /// interval — see [`traffic`] on why nothing there is accumulated.
    fn move_travellers(&mut self, dt: f32, now: f32, fire: &FireSim, scn: &Scenario) {
        let threat = fire.threat();

        // --- heat load, for everybody still travelling ------------------------
        for ti in 0..self.travellers.len() {
            let t = &self.travellers[ti];
            if matches!(t.state, TravelState::Safe | TravelState::Dead | TravelState::Arrived) {
                continue;
            }
            // A haven takes some of it off: all of it at the water's edge,
            // where a person can get into the sea, and rather less in a car
            // park. Nothing else in the model gives relief from flame contact,
            // and this is why the two kinds of haven are distinguished at all.
            let danger = threat.at(t.pos) * (1.0 - t.shelter_relief).clamp(0.0, 1.0);
            let t = &mut self.travellers[ti];
            if danger >= fire::threat::IMPASSABLE {
                t.heat_s += dt * danger;
            } else {
                t.heat_s = (t.heat_s - dt * HEAT_RECOVERY).max(0.0);
            }
            if t.heat_s < LETHAL_EXPOSURE_S {
                continue;
            }
            // Burnt over. A vehicle that dies in a queue has to come out of it,
            // and it can be anywhere in the line rather than at the front —
            // which is exactly the removal the queue's rank bookkeeping is
            // built to survive.
            self.detach(ti);
            let t = &mut self.travellers[ti];
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
        }

        // --- free movement ----------------------------------------------------
        // Everyone on foot, plus any vehicle still crossing open ground between
        // the house and the road. A vehicle that has reached the network is
        // handed to the queue and does not appear here again.
        for ti in 0..self.travellers.len() {
            let t = &self.travellers[ti];
            if matches!(t.state, TravelState::Safe | TravelState::Dead | TravelState::Arrived) {
                continue;
            }
            if t.mode == Mode::Car && t.state == TravelState::OnNetwork {
                continue;
            }

            let danger = threat.at(t.pos) * (1.0 - t.shelter_relief).clamp(0.0, 1.0);
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
            // Smoke and heat slow everyone down well before they stop them.
            speed *= (1.0 - danger * 0.6).clamp(0.3, 1.0);

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
                    let t = &self.travellers[ti];
                    if matches!(
                        t.state,
                        TravelState::Safe | TravelState::Cutoff | TravelState::Arrived
                    ) {
                        break;
                    }
                    // A car that has just reached the road joins the traffic
                    // queue, and its remaining budget is not its to spend: how
                    // far it gets now depends on what is in front of it.
                    if t.mode == Mode::Car {
                        break;
                    }
                }
                if d <= ARRIVE_M && budget <= 0.0 {
                    break;
                }
            }
        }

        // --- vehicles ---------------------------------------------------------
        self.advance_vehicles(dt, now, &threat);

        // --- leaving the window -----------------------------------------------
        // OSM ways run a little past the window edge, and the A10 and the
        // Aurelia both leave it. Driving off the map is leaving the incident, so
        // it ends the same way reaching a refuge does — but only for someone who
        // was heading out. A person walking back to a house inside the window
        // has not left anything.
        for ti in 0..self.travellers.len() {
            let t = &self.travellers[ti];
            if matches!(t.state, TravelState::Safe | TravelState::Dead | TravelState::Arrived) {
                continue;
            }
            if t.goal == Goal::Refuge && !scn.world.contains(t.pos) {
                self.mark_safe(ti);
            }
        }
    }

    /// One sub-step of the traffic queue.
    ///
    /// Vehicles are visited in `(link, ticket)` order — the front of each
    /// queue first — so that a link drains head-first and the car behind sees
    /// the space the one in front freed within the same sub-step. Rank on a
    /// link falls out of that walk rather than being stored, which is what makes
    /// a vehicle leaving from the middle of a queue harmless.
    fn advance_vehicles(&mut self, dt: f32, now: f32, threat: &fire::threat::ThreatField) {
        // (link, ticket, traveller). `NO_LINK` is `u32::MAX` and so sorts last,
        // which is what we want: a vehicle waiting at a junction should be
        // offered the room that the vehicles already moving have just given up.
        let mut order: Vec<(u32, u32, u32)> = Vec::new();
        for (ti, t) in self.travellers.iter().enumerate() {
            if t.mode == Mode::Car && t.state == TravelState::OnNetwork {
                order.push((t.link, t.ticket, ti as u32));
            }
        }
        if order.is_empty() {
            return;
        }
        order.sort_unstable();

        let mut k = 0;
        while k < order.len() {
            let link = order[k].0;
            // Rank within this link's run. Only vehicles that are *still* on the
            // link when we leave them consume a place, so one that gets away
            // promotes the whole queue behind it on this same pass.
            let mut rank = 0u32;
            while k < order.len() && order[k].0 == link {
                let ti = order[k].2 as usize;
                self.travellers[ti].queue_rank = rank;
                self.advance_vehicle(ti, dt, now, rank == 0, threat);
                if self.travellers[ti].link == link {
                    rank += 1;
                }
                k += 1;
            }
        }

        for &(_, _, ti) in &order {
            self.place_vehicle(ti as usize, now);
        }
    }

    /// Carry one vehicle as far along its route as this sub-step allows.
    ///
    /// Bounded rather than open-ended: on Spotorno the median link is 8 m, so a
    /// car at road speed legitimately crosses a dozen of them in a 4 s sub-step,
    /// and a handful of degenerate 0.1 m OSM segments could otherwise spend the
    /// whole budget one hop at a time.
    fn advance_vehicle(
        &mut self,
        ti: usize,
        dt: f32,
        now: f32,
        mut at_head: bool,
        threat: &fire::threat::ThreatField,
    ) {
        const MAX_LINK_HOPS: usize = 96;
        for _ in 0..MAX_LINK_HOPS {
            let t = &self.travellers[ti];
            if t.mode != Mode::Car || t.state != TravelState::OnNetwork {
                return;
            }
            if t.link == NO_LINK {
                // Waiting at a junction — or newly arrived from the driveway —
                // for room on the link it has chosen.
                if !self.try_enter_link(ti, now, threat) {
                    return;
                }
                at_head = true;
                continue;
            }
            if !at_head {
                return;
            }

            let (link, entered_s, speed) = {
                let t = &self.travellers[ti];
                (t.link, t.link_entered_s, t.link_speed)
            };
            let len = self.network.edge_len(Traffic::edge_of(link));
            // Three gates, and it takes all three. The vehicle has to have
            // physically covered the link, the link has to be due to discharge,
            // and the link it wants next has to have somewhere to put it.
            let arrive_s = entered_s + len / speed.max(0.1);
            if now < arrive_s {
                return;
            }
            let gate = self.traffic.release_gate(link);
            if now < gate {
                return;
            }
            if let Some(next) = self.peek_next_link(ti) {
                if !self.traffic.has_room(next) {
                    // Spillback. This is the clause the old per-car multiplier
                    // had no way to express, and the reason a jam here has a
                    // length rather than just a speed.
                    return;
                }
            }

            // Committed. The release time is the moment it *could* go, not the
            // end of the sub-step, so the remainder carries onto the next link
            // and a car does not lose the rest of its budget at every junction.
            let t_rel = arrive_s.max(gate).max(now - dt);
            self.traffic.took_slot(link, t_rel);
            self.traffic.leave(link);

            let node = {
                let t = &mut self.travellers[ti];
                t.link = NO_LINK;
                t.at_node = t.target;
                t.at_node
            };
            if node == NO_NODE {
                return;
            }
            let np = self.network.pos(node);
            {
                let t = &mut self.travellers[ti];
                t.distance_m += dist(t.pos, np);
                t.pos = np;
            }
            self.pick_next_hop(ti);
            if !self.try_enter_link(ti, t_rel, threat) {
                return;
            }
            at_head = true;
        }
    }

    /// The link this vehicle would take on out of the node it is arriving at,
    /// or `None` if it would leave the network there — reaching a refuge,
    /// driving off the map, being cut off, or abandoning the car for foot.
    ///
    /// A deliberate partial echo of [`Abm::pick_next_hop`], because spillback
    /// has to be decided *before* the vehicle gives up its place on the link it
    /// is on: committing first and discovering there is no room afterwards
    /// would leave it parked in a junction with no queue to be in, and unbounded
    /// junction storage is precisely how a spillback model stops spilling back.
    fn peek_next_link(&self, ti: usize) -> Option<u32> {
        let t = &self.travellers[ti];
        let at = t.target;
        if at == NO_NODE {
            return None;
        }
        let next = if t.goal == Goal::Home {
            *t.home_path.iter().find(|&&n| n != at)?
        } else {
            let routes = match (t.goal, t.mode) {
                (Goal::Haven, _) => &self.routes_haven,
                (Goal::Shore, _) => &self.routes_shore,
                (_, Mode::Car) => &self.routes_car,
                (_, Mode::Foot) => &self.routes_foot,
            };
            if routes.cost[at as usize] == 0.0 {
                return None;
            }
            let n = routes.next[at as usize];
            if n == NO_NODE {
                return None;
            }
            n
        };
        let edge = self.network.edge_between(at, next)?;
        Some(Traffic::link_id(edge, at, next))
    }

    /// Put a vehicle onto the link from `at_node` to `target`, if that link has
    /// room. `false` means it waits where it is, which is a driveway that cannot
    /// get out or a junction that is blocked — both real, and both previously
    /// impossible.
    fn try_enter_link(&mut self, ti: usize, at_s: f32, threat: &fire::threat::ThreatField) -> bool {
        let t = &self.travellers[ti];
        if t.mode != Mode::Car || t.state != TravelState::OnNetwork || t.link != NO_LINK {
            return false;
        }
        let (at, to) = (t.at_node, t.target);
        if at == NO_NODE || to == NO_NODE {
            return false;
        }
        let Some(edge) = self.network.edge_between(at, to) else {
            return false;
        };
        let link = Traffic::link_id(edge, at, to);
        if !self.traffic.has_room(link) {
            return false;
        }
        let (ticket, count) = self.traffic.enter(link);
        // Smoke and heat cost a driver speed well before they stop them.
        // Sampled once, as the vehicle enters, rather than integrated along the
        // link: a speed that varied along the link could not be inverted back
        // into a position without accumulating one, and the median link here is
        // 8 m of road.
        let danger = threat.at(self.network.pos(at));
        let speed = (self.traffic.speed(link) * (1.0 - danger * 0.6).clamp(0.3, 1.0)).max(0.5);
        let t = &mut self.travellers[ti];
        t.link = link;
        t.ticket = ticket;
        t.link_entered_s = at_s;
        t.link_speed = speed;
        t.queue_rank = count.saturating_sub(1) as u32;
        // Shown in the inspector, and the honest answer to "how fast is this
        // family moving" is the speed of the road they are on.
        t.free_speed = speed;
        true
    }

    /// Place a vehicle on its link: as far along as free flow would have taken
    /// it, or the back of the queue ahead of it, whichever is nearer.
    ///
    /// Both terms are pure functions of the clock, so this is exact at any step
    /// size and a car that is stopped is stopped *somewhere specific* — the tail
    /// of a queue is a place on the map, which is what makes a jam visible.
    fn place_vehicle(&mut self, ti: usize, now: f32) {
        let t = &self.travellers[ti];
        let link = t.link;
        if link == NO_LINK {
            return;
        }
        let (a, b) = self.network.link_ends(link);
        let (pa, pb) = (self.network.pos(a), self.network.pos(b));
        let len = self.network.edge_len(Traffic::edge_of(link));
        let x_free = (now - t.link_entered_s).max(0.0) * t.link_speed;
        let x_slot = len - t.queue_rank as f32 * self.traffic.spacing(link);
        let f = (x_free.min(x_slot).clamp(0.0, len)) / len.max(1e-3);
        let np = Pos { x: pa.x + (pb.x - pa.x) * f, y: pa.y + (pb.y - pa.y) * f };
        let t = &mut self.travellers[ti];
        t.distance_m += dist(t.pos, np);
        t.heading = (pb.y - pa.y).atan2(pb.x - pa.x);
        t.pos = np;
    }

    /// Take a vehicle out of whatever queue it is in. Safe from any position in
    /// the line, and idempotent.
    fn detach(&mut self, ti: usize) {
        let link = self.travellers[ti].link;
        if link != NO_LINK {
            self.travellers[ti].link = NO_LINK;
            self.traffic.leave(link);
        }
    }

    /// This group is out of the incident: at a refuge, or off the map.
    fn mark_safe(&mut self, ti: usize) {
        self.detach(ti);
        let t = &mut self.travellers[ti];
        t.state = TravelState::Safe;
        t.target = NO_NODE;
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
        let (mode, at, goal) = {
            let t = &self.travellers[ti];
            (t.mode, t.at_node, t.goal)
        };
        if at == NO_NODE {
            return;
        }
        if goal == Goal::Home {
            // A planned path rather than a field: walk it off the front, and
            // when it runs out they are standing at the house.
            let t = &mut self.travellers[ti];
            while t.home_path.first() == Some(&at) {
                t.home_path.remove(0);
            }
            match t.home_path.first().copied() {
                Some(next) => {
                    let t = &mut self.travellers[ti];
                    t.target = next;
                    t.state = TravelState::OnNetwork;
                }
                None => self.arrive_home(ti),
            }
            return;
        }
        let routes = match (goal, mode) {
            (Goal::Haven, _) => &self.routes_haven,
            (Goal::Shore, _) => &self.routes_shore,
            (_, Mode::Car) => &self.routes_car,
            (_, Mode::Foot) => &self.routes_foot,
        };

        if routes.cost[at as usize] == 0.0 {
            // Standing on the destination.
            if matches!(goal, Goal::Haven | Goal::Shore) {
                self.arrive_haven(ti, at);
            } else {
                self.mark_safe(ti);
            }
            return;
        }

        let next = routes.next[at as usize];
        if next == NO_NODE {
            // Cut off. A car with no drivable route abandons the vehicle and
            // continues on foot, which is what people do and what kills them
            // when they do it too late.
            if mode == Mode::Car && self.routes_foot.reachable(at) {
                self.detach(ti);
                let t = &mut self.travellers[ti];
                t.mode = Mode::Foot;
                t.free_speed = 1.1;
                self.pick_next_hop(ti);
                return;
            }
            self.detach(ti);
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

        let t = &mut self.travellers[ti];
        t.target = next;
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
            if t.state == TravelState::Sheltering {
                s.sheltering += 1;
                if t.shelter_relief >= WATER_RELIEF {
                    s.at_the_shore += 1;
                }
            }
        }
        s.lifted = self.boat_lift.map(|b| b.lifted).unwrap_or(0);
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

/// How long the order actually takes to reach *this* household, given what the
/// fire has done to the network and who they are.
///
/// Two things move it off the baked channel, and both are the same shape of
/// correction: the channel is a property of the population and whether it works
/// is a property of the incident.
///
/// - A party staying in a hotel or a let is told by whoever is running the
///   place, over a PA or a knock on the door. That does not go over the mobile
///   network, so it is the one population that gets *more* reliable when the
///   infrastructure fails.
/// - A mobile alert with no mast is not a mobile alert. It falls back to the
///   delay of having no channel at all, which is what "the fire knocked out
///   communications" means for the household on the end of it. A siren is a
///   physical thing on a pole and a neighbour is a neighbour: neither is
///   touched.
fn warning_delay_s(h: &HouseholdAgent, signal: bool) -> f32 {
    if h.transient {
        return MANAGED_DELAY_S;
    }
    if !signal && h.channel == WarningChannel::MobileAlert {
        return channel_delay_s(WarningChannel::None);
    }
    channel_delay_s(h.channel)
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
