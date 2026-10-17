//! The forecast's zones painted by a shader: see `shaders/forecast_zone.wgsl`.
//! The geometry is the isochrones of `forecast_view`, closed lines; a grid
//! draped over the island carries each vertex's signed distance to them, and
//! the shader fills what is inside and draws the line.

use bevy::asset::load_internal_asset;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};
use rocca::forecast::HORIZONS;
use scenario::{Pos, Scenario};

const ZONE_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(0x7A21_C0DE_55B3_4E90_8D16_3F4A_B2C7_E581);

/// Spacing of the draped grid, metres.
const GRID_M: f32 = 30.0;
/// Lift above the ground, as the other signs'.
const LIFT_M: f32 = crate::rings::RING_LIFT_M;
/// Opacity of the fill of +1 h and +3 h, and of the contour lines (none: they
/// made too much confusion, user 2026-10-17).
const FILL: [f32; HORIZONS] = [0.34, 0.22];
const LINE: f32 = 0.0;
/// Distances are held within this, metres: far from every line it is all the same.
const REACH_M: f32 = 300.0;
/// The same pull toward the camera as `FlatMaterial`'s.
const PULL: Vec4 = Vec4::new(0.04, 40.0, 220.0, 0.0);

#[derive(Clone, Copy, ShaderType)]
pub struct ZoneUniform {
    near: Vec4,
    far: Vec4,
    line: Vec4,
    pull: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct ZoneMaterial {
    #[uniform(0)]
    pub uniform: ZoneUniform,
}

impl ZoneMaterial {
    pub fn saturation(&self) -> f32 {
        self.uniform.pull.w
    }

    /// Saturation gain of the tactical view (`tactical.rs`): 0 = as given.
    pub fn set_saturation(&mut self, gain: f32) {
        self.uniform.pull.w = gain;
    }

    /// `colours` as sRGB, `+1 h` first.
    pub fn new(colours: [[f32; 3]; HORIZONS]) -> Self {
        let lin = |c: [f32; 3], a: f32| {
            let l = Color::srgb(c[0], c[1], c[2]).to_linear();
            Vec4::new(l.red, l.green, l.blue, a)
        };
        ZoneMaterial {
            uniform: ZoneUniform {
                near: lin(colours[0], FILL[0]),
                far: lin(colours[1], FILL[1]),
                line: Vec4::new(LINE, 0.0, 0.0, 0.0),
                pull: PULL,
            },
        }
    }
}

impl Material for ZoneMaterial {
    fn vertex_shader() -> ShaderRef {
        ZONE_SHADER_HANDLE.into()
    }
    fn fragment_shader() -> ShaderRef {
        ZONE_SHADER_HANDLE.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn depth_bias(&self) -> f32 {
        -2.0
    }
    fn specialize(
        _: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

pub struct ZonePlugin;

impl Plugin for ZonePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, ZONE_SHADER_HANDLE, "shaders/forecast_zone.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<ZoneMaterial> { prepass_enabled: false, shadows_enabled: false, ..default() });
    }
}

/// Signed distance from `p` to the closed `loops` (positive inside, even-odd
/// across the loops), held within `REACH_M`.
fn signed_distance(loops: &[Vec<Pos>], p: Pos) -> f32 {
    let mut inside = false;
    let mut d2 = REACH_M * REACH_M;
    for l in loops {
        let n = l.len();
        for i in 0..n {
            let (a, b) = (l[i], l[(i + 1) % n]);
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                inside = !inside;
            }
            let (ex, ey) = (b.x - a.x, b.y - a.y);
            let t = (((p.x - a.x) * ex + (p.y - a.y) * ey) / (ex * ex + ey * ey).max(1e-6)).clamp(0.0, 1.0);
            let (dx, dy) = (a.x + t * ex - p.x, a.y + t * ey - p.y);
            d2 = d2.min(dx * dx + dy * dy);
        }
    }
    let d = d2.sqrt();
    if inside { d } else { -d }
}

/// A grid over the whole world, draped on the terrain, with the distances to
/// each horizon's lines in the first texture coordinates and the arrival time
/// (minutes after issue) in the second's x.
pub fn grid_mesh(scn: &Scenario, lines: &[Vec<Vec<Pos>>; HORIZONS], eta_s: &[f32]) -> Mesh {
    let (w, h) = (scn.world.width_m, scn.world.height_m);
    let (nx, ny) = ((w / GRID_M).ceil() as usize, (h / GRID_M).ceil() as usize);
    // where a line is near at all
    let boxes: [Option<(Pos, Pos)>; HORIZONS] = std::array::from_fn(|k| {
        let pts = lines[k].iter().flatten();
        let (mut lo, mut hi) = (Pos { x: f32::MAX, y: f32::MAX }, Pos { x: f32::MIN, y: f32::MIN });
        let mut any = false;
        for p in pts {
            any = true;
            lo = Pos { x: lo.x.min(p.x), y: lo.y.min(p.y) };
            hi = Pos { x: hi.x.max(p.x), y: hi.y.max(p.y) };
        }
        any.then_some((lo, hi))
    });
    // the forecast's arrival time at a point, minutes after issue, bilinear
    // between the fire cells' centres; what never burns counts as the last horizon
    let world = scn.world;
    let last = rocca::forecast::HORIZONS_S[HORIZONS - 1] as f32;
    let cell = |r: i64, c: i64| {
        let (r, c) = (r.clamp(0, world.fire_rows as i64 - 1), c.clamp(0, world.fire_cols as i64 - 1));
        eta_s.get(r as usize * world.fire_cols + c as usize).map_or(last, |t| t.min(last))
    };
    let eta_min = |p: Pos| {
        let (fc, fr) = (p.x / world.cellsize - 0.5, (world.height_m - p.y) / world.cellsize - 0.5);
        let (c0, r0) = (fc.floor(), fr.floor());
        let (tc, tr) = (fc - c0, fr - r0);
        let (c0, r0) = (c0 as i64, r0 as i64);
        let top = cell(r0, c0) * (1.0 - tc) + cell(r0, c0 + 1) * tc;
        let bottom = cell(r0 + 1, c0) * (1.0 - tc) + cell(r0 + 1, c0 + 1) * tc;
        (top * (1.0 - tr) + bottom * tr) / 60.0
    };
    let mut etas: Vec<[f32; 2]> = Vec::new();
    let mut positions = Vec::with_capacity((nx + 1) * (ny + 1));
    let mut dists = Vec::with_capacity(positions.capacity());
    for j in 0..=ny {
        for i in 0..=nx {
            let p = Pos { x: (i as f32 * GRID_M).min(w), y: (j as f32 * GRID_M).min(h) };
            etas.push([eta_min(p), 0.0]);
            positions.push([p.x, scn.terrain.height_at(p) + LIFT_M, -p.y]);
            let d: [f32; HORIZONS] = std::array::from_fn(|k| match boxes[k] {
                Some((lo, hi)) if p.x > lo.x - REACH_M && p.x < hi.x + REACH_M && p.y > lo.y - REACH_M && p.y < hi.y + REACH_M => {
                    signed_distance(&lines[k], p)
                }
                _ => -REACH_M,
            });
            dists.push(d);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let mut indices = Vec::with_capacity(nx * ny * 6);
    let at = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
    for j in 0..ny {
        for i in 0..nx {
            let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            // a cell with no line near it is not drawn
            let near = [a, b, c, d].iter().any(|&v| dists[v as usize].iter().any(|&x| x > -REACH_M + 1.0) );
            if near {
                indices.extend([a, c, b, a, d, c]);
            }
        }
    }
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, dists);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_1, etas);
    m.insert_indices(Indices::U32(indices));
    m
}
