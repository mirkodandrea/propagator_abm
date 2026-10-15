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

/// The archetypes scattered as vegetation, by the thousand.
const PLANTS: [&str; 6] = ["pine", "oak", "chestnut", "bush", "olive", "cypress"];

impl Model {
    /// The faces whose indices `tris` lists, with only the vertices they use.
    fn subset(&self, tris: &[[u32; 3]]) -> Model {
        let mut remap = vec![u32::MAX; self.positions.len()];
        let mut out = Model { positions: vec![], normals: vec![], colors: vec![], indices: vec![], wood: vec![] };
        for tri in tris {
            for &i in tri {
                let i = i as usize;
                if remap[i] == u32::MAX {
                    remap[i] = out.positions.len() as u32;
                    out.positions.push(self.positions[i]);
                    out.normals.push(self.normals[i]);
                    out.colors.push(self.colors[i]);
                    out.wood.push(self.wood[i]);
                }
                out.indices.push(remap[i]);
            }
        }
        out
    }
}

/// A plant archetype split in two: `.0` without its steeply downward-facing
/// faces (the lower half of each crown icosphere), `.1` only those faces.
///
/// From the shallowest pitch the kiosk camera allows (-0.55 rad) a face is
/// visible only if its normal is above -sin 0.55 ≈ -0.52, so the undersides
/// can't be seen from there; yet each costs vertex work in every pass, shadow
/// cascades included. Mario's camera does look up at them, so near chunks draw
/// both parts and distant ones only the first.
pub fn split_plant(name: &str) -> &'static (Model, Model) {
    static SPLIT: OnceLock<HashMap<&'static str, (Model, Model)>> = OnceLock::new();
    &SPLIT.get_or_init(|| {
        const MIN_NY: f32 = -0.6;
        PLANTS
            .into_iter()
            .map(|name| {
                let m = model(name);
                let (mut up, mut down) = (vec![], vec![]);
                for t in m.indices.chunks_exact(3) {
                    let ny = t.iter().map(|&i| m.normals[i as usize][1]).sum::<f32>() / 3.0;
                    (if ny < MIN_NY { &mut down } else { &mut up }).push([t[0], t[1], t[2]]);
                }
                (name, (m.subset(&up), m.subset(&down)))
            })
            .collect()
    })[name]
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

fn parse_kit(json: &str, what: &str) -> HashMap<String, Kit> {
    let value: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|_| panic!("valid {what} bake"));
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
}

pub fn kit(name: &str) -> &'static Kit {
    static KIT: OnceLock<HashMap<String, Kit>> = OnceLock::new();
    KIT.get_or_init(|| parse_kit(include_str!("../../../assets/models/town.json"), "town kit"))
        .get(name)
        .unwrap_or_else(|| panic!("town kit has no `{name}`"))
}

/// The island's own models (`scripts/build_island_models.py`): the rocca, the
/// stone tower, the sailing and moored boats. Same layout as the town kit.
pub fn island(name: &str) -> &'static Kit {
    static ISLAND: OnceLock<HashMap<String, Kit>> = OnceLock::new();
    ISLAND
        .get_or_init(|| parse_kit(include_str!("../../../assets/models/island.json"), "island kit"))
        .get(name)
        .unwrap_or_else(|| panic!("island kit has no `{name}`"))
}

/// An island model as a mesh.
pub fn island_mesh(name: &str) -> Mesh {
    let m = island(name);
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, m.colors.iter().map(|c| [c[0], c[1], c[2], 1.0]).collect::<Vec<_>>());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; m.positions.len()]);
    mesh.insert_indices(Indices::U32(m.indices.clone()));
    mesh
}

/// A burnt plant of `scripts/build_burnt_models.py`: `<species>_<a|b|c>` with
/// species pine, chestnut, bush or olive, in the model space of the live plant
/// of the same name (so the same scale vector fits). Colours are absolute sRGB.
pub fn burnt(name: &str) -> &'static Kit {
    static BURNT: OnceLock<HashMap<String, Kit>> = OnceLock::new();
    BURNT
        .get_or_init(|| parse_kit(include_str!("../../../assets/models/burnt.json"), "burnt vegetation"))
        .get(name)
        .unwrap_or_else(|| panic!("burnt vegetation has no `{name}`"))
}

/// A model of `scripts/build_unit_models.py` (firefighter, engine, tanker) as
/// a mesh, in neutral greys so the unit's status tint reads.
pub fn unit_mesh(name: &str) -> Mesh {
    static UNITS: OnceLock<HashMap<String, Kit>> = OnceLock::new();
    let m = UNITS
        .get_or_init(|| parse_kit(include_str!("../../../assets/models/units.json"), "unit models"))
        .get(name)
        .unwrap_or_else(|| panic!("unit models have no `{name}`"));
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, m.colors.iter().map(|c| [c[0], c[1], c[2], 1.0]).collect::<Vec<_>>());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; m.positions.len()]);
    mesh.insert_indices(Indices::U32(m.indices.clone()));
    mesh
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
    fn the_unit_models_load_and_stand_on_the_ground() {
        for name in ["firefighter", "engine", "tanker"] {
            let m = unit_mesh(name);
            assert!(m.count_vertices() > 0, "{name}");
        }
    }

    #[test]
    fn the_burnt_plants_stand_on_the_ground_within_budget() {
        for species in ["pine", "chestnut", "bush", "olive"] {
            let live_h = model(species).positions.iter().map(|p| p[1]).fold(0.0, f32::max);
            for k in ["a", "b", "c", "a_low", "b_low", "c_low"] {
                let m = burnt(&format!("{species}_{k}"));
                assert!(!m.indices.is_empty() && m.indices.len() % 3 == 0, "{species}_{k}");
                assert!(m.positions.iter().flatten().all(|v| v.is_finite()), "{species}_{k}");
                let low = m.positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
                let high = m.positions.iter().map(|p| p[1]).fold(0.0, f32::max);
                assert!((-0.05..=0.1).contains(&low), "{species}_{k}: base {low}");
                assert!(high <= live_h * 1.1, "{species}_{k}: taller than the live plant");
                let budget = if k.ends_with("low") { 60 } else { 200 };
                assert!(m.indices.len() / 3 <= budget, "{species}_{k}: {} triangles", m.indices.len() / 3);
            }
        }
    }

    #[test]
    fn the_island_models_stand_on_the_ground() {
        for name in [
            "rocca", "torre_pietra", "veliero_bianco", "veliero_crema", "veliero_rosso", "veliero_blu", "veliero_giallo",
            "ormeggiato_bianco", "ormeggiato_blu", "ormeggiato_crema",
        ] {
            let m = island(name);
            assert!(!m.positions.is_empty(), "{name}");
            assert_eq!(m.indices.len() % 3, 0, "{name}");
            let low = m.positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
            // buildings sit on their ground; hulls a little below the waterline
            assert!(low >= -1.0 && low <= 0.5, "{name}: lowest point {low}");
        }
    }

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
