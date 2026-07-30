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
    var col = in.color;
    if col.r > 1.0 {
        let t = globals.time;
        // World-space noise, not UV: the overlay lattice spacing changes with
        // `OVERLAY_SUBDIV`, and this needs to look the same regardless.
        let p = vec2<f32>(in.world_position.x * 0.06 + t * 2.4, in.world_position.z * 0.06 - t * 1.9);
        let n = value_noise(p) * 0.6 + value_noise(p * 2.3 + 5.0) * 0.4;
        let flicker = 0.72 + 0.55 * n;
        col = vec4<f32>(col.r * flicker, col.g * flicker, col.b * flicker, col.a);
    }
    return col;
}
