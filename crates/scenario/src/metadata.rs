//! What a scenario says about itself (`scenario.json`).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ScenarioMetadata {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub world_size_m: [f32; 2],
    pub fire_grid_size: [usize; 2],
    /// The districts, in the order the game lists them.
    #[serde(default)]
    pub localities: Vec<String>,
}
