//! The vehicle queue: capacity, storage, spillback, and the properties the
//! rest of the model relies on it keeping.
//!
//! The report at the bottom (`--ignored traffic_report`) is what sizing
//! decisions get made against, in the way `haven_report` and
//! `refill_threshold_report` are. The assertions above it are what stops the
//! model quietly going back to being inert: the old congestion term passed
//! every test there was, because there were none that asked it to *fire*.

use std::collections::HashMap;

use abm::traffic::{RoadClass, JAM_SPACING_M};
use abm::{Abm, Mode, TravelState};
use fire::{FireSim, Weather};
use scenario::Scenario;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .unwrap()
}

fn load(id: &str) -> Scenario {
    Scenario::load_by_id(data_dir(), id).unwrap()
}

/// A fire far enough away to leave the roads alone, so a traffic measurement is
/// about traffic. Ignition is still real: the households have to have a reason
/// to leave.
fn world(scn: &Scenario) -> (FireSim, Abm) {
    let weather = Weather::default();
    let plan = fire::plan_ignition(scn, weather.wind_dir_deg, 250.0);
    let mut fire = FireSim::new(scn, weather, 42).unwrap();
    fire.ignite_patch(plan.centre, plan.radius_m, scn).unwrap();
    let agents = Abm::new(scn, 42).unwrap();
    (fire, agents)
}

/// Vehicles per directed link, counted off the travellers rather than off the
/// queue, so the two have to agree for any of this to mean anything.
fn occupancy(agents: &Abm) -> HashMap<u32, usize> {
    let mut occ: HashMap<u32, usize> = HashMap::new();
    for t in &agents.travellers {
        if t.mode == Mode::Car && t.state == TravelState::OnNetwork {
            if let Some(link) = t.link() {
                *occ.entry(link).or_default() += 1;
            }
        }
    }
    occ
}

// --- the road actually has properties now -----------------------------------

#[test]
fn a_road_class_reaches_the_graph() {
    // The bake has carried `class` and `oneway` per way since the beginning and
    // `RoadNetwork::build` discarded both, so every drivable edge — the A10 and
    // a farm service track alike — had the same speed and the same capacity.
    // This is the assertion that it no longer does.
    let scn = load("spotorno");
    let net = abm::network::RoadNetwork::build(&scn);
    let mut seen: HashMap<&'static str, usize> = HashMap::new();
    for e in 0..net.edge_count as u32 {
        *seen.entry(net.edge_class(e).label()).or_default() += 1;
    }
    assert!(seen.contains_key("motorway"), "the A10 is in this window: {seen:?}");
    assert!(seen.contains_key("residential"), "{seen:?}");
    assert!(seen.contains_key("service"), "{seen:?}");

    let (fast, lanes, cap) = RoadClass::Motorway.params();
    let (slow, _, cap_s) = RoadClass::Service.params();
    assert!(fast > slow * 3.0, "a motorway is not a service road");
    assert!(cap * lanes > cap_s * 4.0, "nor is its capacity");
}

#[test]
fn storage_is_the_length_of_the_road_in_cars() {
    // The failure this pins is the one the old model had at its root: a count
    // of vehicles on a link means nothing until the link has a length.
    let scn = load("spotorno");
    let net = abm::network::RoadNetwork::build(&scn);
    let traffic = abm::traffic::Traffic::new(&net);
    for e in 0..net.edge_count as u32 {
        let link = e * 2;
        let len = net.edge_len(e);
        let (_, lanes, _) = net.edge_class(e).params();
        let expect = ((len * lanes / JAM_SPACING_M).floor() as u32).max(1) as u16;
        assert_eq!(traffic.storage(link), expect, "edge {e} of {len} m");
        // Never zero: a car arriving at a link with nowhere to be would
        // deadlock the network rather than queue on it.
        assert!(traffic.storage(link) >= 1);
    }
}

// --- the three phenomena ----------------------------------------------------

#[test]
fn a_queue_forms_at_the_single_exit() {
    // `congestion_funnel` exists to produce this and, before the queue model,
    // could not: its peak was 9 cars strung out over a 1,088 m exit road, 270 m
    // apart, which is free flow by any measure.
    let scn = load("congestion_funnel");
    let (mut fire, mut agents) = world(&scn);
    agents.order_evacuation_all();

    let mut peak_queue = 0usize;
    let mut peak_link_cars = 0usize;
    for _ in 0..(90 * 60 / 10) {
        fire.advance(10).unwrap();
        agents.step(10.0, &fire, &scn);
        peak_queue = peak_queue.max(agents.traffic.queued_vehicles());
        peak_link_cars = peak_link_cars.max(occupancy(&agents).values().copied().max().unwrap_or(0));
    }
    assert!(
        peak_queue >= 10,
        "no queue ever formed on the single-exit lab: {peak_queue} vehicles on full links, \
         busiest link {peak_link_cars} cars"
    );
}

#[test]
fn a_link_never_holds_more_than_it_can_store() {
    // Spillback is the constraint doing the work, so the invariant it rests on
    // has to hold everywhere, all the time — including on the scenario with the
    // most vehicles in it.
    let scn = load("mass_evacuation");
    let (mut fire, mut agents) = world(&scn);
    agents.order_evacuation_all();
    for _ in 0..(60 * 60 / 10) {
        fire.advance(10).unwrap();
        agents.step(10.0, &fire, &scn);
        for (link, n) in occupancy(&agents) {
            assert!(
                n as u16 <= agents.traffic.storage(link),
                "link {link} holds {n} cars but stores {}",
                agents.traffic.storage(link)
            );
            assert_eq!(
                n as u16,
                agents.traffic.count(link),
                "link {link}: the queue and the travellers disagree"
            );
        }
    }
}

#[test]
fn a_bottleneck_does_not_discharge_faster_than_its_capacity() {
    // The quantity the old model had no bound on at all: with 1,000 cars on one
    // link it would have moved all 1,000 at the floor speed simultaneously and
    // cleared them together.
    let scn = load("congestion_funnel");
    let (mut fire, mut agents) = world(&scn);
    agents.order_evacuation_all();

    // Watch the single exit: whichever link ends up carrying the most traffic
    // over the run is the neck by construction, since everyone uses it.
    let mut through: HashMap<u32, usize> = HashMap::new();
    let mut last: HashMap<usize, Option<u32>> = HashMap::new();
    let horizon_s = 90.0 * 60.0;
    for _ in 0..(horizon_s as i64 / 10) {
        fire.advance(10).unwrap();
        agents.step(10.0, &fire, &scn);
        for (ti, t) in agents.travellers.iter().enumerate() {
            let now = t.link();
            if let Some(prev) = last.insert(ti, now).flatten() {
                if now != Some(prev) {
                    *through.entry(prev).or_default() += 1;
                }
            }
        }
    }
    let (busiest, count) = through.into_iter().max_by_key(|&(_, c)| c).expect("some traffic");
    let cap_veh = agents.traffic.capacity(busiest) * horizon_s;
    assert!(
        count as f32 <= cap_veh * 1.05,
        "link {busiest} passed {count} vehicles in {horizon_s} s, above its \
         capacity of {cap_veh:.0}"
    );
}

// --- the properties the rest of the model relies on -------------------------

#[test]
fn traffic_is_step_size_invariant() {
    // Finding 5, in the one place it is hardest to keep: a queue is a stateful
    // thing and the obvious implementations of one credit capacity per call.
    // The game steps every 2 s and a batch test every 300 s, so anything that
    // accumulated here would make the evacuation figures a property of the
    // caller.
    let scn = load("congestion_funnel");
    let run = |dt: i64| {
        let (mut fire, mut agents) = world(&scn);
        agents.order_evacuation_all();
        for _ in 0..(60 * 60 / dt) {
            fire.advance(dt).unwrap();
            agents.step(dt as f32, &fire, &scn);
        }
        agents.stats()
    };
    let fine = run(2);
    let coarse = run(30);
    // A queue is a stateful thing and the obvious implementations of one credit
    // capacity per call; this one event-times its discharge instead, so the
    // residue is the movement sub-step's junction hand-off and nothing else.
    // Measured across 2-60 s on a 1,000-household lab it is at most one
    // household — see the `step_size_sweep` report.
    let slack = 3;
    assert!(
        (fine.safe as i64 - coarse.safe as i64).abs() <= slack,
        "safe: {} at dt=2 against {} at dt=30",
        fine.safe,
        coarse.safe
    );
    assert!(
        (fine.moving as i64 - coarse.moving as i64).abs() <= slack,
        "moving: {} against {}",
        fine.moving,
        coarse.moving
    );
}

#[test]
fn the_queue_survives_a_car_leaving_from_the_middle() {
    // Rank is recomputed from the FIFO order every sub-step rather than kept as
    // a served counter, precisely so that a car burnt over in the middle of a
    // line — or turned round by a `last_resort` branch — does not desynchronise
    // everyone behind it. The observable is that nothing deadlocks and the
    // counts stay consistent on the scenario where cars do die in traffic.
    let scn = load("pedrogao");
    let (mut fire, mut agents) = world(&scn);
    agents.order_evacuation_all();
    for _ in 0..(120 * 60 / 10) {
        fire.advance(10).unwrap();
        agents.step(10.0, &fire, &scn);
        for (link, n) in occupancy(&agents) {
            assert_eq!(n as u16, agents.traffic.count(link), "link {link} desynchronised");
        }
    }
    let s = agents.stats();
    assert!(s.safe > 0, "nobody got out at all, which is a deadlock rather than a fire");
}

#[test]
fn the_model_is_deterministic() {
    let scn = load("congestion_funnel");
    let run = || {
        let (mut fire, mut agents) = world(&scn);
        agents.order_evacuation_all();
        for _ in 0..(45 * 60 / 10) {
            fire.advance(10).unwrap();
            agents.step(10.0, &fire, &scn);
        }
        agents
            .travellers
            .iter()
            .map(|t| (t.pos.x.to_bits(), t.pos.y.to_bits(), t.state))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}

// --- the report -------------------------------------------------------------

#[test]
#[ignore]
fn traffic_report() {
    for id in ["spotorno", "congestion_funnel", "town_scale", "mass_evacuation"] {
        let scn = load(id);
        let net = abm::network::RoadNetwork::build(&scn);
        let mut lens: Vec<f32> = (0..net.edge_count as u32).map(|e| net.edge_len(e)).collect();
        lens.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let (mut fire, mut agents) = world(&scn);
        agents.order_evacuation_all();

        println!(
            "\n=== {id}: {} households, {} edges, median edge {:.0} m ===",
            scn.population.households.len(),
            net.edge_count,
            lens[lens.len() / 2],
        );

        let mut peak_queue = 0usize;
        let mut peak_link = 0usize;
        for step in 0..(120 * 60 / 10) {
            fire.advance(10).unwrap();
            agents.step(10.0, &fire, &scn);
            let t_s = (step + 1) * 10;
            let occ = occupancy(&agents);
            let busiest = occ.values().copied().max().unwrap_or(0);
            let queued = agents.traffic.queued_vehicles();
            peak_queue = peak_queue.max(queued);
            peak_link = peak_link.max(busiest);
            if t_s % 600 == 0 {
                let s = agents.stats();
                println!(
                    "  t+{:>5}s  cars {:>4}  busiest link {:>3}  queued {:>4}  \
                     prep {:>4} moving {:>4} safe {:>4}",
                    t_s,
                    occ.values().sum::<usize>(),
                    busiest,
                    queued,
                    s.preparing,
                    s.moving,
                    s.safe
                );
            }
        }
        println!("  peak: {peak_link} cars on one link, {peak_queue} on full links");
    }
}

#[test]
#[ignore]
fn step_size_sweep() {
    // The fire is advanced on a fixed 2 s cadence in every run and only the
    // *agent* step varies. Stepping the fire itself at 1 s and at 60 s produces
    // genuinely different fires — the CA has its own quantum — so a sweep that
    // varied both would be measuring the wrong thing, which is what the first
    // version of this did.
    let scn = load("congestion_funnel");
    for agent_dt in [2i64, 4, 6, 10, 30, 60] {
        let (mut fire, mut agents) = world(&scn);
        agents.order_evacuation_all();
        let mut through = 0usize;
        let mut last: HashMap<usize, Option<u32>> = HashMap::new();
        let mut acc = 0i64;
        for _ in 0..(60 * 60 / 2) {
            fire.advance(2).unwrap();
            acc += 2;
            if acc < agent_dt {
                continue;
            }
            agents.step(acc as f32, &fire, &scn);
            acc = 0;
            // Sampled once per agent step, so this undercounts at coarse
            // steps: a car that crosses three links between two samples reads
            // as one crossing. It is here to show the queue is *working*, not
            // as an invariant — `safe` is the invariant.
            for (ti, t) in agents.travellers.iter().enumerate() {
                let now = t.link();
                if let Some(prev) = last.insert(ti, now).flatten() {
                    if now != Some(prev) {
                        through += 1;
                    }
                }
            }
        }
        let s = agents.stats();
        println!(
            "agent dt={agent_dt:>3}s  safe {:>4}  moving {:>4}  prep {:>4}  link crossings {through:>6}",
            s.safe, s.moving, s.preparing
        );
    }
}
