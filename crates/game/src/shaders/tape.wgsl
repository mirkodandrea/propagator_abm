// Closed roads: hazard tape. Diagonal red/white stripes crawl along the road
// (u = metres along, v = 0..1 across), dark edge lines, a slow pulse. Same
// lies on the road, depth-tested like the ground.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::globals,
    view_transformations::position_world_to_clip,
}

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    let w = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0)).xyz;
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(w);
    out.uv = v.uv;
    return out;
}

@group(2) @binding(0) var<uniform> mode: vec4<f32>;

const WIDTH_M: f32 = 9.6;
const PERIOD_M: f32 = 16.0;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = globals.time;
    if mode.x > 0.5 {
        // the part of the stretch still open to vehicles: a thin dashed line
        if abs(in.uv.y - 0.5) > 0.12 || fract(in.uv.x / 14.0) > 0.55 {
            discard;
        }
        return vec4<f32>(0.55, 0.02, 0.01, 1.0);
    }
    let across = (in.uv.y - 0.5) * WIDTH_M;
    // diagonal bands, drifting along the road
    let band = fract((in.uv.x + across) / PERIOD_M - t * 0.35);
    let red = band < 0.5;
    var col = select(vec3<f32>(1.0, 0.80, 0.70), vec3<f32>(1.0, 0.012, 0.006), red);
    // dark edge lines, so the tape reads against any ground
    let edge = smoothstep(0.38, 0.5, abs(in.uv.y - 0.5));
    col = mix(col, vec3<f32>(0.03, 0.0, 0.0), edge);
    // slow pulse, rolling along the road
    col *= 0.88 + 0.12 * sin(t * 2.4 - in.uv.x * 0.02);
    return vec4<f32>(col, 1.0);
}
