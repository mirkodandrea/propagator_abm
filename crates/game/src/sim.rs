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
    /// Editable agent behaviour in force, from the composer.
    ///
    /// Held as the *library* rather than a compiled runtime because a restart
    /// has to rebuild the runtime from scratch: a compiled graph owns
    /// evaluation scratch space that is only valid for one `Abm`, and reusing
    /// it across a rebuild would be the same class of bug as reusing the
    /// household list.
    ///
    /// Every domain must have an active profile. A missing or invalid graph is
    /// an error; the simulator has no second decision implementation to use.
    pub behaviour: behavior::Library,
    pub playing: bool,
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
    /// A mission with its fire lit exactly where the caller says, rather than
    /// where `plan_ignition` would choose. The kiosk demo's towns pin their
    /// ignition (`demo::spec`), and the COMPARE screen is only honest if the
    /// live run and its headless twin are lit at the same cell.
    pub fn at_ignition(
        scenario: Scenario,
        weather: Weather,
        at: Pos,
        radius_m: f32,
        seed: u64,
        behaviour: behavior::Library,
    ) -> anyhow::Result<Sim> {
        let ignition = IgnitionPlan {
            centre: scenario.world.cell_of(at),
            radius_m,
            households_downwind: 0,
            corridor_fuel: 1.0,
        };
        Self::with_ignition(scenario, weather, ignition, seed, behaviour)
    }

    fn with_ignition(
        scenario: Scenario,
        weather: Weather,
        ignition: IgnitionPlan,
        seed: u64,
        behaviour: behavior::Library,
    ) -> anyhow::Result<Sim> {
        behaviour.validate_runtime()?;
        let household_runtime = Self::runtime(&behaviour)?;
        let person_runtime = Self::person_runtime(&behaviour)?;
        let unit_runtime = Self::unit_runtime(&behaviour)?;
        let mut fire = FireSim::new(&scenario, weather, seed)?;
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

        let agents = Abm::with_behaviours(&scenario, seed, household_runtime, person_runtime)?;
        println!(
            "agents: {} households, {} people, {} road nodes, {} refuges",
            agents.households.len(),
            agents.people.len(),
            agents.network.len(),
            agents.refuges.len()
        );

        let crews = Suppression::with_policy(
            &scenario,
            &staging(&agents, scenario.world.centre_of(ignition.centre)),
            unit_runtime,
        )?;
        println!(
            "suppression: {} units staged, {} air tankers on call",
            crews.units.iter().filter(|u| !u.kind.is_air()).count(),
            crews.units.iter().filter(|u| u.kind.is_air()).count(),
        );

        Ok(Sim {
            fire,
            agents,
            crews,
            scenario,
            behaviour,
            playing: false,
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
        })
    }

    pub fn time_s(&self) -> i64 {
        self.fire.time_s()
    }

    /// Compile the household graph profiles and require at least one assignment.
    fn runtime(lib: &behavior::Library) -> anyhow::Result<abm::BehaviorRuntime> {
        abm::BehaviorRuntime::build(lib)
            .map_err(|e| anyhow::anyhow!("household behaviour: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("household behaviour: no profile has a positive share"))
    }

    /// The same, for the people who are away from their household.
    fn person_runtime(lib: &behavior::Library) -> anyhow::Result<abm::PersonRuntime> {
        abm::PersonRuntime::build(lib)
            .map_err(|e| anyhow::anyhow!("separated-person behaviour: {e}"))?
            .ok_or_else(|| {
                anyhow::anyhow!("separated-person behaviour: no profile has a positive share")
            })
    }

    /// The same, for the suppression half of the library.
    fn unit_runtime(lib: &behavior::Library) -> anyhow::Result<abm::UnitRuntime> {
        abm::UnitRuntime::build(lib)
            .map_err(|e| anyhow::anyhow!("suppression-unit behaviour: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("suppression-unit behaviour: no profile is enabled"))
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
        // Compile and check complete domain coverage before disturbing the live
        // run. An invalid edit must leave the current incident intact.
        self.behaviour.validate_runtime()?;
        let household_runtime = Self::runtime(&self.behaviour)?;
        let person_runtime = Self::person_runtime(&self.behaviour)?;
        let unit_runtime = Self::unit_runtime(&self.behaviour)?;
        let mut fire = FireSim::new(&self.scenario, self.weather, self.seed)?;

        // Replay in time order. Anything at t=0 is lit now; the rest is armed
        // for `step_fire` to light as the clock reaches it.
        let mut ignitions = self.ignitions.clone();
        ignitions.sort_by_key(|i| i.at_s);
        let mut pending = Vec::new();
        for ig in &ignitions {
            if ig.at_s <= 0 {
                fire.ignite_patch(ig.centre, ig.radius_m, &self.scenario)?;
            } else {
                pending.push(*ig);
            }
        }

        let agents = Abm::with_behaviours(
            &self.scenario,
            self.seed,
            household_runtime,
            person_runtime,
        )?;
        // Rebuilt, not reset: a restart has to discard every order the player
        // gave, every litre spent and every metre of line cut, or comparing two
        // plans compares nothing. The roster is deterministic, so unit ids are
        // stable across the rebuild and the views keyed by them survive.
        let ig = self.scenario.world.centre_of(self.ignition.centre);
        let crews = Suppression::with_policy(
            &self.scenario,
            &staging(&agents, ig),
            unit_runtime,
        )?;

        // Commit the rebuilt run only after every mandatory graph and model
        // component succeeded. A rejected edit must not half-restart the live
        // incident.
        self.agents = agents;
        self.crews = crews;
        self.fire = fire;
        self.ignitions = ignitions;
        self.pending_ignitions = pending;
        self.accumulator = 0.0;
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

    /// Apply the staged [`Sim::weather`] to the running fire, from now on.
    ///
    /// Weather is a boundary condition in the core, so this changes what the
    /// front does next without rewriting what it has already done — a wind
    /// shift, which is the single most consequential thing that happens on a
    /// real incident.
    pub fn apply_weather(&mut self) -> anyhow::Result<()> {
        self.fire.set_weather(self.weather)?;
        self.generation += 1;
        Ok(())
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
                    Ok(()) => {}
                }
            }
        }

        self.fire.advance(seconds)?;
        let Sim { fire, agents, crews, scenario, .. } = self;
        agents.step(seconds as f32, fire, scenario);
        // The units read the fire, then the fire is handed what they did.
        // Queued rather than applied, so it lands as one merged boundary
        // condition on the next advance: see `Suppression::step`.
        for action in crews.step(seconds as f32, &agents.network, &agents.traffic, fire, scenario) {
            fire.queue(action);
        }
        self.generation += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Sim;
    use scenario::Scenario;

    #[test]
    fn every_demo_town_starts_a_simulation() -> anyhow::Result<()> {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        for id in demo::ALL {
            let spec = demo::draw(id, 42).expect("demo town has a spec").spec;
            let scenario = Scenario::load_by_id(&data_dir, spec.id)?;
            Sim::at_ignition(
                scenario,
                spec.weather,
                spec.ignition,
                spec.radius_m,
                42,
                behavior::defaults::default_library(),
            )
            .map_err(|error| anyhow::anyhow!("{}: {error:#}", spec.id))?;
        }
        Ok(())
    }

    /// COMPARE honesty: the live kiosk path (`Sim` + `demo::Referee`, with the
    /// behaviour library loaded from `data/` as the kiosk loads it) and the
    /// headless twin (`demo::Run`) give the same outcome, district stories and
    /// bill for the same orders. If they drift, "senza ordini" compares two
    /// different models, not two commanders.
    #[test]
    fn the_live_session_and_its_headless_twin_agree() -> anyhow::Result<()> {
        use abm::suppression::UnitKind;
        use demo::{Order, Parts, Referee};
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let lib = behavior::Library::load_dir_reported(&data_dir.join(behavior::library::DEFAULT_DIR))?.library;
        let variant = crate::kiosk::variant();
        for id in demo::ALL {
            let draw = demo::draw(id, 11).expect("demo town");
            let spec = draw.spec;
            let orders: Vec<(i64, Order)> = vec![
                (0, Order::EvacuateDistrict(0)),
                (0, Order::Defend { kind: UnitKind::Engine, district: 0 }),
                (8 * 60, Order::EvacuateDistrict(1)),
            ];
            let end = 30 * 60;

            let mut twin = demo::Run::with_variant(&data_dir, spec, draw.seed, variant)?;
            let mut next = 0;
            while twin.time_s() < end {
                while next < orders.len() && orders[next].0 <= twin.time_s() {
                    twin.order(orders[next].1);
                    next += 1;
                }
                twin.step()?;
            }

            let mut sim = Sim::at_ignition(Scenario::load_by_id(&data_dir, spec.id)?, spec.weather, spec.ignition, spec.radius_m, draw.seed, lib.clone())?;
            let mut referee = Referee::new(spec, &sim.scenario, &sim.agents, variant);
            let mut next = 0;
            while sim.time_s() < end {
                while next < orders.len() && orders[next].0 <= sim.time_s() {
                    let Sim { scenario, fire, agents, crews, .. } = &mut sim;
                    referee.order(orders[next].1, Parts { scn: scenario, fire, agents, crews });
                    next += 1;
                }
                {
                    let Sim { fire, crews, .. } = &mut sim;
                    referee.before_step(fire, crews)?;
                }
                sim.advance(demo::STEP_S)?;
                let Sim { scenario, fire, agents, crews, .. } = &mut sim;
                referee.after_step(Parts { scn: scenario, fire, agents, crews });
            }
            let live = referee.tally.outcome(&sim.agents, &sim.fire, &sim.scenario.world);
            assert_eq!(live, twin.outcome(), "{id}: the live session and its twin disagree");
            assert_eq!(referee.reports, twin.referee.reports, "{id}: district stories disagree");
            assert_eq!(referee.ledger(end), twin.ledger(), "{id}: the bills disagree");
        }
        Ok(())
    }
}

