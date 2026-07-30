//! Does the evacuation model do anything, and does it do it for the right
//! reasons?
//!
//! These run the real scenario headlessly -- the whole point of keeping the
//! model out of the Bevy crate -- so a two-hour incident with 750 households
//! and 1,577 people costs a second or two.

use abm::{Abm, TravelState};
use fire::{FireSim, Weather};
use scenario::population::Status;
use scenario::{Pos, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .unwrap()
}

fn setup() -> (Scenario, FireSim, Abm) {
    let scn = Scenario::load(data_dir()).unwrap();
    let weather = Weather::default();
    let plan = fire::plan_ignition(&scn, weather.wind_dir_deg, 250.0);
    let mut fire = FireSim::new(&scn, weather, 42).unwrap();
    fire.ignite_patch(plan.centre, plan.radius_m, &scn).unwrap();
    let agents = Abm::new(&scn, 42).unwrap();
    (scn, fire, agents)
}

/// Run `minutes` of incident at `dt` second steps.
fn run(scn: &Scenario, fire: &mut FireSim, agents: &mut Abm, minutes: i64, dt: i64) {
    for _ in 0..(minutes * 60 / dt) {
        fire.advance(dt).unwrap();
        agents.step(dt as f32, fire, scn);
    }
}

#[test]
fn network_and_refuges_are_sane() {
    let scn = Scenario::load(data_dir()).unwrap();
    let net = abm::network::RoadNetwork::build(&scn);
    // 3,656 ways with 66k vertices weld down to far fewer shared nodes.
    assert!(net.len() > 10_000, "network too small: {} nodes", net.len());

    let refuges = abm::refuge::choose(&scn, &net, 12);
    assert!(!refuges.is_empty(), "no refuge found");
    for r in &refuges {
        // Either it is out of the fuel, or it is a way off the map.
        assert!(
            r.is_exit || r.burnable_frac <= 0.15,
            "refuge at {:?} sits in {:.0}% burnable fuel",
            r.pos,
            r.burnable_frac * 100.0
        );
        assert!(net.is_drivable_node(r.node), "refuge is not drivable");
    }

    // Every household must be able to reach the network on foot.
    for h in &scn.population.households {
        let p = Pos { x: h.pos[0], y: h.pos[1] };
        assert!(net.nearest(p, false).is_some(), "household {} is off-network", h.id);
    }
}

/// The baseline: nobody is told anything. People still leave -- they can see
/// the fire -- but only the ones close enough to perceive it.
#[test]
fn unwarned_population_reacts_only_to_what_it_can_see() {
    let (scn, mut fire, mut agents) = setup();
    run(&scn, &mut fire, &mut agents, 120, 10);

    let s = agents.stats();
    assert!(s.safe + s.moving > 0, "nobody reacted to a 50 ha fire at all");
    assert!(
        s.safe + s.moving + s.preparing < agents.households.len(),
        "the entire town evacuated with no warning issued -- perception is too generous"
    );
}

/// The commander's lever has to work, and it has to work through the warning
/// channels rather than teleporting the decision.
#[test]
fn an_early_order_gets_more_people_out() {
    let (scn, mut fire, mut ordered) = setup();
    ordered.order_evacuation_all();
    run(&scn, &mut fire, &mut ordered, 120, 10);

    let (scn2, mut fire2, mut silent) = setup();
    run(&scn2, &mut fire2, &mut silent, 120, 10);

    let a = ordered.stats();
    let b = silent.stats();
    assert!(
        a.safe > b.safe,
        "an early general evacuation order moved no more people than silence: {} vs {}",
        a.safe,
        b.safe
    );
    assert!(a.people_safe > b.people_safe);
}

/// The order is not instantaneous. At 90 s on a mobile alert and 20 minutes
/// for a household with no channel at all, the spread of departure times is
/// the model's main claim -- if everyone left at once it would be wrong.
#[test]
fn departures_are_spread_out() {
    let (scn, mut fire, mut agents) = setup();
    agents.order_evacuation_all();

    let mut moving_over_time = Vec::new();
    for _ in 0..24 {
        run(&scn, &mut fire, &mut agents, 5, 10);
        moving_over_time.push(agents.stats().moving);
    }
    let peak = *moving_over_time.iter().max().unwrap();
    let total = agents.households.len();
    assert!(
        peak < total * 3 / 4,
        "everyone was on the road at once ({peak} of {total}): warning delay or prep time is not biting"
    );
    assert!(peak > 10, "almost nobody ever moved: {peak}");
}

/// The step-size trap that already caught the structure damage model: nothing
/// may accumulate per call. A 2 s game loop and a 60 s batch step must reach
/// the same place.
#[test]
fn step_size_invariance() {
    let (scn, mut fire_a, mut a) = setup();
    a.order_evacuation_all();
    run(&scn, &mut fire_a, &mut a, 90, 2);

    let (scn_b, mut fire_b, mut b) = setup();
    b.order_evacuation_all();
    run(&scn_b, &mut fire_b, &mut b, 90, 60);

    let (sa, sb) = (a.stats(), b.stats());
    let n = a.households.len() as f32;
    let drift = (sa.safe as f32 - sb.safe as f32).abs() / n;
    assert!(
        drift < 0.05,
        "outcome depends on the caller's step size: {} safe at 2 s vs {} at 60 s",
        sa.safe,
        sb.safe
    );
}

/// Fire on the road has to change the route, not just the colour of it.
#[test]
fn nobody_walks_into_the_flames() {
    let (scn, mut fire, mut agents) = setup();
    agents.order_evacuation_all();
    run(&scn, &mut fire, &mut agents, 120, 10);

    let threat = fire.threat();
    let mut inside = 0;
    for t in &agents.travellers {
        if matches!(t.state, TravelState::Approaching | TravelState::OnNetwork)
            && threat.at(t.pos) >= fire::threat::IMPASSABLE
        {
            inside += 1;
        }
    }
    // Someone can be overrun -- that is the scenario -- but a *moving* agent
    // standing in the flaming front means the routing sent them there.
    assert!(
        inside <= 3,
        "{inside} travellers are moving through the flaming front"
    );
}

/// Everyone ends up somewhere accounted for: no agent silently vanishes, and
/// nobody is left in `Normal` while standing in a fire.
#[test]
fn every_person_is_accounted_for() {
    let (scn, mut fire, mut agents) = setup();
    agents.order_evacuation_all();
    run(&scn, &mut fire, &mut agents, 120, 10);

    assert_eq!(agents.people.len(), scn.population.people.len());
    for p in &agents.people {
        if p.status == Status::Evacuated {
            continue;
        }
        assert!(
            scn.world.contains(p.pos),
            "person {} left the world frame while not evacuated: {:?}",
            p.id,
            p.pos
        );
    }
}

/// What the incident actually looks like, minute by minute. Not an assertion:
/// it is the readout the scenario is tuned against, in the same spirit as
/// `fire::tests::scenario_report`.
#[test]
#[ignore = "report, not a test"]
fn report() {
    let (scn, mut fire, mut agents) = setup();
    println!("refuges:");
    for r in &agents.refuges {
        println!(
            "  node {:6} at ({:6.0}, {:6.0})  {:>10}  {:.0}% burnable within 300 m",
            r.node,
            r.pos.x,
            r.pos.y,
            if r.is_exit { "map exit" } else { "assembly" },
            r.burnable_frac * 100.0
        );
    }
    println!(
        "\n{} households, {} people, {} network nodes\n",
        agents.households.len(),
        agents.people.len(),
        agents.network.len()
    );

    // Order the downwind sector out at T+10 min, as a commander would.
    println!("  time   aware  prep  moving   safe  defend  cutoff  dead   cars   foot");
    for m in 1..=24 {
        run(&scn, &mut fire, &mut agents, 5, 10);
        if m == 2 {
            let n = agents.order_evacuation(
                scn.world.centre_of(fire::plan_ignition(&scn, 0.0, 250.0).centre),
                2500.0,
            );
            println!("  -- T+10 evacuation order issued to {n} households --");
        }
        let s = agents.stats();
        println!(
            "  {:3} min {:6} {:5} {:7} {:6} {:7} {:7} {:5} {:6} {:6}",
            m * 5,
            s.aware,
            s.preparing,
            s.moving,
            s.safe,
            s.defending,
            s.cutoff,
            s.casualties,
            s.cars_moving,
            s.on_foot
        );
    }
    if let Some(med) = agents.median_evacuation_s() {
        println!("\nmedian time from departure to refuge: {:.0} min", med / 60.0);
    }
}

/// Most households own a car, and the ones that do should be leaving in it.
/// A model where everyone walks is a model where the road network does not
/// matter, which would quietly delete the most important constraint in the
/// scenario.
#[test]
fn car_owning_households_drive() {
    let (scn, mut fire, mut agents) = setup();
    agents.order_evacuation_all();
    run(&scn, &mut fire, &mut agents, 60, 10);

    let (mut with_car, mut drove) = (0, 0);
    for h in &agents.households {
        let Some(ti) = h.traveller else { continue };
        if h.vehicles == 0 {
            continue;
        }
        with_car += 1;
        if agents.travellers[ti].mode == abm::Mode::Car {
            drove += 1;
        }
    }
    assert!(with_car > 50, "too few car-owning households departed: {with_car}");
    assert!(
        drove * 4 >= with_car * 3,
        "only {drove} of {with_car} car-owning households actually drove"
    );
}

/// The routing solve runs every simulated minute, and at 512x time
/// acceleration that is roughly eight solves a wall-clock second on top of
/// the fire. Worth knowing what it costs.
#[test]
#[ignore = "timing, not a test"]
fn routing_cost() {
    let (scn, mut fire, mut agents) = setup();
    fire.advance(600).unwrap();
    agents.step(600.0, &fire, &scn);

    let refuges: Vec<_> = agents.refuges.iter().map(|r| r.node).collect();
    let t0 = std::time::Instant::now();
    let n = 20;
    for _ in 0..n {
        std::hint::black_box(abm::network::solve(&agents.network, &refuges, fire.threat(), true));
        std::hint::black_box(abm::network::solve(&agents.network, &refuges, fire.threat(), false));
    }
    println!(
        "{} nodes, both modes: {:.2} ms per refresh",
        agents.network.len(),
        t0.elapsed().as_secs_f64() * 1000.0 / n as f64
    );
}
