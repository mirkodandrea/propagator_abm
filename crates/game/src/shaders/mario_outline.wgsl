// Screen-space silhouette contour, tested against scene-only depth.
#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput},
    mesh_view_bindings::view,
}
#ifdef DEPTH_PREPASS
#import bevy_pbr::prepass_utils::prepass_depth
#endif

@group(2) @binding(0) var mario_mask: texture_2d<f32>;
@group(2) @binding(1) var mask_sampler: sampler;
@group(2) @binding(2) var<uniform> bounds: vec4<f32>;

@vertex
fn vertex(in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(mix(bounds.xy, bounds.zw, in.position.xy * 0.5 + 0.5), 0.0, 1.0);
    out.world_position = vec4<f32>(0.0);
    out.world_normal = vec3<f32>(0.0, 0.0, 1.0);
    out.uv = vec2<f32>(in.position.x * 0.5 + 0.5, 0.5 - in.position.y * 0.5);
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = in.instance_index;
#endif
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
    let size = vec2<i32>(textureDimensions(mario_mask));
    let centre = textureLoad(mario_mask, clamp(pixel, vec2<i32>(0), size - 1), 0);
    var coverage_min = centre.a;
    var coverage_max = centre.a;
    var hidden_coverage = 0.0;
    // Dilate/erode the *whole* silhouette, so internal triangles and body-part
    // overlaps never become extra contours. Fixed two-pixel width at any zoom.
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let sample_pixel = clamp(pixel + vec2<i32>(x, y) * 2, vec2<i32>(0), size - 1);
            let neighbour = textureLoad(mario_mask, sample_pixel, 0);
            coverage_min = min(coverage_min, neighbour.a);
            coverage_max = max(coverage_max, neighbour.a);
#ifdef DEPTH_PREPASS
            // Compare at the covered sample itself. Testing an expanded rim
            // against nearby foreground ground would outline visible feet.
            let scene_depth = prepass_depth(vec4<f32>(vec2<f32>(sample_pixel), 0.0, 1.0), 0u);
            if neighbour.a > 0.5 && neighbour.r < scene_depth - 0.000001 {
                hidden_coverage = max(hidden_coverage, neighbour.a);
            }
#endif
        }
    }
    let edge = coverage_max - coverage_min;
    if edge < 0.2 { discard; }
    // Mario skips the scene prepass, so his own front faces cannot trigger it.
    if hidden_coverage < 0.5 { discard; }
    return vec4<f32>(0.025, 0.35, 1.25, edge * 0.95);
}
