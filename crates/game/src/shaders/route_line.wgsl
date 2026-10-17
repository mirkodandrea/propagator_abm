// A route drawn by the pixel shader: a sparse grid draped on the terrain along
// the path, each vertex carrying its distance in metres to the path (uv.x) and
// how far along the path its nearest point is (uv.y). The fragment shader
// draws a dark outline, the coloured core and a flow of light running along
// it toward the destination (wall clock: the world is stopped when orders are
// given). No light and no fog; vertices pulled toward the camera as in
// flat.wgsl.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::{view, globals},
    view_transformations::position_world_to_clip,
}

struct Route {
    color: vec4<f32>,
    // x: core half width, y: outline half width (metres), z: flow speed (m/s), w: flow period (m)
    line: vec4<f32>,
    pull: vec4<f32>,
    // x: overall opacity; y: strength of the flow (0 = a plain line)
    look: vec4<f32>,
};
@group(2) @binding(0) var<uniform> route: Route;

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
    var w = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0)).xyz;
    let to_cam = view.world_position - w;
    let d = max(length(to_cam), 1.0);
    w += to_cam / d * min(clamp(d * route.pull.x, route.pull.y, route.pull.z), d * 0.5);
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(w);
    out.uv = v.uv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = in.uv.x;
    let fw = max(fwidth(d), 1e-3);
    let outer = 1.0 - smoothstep(-0.75, 0.75, (d - route.line.y) / fw);
    let core = 1.0 - smoothstep(-0.75, 0.75, (d - route.line.x) / fw);
    // light running along the route: bright stripes sliding with the time
    let phase = (in.uv.y - globals.time * route.line.z) / route.line.w;
    let flow = 0.5 + 0.5 * cos(phase * 6.2831853);
    let amt = route.look.y;
    let lit = route.color.rgb * (1.0 - 0.3 * amt + 0.3 * amt * flow) + vec3<f32>(0.25 * amt * pow(flow, 4.0));
    let under = vec3<f32>(0.02, 0.03, 0.05);
    let rgb = mix(under, clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)), core);
    let a = max(outer * 0.85, core) * route.look.x;
    if a < 0.003 {
        discard;
    }
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>(clamp(mix(vec3<f32>(luma), rgb, 1.0 + route.pull.w), vec3<f32>(0.0), vec3<f32>(1.0)), a);
}
