//! Spotorno wildfire serious game.
//!
//! The wildfire model runs in-process on the PROPAGATOR Rust core, stepped
//! from the Bevy loop — no external process, no Python at runtime. Python is
//! used only offline, to bake the scenario assets under `data/`.

use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    SelectingScenario,
    Playing,
}

mod agents;
#[cfg(not(target_arch = "wasm32"))]
mod api;
mod browser;
mod buildings;
mod camera;
mod capture;
mod command;
mod composer;
mod far_terrain;
mod field;
mod fire_shader;
mod fire_view;
mod frame;
mod history;
mod ignition_edit;
mod inspect;
mod interview;
mod menu;
mod people;
mod pick;
mod retro;
mod roads;
mod scenario_selector;
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
use bevy_egui::EguiPlugin;

use crate::camera::OrbitCamera;
use crate::sim::Sim;

fn main() -> anyhow::Result<()> {
    // Before anything reads an environment variable, and before
    // `Interview`'s resource is built from one: Cargo does not load `.env`, so
    // a key sitting in one that nothing reads is indistinguishable from a key
    // the game ignored. Existing variables still win — see
    // `chat::config::load_dotenv`.
    if let Some(path) = chat::config::load_dotenv() {
        println!("loaded environment from {path}");
    }

    let data = std::env::var("SPOTORNO_DATA").unwrap_or_else(|_| "data".to_string());
    let data_path_resource = scenario_selector::DataPath(std::path::PathBuf::from(&data));

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Wildfire incident — select scenario".into(),
            #[cfg(target_arch = "wasm32")]
            canvas: Some("#spotorno".into()),
            #[cfg(target_arch = "wasm32")]
            fit_canvas_to_parent: true,
            #[cfg(target_arch = "wasm32")]
            prevent_default_event_handling: true,
            #[cfg(target_arch = "wasm32")]
            resolution: (1280.0, 720.0).into(),
            #[cfg(not(target_arch = "wasm32"))]
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
    .add_plugins(retro::RetroShaderPlugin)
    .add_plugins(sky::SkyPlugin)
    .add_plugins(sea::SeaPlugin)
    .add_plugins(far_terrain::FarTerrainPlugin)
    .add_plugins(composer::ComposerPlugin)
    .init_state::<AppState>()
    .init_resource::<ui::UiFocus>()
    .init_resource::<ui::HelpUi>()
    .init_resource::<ignition_edit::IgnitionTool>()
    .insert_resource(inspect::Selected::from_env())
    .init_resource::<inspect::ClickTracker>()
    .init_resource::<inspect::HoveredTarget>()
    .init_resource::<command::OrderTool>()
    .init_resource::<browser::BrowserUi>()
    .init_resource::<camera::CameraMode>()
    .init_resource::<camera::FirstPersonLook>()
    .init_resource::<buildings::HoveredHousehold>()
    .init_resource::<scenario_selector::ScenarioSelector>()
    .init_resource::<ui::PanelState>()
    .init_resource::<interview::Interview>()
    .add_event::<sim::SimRestarted>()
    .insert_resource(data_path_resource)
    .add_systems(Startup, (scenario_selector::init_selector, ui::setup_style))
    // Everything that spawns a scene entity or caches a per-scenario handle
    // builds on entry and is torn down on exit, so "Load scenario…" is a
    // genuine round trip rather than a second scene laid on top of the first.
    // The four that used to sit in `Startup` (the fire overlay, the ignition
    // hover ring, the selection ring, the order cursor's materials) are here
    // for that reason: `teardown_scene` cannot leave behind a marker whose
    // spawner will never run again.
    .add_systems(
        OnEnter(AppState::Playing),
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
            interview::open_from_env,
        ),
    )
    .add_systems(OnExit(AppState::Playing), teardown_scene)
    // Ordering that matters, and only that: `menu::menubar` decides who owns
    // the pointer and the keyboard this frame, so it runs before every other
    // panel and before every shortcut system; the docked panels have to all
    // land before `sync_viewport` reads what space is left for the 3D camera;
    // and the restart resets have to land before the views that would
    // otherwise read the stale state they are clearing.
    .add_systems(
        Update,
        (
            scenario_selector::handle_launch_selection
                .run_if(in_state(AppState::SelectingScenario))
                .before(scenario_selector::show_selector_ui),
            scenario_selector::show_selector_ui,
            (
                menu::menubar,
                ui::panel,
                ui::help_panel,
                ui::shortcuts_panel,
                ui::dock,
                inspect::panel,
                interview::settings_window,
                ui::sync_viewport,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
            (
                camera::validate_mode,
                camera::controls,
                ignition_edit::hover,
                ignition_edit::place,
                command::hover,
                command::place,
                inspect::hover_map,
                ui::map_hud,
                inspect::pick_click,
                buildings::hover,
            )
                .chain()
                .after(inspect::panel)
                .run_if(in_state(AppState::Playing)),
        ),
    )
    .add_systems(
        Update,
        (
            // Every shortcut system reads `UiFocus::keyboard`, which the menu
            // bar wrote this frame — so they must all run after it.
            (
                controls,
                browser::toggle,
                fire_view::layer_controls,
                command::controls.before(command::hover),
                interview::shortcut,
            )
                .after(menu::menubar)
                .run_if(in_state(AppState::Playing)),
            // Draining the worker's channel is not a shortcut and not a panel:
            // it runs whether or not the window is open, so an answer that
            // lands after the panel was closed is still filed against the
            // transcript rather than lost.
            interview::poll
                .before(ui::panel)
                .run_if(in_state(AppState::Playing)),
            // Inert unless `SPOTORNO_INTERVIEW=selftest` asked for it, and
            // after `poll` so it reads a transcript the worker has already
            // been drained into.
            interview::selftest
                .after(interview::poll)
                .run_if(in_state(AppState::Playing)),
            sim::step_fire
                .after(ui::dock)
                .run_if(in_state(AppState::Playing)),
            (
                fire_view::reset,
                buildings::reset,
                people::reset,
                inspect::reset,
                units::reset,
                command::reset,
                camera::reset,
                interview::reset,
            )
                .after(ui::dock)
                .run_if(in_state(AppState::Playing)),
            (
                fire_view::update_overlay,
                fire_view::update_flames,
                vegetation::burn,
                buildings::damage,
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
                .after(command::reset)
                .run_if(in_state(AppState::Playing)),
            capture::manual.run_if(in_state(AppState::Playing)),
            apply_behaviour
                .after(sim::step_fire)
                .run_if(in_state(AppState::Playing)),
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
                .after(people::reset)
                .run_if(in_state(AppState::Playing)),
        );
    }

    // Unattended capture: runs the scenario forward, grabs one frame per fire
    // layer and exits. Only active when SPOTORNO_SHOT names a directory.
    if let Some(capture) = capture::from_env() {
        app.insert_resource(capture)
            .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
            .add_systems(
                Update,
                capture::scripted
                    .after(fire_view::update_flames)
                    .run_if(in_state(AppState::Playing)),
            );
    }

    // The local control/inspection API: a background thread accepting plain
    // HTTP on localhost, for `tools/mcp` or a `curl` prompt to read the agent
    // history and drive the incident. `api::setup` starts the listener before
    // any scenario is loaded, and `api::serve` — unconditional on `AppState`,
    // unlike almost every other system here — answers each queued request
    // whether or not one is loaded yet. See `crate::api` for why this does
    // not exist on the wasm32 build.
    #[cfg(not(target_arch = "wasm32"))]
    {
        app.add_systems(Startup, api::setup);
        app.add_systems(Update, api::serve);
    }

    app.run();

    Ok(())
}

/// Rebuild the agent model on whatever the composer currently holds.
///
/// Routed through `SimRestarted` like every other restart, so the latched view
/// state — charred buildings, the plume, the vehicle entities — is cleared by
/// the same three `reset` systems (see finding 21 in CLAUDE.md). A behaviour
/// change that left the old run's cars parked on the map would be the same bug
/// in a new coat.
fn apply_behaviour(
    mut events: EventReader<composer::ApplyBehaviour>,
    mut sim: ResMut<Sim>,
    mut composer: ResMut<composer::Composer>,
    mut restarted: EventWriter<sim::SimRestarted>,
) {
    if events.is_empty() {
        return;
    }
    events.clear();

    let lib = composer.lib.clone();
    let described = [
        (lib.assignment().len(), "household"),
        (lib.person_assignment().len(), "person"),
        (lib.unit_assignment().len(), "unit"),
    ]
    .into_iter()
    .map(|(n, what)| format!("{n} {what} profile(s)"))
    .collect::<Vec<_>>()
    .join(", ");

    match sim.apply_behaviour(lib) {
        Ok(()) => {
            restarted.send(sim::SimRestarted);
            composer.dirty = false;
            composer.set_status(format!("restarted on {described}"));
            info!("agent behaviour applied: {described}");
        }
        Err(e) => {
            composer.set_error(format!("{e:#}"));
            error!("applying behaviour failed: {e:#}");
        }
    }
}

fn setup_scene(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<retro::RetroMaterial>>,
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
            directional_light: DirectionalLight {
                shadows_enabled: true,
                ..default()
            },
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

    // Real incidents open on the ignition. A synthetic lab instead opens on
    // the whole finite stage: its road topology and the relative placement of
    // agents, refuges and fire are the experiment, and centring one of those
    // parts can put the defining route outside the initial viewport.
    let (p, h, distance) = if sim.scenario.is_dev() {
        let p = scenario::Pos {
            x: sim.scenario.world.width_m * 0.5,
            y: sim.scenario.world.height_m * 0.5,
        };
        let h = sim.scenario.terrain.height_at(p);
        let distance = sim.scenario.world.width_m.max(sim.scenario.world.height_m) * 1.15;
        (p, h, distance.max(1400.0))
    } else {
        let (p, h) = terrain_mesh::cell_ground(&sim.scenario, sim.ignition.centre);
        (p, h, 2600.0)
    };
    let focus = frame::to_bevy(p, h);
    commands.spawn((
        Camera3dBundle {
            // HDR plus bloom is what makes the flames read as light rather
            // than as orange paint: the fire layers deliberately push vertex
            // colours above 1.0, and without an HDR target that just clips.
            camera: Camera {
                hdr: true,
                ..default()
            },
            tonemapping: Tonemapping::TonyMcMapface,
            transform: Transform::from_xyz(
                focus.x,
                focus.y + distance * 0.77,
                focus.z + distance * 0.77,
            ),
            projection: PerspectiveProjection {
                far: 40_000.0,
                ..default()
            }
            .into(),
            ..default()
        },
        BloomSettings {
            // Fire uses HDR values and still blooms in the lab, but the cyan
            // stage must not: a lower dev intensity keeps road junctions and
            // grid intersections crisp instead of merging into white flares.
            intensity: if sim.scenario.is_dev() { 0.055 } else { 0.20 },
            ..BloomSettings::NATURAL
        },
        // Atmospheric haze, coloured to match the sky every frame by
        // `sky::update_sky` — the fallback here only matters for the one
        // frame before that system first runs.
        FogSettings::default(),
        OrbitCamera {
            focus,
            distance,
            ..default()
        },
    ));
}

/// The global shortcuts: transport, the general evacuation order, the ignition
/// tool, restart, help.
///
/// Gated on [`ui::UiFocus::typing`] like every other shortcut system. Without
/// it, `e` typed into any text field in the application orders the town to
/// evacuate — which is not a hypothetical, since the Entities tab's search box
/// and the composer's node fields are both plain egui text edits.
fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<ui::UiFocus>,
    mut sim: ResMut<Sim>,
    mut tool: ResMut<ignition_edit::IgnitionTool>,
    mut order: ResMut<command::OrderTool>,
    mut cam_mode: ResMut<camera::CameraMode>,
    mut orbit: Query<&mut camera::OrbitCamera>,
    mut selected: ResMut<inspect::Selected>,
    mut help: ResMut<ui::HelpUi>,
    mut panels: ResMut<ui::PanelState>,
    mut composer: ResMut<composer::Composer>,
    mut restarted: EventWriter<sim::SimRestarted>,
) {
    // Escape is the exception: it is what gets you *out* of a state, so it has
    // to work even while a widget has focus.
    if keys.just_pressed(KeyCode::Escape) {
        let cancelling = tool.mode != ignition_edit::EditMode::Off
            || order.is_armed()
            || *cam_mode != camera::CameraMode::Free;
        tool.mode = ignition_edit::EditMode::Off;
        order.disarm();
        *cam_mode = camera::CameraMode::Free;
        if !cancelling {
            selected.target = None;
        }
    }
    if focus.typing() {
        return;
    }

    if keys.just_pressed(KeyCode::Space) {
        sim.playing = !sim.playing;
    }
    // `.` steps one decision interval, running or not. Chosen because it sits
    // next to the speed keys on every layout and because nothing else in the
    // game wants it — see the note on shortcut collisions in CLAUDE.md, which
    // is the reason a new binding gets checked against the table rather than
    // simply added.
    if keys.just_pressed(KeyCode::Period) {
        sim.request_step();
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        sim.speed = (sim.speed * 2.0).min(ui::MAX_SPEED);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        sim.speed = (sim.speed / 2.0).max(ui::MIN_SPEED);
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let primary = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);

    if shift && keys.just_pressed(KeyCode::KeyE) {
        let n = sim.agents.order_evacuation_all();
        info!("general evacuation ordered: {n} households");
    }
    if keys.just_pressed(KeyCode::KeyI) {
        tool.mode = match tool.mode {
            ignition_edit::EditMode::Off => ignition_edit::EditMode::Place,
            ignition_edit::EditMode::Place => ignition_edit::EditMode::Off,
        };
        if tool.mode == ignition_edit::EditMode::Place {
            panels.focus_tab(ui::DockTab::Fire);
        }
    }
    if keys.just_pressed(KeyCode::F1) {
        help.open = !help.open;
    }
    if keys.just_pressed(KeyCode::F2) {
        if panels.bottom_tab == ui::BottomTab::Debug && panels.incident.visible() {
            panels.incident = ui::PanelPlacement::Hidden;
        } else {
            composer.open = false;
            panels.focus_bottom(ui::BottomTab::Debug);
        }
    }
    if shift && keys.just_pressed(KeyCode::Slash) {
        help.shortcuts_open = !help.shortcuts_open;
    }
    if keys.just_pressed(KeyCode::Home) {
        if let Ok(mut orbit) = orbit.get_single_mut() {
            let w = &sim.scenario.world;
            let p = scenario::Pos {
                x: w.width_m * 0.5,
                y: w.height_m * 0.5,
            };
            orbit.focus = frame::to_bevy(p, sim.scenario.terrain.height_at(p));
            orbit.distance = w.width_m.max(w.height_m) * 1.1;
        }
    }
    if keys.just_pressed(KeyCode::KeyF) {
        if let Ok(mut orbit) = orbit.get_single_mut() {
            let target = if shift {
                None
            } else {
                selected.target.and_then(|t| inspect::target_pos(&sim, t))
            };
            if let Some(p) = target {
                orbit.focus = frame::to_bevy(p, sim.scenario.terrain.height_at(p));
                orbit.distance = orbit.distance.clamp(120.0, 650.0);
            } else {
                let (p, h) = terrain_mesh::cell_ground(&sim.scenario, sim.ignition.centre);
                orbit.focus = frame::to_bevy(p, h);
                orbit.distance = 1400.0;
            }
        }
    }
    if primary && keys.just_pressed(KeyCode::KeyR) {
        match sim.restart() {
            Ok(()) => {
                restarted.send(sim::SimRestarted);
            }
            Err(e) => error!("restart failed: {e:#}"),
        }
    }
}

/// Empty the world on the way back to the scenario selector.
///
/// Everything the scene is made of — terrain chunks, roads, buildings, the
/// vegetation, the sky and sea, the figures and vehicles, the camera and the
/// sun, and the map symbols — is a root entity with a `Transform`, and nothing
/// else in the app is. So the query *is* the definition of "the scene", which
/// is what keeps this from being a list that silently goes stale every time
/// something new is spawned. Bevy's own bookkeeping entities (the window, and
/// what `bevy_egui` hangs off it) carry no `Transform` and are untouched.
///
/// Per-scenario resources are not cleared here: each one is re-inserted by its
/// `OnEnter(Playing)` setup, which is the only place that knows what the new
/// scenario's version of it should be.
fn teardown_scene(
    mut commands: Commands,
    roots: Query<Entity, (With<Transform>, Without<Parent>)>,
) {
    let mut n = 0;
    for e in &roots {
        commands.entity(e).despawn_recursive();
        n += 1;
    }
    info!("scene torn down: {n} root entities");
}
