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

mod bench;
mod boats;
mod buildings;
mod camera;
mod coast;
mod field;
mod fields;
mod fire_shader;
mod fire_view;
mod forecast_view;
mod forecast_zone;
mod reach_zone;
mod frame;
mod kiosk;
mod landmarks;
#[cfg(feature = "mario")]
mod mario;
#[cfg(not(feature = "mario"))]
#[path = "mario_off.rs"]
mod mario;
mod models;
mod overlays;
mod pad;
mod pick;
mod playtest;
mod quality;
mod quality_live;
mod flat;
mod tactical;
mod tape;
mod materials;
mod rings;
mod ground_mask;
mod roads;
mod shader_lib;
mod sea;
mod sky;
mod sky_light;
mod sim;
mod terrain_mesh;
mod textures;
mod ui;
mod near_cover;
mod units;
mod vegetation;
mod visual_scale;
#[cfg(not(target_arch = "wasm32"))]
mod window_state;
#[cfg(target_arch = "wasm32")]
mod web_input;

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
                if cfg!(target_arch = "wasm32") { r = r.with_scale_factor_override(1.0); }
                r
            },
            mode: kiosk::window_mode(),
            // KIOSK_SNAP renders to file: a hidden window, so it never takes the focus
            // (KIOSK_SNAP_SHOW=1 shows it: macOS paces a hidden window at ~57 Hz)
            visible: std::env::var("KIOSK_SNAP").is_err() || std::env::var("KIOSK_SNAP_SHOW").is_ok(),
            focused: std::env::var("KIOSK_SNAP").is_err(),
            // in the browser: the page's canvas, filling it
            canvas: Some("#rocca".into()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            // KIOSK_FPS measures the real frame cost, so it must not be vsync-capped.
            present_mode: if std::env::var("KIOSK_FPS").is_ok() || kiosk::snap::bench_s().is_some() { bevy::window::PresentMode::AutoNoVsync } else { bevy::window::PresentMode::AutoVsync },
            ..default()
        }),
        ..default()
    }))
    .insert_resource(gfx.msaa())
    // KIOSK_SNAP renders to file: no sound either
    .insert_resource(bevy::audio::GlobalVolume::new(if std::env::var("KIOSK_SNAP").is_ok() { 0.0 } else { 1.0 }))
    .insert_resource(gfx)
    .insert_resource(bevy::pbr::DirectionalLightShadowMap { size: gfx.shadow_map })
    .add_plugins(EguiPlugin)
    .add_plugins(shader_lib::ShaderLibPlugin)
    .add_plugins(fire_shader::FireShaderPlugin)
    .add_plugins(materials::MaterialsPlugin)
    .add_plugins(near_cover::NearCoverPlugin)
    .add_plugins(flat::FlatPlugin)
    .add_plugins(forecast_zone::ZonePlugin)
    .add_plugins(reach_zone::ReachPlugin)
    .add_plugins(reach_zone::RoutePlugin)
    .add_plugins(tape::TapePlugin)
    .add_plugins(tactical::TacticalPlugin)
    // sun, sky and ambient light from the game's clock; the sea and its wind; the boats
    .add_plugins((sky::SkyPlugin, sea::SeaPlugin, boats::BoatsPlugin, landmarks::LandmarksPlugin, coast::CoastPlugin))
    .init_state::<AppState>()
    .init_resource::<kiosk::snap::SnapView>()
    .init_resource::<kiosk::snap::SnapHide>()
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
            near_cover::spawn,
            buildings::spawn,
            overlays::setup,
            units::setup,
            kiosk::marks::setup,
            kiosk::pins::setup,
            kiosk::characters::load_portraits,
        ),
    )
    .add_systems(OnExit(AppState::Playing), teardown_scene);

    #[cfg(not(target_arch = "wasm32"))]
    window_state::install(&mut app);

    // The FPS counter in the demo reads these diagnostics; KIOSK_FPS also logs them.
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin);
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var("KIOSK_FPS").is_ok() {
        app.add_plugins(bevy::diagnostic::LogDiagnosticsPlugin::filtered(vec![bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS]));
    }
    #[cfg(target_arch = "wasm32")]
    app.add_systems(PreUpdate, (
        web_input::normalize.before(bevy::input::InputSystem).before(bevy_egui::EguiSet::ProcessInput),
        web_input::finish_canceled_touch.after(bevy_egui::EguiSet::ProcessInput).before(bevy_egui::EguiSet::BeginFrame),
    ));
    if kiosk::snap::bench_s().is_some() {
        app.add_plugins(bench::BenchPlugin);
    }
    app.add_plugins(mario::MarioPlugin);
    app.add_plugins(pad::PadPlugin);
    kiosk_systems(&mut app);
    app.add_plugins(forecast_view::ForecastPlugin);
    app.add_systems(PostUpdate, kiosk::snap::hide_ui.after(bevy_egui::EguiSet::ProcessOutput));
    app.add_systems(Update, quality_live::apply_live.run_if(resource_changed::<quality::Graphics>.and_then(not(resource_added::<quality::Graphics>))));
    let exit = app.run();
    #[cfg(not(target_arch = "wasm32"))]
    if let bevy::app::AppExit::Error(code) = exit {
        std::process::exit(i32::from(code.get()));
    }
    Ok(())
}

fn setup_scene(
    mut commands: Commands,
    sim: Res<Sim>,
    gfx: Res<quality::Graphics>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain: ResMut<Assets<materials::TerrainMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    terrain_mesh::build(&sim.scn, &mut commands, &mut meshes, &mut terrain, &mut images, &gfx);
    fields::build(&sim.scn, &mut commands, &mut meshes, &mut standard);
    // Roads are drawn by the terrain shader from `ground_mask`; `roads::build`
    // (ribbon meshes) is no longer called.

    // The sun: `sky::update_sky` places and colours it from the game's clock
    // (11:00 to 14:00), so the shadows lengthen and the light warms as it runs.
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
    }, sky::Sun, bevy::render::view::RenderLayers::from_layers(&[0, terrain_mesh::SHADOW_LAYER])));

    let (p, h) = terrain_mesh::cell_ground(&sim.scn, sim.scn.world.cell_of(sim.case.ignition()));
    let distance = 2600.0;
    let focus = frame::to_bevy(p, h);
    let mut camera = commands.spawn((
        Camera3dBundle {
            camera: Camera { hdr: !cfg!(target_arch = "wasm32"), ..default() },
            tonemapping: Tonemapping::TonyMcMapface,
            color_grading: grading(),
            transform: Transform::from_xyz(focus.x, focus.y + distance * 0.77, focus.z + distance * 0.77),
            projection: PerspectiveProjection { far: 40_000.0, ..default() }.into(),
            ..default()
        },
        OrbitCamera { focus, distance, ..default() },
        // colour and reach are set every frame by `sky::update_sky`
        bevy::pbr::FogSettings::default(),
    ));
    if gfx.smaa {
        camera.insert(bevy::core_pipeline::smaa::SmaaSettings::default());
    }
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
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Update, kiosk::ui::fps.after(kiosk::ui::draw).run_if(in_state(AppState::Playing)));
    app.add_systems(Update, near_cover::update.run_if(in_state(AppState::Playing)));
    app.add_systems(
        Update,
        (
            kiosk::launch.run_if(in_state(AppState::SelectingScenario)),
            (kiosk::ui::draw, kiosk::step, kiosk::snap::drive, kiosk::view::camera.run_if(map_camera_allowed), kiosk::snap::hold, playtest::observe)
                .chain()
                .run_if(in_state(AppState::Playing)),
            kiosk::snap::hide_marks
                .after(kiosk::pins::update)
                .after(kiosk::marks::update_teams)
                .after(forecast_view::update)
                .run_if(in_state(AppState::Playing)),
            (fire_view::reset, buildings::reset, overlays::reset, units::reset, kiosk::marks::reset)
                .after(kiosk::step)
                .run_if(in_state(AppState::Playing)),
            (
                fire_view::update_overlay,
                fire_view::update_flames,
                (vegetation::burn, vegetation::lod, vegetation::fire_clock),
                buildings::damage,
                units::update_units,
                units::animate_units,
                overlays::pulse_rings,
                rings::spin,
                overlays::no_ui_shadows,
                units::sync_orders,
                units::update_work_overlay,
                units::animate_spray,
                kiosk::marks::update_hexes,
                kiosk::marks::update_orders,
                kiosk::marks::update_closed,
                kiosk::marks::update_teams,
                kiosk::marks::bob_zone,
                kiosk::pins::update,
            )
                .after(fire_view::reset)
                .after(buildings::reset)
                .after(units::reset)
                .after(kiosk::marks::reset)
                .run_if(in_state(AppState::Playing)),
        ),
    );
}

fn map_camera_allowed(mario: Res<mario::Playground>) -> bool {
    !mario.active
}

/// The film look on top of the tone mapper: a touch more contrast in the
/// midtones and saturation after it, so the sunlit island does not read
/// washed out (TonyMcMapface desaturates bright, saturated colours). Tuned on
/// snapshots against the sky's fill (`sky_light`), 2026-10-20.
fn grading() -> bevy::render::view::ColorGrading {
    use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};
    let mut g = ColorGrading::with_identical_sections(
        ColorGradingGlobal { post_saturation: 1.15, ..default() },
        ColorGradingSection::default(),
    );
    g.midtones.contrast = 1.15;
    g
}
