// Render only Mario's coverage and reverse-Z depth into an offscreen mask.

import bevy_pbr::render::forward_io::VertexOutput;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var atlas: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var atlas_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(atlas, atlas_sampler, in.uv);
    // Transparent face decals retain the body; Wing Cap gaps stay empty.
    if in.uv.x >= 9.0 / 11.0 && in.uv.x < 1.0 && tex.a < 0.5 { discard; }
    return vec4<f32>(in.position.z, 0.0, 0.0, 1.0);
}
