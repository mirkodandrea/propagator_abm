//! The five things three real disasters asked the model for.
//!
//! `docs/behavior-gaps.md` compared this model against the Attica, Pedrógão
//! Grande and Rhodes fires and found it silent on five specific mechanisms —
//! each one a documented cause of what happened, and each one absent here rather
//! than tuned wrongly. These tests are the evidence that the mechanisms are now
//! present *and* that they are off: every shipped figure was measured without
//! them, and a mechanism that changes the baseline by existing would have
//! invalidated all of them.
//!
//! The pattern each test follows is the one finding 26 is about — a branch that
//! validates, appears on the canvas and never fires looks exactly like a working
//! one, so every test here asserts the branch *fires*, not that the model still
//! runs.

use abm::{Abm, BehaviorRuntime, PersonRuntime, TravelState};
use behavior::Library;
use fire::{FireSim, Weather};
use scenario::{Cell, Pos, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .unwrap()
}

/// South-easterly 35 km/h. On Rocca Ventosa the default north wind drives the
/// planned fire away from every home (0 households threatened in two hours),
/// so nothing downstream of the fire is exercised; this window runs into 160+
/// households (see `fire/tests/exposure.rs`).
fn weather() -> Weather {
    Weather { wind_dir_deg: 135.0, ..Weather::default() }
}

fn fire_for(scn: &Scenario) -> FireSim {
    let weather = weather();
    let plan = fire::plan_ignition(scn, weather.wind_dir_deg, 250.0);
    let mut fire = FireSim::new(scn, weather, 42).unwrap();
    fire.ignite_patch(plan.centre, plan.radius_m, scn).unwrap();
    fire
}

fn run(scn: &Scenario, fire: &mut FireSim, agents: &mut Abm, minutes: i64, dt: i64) {
    for _ in 0..(minutes * 60 / dt) {
        fire.advance(dt).unwrap();
        agents.step(dt as f32, fire, scn);
    }
}

/// The shipped library with one household profile and one person profile forced
/// to the whole population, so a test can turn a single block on and see what it
/// costs.
fn library_with(household: &str, person: &str) -> Library {
    let mut lib = behavior::defaults::default_library();
    for (id, s) in lib.subtypes.iter_mut() {
        if s.graph == behavior::defaults::DEFAULT_GRAPH_ID {
            s.share = if id == household { 1.0 } else { 0.0 };
        } else if s.graph == behavior::defaults::DEFAULT_PERSON_GRAPH_ID {
            s.share = if id == person { 1.0 } else { 0.0 };
        }
    }
    lib
}

fn agents_for(scn: &Scenario, lib: &Library) -> Abm {
    let h = BehaviorRuntime::build(lib).unwrap().expect("a household profile with a share");
    let p = PersonRuntime::build(lib).unwrap().expect("a person profile with a share");
    Abm::with_behaviours(scn, 42, h, p).unwrap()
}

// ---------------------------------------------------------------------------
// Gap 4 and 7: somewhere to go that is not a refuge
// ---------------------------------------------------------------------------

/// Havens are measured off the data, exactly as refuges are (finding 9), and
/// the criterion refuges do not have is the one that matters: **buildings**.
/// Non-vegetated fuel does not distinguish a car park from the old town, so a
/// pure fuel test would offer somebody a lane with houses alight on both sides
/// and call it open ground.
#[test]
fn havens_are_measured_off_the_data() {
    let scn = Scenario::load(data_dir()).unwrap();
    let net = abm::network::RoadNetwork::build(&scn);
    let havens = abm::haven::choose(&scn, &net, abm::haven::MAX_HAVENS);

    // Measured on Rocca Ventosa: 5 havens, two by the Borgo car parks and three
    // around Il Piano (the old 10 km coastal window found dozens).
    assert!(havens.len() >= 4, "only {} havens on an 8 km territory", havens.len());
    for h in &havens {
        assert!(
            h.burnable_frac <= 0.10,
            "haven at {:?} sits in {:.0}% burnable fuel",
            h.pos,
            h.burnable_frac * 100.0
        );
    }
    // Inland: no sea, so the shore derivation must find nothing, not invent it.
    assert!(!havens.iter().any(|h| h.is_water()), "a water haven on an inland territory");
}

/// Rocca Ventosa is inland, so it has no shore in its window. That is a fact
/// about the territory rather than about this code, and the reason every block
/// offering the shore is gated on the distance being finite: a maritime
/// evacuation must be refused, not silently accepted.
#[test]
fn an_inland_window_has_no_shore_and_says_so() {
    let scn = Scenario::load(data_dir()).unwrap();
    let net = abm::network::RoadNetwork::build(&scn);
    let havens = abm::haven::choose(&scn, &net, abm::haven::MAX_HAVENS);
    assert!(
        !havens.iter().any(|h| h.is_water()),
        "the territory has no coast in its window but produced a water haven"
    );

    let mut agents = Abm::new(&scn, 42).unwrap();
    assert!(
        agents.request_boat_lift(0.0, 5.0).is_err(),
        "accepted a maritime evacuation with no water in the window"
    );
}

/// The branch fires, and what it does is visible: households that would have
/// sheltered in the house walk out to open ground instead, and they are counted
/// as sheltering rather than as safe — nobody at a haven has left the incident.
///
/// The situation has to be *constructed*, and that is worth stating plainly
/// rather than hiding in a fixture. At the shipped calibration the threat at a
/// house peaks around 0.3 over a two-hour incident, no structure ever ignites
/// and no route is ever cut — so `block.fire_at_the_door` almost never fires,
/// and neither does anything downstream of it, including the branch that has
/// shipped since before this work. This test lowers that block's own threshold,
/// which is a parameter a profile is meant to move, and it is the only thing it
/// changes.
//
// IGNORED on Rocca Ventosa, and that is a finding rather than a test bug. The
// settlements are carved out of the fuel, so the threat field at a house never
// reaches the block's threshold: lighting the most-surrounded burnable cell
// within 550 m of a haven (4970, 5030, in the Borgo) burns 94 ha in two hours
// and the peak threat at every one of the 245 homes is exactly 0.00, with
// the nearest burnt cell 13 m from a door. A 70 km/h, 3% moisture variant
// burns 542 ha and lifts the peak to 0.097 at two households, neither within
// reach of a haven -- a test that passed on two households is the failure the
// comment above describes. `block.fire_at_the_door` (and everything wired to
// it: evacuate_now, shelter, last resort) is therefore dead on this territory
// at the current model calibration; to be decided by the design owner.
#[test]
#[ignore = "fire_at_the_door never fires on Rocca Ventosa: peak threat at every home is 0 (13 m clearance), see comment"]
fn the_last_resort_profile_sends_people_to_open_ground() {
    let scn = Scenario::load(data_dir()).unwrap();
    let graph = behavior::defaults::default_graph();
    let arrival = graph
        .nodes
        .iter()
        .find(|n| n.type_id == "block.fire_at_the_door")
        .expect("the shipped graph still has a fire-at-the-door block");
    let threat_limit = behavior::BehaviorGraph::override_key(arrival.id, "threat_limit");

    let outcome = |profile: &str| {
        let mut lib = library_with(profile, "walk-out");
        lib.subtypes
            .get_mut(profile)
            .unwrap()
            .overrides
            .insert(threat_limit.clone(), behavior::ParamValue::Number(0.05));
        let mut agents = agents_for(&scn, &lib);
        // In the fuel, where the most houses are around it. The shipped
        // ignition is deliberately inland (finding 4 sized it that way) and at
        // that distance the threat at a house never reaches 0.05, never mind
        // 0.35 — which is the measurement this test's own doc comment is about,
        // and the reason it cannot use the shipped fire.
        //
        // *Most houses around it*, rather than the burnable cell nearest their
        // centroid, because that version put one or two homes over the
        // threshold out of 750 and the test passed on a single household. Any
        // change to the fire's own draw then flipped it — enabling shrub
        // spotting did, and the branch was fine. A situation this test
        // constructs has to be constructed for a population, not a house.
        //
        // Only homes inside the branch's own 600 m walking limit of a haven
        // count: on Rocca Ventosa there are 5 havens, all in the Borgo and Il
        // Piano, so the most-surrounded cell overall (Le Coste farms, or the
        // middle of a village) can leave nobody within reach of open ground.
        let homes: Vec<Pos> = agents
            .households
            .iter()
            .map(|h| h.home)
            .filter(|p| {
                agents.havens.iter().any(|v| {
                    ((v.pos.x - p.x).powi(2) + (v.pos.y - p.y).powi(2)).sqrt() < 550.0
                })
            })
            .collect();
        let at = most_surrounded_burnable(&scn, &homes, 200.0)
            .expect("burnable ground with houses around it");
        let mut fire = FireSim::new(&scn, weather(), 42).unwrap();
        fire.ignite_patch(scn.world.cell_of(at), 400.0, &scn).unwrap();
        agents.order_evacuation_all();
        run(&scn, &mut fire, &mut agents, 120, 10);
        (agents.stats(), agents)
    };

    // The same low threshold, without the block: they shelter in the house,
    // which is what the model has always done.
    let (baseline, _) = outcome("wait-and-see");
    assert_eq!(
        baseline.sheltering, 0,
        "{} groups reached a haven with the last-resort block switched off",
        baseline.sheltering
    );

    let (s, agents) = outcome("reacts-to-events");
    assert!(
        s.sheltering > 0,
        "nothing reached a haven: the last-resort branch validated and never fired"
    );
    let at_haven = agents
        .travellers
        .iter()
        .filter(|t| t.state == TravelState::Sheltering)
        .count();
    assert_eq!(at_haven, s.sheltering);
    // And they are not counted as evacuated, which is the distinction the whole
    // state exists to make: alive at a car park is not out of the incident.
    assert!(agents.travellers.iter().all(|t| {
        t.state != TravelState::Sheltering
            || agents.households[t.household].status != scenario::population::Status::Evacuated
    }));
}

// ---------------------------------------------------------------------------
// Gap 3: closing a road to civilian traffic
// ---------------------------------------------------------------------------

/// The lever investigators found missing at Pedrógão Grande. It has to bind
/// civilian traffic and nothing else: an order that also stopped the engine that
/// asked for it, or the people walking out on foot, would be a different and
/// much worse mechanism wearing the same name.
#[test]
fn a_closure_binds_civilian_traffic_and_nothing_else() {
    let scn = Scenario::load(data_dir()).unwrap();
    let mut fire = fire_for(&scn);
    let mut agents = Abm::new(&scn, 42).unwrap();
    run(&scn, &mut fire, &mut agents, 2, 10);

    // Somewhere with road in it: the first refuge is a drivable node by
    // construction.
    let centre = agents.refuges[0].pos;
    let net = abm::network::RoadNetwork::build(&scn);
    let from = net.nearest(centre, true).unwrap();
    let to = net.nearest(agents.refuges[1].pos, true).unwrap();
    let before = abm::network::route(&net, from, to, fire.threat(), true);
    assert!(before.is_some(), "the two refuges are not connected by road");

    let links = agents.close_road(centre, 300.0, 30.0 * 60.0);
    assert!(links > 0, "a closure over a refuge covered no road at all");

    // A unit asking for a route is unaffected: a barricade is a traffic order.
    let after = abm::network::route(&net, from, to, fire.threat(), true);
    assert_eq!(
        before.map(|p| p.len()),
        after.map(|p| p.len()),
        "the closure rerouted a suppression unit"
    );

    // The civilian field is not: some household near it now reads its own way
    // out as closed.
    run(&scn, &mut fire, &mut agents, 2, 10);
    let noticed = agents
        .households
        .iter()
        .enumerate()
        .filter(|(_, h)| {
            (h.home.x - centre.x).powi(2) + (h.home.y - centre.y).powi(2) < 400.0 * 400.0
        })
        .count();
    assert!(noticed > 0, "no household is near the closure to notice it");

    agents.reopen_roads();
    assert_eq!(agents.closures().len(), 0);
}

/// A closure expires on its own, and the mask is rebuilt from the closures
/// still in force rather than toggled per link — otherwise two overlapping
/// closures leave links closed when the first one lifts.
#[test]
fn closures_expire_and_overlapping_ones_do_not_strand_a_link() {
    let scn = Scenario::load(data_dir()).unwrap();
    let mut fire = fire_for(&scn);
    let mut agents = Abm::new(&scn, 42).unwrap();
    let centre = agents.refuges[0].pos;

    agents.close_road(centre, 400.0, 5.0 * 60.0);
    let long = agents.close_road(centre, 400.0, 60.0 * 60.0);
    assert!(long > 0);

    run(&scn, &mut fire, &mut agents, 10, 10);
    assert_eq!(agents.closures().len(), 1, "the short closure did not expire");

    run(&scn, &mut fire, &mut agents, 60, 10);
    assert_eq!(agents.closures().len(), 0, "the long closure did not expire either");
}

// ---------------------------------------------------------------------------
// Gap 6: a correlated warning failure
// ---------------------------------------------------------------------------

/// The fire takes out a mast and every household under it loses the channel at
/// once. The point is the correlation: a per-household draw can make everybody's
/// warning late and cannot make it late *together*, which is what the reporting
/// on Pedrógão Grande describes.
#[test]
fn the_fire_takes_out_the_warning_network() {
    // Rocca Ventosa derives a single mast (at ~(4296, 3801)) that covers all 245
    // households, so losing it is the worst case: every household loses the
    // channel at once, which is the correlation this test is about. Which of
    // the possible shapes a window has is a property of the window.
    let scn = Scenario::load(data_dir()).unwrap();
    let mut agents = Abm::new(&scn, 42).unwrap();
    assert!(!agents.comms().sites().is_empty(), "no warning infrastructure derived");
    assert_eq!(agents.comms().down(), 0);

    // Light the highest mast rather than waiting for the shipped ignition to
    // find one: this is a test of the mechanism, and a fire that happens to
    // reach a site is a test of the scenario.
    let covered_before =
        agents.households.iter().filter(|h| agents.comms().covered(h.home)).count();
    assert_eq!(
        covered_before,
        agents.households.len(),
        "the derivation left households out of coverage before the fire started, which \
         would change the baseline every shipped figure was measured against"
    );

    let site = agents.comms().sites()[0].pos;
    let mut fire = FireSim::new(&scn, weather(), 42).unwrap();
    fire.ignite_patch(scn.world.cell_of(site), 250.0, &scn).unwrap();
    run(&scn, &mut fire, &mut agents, 25, 10);

    assert!(agents.comms().down() > 0, "the fire burnt over a mast and it kept working");
    let covered_after =
        agents.households.iter().filter(|h| agents.comms().covered(h.home)).count();
    assert!(
        covered_after < covered_before,
        "a mast went down and every one of {covered_before} households still had a signal"
    );
}

/// The other half, and the one worth pinning: an outage delays the warning for
/// the households that were relying on it, and a party in a hotel — told by
/// whoever runs the place, over a PA — is not affected at all. A managed
/// population is the one that gets *more* reliable when the infrastructure
/// fails, which is not the intuition and is what the Rhodes accounts describe.
#[test]
fn a_managed_population_is_warned_when_the_network_is_not() {
    let scn = Scenario::load(data_dir()).unwrap();

    let warned_after = |profile: &str| {
        let lib = library_with(profile, "walk-out");
        let mut agents = agents_for(&scn, &lib);
        let mut fire = FireSim::new(&scn, weather(), 42).unwrap();
        // Take the network out first, then order the evacuation: the sequence
        // is the incident's, and an order issued before the outage would have
        // arrived anyway.
        let site = agents.comms().sites()[0].pos;
        fire.ignite_patch(scn.world.cell_of(site), 250.0, &scn).unwrap();
        run(&scn, &mut fire, &mut agents, 25, 10);
        assert!(agents.comms().down() > 0);

        agents.order_evacuation_all();
        run(&scn, &mut fire, &mut agents, 6, 10);
        agents.households.iter().filter(|h| h.warning_received).count()
    };

    let residents = warned_after("wait-and-see");
    let visitors = warned_after("holiday-let");
    assert!(
        visitors > residents,
        "with the network down, {visitors} of a managed population were warned in six \
         minutes against {residents} residents: the fallback is doing nothing"
    );
}

// ---------------------------------------------------------------------------
// Gap 2: a fire that starts where there was no fire
// ---------------------------------------------------------------------------

/// A front growing into new ground is not a spot fire, and a second ignition two
/// kilometres away is. The distinction is the whole value of the field: without
/// it a new fire behind you reads exactly like the front creeping closer.
///
/// Run in **calm air**, which is the only way to hold the front to contiguous
/// growth now that shrubs throw embers: the kernel's landing distance is
/// proportional to wind speed, so at zero wind every ember falls inside its own
/// cell and is discarded, and anything this test then sees is the detector's
/// doing rather than the core's. In the shipped tramontana the same fire spots
/// for real — [`the_shipped_fire_spots`] is that half.
#[test]
fn only_a_non_contiguous_ignition_counts_as_a_spot_fire() {
    let scn = Scenario::load(data_dir()).unwrap();
    let calm = Weather { wind_speed_kmh: 0.0, ..Weather::default() };
    let plan = fire::plan_ignition(&scn, calm.wind_dir_deg, 250.0);
    let mut fire = FireSim::new(&scn, calm, 42).unwrap();
    fire.ignite_patch(plan.centre, plan.radius_m, &scn).unwrap();
    let mut agents = Abm::new(&scn, 42).unwrap();

    run(&scn, &mut fire, &mut agents, 30, 10);
    assert_eq!(
        agents.spot_fires().len(),
        0,
        "the front growing was reported as {} separate fires",
        agents.spot_fires().len()
    );

    // Somewhere burnable, well clear of the front.
    let front = scn.world.centre_of(fire.active_cells()[0]);
    let away = burnable_away_from(&scn, front, 2000.0).expect("somewhere else to light");
    fire.ignite_patch(scn.world.cell_of(away), 200.0, &scn).unwrap();
    run(&scn, &mut fire, &mut agents, 5, 10);

    assert_eq!(
        agents.spot_fires().len(),
        1,
        "a second ignition produced {} spot fires: the batch is not being deduplicated",
        agents.spot_fires().len()
    );
    let spot = agents.spot_fires().spots().next().copied().unwrap();
    let (d, age) = agents.spot_fires().nearest(spot.pos, agents.time_s());
    assert!(d < 200.0, "the spot fire is {d:.0} m from itself");
    assert!(age <= 6.0, "a fire lit five minutes ago reads as {age:.0} minutes old");
}

/// The shipped fire spots, and nobody has to light a second one for it to.
///
/// This is the assertion that the mechanism is *live* rather than merely
/// present, and it is here because it was not for months: the eu12 table flags
/// `spotting` on conifers alone, conifers are 3% of this window against 7%
/// shrub, and over two hours `mati` and `pedrogao` produced no spot fire at
/// all. `block.spot_fire` and the `reacts-to-events` profile were wired to
/// something that could not happen — the same always-negative shape as houses
/// never burning and wetting the flames. See `scripts/bake_fuels.py`, which
/// carries the divergence from CIMA's table, and `fire/tests/spotting.rs`,
/// which measures what it cost.
#[test]
fn the_shipped_fire_spots() {
    let scn = Scenario::load(data_dir()).unwrap();
    // Not the south-easterly the other tests use: measured on Rocca Ventosa, two
    // hours of 35 km/h gave 0 detached fires from the north, east and
    // south-east, and 2, 6 and 8 from the south, south-west and west (seed 42,
    // planned ignition), because the fire only throws embers once it reaches
    // the shrub belts. South-westerly (6 spot fires) is used here.
    let w = Weather { wind_dir_deg: 225.0, ..Weather::default() };
    let plan = fire::plan_ignition(&scn, w.wind_dir_deg, 250.0);
    let mut fire = FireSim::new(&scn, w, 42).unwrap();
    fire.ignite_patch(plan.centre, plan.radius_m, &scn).unwrap();
    let mut agents = Abm::new(&scn, 42).unwrap();
    run(&scn, &mut fire, &mut agents, 120, 10);
    assert!(
        !agents.spot_fires().is_empty(),
        "two hours of a 35 km/h south-westerly on shrub and not one \
         detached fire: check that the shrub classes still carry `spotting`"
    );
}

// ---------------------------------------------------------------------------
// Everything above must have changed nothing
// ---------------------------------------------------------------------------

/// The baseline is intact. Five mechanisms were added to the model and every
/// figure in `crates/fire/tests` was measured before them, so the shipped
/// profiles have to produce the same run they did — no closure, no lift, no
/// haven, no outage effect, and the same evacuation.
#[test]
fn the_shipped_profiles_are_unchanged_by_any_of_it() {
    let scn = Scenario::load(data_dir()).unwrap();
    let mut fire = fire_for(&scn);
    let mut agents = Abm::new(&scn, 42).unwrap();
    agents.order_evacuation_all();
    run(&scn, &mut fire, &mut agents, 120, 10);

    let s = agents.stats();
    assert_eq!(s.sheltering, 0, "a shipped profile sent somebody to a haven");
    assert_eq!(s.lifted, 0, "somebody left on a boat nobody asked for");
    assert_eq!(agents.closures().len(), 0);
    assert!(agents.boat_lift().is_none());
    // The figure the timeline in CLAUDE.md quotes: a general order on the
    // shipped fire evacuates most of the town. Re-measured on Rocca Ventosa
    // (south-easterly, seed 42): 197 of 245 reached safety in two hours.
    assert!(s.safe > 150, "only {} households reached safety", s.safe);
}

// ---------------------------------------------------------------------------
// The half of the warning sequence the model skipped
// ---------------------------------------------------------------------------

/// Measured on the shipped bake: a general order moves 500 of 750 households
/// and silence moves 109, so the commander's order is not a lever in this model,
/// it *is* the model. Two reasons, and `block.order_confirmation` is about the
/// second — the order is believed instantly, so the whole town leaves inside one
/// warning-delivery window. See finding 42.
///
/// The assertion that matters is the one this file exists for: the branch has to
/// *fire*, not merely validate.
#[test]
fn checking_before_leaving_delays_the_departure_and_spreads_it_out() {
    let scn = Scenario::load(data_dir()).unwrap();

    // The same household in every respect except the confirmation step, so the
    // difference cannot be a different kind of person. `takes-some-convincing`
    // also moves the trust threshold, so this comparison isolates the delay by
    // overriding only the block.
    let mut delayed = library_with("wait-and-see", "walk-out");
    let g = behavior::defaults::default_graph();
    let node = g
        .nodes
        .iter()
        .find(|n| n.type_id == "block.order_confirmation")
        .expect("the shipped graph carries the confirmation block");
    let key = behavior::BehaviorGraph::override_key(node.id, "enabled");
    delayed.subtypes.get_mut("wait-and-see").unwrap().overrides.insert(key, behavior::ParamValue::Bool(true));

    let departed_at = |lib: &Library| {
        let mut fire = fire_for(&scn);
        let mut agents = agents_for(&scn, lib);
        agents.order_evacuation_all();
        let mut curve = Vec::new();
        for _ in 0..12 {
            run(&scn, &mut fire, &mut agents, 10, 10);
            let s = agents.stats();
            curve.push(s.preparing + s.moving + s.safe);
        }
        curve
    };

    let straight = departed_at(&library_with("wait-and-see", "walk-out"));
    let checked = departed_at(&delayed);

    // T+10: the checking window. Fewer households have acted, and by a margin
    // that is the mechanism rather than noise. Re-measured on Rocca Ventosa,
    // where the whole town is within a few km of the fire and nearly everyone
    // has acted by T+20 either way: 141 against 223 of 245 at T+10, 224 against
    // 238 at T+20, so T+10 is where the delay is visible.
    assert!(
        checked[0] + 30 < straight[0],
        "confirmation changed nothing in the first ten minutes: {} vs {}",
        checked[0],
        straight[0]
    );
    // T+120: they get there in the end. This block delays, it does not refuse —
    // refusing is what the trust threshold is for, and conflating the two is
    // how a delay gets tuned to produce a compliance rate.
    let (a, b) = (checked[11], straight[11]);
    assert!(
        a * 10 >= b * 9,
        "confirmation stopped people leaving rather than delaying them: {a} vs {b}"
    );
}

/// The profile is the other half: a threshold in the middle of the trust bake
/// rather than in its bottom fifth. Both together are what "we told them to go"
/// is actually worth.
#[test]
fn the_convincing_profile_leaves_a_real_share_of_the_town_at_home() {
    let scn = Scenario::load(data_dir()).unwrap();

    let outcome = |household: &str| {
        let mut fire = fire_for(&scn);
        let mut agents = agents_for(&scn, &library_with(household, "walk-out"));
        agents.order_evacuation_all();
        run(&scn, &mut fire, &mut agents, 120, 10);
        let s = agents.stats();
        (s.safe + s.moving + s.preparing, agents.households.len())
    };

    let (shipped, n) = outcome("wait-and-see");
    let (convinced, _) = outcome("takes-some-convincing");

    // Re-measured on Rocca Ventosa (south-easterly, seed 42, 2 h, general
    // order): 208 of 245 against 238 for the shipped profile, i.e. the trust
    // threshold filters about an eighth of the town, where on the 750-household
    // windows it filtered over a quarter. Most of the territory is close enough
    // to the fire that it leaves on perception regardless, so the bound is
    // "at least a tenth fewer".
    assert!(
        convinced * 10 < shipped * 9,
        "a trust threshold at 0.65 moved {convinced} of {n} against the shipped {shipped}: \
         it is not filtering anybody, which is the always-negative finding 42 is about"
    );
    assert!(
        convinced > n / 10,
        "only {convinced} of {n} left at all -- the profile refuses rather than convinces"
    );
}

/// The block reads a latched timestamp rather than accumulating, so it may not
/// make the model depend on how often it is stepped. Checked separately from
/// the other mechanisms because it is the only one on the *warning* clock, and
/// that clock advances at the warning-delivery quantum rather than at `dt`.
#[test]
fn confirmation_is_step_size_invariant() {
    let scn = Scenario::load(data_dir()).unwrap();
    let outcome = |dt: i64| {
        let mut fire = fire_for(&scn);
        let mut agents = agents_for(&scn, &library_with("takes-some-convincing", "walk-out"));
        agents.order_evacuation_all();
        run(&scn, &mut fire, &mut agents, 90, dt);
        let s = agents.stats();
        s.safe + s.moving + s.preparing
    };
    let (coarse, fine) = (outcome(60), outcome(5));
    assert!(
        (coarse as i64 - fine as i64).abs() <= (coarse.max(fine) / 20 + 3) as i64,
        "60 s step departed {coarse} and 5 s departed {fine}"
    );
}

/// A road closure, a boat lift and a haven are all things the *incident* does,
/// so none of them may make the model depend on how often it is stepped.
#[test]
fn the_new_mechanisms_are_step_size_invariant() {
    let scn = Scenario::load(data_dir()).unwrap();
    let outcome = |dt: i64| {
        let lib = library_with("reacts-to-events", "to-the-water");
        let mut fire = fire_for(&scn);
        let mut agents = agents_for(&scn, &lib);
        agents.order_evacuation_all();
        agents.close_road(agents.refuges[0].pos, 300.0, f32::INFINITY);
        run(&scn, &mut fire, &mut agents, 90, dt);
        let s = agents.stats();
        (s.safe, s.sheltering, s.casualties)
    };
    let coarse = outcome(60);
    let fine = outcome(5);
    let close = |a: usize, b: usize| (a as i64 - b as i64).abs() <= (a.max(b) / 10 + 3) as i64;
    assert!(
        close(coarse.0, fine.0) && close(coarse.1, fine.1) && close(coarse.2, fine.2),
        "60 s step gave {coarse:?} and 5 s gave {fine:?}"
    );
}

/// A profile that turns everything on must still not be able to make a run
/// irreproducible: same seed, same library, same answer.
#[test]
fn turning_it_all_on_stays_deterministic() {
    let scn = Scenario::load(data_dir()).unwrap();
    let once = || {
        let lib = library_with("reacts-to-events", "to-the-water");
        let mut fire = fire_for(&scn);
        let mut agents = agents_for(&scn, &lib);
        agents.order_evacuation_all();
        agents.request_boat_lift(0.0, 6.0).ok();
        run(&scn, &mut fire, &mut agents, 60, 10);
        let s = agents.stats();
        (s.safe, s.sheltering, s.casualties, s.lifted)
    };
    assert_eq!(once(), once());
}

/// The transient capability is assigned like any other profile — a share,
/// hashed — and what it changes is small and specific. Pinned because "we
/// modelled tourists" is the kind of claim that is easy to make and easy to
/// leave inert.
#[test]
fn a_transient_population_is_assigned_by_share_and_has_no_car() {
    let scn = Scenario::load(data_dir()).unwrap();
    let mut lib = behavior::defaults::default_library();
    for (id, s) in lib.subtypes.iter_mut() {
        if s.graph == behavior::defaults::DEFAULT_GRAPH_ID {
            s.share = match id.as_str() {
                "holiday-let" => 0.3,
                "wait-and-see" => 0.7,
                _ => 0.0,
            };
        }
    }
    let agents = agents_for(&scn, &lib);

    let visitors = agents.households.iter().filter(|h| h.transient).count();
    let total = agents.households.len();
    let share = visitors as f32 / total as f32;
    assert!(
        (share - 0.3).abs() < 0.06,
        "asked for 30% visitors and got {:.0}%",
        share * 100.0
    );
    assert!(
        agents.households.iter().filter(|h| h.transient).all(|h| h.vehicles == 0),
        "a visiting party arrived with its own car"
    );
    // And the people in them inherit it, because a hotel party is transient as
    // a party rather than one member at a time.
    let unset = agents
        .people
        .iter()
        .filter(|p| agents.households[p.household].transient && !p.visitor)
        .count();
    assert_eq!(unset, 0, "{unset} people in a visiting party are not marked as visitors");
}

/// One override, and every number moves. The point of the whole composer, on the
/// blocks this round added: the profile that turns them on is the same graph
/// with different booleans, and it is measurable.
#[test]
#[ignore = "reports numbers rather than asserting them"]
fn incident_mechanism_report() {
    println!("\n--- rocca_ventosa ---");
    incident_report_for();
}

fn incident_report_for() {
    let scn = Scenario::load(data_dir()).unwrap();
    println!(
        "{:38} {:>6} {:>10} {:>7} {:>8} {:>8} {:>7}",
        "profile", "safe", "sheltering", "dead", "on foot", "ppl safe", "lifted"
    );
    for (h, p, lift) in [
        ("wait-and-see", "walk-out", false),
        ("reacts-to-events", "walk-out", false),
        ("reacts-to-events", "to-the-water", false),
        ("reacts-to-events", "to-the-water", true),
    ] {
        let lib = library_with(h, p);
        let mut fire = fire_for(&scn);
        let mut agents = agents_for(&scn, &lib);
        agents.order_evacuation_all();
        if lift {
            agents.request_boat_lift(10.0 * 60.0, 6.0).unwrap();
        }
        run(&scn, &mut fire, &mut agents, 120, 10);
        let s = agents.stats();
        println!(
            "{:38} {:>6} {:>10} {:>7} {:>8} {:>8} {:>7}",
            format!("{h} / {p}{}", if lift { " +boats" } else { "" }),
            s.safe,
            s.sheltering,
            s.casualties,
            s.on_foot,
            s.people_safe,
            s.lifted
        );
    }
}

/// What the derivation actually found in each shipped window, so the numbers in
/// `docs/behavior-gaps.md` come from the data rather than from a claim.
#[test]
#[ignore = "reports numbers rather than asserting them"]
fn haven_report() {
    println!("{:12} {:>8} {:>8} {:>8} {:>7}", "scenario", "refuges", "havens", "shore", "masts");
    for id in ["rocca_ventosa"] {
        let scn = Scenario::load(data_dir()).unwrap();
        let net = abm::network::RoadNetwork::build(&scn);
        let refuges = abm::refuge::choose(&scn, &net, 12);
        let havens = abm::haven::choose(&scn, &net, abm::haven::MAX_HAVENS);
        let homes: Vec<Pos> = scn
            .population
            .households
            .iter()
            .map(|h| Pos { x: h.pos[0], y: h.pos[1] })
            .collect();
        let comms = abm::comms::CommsNet::build(&net, &homes);
        println!(
            "{:12} {:>8} {:>8} {:>8} {:>7}",
            id,
            refuges.len(),
            havens.len(),
            havens.iter().filter(|h| h.is_water()).count(),
            comms.sites().len()
        );
    }
}

/// The burnable cell with the most of `homes` within `r` metres of it — the
/// place to light a fire that has to reach a *population* rather than a house.
fn most_surrounded_burnable(scn: &Scenario, homes: &[Pos], r: f32) -> Option<Pos> {
    let w = scn.world;
    let r2 = r * r;
    let mut best: Option<(usize, Pos)> = None;
    for row in (0..w.fire_rows).step_by(2) {
        for col in (0..w.fire_cols).step_by(2) {
            let c = Cell { row, col };
            if !scn.is_burnable(c) {
                continue;
            }
            let q = w.centre_of(c);
            let n = homes
                .iter()
                .filter(|h| (h.x - q.x).powi(2) + (h.y - q.y).powi(2) <= r2)
                .count();
            if n > 0 && best.map(|(bn, _)| n > bn).unwrap_or(true) {
                best = Some((n, q));
            }
        }
    }
    best.map(|(_, q)| q)
}

/// Radius, in 20 m cells, of the patch [`burnable_away_from`] promises is solid
/// fuel. Matches the 200 m the one caller lights.
const PATCH_CELLS: i64 = 10;

/// A patch of burnable fuel at least `min_m` from `p`, with enough of it around
/// to establish — single-cell ignitions fizzle about a fifth of the time
/// (finding 3), and a test whose second fire quietly failed to light would read
/// as the spot detector missing it.
fn burnable_away_from(scn: &Scenario, p: Pos, min_m: f32) -> Option<Pos> {
    let w = scn.world;
    let min2 = min_m * min_m;
    let mut best: Option<(f32, Pos)> = None;
    for row in (0..w.fire_rows).step_by(4) {
        for col in (0..w.fire_cols).step_by(4) {
            let c = Cell { row, col };
            if !scn.is_burnable(c) {
                continue;
            }
            // A neighbourhood of fuel, not one lucky cell — and one that
            // covers the whole patch the caller is about to light, not just
            // its middle. A 200 m patch straddling a road lights as *two*
            // detached blobs, which is a correct reading of a fuel break and
            // an incorrect reading of one ignition.
            let solid = (-PATCH_CELLS..=PATCH_CELLS).all(|dr| {
                (-PATCH_CELLS..=PATCH_CELLS).all(|dc| {
                    let (r, cc) = (row as i64 + dr, col as i64 + dc);
                    r >= 0
                        && cc >= 0
                        && (r as usize) < w.fire_rows
                        && (cc as usize) < w.fire_cols
                        && scn.is_burnable(Cell { row: r as usize, col: cc as usize })
                })
            });
            if !solid {
                continue;
            }
            let q = w.centre_of(c);
            let d2 = (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
            if d2 < min2 {
                continue;
            }
            if best.map(|(bd, _)| d2 < bd).unwrap_or(true) {
                best = Some((d2, q));
            }
        }
    }
    best.map(|(_, q)| q)
}
