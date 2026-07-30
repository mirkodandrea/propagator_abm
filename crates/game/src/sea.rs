//! The sea: a separate animated mesh over the water, rather than the flat
//! vertex-coloured patch [`crate::terrain_mesh`] paints under the waterline.
//!
//! The render DEM is clamped at 0 m (`data/spotorno_render_terrain.json`:
//! `elev_min: 0.0`) — there is no real bathymetry in this dataset, only a
//! flat sea floor. So "depth" here is not measured depth: it is distance to
//! the nearest dry ground, probed outward from each water vertex at mesh-build
//! time ([`distance_to_shore`]). That is what actually varies near a real
//! coastline — a Ligurian shelf is narrow — and gives the shallow/deep colour
//! split and the shoreline foam something honest to key off, without
//! pretending to know a seabed this data does not have.
//!
//! Waves are a vertex-shader displacement — see `shaders/water.wgsl` — driven
//! by the same [`fire::Weather`] wind the smoke and embers use, so a
//! wind shift from the Wildfire panel chops the sea up exactly when it
//! shifts the fire.

use bevy::asset::load_internal_asset;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderRef, ShaderType, SpecializedMeshPipelineError,
};
use scenario::{Pos, Scenario};

use crate::sim::Sim;
use crate::sky::SunState;

const WATER_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x2C6E_88AA_401D_47CC_8F19_55DD_66EE_77FF);

/// Lattice spacing for the water mesh: coarser than the 5 m terrain posting,
/// since the wave shapes it carries (tens of metres) do not need it and the
/// fine chop is a fragment-shader noise wobble, not geometry (see the wgsl).
const WATER_STRIDE: usize = 4;
const WATER_CHUNK: usize = 64;

/// Must agree with `terrain_mesh::ground_color`'s own sea threshold: this is
/// the elevation below which the ground layer paints "sea", so the water
/// mesh has to cover exactly that footprint or a ring of one or the other
/// colour would show at the coast.
const WATER_ELEV_MAX: f32 = 0.6;

/// Flat base height for the (unwaved) lattice; the vertex shader adds the
/// swell on top of this every frame. Has to clear both `WATER_ELEV_MAX`
/// (ground under the water can sit up to 0.6 m) *and* the deepest wave
/// trough the shader can produce (see the amplitude clamp in
/// `shaders/water.wgsl`) — undershoot either and the terrain pokes through
/// the animated surface, which reads as a patch of dark, static "shadow"
/// sitting in the water. Shipped once at 0.35 m, which cleared neither.
const WATER_BASE_Y: f32 = 2.0;

/// How far past the real window edge the sea keeps its full [`WATER_STRIDE`]
/// resolution.
///
/// Past this the water continues at [`OPEN_SEA_STRIDE_M`] instead of
/// stopping: the real window's edge is a straight line, and a straight
/// world-space line viewed obliquely is a straight line on screen no matter
/// the angle, so *any* boundary where the water hands off to something else
/// shows up as a stark, dead-straight line across the horizon. It shipped
/// twice: first at the window edge itself, then here at 3 km, where the flat
/// "sea colour" ground of `crate::far_terrain` took over — a different
/// colour, differently lit, and not fogged by the same amount at the same
/// distance, so the join read as a solid slab of blue laid over the horizon.
/// Two surfaces pretending to be one sea cannot be made to agree; one sea can.
const SEA_EXTEND_M: f32 = 3000.0;

/// Target lattice spacing for the open sea past [`SEA_EXTEND_M`]. It carries
/// the same waves and the same shader — it is the same sea — but nothing is
/// ever closer than 3 km to it, and the swell it drops at this spacing is
/// half a metre of vertical displacement seen from kilometres away.
///
/// Matched to `far_terrain::SKIRT_STRIDE_M` rather than made as coarse as it
/// could get away with: past the window the coastline is the far terrain's
/// own extruded edge, and where the two disagree about their sampling the
/// waterline turns into a staircase of one lattice cut against the other.
const OPEN_SEA_STRIDE_M: f32 = 150.0;

/// The open sea is trimmed to a disc of this radius about the window's
/// centre, not to the square its lattice is built on. A horizon that is the
/// same distance away in every direction cannot resolve into a straight
/// ruled line the way a box's near side does, and what fog has not quite
/// finished erasing at this range is exactly a straight line's worth of
/// residual blue.
const OPEN_SEA_RADIUS_M: f32 = crate::far_terrain::SKIRT_EXTENT_M;

/// One rectangular water lattice: origin, cell counts and (possibly
/// non-round) cell size. The open-sea bands derive their spacing from their
/// own span rather than taking [`OPEN_SEA_STRIDE_M`] literally, so each band
/// ends *exactly* on the inner mesh's boundary — a lattice that merely came
/// close would leave either a strip of missing sea or a strip of two water
/// surfaces z-fighting all along the join.
struct Patch {
    x0: f32,
    y0: f32,
    cols: usize,
    rows: usize,
    sx: f32,
    sy: f32,
    /// Whether vertices carry a real [`distance_to_shore`]. Only the near
    /// patch does: the probe is 48 terrain samples a vertex, and what it
    /// buys — the shallow-water colour and the shoreline foam — is a
    /// metres-wide band nothing ever sees from 3 km out.
    probe_shore: bool,
}

impl Patch {
    /// A patch spanning `[x0, x1] x [y0, y1]` at close to `target` spacing.
    fn spanning(x0: f32, x1: f32, y0: f32, y1: f32, target: f32, probe_shore: bool) -> Patch {
        let nx = ((x1 - x0) / target).ceil().max(1.0);
        let ny = ((y1 - y0) / target).ceil().max(1.0);
        Patch {
            x0,
            y0,
            cols: nx as usize + 1,
            rows: ny as usize + 1,
            sx: (x1 - x0) / nx,
            sy: (y1 - y0) / ny,
            probe_shore,
        }
    }
}

/// Elevation for a point that may be outside the real terrain grid: the real
/// bilinear field inside the window, `far_terrain`'s continuation beyond it
/// — so the "is this wet" test agrees with the ground the extension actually
/// sits on instead of just clamping to the window edge's own elevation.
fn elev_at(scn: &Scenario, p: Pos) -> f32 {
    if scn.world.contains(p) {
        scn.terrain.height_at(p)
    } else {
        crate::far_terrain::far_height(scn, p)
    }
}

/// Whether a lattice vertex (in Bevy space, as pushed into the mesh) is
/// within [`OPEN_SEA_RADIUS_M`] of the window's centre. Cells outside are
/// dropped, which rounds the sea's outer boundary off into a disc.
fn inside_horizon(scn: &Scenario, p: [f32; 3]) -> bool {
    let dx = p[0] - scn.terrain.width_m * 0.5;
    let dz = -p[2] - scn.terrain.height_m * 0.5;
    dx * dx + dz * dz <= OPEN_SEA_RADIUS_M * OPEN_SEA_RADIUS_M
}

/// Probe ring radii for [`distance_to_shore`], metres. Past the last ring, a
/// water point is "open water" for shading purposes — the shallow/deep split
/// only needs to resolve the first hundred-odd metres off a beach.
const SHORE_PROBE_RADII: [f32; 6] = [10.0, 20.0, 40.0, 70.0, 110.0, 160.0];
const SHORE_PROBE_DIRS: usize = 8;

#[derive(Component)]
struct SeaMesh;

#[derive(Resource)]
struct WaterHandle(Handle<WaterMaterial>);

#[derive(Clone, Copy, Default, ShaderType)]
pub struct WaterUniform {
    pub wind: Vec4,
    pub sun_dir: Vec4,
    pub sun_color: Vec4,
    pub deep_color: Vec4,
    pub shallow_color: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub uniform: WaterUniform,
}

impl Material for WaterMaterial {
    fn vertex_shader() -> ShaderRef {
        WATER_SHADER_HANDLE.into()
    }

    fn fragment_shader() -> ShaderRef {
        WATER_SHADER_HANDLE.into()
    }

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

pub struct SeaPlugin;

impl Plugin for SeaPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, WATER_SHADER_HANDLE, "shaders/water.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default())
            .add_systems(Startup, spawn_sea)
            .add_systems(Update, update_water);
    }
}

/// Straight-line distance to the nearest ground the terrain colours as dry
/// (see the module doc): a cheap stand-in for bathymetric depth, probed
/// outward in rings rather than solved as a real distance transform, because
/// the coastline here is short enough that it does not need one.
fn distance_to_shore(scn: &Scenario, p: Pos) -> f32 {
    for &r in &SHORE_PROBE_RADII {
        for k in 0..SHORE_PROBE_DIRS {
            let a = k as f32 / SHORE_PROBE_DIRS as f32 * std::f32::consts::TAU;
            let probe = Pos { x: p.x + r * a.cos(), y: p.y + r * a.sin() };
            // `elev_at`, not a bare `world.contains` check: a point already
            // out in the `SEA_EXTEND_M` extension is by definition outside
            // the real window, so the old `!contains(probe)` test tripped on
            // the very first, smallest probe ring in every direction and
            // reported the whole extension as if it were lapping the shore.
            if elev_at(scn, probe) > WATER_ELEV_MAX {
                return r;
            }
        }
    }
    SHORE_PROBE_RADII[SHORE_PROBE_RADII.len() - 1]
}

fn spawn_sea(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let scn = &sim.scenario;
    let t = &scn.terrain;
    let stride_m = t.posting * WATER_STRIDE as f32;

    let handle = materials.add(WaterMaterial {
        uniform: WaterUniform {
            wind: Vec4::ZERO,
            sun_dir: Vec4::new(0.0, 1.0, 0.0, 0.0),
            sun_color: Vec4::ONE,
            deep_color: Vec4::new(0.02, 0.09, 0.20, 1.0),
            shallow_color: Vec4::new(0.09, 0.42, 0.40, 1.0),
        },
    });
    commands.insert_resource(WaterHandle(handle.clone()));

    // The near sea at full resolution, then four bands of open sea around it
    // reaching the same distance the far terrain does. The east and west
    // bands run the full height, so the corners belong to them and no band
    // has to meet another one diagonally.
    let (inner_x0, inner_x1) = (-SEA_EXTEND_M, t.width_m + SEA_EXTEND_M);
    let (inner_y0, inner_y1) = (-SEA_EXTEND_M, t.height_m + SEA_EXTEND_M);
    let out = crate::far_terrain::SKIRT_EXTENT_M;
    let (outer_x0, outer_x1) = (-out, t.width_m + out);
    let (outer_y0, outer_y1) = (-out, t.height_m + out);
    let s = OPEN_SEA_STRIDE_M;
    let patches = [
        Patch::spanning(inner_x0, inner_x1, inner_y0, inner_y1, stride_m, true),
        Patch::spanning(outer_x0, inner_x0, outer_y0, outer_y1, s, false),
        Patch::spanning(inner_x1, outer_x1, outer_y0, outer_y1, s, false),
        Patch::spanning(inner_x0, inner_x1, outer_y0, inner_y0, s, false),
        Patch::spanning(inner_x0, inner_x1, inner_y1, outer_y1, s, false),
    ];

    let (mut count, mut tri_count) = (0usize, 0usize);
    for patch in &patches {
        let (c, t) = spawn_patch(patch, scn, &mut commands, &mut meshes, &handle);
        count += c;
        tri_count += t;
    }

    info!("sea: {count} chunks, {} k triangles @ {stride_m} m lattice", tri_count / 1000);
}

/// Mesh one lattice, chunked so the water still culls in pieces.
fn spawn_patch(
    patch: &Patch,
    scn: &Scenario,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    handle: &Handle<WaterMaterial>,
) -> (usize, usize) {
    let (coarse_cols, coarse_rows) = (patch.cols, patch.rows);
    let chunks_x = (coarse_cols - 1).div_ceil(WATER_CHUNK);
    let chunks_y = (coarse_rows - 1).div_ceil(WATER_CHUNK);
    let (mut count, mut tri_count) = (0usize, 0usize);

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let c0 = cx * WATER_CHUNK;
            let r0 = cy * WATER_CHUNK;
            let cn = (WATER_CHUNK + 1).min(coarse_cols - c0);
            let rn = (WATER_CHUNK + 1).min(coarse_rows - r0);
            if cn < 2 || rn < 2 {
                continue;
            }

            let mut positions = Vec::with_capacity(cn * rn);
            let mut normals = Vec::with_capacity(cn * rn);
            let mut colors = Vec::with_capacity(cn * rn);
            let mut uvs = Vec::with_capacity(cn * rn);
            let mut elev = Vec::with_capacity(cn * rn);

            for r in 0..rn {
                for c in 0..cn {
                    let gx = patch.x0 + (c0 + c) as f32 * patch.sx;
                    let gy = patch.y0 + (r0 + r) as f32 * patch.sy;
                    let e = elev_at(scn, Pos { x: gx, y: gy });
                    elev.push(e);

                    positions.push([gx, WATER_BASE_Y, -gy]);
                    normals.push([0.0, 1.0, 0.0]);
                    uvs.push([gx / 200.0, gy / 200.0]);
                    let depth = if patch.probe_shore {
                        distance_to_shore(scn, Pos { x: gx, y: gy })
                    } else {
                        SHORE_PROBE_RADII[SHORE_PROBE_RADII.len() - 1]
                    };
                    colors.push([depth, depth, depth, 1.0]);
                }
            }

            let mut indices = Vec::new();
            for r in 0..rn - 1 {
                for c in 0..cn - 1 {
                    let i00 = r * cn + c;
                    let i10 = i00 + 1;
                    let i01 = i00 + cn;
                    let i11 = i01 + 1;
                    let wet = [i00, i10, i01, i11].iter().all(|&i| elev[i] <= WATER_ELEV_MAX);
                    if !wet || !inside_horizon(scn, positions[i00]) {
                        continue;
                    }
                    let (a, b, cc, d) = (i00 as u32, i10 as u32, i01 as u32, i11 as u32);
                    indices.extend_from_slice(&[a, cc, b, b, cc, d]);
                    tri_count += 2;
                }
            }
            if indices.is_empty() {
                continue;
            }

            let mut mesh =
                Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
            mesh.insert_indices(Indices::U32(indices));

            commands.spawn((
                MaterialMeshBundle { mesh: meshes.add(mesh), material: handle.clone(), ..default() },
                SeaMesh,
                // The water shader computes its own lighting from scratch
                // (see `shaders/water.wgsl`) and never samples the scene's
                // shadow maps, so casting/receiving would only cost a draw
                // and risk an odd wavy shadow on the coastline for nothing.
                NotShadowCaster,
                NotShadowReceiver,
            ));
            count += 1;
        }
    }

    (count, tri_count)
}

fn update_water(
    sim: Res<Sim>,
    sun: Res<SunState>,
    handle: Option<Res<WaterHandle>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let Some(handle) = handle else { return };
    let Some(mat) = materials.get_mut(&handle.0) else { return };

    let weather = sim.fire.weather();
    let from = (weather.wind_dir_deg as f32).to_radians();
    let wind_ms = weather.wind_speed_kmh as f32 / 3.6;
    // Same "blows FROM" convention as the smoke and embers (`fire_view`):
    // drift is the direction the wind carries things, the opposite bearing.
    let drift = Vec3::new(-from.sin(), 0.0, from.cos());

    mat.uniform.wind = Vec4::new(drift.x, drift.z, wind_ms, 0.0);
    mat.uniform.sun_dir = sun.direction.extend(sun.day_gate);
    // `.a` rides along as the water's own brightness gate (see
    // `shaders/water.wgsl`) — `sun.brightness`, not `sun.day_gate`, so the
    // sea tracks the same illuminance curve the rest of the scene is lit
    // from instead of dimming early on a perfectly sunny afternoon.
    mat.uniform.sun_color = sun.color.extend(sun.brightness);
}
