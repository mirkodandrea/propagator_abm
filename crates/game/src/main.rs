//! The wildfire kiosk demo: the one and only mode of this binary.
//!
//! The model runs in-process on the PROPAGATOR Rust core, stepped from the Bevy
//! loop. Python is used only offline, to bake the scenario assets under `data/`.

use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    /// No town loaded: `kiosk::launch` loads the one the session is on.
    SelectingScenario,
    Playing,
}

mod agents;
mod buildings;
mod camera;
mod command;
mod field;
mod fire_shader;
mod fire_view;
mod frame;
mod kiosk;
mod library;
mod models;
mod overlays;
mod people;
mod plinth;
mod pick;
mod retro;
mod rings;
mod roads;
mod sim;
mod terrain_mesh;
mod toy;
mod textures;
mod ui;
mod units;
mod vegetation;

use bevy::core_pipeline::bloom::BloomSettings;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::CascadeShadowConfigBuilder;
use bevy_egui::EguiPlugin;

use crate::camera::OrbitCamera;
use crate::sim::Sim;

/// Where the baked scenarios live (`SPOTORNO_DATA`, default `data`).
#[derive(Resource)]
pub struct DataPath(pub std::path::PathBuf);

fn main() -> anyhow::Result<()> {
    let data = std::env::var("SPOTORNO_DATA").unwrap_or_else(|_| "data".to_string());

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: kiosk::strings_it::TITLE.into(),
            resolution: (1600.0, 1000.0).into(),
            mode: kiosk::window_mode(),
            // KIOSK_FPS measures the real frame cost, so it must not be vsync-capped.
            present_mode: if std::env::var("KIOSK_FPS").is_ok() { bevy::window::PresentMode::AutoNoVsync } else { bevy::window::PresentMode::AutoVsync },
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::srgb(0.80, 0.76, 0.70)))
    .insert_resource(AmbientLight { color: Color::srgb(0.85, 0.88, 1.0), brightness: 420.0 })
    .add_plugins(EguiPlugin)
    .add_plugins(fire_shader::FireShaderPlugin)
    .add_plugins(retro::RetroShaderPlugin)
    .add_plugins(library::LibraryPlugin)
    .init_state::<AppState>()
    .init_resource::<ui::UiFocus>()
    .init_resource::<command::OrderTool>()
    .add_event::<sim::SimRestarted>()
    .insert_resource(DataPath(std::path::PathBuf::from(&data)))
    // Everything that spawns a scene entity or caches a per-scenario handle
    // builds on entry and is torn down on exit, so a town change is a genuine
    // round trip rather than a second scene laid on top of the first.
    .add_systems(
        OnEnter(AppState::Playing),
        (
            setup_scene,
            fire_view::setup,
            command::setup,
            vegetation::spawn,
            buildings::spawn,
            agents::spawn,
            people::setup,
            overlays::setup,
            people::mark_refuges,
            units::setup,
        ),
    )
    .add_systems(OnExit(AppState::Playing), teardown_scene);

    if std::env::var("KIOSK_FPS").is_ok() {
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
            .add_plugins(bevy::diagnostic::LogDiagnosticsPlugin::filtered(vec![bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS]));
    }
    kiosk_systems(&mut app);
    app.run();
    Ok(())
}

fn setup_scene(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<retro::RetroMaterial>>,
) {
    terrain_mesh::build(&sim.scenario, &mut commands, &mut meshes, &mut materials);
    roads::build(&sim.scenario, &mut commands, &mut meshes, &mut materials);
    plinth::build(&sim.scenario, &mut commands, &mut meshes, &mut materials);

    // One fixed sun: no clock, no day/night (spec decisions).
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight { shadows_enabled: true, illuminance: 11_000.0, color: Color::srgb(1.0, 0.93, 0.80), ..default() },
        transform: Transform::from_xyz(-0.55, 0.85, 0.45).looking_at(Vec3::ZERO, Vec3::Y),
        cascade_shadow_config: CascadeShadowConfigBuilder {
            num_cascades: 4,
            minimum_distance: 2.0,
            maximum_distance: 4000.0,
            first_cascade_far_bound: 80.0,
            overlap_proportion: 0.3,
        }
        .into(),
        ..default()
    });

    let (p, h) = terrain_mesh::cell_ground(&sim.scenario, sim.ignition.centre);
    let distance = 2600.0;
    let focus = frame::to_bevy(p, h);
    commands.spawn((
        Camera3dBundle {
            camera: Camera { hdr: true, ..default() },
            tonemapping: Tonemapping::TonyMcMapface,
            transform: Transform::from_xyz(focus.x, focus.y + distance * 0.77, focus.z + distance * 0.77),
            projection: PerspectiveProjection { far: 40_000.0, ..default() }.into(),
            ..default()
        },
        BloomSettings { intensity: 0.20, ..BloomSettings::NATURAL },
        OrbitCamera { focus, distance, ..default() },
        bevy::core_pipeline::prepass::DepthPrepass,
        bevy::core_pipeline::dof::DepthOfFieldSettings {
            mode: bevy::core_pipeline::dof::DepthOfFieldMode::Gaussian,
            focal_distance: distance,
            sensor_height: 0.01866,
            aperture_f_stops: 0.012,
            max_circle_of_confusion_diameter: 48.0,
            max_depth: 12_000.0,
        },
    ));
}

/// Empty the world on the way back to the loading state.
///
/// Everything the scene is made of is a root entity with a `Transform`, and
/// nothing else in the app is, so the query *is* the definition of "the scene"
/// and cannot go stale when something new is spawned.
fn teardown_scene(
    mut commands: Commands,
    roots: Query<Entity, (With<Transform>, Without<Parent>)>,
    mut order: ResMut<command::OrderTool>,
) {
    *order = command::OrderTool::default();
    for e in &roots {
        commands.entity(e).despawn_recursive();
    }
}

/// The kiosk's schedule.
fn kiosk_systems(app: &mut App) {
    app.insert_resource(kiosk::Kiosk::from_env()).add_systems(
        Update,
        (
            kiosk::launch.run_if(in_state(AppState::SelectingScenario)),
            (
                kiosk::activity,
                kiosk::draw,
                kiosk::step,
                kiosk::camera,
                kiosk::shots,
                command::hover,
                command::place,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
            (
                fire_view::reset,
                buildings::reset,
                people::reset,
                overlays::reset,
                units::reset,
                command::reset,
            )
                .after(kiosk::step)
                .run_if(in_state(AppState::Playing)),
            (
                fire_view::update_overlay,
                fire_view::update_flames,
                vegetation::burn,
                buildings::damage,
                people::spawn_vehicles,
                people::update_people,
                people::update_vehicles,
                people::bob_people.after(people::update_people),
                units::update_units,
                overlays::update_markers,
                overlays::update_wind,
                units::sync_orders,
                units::update_work_overlay,
                command::update_cursor,
            )
                .after(fire_view::reset)
                .after(buildings::reset)
                .after(people::reset)
                .after(units::reset)
                .after(command::reset)
                .run_if(in_state(AppState::Playing)),
        ),
    );
}
