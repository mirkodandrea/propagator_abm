//! The synthetic labs isolate ABM behaviours at several scales.  Loading every
//! one here catches schema/count drift; running the focused labs catches the
//! more subtle failure where authored road lines cross visually but their
//! junction vertices are not connected in the routing graph.

use abm::Abm;
use fire::{FireSim, Weather};
use scenario::{Pos, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .unwrap()
}

#[test]
fn dev_scenarios_load_with_consistent_populations() {
    for id in [
        "abm_micro",
        "policy_lab",
        "suppression_access",
        "road_cutoff",
        "congestion_funnel",
        "fire_mild",
        "fire_extreme",
        "town_scale",
        "mass_evacuation",
    ] {
        let scn = Scenario::load_by_id(data_dir(), id).unwrap();
        assert_eq!(scn.population.people.len(), scn.metadata.people_count, "{id}");
        assert_eq!(scn.population.households.len(), scn.metadata.households_count, "{id}");
        assert_eq!(scn.vectors.buildings.len(), scn.metadata.buildings_count, "{id}");
        let net = abm::network::RoadNetwork::build(&scn);
        let drive_components: std::collections::HashSet<_> = (0..net.len())
            .map(|n| n as u32)
            .filter(|&n| net.is_drivable_node(n))
            .map(|n| net.component(n, true))
            .collect();
        assert_eq!(drive_components.len(), 1, "{id}: drivable roads are disconnected");

        for h in &scn.population.households {
            let p = Pos { x: h.pos[0], y: h.pos[1] };
            assert!(net.nearest(p, false).is_some(), "{id}: household {} is off-network", h.id);
        }
    }
}

#[test]
fn focused_dev_scenarios_react_to_an_order() {
    for id in ["abm_micro", "policy_lab", "road_cutoff", "congestion_funnel"] {
        let scn = Scenario::load_by_id(data_dir(), id).unwrap();
        let weather = Weather::default();
        let plan = fire::plan_ignition(&scn, weather.wind_dir_deg, 250.0);
        let mut fire = FireSim::new(&scn, weather, 42).unwrap();
        fire.ignite_patch(plan.centre, plan.radius_m, &scn).unwrap();
        let mut agents = Abm::new(&scn, 42).unwrap();
        agents.order_evacuation_all();
        for _ in 0..(90 * 60 / 10) {
            fire.advance(10).unwrap();
            agents.step(10.0, &fire, &scn);
        }

        let s = agents.stats();
        assert!(
            s.safe + s.moving + s.preparing > 0,
            "{id}: nobody reacted to a general evacuation order"
        );
    }
}

#[test]
fn severity_pair_controls_people_and_roads() {
    let mild = Scenario::load_by_id(data_dir(), "fire_mild").unwrap();
    let extreme = Scenario::load_by_id(data_dir(), "fire_extreme").unwrap();
    assert_eq!(mild.population.people.len(), extreme.population.people.len());
    assert_eq!(mild.population.households.len(), extreme.population.households.len());
    assert_eq!(mild.vectors.roads.len(), extreme.vectors.roads.len());
    let mild_fast_fuel = mild.fuel.iter().filter(|&&f| f >= 9).count();
    let extreme_fast_fuel = extreme.fuel.iter().filter(|&&f| f >= 9).count();
    assert!(extreme_fast_fuel > mild_fast_fuel);
    let mild_plan = fire::plan_ignition(&mild, Weather::default().wind_dir_deg, 250.0);
    let extreme_plan = fire::plan_ignition(&extreme, Weather::default().wind_dir_deg, 250.0);
    assert!(mild_plan.households_downwind > 0, "mild fire has no population at risk");
    assert!(extreme_plan.households_downwind > 0, "extreme fire has no population at risk");
}

#[test]
fn suppression_lab_has_staging_and_all_access_types() {
    let scn = Scenario::load_by_id(data_dir(), "suppression_access").unwrap();
    assert!(scn.vectors.roads.iter().any(|r| r.drivable));
    assert!(scn.vectors.roads.iter().any(|r| r.track && !r.drivable));
    assert!(scn.vectors.water.iter().any(|w| w.kind == "hydrant"));
    assert!(scn.vectors.water.iter().any(|w| w.kind == "open_water"));
    let agents = Abm::new(&scn, 42).unwrap();
    let bases: Vec<_> = agents.refuges.iter().map(|r| r.pos).collect();
    assert!(!bases.is_empty());
    abm::suppression::Suppression::new(&scn, &bases).unwrap();
}
