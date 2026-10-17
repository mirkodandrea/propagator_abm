// The forecast's zones (+1 h, +3 h): an island-wide grid draped on the terrain,
// each vertex carrying its signed distance in metres to the zones' contours
// (uv.x: +1 h, uv.y: +3 h; positive inside). Interpolated, the distance gives
// the fill and a thin contour line of constant width on the screen. No light
// and no fog, like the other signs; vertices are pulled toward the camera as
// in flat.wgsl.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::{view, globals},
    view_transformations::position_world_to_clip,
}

struct Zone {
    // rgb: colour of +1 h / +3 h; a: fill opacity
    near: vec4<f32>,
    far: vec4<f32>,
    // x: contour line opacity
    line: vec4<f32>,
    pull: vec4<f32>,
};
@group(2) @binding(0) var<uniform> zone: Zone;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) eta: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) dist: vec2<f32>,
    @location(1) eta: f32,
    @location(2) ground: vec2<f32>,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    var w = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0)).xyz;
    let to_cam = view.world_position - w;
    let d = max(length(to_cam), 1.0);
    w += to_cam / d * min(clamp(d * zone.pull.x, zone.pull.y, zone.pull.z), d * 0.5);
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(w);
    out.dist = v.uv;
    out.eta = v.eta.x;
    out.ground = w.xz;
    return out;
}

// A single wave runs over the fill, on the wall clock (the world is stopped
// in the pause, which is when the forecast has to catch the eye). Its front
// sweeps the smoothed arrival time (minutes after issue, the members' median)
// from now to the last horizon, so it travels where the fire is expected to
// go, quick where the fire is quick; and again at once, with no rest.
const HORIZON_MIN: f32 = 180.0;
const SWEEP_S: f32 = 4.0;
const CYCLE_S: f32 = 4.0;
const WAVE_WIDTH_MIN: f32 = 14.0;
const BASE: f32 = 0.85;
const GAIN: f32 = 0.5;
// The wave's profile, 0..1: a crest of height 1 at the front.
fn crest(eta: f32) -> f32 {
    let u = (globals.time % CYCLE_S) / SWEEP_S;
    // the front starts a width before 0 and ends a width after the horizon
    let front = mix(-0.3 * WAVE_WIDTH_MIN, HORIZON_MIN + WAVE_WIDTH_MIN, u);
    let x = (eta - front) / WAVE_WIDTH_MIN;
    // sharp leading edge (x > 0: not yet reached), long trailing tail
    let k = select(x * 0.6, x * 1.6, x > 0.0);
    return exp(-k * k);
}

// The crest read as a swell of the ground: its slope on the ground (from the
// screen derivatives, so it holds at any zoom) gives a normal, lit by a fixed
// sun. Returns the shade: 1 flat, brighter on the side facing the sun, darker
// on the other; and in .y a glint on the crest.
const SWELL_M: f32 = 45.0;
const SUN: vec3<f32> = vec3<f32>(-0.45, 0.75, 0.48);
fn swell(h: f32, ground: vec2<f32>) -> vec2<f32> {
    let dg1 = dpdx(ground);
    let dg2 = dpdy(ground);
    let det = dg1.x * dg2.y - dg1.y * dg2.x;
    if abs(det) < 1e-6 {
        return vec2<f32>(1.0, 0.0);
    }
    let dh1 = dpdx(h) * SWELL_M;
    let dh2 = dpdy(h) * SWELL_M;
    let g = vec2<f32>(dh1 * dg2.y - dh2 * dg1.y, dg1.x * dh2 - dg2.x * dh1) / det;
    let n = normalize(vec3<f32>(-g.x, 1.0, -g.y));
    let l = normalize(SUN);
    let shade = dot(n, l) / l.y;
    let spec = pow(max(dot(reflect(-l, n), vec3<f32>(0.0, 1.0, 0.0)), 0.0), 24.0);
    return vec2<f32>(shade, spec);
}

// One zone: its fill, and the line along its contour (premultiplied).
fn layer(d: f32, colour: vec4<f32>, wave: f32) -> vec4<f32> {
    let fw = max(fwidth(d), 1e-3);
    let inside = smoothstep(-0.75, 0.75, d / fw);
    let line = (1.0 - smoothstep(0.6, 1.6, abs(d) / fw)) * zone.line.x;
    let a = max(inside * colour.a * wave, line);
    return vec4<f32>(colour.rgb * a, a);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let h = crest(in.eta);
    let wave = BASE + GAIN * h;
    let far = layer(in.dist.y, zone.far, wave);
    let near = layer(in.dist.x, zone.near, wave);
    // +1 h over +3 h, premultiplied
    var c = near + far * (1.0 - near.a);
    if c.a < 0.003 {
        discard;
    }
    // tactical view: more saturated zones (gain in pull.w)
    let s = swell(h, in.ground);
    let rgb = clamp(c.rgb / c.a * clamp(s.x, 0.35, 1.9) + vec3<f32>(s.y * 0.3), vec3<f32>(0.0), vec3<f32>(1.0));
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>(clamp(mix(vec3<f32>(luma), rgb, 1.0 + zone.pull.w), vec3<f32>(0.0), vec3<f32>(1.0)), c.a);
}
