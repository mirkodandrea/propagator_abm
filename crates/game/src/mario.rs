//! Optional native pause playground. It reads the serious game but never writes
//! Sim/Kiosk, orders, agents, or scores. Kiosk owns the requested ×1 clock.
use crate::{
    camera::OrbitCamera,
    kiosk::{Kiosk, Phase},
    sim::{Sim, SimRestarted},
    terrain_mesh::PauseSolid,
    AppState,
};
use bevy::{
    ecs::system::SystemParam,
    input::gamepad::*,
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        view::NoFrustumCulling,
    },
    window::PrimaryWindow,
};
use bevy_egui::{egui, EguiContexts};
use libsm64_native::{Engine, Input, Surface};
use std::collections::HashMap;

// Mario's ~160-unit height becomes 8 m, matching the exaggerated residents.
const SCALE: f32 = 20.0;
const STEP: f32 = 1.0 / 30.0;
const RADIUS: f32 = 70.0;

#[derive(Resource, Default)]
pub struct Playground {
    pub active: bool,
    toggle: bool,
    error: Option<String>,
    health: i16,
    wing_cap: bool,
    message: &'static str,
}

impl Playground {
    pub(crate) fn request_start_for_snapshot(&mut self) {
        self.toggle = true;
    }
}

#[derive(Component)]
struct Avatar;

struct SavedCamera {
    focus: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
    transform: Transform,
    blur: f32,
    viewport: Option<bevy::render::camera::Viewport>,
}

struct Solid {
    mesh: Handle<Mesh>,
    transform: Transform,
    lo: Vec3,
    hi: Vec3,
    terrain: bool,
}

#[derive(Default)]
struct Runtime {
    engine: Option<Engine>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    avatar: Option<Entity>,
    saved: Option<SavedCamera>,
    solids: Vec<Solid>,
    static_surfaces: Vec<Surface>,
    // Restore every temporary reaction before normal view updates run again.
    restore: HashMap<Entity, Transform>,
    previous: HashMap<Entity, Vec3>,
    reactions: HashMap<Entity, f32>,
    origin: Vec3,
    patch: Vec3,
    spawn: Vec3,
    accumulator: f32,
    fire_cooldown: f32,
    jump_latched: bool,
    attack_latched: bool,
    demo_time: f32,
    demo_fire: Vec3,
}

pub struct MarioPlugin;
impl Plugin for MarioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Playground>()
            .insert_non_send_resource(Runtime::default())
            .add_systems(
                Update,
                restore_reactions
                    .before(crate::people::update_people)
                    .before(crate::people::update_vehicles)
                    .before(crate::life::update)
                    .before(crate::units::update_units)
                    .before(crate::units::update_teams),
            )
            .add_systems(
                Update,
                (hud, drive)
                    .chain()
                    .after(crate::kiosk::view::camera)
                    .after(crate::kiosk::snap::hold)
                    .after(crate::people::update_people)
                    .after(crate::people::update_vehicles)
                    .after(crate::life::update)
                    .after(crate::units::update_units)
                    .after(crate::units::update_teams)
                    .after(crate::buildings::damage)
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(OnExit(AppState::Playing), clear);
    }
}

fn available(k: &Kiosk, sim: &Sim) -> bool {
    !k.title
        && !k.intro
        && k.tour.is_none()
        && k.started
        && k.phase == Phase::Pianifica
        && sim.speed <= 1.0
}

fn hud(
    mut contexts: EguiContexts,
    k: Res<Kiosk>,
    sim: Res<Sim>,
    mut mode: ResMut<Playground>,
    controls: Controls,
) {
    if std::env::var_os("MARIO_DEMO").is_some() {
        return;
    }
    if !available(&k, &sim) && !mode.active {
        return;
    }
    egui::Area::new(egui::Id::new("pause_mario"))
        .anchor(egui::Align2::LEFT_TOP, [16., 16.])
        .order(egui::Order::Foreground)
        .show(contexts.ctx_mut(), |ui| {
            ui.set_width(if mode.active { 540. } else { 160. });
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                if ui.button(if mode.active { "Torna al piano · M / Start" } else { "Mario · M / Start" }).clicked() {
                    mode.toggle = true;
                }
                if mode.active {
                    ui.label(format!("Simulazione ×1 · {}", rocca::words::clock(sim.time_s())));
                    ui.label(controls.pad().and_then(|pad| controls.pads.name(pad))
                        .map_or("Gamepad non connesso".to_string(), |name| format!("Gamepad: {name}")));
                    ui.label("WASD / stick sinistro · Spazio / A (×): salto · J / X (□): pugno");
                    ui.label("Shift / LT: accovacciati · Frecce / stick destro: camera · R / Y (△): ricomincia");
                    ui.label("Zoom camera: rotella · Q/E · L1/R1");
                    ui.label(if mode.wing_cap {
                        "Wing Cap attivo · triplo salto in corsa: volo · V / ○: disattiva"
                    } else {
                        "V / ○: attiva Wing Cap · triplo salto in corsa: volo"
                    });
                    ui.label(format!("Energia: {} / 8   {}", (mode.health.max(0) / 0x100).min(8), mode.message));
                }
                if let Some(error) = &mode.error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
            });
        });
}

fn restore_reactions(mut runtime: NonSendMut<Runtime>, mut transforms: Query<&mut Transform>) {
    for (entity, original) in runtime.restore.drain() {
        if let Ok(mut transform) = transforms.get_mut(entity) {
            *transform = original;
        }
    }
}

fn clear(mut runtime: NonSendMut<Runtime>, mut mode: ResMut<Playground>) {
    if let Some(engine) = runtime.engine.as_mut() {
        engine.despawn();
        engine.load_surfaces(&[]);
    }
    runtime.avatar = None;
    runtime.saved = None;
    runtime.solids.clear();
    runtime.static_surfaces.clear();
    runtime.restore.clear();
    runtime.previous.clear();
    runtime.reactions.clear();
    mode.active = false;
    mode.wing_cap = false;
    mode.toggle = false;
}

#[derive(SystemParam)]
struct Controls<'w> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    pads: Res<'w, Gamepads>,
    buttons: Res<'w, ButtonInput<GamepadButton>>,
    axes: Res<'w, Axis<GamepadAxis>>,
    triggers: Res<'w, Axis<GamepadButton>>,
}

impl Controls<'_> {
    fn pad(&self) -> Option<Gamepad> {
        // Prefer the pad in use, then a stable ID; disconnected pads vanish.
        self.pads
            .iter()
            .filter(|&p| {
                [
                    GamepadButtonType::Start,
                    GamepadButtonType::South,
                    GamepadButtonType::West,
                ]
                .iter()
                .any(|&b| self.buttons.pressed(GamepadButton::new(p, b)))
                    || self.axis_on(p, GamepadAxisType::LeftStickX).abs() > 0.2
                    || self.axis_on(p, GamepadAxisType::LeftStickY).abs() > 0.2
            })
            .min_by_key(|p| p.id)
            .or_else(|| self.pads.iter().min_by_key(|p| p.id))
    }
    fn axis_on(&self, p: Gamepad, axis: GamepadAxisType) -> f32 {
        self.axes.get(GamepadAxis::new(p, axis)).unwrap_or(0.)
    }
    fn axis(&self, axis: GamepadAxisType) -> f32 {
        deadzone(self.pad().map_or(0., |p| self.axis_on(p, axis)))
    }
    fn held(&self, button: GamepadButtonType) -> bool {
        self.pad()
            .is_some_and(|p| self.buttons.pressed(GamepadButton::new(p, button)))
    }
    fn pressed(&self, button: GamepadButtonType) -> bool {
        self.pads
            .iter()
            .any(|p| self.buttons.just_pressed(GamepadButton::new(p, button)))
    }
    fn crouch(&self) -> bool {
        self.keys.pressed(KeyCode::ShiftLeft)
            || self.keys.pressed(KeyCode::ShiftRight)
            || self.held(GamepadButtonType::LeftTrigger2)
            || self.held(GamepadButtonType::RightTrigger2)
            || self.pad().is_some_and(|p| {
                [
                    GamepadButtonType::LeftTrigger2,
                    GamepadButtonType::RightTrigger2,
                ]
                .iter()
                .any(|&b| self.triggers.get(GamepadButton::new(p, b)).unwrap_or(0.) > 0.3)
            })
    }
    fn key_axis(&self, positive: KeyCode, negative: KeyCode) -> f32 {
        f32::from(self.keys.pressed(positive)) - f32::from(self.keys.pressed(negative))
    }
}

fn deadzone(value: f32) -> f32 {
    value.signum() * ((value.abs() - 0.18) / 0.82).clamp(0., 1.)
}

type Actors<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Handle<Mesh>,
        &'static mut Transform,
        &'static Visibility,
        Option<&'static crate::people::PersonView>,
    ),
    (
        Without<OrbitCamera>,
        Without<PauseSolid>,
        Or<(
            With<crate::people::PersonView>,
            With<crate::people::VehicleView>,
            With<crate::life::Ambient>,
            With<crate::units::UnitView>,
            With<crate::units::TeamView>,
        )>,
    ),
>;

#[derive(SystemParam)]
struct RenderAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    images: ResMut<'w, Assets<Image>>,
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    mut runtime: NonSendMut<Runtime>,
    mut mode: ResMut<Playground>,
    k: Res<Kiosk>,
    sim: Res<Sim>,
    controls: Controls,
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut contexts: EguiContexts,
    mut cameras: Query<
        (
            &mut OrbitCamera,
            &mut Transform,
            Option<&mut bevy::core_pipeline::dof::DepthOfFieldSettings>,
            &mut Camera,
        ),
        Without<PauseSolid>,
    >,
    solids: Query<
        (
            &Handle<Mesh>,
            &Transform,
            Option<&crate::terrain_mesh::TerrainChunk>,
        ),
        With<PauseSolid>,
    >,
    mut actors: Actors,
    mut assets: RenderAssets,
    mut restarted: EventReader<SimRestarted>,
    buildings: Res<crate::buildings::Buildings>,
    mut wheel: EventReader<MouseWheel>,
) {
    // Drain even outside Mario so opening it never replays old map scrolling.
    let scroll: f32 = wheel.read().map(|event| match event.unit {
        MouseScrollUnit::Line => event.y,
        MouseScrollUnit::Pixel => event.y / 50.0,
    }).sum();
    let RenderAssets {
        ref mut meshes,
        ref mut materials,
        ref mut images,
    } = assets;
    let can_play = available(&k, &sim);
    let reading = !crate::kiosk::snap::active();
    let keyboard_free = reading && (mode.active || !contexts.ctx_mut().wants_keyboard_input());
    let axis = |a| if reading { controls.axis(a) } else { 0. };
    let held = |b| reading && controls.held(b);
    let pressed = |b| reading && controls.pressed(b);
    let focused = crate::kiosk::snap::active() || windows.get_single().is_ok_and(|w| w.focused);
    let reset = restarted.read().count() > 0;
    let toggle = std::mem::take(&mut mode.toggle)
        || focused && keyboard_free && controls.keys.just_pressed(KeyCode::KeyM)
        || focused && pressed(GamepadButtonType::Start);
    let stop = mode.active
        && (!can_play
            || reset
            || toggle
            || focused && keyboard_free && controls.keys.just_pressed(KeyCode::Escape));
    let Ok((mut camera, mut camera_tf, mut dof, mut render_camera)) = cameras.get_single_mut()
    else {
        return;
    };
    if stop {
        if let Some(saved) = runtime.saved.take() {
            camera.focus = saved.focus;
            camera.distance = saved.distance;
            camera.yaw = saved.yaw;
            camera.pitch = saved.pitch;
            *camera_tf = saved.transform;
            if let Some(dof) = dof.as_mut() {
                dof.max_circle_of_confusion_diameter = saved.blur;
                dof.focal_distance = camera.distance;
            }
            render_camera.viewport = saved.viewport;
        }
        if let Some(avatar) = runtime.avatar.take() {
            commands.entity(avatar).despawn_recursive();
        }
        if let Some(engine) = runtime.engine.as_mut() {
            engine.despawn();
            engine.load_surfaces(&[]);
        }
        runtime.solids.clear();
        runtime.reactions.clear();
        runtime.previous.clear();
        mode.active = false;
        return;
    }
    if !mode.active {
        if !can_play || !toggle || !focused || reset {
            return;
        }
        mode.error = None;
        if runtime.engine.is_none() {
            match load_engine(images, materials) {
                Ok((engine, material)) => {
                    runtime.engine = Some(engine);
                    runtime.material = Some(material);
                }
                Err(e) => {
                    mode.error = Some(e);
                    return;
                }
            }
        }
        runtime.solids = solids
            .iter()
            .filter_map(|(mesh, tf, terrain)| {
                let (lo, hi) = mesh_bounds(meshes.get(mesh)?, tf)?;
                Some(Solid {
                    mesh: mesh.clone(),
                    transform: *tf,
                    lo,
                    hi,
                    terrain: terrain.is_some(),
                })
            })
            .collect();
        runtime.saved = Some(SavedCamera {
            focus: camera.focus,
            distance: camera.distance,
            yaw: camera.yaw,
            pitch: camera.pitch,
            transform: *camera_tf,
            blur: dof.as_ref().map_or(0., |d| d.max_circle_of_confusion_diameter),
            viewport: render_camera.viewport.clone(),
        });
        let demo = std::env::var_os("MARIO_DEMO").is_some();
        let rocca = if demo { None } else { rocca_start(&sim) };
        let base = if demo {
            // Stand just outside the eastern edge of the actual burning front.
            let fire = sim.fire.active_cells().iter()
                .map(|&c| sim.scn.world.centre_of(c))
                .max_by(|a, b| a.x.total_cmp(&b.x))
                .unwrap_or_else(|| sim.case.ignition());
            runtime.demo_fire = crate::frame::to_bevy(fire, sim.scn.terrain.height_at(fire));
            let p = runtime.demo_fire + Vec3::X * 28.;
            p.with_y(sim.scn.terrain.height_at(crate::frame::to_world(p)))
        } else {
            rocca.map_or_else(|| road_spawn(&sim, camera.focus), |(p, _)| p)
        };
        runtime.demo_time = 0.;
        runtime.origin = base;
        runtime.spawn = base + Vec3::Y * 2.;
        runtime.patch = Vec3::splat(f32::INFINITY);
        runtime.accumulator = 0.;
        runtime.fire_cooldown = 0.;
        runtime.jump_latched = false;
        runtime.attack_latched = false;
        refresh_patch(&mut runtime, &meshes, &sim, base, &mut actors);
        // Find an open floor near the map focus, avoiding spawning inside a
        // roof or canopy. All spawn coordinates remain small in SM64 space.
        for i in 0..49 {
            let angle = i as f32 * 2.4;
            let radius = 4.0 * (i as f32).sqrt();
            let p = base + Vec3::new(angle.cos() * radius, 0., angle.sin() * radius);
            let ground = sim.scn.terrain.height_at(crate::frame::to_world(p));
            let local = to_sm64(p.with_y(ground + 200.), runtime.origin);
            let floor =
                runtime.engine.as_ref().unwrap().floor_height(local) / SCALE + runtime.origin.y;
            if (floor - ground).abs() < 1.5 {
                runtime.spawn = p.with_y(floor + 2.0);
                break;
            }
        }
        let spawn = to_sm64(runtime.spawn, runtime.origin);
        if !runtime.engine.as_mut().unwrap().spawn(spawn) {
            mode.error = Some("Mario non trova un terreno su cui stare.".into());
            runtime.saved = None;
            return;
        }
        let handle = runtime
            .mesh
            .get_or_insert_with(|| {
                meshes.add(Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::default(),
                ))
            })
            .clone();
        runtime.avatar = Some(
            commands
                .spawn((
                    PbrBundle {
                        mesh: handle,
                        material: runtime.material.clone().unwrap(),
                        ..default()
                    },
                    Avatar,
                    NoFrustumCulling,
                ))
                .id(),
        );
        camera.distance = 36.;
        camera.pitch = -0.45;
        if demo {
            camera.distance = 52.;
            camera.pitch = -0.65;
            camera.yaw = 1.2;
            eprintln!("mario demo: spawn {:?}, fire {:?}", runtime.spawn, runtime.demo_fire);
        }
        if let Some((_, yaw)) = rocca {
            camera.yaw = yaw;
        }
        if let Some(dof) = dof.as_mut() {
            dof.max_circle_of_confusion_diameter = 0.;
        }
        render_camera.viewport = None;
        mode.active = true;
        mode.message = "";
    }
    if !focused {
        runtime.accumulator = 0.;
        return;
    }
    let dt = time.delta_seconds().min(0.1);
    let keyboard = |positive, negative| {
        if keyboard_free {
            controls.key_axis(positive, negative)
        } else {
            0.
        }
    };
    camera.yaw -= (axis(GamepadAxisType::RightStickX)
        + keyboard(KeyCode::ArrowRight, KeyCode::ArrowLeft))
        * dt
        * 2.0;
    camera.pitch = (camera.pitch
        + (axis(GamepadAxisType::RightStickY) + keyboard(KeyCode::ArrowUp, KeyCode::ArrowDown))
            * dt)
        .clamp(-1.5, 0.65);
    let zoom = keyboard(KeyCode::KeyE, KeyCode::KeyQ)
        + f32::from(held(GamepadButtonType::RightTrigger))
        - f32::from(held(GamepadButtonType::LeftTrigger));
    let scroll = if keyboard_free && !contexts.ctx_mut().wants_pointer_input() { scroll } else { 0.0 };
    camera.distance = (camera.distance * (zoom * dt * 1.5 - scroll * 0.12).exp())
        .clamp(12.0, 240.0);
    let mut stick = Vec2::new(
        axis(GamepadAxisType::LeftStickX) + keyboard(KeyCode::KeyD, KeyCode::KeyA),
        -axis(GamepadAxisType::LeftStickY) + keyboard(KeyCode::KeyS, KeyCode::KeyW),
    )
    .clamp_length_max(1.);
    let mut jump =
        held(GamepadButtonType::South) || keyboard_free && controls.keys.pressed(KeyCode::Space);
    let mut attack =
        held(GamepadButtonType::West) || keyboard_free && controls.keys.pressed(KeyCode::KeyJ);
    runtime.jump_latched |= pressed(GamepadButtonType::South)
        || keyboard_free && controls.keys.just_pressed(KeyCode::Space);
    runtime.attack_latched |= pressed(GamepadButtonType::West)
        || keyboard_free && controls.keys.just_pressed(KeyCode::KeyJ);
    let respawn = pressed(GamepadButtonType::North)
        || keyboard_free && controls.keys.just_pressed(KeyCode::KeyR);
    if respawn {
        let p = to_sm64(runtime.spawn, runtime.origin);
        runtime.engine.as_mut().unwrap().spawn(p);
        mode.wing_cap = false;
    }
    if pressed(GamepadButtonType::East)
        || keyboard_free && controls.keys.just_pressed(KeyCode::KeyV)
    {
        let engine = runtime.engine.as_mut().unwrap();
        let enabled = !engine.has_wing_cap();
        engine.set_wing_cap(enabled);
        mode.message = if enabled { "Wing Cap! Triplo salto in corsa per volare." } else { "Wing Cap disattivato." };
    }
    let mut crouch = reading && controls.crouch();
    if std::env::var_os("MARIO_DEMO").is_some() {
        runtime.demo_time += dt;
        let t = runtime.demo_time;
        // Follow a loop beside the flames, dart into the front, then retreat.
        let route = [
            (1.5, Vec2::new(28., 0.)),
            (5.0, Vec2::new(28., -32.)),
            (8.0, Vec2::new(8., -18.)),
            (11.0, Vec2::new(-8., 0.)),
            (14.0, Vec2::new(30., 22.)),
            (18.0, Vec2::new(50., -12.)),
            (22.0, Vec2::new(28., 0.)),
            (26.0, Vec2::new(28., 0.)),
        ];
        let segment = route.iter().position(|(end, _)| t < *end).unwrap_or(route.len() - 1);
        let (end, offset) = route[segment];
        let (start, from) = if segment == 0 { (0., offset) } else { route[segment - 1] };
        let offset = from.lerp(offset, ((t - start) / (end - start)).clamp(0., 1.));
        let target = runtime.demo_fire + Vec3::new(offset.x, 0., offset.y);
        let pos = from_sm64(runtime.engine.as_ref().unwrap().state.position, runtime.origin);
        let delta = Vec2::new(target.x - pos.x, target.z - pos.z);
        let direction = delta.normalize_or_zero() * (delta.length() / 5.).min(1.);
        camera.yaw = 1.2 + (t * 0.21).sin() * 0.6;
        camera.pitch = -0.65 + (t * 0.3).sin() * 0.08;
        stick = if t < 1.5 || t > 22. { Vec2::ZERO } else {
            Vec2::new(direction.dot(Vec2::new(camera.yaw.cos(), -camera.yaw.sin())),
                direction.dot(Vec2::new(camera.yaw.sin(), camera.yaw.cos())))
        };
        jump = (t > 2. && t < 8. || t > 13. && t < 21.) && t % 1.8 < 0.18;
        attack = t > 22. && t % 0.9 < 0.15;
        crouch = t > 15. && t < 15.2;
        if (t * 30.).round() as u32 % 90 == 0 {
            eprintln!("mario demo: t={t:.1}, pos={pos:?}, health={}", mode.health);
        }
    }
    runtime.accumulator += dt;
    while runtime.accumulator >= STEP {
        runtime.accumulator -= STEP;
        runtime.fire_cooldown = (runtime.fire_cooldown - STEP).max(0.);
        let pos = from_sm64(
            runtime.engine.as_ref().unwrap().state.position,
            runtime.origin,
        );
        refresh_patch(&mut runtime, &meshes, &sim, pos, &mut actors);
        let (drift, speed) = crate::sea::wind_drift(&sim);
        let water = crate::sea::is_water(&sim.scn, crate::frame::to_world(pos));
        let water_y = if water {
            0.8 + crate::sea::wave_height(pos.x, pos.z, time.elapsed_seconds(), drift, speed)
        } else {
            -10000.
        };
        let water_level = ((water_y - runtime.origin.y) * SCALE) as i32;
        let input = Input {
            camera_x: -camera.yaw.sin(),
            camera_z: -camera.yaw.cos(),
            stick_x: stick.x,
            stick_y: stick.y,
            jump: u8::from(jump || runtime.jump_latched),
            attack: u8::from(attack || runtime.attack_latched),
            crouch: u8::from(crouch),
        };
        runtime.jump_latched = false;
        runtime.attack_latched = false;
        let engine = runtime.engine.as_mut().unwrap();
        engine.water_level(water_level);
        engine.tick(&input);
        let pos = from_sm64(engine.state.position, runtime.origin);
        if !water && runtime.fire_cooldown == 0. && touching_fire(&sim, &buildings, pos) {
            let source = to_sm64(pos - Vec3::X * 2., runtime.origin);
            runtime.engine.as_mut().unwrap().hurt(source, true);
            runtime.fire_cooldown = 2.;
            mode.message = "Ahi! Il fuoco brucia.";
        }
        interact(&mut runtime, &mut mode, &mut actors, &meshes, pos);
        let engine = runtime.engine.as_ref().unwrap();
        mode.health = engine.state.health;
        mode.wing_cap = engine.has_wing_cap();
        if engine.state.health < 0x100 || !pos.is_finite() || pos.y < runtime.origin.y - 150. {
            let p = to_sm64(runtime.spawn, runtime.origin);
            runtime.engine.as_mut().unwrap().spawn(p);
            mode.wing_cap = false;
            mode.message = "Ricomincia!";
        }
    }
    // Reactions are visual, reversible, and applied after the ordinary views.
    let origin = runtime.origin;
    let pos = from_sm64(runtime.engine.as_ref().unwrap().state.position, origin);
    for (entity, _, mut tf, vis, person) in &mut actors {
        if *vis == Visibility::Hidden {
            continue;
        }
        if let Some(remaining) = runtime.reactions.get_mut(&entity) {
            *remaining -= dt;
            let phase = (1. - remaining.max(0.) / 0.7).clamp(0., 1.);
            runtime.restore.insert(entity, *tf);
            if person.is_some() {
                tf.translation.y += (phase * std::f32::consts::PI).sin() * 2.;
            }
            tf.rotation *= Quat::from_rotation_z((phase * std::f32::consts::TAU).sin() * 0.18);
        } else if person.is_some() && tf.translation.distance(pos) < 16. {
            runtime.restore.insert(entity, *tf);
            let direction = pos - tf.translation;
            tf.rotation = Quat::from_rotation_y(direction.x.atan2(direction.z));
            tf.rotation *= Quat::from_rotation_z((time.elapsed_seconds() * 6.).sin() * 0.05);
        }
    }
    runtime.reactions.retain(|_, t| *t > 0.);
    let engine = runtime.engine.as_ref().unwrap();
    if let Some(mesh) = runtime.mesh.as_ref().and_then(|h| meshes.get_mut(h)) {
        let n = engine.vertex_count;
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            engine.positions[..n]
                .iter()
                .map(|&p| from_sm64(p, origin).to_array())
                .collect::<Vec<_>>(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, engine.normals[..n].to_vec());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, engine.uvs[..n].to_vec());
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            engine.colors[..n]
                .iter()
                .map(|c| [c[0], c[1], c[2], 1.])
                .collect::<Vec<_>>(),
        );
    }
    camera.focus = pos + Vec3::Y * 5.;
    let dir = Vec3::new(
        camera.yaw.sin() * camera.pitch.cos(),
        -camera.pitch.sin(),
        camera.yaw.cos() * camera.pitch.cos(),
    );
    let distance = runtime
        .static_surfaces
        .iter()
        .filter_map(|surface| {
            let vertices = surface
                .vertices
                .map(|p| from_sm64(p.map(|x| x as f32), origin));
            ray_triangle(camera.focus, dir, vertices).filter(|&t| t > 2.0)
        })
        .fold(camera.distance, |distance, hit| {
            distance.min((hit - 1.).max(3.))
        });
    let eye = camera.focus + dir * distance;
    let ground = sim.scn.terrain.height_at(crate::frame::to_world(eye));
    camera_tf.translation = eye.with_y(eye.y.max(ground + 3.));
    camera_tf.look_at(camera.focus, Vec3::Y);
}

fn load_engine(
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> Result<(Engine, Handle<StandardMaterial>), String> {
    let path = std::env::var_os("MARIO64_ROM")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("data/mario64/Super Mario 64 (USA).z64"));
    let rom = std::fs::read(&path).map_err(|e| format!("ROM {}: {e}", path.display()))?;
    let (engine, mut atlas) = Engine::new(&rom).map_err(str::to_owned)?;
    // libsm64 uses transparent atlas pixels to mean "use vertex colour",
    // including UV (1,1) on untextured body parts. Bevy multiplies the atlas
    // by vertex colour: turn transparency into white, keeping Mario opaque.
    for pixel in atlas.chunks_exact_mut(4) {
        let alpha = pixel[3] as u16;
        for channel in &mut pixel[..3] {
            *channel = ((*channel as u16 * alpha + 255 * (255 - alpha)) / 255) as u8;
        }
        pixel[3] = 255;
    }
    let image = Image::new(
        Extent3d {
            width: libsm64_native::TEXTURE_WIDTH,
            height: libsm64_native::TEXTURE_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        atlas,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(image)),
        perceptual_roughness: 0.8,
        cull_mode: None,
        ..default()
    });
    Ok((engine, material))
}

fn to_sm64(p: Vec3, origin: Vec3) -> [f32; 3] {
    ((p - origin) * SCALE).to_array()
}
fn from_sm64(p: [f32; 3], origin: Vec3) -> Vec3 {
    Vec3::from_array(p) / SCALE + origin
}

fn road_spawn(sim: &Sim, focus: Vec3) -> Vec3 {
    let point = Vec2::new(focus.x, -focus.z);
    let nearest = sim
        .scn
        .vectors
        .roads
        .iter()
        .flat_map(|road| road.line.windows(2))
        .map(|segment| {
            let a = Vec2::from_array(segment[0]);
            let b = Vec2::from_array(segment[1]);
            let delta = b - a;
            a + delta * ((point - a).dot(delta) / delta.length_squared().max(0.001)).clamp(0., 1.)
        })
        .min_by(|a, b| {
            a.distance_squared(point)
                .total_cmp(&b.distance_squared(point))
        });
    let p = nearest.map_or(crate::frame::to_world(focus), |p| scenario::Pos {
        x: p.x,
        y: p.y,
    });
    crate::frame::to_bevy(p, sim.scn.terrain.height_at(p))
}

fn rocca_start(sim: &Sim) -> Option<(Vec3, f32)> {
    let path = sim
        .data
        .join("scenarios")
        .join(&sim.scn.id)
        .join("island.json");
    let island: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let rocca = &island["landmarks"]["rocca"];
    let centre = Vec3::new(
        rocca["pos"][0].as_f64()? as f32,
        0.,
        -rocca["pos"][1].as_f64()? as f32,
    );
    let yaw = (rocca["yaw_deg"].as_f64().unwrap_or(0.) as f32).to_radians();
    let half = rocca["wall_half_m"].as_f64().unwrap_or(31.) as f32;
    let scale = rocca["scale"].as_f64().unwrap_or(1.) as f32;
    // The authored gate faces +X. Start outside its jambs, looking inward.
    let outward = Quat::from_rotation_y(yaw) * Vec3::X;
    let p = centre + outward * ((half + 20.) * scale);
    let ground = sim.scn.terrain.height_at(crate::frame::to_world(p));
    Some((p.with_y(ground), outward.x.atan2(outward.z)))
}

fn mesh_bounds(mesh: &Mesh, tf: &Transform) -> Option<(Vec3, Vec3)> {
    let VertexAttributeValues::Float32x3(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?
    else {
        return None;
    };
    Some(
        positions
            .iter()
            .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), &p| {
                let p = tf.transform_point(Vec3::from_array(p));
                (lo.min(p), hi.max(p))
            }),
    )
}

fn ray_triangle(start: Vec3, direction: Vec3, triangle: [Vec3; 3]) -> Option<f32> {
    let a = triangle[1] - triangle[0];
    let b = triangle[2] - triangle[0];
    let normal = direction.cross(b);
    let determinant = a.dot(normal);
    if determinant.abs() < 0.0001 {
        return None;
    }
    let offset = start - triangle[0];
    let u = offset.dot(normal) / determinant;
    let cross = offset.cross(a);
    let v = direction.dot(cross) / determinant;
    if !(0.0..=1.0).contains(&u) || v < 0. || u + v > 1. {
        return None;
    }
    let distance = b.dot(cross) / determinant;
    (distance > 0.).then_some(distance)
}

fn near(lo: Vec3, hi: Vec3, p: Vec3, radius: f32) -> bool {
    p.x + radius >= lo.x && p.x - radius <= hi.x && p.z + radius >= lo.z && p.z - radius <= hi.z
}

fn triangles(mesh: &Mesh, mut visit: impl FnMut([Vec3; 3])) {
    let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return;
    };
    let index: Vec<usize> = match mesh.indices() {
        Some(Indices::U32(i)) => i.iter().map(|&i| i as usize).collect(),
        Some(Indices::U16(i)) => i.iter().map(|&i| i as usize).collect(),
        None => (0..p.len()).collect(),
    };
    for i in index.chunks_exact(3) {
        visit([
            Vec3::from_array(p[i[0]]),
            Vec3::from_array(p[i[1]]),
            Vec3::from_array(p[i[2]]),
        ]);
    }
}

fn append_surfaces(
    mesh: &Mesh,
    tf: &Transform,
    origin: Vec3,
    pos: Vec3,
    terrain: bool,
    sim: &Sim,
    out: &mut Vec<Surface>,
) {
    triangles(mesh, |vertices| {
        let mut v = vertices.map(|p| tf.transform_point(p));
        let lo = v[0].min(v[1]).min(v[2]);
        let hi = v[0].max(v[1]).max(v[2]);
        if !near(lo, hi, pos, RADIUS) {
            return;
        }
        if terrain {
            // The island DEM has a flat sea floor at zero. Give swimming room
            // below the opaque sea, blending from shallow shore to deep water.
            for p in &mut v {
                let world = crate::frame::to_world(*p);
                if crate::sea::is_water(&sim.scn, world) {
                    p.y -= (crate::sea::shore_distance(&sim.scn, world) * 0.3).min(20.);
                }
            }
        }
        let vertices = v.map(|p| to_sm64(p, origin).map(|x| x.round() as i32));
        out.push(Surface {
            vertices,
            ..default()
        });
    });
}

fn refresh_patch(
    runtime: &mut Runtime,
    meshes: &Assets<Mesh>,
    sim: &Sim,
    pos: Vec3,
    actors: &mut Actors,
) {
    // Recenter to keep SM64's remaining 16-bit spawn/water coordinates safe.
    // The entire island can be traversed without scaling simulation metres.
    let recenter = (pos - runtime.origin).abs().max_element() > 150.;
    if recenter {
        runtime.origin = pos;
        runtime.engine.as_mut().unwrap().position([0.; 3]);
    }
    // libsm64 scans its surface array; load a small neighbourhood instead of
    // millions of terrain/forest triangles. Actor geometry refreshes at 30 Hz.
    if recenter || runtime.patch.distance(pos) > 24. {
        runtime.patch = pos;
        runtime.static_surfaces.clear();
        for solid in &runtime.solids {
            if near(solid.lo, solid.hi, pos, RADIUS) {
                if let Some(mesh) = meshes.get(&solid.mesh) {
                    append_surfaces(
                        mesh,
                        &solid.transform,
                        runtime.origin,
                        pos,
                        solid.terrain,
                        sim,
                        &mut runtime.static_surfaces,
                    );
                }
            }
        }
    }
    let mut surfaces = runtime.static_surfaces.clone();
    for (_, handle, tf, visibility, _) in actors.iter_mut() {
        if *visibility != Visibility::Hidden && tf.translation.distance(pos) < RADIUS {
            if let Some(mesh) = meshes.get(handle) {
                append_surfaces(mesh, &tf, runtime.origin, pos, false, sim, &mut surfaces);
            }
        }
    }
    runtime.engine.as_mut().unwrap().load_surfaces(&surfaces);
}

fn touching_fire(sim: &Sim, buildings: &crate::buildings::Buildings, pos: Vec3) -> bool {
    let world = crate::frame::to_world(pos);
    let w = sim.scn.world;
    if world.x < 0. || world.y < 0. || world.x >= w.width_m || world.y >= w.height_m {
        return false;
    }
    let ground = sim.scn.terrain.height_at(world);
    if pos.y - ground > 12. {
        return false;
    }
    // Use the actual burning state: burnt-out cells cannot hurt Mario.
    let cell = w.cell_of(world);
    sim.fire.cell_state(cell) == fire::CellFire::Burning
        || buildings
            .burning(sim.time_s() as f32)
            .iter()
            .any(|(p, _)| (p.x - world.x).hypot(p.y - world.y) < 12.)
}

fn interact(
    runtime: &mut Runtime,
    mode: &mut Playground,
    actors: &mut Actors,
    meshes: &Assets<Mesh>,
    mario: Vec3,
) {
    for (entity, handle, tf, visibility, person) in actors.iter_mut() {
        if *visibility == Visibility::Hidden {
            continue;
        }
        let old = runtime.previous.insert(entity, tf.translation);
        if tf.translation.distance(mario) > 40. {
            continue;
        }
        let Some(mesh) = meshes.get(handle) else {
            continue;
        };
        let Some((lo, hi)) = mesh_bounds(mesh, &tf) else {
            continue;
        };
        let source = to_sm64(
            Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5),
            runtime.origin,
        );
        let in_reach = near(lo, hi, mario, 3.) && mario.y < hi.y + 2. && mario.y + 8. > lo.y;
        let attacked = in_reach
            && runtime
                .engine
                .as_mut()
                .unwrap()
                .attacking(source, (hi.y - lo.y) * SCALE);
        if attacked && !runtime.reactions.contains_key(&entity) {
            runtime.reactions.insert(entity, 0.7);
            mode.message = if person.is_some() { "Ehi!" } else { "Bong!" };
        }
        if person.is_none()
            && old.is_some_and(|p| p.distance(tf.translation) > 0.05)
            && near(lo, hi, mario, 2.)
            && mario.y < hi.y - 0.5
            && mario.y + 7. > lo.y
        {
            runtime.engine.as_mut().unwrap().hurt(source, false);
            mode.message = "Attento alle auto!";
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinates_and_deadzone() {
        let origin = Vec3::new(7000., 800., -6000.);
        let p = origin + Vec3::new(12., 3., -8.);
        assert_eq!(from_sm64(to_sm64(p, origin), origin), p);
        assert_eq!(deadzone(0.1), 0.);
        assert_eq!(deadzone(-1.), -1.);
        assert_eq!(deadzone(1.), 1.);
    }

    #[test]
    fn follow_camera_stops_at_obstacles() {
        let wall = [
            Vec3::new(-10., -10., 5.),
            Vec3::new(10., -10., 5.),
            Vec3::new(0., 10., 5.),
        ];
        assert_eq!(ray_triangle(Vec3::ZERO, Vec3::Z, wall), Some(5.));
        assert!(ray_triangle(Vec3::ZERO, Vec3::NEG_Z, wall).is_none());
        assert!(ray_triangle(Vec3::ZERO, Vec3::X, wall).is_none());
    }
}
