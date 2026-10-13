// The sky dome: a large inverted sphere around the whole scenario, shaded
// purely from view direction, not from geometry. `crate::sky::update_sky`
// recomputes the four colours below every frame from the sun's actual
// elevation, so this shader only has to blend them and paint the disc.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct SkyUniform {
    sun_dir: vec4<f32>,      // xyz: direction to the sun. w: day gate, 0..1.
    sun_color: vec4<f32>,
    zenith_color: vec4<f32>,
    horizon_color: vec4<f32>,
    moon_dir: vec4<f32>,     // xyz: direction to the moon. w: how high it is, 0..1.
    moon_color: vec4<f32>,
    cloud: vec4<f32>,        // xy: downwind travel, metres (XZ). z: coverage.
    cloud_lit: vec4<f32>,
    cloud_shade: vec4<f32>,
};
@group(2) @binding(0) var<uniform> sky: SkyUniform;

const CLOUD_HEIGHT: f32 = 3200.0;
const CLOUD_SCALE: f32 = 2600.0;

fn hash2(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2<f32>(1.0, 0.0)), u.x),
               mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

fn fbm(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var a = 0.5;
    var s = 0.0;
    for (var k = 0; k < 5; k++) {
        s += a * vnoise(p);
        p = p * 2.03 + vec2<f32>(17.1, 9.7);
        a *= 0.5;
    }
    return s;
}

fn cloud_density(xz: vec2<f32>) -> f32 {
    let n = fbm((xz - sky.cloud.xy) / CLOUD_SCALE);
    // coverage 0..1 moves the threshold; the narrow smoothstep keeps edges soft
    let thr = mix(0.78, 0.30, sky.cloud.z);
    return smoothstep(thr, thr + 0.22, n);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.world_position.xyz - view.world_position);

    // Below the horizon the dome still has to paint *something*: fade to a
    // darker version of the horizon colour rather than showing the zenith
    // gradient upside down.
    let up = clamp(dir.y, -1.0, 1.0);
    let sky_t = clamp(up, 0.0, 1.0);
    var col = mix(sky.horizon_color.rgb, sky.zenith_color.rgb, pow(sky_t, 0.45));
    let below_t = clamp(-up, 0.0, 1.0);
    col = mix(col, sky.horizon_color.rgb * 0.55, below_t * 0.8);

    let sun_dir = normalize(sky.sun_dir.xyz);
    let sun_amt = max(dot(dir, sun_dir), 0.0);
    // A tight disc plus a broad, dim glow around it: bloom picks the disc up,
    // the glow reads even where bloom would otherwise clip it away.
    let disc = smoothstep(0.9994, 0.9999, sun_amt);
    let glow = pow(sun_amt, 200.0) * 0.6 + pow(sun_amt, 8.0) * 0.18;
    col += sky.sun_color.rgb * (disc * 8.0 + glow) * (0.15 + 0.85 * sky.sun_dir.w);

    // The moon: always full (see `crate::sky` for why), so a plain disc plus
    // a soft glow is enough — no terminator, no phase shading. Gated purely
    // by how high it is, unlike the sun's floor, so it is not a ghost in the
    // daytime sky when it sits opposite a sun that is still up.
    let moon_dir = normalize(sky.moon_dir.xyz);
    let moon_amt = max(dot(dir, moon_dir), 0.0);
    let moon_disc = smoothstep(0.9985, 0.9996, moon_amt);
    let moon_glow = pow(moon_amt, 400.0) * 0.4 + pow(moon_amt, 6.0) * 0.05;
    col += sky.moon_color.rgb * (moon_disc * 3.0 + moon_glow) * sky.moon_dir.w;

    // Clouds: a layer on a plane CLOUD_HEIGHT above the ground, sampled where
    // the view ray crosses it, so they have parallax as the camera orbits and
    // thin out toward the horizon. Shaded by looking a step toward the sun.
    if (dir.y > 0.02) {
        let t = (CLOUD_HEIGHT - view.world_position.y) / dir.y;
        let xz = view.world_position.xz + dir.xz * t;
        let d = cloud_density(xz);
        let sun_xz = normalize(sky.sun_dir.xz + vec2<f32>(1e-4, 0.0));
        let toward_sun = cloud_density(xz + sun_xz * 220.0);
        let lit = clamp(1.0 - (toward_sun - d) * 2.2 - d * 0.25, 0.0, 1.0);
        let ccol = mix(sky.cloud_shade.rgb, sky.cloud_lit.rgb, lit);
        let fade = smoothstep(0.02, 0.22, dir.y);
        col = mix(col, ccol, d * fade * 0.92);
    }

    return vec4<f32>(col, 1.0);
}
