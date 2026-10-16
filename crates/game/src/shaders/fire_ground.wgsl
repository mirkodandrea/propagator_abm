// Fragment shader for the ground overlay.
//
// The CPU side (fire_view::update_overlay) rebuilds this mesh's vertex
// colours at most every 150 ms — plenty for the burn perimeter, which does
// not move that fast, but not enough on its own to look "alive" at 1x speed.
// `sample_color` flags a hot, glowing fragment by pushing its red channel
// above 1.0 (the same convention the flame billboards use, so the two never
// disagree about what is currently burning); this shader only adds motion to
// that already-decided glow. Every other layer (Intensity, Arrival, Hazard)
// never sets red above 1.0, so they pass through unanimated.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals
#import rocca::noise::{hash21, value_noise}

@group(2) @binding(0) var road_tex: texture_2d<f32>;
@group(2) @binding(1) var road_samp: sampler;
@group(2) @binding(2) var<uniform> mask_rect: vec4<f32>;

// Keep in step with `ground_mask.rs` and `retro.wgsl`.
const RANGE_M: f32 = 12.0;

// Fractal Brownian motion, domain-warped: the warp is what turns blobby
// noise into the tendrilled, branching texture of a real fire front.
fn fbm(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var a = 0.5;
    var s = 0.0;
    for (var i = 0; i < 5; i++) {
        s += a * value_noise(p);
        p = p * 2.07 + vec2<f32>(17.3, 9.1);
        a *= 0.5;
    }
    return s / 0.96875;
}

fn fire_ramp(h: f32) -> vec3<f32> {
    // ember red -> orange -> yellow -> near white, h in 0..1
    let c0 = vec3<f32>(0.80, 0.08, 0.02);
    let c1 = vec3<f32>(2.50, 0.60, 0.06);
    let c2 = vec3<f32>(3.60, 1.80, 0.28);
    let c3 = vec3<f32>(4.40, 3.50, 1.60);
    let a = clamp(h * 3.0, 0.0, 1.0);
    let b = clamp(h * 3.0 - 1.0, 0.0, 1.0);
    let c = clamp(h * 3.0 - 2.0, 0.0, 1.0);
    return mix(mix(mix(c0, c1, a), c2, b), c3, c);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var col = in.color;
    // `w` fades the fractal treatment in smoothly over the glow gradient so
    // there is no seam where the CPU colour crosses the hot threshold.
    let w = smoothstep(0.35, 1.0, col.r);
    if w > 0.0 {
        let t = globals.time;
        let wp = in.world_position.xz;
        // Heat decoded from the CPU glow (r = char + 2.4 * glow).
        let heat = clamp((col.r - 0.3) / 2.4, 0.0, 1.0);

        // Domain warp, slowly advected by the wind-ish drift.
        let q = wp * 0.05 + vec2<f32>(t * 0.35, -t * 0.28);
        let warp = vec2<f32>(fbm(q + 3.7), fbm(q + 11.2)) - 0.5;
        let p = wp * 0.11 + warp * 2.6;
        let n = fbm(p + vec2<f32>(t * 0.6, -t * 0.45));
        // Fine, fast crackle on top: individual embers winking.
        let crackle = value_noise(wp * 0.9 + vec2<f32>(t * 3.0, t * 2.1));

        // Hot cells are where local heat beats the noise threshold: the
        // front breaks into patches and fingers, with a ragged border.
        // Slow breathing of the whole front, offset by position so it rolls
        // along the line instead of pulsing in unison.
        let breath = 0.5 + 0.5 * sin(t * 2.2 + wp.x * 0.035 + wp.y * 0.027);
        let local = heat * 1.75 - (1.0 - n) * 0.70 + (crackle - 0.5) * 0.30 + breath * 0.10;
        let hot = smoothstep(0.0, 0.18, local);

        let fire = fire_ramp(clamp(local, 0.0, 1.0)) * (0.85 + 0.55 * crackle);
        // What the cooler gaps look like: dark char with a faint red glint.
        let char_col = vec3<f32>(0.06, 0.03, 0.025) + vec3<f32>(0.9, 0.12, 0.0) * (1.0 - hot) * heat * n;
        let fractal = mix(char_col, fire, hot);
        let rgb = mix(col.rgb, fractal, w);

        // Ragged outer edge: alpha is a smooth coverage ramp on the CPU.
        let edge = smoothstep(0.12, 0.5, col.a + (n - 0.5) * 0.55);
        let alpha = mix(col.a, col.a * edge, w);
        col = vec4<f32>(rgb, alpha);
    }

    // Cold scar: ragged edge and ash grain at pixel scale, and the roads
    // showing through it. The vertex lattice is 5 m or coarser, so without
    // this the scar's border is a visible staircase and roads vanish under it.
    let wpc = in.world_position.xz;
    let cold = 1.0 - w;
    if cold > 0.0 && col.a > 0.0 {
        let n2 = 0.6 * value_noise(wpc * 0.45) + 0.4 * value_noise(wpc * 1.7 + 5.3);
        let ragged = smoothstep(0.10, 0.60, col.a + (n2 - 0.5) * 0.7);
        let grain = 0.80 + 0.40 * (0.5 * n2 + 0.5 * value_noise(wpc * 5.0));
        col = vec4<f32>(col.rgb * mix(1.0, grain, cold), mix(col.a, col.a * ragged, cold));
    }
    let road_d = (textureSample(road_tex, road_samp, vec2<f32>(wpc.x, -wpc.y) * mask_rect.xy).rg * 2.0 - 1.0) * RANGE_M;
    let mpp = max(length(dpdx(wpc)), length(dpdy(wpc)));
    let aa = max(0.5 * mpp, 0.25);
    let on_road = max(
        1.0 - smoothstep(max(0.0, 0.8 * mpp - 4.8) - aa, max(0.0, 0.8 * mpp - 4.8) + aa, road_d.x),
        1.0 - smoothstep(max(0.0, 0.8 * mpp - 1.6) - aa, max(0.0, 0.8 * mpp - 1.6) + aa, road_d.y),
    );
    col.a = col.a * (1.0 - 0.72 * on_road * cold);
    return col;
}
