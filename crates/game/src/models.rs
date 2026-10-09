//! Blender-authored meshes baked by `scripts/build_models.py`.
//! Embedded for identical native/web loading; one mesh and material per symbol.
use bevy::prelude::*;
use bevy::render::{
    mesh::{Indices, PrimitiveTopology},
    render_asset::RenderAssetUsages,
};
use std::{collections::HashMap, sync::OnceLock};

pub struct Model {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub wood: Vec<bool>,
}

pub fn model(name: &str) -> &'static Model {
    static MODELS: OnceLock<HashMap<String, Model>> = OnceLock::new();
    &MODELS.get_or_init(|| {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../../../assets/models/meshes.json"))
                .expect("valid Blender mesh bake");
        value
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, v)| {
                (
                    name.clone(),
                    Model {
                        positions: serde_json::from_value(v["positions"].clone()).unwrap(),
                        normals: serde_json::from_value(v["normals"].clone()).unwrap(),
                        colors: serde_json::from_value(v["colors"].clone()).unwrap(),
                        indices: serde_json::from_value(v["indices"].clone()).unwrap(),
                        wood: serde_json::from_value(v["wood"].clone()).unwrap(),
                    },
                )
            })
            .collect()
    })[name]
}

/// One piece of the town kit (`scripts/build_town_models.py`): landmarks,
/// props, vehicle and people variants. Same layout as [`Model`], no bark mask;
/// `footprint` is the (width, depth) a landmark was authored at.
pub struct Kit {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub footprint: Option<[f32; 2]>,
}

pub fn kit(name: &str) -> &'static Kit {
    static KIT: OnceLock<HashMap<String, Kit>> = OnceLock::new();
    KIT.get_or_init(|| {
        let value: serde_json::Value = serde_json::from_str(include_str!("../../../assets/models/town.json")).expect("valid town kit bake");
        value
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, v)| {
                (
                    name.clone(),
                    Kit {
                        positions: serde_json::from_value(v["positions"].clone()).unwrap(),
                        normals: serde_json::from_value(v["normals"].clone()).unwrap(),
                        colors: serde_json::from_value(v["colors"].clone()).unwrap(),
                        indices: serde_json::from_value(v["indices"].clone()).unwrap(),
                        footprint: serde_json::from_value(v["footprint"].clone()).unwrap_or(None),
                    },
                )
            })
            .collect()
    })
    .get(name)
    .unwrap_or_else(|| panic!("town kit has no `{name}`"))
}

/// A kit piece as a standalone mesh (people and vehicles, which move).
pub fn kit_mesh(name: &str) -> Mesh {
    let m = kit(name);
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, m.colors.iter().map(|c| [c[0], c[1], c[2], 1.0]).collect::<Vec<_>>());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; m.positions.len()]);
    mesh.insert_indices(Indices::U32(m.indices.clone()));
    mesh
}

pub fn mesh(name: &str) -> Mesh {
    let m = model(name);
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, m.colors.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; m.positions.len()]);
    mesh.insert_indices(Indices::U32(m.indices.clone()));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_town_kit_is_grounded_and_wound_outward() {
        for name in [
            "church", "chapel", "townhall", "school", "fire_station", "fuel", "water_tower", "substation", "farm", "barn",
            "silo", "mill", "industrial", "lighthouse", "cypress", "street_tree", "fountain", "bench", "umbrella_a",
            "lounger", "beach_hut", "boat", "sailboat", "pc_tent", "camp_tent", "caravan", "goal", "assembly_sign",
            "car_hatch", "car_sedan", "car_suv", "car_van", "car_ape", "car_bus", "car_camper", "person_man",
            "person_woman", "person_child", "person_elder",
        ] {
            let m = kit(name);
            assert!(!m.positions.is_empty(), "{name}");
            assert_eq!(m.positions.len(), m.normals.len(), "{name}");
            assert_eq!(m.indices.len() % 3, 0, "{name}");
            // Finding 11: a back-facing triangle draws nothing. Every face's
            // winding must agree with its baked normal.
            for tri in m.indices.chunks_exact(3) {
                let p: Vec<Vec3> = tri.iter().map(|i| Vec3::from(m.positions[*i as usize])).collect();
                let n = Vec3::from(m.normals[tri[0] as usize]);
                let g = (p[1] - p[0]).cross(p[2] - p[0]);
                assert!(g.dot(n) >= -1e-6, "{name}: a face is wound against its normal");
            }
        }
    }

    #[test]
    fn blender_bakes_are_valid_grounded_triangle_meshes() {
        for name in [
            "pedestrian",
            "firefighter",
            "car",
            "fire_engine",
            "pine",
            "oak",
            "chestnut",
            "bush",
            "olive",
            "cypress",
        ] {
            let m = model(name);
            assert!(!m.positions.is_empty(), "{name}");
            assert_eq!(m.positions.len(), m.normals.len(), "{name}");
            assert_eq!(m.positions.len(), m.colors.len(), "{name}");
            assert_eq!(m.positions.len(), m.wood.len(), "{name}");
            assert_eq!(m.indices.len() % 3, 0, "{name}");
            assert!(
                m.positions.iter().flatten().all(|v| v.is_finite()),
                "{name}"
            );
            let min_y = m
                .positions
                .iter()
                .map(|p| p[1])
                .fold(f32::INFINITY, f32::min);
            assert!(min_y >= -0.05 && min_y <= 0.1, "{name}: base {min_y}");
            for triangle in m.indices.chunks_exact(3) {
                let p: Vec<Vec3> = triangle
                    .iter()
                    .map(|i| Vec3::from(m.positions[*i as usize]))
                    .collect();
                assert!(
                    (p[1] - p[0]).cross(p[2] - p[0]).length_squared() > 1e-14,
                    "{name}: degenerate triangle"
                );
            }
            // Vegetation is copied many thousands of times: enforce its budget.
            if matches!(name, "pine" | "oak" | "chestnut" | "bush" | "olive" | "cypress") {
                assert!(
                    m.positions.len() <= 120 && m.indices.len() / 3 <= 160,
                    "{name}"
                );
            }
        }
    }
}
