// Restrained late-90s training-simulation finish. This is an extension of the
// stock PBR fragment path: vertex colours and StandardMaterial alpha/culling
// remain intact, while each material decides how much rim and motion it gets.

#import bevy_pbr::{
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
    forward_io::{VertexOutput, FragmentOutput},
}
#import bevy_pbr::mesh_view_bindings::globals

struct RetroUniform {
    enabled: vec4<f32>,
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
    let effect = retro.enabled.x;
    let edge_strength = retro.enabled.y * effect;
    let pulse_strength = retro.enabled.z * effect;
    let pulse = (0.012 + carrier * 0.018 + scan * 0.006 + grain * 0.003) * pulse_strength;

    // Two complementary edge detectors keep this useful across the very
    // different dev meshes. Fresnel catches the visible silhouette; fwidth of
    // the interpolated normal catches hard polygon breaks on house walls,
    // roofs, tree lobes and the deliberately low-poly agent symbols.
    let facing = abs(dot(normalize(in.world_normal), normalize(pbr_input.V)));
    // Widen the rim enough to survive command-camera distance, but let the
    // material suppress it entirely for dense background geometry.
    let silhouette_edge = smoothstep(0.02, 0.42, 1.0 - facing);
    let facet_edge = clamp(length(fwidth(normalize(in.world_normal))) * 8.0, 0.0, 1.0);
    var burn_gate = 1.0;
#ifdef VERTEX_COLORS
    // Vegetation writes charcoal colours after it burns. Use that existing
    // per-vertex state to stop dead foliage from looking electrically alive;
    // bright, actively burning vertices remain fully eligible for the pulse.
    let vertex_luma = dot(in.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    burn_gate = smoothstep(0.075, 0.13, vertex_luma);
#endif
    let edge = max(silhouette_edge, facet_edge) * edge_strength * burn_gate;
    let edge_pulse = (0.30 + carrier * 0.18 + scan * 0.08 + grain * 0.03) * edge;

    // Vegetation only (`enabled.w` = 2): breaks the flat vertex colour into
    // leaf clumps and a fine speckle, plus a warm sunlit / cool shaded lean.
    // Two value-noise taps, faded out once a pixel spans more than ~2 m, so
    // the far forest costs next to nothing and never shimmers.
    if retro.enabled.w > 1.5 {
        let px_m = max(length(dpdx(p)), length(dpdy(p)));
        let fine = 1.0 - smoothstep(1.0, 3.5, px_m);
        let mid = 1.0 - smoothstep(2.0, 7.0, px_m);
        // Per-patch tint on a ~3.5 m lattice: about one macchia shrub each, so
        // neighbouring bushes differ in tone; it outlives the fine detail.
        let patch_tone = hash21(floor(p.xz / 3.5)) - 0.5;
        let clump = value_noise(p.xz * 0.9 + vec2<f32>(p.y * 0.6, p.y * 0.35));
        // Posterised clumps: distinct light and dark leaf masses with soft
        // steps between them, rather than a smooth gradient.
        let steps = floor(clump * 3.0) / 3.0 + smoothstep(0.0, 0.18, fract(clump * 3.0)) / 3.0;
        var tone = patch_tone * 0.45 * mid + (mix(clump, steps, 0.7) - 0.5) * 0.75 * mid;
        if fine > 0.0 {
            let speck = value_noise(p.xz * 3.2 + vec2<f32>(p.y * 3.3, -p.y * 2.1));
            // Sharp leaf dots: bright tips, dark gaps between them.
            let leaves = smoothstep(0.38, 0.62, speck);
            tone += (leaves - 0.5) * 0.60 * fine - (1.0 - leaves) * 0.10 * fine;
        }
        let up = clamp(normalize(in.world_normal).y * 0.5 + 0.5, 0.0, 1.0);
        let warm = vec3<f32>(1.14, 1.06, 0.76);
        let cool = vec3<f32>(0.80, 0.96, 1.08);
        let lean = mix(cool, warm, up * (0.5 + clump * 0.5));
        // Which vertices are living foliage is decided where the mesh is
        // built: vertex alpha is 1 on leaves, 0 on wood, rock, grass and
        // anything burning or burnt (`vegetation.rs`).
        let c = pbr_input.material.base_color.rgb;
#ifdef VERTEX_COLORS
        let alive = in.color.a;
#else
        let alive = 0.0;
#endif
        let k = mid * alive;
        let rgb = c * (1.0 + tone * alive) * mix(vec3<f32>(1.0), lean, k);
        pbr_input.material.base_color = vec4<f32>(rgb, pbr_input.material.base_color.a);
    }

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
    // restrained rim after that branch so structural edges remain visible.
    out.color = vec4<f32>(
        out.color.rgb + vec3<f32>(0.0, edge_pulse * 0.65, edge_pulse),
        out.color.a,
    );
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
