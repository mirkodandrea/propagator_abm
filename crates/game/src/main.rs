//! Spotorno wildfire serious game.
//!
//! The wildfire model runs in-process on the PROPAGATOR Rust core, stepped
//! from the Bevy loop — no external process, no Python at runtime. Python is
//! used only offline, to bake the scenario assets under `data/`.

mod agents;
mod browser;
mod buildings;
mod camera;
mod capture;
mod command;
mod field;
mod fire_shader;
mod fire_view;
mod frame;
mod ignition_edit;
mod inspect;
mod people;
mod pick;
mod roads;
mod sea;
mod selftest;
mod sim;
mod sky;
mod terrain_mesh;
mod textures;
mod ui;
mod units;
mod vegetation;

use bevy::core_pipeline::bloom::BloomSettings;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{CascadeShadowConfigBuilder, FogSettings};
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
        .add_plugins(fire_shader::FireShaderPlugin)
        .add_plugins(sky::SkyPlugin)
        .add_plugins(sea::SeaPlugin)
        .init_resource::<ui::UiFocus>()
        .init_resource::<ignition_edit::IgnitionTool>()
        .init_resource::<inspect::Selected>()
        .init_resource::<inspect::ClickTracker>()
        .init_resource::<command::OrderTool>()
        .init_resource::<browser::BrowserUi>()
        .add_event::<sim::SimRestarted>()
        .insert_resource(sim)
        .add_systems(
            Startup,
            (
                setup_scene,
                fire_view::setup,
                ignition_edit::setup,
                inspect::setup,
                command::setup,
                vegetation::spawn,
                buildings::spawn,
                agents::spawn,
                people::setup,
                people::mark_refuges,
                units::setup,
            ),
        )
        // Ordering that matters, and only that: the panels decide whether the
        // pointer belongs to the UI, so they run before anything that reads the
        // mouse; and the restart resets have to land before the views that
        // would otherwise read the stale state they are clearing.
        .add_systems(
            Update,
            (
                (
                    ui::panel,
                    ui::wildfire_panel,
                    inspect::panel,
                    command::panel,
                    browser::panel,
                )
                    .chain(),
                (
                    camera::controls,
                    ignition_edit::hover,
                    ignition_edit::place,
                    command::hover,
                    command::place,
                    inspect::pick_click,
                )
                    .chain()
                    .after(command::panel),
            ),
        )
        .add_systems(
            Update,
            (
                controls,
                browser::toggle,
                fire_view::layer_controls,
                command::controls.before(command::hover),
                sim::step_fire.after(ui::wildfire_panel),
                (
                    fire_view::reset,
                    buildings::reset,
                    people::reset,
                    inspect::reset,
                    units::reset,
                    command::reset,
                )
                    .after(ui::wildfire_panel),
                (
                    fire_view::update_overlay,
                    fire_view::update_flames,
                    vegetation::burn,
                    buildings::damage,
                    agents::animate_beacons,
                    agents::update_beacons,
                    people::spawn_vehicles,
                    people::update_people,
                    people::update_vehicles,
                    ignition_edit::sync_markers,
                    ignition_edit::show_markers.after(ignition_edit::sync_markers),
                    ignition_edit::update_hover,
                    inspect::update_ring,
                    units::update_units,
                    units::sync_orders,
                    units::update_work_overlay,
                    command::update_cursor,
                )
                    .after(fire_view::reset)
                    .after(buildings::reset)
                    .after(people::reset)
                    .after(inspect::reset)
                    .after(units::reset)
                    .after(command::reset),
                capture::manual,
            ),
        );

    // Unattended exercise of the wildfire controls. Runs after the resets so
    // it observes the state the views will actually see.
    if let Some(test) = selftest::from_env() {
        app.insert_resource(test).add_systems(
            Update,
            selftest::run
                .after(sim::step_fire)
                .after(fire_view::reset)
                .after(buildings::reset)
                .after(people::reset),
        );
    }

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

    // Transform, illuminance and colour are all overwritten every frame by
    // `sky::update_sky`, which computes them from real solar geometry for
    // Spotorno's latitude and the simulated clock (`SPOTORNO_START_HOUR`).
    // What is spawned here only has to exist and cast shadows.
    //
    // The cascade config is not cosmetic here: Bevy's default assumes a
    // ~1000 m scene and a 5 m first cascade, tuned for the default camera
    // examples ship with. Ours is 10.24 km with a 40 000 m far plane, and
    // with the stock config the near cascade degenerates once the camera
    // gets close to the ground — the failure mode is not blocky shadows,
    // it is a shadow-shaped region that tracks the *camera*, not the
    // terrain, because the broken cascade's coverage is defined in
    // view-space distance rather than world space. Explicit, scene-sized
    // bounds are what keep the near cascade well-conditioned down to the
    // camera's minimum zoom.
    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight { shadows_enabled: true, ..default() },
            cascade_shadow_config: CascadeShadowConfigBuilder {
                num_cascades: 4,
                minimum_distance: 2.0,
                maximum_distance: 4000.0,
                first_cascade_far_bound: 80.0,
                overlap_proportion: 0.3,
            }
            .into(),
            ..default()
        },
        sky::Sun,
    ));

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
        // Atmospheric haze, coloured to match the sky every frame by
        // `sky::update_sky` — the fallback here only matters for the one
        // frame before that system first runs.
        FogSettings::default(),
        OrbitCamera { focus, distance: 2600.0, ..default() },
    ));
}

fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<Sim>,
    mut tool: ResMut<ignition_edit::IgnitionTool>,
    mut restarted: EventWriter<sim::SimRestarted>,
) {
    if keys.just_pressed(KeyCode::Space) {
        sim.playing = !sim.playing;
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        sim.speed = (sim.speed * 2.0).min(ui::MAX_SPEED);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        sim.speed = (sim.speed / 2.0).max(ui::MIN_SPEED);
    }
    if keys.just_pressed(KeyCode::KeyE) {
        let n = sim.agents.order_evacuation_all();
        info!("general evacuation ordered: {n} households");
    }
    if keys.just_pressed(KeyCode::KeyI) {
        tool.mode = match tool.mode {
            ignition_edit::EditMode::Off => ignition_edit::EditMode::Place,
            ignition_edit::EditMode::Place => ignition_edit::EditMode::Off,
        };
    }
    // Escape leaves placing mode, which is the reflex for it, without also
    // being a second binding for anything else.
    if keys.just_pressed(KeyCode::Escape) {
        tool.mode = ignition_edit::EditMode::Off;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        match sim.restart() {
            Ok(()) => {
                restarted.send(sim::SimRestarted);
            }
            Err(e) => error!("restart failed: {e:#}"),
        }
    }
}

