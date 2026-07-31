//! Driving the wildfire controls without a human at the keyboard.
//!
//! The controls added to the UI — place an ignition, shift the wind, restart —
//! are the only parts of this project that cannot be reached from a test in
//! `crates/fire` or `crates/abm`, because they are *Bevy* behaviour: they
//! depend on resources, events and the reset systems that consume them. A
//! restart that leaves a charred building standing or a stale vehicle on a road
//! is exactly the kind of bug that only shows up in the assembled app.
//!
//! So this drives them in order, in the real app with the real systems running,
//! and prints what the fire and the town did at each stage. Enabled with
//! `SPOTORNO_SELFTEST=1`; it fast-forwards, so it takes a few seconds.
//!
//! It asserts the things that would be silent failures — a restart that did not
//! actually rewind, an ignition the core quietly ignored — and exits non-zero
//! if any of them break.

use bevy::prelude::*;
use fire::CellFire;
use scenario::Cell;

use crate::composer::Composer;
use crate::ignition_edit::IgnitionTool;
use crate::people::PersonView;
use crate::scenario_selector::ScenarioSelector;
use crate::sim::{Sim, SimRestarted};
use crate::AppState;

/// How far to run before each checkpoint, in simulated seconds.
const LEG_S: i64 = 900;

#[derive(Resource, Default)]
pub struct SelfTest {
    stage: Stage,
    /// Burnt area at the end of the first leg, to compare the restart against.
    first_leg_ha: f32,
    /// Water the fire had received before the restart, which afterwards must be
    /// zero again: a restart that keeps the old run's suppression is not a
    /// clean comparison.
    first_leg_water_l: f64,
    /// Households evacuating under the shipped model, to compare the authored
    /// behaviour against.
    shipped_departed: usize,
    /// Person figures on screen before the scenario reload — the count the
    /// rebuilt scene has to match exactly.
    figures_before: usize,
    failures: Vec<String>,
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
enum Stage {
    #[default]
    Burn,
    AddIgnition,
    BurnMore,
    Dispatch,
    Working,
    ShiftWind,
    BurnShifted,
    Restart,
    Verify,
    ShippedLeg,
    BehaviourRun,
    ReloadScenario,
    Reloaded,
    Done,
}

pub fn from_env() -> Option<SelfTest> {
    std::env::var("SPOTORNO_SELFTEST")
        .ok()
        .map(|_| SelfTest::default())
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    mut test: ResMut<SelfTest>,
    mut sim: ResMut<Sim>,
    mut tool: ResMut<IgnitionTool>,
    mut composer: ResMut<Composer>,
    mut restarted: EventWriter<SimRestarted>,
    mut next_state: ResMut<NextState<AppState>>,
    selector: Res<ScenarioSelector>,
    figures: Query<(), With<PersonView>>,
    mut exit: EventWriter<AppExit>,
) {
    // Always running, always as fast as the step cap allows.
    sim.playing = true;
    sim.speed = 512.0;

    let burnt = burnt_ha(&sim);
    let t = sim.time_s();

    match test.stage {
        Stage::Burn => {
            if t < LEG_S {
                return;
            }
            test.first_leg_ha = burnt;
            println!("[selftest] T+{t}s baseline: {burnt:.1} ha burnt");
            check(&mut test, burnt > 5.0, "opening fire never established");
            test.stage = Stage::AddIgnition;
        }

        // A second start, well clear of the first so its growth is its own.
        Stage::AddIgnition => {
            let seed_cell = sim.ignition.centre;
            let target = Cell {
                row: seed_cell.row.saturating_sub(60),
                col: seed_cell.col.saturating_sub(60),
            };
            let before = sim.ignitions.len();
            tool.radius_m = 120.0;
            match sim.add_ignition(target, tool.radius_m) {
                Ok(()) => {
                    check(
                        &mut test,
                        sim.ignitions.len() == before + 1,
                        "added ignition was not recorded",
                    );
                    check(
                        &mut test,
                        sim.ignitions.last().is_some_and(|i| i.at_s == t),
                        "added ignition did not carry its timestamp",
                    );
                    println!(
                        "[selftest] lit a second patch at ({}, {}) r=120 m, T+{t}s",
                        target.row, target.col
                    );
                }
                // Not a failure: the offset cell may be non-burnable. The
                // point of the stage is that the API path works, and a refusal
                // is a legitimate answer from it.
                Err(e) => println!("[selftest] second patch refused (non-burnable): {e:#}"),
            }
            test.stage = Stage::BurnMore;
        }
        Stage::BurnMore => {
            if t < LEG_S * 2 {
                return;
            }
            println!("[selftest] T+{t}s after second patch: {burnt:.1} ha burnt");
            let baseline = test.first_leg_ha;
            check(
                &mut test,
                burnt > baseline,
                "fire did not grow after the second ignition",
            );
            test.stage = Stage::Dispatch;
        }

        // Commit the ground units to the head of the fire and call for air.
        // Only reachable through the same `Suppression` API the panel uses, so
        // this exercises the assignment rules as well as the movement.
        Stage::Dispatch => {
            let head = head_of_fire(&sim);
            let ids: Vec<usize> = sim.crews.units.iter().map(|u| u.id).collect();
            let mut sent = 0;
            for id in ids {
                if sim.crews.units[id].kind.is_air() {
                    continue;
                }
                match sim.crews.assign(id, abm::Task::Attack { at: head }) {
                    Ok(()) => sent += 1,
                    Err(why) => println!("[selftest] {id} refused the order: {why}"),
                }
            }
            check(
                &mut test,
                sent > 0,
                "no ground unit accepted an attack order",
            );

            let air = sim.crews.request_air();
            check(
                &mut test,
                air > 0,
                "no air support was available to request",
            );
            check(
                &mut test,
                sim.crews.air_eta_s().is_some_and(|e| e > 0.0),
                "air support was requested but has no arrival time",
            );
            // An inbound aircraft can be briefed but must not teleport onto the
            // incident to serve the briefing.
            let first_air = sim
                .crews
                .units
                .iter()
                .find(|u| u.kind.is_air())
                .map(|u| u.id);
            if let Some(id) = first_air {
                check(
                    &mut test,
                    sim.crews.assign(id, abm::Task::Drop { at: head }).is_ok(),
                    "an inbound aircraft could not be briefed",
                );
                check(
                    &mut test,
                    !sim.crews.units[id].on_scene(),
                    "briefing an inbound aircraft put it on the incident early",
                );
            }
            println!(
                "[selftest] dispatched {sent} ground units to ({:.0}, {:.0}), \
                 {air} aircraft inbound",
                head.x, head.y
            );
            test.stage = Stage::Working;
        }

        // Long enough for the aircraft to arrive (25 min) and work.
        Stage::Working => {
            if t < LEG_S * 2 + 30 * 60 {
                return;
            }
            // Task the aircraft now that they are on station.
            let head = head_of_fire(&sim);
            let air: Vec<usize> = sim
                .crews
                .units
                .iter()
                .filter(|u| u.kind.is_air())
                .map(|u| u.id)
                .collect();
            let mut tasked = 0;
            for id in air {
                if !sim.crews.units[id].on_scene() {
                    continue;
                }
                if sim.crews.assign(id, abm::Task::Drop { at: head }).is_ok() {
                    tasked += 1;
                }
            }
            check(
                &mut test,
                tasked > 0,
                "no aircraft was on the incident after the response time",
            );

            let s = sim.crews.stats();
            println!(
                "[selftest] T+{t}s suppression: {} working, {:.0} L used, {:.0} m line, \
                 {} drops, {tasked} aircraft now tasked",
                s.working, s.water_l, s.line_m, s.drops
            );
            check(
                &mut test,
                s.lost == 0,
                "a unit was burnt over obeying an order",
            );
            // The work has to have reached the *fire*, not just the unit's own
            // counters: this is the whole intervention path, queue included.
            check(
                &mut test,
                sim.fire.litres_applied > 0.0 || sim.fire.cleared().iter().any(|c| *c),
                "units reported work but the fire received no intervention",
            );
            test.first_leg_water_l = sim.fire.litres_applied;
            test.stage = Stage::ShiftWind;
        }

        // A 90-degree wind shift, applied live. This is the control whose whole
        // value is that it does *not* rewrite the existing scar.
        Stage::ShiftWind => {
            let scar_before = burnt;
            sim.weather.wind_dir_deg = 270.0;
            sim.weather.wind_speed_kmh = 50.0;
            check(
                &mut test,
                sim.weather_dirty(),
                "staged weather did not read as pending",
            );
            if let Err(e) = sim.apply_weather() {
                check(&mut test, false, &format!("applying weather failed: {e:#}"));
            }
            check(
                &mut test,
                !sim.weather_dirty(),
                "weather still pending after being applied",
            );
            check(
                &mut test,
                (burnt_ha(&sim) - scar_before).abs() < 0.01,
                "applying weather changed the existing burn scar",
            );
            println!(
                "[selftest] wind shifted to 50 km/h from W, scar intact at {scar_before:.1} ha"
            );
            test.stage = Stage::BurnShifted;
        }
        Stage::BurnShifted => {
            if t < LEG_S * 3 {
                return;
            }
            println!("[selftest] T+{t}s after wind shift: {burnt:.1} ha burnt");
            test.stage = Stage::Restart;
        }

        Stage::Restart => {
            let ignitions = sim.ignitions.len();
            let gen_before = sim.generation;
            match sim.restart() {
                Ok(()) => {
                    restarted.send(SimRestarted);
                }
                Err(e) => check(&mut test, false, &format!("restart failed: {e:#}")),
            }
            check(
                &mut test,
                sim.time_s() == 0,
                "restart did not rewind the clock",
            );
            check(
                &mut test,
                sim.generation > gen_before,
                "restart did not invalidate the views (generation must never rewind)",
            );
            check(
                &mut test,
                sim.ignitions.len() == ignitions,
                "restart lost an ignition from the replay list",
            );
            // The opening fire is relit immediately; the mid-run patch is armed
            // for its own time, so straight after a restart the scar is only
            // ever the opening fire's.
            let after = burnt_ha(&sim);
            let baseline = test.first_leg_ha;
            check(
                &mut test,
                after < baseline,
                &format!(
                    "restart left {after:.1} ha burnt, more than the opening fire's \
                     first {LEG_S}s ({baseline:.1} ha) -- the old scar survived"
                ),
            );
            check(
                &mut test,
                sim.agents.households.iter().all(|h| !h.ordered),
                "restart left households still under an evacuation order",
            );
            // The suppression roster is rebuilt, so every order, every litre and
            // every metre of line from the old run has to be gone -- including
            // the fuel the old run's crews cut, which lives in the fire.
            let s = sim.crews.stats();
            check(
                &mut test,
                s.water_l == 0.0 && s.line_m == 0.0 && s.drops == 0,
                "restart kept the old run's suppression work on the units",
            );
            check(
                &mut test,
                s.unrequested > 0,
                "restart left air support already requested",
            );
            check(
                &mut test,
                sim.crews
                    .units
                    .iter()
                    .all(|u| matches!(u.task, abm::Task::Hold)),
                "restart left a unit still under orders",
            );
            let had_water = test.first_leg_water_l > 0.0;
            check(
                &mut test,
                had_water && sim.fire.litres_applied == 0.0,
                "restart kept water the previous run had put on the fire",
            );
            check(
                &mut test,
                !sim.fire.cleared().iter().any(|c| *c),
                "restart kept fuel the previous run's crews had cut",
            );
            println!("[selftest] restarted: T+0, {after:.1} ha, {ignitions} ignition(s) replayed");
            test.stage = Stage::Verify;
        }

        // Run the replayed scenario back out to the first checkpoint. It has
        // the shifted wind now, so it will not match the baseline -- what is
        // being checked is that a restarted sim runs at all, and that the
        // mid-run ignition came back on schedule rather than at T+0.
        Stage::Verify => {
            if t < LEG_S + 60 {
                return;
            }
            println!("[selftest] T+{t}s after restart: {burnt:.1} ha burnt");
            check(&mut test, burnt > 5.0, "restarted fire never established");

            // Rewind to a clean run on the shipped model. The two behaviour
            // legs below then differ in exactly one thing -- the decision
            // layer -- which is the only way the comparison they print means
            // anything.
            if let Err(e) = sim.apply_behaviour(None) {
                check(&mut test, false, &format!("rewinding to the shipped model failed: {e:#}"));
            }
            restarted.send(SimRestarted);
            sim.agents.order_evacuation_all();
            test.stage = Stage::ShippedLeg;
        }

        // The Agent Behaviour Composer. Its data structures are tested in
        // `crates/behavior` and its effect on the model in `crates/abm`, but
        // the *wiring* -- the editor's projection of its own canvas, the
        // library it holds, the restart that adopts it -- is Bevy behaviour,
        // and this is the only place it can be exercised.
        Stage::ShippedLeg => {
            if t < LEG_S {
                return;
            }
            test.shipped_departed = departed(&sim);
            println!(
                "[selftest] T+{t}s shipped model: {} households departed",
                test.shipped_departed
            );

            // What the editor would hand to the model is the projection of its
            // own canvas, not the file it loaded. If those disagree, every edit
            // a scientist makes is applied to something other than what they
            // see.
            composer.sync();
            check(
                &mut test,
                composer.runnable(),
                &format!(
                    "the composer opened on a behaviour with {} error(s)",
                    composer.report.error_count()
                ),
            );
            check(
                &mut test,
                composer.graph.nodes.len() == composer.snarl.nodes().count(),
                "the composer's projection lost a node",
            );
            check(
                &mut test,
                !composer.lib.assignment().is_empty(),
                "the shipped behaviour library has no profile in play",
            );

            composer.commit();
            let lib = composer.lib.clone();
            let profiles = lib.assignment().len();
            match sim.apply_behaviour(Some(lib)) {
                Ok(()) => {
                    restarted.send(SimRestarted);
                }
                Err(e) => check(&mut test, false, &format!("applying behaviour failed: {e:#}")),
            }
            check(&mut test, sim.time_s() == 0, "adopting a behaviour did not restart");
            check(
                &mut test,
                sim.agents.behaviour_of(0).is_some(),
                "the households are not running the authored behaviour",
            );
            // Every profile has to reach some household, or the assignment is
            // silently collapsing the population into one.
            let assigned: std::collections::BTreeSet<String> = (0..sim.agents.households.len())
                .filter_map(|i| sim.agents.behaviour_of(i).map(|(id, _, _)| id.to_string()))
                .collect();
            check(
                &mut test,
                assigned.len() == profiles,
                &format!("{} of {profiles} profile(s) reached a household", assigned.len()),
            );
            // The suppression half of the same library. Applying a behaviour
            // rebuilds `Suppression` too, and a unit runtime that failed to
            // build would leave the units silently on the hand-written policy —
            // which looks exactly like a policy that agrees with it.
            let unit_profiles = composer.lib.unit_assignment().len();
            check(
                &mut test,
                unit_profiles > 0,
                "the shipped behaviour library has no unit profile in play",
            );
            let governed = (0..sim.crews.units.len())
                .filter(|i| sim.crews.policy_of(*i).is_some())
                .count();
            check(
                &mut test,
                governed == sim.crews.units.len(),
                &format!(
                    "{governed} of {} units are running the authored policy",
                    sim.crews.units.len()
                ),
            );
            println!(
                "[selftest] behaviour applied: {profiles} household profile(s), \
                 {unit_profiles} unit profile(s) governing {governed} units"
            );
            sim.agents.order_evacuation_all();
            test.stage = Stage::BehaviourRun;
        }

        Stage::BehaviourRun => {
            if t < LEG_S {
                return;
            }
            let n = departed(&sim);
            println!(
                "[selftest] T+{t}s authored behaviour: {n} households departed \
                 (shipped model: {}, same fire, same order, same seed)",
                test.shipped_departed
            );
            check(&mut test, n > 20, "the authored behaviour evacuated almost nobody");

            // The inspector's explanation has to agree with the run, or the
            // panel is showing a household a decision it never made.
            match sim.agents.explain(0, &sim.fire) {
                Some((again, trace)) => {
                    let live = sim.agents.behaviour_of(0).map(|(_, _, d)| d.action);
                    check(
                        &mut test,
                        live == Some(again.action),
                        "the inspector's trace disagrees with the household's decision",
                    );
                    check(&mut test, !trace.nodes.is_empty(), "the trace is empty");
                }
                None => check(&mut test, false, "no explanation for a household under behaviour"),
            }

            // The same for a unit. `Suppression::explain` builds the whole unit
            // observation, including the two parts the civilians have no
            // analogue of — reachability and work availability — so this is
            // where a panic or a nonsense value in either would show up.
            match sim.crews.explain(0, &sim.agents.network, &sim.fire, &sim.scenario) {
                Some((_, trace)) => {
                    check(&mut test, !trace.nodes.is_empty(), "the unit trace is empty");
                }
                None => check(&mut test, false, "no explanation for a unit under an authored policy"),
            }

            // And back off it again: the shipped model has to still be one call
            // away, because every measurement in `crates/fire/tests` was taken
            // on it.
            match sim.apply_behaviour(None) {
                Ok(()) => {
                    restarted.send(SimRestarted);
                }
                Err(e) => check(&mut test, false, &format!("reverting behaviour failed: {e:#}")),
            }
            check(
                &mut test,
                sim.agents.behaviour_of(0).is_none(),
                "reverting left the authored behaviour in place",
            );
            check(
                &mut test,
                sim.crews.policy().is_none(),
                "reverting left the authored unit policy in place",
            );
            test.stage = Stage::ReloadScenario;
        }

        // Scenario ▸ Load scenario… and straight back in. This is the only
        // place the teardown can be tested: it is a state transition with an
        // `OnExit` system, and what it has to get right is that the scene comes
        // back exactly once. A teardown that missed something leaves the old
        // scene's entities behind, so the figure count comes back *doubled* —
        // which renders perfectly and is invisible in a screenshot.
        Stage::ReloadScenario => {
            let before = figures.iter().count();
            test.figures_before = before;
            check(
                &mut test,
                before > 0,
                "no person figures before the scenario reload",
            );
            // `confirmed` is left set, so the selector relaunches the same
            // scenario on its first frame rather than waiting for a click.
            let _ = &selector;
            next_state.set(AppState::SelectingScenario);
            test.stage = Stage::Reloaded;
        }

        Stage::Reloaded => {
            let now = figures.iter().count();
            let before = test.figures_before;
            println!(
                "[selftest] scenario reloaded: {now} figures (was {before}), \
                 T+{t}s, {burnt:.1} ha"
            );
            check(
                &mut test,
                now == before,
                &format!(
                    "the reloaded scene has {now} person figures, not {before} — \
                     the teardown left the old scene behind"
                ),
            );
            check(&mut test, t == 0, "the reloaded scenario did not start at T+0");
            test.stage = Stage::Done;
        }

        Stage::Done => {
            if test.failures.is_empty() {
                println!("[selftest] PASS");
                exit.send(AppExit::Success);
            } else {
                for f in &test.failures {
                    println!("[selftest] FAIL: {f}");
                }
                exit.send(AppExit::Error(
                    std::num::NonZeroU8::new(1).expect("1 is non-zero"),
                ));
            }
        }
    }
}

/// The furthest-downwind burning cell, in world metres — what a commander reads
/// off the map when deciding where to put the units. Falls back to the ignition
/// if nothing is alight, so the stage still exercises the assignment path.
fn head_of_fire(sim: &Sim) -> scenario::Pos {
    sim.fire
        .active_cells()
        .iter()
        .map(|c| sim.scenario.world.centre_of(*c))
        .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or_else(|| sim.scenario.world.centre_of(sim.ignition.centre))
}

/// Households that have got past deciding: milling, moving or out.
fn departed(sim: &Sim) -> usize {
    use scenario::population::Status;
    sim.agents
        .households
        .iter()
        .filter(|h| {
            matches!(h.status, Status::Preparing | Status::Evacuating | Status::Evacuated)
        })
        .count()
}

fn burnt_ha(sim: &Sim) -> f32 {
    let cell_ha = sim.scenario.world.cellsize * sim.scenario.world.cellsize / 10_000.0;
    sim.fire
        .state()
        .iter()
        .filter(|s| matches!(s, CellFire::Burning | CellFire::Burnt))
        .count() as f32
        * cell_ha
}

fn check(test: &mut SelfTest, ok: bool, what: &str) {
    if !ok {
        test.failures.push(what.to_string());
    }
}
