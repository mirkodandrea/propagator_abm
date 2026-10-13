//! The wildfire kiosk demo: the one and only mode of this binary.
//!
//! The model runs in-process on the PROPAGATOR Rust core, stepped from the Bevy
//! loop. Python is used only offline, to bake the scenario assets under `data/`.

use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    /// Nothing loaded yet: `kiosk::launch` loads the territory and the first game.
    SelectingScenario,
    Playing,
}

mod agents;
mod boats;
mod buildings;
mod camera;
mod field;
mod fire_shader;
mod fire_view;
mod frame;
mod kiosk;
mod landmarks;
mod models;
#[cfg(not(target_arch = "wasm32"))]
mod mario;
mod overlays;
mod life;
mod people;
mod pick;
mod quality;
mod retro;
mod rings;
mod roads;
mod sea;
mod sky;
mod sim;
mod terrain_mesh;
mod toy;
mod textures;
mod ui;
mod units;
mod vegetation;
mod visual_scale;

use bevy::core_pipeline::bloom::BloomSettings;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::CascadeShadowConfigBuilder;
use bevy_egui::EguiPlugin;

use crate::camera::OrbitCamera;
use crate::sim::Sim;

/// Where the territory lives (`ROCCA_DATA`, default `data`).
#[derive(Resource)]
pub struct DataPath(pub std::path::PathBuf);

fn main() -> anyhow::Result<()> {
    let data = std::env::var("ROCCA_DATA").unwrap_or_else(|_| "data".to_string());
    let gfx = quality::Graphics::from_env();
    info!("grafica: {gfx:?}");

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: kiosk::TITLE.into(),
            // KIOSK_SCALE=1 renders one pixel per point on a Retina screen,
            // as a 1080p kiosk monitor would.
            resolution: {
                // KIOSK_SIZE=900x600 checks the layout on a small screen.
                let (w, h) = std::env::var("KIOSK_SIZE").ok()
                    .and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?))))
                    .unwrap_or((1600.0, 1000.0));
                let mut r: bevy::window::WindowResolution = (w, h).into();
                if let Some(s) = std::env::var("KIOSK_SCALE").ok().and_then(|v| v.parse::<f32>().ok()) {
                    r = r.with_scale_factor_override(s);
                }
                r
            },
            mode: kiosk::window_mode(),
            // in the browser: the page's canvas, filling it
            canvas: Some("#rocca".into()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            // KIOSK_FPS measures the real frame cost, so it must not be vsync-capped.
            present_mode: if std::env::var("KIOSK_FPS").is_ok() { bevy::window::PresentMode::AutoNoVsync } else { bevy::window::PresentMode::AutoVsync },
            ..default()
        }),
        ..default()
    }))
    .insert_resource(gfx.msaa())
    .insert_resource(gfx)
    .insert_resource(bevy::pbr::DirectionalLightShadowMap { size: gfx.shadow_map })
    .add_plugins(EguiPlugin)
    .add_plugins(fire_shader::FireShaderPlugin)
    .add_plugins(retro::RetroShaderPlugin)
    // sun, sky and ambient light from the game's clock; the sea and its wind; the boats
    .add_plugins((sky::SkyPlugin, sea::SeaPlugin, boats::BoatsPlugin, landmarks::LandmarksPlugin))
    .init_state::<AppState>()
    .init_resource::<kiosk::snap::SnapView>()
    .init_resource::<ui::UiFocus>()
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
            vegetation::spawn,
            buildings::spawn,
            agents::spawn,
            people::setup,
            overlays::setup,
            people::mark_refuges,
            life::setup,
            units::setup,
            kiosk::ui::load_logo,
            kiosk::characters::load_portraits,
        ),
    )
    .add_systems(OnExit(AppState::Playing), teardown_scene);

    if std::env::var("KIOSK_FPS").is_ok() {
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
            .add_plugins(bevy::diagnostic::LogDiagnosticsPlugin::filtered(vec![bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS]));
    }
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(mario::MarioPlugin);
    if std::env::var_os("MARIO_DEMO").is_some() {
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 30.0),
        ));
    }
    kiosk_systems(&mut app);
    app.run();
    Ok(())
}

fn setup_scene(
    mut commands: Commands,
    sim: Res<Sim>,
    gfx: Res<quality::Graphics>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<retro::RetroMaterial>>,
) {
    terrain_mesh::build(&sim.scn, &mut commands, &mut meshes, &mut materials);
    roads::build(&sim.scn, &mut commands, &mut meshes, &mut materials);

    // The sun: `sky::update_sky` places and colours it from the game's clock
    // (14:00 to 17:00), so the shadows lengthen and the light warms as it runs.
    commands.spawn((DirectionalLightBundle {
        directional_light: DirectionalLight { shadows_enabled: gfx.shadows, ..default() },
        cascade_shadow_config: CascadeShadowConfigBuilder {
            num_cascades: gfx.cascades,
            minimum_distance: 2.0,
            maximum_distance: gfx.shadow_distance,
            first_cascade_far_bound: 80.0,
            overlap_proportion: 0.3,
        }
        .into(),
        ..default()
    }, sky::Sun));

    let (p, h) = terrain_mesh::cell_ground(&sim.scn, sim.scn.world.cell_of(sim.case.ignition()));
    let distance = 2600.0;
    let focus = frame::to_bevy(p, h);
    let mut camera = commands.spawn((
        Camera3dBundle {
            camera: Camera { hdr: true, ..default() },
            tonemapping: Tonemapping::TonyMcMapface,
            transform: Transform::from_xyz(focus.x, focus.y + distance * 0.77, focus.z + distance * 0.77),
            projection: PerspectiveProjection { far: 40_000.0, ..default() }.into(),
            ..default()
        },
        OrbitCamera { focus, distance, ..default() },
        // colour and reach are set every frame by `sky::update_sky`
        bevy::pbr::FogSettings::default(),
    ));
    if gfx.bloom {
        camera.insert(BloomSettings { intensity: 0.20, ..BloomSettings::NATURAL });
    }
    // The depth prepass is there for the depth of field only.
    if gfx.dof {
        camera.insert((
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
}

/// Empty the world on the way back to the loading state.
///
/// Everything the scene is made of is a root entity with a `Transform`, and
/// nothing else in the app is, so the query *is* the definition of "the scene"
/// and cannot go stale when something new is spawned.
fn teardown_scene(
    mut commands: Commands,
    roots: Query<Entity, (With<Transform>, Without<Parent>)>,
) {
    for e in &roots {
        commands.entity(e).despawn_recursive();
    }
}

/// The kiosk's schedule.
fn kiosk_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            kiosk::launch.run_if(in_state(AppState::SelectingScenario)),
            (kiosk::ui::draw, kiosk::step, kiosk::snap::drive, kiosk::view::camera.run_if(map_camera_allowed), kiosk::snap::hold, kiosk::shots)
                .chain()
                .run_if(in_state(AppState::Playing)),
            kiosk::view::hide_game_overlays
                .after(overlays::update_markers)
                .after(overlays::update_wind)
                .after(overlays::update_orders)
                .after(overlays::update_routes)
                .after(units::sync_orders)
                .after(units::update_work_overlay)
                .run_if(in_state(AppState::Playing)),
            (fire_view::reset, buildings::reset, people::reset, overlays::reset, units::reset)
                .after(kiosk::step)
                .run_if(in_state(AppState::Playing)),
            (
                fire_view::update_overlay,
                fire_view::update_flames,
                (vegetation::burn, vegetation::lod),
                buildings::damage,
                people::spawn_vehicles,
                people::update_people,
                people::update_vehicles,
                life::update.run_if(resource_exists::<life::LifeAssets>),
                units::update_units,
                units::update_teams,
                overlays::update_markers,
                overlays::update_wind,
                overlays::animate_wind,
                overlays::update_orders,
                overlays::pulse_rings,
                rings::spin,
                overlays::no_ui_shadows,
                overlays::update_routes,
                units::sync_orders,
                units::update_work_overlay,
            )
                .after(fire_view::reset)
                .after(buildings::reset)
                .after(people::reset)
                .after(units::reset)
                .run_if(in_state(AppState::Playing)),
        ),
    );
}

fn map_camera_allowed(
    #[cfg(not(target_arch = "wasm32"))] mario: Res<mario::Playground>,
) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    { !mario.active }
    #[cfg(target_arch = "wasm32")]
    { true }
}
