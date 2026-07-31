//! Rendering the fire.
//!
//! The model runs on a 20 m raster; the fire is deliberately *not* drawn on
//! one. Everything here samples the CA through [`crate::field`], which
//! interpolates and noise-warps the cell fields, so the visible fire has
//! structure finer than the grid that produced it: ragged scar edges, flames
//! at sub-cell positions along the front, smoke and embers as free particles.
//!
//! Four things are drawn:
//!
//! - a **ground overlay** on a half-cell lattice, carrying the analytical
//!   layers (intensity, arrival time, spread probability);
//! - **flames**, only in the flaming band behind the front — a cell burns for
//!   20 minutes but flames for a few, and drawing the whole burning area as
//!   flame is what makes CA fire look like a spreading blob;
//! - **smoke**, which is what actually reads at commander altitude;
//! - **embers**, the mechanism that destroys houses ahead of the front.
//!
//! Flame size comes from the cell's own fireline intensity through Byram's
//! flame-length relation — the same number the exposure model uses to decide
//! what a cell can threaten.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use fire::exposure::flame_length_m;
use fire::CellFire;
use scenario::Pos;

use crate::field::{noise, FireField};
use crate::fire_shader::{FireGroundMaterial, FireMaterial};
use crate::sim::Sim;
use crate::textures;

/// Which layer the ground overlay shows. Flames, smoke and embers are always
/// drawn: the analytical layers annotate the fire, they do not replace it.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FireLayer {
    #[default]
    Flames,
    Intensity,
    Arrival,
    Hazard,
}

impl FireLayer {
    pub const ALL: [FireLayer; 4] = [
        FireLayer::Flames,
        FireLayer::Intensity,
        FireLayer::Arrival,
        FireLayer::Hazard,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FireLayer::Flames => "Flames",
            FireLayer::Intensity => "Intensity",
            FireLayer::Arrival => "Arrival",
            FireLayer::Hazard => "Spread risk",
        }
    }

    pub fn legend(self) -> &'static str {
        match self {
            FireLayer::Flames => "fresh burn → cooling → scar",
            FireLayer::Intensity => "kW/m, log scale: 10 → 10 000",
            FireLayer::Arrival => "10-minute isochrones since ignition",
            FireLayer::Hazard => "probability the front takes this ground next",
        }
    }
}

/// Overlay rebuild throttle. At high time acceleration the generation advances
/// several times per frame, and the overlay is the one mesh here big enough
/// for that to cost anything.
const OVERLAY_MIN_INTERVAL_S: f32 = 0.15;

/// Overlay samples per fire cell along each axis. 2 gives a 10 m lattice,
/// which with the domain warp is enough to hide the raster completely.
const OVERLAY_SUBDIV: usize = 2;

/// Above this many cells across, drop to one sample per cell — a fire that
/// large is being viewed from far enough away that the edge detail is lost
/// anyway, and the vertex count would not be.
const OVERLAY_SUBDIV_MAX_SPAN: usize = 280;

/// How long a cell actually *flames* after the front arrives. The model keeps
/// a cell alight for 20 minutes (`fire::BURNOUT_S`), which is residence time
/// including smouldering; the flaming zone of a real fire is a band of tens of
/// metres, not the whole burnt area.
const FLAMING_S: f32 = 400.0;

/// How long ground stays visibly incandescent after the front passes. Much
/// shorter than the burn-out window: past this it is a scar with smoke over
/// it, not a glowing surface. Keeping this tight is what stops the overlay
/// from looking like orange fog painted over the hillside.
const GLOWING_S: f32 = 420.0;

/// Caps, all of them per-frame geometry budgets rather than physical limits.
const MAX_FLAME_CELLS: usize = 3_500;
const MAX_SMOKE: usize = 800;
const MAX_EMBERS: usize = 700;

/// Shape of the firebrand's flight, mirroring `compute_spotting` in
/// `propagator-core/src/kernel.rs` — the median landing distance is linear in
/// wind speed and grows with the source cell's fireline intensity through
/// plume lofting (`d ~ U * I^(1/3)`), concentrated downwind. This is a visual
/// echo only: the core decides where the fire actually spots, this only
/// decides where the ember on screen appears to land. Kept in step with the
/// core's constants so a low-wind creeping fire visibly throws embers a few
/// metres and a wind-driven crown fire throws them across a field, matching
/// what the model is doing underneath.
const SPOT_DISTANCE_REF_M: f32 = 100.0;
const SPOT_WIND_REF_KMH: f32 = 20.0;
const SPOT_FLI_REF: f32 = 10_000.0;
const SPOT_FLI_EXPONENT: f32 = 1.0 / 3.0;
const SPOT_ANISOTROPY: f32 = 5.0;

#[derive(Component)]
pub struct FireOverlay;

#[derive(Resource)]
pub struct FireView {
    overlay: Handle<Mesh>,
    flames: Handle<Mesh>,
    smoke_mesh: Handle<Mesh>,
    sparks: Handle<Mesh>,
    last_generation: u64,
    last_layer: FireLayer,
    since_overlay: f32,
    smoke: Vec<Particle>,
    embers: Vec<Particle>,
    seed: u64,
}

/// A drifting particle: smoke puff or ember, which differ only in how they are
/// spawned, how they move and what texture they carry.
struct Particle {
    pos: Vec3,
    vel: Vec3,
    age: f32,
    life: f32,
    size: f32,
    /// Per-particle randomness, reused for rotation and shade.
    phase: f32,
    /// Set on the brief flash spawned where a firebrand lands on unburnt
    /// fuel — the visual echo of the core's own spotting model landing an
    /// ember (see `step_particles`). Ordinary embers leave this false.
    flare: bool,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fire_materials: ResMut<Assets<FireMaterial>>,
    mut ground_materials: ResMut<Assets<FireGroundMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let overlay = meshes.add(empty_mesh());
    let flames = meshes.add(empty_mesh());
    let smoke_mesh = meshes.add(empty_mesh());
    let sparks = meshes.add(empty_mesh());

    let flame_tex = images.add(textures::flame_tongue());
    let puff_tex = images.add(textures::puff());
    let spark_tex = images.add(textures::spark());

    // A shader material: the overlay's own colour already carries the glow
    // flag (red channel > 1.0), so the fragment shader only needs to animate
    // it. See `crate::fire_shader`.
    let overlay_mat = ground_materials.add(FireGroundMaterial {});
    // Shader materials, additive: flames and embers are light, not surfaces.
    // Vertex colours run above 1.0 so the camera's bloom pass catches them,
    // and the fragment shader domain-warps the texture lookup so the flame
    // edge roils instead of sitting still.
    let flame_mat = fire_materials.add(FireMaterial { texture: flame_tex });
    let spark_mat = fire_materials.add(FireMaterial { texture: spark_tex });
    // Smoke occludes: alpha blending, and unlit so a plume does not flicker
    // as it crosses the terrain's shadow line.
    let smoke_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(puff_tex),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        MaterialMeshBundle {
            mesh: overlay.clone(),
            material: overlay_mat,
            ..default()
        },
        FireOverlay,
    ));
    commands.spawn(MaterialMeshBundle {
        mesh: flames.clone(),
        material: flame_mat,
        ..default()
    });
    commands.spawn(MaterialMeshBundle {
        mesh: sparks.clone(),
        material: spark_mat,
        ..default()
    });
    commands.spawn(PbrBundle {
        mesh: smoke_mesh.clone(),
        material: smoke_mat,
        ..default()
    });

    commands.init_resource::<FireLayer>();
    commands.insert_resource(FireView {
        overlay,
        flames,
        smoke_mesh,
        sparks,
        last_generation: u64::MAX,
        last_layer: FireLayer::Flames,
        since_overlay: OVERLAY_MIN_INTERVAL_S,
        smoke: Vec::new(),
        embers: Vec::new(),
        seed: 0x5EED_1E55,
    });
}

fn empty_mesh() -> Mesh {
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    m.insert_indices(Indices::U32(Vec::new()));
    m
}

/// Number keys switch layers; the panel has buttons for the same thing.
pub fn layer_controls(keys: Res<ButtonInput<KeyCode>>, mut layer: ResMut<FireLayer>) {
    for (key, value) in [
        (KeyCode::Digit1, FireLayer::Flames),
        (KeyCode::Digit2, FireLayer::Intensity),
        (KeyCode::Digit3, FireLayer::Arrival),
        (KeyCode::Digit4, FireLayer::Hazard),
    ] {
        if keys.just_pressed(key) && *layer != value {
            *layer = value;
        }
    }
}

/// Clear the drifting particles when the sim restarts.
///
/// The overlay and the flame billboards are rebuilt from the fire state every
/// time it goes stale, so they need no help — the generation bump does it. Smoke
/// and embers are the exception: they are *simulated here*, carrying their own
/// position and age, and a plume left over from the old fire would go on
/// drifting over a landscape that never burnt.
pub fn reset(mut restarted: EventReader<crate::sim::SimRestarted>, mut view: ResMut<FireView>) {
    if restarted.is_empty() {
        return;
    }
    restarted.clear();
    view.smoke.clear();
    view.embers.clear();
}

// --- ground overlay --------------------------------------------------------

pub fn update_overlay(
    sim: Res<Sim>,
    layer: Res<FireLayer>,
    time: Res<Time>,
    mut view: ResMut<FireView>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    view.since_overlay += time.delta_seconds();
    let layer = *layer;
    let stale = sim.generation != view.last_generation || layer != view.last_layer;
    if !stale || view.since_overlay < OVERLAY_MIN_INTERVAL_S {
        return;
    }
    view.last_generation = sim.generation;
    view.last_layer = layer;
    view.since_overlay = 0.0;

    let scn = &sim.scenario;
    let w = scn.world;
    let hazard = sim.fire.hazard().as_slice();
    let field = FireField {
        state: sim.fire.state(),
        arrival: sim.fire.arrival_times(),
        intensity: sim.fire.intensity(),
        world: w,
    };

    // Work over the touched region only: the overlay is a lattice, but it is a
    // lattice over the fire, not over Liguria.
    let Some((r0, r1, c0, c1)) = touched_bounds(&sim, hazard, layer) else {
        meshes.insert(&view.overlay, empty_mesh());
        return;
    };
    let span = (r1 - r0).max(c1 - c0);
    let subdiv = if span > OVERLAY_SUBDIV_MAX_SPAN {
        1
    } else {
        OVERLAY_SUBDIV
    };
    let step = w.cellsize / subdiv as f32;

    let nx = (c1 - c0) * subdiv + 1;
    let ny = (r1 - r0) * subdiv + 1;
    let now = sim.time_s() as f32;

    let mut mesh = QuadBuilder::default();
    // Vertex lattice first, then only the quads that carry any coverage.
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(nx * ny);
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(nx * ny);

    for iy in 0..ny {
        for ix in 0..nx {
            let p = Pos {
                x: (c0 * subdiv + ix) as f32 * step,
                y: w.height_m - (r0 * subdiv + iy) as f32 * step,
            };
            let warped = field.warp(p);
            let color = sample_color(layer, &field, hazard, warped, now, scn.vr_palette());
            // Lift the whole overlay slightly and let hot ground sit higher,
            // so a flaming edge is never buried under the scar beside it.
            let lift = 0.5 + 1.2 * color[3].min(1.0) * f32::from(color[0] > 1.0);
            positions.push([p.x, scn.terrain.height_at(p) + lift, -p.y]);
            colors.push(color);
        }
    }

    for iy in 0..ny.saturating_sub(1) {
        for ix in 0..nx.saturating_sub(1) {
            let quad = [
                iy * nx + ix,
                iy * nx + ix + 1,
                (iy + 1) * nx + ix + 1,
                (iy + 1) * nx + ix,
            ];
            if quad.iter().all(|&i| colors[i][3] < 0.02) {
                continue;
            }
            let base = mesh.positions.len() as u32;
            for i in quad {
                mesh.positions.push(positions[i]);
                mesh.normals.push([0.0, 1.0, 0.0]);
                mesh.colors.push(colors[i]);
                mesh.uvs.push([0.5, 0.5]);
            }
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    meshes.insert(&view.overlay, mesh.finish());
}

/// Row/col bounds of everything the overlay might draw, padded by one cell so
/// the feathered edge has somewhere to fade into.
fn touched_bounds(
    sim: &Sim,
    hazard: &[f32],
    layer: FireLayer,
) -> Option<(usize, usize, usize, usize)> {
    let w = sim.scenario.world;
    let (mut r0, mut r1, mut c0, mut c1) = (usize::MAX, 0usize, usize::MAX, 0usize);
    for (i, state) in sim.fire.state().iter().enumerate() {
        let touched =
            *state != CellFire::Unburnt || (layer == FireLayer::Hazard && hazard[i] > 0.01);
        if !touched {
            continue;
        }
        let (r, c) = (i / w.fire_cols, i % w.fire_cols);
        r0 = r0.min(r);
        r1 = r1.max(r);
        c0 = c0.min(c);
        c1 = c1.max(c);
    }
    if r0 == usize::MAX {
        return None;
    }
    Some((
        r0.saturating_sub(1),
        (r1 + 2).min(w.fire_rows - 1),
        c0.saturating_sub(1),
        (c1 + 2).min(w.fire_cols - 1),
    ))
}

/// Colour and alpha of the overlay at one world point, per layer.
/// Recolours a ramp output into a VR-training palette while preserving its
/// luma — so intensity bands, isochrone contours and hazard gradients stay
/// exactly as legible, just in the scenario's void/accent hues instead of
/// realistic fire colour. `None` for the realistic scenarios.
fn stylize(c: [f32; 3], pal: Option<scenario::VrPalette>) -> [f32; 3] {
    let Some(pal) = pal else { return c };
    let luma = (0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]).clamp(0.0, 1.0);
    [
        pal.void[0] + (pal.accent[0] - pal.void[0]) * luma,
        pal.void[1] + (pal.accent[1] - pal.void[1]) * luma,
        pal.void[2] + (pal.accent[2] - pal.void[2]) * luma,
    ]
}

fn sample_color(
    layer: FireLayer,
    field: &FireField,
    hazard: &[f32],
    p: Pos,
    now: f32,
    pal: Option<scenario::VrPalette>,
) -> [f32; 4] {
    // Coverage feathered around the half-burnt contour: this is what turns a
    // staircase of cell edges into a burn perimeter. The band is narrow —
    // widen it and the perimeter stops being a perimeter and becomes a haze.
    let cover = ((field.burnt_fraction(p) - 0.42) / 0.20).clamp(0.0, 1.0);
    if cover <= 0.0 && layer != FireLayer::Hazard {
        return [0.0, 0.0, 0.0, 0.0];
    }

    let [r, g, b, a] = match layer {
        FireLayer::Flames => {
            let age = field.arrival(p).map(|t| now - t).unwrap_or(0.0).max(0.0);
            // Incandescent only just behind the front, then straight to scar.
            // The glow is squared twice so the bright band is genuinely a
            // band, not a gradient across the whole burn.
            let glow = (1.0 - age / GLOWING_S).clamp(0.0, 1.0).powi(3);
            let embers = (1.0 - age / (GLOWING_S * 4.0)).clamp(0.0, 1.0);
            // Char and ash, at a scale the CA has no opinion about: a burn is
            // never one flat tone, and without this the scar reads as paint.
            let ash = 0.72
                + 0.85
                    * (0.6 * noise(p.x / 23.0, p.y / 23.0, 0x77)
                        + 0.4 * noise(p.x / 8.0, p.y / 8.0, 0x88));
            [
                (0.085 + 0.10 * embers) * ash + 2.4 * glow,
                0.065 * ash + 0.70 * glow * glow,
                0.060 * ash + 0.06 * glow,
                cover * 0.95,
            ]
        }
        FireLayer::Intensity => {
            let c = intensity_color(field.intensity(p));
            [c[0], c[1], c[2], cover * 0.9]
        }
        FireLayer::Arrival => match field.arrival(p) {
            None => [0.0, 0.0, 0.0, 0.0],
            Some(t) => {
                let c = arrival_color(t, now);
                [c[0], c[1], c[2], cover * 0.9]
            }
        },
        FireLayer::Hazard => {
            let scar = cover;
            let risk = bilinear_hazard(field, hazard, p) * (1.0 - scar);
            if risk > 0.01 {
                let c = hazard_color(risk);
                [c[0], c[1], c[2], (0.25 + 0.6 * risk.sqrt()).min(0.9)]
            } else if scar > 0.0 {
                // The scar stays, dimmed: risk ahead of the front is only
                // legible against where the fire has already been.
                [0.10, 0.09, 0.09, scar * 0.7]
            } else {
                [0.0, 0.0, 0.0, 0.0]
            }
        }
    };
    let [r, g, b] = stylize([r, g, b], pal);
    [r, g, b, a]
}

/// The hazard field shares the fire grid, so it interpolates the same way.
fn bilinear_hazard(field: &FireField, hazard: &[f32], p: Pos) -> f32 {
    let w = field.world;
    let fx = (p.x / w.cellsize - 0.5).clamp(0.0, (w.fire_cols - 1) as f32);
    let fy = ((w.height_m - p.y) / w.cellsize - 0.5).clamp(0.0, (w.fire_rows - 1) as f32);
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let x1 = (x0 + 1).min(w.fire_cols - 1);
    let y1 = (y0 + 1).min(w.fire_rows - 1);
    let (tx, ty) = (fx.fract(), fy.fract());
    let v = |r: usize, c: usize| hazard[r * w.fire_cols + c];
    let top = v(y0, x0) * (1.0 - tx) + v(y0, x1) * tx;
    let bot = v(y1, x0) * (1.0 - tx) + v(y1, x1) * tx;
    top * (1.0 - ty) + bot * ty
}

/// Log ramp over 10 .. 10 000 kW/m: the decade is what changes the tactics.
/// Roughly the standard suppression bands — direct attack (green/yellow),
/// heavy equipment only (orange), nothing works (white-hot).
fn intensity_color(fli: f32) -> [f32; 3] {
    let t = ((fli.max(10.0).log10() - 1.0) / 3.0).clamp(0.0, 1.0);
    ramp(
        t,
        &[
            (0.00, [0.15, 0.35, 0.30]),
            (0.35, [0.85, 0.85, 0.25]),
            (0.60, [0.95, 0.50, 0.10]),
            (0.80, [0.90, 0.15, 0.10]),
            (1.00, [1.00, 0.95, 0.90]),
        ],
    )
}

/// 10-minute isochrones.
///
/// The spacing between bands *is* the rate of spread, so the bands have to be
/// countable: each gets its own colour from a repeating six-step ramp, with a
/// dark line drawn where one band meets the next. A smooth ramp over elapsed
/// time — the obvious implementation — is unreadable, because a fire that has
/// been running 25 minutes covers a quarter of it.
fn arrival_color(arrival_s: f32, now_s: f32) -> [f32; 3] {
    const BAND_S: f32 = 600.0;
    let _ = now_s;
    let bands = (arrival_s.max(0.0) / BAND_S).max(0.0);
    let index = bands.floor() as usize;
    let within = bands.fract();

    // Six steps, then the cycle repeats: an hour of fire per revolution.
    let palette = [
        [0.85, 0.16, 0.10],
        [0.96, 0.55, 0.12],
        [0.95, 0.85, 0.25],
        [0.45, 0.75, 0.35],
        [0.20, 0.62, 0.70],
        [0.35, 0.36, 0.72],
    ];
    let c = palette[index % palette.len()];

    // Dark contour in the last tenth of each band: the isochrone itself.
    let edge = if within > 0.9 { 0.45 } else { 1.0 };
    [c[0] * edge, c[1] * edge, c[2] * edge]
}

fn hazard_color(p: f32) -> [f32; 3] {
    ramp(
        p.clamp(0.0, 1.0).sqrt(),
        &[
            (0.00, [0.25, 0.45, 0.85]),
            (0.45, [0.95, 0.90, 0.30]),
            (1.00, [1.00, 0.20, 0.10]),
        ],
    )
}

/// Piecewise-linear colour ramp over sorted stops.
fn ramp(t: f32, stops: &[(f32, [f32; 3])]) -> [f32; 3] {
    let t = t.clamp(stops[0].0, stops[stops.len() - 1].0);
    for pair in stops.windows(2) {
        let (t0, c0) = pair[0];
        let (t1, c1) = pair[1];
        if t <= t1 {
            let k = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
            return [
                c0[0] + (c1[0] - c0[0]) * k,
                c0[1] + (c1[1] - c0[1]) * k,
                c0[2] + (c1[2] - c0[2]) * k,
            ];
        }
    }
    stops[stops.len() - 1].1
}

#[derive(Default)]
struct QuadBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl QuadBuilder {
    /// A textured billboard. `right`/`up` are world-space half-extents;
    /// `taper` narrows the top edge, which is what gives a flame tongue its
    /// shape independently of the texture.
    fn billboard(
        &mut self,
        centre: Vec3,
        right: Vec3,
        up: Vec3,
        taper: f32,
        bottom: [f32; 4],
        top: [f32; 4],
    ) {
        let base = self.positions.len() as u32;
        for (offset, color, uv) in [
            (centre - right - up, bottom, [0.0, 1.0]),
            (centre + right - up, bottom, [1.0, 1.0]),
            (centre + right * taper + up, top, [1.0, 0.0]),
            (centre - right * taper + up, top, [0.0, 0.0]),
        ] {
            self.positions.push(offset.into());
            self.normals.push([0.0, 1.0, 0.0]);
            self.colors.push(color);
            self.uvs.push(uv);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn finish(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

// --- flames, smoke, embers -------------------------------------------------

/// Rebuild the particle geometry. Runs every frame: the fire has to move even
/// when the sim is paused between steps or running at 1x.
pub fn update_flames(
    sim: Res<Sim>,
    time: Res<Time>,
    camera: Query<&Transform, With<Camera3d>>,
    mut view: ResMut<FireView>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Ok(cam) = camera.get_single() else {
        return;
    };
    let scn = &sim.scenario;
    let t = time.elapsed_seconds();
    let dt = time.delta_seconds().min(0.1);
    let now = sim.time_s() as f32;

    // Billboard axes: horizontal-facing, so flames stay upright rather than
    // tipping over when the camera looks down at the ground.
    let mut right = cam.rotation * Vec3::X;
    right.y = 0.0;
    let right = right.normalize_or_zero();
    let up = Vec3::Y;

    let field = FireField {
        state: sim.fire.state(),
        arrival: sim.fire.arrival_times(),
        intensity: sim.fire.intensity(),
        world: scn.world,
    };

    let active = sim.fire.active_cells();
    let stride = (active.len() / MAX_FLAME_CELLS).max(1);
    let mut flames = QuadBuilder::default();

    for cell in active.iter().step_by(stride) {
        let fli = sim.fire.cell_intensity(*cell);
        let arrival = sim.fire.arrival_time(*cell).unwrap_or(now as i32) as f32;
        // The flaming band: only cells the front has just crossed carry flame.
        let flaming = (1.0 - (now - arrival) / FLAMING_S).clamp(0.0, 1.0);
        if flaming < 0.05 {
            continue;
        }

        // Byram flame length, with a floor: a metre-high creeping flame is
        // physically right and visually nothing, and the player still has to
        // be able to see where the fire is.
        let flame_m = flame_length_m(fli).clamp(1.5, 45.0) * (0.45 + 0.55 * flaming);
        // A hot cell carries several tongues, a creeping one carries a single
        // flicker. Sub-cell placement is what stops them lining up on a grid.
        let tongues = (1.0 + (fli / 700.0).min(4.0) * flaming).round() as usize;
        let centre = scn.world.centre_of(*cell);
        let half = scn.world.cellsize * 0.5;

        for k in 0..tongues {
            let h = hash01(cell.row as u64 * 7919 + cell.col as u64 * 104_729 + k as u64 * 31);
            let h2 = hash01(cell.row as u64 * 104_729 + cell.col as u64 * 7919 + k as u64 * 97);
            let p = Pos {
                x: centre.x + (h - 0.5) * 2.0 * half,
                y: centre.y + (h2 - 0.5) * 2.0 * half,
            };
            let ground = scn.terrain.height_at(p);

            let phase = h * 30.0 + k as f32 * 2.3;
            let flicker = 0.62 + 0.38 * (t * 5.5 + phase).sin() * (t * 2.3 + phase).cos();
            // Local intensity, not the cell's: the interpolated field varies
            // across the cell, so tongues differ within one cell too.
            let local = field.intensity(p).max(fli * 0.4);
            let h_m = flame_m * flicker * (0.6 + 0.4 * (local / fli.max(1.0)).min(1.5));
            let half_w = (h_m * 0.42).min(scn.world.cellsize * 0.5);
            let sway = (t * 2.6 + phase).sin() * h_m * 0.16;

            // Additive light competing with a daylit hillside: the multiplier
            // has to be well above 1 or the flame is a pale smudge.
            let heat = (local / 2000.0).clamp(0.55, 2.2) * (0.6 + 0.4 * flaming);
            flames.billboard(
                Vec3::new(p.x, ground + h_m * 0.5, -p.y) + right * sway,
                right * half_w,
                up * h_m * 0.5,
                0.55,
                [3.6 * heat, 1.55 * heat, 0.28 * heat, 1.0],
                [2.0 * heat, 0.50 * heat, 0.07 * heat, 1.0],
            );
        }
    }

    step_particles(&mut view, &sim, dt, now);

    let mut smoke = QuadBuilder::default();
    for puff in &view.smoke {
        let k = (puff.age / puff.life).clamp(0.0, 1.0);
        // Grows as it disperses, darkest and densest near the fire.
        let size = puff.size * (0.6 + 1.9 * k);
        let shade = 0.16 + 0.42 * k;
        // Fade in fast, out slowly: a puff should never pop into existence.
        // Kept thin — hundreds of puffs overlap, and at 0.55 each the plume
        // turned into a white wall as soon as the camera dropped into it.
        let alpha = (k * 6.0).min(1.0) * (1.0 - k).powf(1.4) * 0.32;
        let spin = puff.phase * 6.28 + k * 0.6;
        let (s, c) = spin.sin_cos();
        let r = (right * c + up * s) * size;
        let u = (up * c - right * s) * size;
        smoke.billboard(
            puff.pos,
            r,
            u,
            1.0,
            [shade, shade * 0.94, shade * 0.88, alpha],
            [shade, shade * 0.94, shade * 0.88, alpha],
        );
    }

    let mut sparks = QuadBuilder::default();
    for ember in &view.embers {
        let k = (ember.age / ember.life).clamp(0.0, 1.0);
        if ember.flare {
            // A quick, bright pop where a firebrand has just landed on
            // unburnt fuel — sharp attack, quick decay, not a fade like an
            // ordinary ember: this is a spot-ignition risk lighting up, not
            // embers cooling.
            let pulse = (1.0 - k).powf(0.5) * (1.0 - (k * 3.0 - 1.0).max(0.0));
            let size = ember.size * (1.0 + 1.5 * (1.0 - k));
            let c = [4.0 * pulse, 2.2 * pulse, 0.9 * pulse, 1.0];
            sparks.billboard(ember.pos, right * size, up * size, 1.0, c, c);
            continue;
        }
        let fade = 1.0 - k;
        let size = ember.size * (0.6 + 0.6 * fade);
        let c = [2.6 * fade, 0.65 * fade, 0.10 * fade, 1.0];
        sparks.billboard(ember.pos, right * size, up * size, 1.0, c, c);
    }

    meshes.insert(&view.flames, flames.finish());
    meshes.insert(&view.smoke_mesh, smoke.finish());
    meshes.insert(&view.sparks, sparks.finish());
}

/// Advance smoke and embers, and top both up from the active front.
fn step_particles(view: &mut FireView, sim: &Sim, dt: f32, now: f32) {
    let scn = &sim.scenario;
    let weather = sim.fire.weather();
    // `wind_dir_deg` is the bearing the wind blows *from*, so drift is the
    // opposite bearing. (Bevy -Z is north.)
    let from = (weather.wind_dir_deg as f32).to_radians();
    let wind_ms = weather.wind_speed_kmh as f32 / 3.6;
    let drift = Vec3::new(-from.sin(), 0.0, from.cos()) * wind_ms;

    for puff in &mut view.smoke {
        puff.age += dt;
        // Buoyancy dies off as the plume cools and entrains air, after which
        // the puff simply goes where the wind goes.
        let k = (puff.age / puff.life).clamp(0.0, 1.0);
        let buoyancy = Vec3::Y * (7.0 * (1.0 - k) * (1.0 - k));
        let turbulence = Vec3::new(
            noise(puff.pos.x * 0.01, puff.age * 0.4 + puff.phase, 3) - 0.5,
            0.0,
            noise(puff.pos.z * 0.01, puff.age * 0.4 + puff.phase, 5) - 0.5,
        ) * wind_ms
            * 0.6;
        puff.vel = puff.vel.lerp(drift + buoyancy + turbulence, dt * 1.2);
        puff.pos += puff.vel * dt;
    }
    view.smoke.retain(|p| p.age < p.life);

    for ember in &mut view.embers {
        ember.age += dt;
        if ember.flare {
            continue;
        }
        // A real firebrand's horizontal speed barely changes in flight — the
        // wind carries it — so only gravity acts on the vertical component;
        // the horizontal speed was chosen at launch to cover the modelled
        // spotting distance in the modelled travel time (see the spawn
        // site), and damping it away early would strand it short.
        ember.vel.y -= 9.0 * dt;
        ember.pos += ember.vel * dt;
    }
    // A firebrand that reaches ground level on unburnt, burnable fuel is
    // exactly what the core's own spotting model is throwing ahead of the
    // front — flash it there instead of silently despawning it like a spent,
    // harmless spark.
    let mut flares = Vec::new();
    for e in &view.embers {
        if e.flare {
            continue;
        }
        let ground = scn.terrain.height_at(Pos {
            x: e.pos.x,
            y: -e.pos.z,
        });
        let dying = e.age >= e.life || e.pos.y <= ground + 0.4;
        if !dying {
            continue;
        }
        let landing = Pos {
            x: e.pos.x,
            y: -e.pos.z,
        };
        if !scn.world.contains(landing) {
            continue;
        }
        let cell = scn.world.cell_of(landing);
        if scn.is_burnable(cell) && sim.fire.cell_state(cell) == CellFire::Unburnt {
            flares.push(Particle {
                pos: Vec3::new(e.pos.x, ground + 0.6, e.pos.z),
                vel: Vec3::ZERO,
                age: 0.0,
                life: 1.1,
                size: 2.6,
                phase: e.phase,
                flare: true,
            });
        }
    }
    view.embers.retain(|e| {
        e.age < e.life
            && (e.flare
                || e.pos.y
                    > scn.terrain.height_at(Pos {
                        x: e.pos.x,
                        y: -e.pos.z,
                    }) + 0.4)
    });
    view.embers.extend(flares);

    let active = sim.fire.active_cells();
    if active.is_empty() {
        return;
    }

    // Spawn budgets scale with the front but are capped: a 20 000-cell fire
    // does not get 20 000 plumes, it gets a full sky.
    let smoke_budget =
        ((active.len() as f32 * dt * 1.2) as usize).min(MAX_SMOKE.saturating_sub(view.smoke.len()));
    let ember_budget = ((active.len() as f32 * dt * 0.5) as usize)
        .min(MAX_EMBERS.saturating_sub(view.embers.len()));

    for i in 0..smoke_budget + ember_budget {
        view.seed = view.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let r = (view.seed >> 33) as usize;
        let cell = active[r % active.len()];
        let fli = sim.fire.cell_intensity(cell);
        let arrival = sim.fire.arrival_time(cell).unwrap_or(now as i32) as f32;
        let flaming = (1.0 - (now - arrival) / (FLAMING_S * 3.0)).clamp(0.0, 1.0);

        let centre = scn.world.centre_of(cell);
        let half = scn.world.cellsize * 0.5;
        let jx = hash01(view.seed) - 0.5;
        let jy = hash01(view.seed ^ 0xABCD) - 0.5;
        let p = Pos {
            x: centre.x + jx * 2.0 * half,
            y: centre.y + jy * 2.0 * half,
        };
        let ground = scn.terrain.height_at(p);
        let phase = hash01(view.seed ^ 0x1234);

        if i < smoke_budget {
            // Smouldering ground smokes too — often more visibly than flame.
            let plume = (fli / 1200.0).clamp(0.3, 3.0);
            view.smoke.push(Particle {
                pos: Vec3::new(p.x, ground + 4.0 + 10.0 * flaming, -p.y),
                vel: Vec3::Y * (4.0 + 8.0 * plume * flaming) + drift * 0.3,
                age: 0.0,
                life: 22.0 + phase * 20.0,
                size: 12.0 + 16.0 * plume,
                phase,
                flare: false,
            });
        } else {
            // Only a cell with a real plume lofts embers.
            if fli < 400.0 {
                continue;
            }
            // Launch azimuth measured *from* the downwind direction, so
            // `alignment = cos(offset)` is 1 directly downwind — the same
            // quantity `compute_spotting` calls `(w_dir - angle).cos()`.
            let offset = phase * std::f32::consts::TAU;
            let wind_hat = if wind_ms > 0.05 {
                drift / wind_ms
            } else {
                Vec3::Z
            };
            let perp = Vec3::new(-wind_hat.z, 0.0, wind_hat.x);
            let (s, c) = offset.sin_cos();
            let launch_dir = wind_hat * c + perp * s;
            let alignment = c;

            // Median landing distance: linear in wind, ~intensity^(1/3) via
            // plume lofting, concentrated downwind. Jittered log-normally
            // about that median, same as the core.
            let d_median = SPOT_DISTANCE_REF_M
                * (weather.wind_speed_kmh as f32 / SPOT_WIND_REF_KMH).max(0.0)
                * (fli / SPOT_FLI_REF).max(0.001).powf(SPOT_FLI_EXPONENT);
            let directional = (SPOT_ANISOTROPY * (alignment - 1.0)).exp();
            let jitter = 0.55 + hash01(view.seed ^ 0x9E37) * 0.9;
            // Capped at the same 400 m the threat field uses for ember reach
            // (`ThreatField`, `fire::threat`) — the distance a real crown
            // fire throws brands can run past a kilometre, but this is a
            // particle the player watches fly, not a hazard sample.
            let distance = (d_median * directional * jitter).max(2.0).min(400.0);

            let transport_speed = (wind_ms * alignment.max(0.15)).max(0.5);
            let travel_time = (distance / transport_speed).clamp(2.0, 12.0);

            // Ballistic vertical launch: apex at travel_time / 2, back to
            // spawn height at travel_time, so a long throw arcs high and a
            // short one barely lifts off — the loft the core's model
            // attributes to plume convection (`H ~ I^(2/3)`) made visible.
            const GRAVITY: f32 = 9.0;
            let v_y0 = 0.5 * GRAVITY * travel_time;
            view.embers.push(Particle {
                pos: Vec3::new(p.x, ground + 3.0, -p.y),
                vel: launch_dir * (distance / travel_time) + Vec3::Y * v_y0,
                age: 0.0,
                life: travel_time,
                size: 1.6 + phase * 1.6,
                phase,
                flare: false,
            });
        }
    }
}

/// Hash to `[0, 1)`, for per-instance jitter that must be stable frame to
/// frame (a tongue that teleports every frame reads as noise, not fire).
fn hash01(x: u64) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h >> 40) as f32 / (1u32 << 24) as f32
}
