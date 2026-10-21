// The zone where a team can be placed: a grid draped on the terrain over the
// reachable roads, each vertex carrying its distance in metres to the nearest
// road. The fragment shader cuts the band at `half` metres with an edge one
// pixel wide, so the zone is one connected surface with no seams at the
// junctions. No light and no fog; vertices are pulled toward the camera as in
// flat.wgsl.

import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_world_to_clip;

struct Reach {
    // rgb: colour; a: opacity
    color: vec4<f32>,
    // x: half width of the band, metres
    band: vec4<f32>,
    pull: vec4<f32>,
};
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> reach: Reach;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) dist: f32,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    var w = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0)).xyz;
    let to_cam = view.world_position - w;
    let d = max(length(to_cam), 1.0);
    w += to_cam / d * min(clamp(d * reach.pull.x, reach.pull.y, reach.pull.z), d * 0.5);
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(w);
    out.dist = v.uv.x;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let fw = max(fwidth(in.dist), 1e-3);
    let inside = 1.0 - smoothstep(-0.75, 0.75, (in.dist - reach.band.x) / fw);
    // a brighter rim along the edge, so the limit of the zone reads
    let rim = 1.0 - smoothstep(0.0, 2.5, abs(in.dist - reach.band.x) / fw);
    let a = reach.color.a * inside + 0.35 * rim * reach.band.y;
    if a < 0.003 {
        discard;
    }
    let luma = dot(reach.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let rgb = clamp(mix(vec3<f32>(luma), reach.color.rgb, 1.0 + reach.pull.w), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, min(a, 1.0));
}
