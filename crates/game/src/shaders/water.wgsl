// The sea.
//
// The vertex shader displaces a flat lattice with a small stack of sine
// waves travelling in the wind's downwind direction, amplitude and speed
// scaled by wind speed (see `crate::sea`). The fragment shader never derives
// the wave slope by hand: `dpdx`/`dpdy` of the already-displaced world
// position gives the facet normal for free, which is what makes the big
// rollers shade correctly without a closed-form derivative of the sum of
// sines. A second, finer normal wobble from scrolling noise adds wind chop
// at a scale too small to be worth displacing geometry for.

#import bevy_pbr::forward_io::{Vertex, VertexOutput}
#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::mesh_view_bindings::{view, globals, fog, lights}
#import bevy_pbr::mesh_view_types
#import bevy_pbr::fog as scene_fog

struct WaterUniform {
    // xy: downwind unit direction in world XZ (bevy x, z). z: wind speed, m/s.
    wind: vec4<f32>,
    // xyz: unit direction toward the sun. w: day gate, 0 at night to 1 at noon.
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    deep_color: vec4<f32>,
    shallow_color: vec4<f32>,
};
@group(2) @binding(0) var<uniform> water: WaterUniform;

fn wave_height(p: vec2<f32>, t: f32) -> f32 {
    let dir0 = water.wind.xy;
    let perp = vec2<f32>(-dir0.y, dir0.x);
    let dir1 = normalize(dir0 * 0.6 + perp * 0.5);
    let speed = water.wind.z;
    // A dead calm sea is glassy: amplitude collapses toward zero with speed,
    // same as the ember/spotting model treats zero wind as no transport.
    // Capped at 0.5: `WATER_BASE_Y` (`crate::sea`) is only sized to clear a
    // trough this deep, and a bigger cap here needs a bigger base height to
    // match or the terrain pokes through at the bottom of the swell.
    let amp = clamp(speed * 0.045, 0.03, 0.5);
    let s = 0.7 + speed * 0.05;
    var h = 0.0;
    h += amp * sin(dot(p, dir0) * 0.055 + t * s);
    h += amp * 0.5 * sin(dot(p, dir1) * 0.11 + t * s * 1.5 + 1.7);
    h += amp * 0.25 * sin(dot(p, dir0) * 0.21 - t * s * 2.1 + 4.1);
    return h;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    var world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    );
    world_position.y += wave_height(world_position.xz, globals.time);

    out.position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
    out.uv = vertex.uv;
    out.color = vertex.color;
    return out;
}

// The scene's own distance fog, applied by hand.
//
// `StandardMaterial` gets this for free at the end of its fragment shader;
// this material does not, and the sea is big enough that going without shows.
// Unfogged water stays the same flat, saturated blue all the way to the far
// edge of the mesh while the coastline beside it hazes out properly, so the
// water reads as a solid slab laid over the horizon, with a hard straight
// line where it ends and `crate::far_terrain`'s (correctly fogged) flat sea
// takes over. That line is the whole "the world outside the window is broken"
// look — the sea being the one surface in the scene that ignores the
// atmosphere.
//
// The body is `bevy_pbr::pbr_functions::apply_fog` transcribed, because
// importing that module would also pull in `pbr_bindings`, which declares
// `StandardMaterial`'s own `@group(2)` layout on top of this material's.
fn apply_scene_fog(input_color: vec4<f32>, world_position: vec3<f32>) -> vec4<f32> {
    let view_to_world = world_position - view.world_position.xyz;
    let distance = length(view_to_world);

    var scattering = vec3<f32>(0.0);
    if fog.directional_light_color.a > 0.0 {
        let view_to_world_normalized = view_to_world / distance;
        for (var i: u32 = 0u; i < lights.n_directional_lights; i = i + 1u) {
            let light = lights.directional_lights[i];
            scattering += pow(
                max(dot(view_to_world_normalized, light.direction_to_light), 0.0),
                fog.directional_light_exponent,
            ) * light.color.rgb * view.exposure;
        }
    }

    if fog.mode == mesh_view_types::FOG_MODE_LINEAR {
        return scene_fog::linear_fog(fog, input_color, distance, scattering);
    } else if fog.mode == mesh_view_types::FOG_MODE_EXPONENTIAL {
        return scene_fog::exponential_fog(fog, input_color, distance, scattering);
    } else if fog.mode == mesh_view_types::FOG_MODE_EXPONENTIAL_SQUARED {
        return scene_fog::exponential_squared_fog(fog, input_color, distance, scattering);
    } else if fog.mode == mesh_view_types::FOG_MODE_ATMOSPHERIC {
        return scene_fog::atmospheric_fog(fog, input_color, distance, scattering);
    }
    return input_color;
}

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
    let dpx = dpdx(in.world_position.xyz);
    let dpy = dpdy(in.world_position.xyz);
    var normal = normalize(cross(dpy, dpx));
    if normal.y < 0.0 {
        normal = -normal;
    }

    let speed = water.wind.z;
    let chop_dir = water.wind.xy;
    let chop = clamp(speed * 0.05, 0.0, 1.0);
    let uv = in.world_position.xz * 0.06 + chop_dir * t * (0.15 + speed * 0.03);
    let n1 = value_noise(uv * 3.0) - 0.5;
    let n2 = value_noise(uv * 7.0 + vec2<f32>(11.0, 5.0)) - 0.5;
    normal = normalize(normal + vec3<f32>(n1 * 0.35 * chop, 0.0, n2 * 0.35 * chop));

    let view_dir = normalize(view.world_position - in.world_position.xyz);
    let fresnel = pow(1.0 - clamp(dot(normal, view_dir), 0.0, 1.0), 4.0);

    // `in.color.r` carries depth in metres from `crate::sea` (0 at the
    // shoreline), sampled from the same terrain heightfield the ground
    // colour under the water uses.
    let depth = clamp(in.color.r / 6.0, 0.0, 1.0);
    var col = mix(water.shallow_color.rgb, water.deep_color.rgb, depth);

    // This shader does not run through the scene's own lighting, so it has
    // to dim itself: without this the sea stays lit like noon after the sun
    // sets, which is exactly the always-bright-water bug this comment is
    // here to stop someone reintroducing. `sun_color.a` carries the sun's
    // own illuminance-normalised brightness (`SunState::brightness`), not
    // the coarser `sin(elevation)` gate used for the specular term below —
    // that gate alone stayed under 0.5 at a properly sunny mid-afternoon
    // sun and made the sea visibly dimmer than a shoreline lit by the same
    // light.
    let light = mix(0.12, 1.0, water.sun_color.a);
    col *= light;

    let sky_reflection = (water.deep_color.rgb * 0.4 + vec3<f32>(0.55, 0.62, 0.68)) * light;
    col = mix(col, sky_reflection, fresnel * 0.6);

    let sun_dir = normalize(water.sun_dir.xyz);
    let half_dir = normalize(sun_dir + view_dir);
    // A calm sea gives a tight glitter; wind-chopped water spreads the same
    // light over a wider, softer streak.
    let spec_power = mix(700.0, 60.0, chop);
    let spec = pow(clamp(dot(normal, half_dir), 0.0, 1.0), spec_power) * water.sun_dir.w;
    col += water.sun_color.rgb * spec * 2.5;

    // Foam only right at the shoreline, thickening with wind.
    let foam_edge = 1.0 - smoothstep(0.0, 0.35 + chop * 0.5, depth);
    let foam_noise = value_noise(in.world_position.xz * 0.15 + t * 0.3);
    let foam = foam_edge * step(0.35, foam_noise + foam_edge * 0.4);
    col = mix(col, vec3<f32>(0.9, 0.93, 0.95) * light, foam * 0.85);

    return apply_scene_fog(vec4<f32>(col, 1.0), in.world_position.xyz);
}
