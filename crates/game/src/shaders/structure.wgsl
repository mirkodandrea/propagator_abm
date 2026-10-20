// Structures (`materials/structure.rs`): StandardMaterial, so a model keeps
// its own colour, maps and alpha, plus a procedural finish chosen by a shader
// def (one pipeline per finish, no branch per pixel): plaster, tiles and
// planks (`FINISH_TOWN`), dressed stone (`FINISH_STONE`), boats
// (`FINISH_BOAT`), the island's limestone (`FINISH_ROCK`). Without a finish it
// is StandardMaterial as Bevy draws it.

#import bevy_pbr::{
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
    forward_io::{VertexOutput, FragmentOutput},
}
#import bevy_pbr::mesh_functions::get_world_from_local
#import rocca::noise::{hash21, value_noise}
#import rocca::scenery::{limestone, rock_bed, rock_bump}

// Object-space position in metres: the world offset from the mesh origin
// projected on its (orthogonal) axes. For merged meshes with an identity
// transform (all the town) it is simply the world position.
fn local_metres(p: vec3<f32>, instance_index: u32) -> vec3<f32> {
    let m = get_world_from_local(instance_index);
    let d = p - m[3].xyz;
    let ax = normalize(m[0].xyz);
    let ay = normalize(m[1].xyz);
    let az = normalize(m[2].xyz);
    return vec3<f32>(dot(d, ax), dot(d, ay), dot(d, az));
}

// Exposed so windows (dark glass), lit windows and burning or charred
// vertices keep their colour: only ordinary painted surfaces get a texture.
fn surface_gate(c: vec3<f32>) -> f32 {
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    return smoothstep(0.09, 0.16, luma) * (1.0 - smoothstep(1.0, 1.3, max(c.r, max(c.g, c.b))));
}

// Town: plaster, roof tiles, flat-roof gravel and the
// planks of the pier, all in world metres.
fn town_albedo(base: vec4<f32>, p: vec3<f32>, normal: vec3<f32>) -> vec4<f32> {
    let n = normalize(normal);
    let px_m = max(length(dpdx(p)), length(dpdy(p)));
    let fine = 1.0 - smoothstep(0.25, 1.2, px_m);
    let mid = 1.0 - smoothstep(0.9, 4.0, px_m);
    let c = base.rgb;
    let gate = surface_gate(c);
    var tone = 0.0;
    var rgb = c;
    let wood = c.r > c.g && c.g > c.b && c.r < 0.6 && c.r - c.b > 0.12 && c.r - c.b < 0.3;
    if n.y > 0.35 && wood {
        // Pier and decking: planks across the walkway, each with its own
        // tone, a dark gap between them and a long grain.
        let row = floor(p.z / 0.28);
        let f = fract(p.z / 0.28);
        let gap = 1.0 - smoothstep(0.0, 0.12 + 0.5 * px_m, min(f, 1.0 - f) * 2.0);
        let grain = value_noise(vec2<f32>(p.x * 5.0, p.z * 40.0));
        tone = (hash21(vec2<f32>(row, 3.0)) - 0.5) * 0.35 + (grain - 0.5) * 0.25 * fine - gap * 0.35 * mid;
    } else if n.y > 0.35 {
        // Roofs. Pitched ones: courses of clay tiles along the slope, rows
        // at constant height, columns along the eave. Flat ones: gravel.
        let along = normalize(vec2<f32>(-n.z, n.x) + vec2<f32>(1e-4, 0.0));
        let u = dot(p.xz, along) / 0.34;
        let v = p.y / (0.30 * max(n.y, 0.25) / max(length(n.xz), 0.25) + 0.12);
        if n.y < 0.93 {
            let rowi = floor(v);
            let uu = u + 0.5 * (rowi - 2.0 * floor(rowi * 0.5));
            let col = hash21(vec2<f32>(floor(uu), rowi));
            let fv = fract(v);
            let fu = fract(uu);
            let lip = smoothstep(0.0, 0.35, fv) * (1.0 - 0.45 * smoothstep(0.6, 1.0, fv)) ;
            let seam = 1.0 - smoothstep(0.0, 0.2 + 0.4 * px_m, min(fu, 1.0 - fu));
            let moss = smoothstep(0.55, 0.85, value_noise(p.xz * 0.5 + 11.0));
            tone = (col - 0.5) * 0.40 * mid + (lip - 0.75) * 0.55 * mid - seam * 0.22 * fine - moss * 0.08 * mid
                 + (value_noise(p.xz * 0.12) - 0.5) * 0.25;
        } else {
            let grit = value_noise(p.xz * 6.0);
            tone = (value_noise(p.xz * 0.35) - 0.5) * 0.28 + (grit - 0.5) * 0.30 * fine;
        }
    } else {
        // Walls: lime plaster. Large damp and sun patches, small stucco
        // grain, vertical runs of dirt under the eaves and a darker foot.
        let blot = value_noise(vec2<f32>((p.x + p.z) * 0.22, p.y * 0.30) + 3.0);
        let grain = value_noise(vec2<f32>(p.x + p.z, p.y) * 3.5);
        let streak = value_noise(vec2<f32>((p.x + p.z) * 2.2, p.y * 0.18));
        tone = (blot - 0.5) * 0.30 * mid + (grain - 0.5) * 0.22 * fine - smoothstep(0.62, 0.9, streak) * 0.12 * mid;
        // stone plinth: the lowest metre or so reads darker
        let foot = 1.0 - smoothstep(0.0, 1.6, p.y - 0.0);
        tone -= 0.0 * foot;
    }
    rgb = c * (1.0 + tone * gate);
    return vec4<f32>(rgb, base.a);
}

// Stone: dressed ashlar in running bond, in the object's
// own frame so the courses follow the walls, with lichen on the top faces.
fn stone_albedo(base: vec4<f32>, lp: vec3<f32>, normal: vec3<f32>, p: vec3<f32>) -> vec4<f32> {
    let n = normalize(normal);
    let px_m = max(length(dpdx(p)), length(dpdy(p)));
    let fine = 1.0 - smoothstep(0.35, 1.6, px_m);
    let mid = 1.0 - smoothstep(1.2, 6.0, px_m);
    let c = base.rgb;
    let gate = surface_gate(c);
    let along = normalize(vec2<f32>(-n.z, n.x) + vec2<f32>(1e-4, 0.0));
    var q: vec2<f32>;
    if abs(n.y) > 0.7 {
        q = lp.xz / vec2<f32>(1.6, 1.0);
    } else {
        q = vec2<f32>(dot(lp.xz, along) / 1.6, lp.y / 0.8);
    }
    let row = floor(q.y);
    let qq = vec2<f32>(q.x + 0.5 * (row - 2.0 * floor(row * 0.5)) + hash21(vec2<f32>(row, 1.0)), q.y);
    let cell = floor(qq);
    let f = fract(qq);
    let e = min(min(f.x * 1.6, (1.0 - f.x) * 1.6), min(f.y * 0.8, (1.0 - f.y) * 0.8));
    let mortar = 1.0 - smoothstep(0.02, 0.05 + 0.6 * px_m, e);
    let block = (hash21(cell) - 0.5) * 0.30 + (value_noise(lp.xz * 1.3 + lp.y * 0.7) - 0.5) * 0.18 * fine;
    let weather = (value_noise(lp.xz * 0.15 + lp.y * 0.1) - 0.5) * 0.30;
    let lichen = smoothstep(0.55, 0.8, value_noise(lp.xz * 0.8 + 5.0)) * smoothstep(0.2, 0.8, n.y);
    var rgb = c * (1.0 + (block - mortar * 0.38) * mid * gate + weather * gate);
    rgb = mix(rgb, rgb * vec3<f32>(0.78, 0.92, 0.55), lichen * 0.55 * gate * mid);
    return vec4<f32>(rgb, base.a);
}

// Bare rock: the island's own limestone on meshes that are not
// terrain (the crag under the rocca, the coast's boulders). Same beds as the
// terrain's rock faces and, because they are cut from the pixel's world height,
// the same bands at the same altitudes: the crag reads as the mountain's
// bedrock coming through, not as a separate object. The vertex colour only
// tints (so shaded and sunlit blocks still differ).
fn rock_albedo(base: vec4<f32>, p: vec3<f32>, nrm: vec3<f32>) -> vec4<f32> {
    let n = normalize(nrm);
    let mpp = max(length(dpdx(p)), length(dpdy(p)));
    let fine = 1.0 - smoothstep(1.0, 3.5, mpp);
    let rb = rock_bed(p, n);
    let along = rb.x;
    let yy = rb.y;
    let u = rb.z;
    let bed = value_noise(vec2<f32>(along * 0.10, yy * 0.8));
    let seam = smoothstep(0.86, 1.0, abs(fract(u) - 0.5) * 2.0);
    let joint_off = hash21(vec2<f32>(floor(u), 3.0));
    let joint = smoothstep(0.93, 1.0, abs(fract(along * 0.45 + joint_off) - 0.5) * 2.0)
        * (1.0 - smoothstep(0.0, 1.0, abs(fract(u) - 0.5) * 2.0));
    let tone = 0.80 + 0.34 * bed + 0.10 * (value_noise(vec2<f32>(along * 0.7, yy * 0.25)) - 0.5);
    // the baked colour sets the brightness (warm grey, darker than the terrain's pale
    // faces and the ochre masonry); the limestone beds only modulate it, relative to
    // their mean, so the bands line up with the mountain's without its glare
    let beds = limestone(p) / vec3<f32>(0.59, 0.54, 0.45);
    var rock = base.rgb * 1.55 * mix(vec3<f32>(1.0), beds, 0.85) * (0.94 + 0.12 * value_noise(p.xz * 0.5)) * tone
             * (1.0 - fine * (0.42 * seam + 0.24 * joint)) * vec3<f32>(1.0, 0.95, 0.88);
    // a little lichen and dust on the upward faces
    let lichen = smoothstep(0.55, 0.8, value_noise(p.xz * 0.6 + 11.0)) * smoothstep(0.5, 0.9, n.y);
    rock = mix(rock, rock * vec3<f32>(0.82, 0.93, 0.62), lichen * 0.45);
    return vec4<f32>(rock, base.a);
}

// Boats: planked hulls, panelled sails. Told apart by
// the baked colour: pale and neutral is cloth, anything else is timber or paint.
fn boat_albedo(base: vec4<f32>, lp: vec3<f32>, normal: vec3<f32>, p: vec3<f32>) -> vec4<f32> {
    let n = normalize(normal);
    let px_m = max(length(dpdx(p)), length(dpdy(p)));
    let fine = 1.0 - smoothstep(0.4, 2.5, px_m);
    let mid = 1.0 - smoothstep(1.5, 8.0, px_m);
    let c = base.rgb;
    let gate = surface_gate(c);
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let sat = max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
    let cloth = smoothstep(0.55, 0.75, luma) * (1.0 - smoothstep(0.12, 0.3, sat)) * (1.0 - smoothstep(0.55, 0.9, abs(n.y)));
    var tone = 0.0;
    if cloth > 0.5 && abs(n.y) < 0.8 {
        // Sail: cloths sewn in vertical panels with a seam, billowing shade
        // and a sun-bleached upper edge.
        let panel = lp.x * 0.7 + lp.z * 0.7;
        let seam = 1.0 - smoothstep(0.0, 0.05 + 0.5 * px_m, abs(fract(panel / 0.55) - 0.5) * 2.0 - 0.0);
        let seam_line = smoothstep(0.92, 1.0, abs(fract(panel / 0.55) - 0.5) * 2.0);
        tone = -seam_line * 0.30 * mid + (value_noise(vec2<f32>(panel * 0.6, lp.y * 0.5)) - 0.5) * 0.22 * mid
             + (value_noise(vec2<f32>(panel * 4.0, lp.y * 4.0)) - 0.5) * 0.07 * fine;
    } else if abs(n.y) > 0.6 {
        // Deck: teak boards along the boat.
        let row = floor(lp.z / 0.22);
        let f = fract(lp.z / 0.22);
        let gap = 1.0 - smoothstep(0.0, 0.1 + 0.5 * px_m, min(f, 1.0 - f) * 2.0);
        tone = (hash21(vec2<f32>(row, 9.0)) - 0.5) * 0.35 * mid - gap * 0.50 * mid
             + (value_noise(vec2<f32>(lp.x * 3.0, lp.z * 30.0)) - 0.5) * 0.14 * fine;
    } else {
        // Hull: strakes of planking, a slightly glossy paint, grain.
        let f = fract(lp.y / 0.28);
        let gap = 1.0 - smoothstep(0.0, 0.1 + 0.5 * px_m, min(f, 1.0 - f) * 2.0);
        tone = -gap * 0.45 * mid + (hash21(vec2<f32>(floor(lp.y / 0.28), 4.0)) - 0.5) * 0.22 * mid
             + (value_noise(vec2<f32>(lp.x * 2.5, lp.y * 24.0)) - 0.5) * 0.10 * fine;
    }
    return vec4<f32>(c * (1.0 + tone * gate), base.a);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, is_front);
    pbr.material.base_color = alpha_discard(pbr.material, pbr.material.base_color);
    let p = in.world_position.xyz;
    let c = pbr.material.base_color;
#ifdef FINISH_TOWN
    pbr.material.base_color = town_albedo(c, p, in.world_normal);
#else ifdef FINISH_STONE
    pbr.material.base_color = stone_albedo(c, local_metres(p, in.instance_index), in.world_normal, p);
#else ifdef FINISH_BOAT
    pbr.material.base_color = boat_albedo(c, local_metres(p, in.instance_index), in.world_normal, p);
#else ifdef FINISH_ROCK
    pbr.material.base_color = rock_albedo(c, p, in.world_normal);
    pbr.N = normalize(pbr.N + rock_bump(p, normalize(in.world_normal)));
#endif

    var out: FragmentOutput;
    if (pbr.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr);
    } else {
        out.color = pbr.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
}
