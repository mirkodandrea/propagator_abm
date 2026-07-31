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
fn load_scenario_by_id() -> anyhow::Result<()> {
    let data_path = Path::new("../../data");
    let scn = Scenario::load_by_id(data_path, "spotorno")?;

    assert_eq!(scn.id, "spotorno");
    assert_eq!(scn.metadata.id, "spotorno");
    assert!(!scn.vectors.buildings.is_empty());
    assert!(!scn.population.households.is_empty());

    println!("Loaded scenario: {}", scn.metadata.name);
    println!("  Buildings: {}", scn.vectors.buildings.len());
    println!("  Households: {}", scn.population.households.len());

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
