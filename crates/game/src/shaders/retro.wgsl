// Animated late-90s training-simulation finish. This is an extension of the
// stock PBR fragment path: vertex colours and StandardMaterial alpha/culling
// remain intact, while the dev pulse is evaluated once per fragment on GPU.

#import bevy_pbr::{
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
    forward_io::{VertexOutput, FragmentOutput},
}
#import bevy_pbr::mesh_view_bindings::globals

struct RetroUniform {
    enabled: f32,
};
@group(2) @binding(100) var<uniform> retro: RetroUniform;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material,
        pbr_input.material.base_color,
    );

    let t = globals.time;
    let p = in.world_position.xyz;
    let phase = p.x * 0.013 + p.z * 0.017;
    let carrier = 0.5 + 0.5 * sin(t * 2.2 + phase);
    let scan = 0.5 + 0.5 * sin(p.y * 0.42 - t * 5.0 + phase * 0.7);
    let grain = hash21(floor(p.xz * 0.08 + t * 0.08));
    let pulse = (0.025 + carrier * 0.04 + scan * 0.012 + grain * 0.008) * retro.enabled;

    // Two complementary edge detectors keep this useful across the very
    // different dev meshes. Fresnel catches the visible silhouette; fwidth of
    // the interpolated normal catches hard polygon breaks on house walls,
    // roofs, tree lobes and the deliberately low-poly agent symbols.
    let facing = abs(dot(normalize(in.world_normal), normalize(pbr_input.V)));
    // Widen the rim enough to survive command-camera distance. The facet
    // term is deliberately aggressive because the low-poly tree and house
    // meshes are the intended 90s training-sim silhouette.
    let silhouette_edge = smoothstep(0.02, 0.42, 1.0 - facing);
    let facet_edge = clamp(length(fwidth(normalize(in.world_normal))) * 12.0, 0.0, 1.0);
    var burn_gate = 1.0;
#ifdef VERTEX_COLORS
    // Vegetation writes charcoal colours after it burns. Use that existing
    // per-vertex state to stop dead foliage from looking electrically alive;
    // bright, actively burning vertices remain fully eligible for the pulse.
    let vertex_luma = dot(in.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    burn_gate = smoothstep(0.075, 0.13, vertex_luma);
#endif
    let edge = max(silhouette_edge, facet_edge) * retro.enabled * burn_gate;
    let edge_pulse = (0.42 + carrier * 0.75 + scan * 0.28 + grain * 0.12) * edge;

    // A small additive lift is much more visible than changing emissive on an
    // unlit StandardMaterial, and it remains a single coherent GPU pass.
    pbr_input.material.base_color = vec4<f32>(
        pbr_input.material.base_color.rgb * (1.0 + pulse),
        pbr_input.material.base_color.a,
    );
    pbr_input.material.emissive = vec4<f32>(
        pbr_input.material.emissive.rgb + vec3<f32>(0.0, pulse * 0.18, pulse * 0.26),
        pbr_input.material.emissive.a,
    );

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    // StandardMaterial's unlit path intentionally ignores emissive. Add the
    // retro signal after that branch so the pulse is visible on the flat dev
    // palette as well as on lit materials, including vegetation cards.
    out.color = vec4<f32>(
        out.color.rgb + vec3<f32>(0.0, edge_pulse * 2.6, edge_pulse * 4.2),
        out.color.a,
    );
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
