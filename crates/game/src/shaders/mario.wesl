// SM64-style Gouraud shade and decal/cutout combiners. No scene PBR or rim.

import bevy_pbr::render::forward_io::{Vertex, VertexOutput};
import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::view_transformations::position_world_to_clip;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var atlas: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var atlas_sampler: sampler;

@vertex
fn vertex(in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let model = mesh_functions::get_world_from_local(in.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(in.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(in.normal, in.instance_index);
    out.uv = in.uv;
    // Original Mario Lights1 groups use half-strength ambient and (40,40,40)
    // directional light. Evaluate at vertices, then interpolate across faces.
    let diffuse = max(dot(normalize(out.world_normal), normalize(vec3<f32>(1.0))), 0.0);
    let shade = min(0.5 + diffuse, 1.0);
    out.color = vec4<f32>(in.color.rgb * shade, 1.0);
    @if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = in.instance_index;
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample uniformly before branching so texture derivatives remain valid.
    let tex = textureSample(atlas, atlas_sampler, in.uv);
    var rgb = in.color.rgb;
    if all(in.uv == vec2<f32>(1.0)) {
        // libsm64's explicit sentinel for untextured body geometry.
        rgb = in.color.rgb;
    } else if in.uv.x >= 9.0 / 11.0 {
        // Final two atlas tiles: DECALRGBA wing feathers. White stays opaque;
        // only ROM alpha removes the spaces around/between the feathers.
        if tex.a < 0.5 { discard; }
        rgb = tex.rgb;
    } else {
        // Facial details / logo / buttons use BLENDRGBFADEA: transparent
        // texels retain shade rather than punching holes through the body.
        rgb = mix(in.color.rgb, tex.rgb, tex.a);
    }
    return vec4<f32>(srgb_to_linear(rgb), 1.0);
}
