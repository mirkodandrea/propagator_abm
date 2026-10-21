// Signs on the map (hexes, routes, the forecast): one flat colour, no light,
// no fog. Each vertex is pulled toward the camera along its view ray: on
// screen it stays where it is, but it wins the depth test against the canopy
// (and anything else a few tens of metres tall), while a hill in front still
// hides it.

import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_world_to_clip;

struct Flat {
    color: vec4<f32>,
    // x: pull as a fraction of the distance, y: at least, z: at most (metres),
    // w: saturation gain (tactical view), 0 = the colour as given
    pull: vec4<f32>,
};
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> flat: Flat;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    var w = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0)).xyz;
    let to_cam = view.world_position - w;
    let d = max(length(to_cam), 1.0);
    w += to_cam / d * min(clamp(d * flat.pull.x, flat.pull.y, flat.pull.z), d * 0.5);
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(w);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let luma = dot(flat.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let rgb = clamp(mix(vec3<f32>(luma), flat.color.rgb, 1.0 + flat.pull.w), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, flat.color.a);
}
