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
@group(2) @binding(2) var<uniform> mode: vec4<f32>;

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

fn fbm2(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var a = 0.5;
    var s = 0.0;
    for (var i = 0; i < 3; i++) {
        s += a * value_noise(p);
        p = p * 2.13 + vec2<f32>(5.1, 1.7);
        a *= 0.5;
    }
    return s / 0.875;
}

// Blackbody-ish ramp: deep red tip -> orange -> yellow -> white-hot core.
fn flame_ramp(h: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.9, 0.10, 0.02);
    let c1 = vec3<f32>(2.4, 0.55, 0.06);
    let c2 = vec3<f32>(3.4, 1.7, 0.25);
    let c3 = vec3<f32>(4.2, 3.4, 1.8);
    let a = clamp(h * 3.0, 0.0, 1.0);
    let b = clamp(h * 3.0 - 1.0, 0.0, 1.0);
    let c = clamp(h * 3.0 - 2.0, 0.0, 1.0);
    return mix(mix(mix(c0, c1, a), c2, b), c3, c);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = globals.time;
    if mode.x < 0.5 {
        let s0 = in.world_position.x * 0.13 + in.world_position.z * 0.071;
        let tx = textureSample(base_texture, base_sampler, in.uv);
        let fl = 0.80 + 0.35 * value_noise(vec2<f32>(t * 11.0 + s0 * 7.0, s0 * 3.0));
        var c0 = in.color * tx * fl;
        c0.a = tx.a * in.color.a;
        return c0;
    }
    // A per-tongue phase from its own world position, so two billboards
    // spawned on the same frame do not breathe in unison.
    let seed = in.world_position.x * 0.13 + in.world_position.z * 0.071;

    // uv.y: 0 at the tip, 1 at the foot (see QuadBuilder::billboard).
    var uv = in.uv;
    let foot = uv.y;

    // Fire rises: noise scrolls upward fast, and the sideways warp grows
    // toward the tip so the flame licks and curls instead of standing still.
    let rise = vec2<f32>(uv.x * 4.0 + seed, uv.y * 3.5 + t * 2.6);
    let warp = fbm2(rise) - 0.5;
    uv.x += warp * (0.22 - 0.12 * foot);
    uv.y += (value_noise(vec2<f32>(uv.x * 6.0 - seed, uv.y * 5.0 + t * 4.5)) - 0.5) * 0.06;

    let tex = textureSample(base_texture, base_sampler, uv);

    // Erode the silhouette with rising noise: tongues break into flickering
    // wisps toward the tip, but the foot stays solid.
    let erode = fbm2(vec2<f32>(uv.x * 7.0 + seed * 2.0, uv.y * 6.0 + t * 3.8));
    let body = tex.a * smoothstep(0.0, 0.55, tex.a * 1.6 + (erode - 0.5) * (0.9 - 0.5 * foot));

    // Heat: hot at the foot and core, cooling toward the tip and edges.
    let flicker = 0.85 + 0.30 * value_noise(vec2<f32>(t * 9.0 + seed * 7.0, seed * 3.0));
    let heat = clamp(body * (0.35 + 0.85 * foot) * flicker + (erode - 0.5) * 0.25, 0.0, 1.0);

    // in.color carries the CPU's brightness (cell intensity); use it as a
    // gain on the ramp, not as the colour itself.
    let gain = clamp(in.color.r * 0.30, 0.55, 1.6);
    var col = vec4<f32>(flame_ramp(heat) * gain * flicker, body * in.color.a);
    return col;
}
