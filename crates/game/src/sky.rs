//! Sun position, sky dome and ambient atmosphere.
//!
//! The sun used to be a fixed `Transform` chosen by eye ("late-afternoon,
//! south-west"). It is now computed from actual solar geometry for
//! Spotorno's latitude, advanced by the simulated clock, so a two-hour
//! incident visibly moves the shadows and a scenario started at dawn looks
//! nothing like one started at dusk. [`DayClock::start_hour`] is what
//! `SPOTORNO_START_HOUR` sets: the simulation can be run at any hour, not
//! just the one the fire was originally tuned for.
//!
//! Longitude and the equation of time are deliberately left out: the game's
//! clock is "hours since start", not a civil time in a timezone, so local
//! solar time is the more honest thing to advance. Declination is likewise
//! frozen at one representative midsummer day — it drifts a fraction of a
//! degree over the length of any one incident, and pulling in a calendar to
//! track that would buy nothing a player could see.
//!
//! The same [`SunState`] this module computes is what [`crate::sea`] lights
//! its glint from, so the sky, the terrain's shadows and the sea's highlight
//! all agree about where the sun is.

use bevy::asset::load_internal_asset;
use bevy::pbr::{FogFalloff, FogSettings, Material, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderRef, ShaderType, SpecializedMeshPipelineError,
};

use crate::sim::Sim;

/// Spotorno, see `CLAUDE.md`. Only latitude enters the solar geometry below.
const SITE_LAT_DEG: f32 = 44.2265;

/// A fixed representative midsummer day — Ligurian fire season peaks
/// July-August — used for the sun's declination. See the module doc for why
/// this is frozen rather than calendar-driven.
const DAY_OF_YEAR: f32 = 210.0;

const SKY_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x7A11_9C40_55EE_4B21_9E33_11AA_22BB_33CC);

/// Radius of the sky dome. Comfortably inside the camera's far plane
/// (40 000 m, see `main::setup_scene`) and comfortably outside anywhere the
/// orbit camera can reach around a 10.24 km scenario.
pub const SKY_RADIUS: f32 = 30_000.0;

/// Local solar time at `T+0`, in hours. `SPOTORNO_START_HOUR` overrides it —
/// the scenario was tuned around a late-afternoon start, but nothing about
/// the fire model requires that hour, so the sun should not either.
#[derive(Resource)]
pub struct DayClock {
    pub start_hour: f32,
}

impl Default for DayClock {
    fn default() -> Self {
        let start_hour = std::env::var("SPOTORNO_START_HOUR")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(16.5);
        DayClock { start_hour: start_hour.rem_euclid(24.0) }
    }
}

/// The sun's direction and colour as of the last [`update_sky`] run, shared
/// with anything else that needs to agree with it — currently `crate::sea`.
#[derive(Resource, Clone, Copy)]
pub struct SunState {
    /// Unit vector from the ground toward the sun, in Bevy world space.
    pub direction: Vec3,
    pub color: Vec3,
    /// 0 at and under the horizon, 1 at full daylight. A `sin(elevation)`
    /// gate — deliberately conservative, so it is right for gating things
    /// that should have no business happening at night (a sun glint) but
    /// wrong for anything meant to track how bright the *scene* actually is:
    /// it is still only ~0.5 at a properly sunny 30° mid-afternoon sun.
    pub day_gate: f32,
    /// `illuminance / ILLUMINANCE`'s own daylight ceiling, clamped to
    /// [0, 1] — tracks the same curve the `DirectionalLight` itself is lit
    /// from, so anything blended by this looks as bright as the rest of the
    /// sunlit scene rather than dimming early the way `day_gate` does.
    pub brightness: f32,
}

impl Default for SunState {
    fn default() -> Self {
        SunState { direction: Vec3::Y, color: Vec3::ONE, day_gate: 0.0, brightness: 0.0 }
    }
}

/// Marks the single `DirectionalLight` entity [`update_sky`] drives. Spawned
/// in `main::setup_scene` with no particular transform or illuminance — both
/// are overwritten the first time `update_sky` runs.
#[derive(Component)]
pub struct Sun;

#[derive(Component)]
struct SkyDome;

#[derive(Resource)]
pub(crate) struct SkyHandle(Handle<SkyMaterial>);

#[derive(Clone, Copy, Default, ShaderType)]
pub struct SkyUniform {
    pub sun_dir: Vec4,
    pub sun_color: Vec4,
    pub zenith_color: Vec4,
    pub horizon_color: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct SkyMaterial {
    #[uniform(0)]
    pub uniform: SkyUniform,
}

impl Material for SkyMaterial {
    fn fragment_shader() -> ShaderRef {
        SKY_SHADER_HANDLE.into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Seen from inside the dome; winding is whatever `uv_sphere` produced
        // and does not matter here.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, SKY_SHADER_HANDLE, "shaders/sky.wgsl", Shader::from_wgsl);
        app.init_resource::<DayClock>()
            .init_resource::<SunState>()
            .add_plugins(MaterialPlugin::<SkyMaterial>::default())
            .add_systems(Startup, spawn_sky)
            .add_systems(Update, update_sky);
    }
}

fn spawn_sky(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    let mesh = meshes.add(uv_sphere(SKY_RADIUS, 24, 16));
    let handle = materials.add(SkyMaterial { uniform: SkyUniform::default() });
    commands.insert_resource(SkyHandle(handle.clone()));
    commands.spawn((MaterialMeshBundle { mesh, material: handle, ..default() }, SkyDome));
}

/// A plain UV sphere. Vertex count barely matters here — the dome is shaded
/// entirely from the fragment shader's view direction, not from baked vertex
/// data — so a coarse sphere is exactly as smooth as a fine one.
fn uv_sphere(radius: f32, sectors: usize, stacks: usize) -> Mesh {
    let mut positions = Vec::with_capacity((sectors + 1) * (stacks + 1));
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut uvs = Vec::with_capacity(positions.capacity());

    for i in 0..=stacks {
        let phi = std::f32::consts::PI * i as f32 / stacks as f32;
        let (sin_phi, cos_phi) = phi.sin_cos();
        for j in 0..=sectors {
            let theta = std::f32::consts::TAU * j as f32 / sectors as f32;
            let (sin_theta, cos_theta) = theta.sin_cos();
            let (x, y, z) = (sin_phi * cos_theta, cos_phi, sin_phi * sin_theta);
            positions.push([x * radius, y * radius, z * radius]);
            normals.push([x, y, z]);
            uvs.push([j as f32 / sectors as f32, i as f32 / stacks as f32]);
        }
    }

    let row = sectors + 1;
    let mut indices = Vec::with_capacity(stacks * sectors * 6);
    for i in 0..stacks {
        for j in 0..sectors {
            let a = (i * row + j) as u32;
            let b = a + row as u32;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Simplified solar position: elevation and azimuth (radians, azimuth
/// clockwise from north) for a given local solar hour. Standard declination
/// / hour-angle formula; see the module doc for what it deliberately skips.
fn solar_position(hour_of_day: f32) -> (f32, f32) {
    let lat = SITE_LAT_DEG.to_radians();
    let decl = 23.44f32.to_radians() * (((360.0 / 365.0) * (DAY_OF_YEAR + 284.0)).to_radians()).sin();
    let h = (hour_of_day - 12.0) * 15f32.to_radians();

    let el = (lat.sin() * decl.sin() + lat.cos() * decl.cos() * h.cos()).clamp(-1.0, 1.0).asin();
    let az_cos = ((decl.sin() - el.sin() * lat.sin()) / (el.cos() * lat.cos())).clamp(-1.0, 1.0);
    let az = if h.rem_euclid(std::f32::consts::TAU) <= std::f32::consts::PI {
        az_cos.acos()
    } else {
        std::f32::consts::TAU - az_cos.acos()
    };
    (el, az)
}

/// Unit vector from the ground toward the sun, in Bevy world space (+x east,
/// +y up, +z south — see `crate::frame`).
fn sun_direction(elevation: f32, azimuth: f32) -> Vec3 {
    let horiz = elevation.cos();
    Vec3::new(azimuth.sin() * horiz, elevation.sin(), -azimuth.cos() * horiz)
}

/// Piecewise-linear interpolation over sorted `(elevation_deg, value)` stops,
/// clamped past the ends. Same idea as `fire_view::ramp`, kept local because
/// the stop tables here are colours *and* scalars.
fn ramp3(t: f32, stops: &[(f32, [f32; 3])]) -> [f32; 3] {
    let t = t.clamp(stops[0].0, stops[stops.len() - 1].0);
    for pair in stops.windows(2) {
        let (t0, c0) = pair[0];
        let (t1, c1) = pair[1];
        if t <= t1 {
            let k = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
            return [
                c0[0] + (c1[0] - c0[0]) * k,
                c0[1] + (c1[1] - c0[1]) * k,
                c0[2] + (c1[2] - c0[2]) * k,
            ];
        }
    }
    stops[stops.len() - 1].1
}

fn ramp1(t: f32, stops: &[(f32, f32)]) -> f32 {
    let t = t.clamp(stops[0].0, stops[stops.len() - 1].0);
    for pair in stops.windows(2) {
        let (t0, v0) = pair[0];
        let (t1, v1) = pair[1];
        if t <= t1 {
            let k = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
            return v0 + (v1 - v0) * k;
        }
    }
    stops[stops.len() - 1].1
}

const ILLUMINANCE: [(f32, f32); 6] =
    [(-90.0, 0.0), (-6.0, 15.0), (0.0, 600.0), (10.0, 4200.0), (30.0, 8200.0), (60.0, 10_500.0)];
const SUN_COLOR: [(f32, [f32; 3]); 6] = [
    (-90.0, [0.60, 0.60, 0.70]),
    (-6.0, [1.00, 0.35, 0.18]),
    (0.0, [1.00, 0.50, 0.25]),
    (10.0, [1.00, 0.75, 0.52]),
    (30.0, [1.00, 0.90, 0.78]),
    (60.0, [1.00, 0.96, 0.90]),
];
const ZENITH: [(f32, [f32; 3]); 6] = [
    (-90.0, [0.02, 0.03, 0.07]),
    (-6.0, [0.05, 0.07, 0.16]),
    (0.0, [0.10, 0.14, 0.28]),
    (10.0, [0.16, 0.28, 0.52]),
    (30.0, [0.20, 0.38, 0.68]),
    (60.0, [0.22, 0.42, 0.75]),
];
const HORIZON: [(f32, [f32; 3]); 6] = [
    (-90.0, [0.03, 0.04, 0.09]),
    (-6.0, [0.30, 0.18, 0.20]),
    (0.0, [0.85, 0.45, 0.28]),
    (10.0, [0.78, 0.62, 0.48]),
    (30.0, [0.68, 0.74, 0.80]),
    (60.0, [0.62, 0.73, 0.84]),
];
const AMBIENT_BRIGHTNESS: [(f32, f32); 6] =
    [(-90.0, 6.0), (-6.0, 20.0), (0.0, 45.0), (10.0, 80.0), (30.0, 130.0), (60.0, 170.0)];
const AMBIENT_COLOR: [(f32, [f32; 3]); 6] = [
    (-90.0, [0.05, 0.07, 0.14]),
    (-6.0, [0.20, 0.16, 0.22]),
    (0.0, [0.45, 0.35, 0.35]),
    (10.0, [0.55, 0.55, 0.55]),
    (30.0, [0.68, 0.74, 0.85]),
    (60.0, [0.72, 0.78, 0.92]),
];

pub fn update_sky(
    sim: Res<Sim>,
    day: Res<DayClock>,
    sky_handle: Option<Res<SkyHandle>>,
    mut sky_materials: ResMut<Assets<SkyMaterial>>,
    mut sun_state: ResMut<SunState>,
    mut ambient: ResMut<AmbientLight>,
    mut clear_color: ResMut<ClearColor>,
    mut sun_q: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    mut fog_q: Query<&mut FogSettings>,
) {
    let hour = day.start_hour + sim.time_s() as f32 / 3600.0;
    let (el, az) = solar_position(hour);
    let el_deg = el.to_degrees();
    let dir = sun_direction(el, az);

    let illuminance = ramp1(el_deg, &ILLUMINANCE);
    let sun_color = ramp3(el_deg, &SUN_COLOR);
    let zenith = ramp3(el_deg, &ZENITH);
    let horizon = ramp3(el_deg, &HORIZON);
    let ambient_brightness = ramp1(el_deg, &AMBIENT_BRIGHTNESS);
    let ambient_color = ramp3(el_deg, &AMBIENT_COLOR);
    let day_gate = el.sin().clamp(0.0, 1.0);
    let brightness = (illuminance / ILLUMINANCE[ILLUMINANCE.len() - 1].1).clamp(0.0, 1.0);

    *sun_state = SunState { direction: dir, color: Vec3::from(sun_color), day_gate, brightness };

    for (mut transform, mut light) in &mut sun_q {
        *transform = Transform::IDENTITY.looking_to(-dir, Vec3::Y);
        light.illuminance = illuminance;
        light.color = Color::srgb(sun_color[0], sun_color[1], sun_color[2]);
    }

    ambient.brightness = ambient_brightness;
    ambient.color = Color::srgb(ambient_color[0], ambient_color[1], ambient_color[2]);
    clear_color.0 = Color::srgb(horizon[0], horizon[1], horizon[2]);

    if let Some(handle) = sky_handle {
        if let Some(mat) = sky_materials.get_mut(&handle.0) {
            mat.uniform = SkyUniform {
                sun_dir: dir.extend(day_gate),
                sun_color: Vec3::from(sun_color).extend(1.0),
                zenith_color: Vec3::from(zenith).extend(1.0),
                horizon_color: Vec3::from(horizon).extend(1.0),
            };
        }
    }

    for mut fog in &mut fog_q {
        let horizon_col = Color::srgb(horizon[0], horizon[1], horizon[2]);
        fog.color = horizon_col;
        fog.directional_light_color = Color::srgba(sun_color[0], sun_color[1], sun_color[2], 0.5);
        fog.directional_light_exponent = 30.0;
        // Generous visibility: the incident area is 10 km across and the
        // point of the haze is depth cues on the far ridgeline, not a wall.
        fog.falloff = FogFalloff::from_visibility_colors(
            11_000.0,
            horizon_col,
            Color::srgb(horizon[0] * 0.92, horizon[1] * 0.94, horizon[2] * 0.98),
        );
    }
}
