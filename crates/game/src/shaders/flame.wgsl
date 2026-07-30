// Fragment shader for flame billboards.
//
// The CPU side (fire_view::update_flames) already decides *where* a tongue
// stands, how tall it is and how hot: this shader is only responsible for
// making one quad look like fire instead of a lit sprite. It domain-warps
// the texture lookup with a small animated noise field and adds a fast
// per-instance flicker, both driven by `globals.time` and the billboard's
// own world position (so tongues a metre apart do not flicker in lockstep).

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

@group(2) @binding(0) var base_texture: texture_2d<f32>;
@group(2) @binding(1) var base_sampler: sampler;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
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

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = globals.time;
    // A per-tongue phase from its own world position, so two billboards
    // spawned on the same frame do not breathe in unison.
    let seed = in.world_position.x * 0.13 + in.world_position.z * 0.071;

    var uv = in.uv;
    // Turbulence is strongest at the foot of the tongue (uv.y close to 1,
    // see QuadBuilder::billboard) and calms toward the tip: a real flame
    // tip is thin and quick, the base is where it roils.
    let turb_amp = 0.11 * (0.3 + 0.7 * uv.y);
    let n1 = value_noise(vec2<f32>(uv.x * 5.0 + seed, uv.y * 3.0 - t * 3.0));
    let n2 = value_noise(vec2<f32>(uv.x * 9.0 - seed, uv.y * 6.0 - t * 5.5 + 4.0));
    uv.x += (n1 - 0.5) * turb_amp;
    uv.y += (n2 - 0.5) * turb_amp * 0.4;

    let tex = textureSample(base_texture, base_sampler, uv);

    // Fast micro-flicker on brightness only, independent of the slower
    // per-tongue sway the CPU side already applies.
    let flicker = 0.80 + 0.35 * value_noise(vec2<f32>(t * 11.0 + seed * 7.0, seed * 3.0));

    var col = in.color * tex * flicker;
    col.a = tex.a * in.color.a;
    return col;
}
