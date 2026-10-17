// The forecast's zones (+1 h, +3 h): an island-wide grid draped on the terrain,
// each vertex carrying its signed distance in metres to the zones' contours
// (uv.x: +1 h, uv.y: +3 h; positive inside). Interpolated, the distance gives
// the fill and a thin contour line of constant width on the screen. No light
// and no fog, like the other signs; vertices are pulled toward the camera as
// in flat.wgsl.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}

struct Zone {
    // rgb: colour of +1 h / +3 h; a: fill opacity
    near: vec4<f32>,
    far: vec4<f32>,
    // x: contour line opacity
    line: vec4<f32>,
    pull: vec4<f32>,
};
@group(2) @binding(0) var<uniform> zone: Zone;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) dist: vec2<f32>,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    var w = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0)).xyz;
    let to_cam = view.world_position - w;
    let d = max(length(to_cam), 1.0);
    w += to_cam / d * min(clamp(d * zone.pull.x, zone.pull.y, zone.pull.z), d * 0.5);
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(w);
    out.dist = v.uv;
    return out;
}

// One zone: its fill, and the line along its contour (premultiplied).
fn layer(d: f32, colour: vec4<f32>) -> vec4<f32> {
    let fw = max(fwidth(d), 1e-3);
    let inside = smoothstep(-0.75, 0.75, d / fw);
    let line = (1.0 - smoothstep(0.6, 1.6, abs(d) / fw)) * zone.line.x;
    let a = max(inside * colour.a, line);
    return vec4<f32>(colour.rgb * a, a);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let far = layer(in.dist.y, zone.far);
    let near = layer(in.dist.x, zone.near);
    // +1 h over +3 h, premultiplied
    let c = near + far * (1.0 - near.a);
    if c.a < 0.003 {
        discard;
    }
    // tactical view: more saturated zones (gain in pull.w)
    let rgb = c.rgb / c.a;
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>(clamp(mix(vec3<f32>(luma), rgb, 1.0 + zone.pull.w), vec3<f32>(0.0), vec3<f32>(1.0)), c.a);
}
