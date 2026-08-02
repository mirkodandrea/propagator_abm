//! Scenario metadata and registry for discovering and loading scenarios.

use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioType {
    Real,
    Synthetic,
    Hybrid,
}

/// Flat colour palette for the "VR training mission" look applied to dev
/// scenarios: a void-colored background, a quiet void floor, and flat
/// unlit geometry — no textures, no realistic sun/sky. Only meaningful when
/// [`ScenarioMetadata::is_dev`] is set; see [`crate::Scenario::vr_palette`].
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct VrPalette {
    pub void: [f32; 3],
    pub grid: [f32; 3],
    pub accent: [f32; 3],
}

impl VrPalette {
    /// MGS1 VR-mission navy-and-cyan, used for any dev scenario that does not
    /// specify its own palette.
    pub const DEFAULT: VrPalette = VrPalette {
        void: [0.02, 0.03, 0.08],
        grid: [0.0, 0.85, 1.0],
        accent: [0.90, 0.95, 1.0],
    };
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScenarioMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub location: String,
    pub country: String,
    pub coordinates: [f64; 2],
    pub utm_zone: u8,
    pub world_size_m: [f32; 2],
    pub fire_grid_size: [usize; 2],
    pub buildings_count: usize,
    pub households_count: usize,
    pub people_count: usize,
    pub scenario_type: ScenarioType,
    pub creation_date: String,
    pub version: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Mark as development/test scenario for ABM testing
    #[serde(default)]
    pub is_dev: bool,
    /// Optional per-scenario override for the VR-training palette. Ignored
    /// unless `is_dev` is set; falls back to [`VrPalette::DEFAULT`] when dev
    /// and unset.
    #[serde(default)]
    pub vr_palette: Option<VrPalette>,
}

#[derive(Deserialize)]
struct RegistryFile {
    default: String,
    scenarios: Vec<ScenarioMetadata>,
}

/// Registry of available scenarios discovered from the data directory.
pub struct ScenarioRegistry {
    scenarios: HashMap<String, ScenarioMetadata>,
    default: String,
}

impl ScenarioRegistry {
    /// Discover all available scenarios by scanning the scenarios.json registry file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn discover(data_dir: &Path) -> Result<Self> {
        let registry_path = data_dir.join("scenarios.json");
        let bytes = std::fs::read(&registry_path)
            .with_context(|| format!("reading {}", registry_path.display()))?;

        Self::from_bytes(&bytes).context("parsing scenarios.json")
    }

    /// Load the registry compiled into a browser build.
    #[cfg(target_arch = "wasm32")]
    pub fn load_web() -> Result<Self> {
        Self::from_bytes(crate::web_assets::REGISTRY).context("parsing embedded scenarios.json")
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let reg: RegistryFile = serde_json::from_slice(bytes)?;

        let mut scenarios = HashMap::new();
        for scenario in reg.scenarios {
            let id = scenario.id.clone();
            anyhow::ensure!(
                scenarios.insert(id.clone(), scenario).is_none(),
                "duplicate scenario id {id:?}"
            );
        }

        anyhow::ensure!(!scenarios.is_empty(), "scenario registry is empty");
        anyhow::ensure!(
            scenarios.contains_key(&reg.default),
            "default scenario {:?} is not registered",
            reg.default
        );

        Ok(ScenarioRegistry {
            scenarios,
            default: reg.default,
        })
    }

    /// List all available scenario metadata.
    pub fn list(&self) -> Vec<&ScenarioMetadata> {
        let mut list: Vec<_> = self.scenarios.values().collect();
        list.sort_by_key(|s| s.id.as_str());
        list
    }

    /// Get metadata for a specific scenario by ID.
    pub fn get(&self, id: &str) -> Option<&ScenarioMetadata> {
        self.scenarios.get(id)
    }

    /// Get the default scenario metadata.
    pub fn default_scenario(&self) -> Option<&ScenarioMetadata> {
        self.scenarios.get(&self.default)
    }

    /// Get the ID of the default scenario.
    pub fn default_id(&self) -> &str {
        &self.default
    }
}
