use scenario::{Scenario, ScenarioRegistry};
use std::path::Path;

#[test]
fn discover_scenarios() -> anyhow::Result<()> {
    let data_path = Path::new("../../data");
    let registry = ScenarioRegistry::discover(data_path)?;

    let scenarios = registry.list();
    assert!(!scenarios.is_empty(), "should find at least one scenario");

    // Check default scenario exists
    let default = registry.default_scenario();
    assert!(default.is_some(), "should have a default scenario");

    println!("Found {} scenario(s)", scenarios.len());
    for s in scenarios {
        println!("  - {} ({})", s.name, s.id);
    }

    Ok(())
}

#[test]
fn registry_separates_real_and_development_scenarios() -> anyhow::Result<()> {
    let data_path = Path::new("../../data");
    let registry = ScenarioRegistry::discover(data_path)?;
    let real = registry.real_scenarios();
    let development = registry.development_scenarios();

    assert!(
        !real.is_empty(),
        "the catalog should contain real incidents"
    );
    assert!(
        !development.is_empty(),
        "the catalog should contain development labs"
    );
    assert!(real.iter().all(|scenario| !scenario.is_dev));
    assert!(development.iter().all(|scenario| scenario.is_dev));
    assert_eq!(real.len() + development.len(), registry.list().len());
    assert!(
        !registry
            .default_scenario()
            .expect("validated default")
            .is_dev,
        "the player-facing default must be a real incident"
    );

    Ok(())
}

#[test]
fn load_every_registered_scenario() -> anyhow::Result<()> {
    let data_path = Path::new("../../data");
    let registry = ScenarioRegistry::discover(data_path)?;

    for registered in registry.list() {
        let scn = Scenario::load_by_id(data_path, &registered.id)?;

        assert_eq!(scn.id, registered.id);
        assert_eq!(scn.metadata.id, registered.id);
        assert_eq!(scn.world.fire_rows, registered.fire_grid_size[0], "{}", registered.id);
        assert_eq!(scn.world.fire_cols, registered.fire_grid_size[1], "{}", registered.id);
        assert_eq!(scn.vectors.buildings.len(), registered.buildings_count, "{}", registered.id);
        assert_eq!(scn.population.households.len(), registered.households_count, "{}", registered.id);
        assert_eq!(scn.population.people.len(), registered.people_count, "{}", registered.id);
        assert_eq!(scn.fuel.len(), scn.world.fire_rows * scn.world.fire_cols, "{}", registered.id);
        assert_eq!(scn.dem.len(), scn.world.fire_rows * scn.world.fire_cols, "{}", registered.id);

        println!("Loaded scenario: {} ({})", registered.name, registered.id);
    }

    Ok(())
}

#[test]
fn load_default_scenario() -> anyhow::Result<()> {
    let data_path = Path::new("../../data");
    let scn = Scenario::load(data_path)?;

    assert!(!scn.id.is_empty());
    assert!(!scn.vectors.buildings.is_empty());

    println!("Loaded default scenario: {}", scn.metadata.name);

    Ok(())
}
