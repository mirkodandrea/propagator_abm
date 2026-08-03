//! What the people who are not with their household do.
//!
//! Until this layer existed, someone who was out when the fire started walked
//! to the nearest refuge and never reconsidered — a decision taken once, in the
//! constructor, that nothing in the model could revisit. That is a defensible
//! default and it is what the guidance says people should do; it is not what
//! post-fire interviews find them doing.
//!
//! The two things worth pinning here are therefore opposites: that switching
//! the layer on with the shipped profile changes *nothing*, and that switching
//! on the reunification profile changes something real.

use abm::{Abm, BehaviorRuntime, PersonRuntime, TravelState};
use behavior::{Library, ParamValue};
use fire::{FireSim, Weather};
use scenario::population::Status;
use scenario::Scenario;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .unwrap()
}

fn fire_for(scn: &Scenario) -> FireSim {
    let weather = Weather::default();
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

/// The shipped person library, with `family-first` given whatever share the
/// caller wants. Zero is what ships.
fn library(family_share: f32) -> Library {
    let mut lib = Library::default();
    let g = behavior::defaults::default_person_graph();
    for mut s in behavior::defaults::default_person_subtypes() {
        if s.id == "family-first" {
            s.share = family_share;
        } else {
            s.share = 1.0 - family_share;
        }
        lib.subtypes.insert(s.id.clone(), s);
    }
    lib.graphs.insert(g.id.clone(), g);
    lib
}

fn runtime(lib: &Library) -> PersonRuntime {
    PersonRuntime::build(lib).unwrap().expect("a person profile with a share")
}

fn agents_with(scn: &Scenario, lib: Option<&Library>) -> Abm {
    let defaults = behavior::defaults::default_library();
    let household = BehaviorRuntime::build(&defaults).unwrap().unwrap();
    let person = lib.map(runtime).unwrap_or_else(|| runtime(&defaults));
    Abm::with_behaviours(scn, 42, household, person).unwrap()
}

fn away_count(agents: &Abm) -> usize {
    agents.people.iter().filter(|p| p.away).count()
}

/// The scenario has to actually contain the agents this domain is about, or
/// every test below is vacuously true.
#[test]
fn some_people_start_away_from_home() {
    let scn = Scenario::load(data_dir()).unwrap();
    let agents = agents_with(&scn, None);
    let away = away_count(&agents);
    assert!(away > 20, "only {away} people are away from home");
    assert!(away < agents.people.len(), "everyone is away from home");
    // And every one of them is already walking, from the constructor rather
    // than from the decision layer.
    for p in agents.people.iter().filter(|p| p.away) {
        assert!(p.traveller.is_some(), "person {} is away and stationary", p.id);
    }
}

/// The convenience constructor and an explicitly compiled copy of the
/// baseline person graph must agree.
#[test]
fn the_default_person_runtime_matches_the_reference_graph() {
    let scn = Scenario::load(data_dir()).unwrap();
    let lib = library(0.0);

    let outcome = |lib: Option<&Library>| {
        let mut fire = fire_for(&scn);
        let mut agents = agents_with(&scn, lib);
        agents.order_evacuation_all();
        run(&scn, &mut fire, &mut agents, 90, 10);
        let s = agents.stats();
        (s.people_safe, s.people_moving, s.people_at_risk)
    };

    assert_eq!(outcome(None), outcome(Some(&lib)));
}

/// And the point of the domain: a profile that sends people back for their
/// families produces a measurably different incident.
#[test]
fn going_back_for_family_changes_the_outcome() {
    let scn = Scenario::load(data_dir()).unwrap();

    let safe_at_90 = |family_share: f32| {
        let lib = library(family_share);
        let mut fire = fire_for(&scn);
        let mut agents = agents_with(&scn, Some(&lib));
        agents.order_evacuation_all();
        run(&scn, &mut fire, &mut agents, 90, 10);
        let arrived = agents
            .travellers
            .iter()
            .filter(|t| t.state == TravelState::Arrived)
            .count();
        (agents.stats().people_safe, arrived)
    };

    let (baseline, never_home) = safe_at_90(0.0);
    let (with_family, went_home) = safe_at_90(1.0);

    assert_eq!(never_home, 0, "the baseline profile sent someone home");
    assert!(went_home > 0, "nobody made it back to a house");
    assert_ne!(baseline, with_family, "reunification changed nothing at all");
}

/// A person who walks home is folded back into the household and stops being
/// an agent of their own — which is what makes this reunification rather than
/// a detour. Checked on the state rather than the counts, because the counts
/// would also be satisfied by someone who simply stopped.
#[test]
fn a_person_who_gets_home_rejoins_the_household() {
    let scn = Scenario::load(data_dir()).unwrap();
    let lib = library(1.0);
    let mut fire = fire_for(&scn);
    let mut agents = agents_with(&scn, Some(&lib));
    // No order: the households stay at home, so there is someone to come back
    // to. An order empties the houses and the whole branch turns itself off.
    run(&scn, &mut fire, &mut agents, 45, 10);

    let home_again: Vec<usize> = agents
        .people
        .iter()
        .filter(|p| !p.away && p.traveller.is_none())
        .map(|p| p.id)
        .filter(|id| {
            // Started away — the constructor is deterministic, so this is the
            // same set every run.
            agents.travellers.iter().any(|t| t.solo && t.household == agents.people[*id].household)
        })
        .collect();
    assert!(!home_again.is_empty(), "nobody rejoined their household");

    for id in home_again {
        let p = &agents.people[id];
        assert!(!matches!(p.status, Status::Evacuating), "person {id} is still travelling");
        let home = agents.households[p.household].home;
        let d = ((p.pos.x - home.x).powi(2) + (p.pos.y - home.y).powi(2)).sqrt();
        assert!(d < 60.0, "person {id} is {d:.0} m from the house they walked to");
    }
}

/// The same step-size guarantee the household layer has. A person behaviour
/// that accumulated anything per call would break it silently, and the
/// destination-not-pace boundary in `decide_people` is what stops it.
#[test]
fn an_authored_person_behaviour_is_step_size_invariant() {
    let scn = Scenario::load(data_dir()).unwrap();
    let lib = library(1.0);

    let safe = |dt: i64| {
        let mut fire = fire_for(&scn);
        let mut agents = agents_with(&scn, Some(&lib));
        agents.order_evacuation_all();
        run(&scn, &mut fire, &mut agents, 60, dt);
        agents.stats().people_safe
    };

    let (fine, coarse) = (safe(2), safe(60));
    let drift = (fine as f32 - coarse as f32).abs() / fine.max(1) as f32;
    assert!(drift < 0.06, "{fine} safe at 2 s, {coarse} at 60 s");
}

/// Profiles are hashed from the person id, so the same person gets the same
/// behaviour across a restart — the property that makes "same fire, different
/// behaviour" a controlled comparison rather than a new roll of the dice.
#[test]
fn person_profiles_survive_a_rebuild() {
    let scn = Scenario::load(data_dir()).unwrap();
    let lib = library(0.5);
    let profiles = || {
        let agents = agents_with(&scn, Some(&lib));
        (0..agents.people.len())
            .map(|i| agents.person_behaviour_of(i).map(|(id, _, _)| id.to_string()))
            .collect::<Vec<_>>()
    };
    let a = profiles();
    assert_eq!(a, profiles());
    assert!(a.iter().flatten().any(|id| id == "family-first"));
    assert!(a.iter().flatten().any(|id| id == "walk-out"));
}

/// Nobody walks into a house the fire has taken, whatever the profile says.
/// The block refuses it, and this is the check that the refusal survives
/// contact with a real fire rather than only the test bench.
#[test]
fn the_reunification_branch_respects_its_own_limits() {
    let scn = Scenario::load(data_dir()).unwrap();
    // Everyone goes home, from any distance, however hot it is where they are.
    // Only "the house is gone" and "the family is safe" are left to stop them.
    let mut lib = library(1.0);
    if let Some(s) = lib.subtypes.get_mut("family-first") {
        let g = &lib.graphs[&s.graph];
        let node = g.nodes.iter().find(|n| n.type_id == "block.person_reunite").unwrap();
        s.overrides.insert(
            behavior::BehaviorGraph::override_key(node.id, "max_threat"),
            ParamValue::Number(1.0),
        );
        s.overrides.insert(
            behavior::BehaviorGraph::override_key(node.id, "max_detour"),
            ParamValue::Number(10.0),
        );
    }

    let mut fire = fire_for(&scn);
    let mut agents = agents_with(&scn, Some(&lib));
    run(&scn, &mut fire, &mut agents, 90, 10);

    for p in agents.people.iter().filter(|p| p.away) {
        let Some(ti) = p.traveller else { continue };
        if agents.travellers[ti].goal != abm::Goal::Home {
            continue;
        }
        assert!(
            !fire.exposure().get(p.household).alight,
            "person {} is walking back to a house that is alight",
            p.id
        );
    }
}
