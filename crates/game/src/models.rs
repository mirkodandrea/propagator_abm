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
    fn blender_bakes_are_valid_grounded_triangle_meshes() {
        for name in [
            "pedestrian",
            "firefighter",
            "car",
            "fire_engine",
            "pine",
            "oak",
            "bush",
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
            if matches!(name, "pine" | "oak" | "bush") {
                assert!(
                    m.positions.len() <= 120 && m.indices.len() / 3 <= 160,
                    "{name}"
                );
            }
        }
    }
}
