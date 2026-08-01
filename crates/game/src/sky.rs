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
use bevy::pbr::{
    FogFalloff, FogSettings, Material, MaterialPipeline, MaterialPipelineKey, NotShadowCaster,
    NotShadowReceiver,
};
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
    /// The dock's time-of-day slider, when it has been unlinked from the
    /// simulated clock: `Some(hour)` pins the sun (and moon) to that hour
    /// regardless of `Sim::time_s`, for lighting a screenshot or scrubbing
    /// through a day without running the incident. `None` is the default —
    /// linked — and is what makes `hour()` reduce to `start_hour +
    /// elapsed`, exactly the behaviour before this field existed.
    pub manual_hour: Option<f32>,
}

impl DayClock {
    /// Hour of day `update_sky` should light the scene for: the manual
    /// override if the slider is unlinked, otherwise the simulated clock.
    pub fn hour(&self, sim_time_s: f32) -> f32 {
        self.manual_hour
            .unwrap_or(self.start_hour + sim_time_s / 3600.0)
    }

    pub fn linked(&self) -> bool {
        self.manual_hour.is_none()
    }
}

impl Default for DayClock {
    fn default() -> Self {
        let start_hour = std::env::var("SPOTORNO_START_HOUR")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(16.5);
        DayClock {
            start_hour: start_hour.rem_euclid(24.0),
            manual_hour: None,
        }
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
        SunState {
            direction: Vec3::Y,
            color: Vec3::ONE,
            day_gate: 0.0,
            brightness: 0.0,
        }
    }
}

/// Marks the single `DirectionalLight` entity [`update_sky`] drives. Spawned
/// in `main::setup_scene` with no particular transform or illuminance — both
/// are overwritten the first time `update_sky` runs.
#[derive(Component)]
pub struct Sun;

#[derive(Component)]
pub struct SkyDome;

#[derive(Resource)]
pub(crate) struct SkyHandle(Handle<SkyMaterial>);

#[derive(Clone, Copy, Default, ShaderType)]
pub struct SkyUniform {
    pub sun_dir: Vec4,
    pub sun_color: Vec4,
    pub zenith_color: Vec4,
    pub horizon_color: Vec4,
    pub moon_dir: Vec4,
    pub moon_color: Vec4,
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
        load_internal_asset!(
            app,
            SKY_SHADER_HANDLE,
            "shaders/sky.wgsl",
            Shader::from_wgsl
        );
        app.init_resource::<DayClock>()
            .init_resource::<SunState>()
            .add_plugins(MaterialPlugin::<SkyMaterial>::default())
            .add_systems(OnEnter(crate::AppState::Playing), spawn_sky)
            .add_systems(
                Update,
                update_sky.run_if(bevy::prelude::in_state(crate::AppState::Playing)),
            );
    }
}

fn spawn_sky(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    let mesh = meshes.add(uv_sphere(SKY_RADIUS, 24, 16));
    let handle = materials.add(SkyMaterial {
        uniform: SkyUniform::default(),
    });
    commands.insert_resource(SkyHandle(handle.clone()));
    commands.spawn((
        MaterialMeshBundle {
            mesh,
            material: handle,
            ..default()
        },
        SkyDome,
        // A 30 km sphere left as a default shadow caster/receiver blows up
        // the directional light's cascade-fitting: Bevy sizes each cascade's
        // shadow-map depth range to cover every caster inside that cascade's
        // view slice, and the dome's radius dwarfs everything else in the
        // scene by two to three orders of magnitude. The near cascade's
        // depth range collapses trying to span metres to tens of
        // kilometres in one map, and the failure reads as a solid,
        // roughly-view-aligned shadow that tracks the camera rather than
        // the terrain — because the dome itself is always centred on
        // wherever the camera happens to be standing.
        NotShadowCaster,
        NotShadowReceiver,
    ));
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

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
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
    let decl =
        23.44f32.to_radians() * (((360.0 / 365.0) * (DAY_OF_YEAR + 284.0)).to_radians()).sin();
    let h = (hour_of_day - 12.0) * 15f32.to_radians();

    let el = (lat.sin() * decl.sin() + lat.cos() * decl.cos() * h.cos())
        .clamp(-1.0, 1.0)
        .asin();
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
    Vec3::new(
        azimuth.sin() * horiz,
        elevation.sin(),
        -azimuth.cos() * horiz,
    )
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

const ILLUMINANCE: [(f32, f32); 6] = [
    (-90.0, 0.0),
    (-6.0, 15.0),
    (0.0, 600.0),
    (10.0, 4200.0),
    (30.0, 8200.0),
    (60.0, 10_500.0),
];
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
/// Open shade under a clear sky runs at roughly a sixth to an eighth of
/// direct sun, not the couple of orders of magnitude a hand-tuned, much
/// smaller ambient constant (this shipped at a flat 130 against a 9 500 lux
/// sun — a 73:1 ratio) implies. That mismatch is what a self-shadowed
/// hillside close to the camera exposed: correct terrain self-shadowing,
/// crushed to flat black because the ambient fill light was never scaled to
/// the direct sun it now has to counter across a full day. Tied to
/// `illuminance` rather than its own elevation ramp so the two can never
/// drift back out of proportion.
const AMBIENT_FILL_RATIO: f32 = 6.0;
const AMBIENT_COLOR: [(f32, [f32; 3]); 6] = [
    (-90.0, [0.05, 0.07, 0.14]),
    (-6.0, [0.20, 0.16, 0.22]),
    (0.0, [0.45, 0.35, 0.35]),
    (10.0, [0.55, 0.55, 0.55]),
    (30.0, [0.68, 0.74, 0.85]),
    (60.0, [0.72, 0.78, 0.92]),
];

/// The moon is always full: this scenario is never open long enough (1-3 h)
/// for a phase to matter, and a full moon is the one phase that is actually
/// opposite the sun in the sky — which is also what makes `-dir` the right
/// direction for it, no separate ephemeris needed. `moon_direction` and
/// `moon_direction_elevation` below are exactly that mirroring, spelled out
/// so the "why -dir" isn't left implicit at the call site.
///
/// Colour warms near the horizon the same way the sun's does (Rayleigh
/// scattering through more atmosphere), fading to a pale cool white
/// overhead — a full moon has none of the sun's saturated orange, since it is
/// reflected light with none of the sun's own colour temperature swing.
const MOON_COLOR: [(f32, [f32; 3]); 3] = [
    (0.0, [0.80, 0.62, 0.48]),
    (20.0, [0.85, 0.87, 0.93]),
    (90.0, [0.82, 0.87, 1.00]),
];

/// How much a full moon actually brightens a clear night — a few tens of lux
/// at most, nothing like the sun's thousands, but well above zero: a full
/// moon overhead casts shadows a new moon night does not. Zero below the
/// horizon by construction (the first two stops), so this needs no separate
/// gate.
const MOON_ILLUMINANCE: [(f32, f32); 4] = [(-90.0, 0.0), (0.0, 0.0), (30.0, 40.0), (90.0, 60.0)];

/// The ambient tint a risen full moon adds, blended into `AMBIENT_COLOR`'s
/// night entries in proportion to how high the moon is — cooler and a touch
/// brighter than the flat navy floor, because that floor was tuned as "no
/// light source at all" and a full moon is very much a light source.
const MOON_AMBIENT_TINT: [(f32, [f32; 3]); 3] = [
    (0.0, [0.10, 0.13, 0.22]),
    (30.0, [0.16, 0.20, 0.34]),
    (90.0, [0.20, 0.25, 0.40]),
];

/// The VR-training look has no sun and no procedural sky: a flat void colour,
/// a matching flat fog that fades to it quickly (the "training room" has no
/// horizon), and the dome hidden so `ClearColor` shows through unobstructed.
fn apply_vr_sky(
    pal: scenario::VrPalette,
    mut ambient: ResMut<AmbientLight>,
    mut clear_color: ResMut<ClearColor>,
    mut sun_q: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    mut fog_q: Query<&mut FogSettings>,
    mut dome_q: Query<&mut Visibility, With<SkyDome>>,
) {
    let void = Color::srgb(pal.void[0], pal.void[1], pal.void[2]);
    clear_color.0 = void;
    // Flat, bright and colourless: geometry is unlit in this mode anyway, so
    // this only matters for anything that still reads lighting (e.g. shadow
    // receivers left on by default).
    ambient.brightness = 400.0;
    ambient.color = Color::WHITE;
    for (_, mut light) in &mut sun_q {
        light.illuminance = 0.0;
    }
    for mut fog in &mut fog_q {
        fog.color = void;
        fog.directional_light_color = Color::NONE;
        // The camera starts 2 600 m out from its focus (`main::setup_scene`)
        // and a dev scenario's world is up to 5 120 m across, so a "fades
        // quickly" distance still has to clear both or the fog swallows the
        // entire map before it reaches the lens -- which reads as nothing
        // rendering at all, not as a training room with no horizon.
        fog.falloff = FogFalloff::from_visibility_colors(7000.0, void, void);
    }
    for mut vis in &mut dome_q {
        *vis = Visibility::Hidden;
    }
}

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
    mut dome_q: Query<&mut Visibility, With<SkyDome>>,
) {
    if let Some(pal) = sim.scenario.vr_palette() {
        apply_vr_sky(pal, ambient, clear_color, sun_q, fog_q, dome_q);
        return;
    }

    let hour = day.hour(sim.time_s() as f32);
    let (el, az) = solar_position(hour);
    let el_deg = el.to_degrees();
    let dir = sun_direction(el, az);

    let illuminance = ramp1(el_deg, &ILLUMINANCE);
    let sun_color = ramp3(el_deg, &SUN_COLOR);
    let zenith = ramp3(el_deg, &ZENITH);
    let horizon = ramp3(el_deg, &HORIZON);
    let day_gate = el.sin().clamp(0.0, 1.0);
    let brightness = (illuminance / ILLUMINANCE[ILLUMINANCE.len() - 1].1).clamp(0.0, 1.0);

    // The moon sits exactly opposite the sun (see `MOON_COLOR`'s doc), so its
    // elevation is the sun's negated and its direction is simply `-dir`.
    let moon_el_deg = -el_deg;
    let moon_gate = (-el).sin().clamp(0.0, 1.0);
    let moon_dir = -dir;
    let moon_color = ramp3(moon_el_deg, &MOON_COLOR);
    let moon_illuminance = ramp1(moon_el_deg, &MOON_ILLUMINANCE);

    // A floor so night keeps a faint moonlight-like fill rather than going to
    // literal zero, same spirit as the sun's own illuminance floor — plus the
    // *actual* moon's contribution on top, so a full moon overhead reads as
    // brighter than one that has just set.
    let ambient_brightness = (illuminance / AMBIENT_FILL_RATIO).max(4.0) + moon_illuminance;
    let base_ambient_color = ramp3(el_deg, &AMBIENT_COLOR);
    let moon_tint = ramp3(moon_el_deg, &MOON_AMBIENT_TINT);
    let ambient_color = [
        base_ambient_color[0] + (moon_tint[0] - base_ambient_color[0]) * moon_gate,
        base_ambient_color[1] + (moon_tint[1] - base_ambient_color[1]) * moon_gate,
        base_ambient_color[2] + (moon_tint[2] - base_ambient_color[2]) * moon_gate,
    ];

    *sun_state = SunState {
        direction: dir,
        color: Vec3::from(sun_color),
        day_gate,
        brightness,
    };

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
                moon_dir: moon_dir.extend(moon_gate),
                moon_color: Vec3::from(moon_color).extend(1.0),
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

    for mut vis in &mut dome_q {
        *vis = Visibility::Inherited;
    }
}
