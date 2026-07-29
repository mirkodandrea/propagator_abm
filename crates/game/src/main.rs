//! Spotorno wildfire serious game.
//!
//! The wildfire model runs in-process on the PROPAGATOR Rust core, stepped
//! from the Bevy loop — no external process, no Python at runtime. Python is
//! used only offline, to bake the scenario assets under `data/`.

mod agents;
mod camera;
mod capture;
mod field;
mod fire_view;
mod frame;
mod roads;
mod sim;
mod terrain_mesh;
mod textures;
mod ui;
mod vegetation;

use bevy::core_pipeline::bloom::BloomSettings;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use fire::Weather;
use scenario::Scenario;

use crate::camera::OrbitCamera;
use crate::sim::Sim;

fn main() -> anyhow::Result<()> {
    let data = std::env::var("SPOTORNO_DATA").unwrap_or_else(|_| "data".to_string());
    let scn = Scenario::load(&data)
        .map_err(|e| anyhow::anyhow!("loading scenario from {data}: {e:#}"))?;

    println!(
        "scenario: {:.1} x {:.1} km, {} fire cells, {} buildings, {} households",
        scn.world.width_m / 1000.0,
        scn.world.height_m / 1000.0,
        scn.world.fire_rows * scn.world.fire_cols,
        scn.vectors.buildings.len(),
        scn.population.households.len(),
    );

    let sim = Sim::new(scn, Weather::default(), 42)?;

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Spotorno — wildfire incident command".into(),
                resolution: (1600.0, 1000.0).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.66, 0.78)))
        // Ambient is sky bounce, so it is cool and weak; the sun carries the
        // scene. Kept modest because vegetation is drawn at sub-pixel scale
        // from altitude, and an over-lit canopy aliases into white sparkle.
        .insert_resource(AmbientLight {
            color: Color::srgb(0.72, 0.78, 0.92),
            brightness: 130.0,
        })
        .add_plugins(EguiPlugin)
        .init_resource::<ui::UiFocus>()
        .insert_resource(sim)
        .add_systems(
            Startup,
            (setup_scene, fire_view::setup, vegetation::spawn, agents::spawn),
        )
        .add_systems(
            Update,
            (
                ui::panel,
                camera::controls.after(ui::panel),
                controls,
                fire_view::layer_controls,
                sim::step_fire,
                fire_view::update_overlay,
                fire_view::update_flames,
                vegetation::burn,
                agents::animate_beacons,
                agents::update_beacons,
                capture::manual,
            ),
        );

    // Unattended capture: runs the scenario forward, grabs one frame per fire
    // layer and exits. Only active when SPOTORNO_SHOT names a directory.
    if let Some(capture) = capture::from_env() {
        app.insert_resource(capture)
            .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
            .add_systems(Update, capture::scripted.after(fire_view::update_flames));
    }

    app.run();

    Ok(())
}

fn setup_scene(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    terrain_mesh::build(&sim.scenario, &mut commands, &mut meshes, &mut materials);
    roads::build(&sim.scenario, &mut commands, &mut meshes, &mut materials);

    // Late-afternoon sun from the south-west: the hour when Ligurian fires run.
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 9_500.0,
            // Warm, low sun: the light a Ligurian fire actually runs under.
            color: Color::srgb(1.0, 0.93, 0.82),
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_rotation(Quat::from_euler(
            EulerRot::YXZ,
            -0.9,
            -0.65,
            0.0,
        )),
        ..default()
    });

    // Start the camera looking at the ignition point.
    let (p, h) = terrain_mesh::cell_ground(&sim.scenario, sim.ignition.centre);
    let focus = frame::to_bevy(p, h);
    commands.spawn((
        Camera3dBundle {
            // HDR plus bloom is what makes the flames read as light rather
            // than as orange paint: the fire layers deliberately push vertex
            // colours above 1.0, and without an HDR target that just clips.
            camera: Camera { hdr: true, ..default() },
            tonemapping: Tonemapping::TonyMcMapface,
            transform: Transform::from_xyz(focus.x, focus.y + 2000.0, focus.z + 2000.0),
            projection: PerspectiveProjection { far: 40_000.0, ..default() }.into(),
            ..default()
        },
        BloomSettings { intensity: 0.20, ..BloomSettings::NATURAL },
        OrbitCamera { focus, distance: 2600.0, ..default() },
    ));
}

fn controls(keys: Res<ButtonInput<KeyCode>>, mut sim: ResMut<Sim>) {
    if keys.just_pressed(KeyCode::Space) {
        sim.playing = !sim.playing;
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        sim.speed = (sim.speed * 2.0).min(ui::MAX_SPEED);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        sim.speed = (sim.speed / 2.0).max(ui::MIN_SPEED);
    }
}

