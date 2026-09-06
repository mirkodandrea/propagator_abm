//! Placing ignitions by hand.
//!
//! Two things this has to get right, both of them consequences of findings the
//! model already paid for:
//!
//! **A placed ignition must be a patch, not a point.** A single lit cell fails
//! to establish about a fifth of the time at `realizations=1` (see
//! [`fire::FireSim::ignite_patch`]). Clicking the map and getting nothing, one
//! time in five, would read as a broken control rather than as physics, so the
//! radius is a first-class part of the gesture and is floored at
//! [`crate::sim::MIN_IGNITION_RADIUS_M`].
//!
//! **The player has to be shown where a fire can actually start.** Buildings,
//! roads and rock sit on non-burnable fuel, and the town is *mostly* those, so
//! a large share of the map is simply not ignitable. The cursor ring turns red
//! there instead of letting the click fail with an error in the log.
//!
//! The rings are drawn as annuli draped on the 5 m render field rather than as
//! flat discs: a disc at this scale is either buried in a hillside or floating
//! off it, and a filled shape hides the fire it is marking.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use scenario::{Cell, Pos, Scenario};

use crate::camera::OrbitCamera;
use crate::retro;
use crate::retro::RetroMaterial;
use crate::pick;
use crate::sim::{Sim, MAX_IGNITION_RADIUS_M, MIN_IGNITION_RADIUS_M};

/// Ring thickness in metres. Scaled with the radius so a 600 m ignition does
/// not read as a hairline and a 60 m one does not read as a blob.
fn ring_width(radius_m: f32) -> f32 {
    (radius_m * 0.06).clamp(4.0, 20.0)
}

/// Segments around a ring. Fixed rather than radius-dependent: even the
/// smallest ring is 60 m across on screen, where faceting would show.
const RING_SEGMENTS: usize = 96;

/// Lift above the ground.
///
/// Not a z-fighting margin like the road ribbons' half-metre: a ring on the
/// ground is *under the canopy*, and the plants are 5–15 m tall, so at 2 m it
/// is drawn perfectly and seen never. This puts it in the same map-symbol layer
/// as the household beacons (+22 m) and the refuge markers (+30 m) — high
/// enough to clear the vegetation, still draped so it reads as lying on the
/// hillside rather than hovering over it.
pub(crate) const RING_LIFT_M: f32 = 20.0;

/// What a left-click currently means.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum EditMode {
    /// Left-drag orbits the camera, as normal.
    #[default]
    Off,
    /// Left-click lights a fire. Orbiting still works on right-drag, so the
    /// player is never trapped in a view they cannot adjust.
    Place,
}

#[derive(Resource)]
pub struct IgnitionTool {
    pub mode: EditMode,
    pub radius_m: f32,
    /// Where the cursor is on the ground, and whether a fire could start
    /// there. Recomputed each frame while placing; `None` off-map or at the sky.
    pub hover: Option<(Pos, bool)>,
}

impl Default for IgnitionTool {
    fn default() -> Self {
        IgnitionTool {
            // SPOTORNO_PLACE=1 opens armed, so a scripted screenshot can show
            // the placement rings without a hand on the mouse.
            mode: if std::env::var("SPOTORNO_PLACE").is_ok() {
                EditMode::Place
            } else {
                EditMode::Off
            },
            // Opens at the shipped scenario's own starting radius, which is the
            // size measured to make a scenario rather than a curiosity.
            radius_m: crate::sim::START_RADIUS_M,
            hover: None,
        }
    }
}

/// Marks a ring drawn for an already-placed ignition.
#[derive(Component)]
pub struct IgnitionMarker;

/// Marks the single ring that follows the cursor.
#[derive(Component)]
pub struct HoverRing;

#[derive(Resource)]
pub struct RingAssets {
    placed: Handle<RetroMaterial>,
    /// Cursor ring where a fire can start, and where it cannot.
    ok: Handle<RetroMaterial>,
    blocked: Handle<RetroMaterial>,
    /// Rings already given an entity, so placing one more is not a rescan.
    drawn: usize,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RetroMaterial>>,
) {
    // Unlit and emissive: these are map symbols, and a symbol must not change
    // value because it crossed into a hillside's shadow.
    let mut ring = |r: f32, g: f32, b: f32, a: f32| {
        materials.add(retro::material(StandardMaterial {
            base_color: Color::srgba(r, g, b, a),
            emissive: LinearRgba::rgb(r * 1.6, g * 1.6, b * 1.6),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        }, true))
    };
    let placed = ring(1.0, 0.45, 0.10, 0.85);
    let ok = ring(1.0, 0.92, 0.35, 0.80);
    let blocked = ring(0.95, 0.15, 0.15, 0.65);

    // The hover ring exists from the start and is hidden when not placing:
    // spawning and despawning it per frame would fight the asset system for no
    // gain.
    commands.spawn((
        MaterialMeshBundle::<RetroMaterial> {
            mesh: meshes.add(empty_mesh()),
            material: ok.clone(),
            visibility: Visibility::Hidden,
            ..default()
        },
        HoverRing,
    ));

    commands.insert_resource(RingAssets {
        placed,
        ok,
        blocked,
        drawn: 0,
    });
}

/// Track the ground point under the cursor, and whether it is ignitable.
pub fn hover(
    sim: Res<Sim>,
    ui_focus: Res<crate::ui::UiFocus>,
    mut tool: ResMut<IgnitionTool>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<OrbitCamera>>,
) {
    if tool.mode != EditMode::Place || ui_focus.pointer {
        if tool.hover.is_some() {
            tool.hover = None;
        }
        return;
    }
    let (Ok(window), Ok((camera, cam_tf))) = (windows.get_single(), camera.get_single()) else {
        return;
    };
    tool.hover = pick::cursor_ground(&sim.scenario, camera, cam_tf, window).map(|p| {
        (
            p,
            ignitable(&sim.scenario, sim.scenario.world.cell_of(p), tool.radius_m),
        )
    });
}

/// Light a fire where the player clicked.
pub fn place(
    mut sim: ResMut<Sim>,
    tool: Res<IgnitionTool>,
    buttons: Res<ButtonInput<MouseButton>>,
) {
    if tool.mode != EditMode::Place || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some((p, ok)) = tool.hover else { return };
    if !ok {
        // Silent: the ring is already red, and a log line per rejected click
        // would bury the ones that mattered.
        return;
    }
    let cell = sim.scenario.world.cell_of(p);
    let radius = tool.radius_m;
    if let Err(e) = sim.add_ignition(cell, radius) {
        warn!("ignition at ({}, {}) rejected: {e:#}", cell.row, cell.col);
    }
}

/// Is there any burnable fuel within `radius_m` of `centre`?
///
/// The same test [`fire::FireSim::ignite_patch`] applies, run ahead of the
/// click so a refusal is visible in the cursor rather than only in the log.
fn ignitable(scn: &Scenario, centre: Cell, radius_m: f32) -> bool {
    let r = (radius_m / scn.world.cellsize).ceil() as i64;
    for dr in -r..=r {
        for dc in -r..=r {
            if (dr * dr + dc * dc) as f32 > (r * r) as f32 {
                continue;
            }
            let (row, col) = (centre.row as i64 + dr, centre.col as i64 + dc);
            if row < 0 || col < 0 {
                continue;
            }
            let c = Cell {
                row: row as usize,
                col: col as usize,
            };
            if c.row < scn.world.fire_rows && c.col < scn.world.fire_cols && scn.is_burnable(c) {
                return true;
            }
        }
    }
    false
}

/// Redraw the cursor ring.
pub fn update_hover(
    sim: Res<Sim>,
    tool: Res<IgnitionTool>,
    assets: Res<RingAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<
        (
            &Handle<Mesh>,
            &mut Visibility,
            &mut Handle<RetroMaterial>,
        ),
        With<HoverRing>,
    >,
) {
    let Ok((mesh, mut vis, mut mat)) = query.get_single_mut() else {
        return;
    };
    let Some((p, ok)) = tool.hover else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    };
    if *vis != Visibility::Inherited {
        *vis = Visibility::Inherited;
    }

    // Rebuilt in world space every frame rather than built once and moved by a
    // transform: the whole reason it reads as attached to the hillside is that
    // its vertices are draped on the terrain, and a translation cannot do that.
    // One ring of 96 segments is 194 bilinear lookups — free next to the 15 M
    // triangles of vegetation this is drawn over.
    if let Some(m) = meshes.get_mut(mesh) {
        *m = ring_mesh(&sim.scenario, p, tool.radius_m);
    }

    let want = if ok { &assets.ok } else { &assets.blocked };
    if *mat != *want {
        *mat = want.clone();
    }
}

/// Give a ring to every ignition that does not have one, and drop the lot when
/// the sim restarts and the indices no longer mean anything.
pub fn sync_markers(
    mut commands: Commands,
    sim: Res<Sim>,
    mut assets: ResMut<RingAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut restarted: EventReader<crate::sim::SimRestarted>,
    existing: Query<Entity, With<IgnitionMarker>>,
) {
    // A restart can *remove* ignitions as well as replay them ("Forget added",
    // "Replan for wind"), so the rings are rebuilt wholesale rather than
    // reconciled — there are never more than a handful.
    if !restarted.is_empty() {
        restarted.clear();
        for e in &existing {
            commands.entity(e).despawn();
        }
        assets.drawn = 0;
    }
    if assets.drawn == sim.ignitions.len() {
        return;
    }
    for ig in sim.ignitions.iter().skip(assets.drawn) {
        let p = sim.scenario.world.centre_of(ig.centre);
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(ring_mesh(&sim.scenario, p, ig.radius_m)),
                material: assets.placed.clone(),
                // Shown only while placing: see `show_markers`.
                visibility: Visibility::Hidden,
                ..default()
            },
            IgnitionMarker,
        ));
    }
    assets.drawn = sim.ignitions.len();
}

/// Show ignition rings while editing and during the initial paused briefing.
/// Before the first simulation tick the flame layer has no visible front yet.
pub fn show_markers(
    sim: Res<Sim>,
    tool: Res<IgnitionTool>,
    mut markers: Query<&mut Visibility, With<IgnitionMarker>>,
) {
    let want = if tool.mode == EditMode::Place || sim.time_s() == 0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut vis in &mut markers {
        if *vis != want {
            *vis = want;
        }
    }
}

/// An annulus in world space, draped on the render terrain.
///
/// `pub(crate)` because the suppression order markers ([`crate::units`]) need
/// exactly this symbol and exactly this draping: a ring built once and moved by
/// a transform floats off the hillside, and a ring wound the other way is
/// invisible. Both traps are already solved here, so they are solved once.
pub(crate) fn ring_mesh(scn: &Scenario, centre: Pos, radius_m: f32) -> Mesh {
    let w = ring_width(radius_m) * 0.5;
    let (inner, outer) = ((radius_m - w).max(1.0), radius_m + w);

    let mut positions = Vec::with_capacity((RING_SEGMENTS + 1) * 2);
    let mut normals = Vec::with_capacity((RING_SEGMENTS + 1) * 2);
    let mut uvs = Vec::with_capacity((RING_SEGMENTS + 1) * 2);
    let mut indices = Vec::with_capacity(RING_SEGMENTS * 6);

    for i in 0..=RING_SEGMENTS {
        let a = i as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        for r in [inner, outer] {
            let p = Pos {
                x: centre.x + c * r,
                y: centre.y + s * r,
            };
            // Clamped, not skipped: a ring straddling the window edge should
            // still close rather than come apart.
            let h = scn.terrain.height_at(p) + RING_LIFT_M;
            positions.push([p.x, h, -p.y]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([if r == inner { 0.0 } else { 1.0 }, i as f32]);
        }
    }
    for i in 0..RING_SEGMENTS as u32 {
        let a = i * 2;
        // Wound counter-clockwise from above, for the same reason the road
        // ribbons are: the other way round is invisible, not inside-out.
        indices.extend_from_slice(&[a, a + 2, a + 1, a + 1, a + 2, a + 3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn empty_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

/// Clamp a radius the UI may have been given by keyboard entry.
pub fn clamp_radius(r: f32) -> f32 {
    r.clamp(MIN_IGNITION_RADIUS_M, MAX_IGNITION_RADIUS_M)
}
