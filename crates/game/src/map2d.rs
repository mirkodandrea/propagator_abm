//! Flat tactical map. Separate, unlit batches replace the scene geometry;
//! the existing camera and world frame keep inspection and orders aligned.
use crate::{camera::OrbitCamera, fire_view::FireLayer, frame, sim::Sim};
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::render::{
    mesh::{Indices, PrimitiveTopology},
    render_asset::RenderAssetUsages,
    view::RenderLayers,
};
use scenario::{Cell, Pos};

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub enum Renderer {
    Scene3d,
    Map2d,
}

impl Default for Renderer {
    fn default() -> Self {
        if std::env::var("SPOTORNO_RENDERER").as_deref() == Ok("2d") {
            Self::Map2d
        } else {
            Self::Scene3d
        }
    }
}

#[derive(Component)]
pub struct DynamicMap;

#[derive(Default)]
struct Batch {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}
impl Batch {
    fn quad(&mut self, points: [Vec2; 4], height: f32, color: Color) {
        let base = self.positions.len() as u32;
        for p in points {
            self.positions
                .push(frame::to_bevy(Pos { x: p.x, y: p.y }, height).to_array());
            self.colors.push(color.to_linear().to_f32_array());
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    fn square(&mut self, p: Pos, radius: f32, height: f32, color: Color) {
        let p = Vec2::new(p.x, p.y);
        self.quad(
            [
                p + Vec2::new(-radius, -radius),
                p + Vec2::new(radius, -radius),
                p + Vec2::new(radius, radius),
                p + Vec2::new(-radius, radius),
            ],
            height,
            color,
        );
    }
    fn line(&mut self, a: Vec2, b: Vec2, width: f32, height: f32, color: Color) {
        let d = (b - a).normalize_or_zero().perp() * width * 0.5;
        self.quad([a - d, b - d, b + d, a + d], height, color);
    }
    fn mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        let normals = vec![[0.0, 1.0, 0.0]; self.positions.len()];
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

pub fn setup(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let scn = &sim.scenario;
    let mut map = Batch::default();
    map.quad(
        [
            Vec2::ZERO,
            Vec2::new(scn.world.width_m, 0.0),
            Vec2::new(scn.world.width_m, scn.world.height_m),
            Vec2::new(0.0, scn.world.height_m),
        ],
        0.0,
        Color::srgb(0.16, 0.23, 0.21),
    );
    for road in &scn.vectors.roads {
        for pair in road.line.windows(2) {
            map.line(
                Vec2::from_array(pair[0]),
                Vec2::from_array(pair[1]),
                if road.drivable { 7.0 } else { 3.0 },
                2.0,
                if road.drivable {
                    Color::srgb(0.6, 0.64, 0.62)
                } else {
                    Color::srgb(0.34, 0.4, 0.34)
                },
            );
        }
    }
    for building in &scn.vectors.buildings {
        for pair in building.ring.windows(2) {
            map.line(
                Vec2::from_array(pair[0]),
                Vec2::from_array(pair[1]),
                2.0,
                3.0,
                Color::srgb(0.65, 0.65, 0.55),
            );
        }
    }
    let material = materials.add(StandardMaterial {
        unlit: true,
        fog_enabled: false,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(map.mesh()),
            material: material.clone(),
            ..default()
        },
        RenderLayers::layer(1),
    ));
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Batch::default().mesh()),
            material,
            ..default()
        },
        RenderLayers::layer(1),
        DynamicMap,
    ));
}

pub fn sync_camera(
    mut commands: Commands,
    renderer: Res<Renderer>,
    mut mode: ResMut<crate::camera::CameraMode>,
    mut cameras: Query<(
        Entity,
        &OrbitCamera,
        &mut Transform,
        &mut Projection,
        &mut bevy::core_pipeline::tonemapping::Tonemapping,
    )>,
) {
    for (entity, orbit, mut transform, mut projection, mut tone) in &mut cameras {
        if *renderer == Renderer::Map2d {
            if let crate::camera::CameraMode::FirstPerson(target) = *mode {
                *mode = crate::camera::CameraMode::Follow(target);
            }
            // Orthographic rays share x/y at every elevation, including the
            // heightfield used by existing map tools and elevated map rings.
            let focus = frame::to_bevy(frame::to_world(orbit.focus), 0.0);
            *transform = Transform::from_translation(focus + Vec3::Y * 20_000.0)
                .looking_at(focus, Vec3::NEG_Z);
            *projection = OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical(orbit.distance),
                near: 0.0,
                far: 40_000.0,
                ..default()
            }
            .into();
            *tone = bevy::core_pipeline::tonemapping::Tonemapping::None;
            commands
                .entity(entity)
                .insert(RenderLayers::layer(1))
                .remove::<bevy::pbr::FogSettings>();
        } else if matches!(*projection, Projection::Orthographic(_)) {
            *projection = PerspectiveProjection {
                far: 40_000.0,
                ..default()
            }
            .into();
            *tone = bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface;
            commands
                .entity(entity)
                .insert((RenderLayers::layer(0), bevy::pbr::FogSettings::default()));
        }
    }
}

pub fn update(
    renderer: Res<Renderer>,
    sim: Res<Sim>,
    layer: Res<FireLayer>,
    cameras: Query<&OrbitCamera>,
    maps: Query<&Handle<Mesh>, With<DynamicMap>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if *renderer != Renderer::Map2d {
        return;
    }
    let Ok(handle) = maps.get_single() else {
        return;
    };
    let mut map = Batch::default();
    let w = &sim.scenario.world;
    for (i, state) in sim.fire.state().iter().enumerate() {
        let risk = sim.fire.hazard().as_slice()[i];
        if *state == fire::CellFire::Unburnt && (*layer != FireLayer::Hazard || risk <= 0.01) {
            continue;
        }
        let rgb = match *layer {
            FireLayer::Flames => {
                if *state == fire::CellFire::Burning {
                    [1.0, 0.25, 0.03]
                } else {
                    [0.07, 0.06, 0.06]
                }
            }
            FireLayer::Intensity => crate::fire_view::intensity_color(sim.fire.intensity()[i]),
            FireLayer::Arrival => crate::fire_view::arrival_color(
                sim.fire.arrival_times()[i] as f32,
                sim.fire.time_s() as f32,
            ),
            FireLayer::Hazard => {
                if *state == fire::CellFire::Unburnt {
                    crate::fire_view::hazard_color(risk)
                } else {
                    [0.1, 0.09, 0.09]
                }
            }
        };
        map.square(
            w.centre_of(Cell {
                row: i / w.fire_cols,
                col: i % w.fire_cols,
            }),
            w.cellsize * 0.5,
            4.0,
            Color::linear_rgb(rgb[0], rgb[1], rgb[2]),
        );
    }
    let size = cameras
        .get_single()
        .map(|c| c.distance * 0.003)
        .unwrap_or(10.0)
        .max(2.0);
    for refuge in &sim.agents.refuges {
        map.square(refuge.pos, size * 2.0, 5.0, Color::srgb(0.2, 0.95, 0.85));
        map.square(refuge.pos, size, 5.1, Color::srgb(0.06, 0.2, 0.18));
    }
    for h in &sim.agents.households {
        map.square(h.home, size, 6.0, crate::agents::status_color(h.status));
    }
    for p in &sim.agents.people {
        if matches!(
            p.status,
            scenario::population::Status::Evacuating | scenario::population::Status::Trapped
        ) {
            map.square(p.pos, size, 7.0, crate::agents::status_color(p.status));
        }
    }
    for t in &sim.agents.travellers {
        if matches!(
            t.state,
            abm::TravelState::Approaching | abm::TravelState::OnNetwork
        ) {
            map.square(t.pos, size * 1.2, 8.0, Color::srgb(0.3, 0.7, 0.98));
        }
    }
    for u in &sim.crews.units {
        if u.on_scene() || u.state == abm::suppression::UnitState::Lost {
            map.square(u.pos, size * 2.0, 9.0, Color::WHITE);
            map.square(
                u.pos,
                size * 1.5,
                9.1,
                crate::units::colour(u.kind, u.state),
            );
        }
    }
    if let Some(mesh) = meshes.get_mut(handle) {
        *mesh = map.mesh();
    }
}

/// The existing command/selection geometry remains useful on the flat map.
pub fn share_markers(
    mut commands: Commands,
    markers: Query<
        Entity,
        (
            Without<RenderLayers>,
            Or<(
                With<crate::inspect::SelectionRing>,
                With<crate::ignition_edit::IgnitionMarker>,
                With<crate::ignition_edit::HoverRing>,
                With<crate::command::OrderCursor>,
                With<crate::command::EvacuationPreview>,
                With<crate::units::OrderMarker>,
                With<crate::units::WorkOverlay>,
            )>,
        ),
    >,
) {
    for entity in &markers {
        commands
            .entity(entity)
            .insert(RenderLayers::from_layers(&[0, 1]));
    }
}

/// Keep the costly visual fire meshes idle while the map draws cell data.
pub fn scene3d(renderer: Res<Renderer>) -> bool {
    *renderer == Renderer::Scene3d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_projection_preserves_orbit_and_restores_perspective() {
        let mut app = App::new();
        app.insert_resource(Renderer::Map2d)
            .init_resource::<crate::camera::CameraMode>()
            .add_systems(Update, sync_camera);
        let entity = app
            .world_mut()
            .spawn((
                OrbitCamera {
                    focus: frame::to_bevy(Pos { x: 300.0, y: 400.0 }, 50.0),
                    ..default()
                },
                Transform::default(),
                Projection::Perspective(PerspectiveProjection::default()),
                bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface,
            ))
            .id();
        app.update();
        let world = app.world();
        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(
            frame::to_world(transform.translation),
            Pos { x: 300.0, y: 400.0 }
        );
        assert!(transform.forward().dot(Vec3::NEG_Y) > 0.999);
        assert!(transform.up().dot(Vec3::NEG_Z) > 0.999);
        assert!(matches!(
            world.get::<Projection>(entity),
            Some(Projection::Orthographic(_))
        ));
        assert_eq!(
            world.get::<RenderLayers>(entity),
            Some(&RenderLayers::layer(1))
        );
        *app.world_mut().resource_mut::<Renderer>() = Renderer::Scene3d;
        app.update();
        assert!(matches!(
            app.world().get::<Projection>(entity),
            Some(Projection::Perspective(_))
        ));
        assert_eq!(
            app.world().get::<RenderLayers>(entity),
            Some(&RenderLayers::layer(0))
        );
        assert_eq!(app.world().get::<OrbitCamera>(entity).unwrap().yaw, 0.35);
    }
}
