// Restrained late-90s training-simulation finish. This is an extension of the
// stock PBR fragment path: vertex colours and StandardMaterial alpha/culling
// remain intact, while each material decides how much rim and motion it gets.

#import bevy_pbr::{
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
    forward_io::{VertexOutput, FragmentOutput},
}
#import bevy_pbr::mesh_view_bindings::globals
#import bevy_pbr::mesh_functions::get_world_from_local
#import rocca::noise::{hash21, value_noise}

struct RetroUniform {
    enabled: vec4<f32>,
};
@group(2) @binding(100) var<uniform> retro: RetroUniform;

@group(2) @binding(101) var road_tex: texture_2d<f32>;
@group(2) @binding(102) var road_samp: sampler;
@group(2) @binding(103) var<uniform> mask_rect: vec4<f32>;
@group(2) @binding(104) var grass_tex: texture_2d<f32>;
@group(2) @binding(105) var grass_samp: sampler;
// x: tactical desaturation, 0 (full colour) to 1; see `tactical.rs`.
@group(2) @binding(106) var<uniform> tactical: vec4<f32>;

// Road mask encoding and ribbon geometry: keep in step with `ground_mask.rs`.
const RANGE_M: f32 = 12.0;
const KERB_M: f32 = 1.3;
const ASPHALT: vec3<f32> = vec3<f32>(0.214, 0.214, 0.223);
const ASPHALT_KERB: vec3<f32> = vec3<f32>(0.045, 0.045, 0.050);
const PAVING: vec3<f32> = vec3<f32>(0.74, 0.67, 0.55);
const TRACK: vec3<f32> = vec3<f32>(0.296, 0.234, 0.141);
const TRACK_KERB: vec3<f32> = vec3<f32>(0.119, 0.091, 0.056);

// One layer of tussocks on a jittered grid of `size` metres. Returns
// (coverage, tone, tip lightness, sun-side shading). `occupancy` is the share
// of cells that hold a tuft; radii are in cell units.
fn tuft_layer(wp: vec2<f32>, size: f32, occupancy: f32, r_min: f32, r_max: f32, seed: f32) -> vec4<f32> {
    let q = wp / size;
    let base_cell = floor(q);
    // Tuft centres sit in [0.15, 0.85] of their cell and radii are at most 0.5,
    // so only the own cell and the neighbour on the nearer side can reach a
    // pixel: 2 x 2 cells, not 3 x 3.
    let f = q - base_cell;
    let step_x = select(-1, 1, f.x > 0.5);
    let step_y = select(-1, 1, f.y > 0.5);
    var best = vec4<f32>(0.0, 0.5, 0.0, 0.0);
    for (var iy = 0; iy < 2; iy++) {
        for (var ix = 0; ix < 2; ix++) {
            let dx = ix * step_x;
            let dy = iy * step_y;
            let c = base_cell + vec2<f32>(f32(dx), f32(dy)) + seed;
            let h1 = hash21(c);
            if h1 > occupancy { continue; }
            let h2 = hash21(c + 17.3);
            let h3 = hash21(c + 31.7);
            let centre = c - seed + vec2<f32>(0.15 + 0.7 * h2, 0.15 + 0.7 * h3);
            let r = r_min + (r_max - r_min) * hash21(c + 5.1);
            let v = (q - centre) / r;
            let d = length(v);
            let ang = atan2(v.y, v.x);
            // Blades: radial streaks that lengthen and shorten around the tuft.
            let blades = 0.5 + 0.5 * sin(ang * (9.0 + 7.0 * h2) + h3 * 6.28 + d * 1.5);
            let reach = 0.55 + 0.45 * blades;
            let a = 1.0 - smoothstep(reach * 0.80, reach, d);
            if a > best.x {
                let side = dot(v / max(d, 0.001), vec2<f32>(-0.55, 0.83)) * (1.0 - d * 0.5);
                best = vec4<f32>(a, h1 / max(occupancy, 0.001) * 0.5 + 0.5 * hash21(c + 9.9), smoothstep(0.30, 0.95, d / reach), side);
            }
        }
    }
    return best;
}

// Terrain only (`enabled.w` = 1): the road network, paving, waterline and
// meadow, drawn over the ground colour from the distance-field mask.
fn terrain_albedo(base: vec4<f32>, p: vec3<f32>, mask_s: vec4<f32>, meadow: f32) -> vec4<f32> {
    let mask_d = (mask_s.rg * 2.0 - 1.0) * RANGE_M;
    let metres_per_px = max(length(dpdx(p.xz)), length(dpdy(p.xz)));
    // A road never gets thinner than about a pixel and a half, so the
    // network stays legible from high up; the kerb fades into the asphalt
    // as it would become sub-pixel.
    let grow_t = max(0.0, 0.8 * metres_per_px - 1.6);
    let grow_d = max(0.0, 0.8 * metres_per_px - 4.8);
    let aa = max(0.5 * metres_per_px, 0.25);
    let kerb_fade = clamp(1.0 - (metres_per_px - 3.0) / 8.0, 0.0, 1.0);
    var rgb = base.rgb;

    // Meadow: dry tussocks over the ground colour, in two sizes and in
    // clumps with bare patches between. Far away they average into a
    // grain of the same mean tone.
    if mask_rect.z > 0.5 && meadow > 0.01 {
        let wp = p.xz;
        let patch_dry = value_noise(wp * 0.02 + 40.0);
        let cover = smoothstep(0.35, 0.65, meadow + (value_noise(wp * 0.35) - 0.5) * 0.45);
        let clump = smoothstep(0.20, 0.70, value_noise(wp * 0.045 + 7.0) * 0.7 + value_noise(wp * 0.17) * 0.3);
        let fine = 1.0 - smoothstep(1.2, 4.0, metres_per_px);
        var big = vec4<f32>(0.0, 0.5, 0.0, 0.0);
        var small = vec4<f32>(0.0, 0.5, 0.0, 0.0);
        if fine > 0.02 {
            big = tuft_layer(wp, 5.0, 0.25 + 0.75 * cover * (0.35 + 0.65 * clump), 0.28, 0.50, 0.0);
            // The small tufts only show up close.
            if fine > 0.5 {
                small = tuft_layer(wp, 2.2, 0.55 * cover * clump, 0.22, 0.40, 40.0);
            }
        }
        // Dark brown-olive tussocks, lit on the sun side and lighter at the tips.
        let body_k = vec3<f32>(0.50, 0.46, 0.36) * mix(1.0, 0.9, patch_dry);
        var near = rgb * 0.97;
        for (var i = 0; i < 2; i++) {
            let t = select(big, small, i == 1);
            let lit = 1.0 + 0.22 * t.w;
            let col = mix(rgb * body_k * (0.78 + 0.5 * t.y) * lit, rgb * vec3<f32>(0.92, 0.88, 0.70), t.z * 0.55);
            near = mix(near, col, t.x);
        }
        let far = rgb * (1.0 - 0.14 * cover * (0.5 + 0.5 * clump));
        rgb = mix(rgb, mix(far, near, fine), cover);
    }

    // Waterline: signed distance, positive in the sea. The seabed under
    // the (partly transparent) water goes from sand through turquoise to
    // deep blue; the dry side gets a damp edge. Vertex colour only knows
    // the coast to 5 m, the mask knows it to the pixel.
    let sd = mask_s.a * 255.0 - 127.5;
    let wet = smoothstep(-0.5, 1.5, sd);
    if wet > 0.0 {
        let ripple = 0.5 * value_noise(p.xz * 0.9) + 0.5 * value_noise(p.xz * 0.23 + 4.0);
        let sandy = vec3<f32>(0.62, 0.57, 0.40) * (0.88 + 0.22 * ripple);
        let lagoon = vec3<f32>(0.10, 0.30, 0.32);
        let deep = vec3<f32>(0.012, 0.034, 0.075);
        var bed = mix(sandy, lagoon, smoothstep(2.0, 18.0, sd));
        bed = mix(bed, deep, smoothstep(14.0, 55.0, sd));
        rgb = mix(rgb, bed, wet);
    } else {
        rgb = rgb * mix(1.0, 0.80, smoothstep(-3.5, 0.0, sd));
    }

    // Paved ground: town squares and yards, as running-bond stones. The
    // edge is ragged with noise; the stone pattern fades to its mean
    // colour as it shrinks below a pixel, so it never shimmers.
    let wp = p.xz;
    let ragged = (0.6 * value_noise(wp * 0.7) + 0.4 * value_noise(wp * 0.15 + 9.1) - 0.5) * 0.30;
    let paved = smoothstep(0.42, 0.58, mask_s.b + ragged);
    // Slabs 1.6 x 0.8 m in running bond, each its own tone, with a seam
    // that never gets thinner than a pixel.
    let q0 = vec2<f32>(wp.x / 1.6, wp.y / 0.8);
    let row = floor(q0.y);
    let q = vec2<f32>(q0.x + 0.5 * (row - 2.0 * floor(row * 0.5)), q0.y);
    let cell = floor(q);
    let f = fract(q);
    let seam_w = max(0.04, 0.7 * metres_per_px / 0.8);
    let edge_d = min(min(f.x * 2.0, (1.0 - f.x) * 2.0), min(f.y, 1.0 - f.y));
    let seam = 1.0 - smoothstep(seam_w * 0.5, seam_w * 1.5, edge_d);
    let detail = 1.0 - smoothstep(0.7, 1.8, metres_per_px);
    let tone = hash21(cell);
    let stone = mix(vec3<f32>(0.90, 0.92, 0.95), vec3<f32>(1.08, 1.02, 0.92), tone) * (0.90 + 0.16 * hash21(cell + 7.7));
    let patch_tone = 0.90 + 0.16 * value_noise(wp * 0.05) + 0.06 * value_noise(wp * 0.4);
    let paving = PAVING * patch_tone * mix(vec3<f32>(0.97), stone * (1.0 - 0.22 * seam), detail);
    rgb = mix(rgb, paving, paved);
    let t_kerb = 1.0 - smoothstep(grow_t + KERB_M - aa, grow_t + KERB_M + aa, mask_d.y);
    let t_surf = 1.0 - smoothstep(grow_t - aa, grow_t + aa, mask_d.y);
    let d_kerb = 1.0 - smoothstep(grow_d + KERB_M - aa, grow_d + KERB_M + aa, mask_d.x);
    let d_surf = 1.0 - smoothstep(grow_d - aa, grow_d + aa, mask_d.x);
    rgb = mix(rgb, mix(TRACK, TRACK_KERB, kerb_fade), t_kerb);
    rgb = mix(rgb, TRACK, t_surf);
    rgb = mix(rgb, mix(ASPHALT, ASPHALT_KERB, kerb_fade), d_kerb);
    let tarmac = ASPHALT * mix(0.97, 0.86 + 0.28 * value_noise(wp * 2.3) * value_noise(wp * 0.4 + 3.0) * 1.6, detail);
    rgb = mix(rgb, tarmac, d_surf);
    return vec4<f32>(rgb, base.a);
}

// Vegetation only (`enabled.w` = 2): breaks the flat vertex colour into
// leaf clumps and a fine speckle, plus a warm sunlit / cool shaded lean.
// Two value-noise taps, faded out once a pixel spans more than ~2 m, so
// the far forest costs next to nothing and never shimmers.
fn foliage_albedo(base: vec4<f32>, p: vec3<f32>, normal: vec3<f32>, alive: f32) -> vec4<f32> {
    let px_m = max(length(dpdx(p)), length(dpdy(p)));
    let fine = 1.0 - smoothstep(1.0, 3.5, px_m);
    let mid = 1.0 - smoothstep(2.0, 7.0, px_m);
    // Per-patch tint on a ~3.5 m lattice: about one macchia shrub each, so
    // neighbouring bushes differ in tone; it outlives the fine detail.
    let patch_tone = hash21(floor(p.xz / 3.5)) - 0.5;
    let clump = value_noise(p.xz * 0.9 + vec2<f32>(p.y * 0.6, p.y * 0.35));
    // Posterised clumps: distinct light and dark leaf masses with soft
    // steps between them, rather than a smooth gradient.
    let steps = floor(clump * 3.0) / 3.0 + smoothstep(0.0, 0.18, fract(clump * 3.0)) / 3.0;
    var tone = patch_tone * 0.45 * mid + (mix(clump, steps, 0.7) - 0.5) * 0.75 * mid;
    if fine > 0.0 {
        let speck = value_noise(p.xz * 3.2 + vec2<f32>(p.y * 3.3, -p.y * 2.1));
        // Sharp leaf dots: bright tips, dark gaps between them.
        let leaves = smoothstep(0.38, 0.62, speck);
        tone += (leaves - 0.5) * 0.60 * fine - (1.0 - leaves) * 0.10 * fine;
    }
    let up = clamp(normalize(normal).y * 0.5 + 0.5, 0.0, 1.0);
    let warm = vec3<f32>(1.14, 1.06, 0.76);
    let cool = vec3<f32>(0.80, 0.96, 1.08);
    let lean = mix(cool, warm, up * (0.5 + clump * 0.5));
    let c = base.rgb;
    let k = mid * alive;
    let rgb = c * (1.0 + tone * alive) * mix(vec3<f32>(1.0), lean, k);
    return vec4<f32>(rgb, base.a);
}


// Burning vegetation (`vegetation.rs`, `paint`): a burning vertex keeps its
// colour and carries its ignition time in a negative alpha. Here the plant is
// eaten against the simulated clock (`tactical.y`, flaming time in
// `tactical.z`): scorch, then a glowing front that crawls through the crown
// in patches, red embers veining the char, leaves falling away to a black
// skeleton. `prog` is 0 at ignition and 1 when the plant is replaced by its
// charred remains, which the last stage already looks like.
const LEAF_OFFSET: f32 = 1.0e5;
const REMAINS_OFFSET: f32 = 2.0e5;
const SWAP_OFFSET: f32 = 4.0e5;

struct Burnt {
    rgb: vec3<f32>,
    glow: vec3<f32>,
    // 1 where the leaf has been eaten away
    gone: f32,
    hole: f32,
};

fn burn_plant(base: vec3<f32>, p: vec3<f32>, prog: f32, t: f32) -> Burnt {
    // Where the fire takes hold first is a property of the place, so patches
    // of one crown light up at different moments and the front crawls.
    let n = 0.55 * value_noise(p.xz * 0.8 + vec2<f32>(p.y * 0.5, 3.0))
          + 0.45 * value_noise(p.xz * 2.7 + vec2<f32>(-p.y * 1.9, 7.0));
    let l = prog * 1.6 - n * 0.9;

    let scorch = smoothstep(-0.35, 0.0, l);
    let hot = smoothstep(0.0, 0.07, l) * (1.0 - smoothstep(0.10, 0.36, l));
    let smoulder = smoothstep(0.08, 0.20, l) * (1.0 - smoothstep(0.30, 0.70, l));
    let charred = smoothstep(0.12, 0.40, l);
    let gone = smoothstep(0.34, 0.62, l);

    let flick = 0.70 + 0.55 * value_noise(p.xz * 2.4 + vec2<f32>(t * 5.0, t * 7.3 + p.y));
    // Embers show in veins and flecks of the char, not as a flat wash.
    let vein = smoothstep(0.50, 0.72, value_noise(p.xz * 3.6 + vec2<f32>(p.y * 2.7, 11.0)));
    let speck = smoothstep(0.78, 0.95, value_noise(p.xz * 9.0 + vec2<f32>(t * 0.6, p.y * 5.0)));

    var rgb = mix(base, base * vec3<f32>(0.80, 0.55, 0.30), scorch * 0.65);
    let ash = vec3<f32>(0.030, 0.026, 0.024) * (0.7 + 0.8 * value_noise(p.xz * 1.7 + p.y));
    rgb = mix(rgb, ash, charred);

    let hot_col = mix(vec3<f32>(3.6, 2.1, 0.45), vec3<f32>(2.5, 0.60, 0.07), smoothstep(0.0, 0.22, l));
    let ember_col = vec3<f32>(1.5, 0.20, 0.02);
    // Fades out as the plant approaches its charred remains.
    let late = 1.0 - smoothstep(0.80, 1.0, prog);
    let glow = (hot * hot_col * flick + smoulder * ember_col * (0.25 + 0.9 * vein + 1.2 * speck) * flick) * late;
    return Burnt(rgb, glow, gone, value_noise(p.xz * 3.1 + vec2<f32>(p.y * 2.3, 17.0)));
}

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

// Town (`enabled.w` = 3): plaster, roof tiles, flat-roof gravel and the
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

// Stone (`enabled.w` = 4): dressed ashlar in running bond, in the object's
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

// Boats (`enabled.w` = 5): planked hulls, panelled sails. Told apart by
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
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material,
        pbr_input.material.base_color,
    );

    let t = globals.time;
    let p = in.world_position.xyz;
    let phase = p.x * 0.013 + p.z * 0.017;
    let carrier = 0.5 + 0.5 * sin(t * 2.2 + phase);
    let scan = 0.5 + 0.5 * sin(p.y * 0.42 - t * 5.0 + phase * 0.7);
    let grain = hash21(floor(p.xz * 0.08 + t * 0.08));
    let effect = retro.enabled.x;
    let edge_strength = retro.enabled.y * effect;
    let pulse_strength = retro.enabled.z * effect;
    let pulse = (0.012 + carrier * 0.018 + scan * 0.006 + grain * 0.003) * pulse_strength;

    // Two complementary edge detectors keep this useful across the very
    // different dev meshes. Fresnel catches the visible silhouette; fwidth of
    // the interpolated normal catches hard polygon breaks on house walls,
    // roofs, tree lobes and the deliberately low-poly agent symbols.
    let facing = abs(dot(normalize(in.world_normal), normalize(pbr_input.V)));
    // Widen the rim enough to survive command-camera distance, but let the
    // material suppress it entirely for dense background geometry.
    let silhouette_edge = smoothstep(0.02, 0.42, 1.0 - facing);
    let facet_edge = clamp(length(fwidth(normalize(in.world_normal))) * 8.0, 0.0, 1.0);
    var burn_glow = vec3<f32>(0.0);
    var burn_gate = 1.0;
#ifdef VERTEX_COLORS
    // Vegetation writes charcoal colours after it burns. Use that existing
    // per-vertex state to stop dead foliage from looking electrically alive;
    // bright, actively burning vertices remain fully eligible for the pulse.
    let vertex_luma = dot(in.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    burn_gate = smoothstep(0.075, 0.13, vertex_luma);
#endif
    let edge = max(silhouette_edge, facet_edge) * edge_strength * burn_gate;
    let edge_pulse = (0.30 + carrier * 0.18 + scan * 0.08 + grain * 0.03) * edge;

    // Terrain only: the mask is sampled outside any branch (derivatives need
    // uniform control flow).
    let mask_uv = vec2<f32>(p.x, -p.z) * mask_rect.xy;
    let mask_s = textureSample(road_tex, road_samp, mask_uv);
    let meadow = textureSample(grass_tex, grass_samp, mask_uv).r;
    if retro.enabled.w > 0.5 && retro.enabled.w < 1.5 {
        pbr_input.material.base_color = terrain_albedo(pbr_input.material.base_color, p, mask_s, meadow);
    }

    if retro.enabled.w > 2.5 {
        let lp = local_metres(p, in.instance_index);
        if retro.enabled.w < 3.5 {
            pbr_input.material.base_color = town_albedo(pbr_input.material.base_color, p, in.world_normal);
        } else if retro.enabled.w < 4.5 {
            pbr_input.material.base_color = stone_albedo(pbr_input.material.base_color, lp, in.world_normal, p);
        } else {
            pbr_input.material.base_color = boat_albedo(pbr_input.material.base_color, lp, in.world_normal, p);
        }
    } else if retro.enabled.w > 1.5 {
        // Which vertices are living foliage is decided where the mesh is
        // built: vertex alpha is 1 on leaves, 0 on wood, rock, grass and
        // anything burnt (`vegetation.rs`). Below -0.5 the vertex is burning
        // and the alpha holds its ignition time.
#ifdef VERTEX_COLORS
        let a = in.color.a;
#else
        let a = 0.0;
#endif
        let px_m = max(length(dpdx(p)), length(dpdy(p)));
        let burning = a < -0.5;
        let b = -a;
        // decode: a standing vertex of a plant that has remains carries
        // SWAP_OFFSET on top of the leaf flag and the ignition time
        let swap = burning && b >= SWAP_OFFSET * 0.9;
        let b2 = b - select(0.0, SWAP_OFFSET, swap);
        let remains = burning && b2 > REMAINS_OFFSET * 0.75;
        let leaf = burning && !remains && b2 > LEAF_OFFSET * 0.5;
        let alive = select(clamp(a, 0.0, 1.0), select(0.0, 1.0, leaf), burning);
        pbr_input.material.base_color = foliage_albedo(pbr_input.material.base_color, p, in.world_normal, alive);
        if burning {
            let t0 = b2 - 1.0 - select(select(0.0, LEAF_OFFSET, leaf), REMAINS_OFFSET, remains);
            let prog = clamp((tactical.y - t0) / max(tactical.z, 1.0), 0.0, 1.0);
            let near = 1.0 - smoothstep(1.0, 3.5, px_m);
            if remains || swap {
                // Crown and burnt remains share one dissolve: the crown stays
                // where the field is above the fade, the remains below it,
                // so there is never a moment of both, nor of neither. Far
                // away the dissolve would only shimmer: it flips half way.
                let fade = smoothstep(0.10, 0.90, prog);
                let h = smoothstep(0.25, 0.75, value_noise(p.xz * 3.1 + vec2<f32>(p.y * 2.3, 17.0)));
                if near > 0.3 {
                    // strict on both sides: at 0 only the crown, at 1 only the remains
                    let done = fade > 0.999;
                    if (remains && h >= fade && !done) || (swap && (h < fade || done)) {
                        discard;
                    }
                } else if (remains && prog < 0.5) || (swap && prog >= 0.5) {
                    discard;
                }
                // The edge of the dissolve glows on both sides.
                let rim = (1.0 - smoothstep(0.0, 0.10, abs(fade - h))) * (1.0 - smoothstep(0.85, 1.0, fade));
                burn_glow = vec3<f32>(2.4, 0.55, 0.06) * rim * (0.7 + 0.6 * value_noise(p.xz * 2.4 + vec2<f32>(t * 5.0, p.y)));
                if swap {
                    // scorching browns the crown; it never goes grey, it goes away
                    let scorch = smoothstep(0.0, 0.7, prog);
                    let c = pbr_input.material.base_color.rgb;
                    pbr_input.material.base_color = vec4<f32>(mix(c, c * vec3<f32>(0.78, 0.50, 0.26), scorch), pbr_input.material.base_color.a);
                }
            } else {
            let r = burn_plant(pbr_input.material.base_color.rgb, p, prog, t);
            // Close up, an eaten leaf is a hole; far away holes would only
            // shimmer, so the same share just darkens the crown.
            if leaf && near > 0.3 && r.hole < r.gone {
                discard;
            }
            let dim = 1.0 - 0.55 * r.gone * (1.0 - near);
            pbr_input.material.base_color = vec4<f32>(r.rgb * dim, pbr_input.material.base_color.a);
            burn_glow = r.glow;
            }
        }
    }

    // A small additive lift is much more visible than changing emissive on an
    // unlit StandardMaterial, and it remains a single coherent GPU pass.
    pbr_input.material.base_color = vec4<f32>(
        pbr_input.material.base_color.rgb * (1.0 + pulse),
        pbr_input.material.base_color.a,
    );
    pbr_input.material.emissive = vec4<f32>(
        pbr_input.material.emissive.rgb + vec3<f32>(0.0, pulse * 0.18, pulse * 0.26),
        pbr_input.material.emissive.a,
    );

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    // StandardMaterial's unlit path intentionally ignores emissive. Add the
    // restrained rim after that branch so structural edges remain visible.
    out.color = vec4<f32>(
        out.color.rgb + vec3<f32>(0.0, edge_pulse * 0.65, edge_pulse),
        out.color.a,
    );
    // Fire is light, not a lit surface: added after lighting so a burning
    // crown glows on its shaded side too (and in the bloom pass).
    out.color = vec4<f32>(out.color.rgb + burn_glow, out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    // Tactical view: terrain and foliage recede toward grey so houses, units
    // and the interface carry the colour. After the fog, whose blue would
    // otherwise keep the far ground tinted.
    if retro.enabled.w > 0.5 && retro.enabled.w < 2.5 {
        let luma = dot(out.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        // x < 0 pushes the colour away from grey (more saturated)
        out.color = vec4<f32>(max(mix(out.color.rgb, vec3<f32>(luma) * 0.9, tactical.x), vec3<f32>(0.0)), out.color.a);
    }
    return out;
}
