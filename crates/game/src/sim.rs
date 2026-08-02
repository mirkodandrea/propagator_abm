//! Driving the fire model from the Bevy loop.
//!
//! The fire runs *in-process* on the PROPAGATOR Rust core — there is no
//! external process and no Python at runtime. Simulated time is decoupled from
//! frame time by a fixed accumulator: at 1x, one wall-clock second is one
//! simulated second; the speed control multiplies that. Stepping is capped per
//! frame so a slow step cannot spiral into a death loop.

use abm::{Abm, Suppression};
use bevy::prelude::*;
use fire::{FireSim, IgnitionPlan, Weather};
use scenario::{Cell, Pos, Scenario};

/// Radius of the fire the scenario opens with: already a going fire at the
/// WUI edge, which is the situation an incident commander is called to.
///
/// Sized empirically (`crates/fire/tests/sizing.rs`). A fire only travels
/// ~500-800 m in a two-hour initial-attack window, so a small ignition simply
/// cannot reach the coastal settlement from anywhere it can sustain itself.
/// Measured over a 2 h run: 150 m radius threatens 37-80 households, 250 m
/// threatens 137 while burning half as much ground as 500 m. 250 m it is.
pub const START_RADIUS_M: f32 = 250.0;

/// Never advance more than this much simulated time in a single frame, however
/// far behind the accumulator has fallen.
const MAX_STEP_PER_FRAME_S: f32 = 30.0;
/// Granularity handed to the core. Small enough that the fire front updates
/// smoothly, large enough that the event heap isn't churned pointlessly.
const STEP_QUANTUM_S: i64 = 2;

/// Smallest ignition the UI will let you place.
///
/// Not an arbitrary minimum: below roughly this size a patch is a coin flip at
/// `realizations=1` and simply fails to establish about a fifth of the time
/// (see [`FireSim::ignite_patch`]). Letting the player place a 20 m ignition
/// would mean shipping that coin flip as a feature.
pub const MIN_IGNITION_RADIUS_M: f32 = 60.0;
pub const MAX_IGNITION_RADIUS_M: f32 = 600.0;

/// One lit patch, and when it was lit.
///
/// Kept as a list rather than a single point so the scenario is *reproducible*:
/// a restart replays exactly these, at exactly these times, which is what makes
/// "same fire, different wind" a comparison rather than a new roll of the dice.
#[derive(Debug, Clone, Copy)]
pub struct Ignition {
    pub centre: Cell,
    pub radius_m: f32,
    /// Simulated time this patch is lit at. Zero for the scenario's opening
    /// fire; later for anything the player adds mid-run — a spot fire across a
    /// ridge, a second start down the valley.
    pub at_s: i64,
}

#[derive(Resource)]
pub struct Sim {
    pub fire: FireSim,
    /// The civilian agent model. Stepped with the same simulated seconds the
    /// fire gets, immediately after it, so agents always react to the fire
    /// state of the step they are in rather than the previous one.
    pub agents: Abm,
    /// Crews, engines and aircraft. Stepped alongside the civilians and reading
    /// the same fire state; the interventions it returns are queued for the
    /// core's *next* advance, which is one step (2 s of simulated time) of
    /// latency between a crew swinging a tool and the fuel changing. Cheaper
    /// than the alternative, which is borrowing the fire mutably while the units
    /// are still reading the threat field off it.
    pub crews: Suppression,
    pub scenario: Scenario,
    /// Authored agent behaviour in force, from the composer.
    ///
    /// Held as the *library* rather than a compiled runtime because a restart
    /// has to rebuild the runtime from scratch: a compiled graph owns
    /// evaluation scratch space that is only valid for one `Abm`, and reusing
    /// it across a rebuild would be the same class of bug as reusing the
    /// household list.
    ///
    /// `None` runs the hand-written decision layer, which stays the default.
    pub behaviour: Option<behavior::Library>,
    /// Per-run log of what happened to each agent, for the Inspector's
    /// History section and (later) an LLM "interview an agent" feature.
    /// Rebuilt from scratch alongside `agents`/`crews` on every restart —
    /// see `crate::history` for why it does not outlive one run.
    pub history: crate::history::History,
    pub playing: bool,
    /// Simulated seconds per wall-clock second.
    pub speed: f32,
    /// Decision ticks the player has asked for one at a time, and which run
    /// whether or not the sim is playing.
    ///
    /// A counter rather than a flag so holding the key down queues steps instead
    /// of dropping all but the last, and so a step asked for on a frame that
    /// already stepped is not silently lost.
    pub step_requests: u32,
    accumulator: f32,
    /// Bumped whenever the fire state changes, so views rebuild only then.
    ///
    /// Monotonic **across restarts** as well: it is a "has the view gone
    /// stale" token, not a step count, and resetting it to zero on restart
    /// would leave every cached view believing it was already up to date.
    pub generation: u64,
    /// The initial-attack fire this scenario was designed around, kept for its
    /// provenance (households downwind, corridor fuel) and as the target the
    /// camera opens on. The *live* set of ignitions is [`Sim::ignitions`].
    pub ignition: IgnitionPlan,
    /// Every patch lit or scheduled, in the order it was added.
    pub ignitions: Vec<Ignition>,
    /// Ignitions from [`Sim::ignitions`] not yet lit, because their `at_s` is
    /// still in the future. Only non-empty after a restart that replayed a run
    /// with mid-run ignitions in it.
    pending_ignitions: Vec<Ignition>,
    /// Weather the *next* restart starts from. Diverges from
    /// `fire.weather()` only between the player editing it and applying it.
    pub weather: Weather,
    /// Seed for the fire core and the agent model. Editable so a scenario can
    /// be re-rolled without restarting the process.
    pub seed: u64,
    /// Simulated time at which a general evacuation order fires by itself.
    /// Only set by `SPOTORNO_ORDER_AT`, for unattended runs and screenshots --
    /// in play the order is the commander's to give.
    auto_order_s: Option<i64>,
    /// `SPOTORNO_ORDER_AT`, kept so a restart re-arms the same auto-order.
    auto_order_env_s: Option<i64>,
    /// Simulated time at which every unit is committed to the head of the fire
    /// by itself, and air support is requested. `SPOTORNO_ATTACK_AT` only, for
    /// screenshots and unattended runs — in play this is the commander's job,
    /// and doing it well is most of the game.
    auto_attack_s: Option<i64>,
    auto_attack_env_s: Option<i64>,
}

/// Where suppression units stage, in the order they are handed out.
///
/// The measured refuges (`abm::refuge`), closest to the fire first. Both halves
/// matter: a refuge is already known to be out of the fuel and reachable by
/// vehicle, which is exactly what a staging area needs to be; and ordering by
/// distance to the ignition puts the roster on the near side of town rather than
/// round the far side of the bay, so the first engine is a few minutes out
/// instead of twenty.
fn staging(agents: &Abm, ignition: Pos) -> Vec<Pos> {
    let mut v: Vec<Pos> = agents.refuges.iter().map(|r| r.pos).collect();
    v.sort_by(|a, b| {
        let d = |p: &Pos| (p.x - ignition.x).powi(2) + (p.y - ignition.y).powi(2);
        d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    v
}

/// Raised on the frame a restart happened, so views holding state the sim no
/// longer explains can drop it. Anything derived purely from `Sim` each frame
/// needs no handling; anything *sticky* — a structure's ignition timestamp, a
/// vehicle entity, a smoke particle — does.
#[derive(Event)]
pub struct SimRestarted;

impl Sim {
    pub fn new(scenario: Scenario, weather: Weather, seed: u64) -> anyhow::Result<Sim> {
        let mut fire = FireSim::new(&scenario, weather, seed)?;
        // A going fire at the WUI edge, not a single cell: see
        // FireSim::ignite_patch and fire::ignition for why both the size and
        // the placement matter.
        let ignition = fire::plan_ignition(&scenario, weather.wind_dir_deg, START_RADIUS_M);
        // println, not info!: Sim::new runs before Bevy installs its logger.
        println!(
            "ignition ({}, {}) r={:.0} m: {} households downwind, corridor {:.0}% burnable",
            ignition.centre.row,
            ignition.centre.col,
            ignition.radius_m,
            ignition.households_downwind,
            ignition.corridor_fuel * 100.0
        );
        fire.ignite_patch(ignition.centre, ignition.radius_m, &scenario)?;

        let agents = Abm::new(&scenario, seed)?;
        println!(
            "agents: {} households, {} people, {} road nodes, {} refuges",
            agents.households.len(),
            agents.people.len(),
            agents.network.len(),
            agents.refuges.len()
        );

        let crews = Suppression::new(
            &scenario,
            &staging(&agents, scenario.world.centre_of(ignition.centre)),
        )?;
        println!(
            "suppression: {} units staged, {} air tankers on call",
            crews.units.iter().filter(|u| !u.kind.is_air()).count(),
            crews.units.iter().filter(|u| u.kind.is_air()).count(),
        );

        let auto_order_env_s = std::env::var("SPOTORNO_ORDER_AT")
            .ok()
            .and_then(|v| v.parse().ok());
        let auto_attack_env_s = std::env::var("SPOTORNO_ATTACK_AT")
            .ok()
            .and_then(|v| v.parse().ok());

        let history = crate::history::History::new(&agents, &crews);
        history.record_run_started(seed, weather);
        history.record_ignition(0, ignition.centre.row, ignition.centre.col, ignition.radius_m);
        history.record_locations(&scenario.population);

        Ok(Sim {
            fire,
            agents,
            crews,
            scenario,
            behaviour: None,
            history,
            // SPOTORNO_AUTOPLAY=1 starts running immediately, for screenshots
            // and for headless timing runs.
            playing: std::env::var("SPOTORNO_AUTOPLAY").is_ok(),
            speed: 8.0,
            step_requests: 0,
            accumulator: 0.0,
            generation: 0,
            ignitions: vec![Ignition {
                centre: ignition.centre,
                radius_m: ignition.radius_m,
                at_s: 0,
            }],
            ignition,
            pending_ignitions: Vec::new(),
            weather,
            seed,
            auto_order_s: auto_order_env_s,
            auto_order_env_s,
            auto_attack_s: auto_attack_env_s,
            auto_attack_env_s,
        })
    }

    pub fn time_s(&self) -> i64 {
        self.fire.time_s()
    }

    /// Compile the authored behaviour, if there is one.
    ///
    /// Fails loudly rather than falling back to the shipped model: a run that
    /// silently ignored the behaviour a scientist just authored, and reported
    /// its numbers anyway, is the worst outcome available here.
    fn runtime(&self) -> anyhow::Result<Option<abm::BehaviorRuntime>> {
        match &self.behaviour {
            None => Ok(None),
            Some(lib) => abm::BehaviorRuntime::build(lib)
                .map_err(|e| anyhow::anyhow!("authored behaviour: {e}")),
        }
    }

    /// The same, for the people who are away from their household.
    fn person_runtime(&self) -> anyhow::Result<Option<abm::PersonRuntime>> {
        match &self.behaviour {
            None => Ok(None),
            Some(lib) => abm::PersonRuntime::build(lib)
                .map_err(|e| anyhow::anyhow!("authored person behaviour: {e}")),
        }
    }

    /// The same, for the suppression half of the library.
    ///
    /// Separate from [`Sim::runtime`] rather than one call returning both,
    /// because they are independent: a library may author the civilians, the
    /// separated people, the units, any combination or none, and each falls back
    /// to its own hand-written model on its own.
    fn unit_runtime(&self) -> anyhow::Result<Option<abm::UnitRuntime>> {
        match &self.behaviour {
            None => Ok(None),
            Some(lib) => abm::UnitRuntime::build(lib)
                .map_err(|e| anyhow::anyhow!("authored unit policy: {e}")),
        }
    }

    /// Adopt an authored behaviour library and restart onto it.
    ///
    /// A restart rather than a hot swap, deliberately. Households are
    /// mid-decision — milling, on the road, committed to defending — and a new
    /// decision layer would be answering about a state the old one produced.
    /// The fire, the weather, the seed and the ignition list are untouched, so
    /// this *is* the controlled comparison.
    pub fn apply_behaviour(&mut self, lib: Option<behavior::Library>) -> anyhow::Result<()> {
        let previous = self.behaviour.take();
        self.behaviour = lib;
        match self.restart() {
            Ok(()) => Ok(()),
            Err(e) => {
                // Leave the sim on something that runs. A composer error must
                // not take the game down with it.
                self.behaviour = previous;
                let _ = self.restart();
                Err(e)
            }
        }
    }

    /// Rebuild the fire and the agents from scratch and replay the ignition
    /// list, keeping the loaded scenario.
    ///
    /// The `Scenario` — terrain, vectors, population, rasters — is immutable
    /// and by far the most expensive thing to load, so it is deliberately *not*
    /// reloaded. What is thrown away is everything stateful: the core's event
    /// heap, the burn mask, structure exposure, and every household's decision
    /// history. That is the point: a restart has to be a genuinely clean run,
    /// or comparing two wind directions compares nothing.
    pub fn restart(&mut self) -> anyhow::Result<()> {
        let mut fire = FireSim::new(&self.scenario, self.weather, self.seed)?;

        // Replay in time order. Anything at t=0 is lit now; the rest is armed
        // for `step_fire` to light as the clock reaches it.
        let mut ignitions = std::mem::take(&mut self.ignitions);
        ignitions.sort_by_key(|i| i.at_s);
        let mut pending = Vec::new();
        for ig in &ignitions {
            if ig.at_s <= 0 {
                fire.ignite_patch(ig.centre, ig.radius_m, &self.scenario)?;
            } else {
                pending.push(*ig);
            }
        }

        self.agents = Abm::with_behaviours(
            &self.scenario,
            self.seed,
            self.runtime()?,
            self.person_runtime()?,
        )?;
        // Rebuilt, not reset: a restart has to discard every order the player
        // gave, every litre spent and every metre of line cut, or comparing two
        // plans compares nothing. The roster is deterministic, so unit ids are
        // stable across the rebuild and the views keyed by them survive.
        let ig = self.scenario.world.centre_of(self.ignition.centre);
        self.crews = Suppression::with_policy(
            &self.scenario,
            &staging(&self.agents, ig),
            self.unit_runtime()?,
        )?;
        self.fire = fire;
        self.ignitions = ignitions;
        self.pending_ignitions = pending;
        self.accumulator = 0.0;
        // Fresh log, not a reset one: a restart discards every household's
        // decision history exactly as it discards `agents` itself, or a
        // household's "why" would answer for a run that no longer exists.
        self.history = crate::history::History::new(&self.agents, &self.crews);
        self.history.record_run_started(self.seed, self.weather);
        self.history.record_locations(&self.scenario.population);
        for ig in &self.ignitions {
            if ig.at_s <= 0 {
                self.history.record_ignition(ig.at_s, ig.centre.row, ig.centre.col, ig.radius_m);
            }
        }
        self.auto_order_s = self.auto_order_env_s;
        self.auto_attack_s = self.auto_attack_env_s;
        // Not reset: `generation` (a staleness token -- see the field) and
        // `speed`/`playing`, which are the player's view settings and not part
        // of the scenario.
        self.generation += 1;
        info!(
            "restarted: {} ignition(s), wind {:.0} km/h from {:.0}deg, {:.0}% moisture, seed {}",
            self.ignitions.len(),
            self.weather.wind_speed_kmh,
            self.weather.wind_dir_deg,
            self.weather.moisture_pct,
            self.seed
        );
        Ok(())
    }

    /// Light a new patch at the current simulated time.
    ///
    /// Recorded in [`Sim::ignitions`] with its timestamp, so it survives a
    /// restart as a scheduled event rather than silently becoming part of the
    /// opening fire.
    pub fn add_ignition(&mut self, centre: Cell, radius_m: f32) -> anyhow::Result<()> {
        let at_s = self.time_s();
        self.fire.ignite_patch(centre, radius_m, &self.scenario)?;
        self.ignitions.push(Ignition {
            centre,
            radius_m,
            at_s,
        });
        self.history.record_ignition(at_s, centre.row, centre.col, radius_m);
        self.generation += 1;
        info!(
            "ignition added at ({}, {}) r={radius_m:.0} m, T+{at_s} s",
            centre.row, centre.col
        );
        Ok(())
    }

    /// Apply the staged [`Sim::weather`] to the running fire, from now on.
    ///
    /// Weather is a boundary condition in the core, so this changes what the
    /// front does next without rewriting what it has already done — a wind
    /// shift, which is the single most consequential thing that happens on a
    /// real incident.
    pub fn apply_weather(&mut self) -> anyhow::Result<()> {
        self.fire.set_weather(self.weather)?;
        self.history.record_weather(self.time_s(), self.weather);
        self.generation += 1;
        Ok(())
    }

    /// Has the staged weather drifted from what the fire is actually running?
    pub fn weather_dirty(&self) -> bool {
        let live = self.fire.weather();
        (live.wind_dir_deg - self.weather.wind_dir_deg).abs() > 1e-6
            || (live.wind_speed_kmh - self.weather.wind_speed_kmh).abs() > 1e-6
            || (live.moisture_pct - self.weather.moisture_pct).abs() > 1e-6
    }

    /// The furthest-downwind burning cell: the head of the fire, which is where
    /// an initial attack goes. Falls back to the ignition before anything is
    /// alight.
    pub fn fire_head(&self) -> Pos {
        self.fire
            .active_cells()
            .iter()
            .map(|c| self.scenario.world.centre_of(*c))
            .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or_else(|| self.scenario.world.centre_of(self.ignition.centre))
    }

    /// Send every ground unit to the head of the fire, request air support, and
    /// task whatever aircraft are already on station. Returns units committed.
    ///
    /// The "everything, now" plan. Not a good plan — the measured comparison in
    /// `abm`'s `suppression_changes_the_outcome` shows re-tasking the aircraft
    /// onto the moving head saves half again as much ground — but it is the
    /// plan a screenshot and an unattended run need, and it is what a player
    /// does in their first thirty seconds.
    pub fn commit_all_to_head(&mut self) -> usize {
        let head = self.fire_head();
        // Air support is asked for *first*: an aircraft that has not been
        // requested cannot be tasked, but one that is merely inbound can be
        // briefed, and then goes to work the moment it is on station.
        self.crews.request_air();
        let ids: Vec<usize> = self.crews.units.iter().map(|u| u.id).collect();
        let mut n = 0;
        for id in ids {
            let task = if self.crews.units[id].kind.is_air() {
                abm::Task::Drop { at: head }
            } else {
                abm::Task::Attack { at: head }
            };
            if self.crews.assign(id, task).is_ok() {
                n += 1;
            }
        }
        n
    }

    /// `HH:MM:SS` since ignition, for the HUD.
    pub fn clock(&self) -> String {
        let t = self.fire.time_s();
        format!("{:02}:{:02}:{:02}", t / 3600, (t / 60) % 60, t % 60)
    }

    /// Ask for one decision tick, to be run on the next frame whether or not
    /// the sim is playing.
    ///
    /// This is the granularity a behaviour is *authored* at: the fire's own
    /// quantum is 2 s and almost every one of those shows no behavioural change
    /// at all, so stepping by it would mean pressing the key three times to see
    /// anything and having no way to know which press did it.
    pub fn request_step(&mut self) {
        self.step_requests = self.step_requests.saturating_add(1);
    }

    /// Advance the whole incident by exactly `seconds` of simulated time.
    ///
    /// Extracted from the frame loop so a step and a play frame are the same
    /// code: a single-step facility that took a different path through the
    /// scheduled ignitions or the auto-order would step something other than
    /// what plays.
    pub fn advance(&mut self, seconds: i64) -> anyhow::Result<()> {
        if seconds <= 0 {
            return Ok(());
        }

        if let Some(at) = self.auto_order_s {
            if self.time_s() >= at {
                let n = self.agents.order_evacuation_all();
                info!("scheduled general evacuation at T+{at}s: {n} households");
                self.auto_order_s = None;
            }
        }

        if let Some(at) = self.auto_attack_s {
            if self.time_s() >= at {
                self.auto_attack_s = None;
                let n = self.commit_all_to_head();
                info!("scheduled initial attack at T+{at}s: {n} units committed");
            }
        }

        // Replayed mid-run ignitions, lit as the clock reaches them. Only ever
        // non-empty after a restart: see `Sim::restart`.
        if !self.pending_ignitions.is_empty() {
            let now = self.time_s();
            let due: Vec<Ignition> = self
                .pending_ignitions
                .iter()
                .copied()
                .filter(|i| i.at_s <= now)
                .collect();
            self.pending_ignitions.retain(|i| i.at_s > now);
            for ig in due {
                let lit = {
                    let Sim { fire, scenario, .. } = self;
                    fire.ignite_patch(ig.centre, ig.radius_m, scenario)
                };
                match lit {
                    Err(e) => warn!("replayed ignition at T+{}s failed: {e:#}", ig.at_s),
                    Ok(()) => {
                        self.history.record_ignition(ig.at_s, ig.centre.row, ig.centre.col, ig.radius_m)
                    }
                }
            }
        }

        self.fire.advance(seconds)?;
        let Sim { fire, agents, crews, scenario, .. } = self;
        agents.step(seconds as f32, fire, scenario);
        // The units read the fire, then the fire is handed what they did.
        // Queued rather than applied, so it lands as one merged boundary
        // condition on the next advance: see `Suppression::step`.
        for action in crews.step(seconds as f32, &agents.network, fire, scenario) {
            fire.queue(action);
        }
        self.generation += 1;
        self.history.observe(self.time_s(), &self.agents, &self.crews);
        Ok(())
    }
}

/// Simulated seconds one requested step covers.
///
/// One household decision interval, rounded up to the fire's own quantum so the
/// step lands on a boundary the core can actually advance to. Every agent in
/// every domain gets exactly one decision out of it.
pub const STEP_S: i64 =
    ((abm::DECISION_S as i64 + STEP_QUANTUM_S - 1) / STEP_QUANTUM_S) * STEP_QUANTUM_S;

pub fn step_fire(mut sim: ResMut<Sim>, time: Res<Time>) {
    // A requested step runs whether or not the clock is running, and takes
    // precedence: someone who has paused to read a behaviour and pressed the
    // step key wants that step, not a frame of free running.
    let advance = if sim.step_requests > 0 {
        sim.step_requests -= 1;
        STEP_S
    } else if sim.playing {
        let dt = time.delta_seconds().min(0.25) * sim.speed;
        sim.accumulator = (sim.accumulator + dt).min(MAX_STEP_PER_FRAME_S);
        let whole = sim.accumulator as i64;
        if whole < STEP_QUANTUM_S {
            return;
        }
        let advance = (whole / STEP_QUANTUM_S) * STEP_QUANTUM_S;
        sim.accumulator -= advance as f32;
        advance
    } else {
        return;
    };

    if let Err(e) = sim.advance(advance) {
        error!("fire core failed: {e:#}");
        sim.playing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::Sim;
    use fire::Weather;
    use scenario::{Scenario, ScenarioRegistry};

    #[test]
    fn every_registered_scenario_starts_a_simulation() -> anyhow::Result<()> {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let registry = ScenarioRegistry::discover(&data_dir)?;

        for metadata in registry.list() {
            let scenario = Scenario::load_by_id(&data_dir, &metadata.id)?;
            Sim::new(scenario, Weather::default(), 42)
                .map_err(|error| anyhow::anyhow!("{}: {error:#}", metadata.id))?;
        }

        Ok(())
    }
}
