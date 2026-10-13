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
    asset::load_internal_asset,
    ecs::system::SystemParam,
    input::gamepad::*,
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    render::{
        camera::CameraProjection,
        mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexAttributeValues},
        render_asset::RenderAssetUsages,
        render_resource::{AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor, ShaderRef,
            SpecializedMeshPipelineError, TextureDimension, TextureFormat, TextureUsages},
        texture::ImageSampler,
        view::{NoFrustumCulling, RenderLayers},
    },
    window::PrimaryWindow,
};
use bevy_egui::{egui, EguiContexts};
use libsm64_native::{Engine, Input, Surface};
use std::collections::HashMap;
#[path = "mario_audio.rs"]
mod audio;
use audio::NativeAudio;
use bevy::audio::AddAudioSource;

// Mario's ~160-unit height becomes 8 m, matching the exaggerated residents.
const SCALE: f32 = 20.0;
const STEP: f32 = 1.0 / 30.0;
const COLLISION_RADIUS: f32 = 18.0;
const PATCH_RADIUS: f32 = 26.0;
const PATCH_STEP: f32 = 6.0;
const TERRAIN_STEP: f32 = 4.0;
const COLLISION_CELL: f32 = 16.0;

const MARIO_SHADER: Handle<Shader> =
    Handle::weak_from_u128(0x64AA_2026_1013_4B20_9000_0000_0000_0001);
const MARIO_OUTLINE_SHADER: Handle<Shader> =
    Handle::weak_from_u128(0x64AA_2026_1013_4B20_9000_0000_0000_0002);
const MARIO_MASK_SHADER: Handle<Shader> =
    Handle::weak_from_u128(0x64AA_2026_1013_4B20_9000_0000_0000_0003);
const MASK_LAYER: usize = 31;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct MarioOutlineMaterial {
    #[texture(0)]
    #[sampler(1)]
    mask: Handle<Image>,
    #[uniform(2)]
    bounds: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct MarioMaskMaterial {
    #[texture(0)]
    #[sampler(1)]
    atlas: Handle<Image>,
}

impl Material for MarioMaskMaterial {
    fn fragment_shader() -> ShaderRef { MARIO_MASK_SHADER.into() }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

impl Material for MarioOutlineMaterial {
    fn vertex_shader() -> ShaderRef { MARIO_OUTLINE_SHADER.into() }
    fn fragment_shader() -> ShaderRef { MARIO_OUTLINE_SHADER.into() }
    fn alpha_mode(&self) -> AlphaMode { AlphaMode::Blend }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            // Bevy uses reverse Z: smaller depth means *behind* the scene.
            // Occlusion is checked against the scene-only prepass in WGSL,
            // avoiding Mario's own front faces triggering his hidden outline.
            depth.depth_compare = CompareFunction::Always;
            depth.depth_write_enabled = false;
        }
        Ok(())
    }
}

/// Mario retains his own N64 shading and texture combiner, independent of PBR.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct MarioMaterial {
    #[texture(0)]
    #[sampler(1)]
    atlas: Handle<Image>,
}

impl Material for MarioMaterial {
    fn vertex_shader() -> ShaderRef {
        MARIO_SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        MARIO_SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Mask(0.5)
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The native wing display list disables backface culling.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Resource, Default)]
pub struct Playground {
    pub active: bool,
    toggle: bool,
    snapshot_wing_cap: bool,
    respawn_nearby: bool,
    error: Option<String>,
    health: i16,
    wing_cap: bool,
    message: &'static str,
}

impl Playground {
    pub(crate) fn request_start(&mut self) {
        self.error = None;
        self.toggle = true;
    }
    pub(crate) fn error_message(&self) -> Option<&str> { self.error.as_deref() }
    pub(crate) fn request_start_for_snapshot(&mut self, wing_cap: bool) {
        self.toggle = true;
        self.snapshot_wing_cap = wing_cap;
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
    depth_prepass: bool,
}

#[derive(Clone)]
struct CollisionTriangle {
    vertices: [Vec3; 3],
    lo: Vec3,
    hi: Vec3,
}

impl CollisionTriangle {
    fn new(vertices: [Vec3; 3]) -> Self {
        Self {
            lo: vertices[0].min(vertices[1]).min(vertices[2]),
            hi: vertices[0].max(vertices[1]).max(vertices[2]),
            vertices,
        }
    }

    fn surface(&self, origin: Vec3) -> Surface {
        Surface {
            vertices: self.vertices.map(|p| to_sm64(p, origin).map(|x| x.round() as i32)),
            ..default()
        }
    }
}

/// World-space geometry indexed once when entering Mario, independently of
/// the small native collision patch and floating SM64 origin.
#[derive(Default)]
struct CollisionWorld {
    triangles: Vec<CollisionTriangle>,
    cells: HashMap<(i32, i32), Vec<usize>>,
}

impl CollisionWorld {
    fn insert(&mut self, triangle: CollisionTriangle) {
        let index = self.triangles.len();
        for x in collision_cell(triangle.lo.x)..=collision_cell(triangle.hi.x) {
            for z in collision_cell(triangle.lo.z)..=collision_cell(triangle.hi.z) {
                self.cells.entry((x, z)).or_default().push(index);
            }
        }
        self.triangles.push(triangle);
    }

    fn query(&self, lo: Vec3, hi: Vec3) -> Vec<usize> {
        let mut ids = Vec::new();
        for x in collision_cell(lo.x)..=collision_cell(hi.x) {
            for z in collision_cell(lo.z)..=collision_cell(hi.z) {
                if let Some(cell) = self.cells.get(&(x, z)) {
                    ids.extend_from_slice(cell);
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        ids.retain(|&i| {
            let t = &self.triangles[i];
            t.lo.x <= hi.x && t.hi.x >= lo.x && t.lo.z <= hi.z && t.hi.z >= lo.z
        });
        ids
    }
}

fn collision_cell(value: f32) -> i32 {
    (value / COLLISION_CELL).floor() as i32
}

#[derive(Default)]
struct RenderPose {
    positions: Vec<Vec3>,
    normals: Vec<[f32; 3]>,
    position: Vec3,
}

impl RenderPose {
    fn positions(&self, engine: &Engine, origin: Vec3, alpha: f32) -> Vec<Vec3> {
        let interpolate = self.positions.len() == engine.vertex_count;
        engine.positions[..engine.vertex_count].iter().enumerate().map(|(i, &p)| {
            let current = from_sm64(p, origin);
            if interpolate { self.positions[i].lerp(current, alpha) } else { current }
        }).collect()
    }
}

fn capture_pose(runtime: &mut Runtime) {
    let engine = runtime.engine.as_ref().unwrap();
    let n = engine.vertex_count;
    runtime.previous_pose.positions.clear();
    runtime.previous_pose.positions.extend(engine.positions[..n].iter()
        .map(|&p| from_sm64(p, runtime.origin)));
    runtime.previous_pose.normals.clear();
    runtime.previous_pose.normals.extend_from_slice(&engine.normals[..n]);
    runtime.previous_pose.position = from_sm64(engine.state.position, runtime.origin);
}

#[derive(Default)]
struct Runtime {
    engine: Option<Engine>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<MarioMaterial>>,
    outline_material: Option<Handle<MarioOutlineMaterial>>,
    mask_material: Option<Handle<MarioMaskMaterial>>,
    mask_image: Option<Handle<Image>>,
    mask_camera: Option<Entity>,
    outline: Option<Entity>,
    avatar: Option<Entity>,
    saved: Option<SavedCamera>,
    collision_world: CollisionWorld,
    static_triangles: Vec<CollisionTriangle>,
    terrain_triangles: Vec<CollisionTriangle>,
    surfaces: Vec<Surface>,
    // Restore every temporary reaction before normal view updates run again.
    restore: HashMap<Entity, Transform>,
    previous: HashMap<Entity, Vec3>,
    reactions: HashMap<Entity, f32>,
    origin: Vec3,
    patch: Vec3,
    spawn: Vec3,
    last_valid: Vec3,
    empty_ticks: u32,
    accumulator: f32,
    previous_pose: RenderPose,
    render_bounds: Option<(Vec3, Vec3)>,
    audio: NativeAudio,
    audio_entity: Option<Entity>,
    fire_cooldown: f32,
    jump_latched: bool,
    attack_latched: bool,
    demo_time: f32,
    demo_fire: Vec3,
    profile_collisions: bool,
    collision_count: usize,
}

pub struct MarioPlugin;
impl Plugin for MarioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<NativeAudio>();
        load_internal_asset!(app, MARIO_SHADER, "shaders/mario.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, MARIO_OUTLINE_SHADER, "shaders/mario_outline.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, MARIO_MASK_SHADER, "shaders/mario_mask.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<MarioMaterial> {
            prepass_enabled: false,
            shadows_enabled: false,
            ..default()
        });
        app.add_plugins(MaterialPlugin::<MarioOutlineMaterial> {
            prepass_enabled: false,
            shadows_enabled: false,
            ..default()
        });
        app.add_plugins(MaterialPlugin::<MarioMaskMaterial> {
            prepass_enabled: false,
            shadows_enabled: false,
            ..default()
        });
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
    k.title || (!k.title
        && !k.intro
        && k.tour.is_none()
        && k.started
        && k.phase == Phase::Pianifica
        && sim.speed <= 1.0)
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
    if k.title && !mode.active { return; }
    egui::Area::new(egui::Id::new("pause_mario"))
        .anchor(egui::Align2::LEFT_TOP, [16., 16.])
        .order(egui::Order::Foreground)
        .show(contexts.ctx_mut(), |ui| {
            ui.set_width(if mode.active { 540. } else { 160. });
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                if ui.button(if mode.active && k.title { "Torna al titolo · M / Start" }
                    else if mode.active { "Torna al piano · M / Start" } else { "Mario · M / Start" }).clicked() {
                    mode.toggle = true;
                }
                if mode.active {
                    if ui.button("Riposiziona sul terreno qui vicino · R / Y (△)").clicked() {
                        mode.respawn_nearby = true;
                    }
                    if k.title {
                        ui.label("Esplorazione libera · partita non iniziata");
                    } else {
                        ui.label(format!("Simulazione ×1 · {}", rocca::words::clock(sim.time_s())));
                    }
                    ui.label(controls.pad().and_then(|pad| controls.pads.name(pad))
                        .map_or("Gamepad non connesso".to_string(), |name| format!("Gamepad: {name}")));
                    ui.label("WASD / stick sinistro · Spazio / A (×): salto · J / X (□): pugno");
                    ui.label("Shift / LT: accovacciati · Frecce / stick destro: camera");
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

fn clear(mut commands: Commands, mut runtime: NonSendMut<Runtime>, mut mode: ResMut<Playground>) {
    runtime.audio.stop();
    if let Some(entity) = runtime.audio_entity.take() { commands.entity(entity).despawn_recursive(); }
    if let Some(camera) = runtime.mask_camera.take() { commands.entity(camera).despawn_recursive(); }
    if let Some(outline) = runtime.outline.take() { commands.entity(outline).despawn_recursive(); }
    if let Some(engine) = runtime.engine.as_mut() {
        engine.despawn();
        engine.set_audio_active(false);
        engine.load_surfaces(&[]);
    }
    runtime.avatar = None;
    runtime.saved = None;
    runtime.collision_world = CollisionWorld::default();
    runtime.static_triangles.clear();
    runtime.terrain_triangles.clear();
    runtime.surfaces.clear();
    runtime.restore.clear();
    runtime.previous.clear();
    runtime.reactions.clear();
    mode.active = false;
    mode.wing_cap = false;
    mode.toggle = false;
    mode.respawn_nearby = false;
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
    materials: ResMut<'w, Assets<MarioMaterial>>,
    outline_materials: ResMut<'w, Assets<MarioOutlineMaterial>>,
    mask_materials: ResMut<'w, Assets<MarioMaskMaterial>>,
    audio_sources: ResMut<'w, Assets<NativeAudio>>,
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
            Entity,
            &mut OrbitCamera,
            &mut Transform,
            Option<&mut bevy::core_pipeline::dof::DepthOfFieldSettings>,
            &mut Camera,
            Has<bevy::core_pipeline::prepass::DepthPrepass>,
            &Projection,
        ),
        Without<PauseSolid>,
    >,
    solids: Query<
        (
            &Handle<Mesh>,
            &Transform,
            Option<&crate::terrain_mesh::TerrainChunk>,
            Option<&crate::roads::RoadChunk>,
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
        ref mut outline_materials,
        ref mut mask_materials,
        ref mut audio_sources,
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
    let Ok((camera_entity, mut camera, mut camera_tf, mut dof, mut render_camera, depth_prepass, projection)) = cameras.get_single_mut()
    else {
        return;
    };
    if stop {
        runtime.audio.stop();
        if let Some(entity) = runtime.audio_entity.take() { commands.entity(entity).despawn_recursive(); }
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
            if !saved.depth_prepass {
                commands.entity(camera_entity).remove::<bevy::core_pipeline::prepass::DepthPrepass>();
            }
        }
        if let Some(avatar) = runtime.avatar.take() {
            commands.entity(avatar).despawn_recursive();
        }
        if let Some(camera) = runtime.mask_camera.take() { commands.entity(camera).despawn_recursive(); }
        if let Some(outline) = runtime.outline.take() { commands.entity(outline).despawn_recursive(); }
        if let Some(engine) = runtime.engine.as_mut() {
            engine.despawn();
            engine.set_audio_active(false);
            engine.load_surfaces(&[]);
        }
        runtime.collision_world = CollisionWorld::default();
        runtime.static_triangles.clear();
        runtime.terrain_triangles.clear();
        runtime.reactions.clear();
        runtime.previous.clear();
        mode.active = false;
        return;
    }
    if !mode.active {
        if reset && toggle { mode.toggle = true; }
        if !can_play || !toggle || !focused || reset {
            return;
        }
        mode.error = None;
        runtime.profile_collisions = std::env::var_os("MARIO_COLLISION_PROFILE").is_some();
        if runtime.engine.is_none() {
            match load_engine(images, materials) {
                Ok((engine, material)) => {
                    runtime.mask_material = Some(mask_materials.add(MarioMaskMaterial {
                        atlas: materials.get(&material).unwrap().atlas.clone(),
                    }));
                    runtime.engine = Some(engine);
                    runtime.material = Some(material);
                }
                Err(e) => {
                    mode.error = Some(e);
                    return;
                }
            }
        }
        runtime.collision_world = CollisionWorld::default();
        for (handle, tf, terrain, road) in &solids {
            // Draped roads share the smoothed terrain floor. Loading their
            // unsmoothed ribbons would reintroduce bumps and overlapping floors.
            if terrain.is_some() || road.is_some() {
                continue;
            }
            if let Some(mesh) = meshes.get(handle) {
                triangles(mesh, |v| {
                    runtime.collision_world.insert(CollisionTriangle::new(
                        v.map(|p| tf.transform_point(p)),
                    ));
                });
            }
        }
        runtime.saved = Some(SavedCamera {
            focus: camera.focus,
            distance: camera.distance,
            yaw: camera.yaw,
            pitch: camera.pitch,
            transform: *camera_tf,
            blur: dof.as_ref().map_or(0., |d| d.max_circle_of_confusion_diameter),
            viewport: render_camera.viewport.clone(),
            depth_prepass,
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
        runtime.last_valid = base;
        runtime.patch = Vec3::splat(f32::INFINITY);
        runtime.accumulator = 0.;
        runtime.fire_cooldown = 0.;
        runtime.jump_latched = false;
        runtime.attack_latched = false;
        if !respawn_nearby(&mut runtime, &meshes, &sim, base, &mut actors) {
            mode.error = Some("Mario non trova un terreno su cui stare.".into());
            runtime.saved = None;
            return;
        }
        runtime
            .engine
            .as_mut()
            .unwrap()
            .set_wing_cap(std::mem::take(&mut mode.snapshot_wing_cap));
        let size = windows.get_single().map_or(UVec2::new(1280, 720), |w|
            UVec2::new(w.physical_width().max(1), w.physical_height().max(1)));
        let mask = runtime.mask_image.get_or_insert_with(|| {
            let mut image = Image::new_fill(
                Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 },
                TextureDimension::D2, &[0; 8], TextureFormat::Rgba16Float,
                RenderAssetUsages::default(),
            );
            image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
            image.sampler = ImageSampler::nearest();
            images.add(image)
        }).clone();
        let outline_material = outline_materials.add(MarioOutlineMaterial {
            mask: mask.clone(), bounds: Vec4::new(-1.0, -1.0, 1.0, 1.0),
        });
        runtime.outline_material = Some(outline_material.clone());
        runtime.mask_camera = Some(commands.spawn((
            Camera3dBundle {
                camera: Camera {
                    order: -1,
                    // Keep depth values in a linear float target throughout;
                    // an LDR intermediate quantizes them like colour data.
                    hdr: true,
                    target: bevy::render::camera::RenderTarget::Image(mask),
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                tonemapping: bevy::core_pipeline::tonemapping::Tonemapping::None,
                deband_dither: bevy::core_pipeline::tonemapping::DebandDither::Disabled,
                ..default()
            },
            RenderLayers::layer(MASK_LAYER),
            crate::camera::AuxiliaryCamera,
        )).id());
        runtime.outline = Some(commands.spawn((
            MaterialMeshBundle::<MarioOutlineMaterial> {
                mesh: meshes.add(Rectangle::new(2.0, 2.0)),
                material: outline_material,
                ..default()
            },
            NoFrustumCulling,
        )).id());
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
                    MaterialMeshBundle::<MarioMaterial> {
                        mesh: handle.clone(),
                        material: runtime.material.clone().unwrap(),
                        ..default()
                    },
                    Avatar,
                    NoFrustumCulling,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        MaterialMeshBundle::<MarioMaskMaterial> {
                            mesh: handle,
                            material: runtime.mask_material.clone().unwrap(),
                            ..default()
                        },
                        NoFrustumCulling,
                        RenderLayers::layer(MASK_LAYER),
                    ));
                })
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
        runtime.engine.as_mut().unwrap().set_audio_active(true);
        runtime.audio.start();
        runtime.audio_entity = Some(commands.spawn(AudioSourceBundle::<NativeAudio> {
            source: audio_sources.add(runtime.audio.clone()),
            settings: PlaybackSettings::ONCE,
            ..default()
        }).id());
        mode.message = "";
    }
    if !focused {
        runtime.accumulator = 0.;
        runtime.audio.clear();
        capture_pose(&mut runtime);
        return;
    }
    if !depth_prepass {
        commands.entity(camera_entity).insert(bevy::core_pipeline::prepass::DepthPrepass);
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
    let respawn = std::mem::take(&mut mode.respawn_nearby)
        || pressed(GamepadButtonType::North)
        || keyboard_free && controls.keys.just_pressed(KeyCode::KeyR);
    if respawn {
        let pos = from_sm64(runtime.engine.as_ref().unwrap().state.position, runtime.origin);
        if respawn_nearby(&mut runtime, &meshes, &sim, pos, &mut actors) {
            mode.wing_cap = false;
            mode.error = None;
            mode.message = "Riposizionato sul terreno qui vicino.";
        } else {
            mode.error = Some("Non trovo un punto libero qui vicino. Prova a spostarti.".into());
        }
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
        // Save world-space pose before possible floating-origin recentering.
        capture_pose(&mut runtime);
        runtime.fire_cooldown = (runtime.fire_cooldown - STEP).max(0.);
        let mut pos = from_sm64(
            runtime.engine.as_ref().unwrap().state.position,
            runtime.origin,
        );
        if !pos.is_finite() {
            let fallback = runtime.last_valid;
            if !respawn_nearby(&mut runtime, &meshes, &sim, fallback, &mut actors) {
                mode.error = Some("Riposizionamento fallito: usa il pulsante per riprovare.".into());
                break;
            }
            pos = runtime.last_valid;
        }
        let profile_start = runtime.profile_collisions.then(std::time::Instant::now);
        refresh_patch(&mut runtime, &meshes, &sim, pos, &mut actors);
        let refresh_us = profile_start.map(|start| start.elapsed().as_micros());
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
        if let Some(start) = profile_start {
            eprintln!("mario collision: surfaces={} refresh_us={} tick_us={}",
                runtime.collision_count, refresh_us.unwrap(),
                start.elapsed().as_micros().saturating_sub(refresh_us.unwrap()));
        }
        let engine = runtime.engine.as_ref().unwrap();
        let pos = from_sm64(engine.state.position, runtime.origin);
        if pos.is_finite() && !water && runtime.fire_cooldown == 0. && touching_fire(&sim, &buildings, pos) {
            let source = to_sm64(pos - Vec3::X * 2., runtime.origin);
            runtime.engine.as_mut().unwrap().hurt(source, true);
            runtime.fire_cooldown = 2.;
            mode.message = "Ahi! Il fuoco brucia.";
        }
        if pos.is_finite() { interact(&mut runtime, &mut mode, &mut actors, &meshes, pos); }
        let engine = runtime.engine.as_ref().unwrap();
        mode.health = engine.state.health;
        mode.wing_cap = engine.has_wing_cap();
        let dead = engine.state.health < 0x100;
        runtime.empty_ticks = if engine.vertex_count == 0 { runtime.empty_ticks + 1 } else { 0 };
        let ground = if pos.is_finite() { collision_ground(&sim, pos.x, pos.z) } else { 0.0 };
        if recovery_needed(pos, ground, runtime.empty_ticks) || dead {
            if respawn_nearby(&mut runtime, &meshes, &sim, pos, &mut actors) {
                mode.wing_cap = false;
                mode.message = "Mario recuperato sul terreno qui vicino.";
                mode.error = None;
            } else {
                mode.error = Some("Recupero fallito: usa il pulsante per riprovare.".into());
            }
        } else {
            runtime.last_valid = pos;
        }
        let queued = runtime.audio.queued_frames();
        let mut pcm = [0; libsm64_native::AUDIO_BUFFER_SAMPLES];
        let count = runtime.engine.as_mut().unwrap().audio_tick(queued, &mut pcm);
        runtime.audio.push(&pcm[..count]);
    }
    // Reactions are visual, reversible, and applied after the ordinary views.
    let origin = runtime.origin;
    let pos = from_sm64(runtime.engine.as_ref().unwrap().state.position, origin);
    let pos = if pos.is_finite() { pos } else { runtime.last_valid };
    let alpha = (runtime.accumulator / STEP).clamp(0.0, 1.0);
    let camera_pos = runtime.previous_pose.position.lerp(pos, alpha);
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
    // Count changes denote cap/visibility topology changes. Eye textures can
    // switch instantly without interrupting the body's interpolated motion.
    let interpolate = runtime.previous_pose.positions.len() == engine.vertex_count;
    let positions = runtime.previous_pose.positions(engine, origin, alpha);
    let bounds = positions.iter().fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(lo, hi), &p| (lo.min(p), hi.max(p)));
    if let Some(mesh) = runtime.mesh.as_ref().and_then(|h| meshes.get_mut(h)) {
        let n = engine.vertex_count;
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            positions.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, engine.normals[..n].iter().enumerate()
            .map(|(i, &normal)| {
                let current = Vec3::from_array(normal);
                if interpolate {
                    Vec3::from_array(runtime.previous_pose.normals[i]).lerp(current, alpha)
                        .normalize_or_zero().to_array()
                } else { normal }
            }).collect::<Vec<_>>());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, engine.uvs[..n].to_vec());
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            engine.colors[..n]
                .iter()
                .map(|c| [c[0], c[1], c[2], 1.])
                .collect::<Vec<_>>(),
        );
    }
    runtime.render_bounds = Some(bounds);
    camera.focus = camera_pos + Vec3::Y * 5.;
    let dir = Vec3::new(
        camera.yaw.sin() * camera.pitch.cos(),
        -camera.pitch.sin(),
        camera.yaw.cos() * camera.pitch.cos(),
    );
    // Camera uses its own ray-sized query, not Mario's tiny physics patch.
    let end = camera.focus + dir * camera.distance;
    let distance = runtime.collision_world
        .query(camera.focus.min(end), camera.focus.max(end))
        .into_iter()
        .filter_map(|i| {
            ray_triangle(camera.focus, dir, runtime.collision_world.triangles[i].vertices)
                .filter(|&t| t > 2.0)
        })
        .fold(camera.distance, |distance, hit| {
            distance.min((hit - 1.).max(3.))
        });
    let eye = camera.focus + dir * distance;
    let ground = sim.scn.terrain.height_at(crate::frame::to_world(eye));
    camera_tf.translation = eye.with_y(eye.y.max(ground + 3.));
    camera_tf.look_at(camera.focus, Vec3::Y);
    if let Some(mask_camera) = runtime.mask_camera {
        commands.entity(mask_camera).insert((*camera_tf, projection.clone()));
    }
    if let Some(mask) = runtime.mask_image.as_ref().and_then(|h| images.get_mut(h)) {
        if let Ok(window) = windows.get_single() {
            let size = Extent3d { width: window.physical_width().max(1),
                height: window.physical_height().max(1), depth_or_array_layers: 1 };
            if mask.texture_descriptor.size != size { mask.resize(size); }
        }
    }
    if let Some(outline) = runtime.outline {
        commands.entity(outline).insert(Transform::from_translation(pos));
    }
    if let Some(material) = runtime.outline_material.as_ref().and_then(|h| outline_materials.get_mut(h)) {
        let engine = runtime.engine.as_ref().unwrap();
        if engine.vertex_count == 0 {
            material.bounds = Vec4::ZERO;
            return;
        }
        let (lo, hi) = runtime.render_bounds.unwrap();
        let clip_from_world = projection.get_clip_from_view() * camera_tf.compute_matrix().inverse();
        let mut min = Vec2::ONE;
        let mut max = -Vec2::ONE;
        for x in [lo.x, hi.x] {
            for y in [lo.y, hi.y] {
                for z in [lo.z, hi.z] {
                    let clip = clip_from_world * Vec3::new(x, y, z).extend(1.0);
                    if clip.w <= 0.0 { min = -Vec2::ONE; max = Vec2::ONE; continue; }
                    let ndc = clip.truncate().truncate() / clip.w;
                    min = min.min(ndc);
                    max = max.max(ndc);
                }
            }
        }
        let size = windows.get_single().map_or(Vec2::new(1280., 720.), |w|
            Vec2::new(w.physical_width().max(1) as f32, w.physical_height().max(1) as f32));
        let padding = Vec2::splat(6.0) / size;
        // Restrict the mask filter to Mario's screen rectangle: no full-screen
        // nine-tap pass for a character occupying only a few thousand pixels.
        let lower = (min - padding).max(-Vec2::ONE);
        let upper = (max + padding).min(Vec2::ONE);
        material.bounds = Vec4::new(lower.x, lower.y, upper.x, upper.y);
    }
}

fn load_engine(
    images: &mut Assets<Image>,
    materials: &mut Assets<MarioMaterial>,
) -> Result<(Engine, Handle<MarioMaterial>), String> {
    let path = std::env::var_os("MARIO64_ROM")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("data/mario64/Super Mario 64 (USA).z64"));
    let rom = std::fs::read(&path).map_err(|e| format!("ROM {}: {e}", path.display()))?;
    load_engine_from_rom(&rom, images, materials)
}

fn load_engine_from_rom(
    rom: &[u8],
    images: &mut Assets<Image>,
    materials: &mut Assets<MarioMaterial>,
) -> Result<(Engine, Handle<MarioMaterial>), String> {
    let (mut engine, atlas) = Engine::new(rom).map_err(str::to_owned)?;
    engine.init_audio(rom).map_err(str::to_owned)?;
    // Preserve ROM alpha: facial decals blend with shade, wings cut out.
    let mut image = Image::new(
        Extent3d {
            width: libsm64_native::TEXTURE_WIDTH,
            height: libsm64_native::TEXTURE_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        atlas,
        // N64 combines and interpolates the original encoded colour values.
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    let material = materials.add(MarioMaterial {
        atlas: images.add(image),
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
    let mut emit = |i: [usize; 3]| visit(i.map(|index| Vec3::from_array(p[index])));
    match mesh.indices() {
        Some(Indices::U32(indices)) => {
            for i in indices.chunks_exact(3) { emit([i[0] as usize, i[1] as usize, i[2] as usize]); }
        }
        Some(Indices::U16(indices)) => {
            for i in indices.chunks_exact(3) { emit([i[0] as usize, i[1] as usize, i[2] as usize]); }
        }
        None => {
            for i in (0..p.len().saturating_sub(2)).step_by(3) { emit([i, i + 1, i + 2]); }
        }
    }
}

fn close_triangle(t: &CollisionTriangle, pos: Vec3) -> bool {
    near(t.lo, t.hi, pos, COLLISION_RADIUS)
        && t.lo.y <= pos.y + 12.0 && t.hi.y >= pos.y - 24.0
}

fn smooth_height(x: f32, z: f32, spacing: f32, sample: impl Fn(f32, f32) -> f32) -> f32 {
    let centre = sample(x, z);
    let weights = [1.0, 2.0, 1.0];
    let mut sum = 0.0;
    for (iz, wz) in weights.iter().enumerate() {
        for (ix, wx) in weights.iter().enumerate() {
            sum += wx * wz * sample(x + (ix as f32 - 1.0) * spacing,
                                    z + (iz as f32 - 1.0) * spacing);
        }
    }
    // Remove small DEM bumps without letting the invisible floor drift far
    // from roads, building entrances and the rendered terrain.
    centre + (sum / 16.0 - centre).clamp(-1.25, 1.25)
}

fn collision_ground(sim: &Sim, x: f32, z: f32) -> f32 {
    let terrain = &sim.scn.terrain;
    let p = scenario::Pos { x, y: -z };
    if crate::sea::is_water(&sim.scn, p) {
        return terrain.height_at(p) - (crate::sea::shore_distance(&sim.scn, p) * 0.3).min(20.0);
    }
    smooth_height(x, z, terrain.posting, |x, z| {
        terrain.height_at(scenario::Pos { x, y: -z })
    }) + 0.15
}

fn terrain_patch(pos: Vec3, height: impl Fn(f32, f32) -> f32) -> Vec<CollisionTriangle> {
    // Snap to a world-fixed lattice: overlapping patches have identical
    // vertices, so refreshing does not move the floor under Mario's feet.
    let x0 = ((pos.x - PATCH_RADIUS) / TERRAIN_STEP).floor() as i32;
    let x1 = ((pos.x + PATCH_RADIUS) / TERRAIN_STEP).ceil() as i32;
    let z0 = ((pos.z - PATCH_RADIUS) / TERRAIN_STEP).floor() as i32;
    let z1 = ((pos.z + PATCH_RADIUS) / TERRAIN_STEP).ceil() as i32;
    let width = (x1 - x0 + 1) as usize;
    let mut vertices = Vec::new();
    for z in z0..=z1 {
        for x in x0..=x1 {
            let (x, z) = (x as f32 * TERRAIN_STEP, z as f32 * TERRAIN_STEP);
            vertices.push(Vec3::new(x, height(x, z), z));
        }
    }
    let mut triangles = Vec::new();
    for z in 0..(z1 - z0) as usize {
        for x in 0..(x1 - x0) as usize {
            let i = z * width + x;
            for indices in [[i, i + width, i + 1], [i + 1, i + width, i + width + 1]] {
                triangles.push(CollisionTriangle::new(indices.map(|i| vertices[i])));
            }
        }
    }
    triangles
}

fn refresh_patch(
    runtime: &mut Runtime,
    meshes: &Assets<Mesh>,
    sim: &Sim,
    pos: Vec3,
    actors: &mut Actors,
) {
    // World geometry stays fixed; only its native coordinates are recentered.
    let recenter = (pos - runtime.origin).abs().max_element() > 150.;
    if recenter {
        runtime.origin = pos;
        runtime.engine.as_mut().unwrap().position([0.; 3]);
    }
    if !runtime.patch.is_finite() || runtime.patch.distance(pos) > PATCH_STEP {
        runtime.patch = pos;
        let ids = runtime.collision_world.query(pos - Vec3::splat(PATCH_RADIUS),
                                               pos + Vec3::splat(PATCH_RADIUS));
        runtime.static_triangles = ids.into_iter()
            .map(|i| runtime.collision_world.triangles[i].clone()).collect();
        runtime.terrain_triangles = terrain_patch(pos, |x, z| collision_ground(sim, x, z));
    }
    // Reuse the Rust buffer; send libsm64 only immediate scenery, with height
    // culling too. Always retain a terrain floor below airborne Mario.
    runtime.surfaces.clear();
    for t in &runtime.static_triangles {
        if close_triangle(t, pos) { runtime.surfaces.push(t.surface(runtime.origin)); }
    }
    for t in &runtime.terrain_triangles {
        if near(t.lo, t.hi, pos, COLLISION_RADIUS) {
            runtime.surfaces.push(t.surface(runtime.origin));
        }
    }
    for (_, handle, tf, visibility, _) in actors.iter_mut() {
        if *visibility == Visibility::Hidden { continue; }
        // Expanded broad phase accounts for large vehicles/team symbols.
        if !near(tf.translation, tf.translation, pos, COLLISION_RADIUS + 16.0) { continue; }
        if let Some(mesh) = meshes.get(handle) {
            triangles(mesh, |vertices| {
                let t = CollisionTriangle::new(vertices.map(|p| tf.transform_point(p)));
                if close_triangle(&t, pos) { runtime.surfaces.push(t.surface(runtime.origin)); }
            });
        }
    }
    runtime.collision_count = runtime.surfaces.len();
    runtime.engine.as_mut().unwrap().load_surfaces(&runtime.surfaces);
}

fn recovery_needed(pos: Vec3, ground: f32, empty_ticks: u32) -> bool {
    !pos.is_finite() || pos.y < ground - 12.0 || empty_ticks >= 30
}

fn respawn_nearby(
    runtime: &mut Runtime,
    meshes: &Assets<Mesh>,
    sim: &Sim,
    requested: Vec3,
    actors: &mut Actors,
) -> bool {
    let base = if requested.is_finite() { requested } else { runtime.last_valid };
    let old_origin = runtime.origin;
    let old_position = from_sm64(runtime.engine.as_ref().unwrap().state.position, old_origin);
    let mut changed_patch = false;
    // Check candidates in world space before moving/recentering native Mario.
    // Never rely on collision data still loaded around a distant old spawn.
    for i in 0..49 {
        let angle = i as f32 * 2.4;
        let radius = 3.0 * (i as f32).sqrt();
        let p = base + Vec3::new(angle.cos() * radius, 0., angle.sin() * radius);
        let world = crate::frame::to_world(p);
        if world.x < 0.0 || world.y < 0.0 || world.x > sim.scn.world.width_m
            || world.y > sim.scn.world.height_m {
            continue;
        }
        let ground = collision_ground(sim, p.x, p.z);
        let feet = p.with_y(ground + 0.3);
        let ids = runtime.collision_world.query(feet - Vec3::splat(3.0), feet + Vec3::splat(3.0));
        let blocked = ids.iter().any(|&i| {
            let t = &runtime.collision_world.triangles[i];
            (near(t.lo, t.hi, feet, 3.0) && t.hi.y > ground + 0.5 && t.lo.y < ground + 10.0)
                || ray_triangle(feet, Vec3::Y, t.vertices).is_some_and(|d| d < 100.0)
        });
        if blocked { continue; }
        if actors.iter_mut().any(|(_, handle, tf, visibility, _)| {
            if *visibility == Visibility::Hidden || tf.translation.distance(feet) > 30.0 { return false; }
            meshes.get(handle).and_then(|mesh| mesh_bounds(mesh, &tf))
                .is_some_and(|(lo, hi)| near(lo, hi, feet, 3.0)
                    && hi.y > ground && lo.y < ground + 10.0)
        }) { continue; }
        runtime.origin = feet;
        runtime.patch = Vec3::splat(f32::INFINITY);
        refresh_patch(runtime, meshes, sim, feet, actors);
        changed_patch = true;
        let floor = runtime.engine.as_ref().unwrap().floor_height([0., 80.0, 0.]) / SCALE + feet.y;
        if !floor.is_finite() || (floor - ground).abs() > 2.0 { continue; }
        let spawn = feet.with_y(floor + 2.0);
        if !runtime.engine.as_mut().unwrap().spawn(to_sm64(spawn, runtime.origin)) { continue; }
        runtime.spawn = spawn;
        runtime.last_valid = spawn;
        runtime.empty_ticks = 0;
        runtime.fire_cooldown = 2.0;
        runtime.jump_latched = false;
        runtime.attack_latched = false;
        // spawn() clears geometry. Populate it immediately so a button click
        // or a recovery between fixed ticks cannot produce an empty avatar.
        let water_level = if crate::sea::is_water(&sim.scn, world) {
            ((0.8 - runtime.origin.y) * SCALE) as i32
        } else { -10000 };
        runtime.engine.as_mut().unwrap().water_level(water_level);
        runtime.engine.as_mut().unwrap().tick(&Input::default());
        runtime.audio.clear();
        capture_pose(runtime);
        return true;
    }
    if changed_patch {
        runtime.origin = old_origin;
        runtime.patch = Vec3::splat(f32::INFINITY);
        if old_position.is_finite() { refresh_patch(runtime, meshes, sim, old_position, actors); }
    }
    false
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
    fn rom_atlas_retains_wing_cutouts_and_white_feathers() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/mario64/Super Mario 64 (USA).z64");
        let Ok(rom) = std::fs::read(path) else {
            eprintln!("Skipping ROM-dependent rendering check: local ROM unavailable.");
            return;
        };
        let mut images = Assets::<Image>::default();
        let mut materials = Assets::<MarioMaterial>::default();
        let (mut engine, handle) = load_engine_from_rom(&rom, &mut images, &mut materials).unwrap();
        let material = materials.get(&handle).unwrap();
        assert_eq!(material.alpha_mode(), AlphaMode::Mask(0.5));
        let image = images.get(&material.atlas).unwrap();
        let wing_pixels: Vec<_> = image.data.chunks_exact(4).enumerate()
            .filter(|(i, _)| {
                let x = i % libsm64_native::TEXTURE_WIDTH as usize;
                x >= 9 * 64 && x % 64 < 32 // exclude empty atlas padding
            })
            .map(|(_, pixel)| pixel).collect();
        assert!(wing_pixels.iter().any(|p| p[3] == 0), "feather gaps retain ROM alpha");
        assert!(wing_pixels.iter().any(|p| p[0] > 240 && p[1] > 240 && p[2] > 240 && p[3] == 255),
            "white feathers remain opaque");
        engine.load_surfaces(&[Surface {
            vertices: [[-8000, 0, -8000], [-8000, 0, 8000], [8000, 0, -8000]],
            ..default()
        }]);
        assert!(engine.spawn([0., 100., 0.]));
        engine.set_wing_cap(true);
        engine.tick(&Input::default());
        assert!(engine.uvs[..engine.vertex_count].iter().any(|uv| *uv == [1., 1.]),
            "body uses the opaque untextured sentinel");
        assert!(engine.uvs[..engine.vertex_count].iter().any(|uv| uv[0] >= 9. / 11. && uv[0] < 1.),
            "native Wing Cap uses the cutout tiles");

        // Travel beyond both the old patch and origin, then recreate Mario
        // only after loading a new local floor. Geometry must exist at once.
        let origin = Vec3::new(300., 50., -200.);
        let surfaces: Vec<_> = terrain_patch(origin, |_, _| 50.0).iter()
            .map(|t| t.surface(origin)).collect();
        engine.load_surfaces(&surfaces);
        assert!(engine.spawn([0., 40., 0.]));
        engine.water_level(-10000);
        for _ in 0..90 { engine.tick(&Input::default()); }
        assert!(engine.state.position[1].abs() < 0.1);
        assert!(engine.vertex_count > 0);

        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let sim = Sim::new(&data, "Rocco3", 17).unwrap();
        let base = rocca_start(&sim).unwrap().0;
        let mut runtime = Runtime {
            engine: Some(engine), origin: base, last_valid: base,
            ..default()
        };
        let meshes = Assets::<Mesh>::default();
        let mut world = World::new();
        let mut state = bevy::ecs::system::SystemState::<Actors>::new(&mut world);
        let mut actors = state.get_mut(&mut world);
        assert!(respawn_nearby(&mut runtime, &meshes, &sim, base, &mut actors));
        let distant = base + Vec3::Z * 300.;
        assert!(respawn_nearby(&mut runtime, &meshes, &sim, distant, &mut actors));
        assert!((runtime.spawn.x - distant.x).hypot(runtime.spawn.z - distant.z) < 22.0);
        assert!(runtime.engine.as_ref().unwrap().vertex_count > 0);
        // The render pose survives origin recentering, and a render frame
        // between physics ticks shows an intermediate animated pose.
        for _ in 0..90 { runtime.engine.as_mut().unwrap().tick(&Input::default()); }
        capture_pose(&mut runtime);
        let old_position = from_sm64(runtime.engine.as_ref().unwrap().state.position, runtime.origin);
        runtime.origin += Vec3::X * 150.0;
        let local = to_sm64(old_position, runtime.origin);
        runtime.engine.as_mut().unwrap().position(local);
        refresh_patch(&mut runtime, &meshes, &sim, old_position, &mut actors);
        runtime.engine.as_mut().unwrap().tick(&Input::default());
        let engine = runtime.engine.as_ref().unwrap();
        let a = runtime.previous_pose.positions(engine, runtime.origin, 0.0);
        let b = runtime.previous_pose.positions(engine, runtime.origin, 0.5);
        let c = runtime.previous_pose.positions(engine, runtime.origin, 1.0);
        assert!(a == runtime.previous_pose.positions, "interpolation starts at the captured world-space pose");
        assert!(a.iter().zip(&c).any(|(a, c)| a.distance(*c) > 0.01));
        for ((a, b), c) in a.iter().zip(&b).zip(&c) {
            assert!(b.distance((*a + *c) * 0.5) < 0.001);
            assert!(b.is_finite());
        }
        runtime.engine.as_mut().unwrap().set_wing_cap(true);
        runtime.engine.as_mut().unwrap().tick(&Input::default());
        let engine = runtime.engine.as_ref().unwrap();
        assert_ne!(engine.vertex_count, runtime.previous_pose.positions.len());
        let wing_pose = runtime.previous_pose.positions(engine, runtime.origin, 0.5);
        assert!(wing_pose == engine.positions[..engine.vertex_count].iter()
            .map(|&p| from_sm64(p, runtime.origin)).collect::<Vec<_>>(),
            "topology changes display the new cap immediately");
        assert!(runtime.collision_count < 600);
        let origin = runtime.origin;
        let position = runtime.engine.as_ref().unwrap().state.position;
        assert!(!respawn_nearby(&mut runtime, &meshes, &sim, Vec3::splat(100000.), &mut actors));
        assert_eq!(runtime.origin, origin, "a failed search leaves Mario in place");
        assert_eq!(runtime.engine.as_ref().unwrap().state.position, position);
        assert!(respawn_nearby(&mut runtime, &meshes, &sim, Vec3::splat(f32::NAN), &mut actors));
        assert!(runtime.engine.as_ref().unwrap().vertex_count > 0);
    }

    #[test]
    fn spatial_collision_query_is_local_unique_and_handles_negative_cells() {
        let mut world = CollisionWorld::default();
        let near = CollisionTriangle::new([
            Vec3::new(-20., 0., -20.), Vec3::new(-20., 0., 20.), Vec3::new(20., 0., -20.),
        ]);
        world.insert(near);
        world.insert(CollisionTriangle::new([
            Vec3::new(200., 0., 200.), Vec3::new(200., 0., 210.), Vec3::new(210., 0., 200.),
        ]));
        assert_eq!(world.query(Vec3::splat(-2.), Vec3::splat(2.)), vec![0]);
        assert!(world.query(Vec3::splat(60.), Vec3::splat(80.)).is_empty());
        assert!(!close_triangle(&world.triangles[0], Vec3::new(0., 80., 0.)));
    }

    #[test]
    fn collision_smoothing_removes_bumps_and_preserves_slopes() {
        let plane = |x: f32, z: f32| 20.0 + x * 0.2 + z * 0.1;
        assert!((smooth_height(4., -3., 5., plane) - plane(4., -3.)).abs() < 0.001);
        let bump = |x: f32, z: f32| if x == 0. && z == 0. { 1.0 } else { 0.0 };
        assert_eq!(smooth_height(0., 0., 5., bump), 0.25);
        let spike = |x: f32, z: f32| if x == 0. && z == 0. { 10.0 } else { 0.0 };
        assert_eq!(smooth_height(0., 0., 5., spike), 8.75);
    }

    #[test]
    fn terrain_patches_share_identical_vertices_and_upward_winding() {
        let height = |x: f32, z: f32| x * 0.1 + z * 0.2;
        let a = terrain_patch(Vec3::ZERO, height);
        let b = terrain_patch(Vec3::new(7., 100., 0.), height);
        let triangle = a.iter().find(|t| t.vertices.iter().all(|p|
            p.x >= 0. && p.x <= 8. && p.z >= 0. && p.z <= 8.)).unwrap();
        for p in triangle.vertices {
            assert!(b.iter().any(|t| t.vertices.contains(&p)));
        }
        for t in &a {
            assert!((t.vertices[1] - t.vertices[0]).cross(t.vertices[2] - t.vertices[0]).y > 0.0);
        }
        // Margin survives a refresh interval plus fastest native flight step.
        assert!(PATCH_RADIUS - PATCH_STEP > COLLISION_RADIUS);
    }

    #[test]
    fn recovery_uses_local_ground_instead_of_a_moving_origin() {
        assert!(recovery_needed(Vec3::new(200., 30., 100.), 50., 0));
        assert!(!recovery_needed(Vec3::new(200., 130., 100.), 50., 0));
        assert!(recovery_needed(Vec3::splat(f32::NAN), 0., 0));
        assert!(!recovery_needed(Vec3::ZERO, 0., 1)); // normal invincibility blink
        assert!(recovery_needed(Vec3::ZERO, 0., 30));
    }
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
