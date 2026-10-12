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
};
@group(2) @binding(0) var<uniform> sky: SkyUniform;

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

    return vec4<f32>(col, 1.0);
}
