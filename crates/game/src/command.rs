//! Directing the suppression units: select, arm an order, click the ground.
//!
//! The gesture is the ignition tool's, because that one already works and the
//! player has already learnt it: arm a mode, get a cursor ring that tells you
//! whether the click will do anything, click. What is different is that an order
//! belongs to a *unit*, so there is a selection first, and the ring is drawn in
//! the colour of "this will work" or "this will not" for reasons specific to the
//! order — an engine needs a road, a crew needs fuel to cut, an aircraft needs
//! something unburnt to drop on.
//!
//! **Three things own left-click**, and they must never be armed at once:
//! [`crate::ignition_edit`] (place a fire), [`crate::inspect`] (select an agent),
//! and this. Arming an order disarms the ignition tool, and `inspect::pick_click`
//! stands down while an order is armed. The rule is that at most one of the three
//! is armed, and `esc` returns to plain inspect-and-orbit.
//!
//! A hand line takes two clicks — where to start and where to end — because an
//! alignment is a line and no single point describes one. The first click is
//! remembered in [`OrderTool::line_from`] and drawn as a marker, so the second
//! click is placed in relation to it rather than from memory.

use abm::suppression::{Task, UnitKind, ENGINE_REACH_M};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use scenario::Pos;

use crate::camera::OrbitCamera;
use crate::rings::ring_mesh;
use crate::pick;
use crate::retro;
use crate::retro::RetroMaterial;
use crate::sim::Sim;

/// What an armed left-click will order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrderKind {
    /// Work the fire here: wet the fuel (engine) or cut across the front (crew).
    Attack,
    /// Cut a line along an alignment. Two clicks.
    Line,
    /// Put a load here. Aircraft only.
    Drop,
}

impl OrderKind {
    pub fn label(self) -> &'static str {
        match self {
            OrderKind::Attack => "Attack here",
            OrderKind::Line => "Cut line",
            OrderKind::Drop => "Drop here",
        }
    }

    pub fn label_for(self, kind: UnitKind) -> &'static str {
        match (self, kind) {
            (Self::Attack, UnitKind::Engine) => "Suppress from road",
            (Self::Attack, UnitKind::HandCrew) => "Build defensive line here",
            (Self::Line, _) => "Draw line (2 clicks)",
            (Self::Drop, _) => "Drop water here",
            _ => self.label(),
        }
    }

    /// Can this unit take this order at all? The same rules
    /// [`abm::suppression::Suppression::assign`] enforces, asked early so the
    /// button can be greyed out rather than the click refused.
    pub fn allowed_for(self, kind: UnitKind) -> bool {
        match self {
            OrderKind::Attack => !kind.is_air(),
            OrderKind::Line => kind == UnitKind::HandCrew,
            OrderKind::Drop => kind.is_air(),
        }
    }
}

#[derive(Resource, Default)]
pub struct OrderTool {
    /// Unit the panel has selected, by id.
    pub selected: Option<usize>,
    /// Order waiting for a ground click.
    pub armed: Option<OrderKind>,
    /// Where the cursor is, and whether the armed order would achieve anything
    /// there. Recomputed each frame while armed.
    pub hover: Option<(Pos, bool)>,
    /// First click of a two-click line order.
    pub line_from: Option<Pos>,
    /// Last refusal, for the panel to show. Not a log line: the reason an order
    /// was refused is the most useful thing the model knows about the map.
    pub refusal: Option<String>,
    pub confirmation: Option<String>,
    /// An order the map click produced, for the kiosk to give through its
    /// books (`demo::Referee`) rather than straight to the units.
    pub issued: Option<demo::Order>,
    /// Keep the matching mouse release from selecting an entity after disarming.
    pub click_consumed: bool,
    /// Planned road approach for the current cursor, reused by the overlay.
    pub preview_route: Vec<Pos>,
    pub preview_road: Option<Pos>,
    pub preview_reason: Option<&'static str>,
    preview_key: Option<(usize, OrderKind, i64, i32, i32)>,
}

impl OrderTool {
    pub fn is_armed(&self) -> bool {
        self.armed.is_some()
    }

    /// Arm an order, or disarm if it was already armed.
    pub fn toggle(&mut self, kind: OrderKind) {
        if self.armed == Some(kind) {
            self.disarm();
        } else {
            self.armed = Some(kind);
            self.line_from = None;
            self.refusal = None;
            self.confirmation = None;
        }
    }

    pub fn disarm(&mut self) {
        self.armed = None;
        self.hover = None;
        self.line_from = None;
        self.preview_route.clear();
        self.preview_road = None;
        self.preview_reason = None;
        self.preview_key = None;
    }
}

/// Marks the cursor ring and the pending line's first-point ring.
#[derive(Component)]
pub struct OrderCursor;

#[derive(Resource)]
pub struct CursorAssets {
    ok: Handle<RetroMaterial>,
    blocked: Handle<RetroMaterial>,
    anchor: Handle<RetroMaterial>,
}

/// Radius of the cursor ring, metres. Sized to the thing being ordered: an
/// engine's is its hose reach, so the ring *is* the area it can work.
const CURSOR_R_M: f32 = 60.0;

pub fn setup(mut commands: Commands, mut materials: ResMut<Assets<RetroMaterial>>) {
    let mut ring = |r: f32, g: f32, b: f32, a: f32| {
        materials.add(retro::material(
            StandardMaterial {
                base_color: Color::srgba(r, g, b, a),
                emissive: LinearRgba::rgb(r * 1.6, g * 1.6, b * 1.6),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            },
            true,
        ))
    };
    commands.insert_resource(CursorAssets {
        ok: ring(0.35, 0.95, 1.00, 0.80),
        blocked: ring(0.95, 0.20, 0.20, 0.65),
        anchor: ring(1.00, 0.85, 0.30, 0.85),
    });
}

/// Track the ground point under the cursor and whether the armed order would
/// achieve anything there.
pub fn hover(
    sim: Res<Sim>,
    buttons: Res<ButtonInput<MouseButton>>,
    ui_focus: Res<crate::ui::UiFocus>,
    mut tool: ResMut<OrderTool>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<OrbitCamera>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        tool.click_consumed = false;
    }
    if !tool.is_armed() || ui_focus.pointer {
        if tool.hover.is_some() {
            tool.hover = None;
        }
        return;
    }
    let (Ok(window), Ok((camera, cam_tf))) = (windows.get_single(), camera.get_single()) else {
        return;
    };
    let (kind, unit) = (tool.armed, tool.selected);
    tool.hover = pick::cursor_ground(&sim.scenario, camera, cam_tf, window).map(|p| {
        // Keep pathfinding out of stationary rendering frames. A metre of
        // cursor movement or a simulation tick invalidates the preview.
        let key = unit.zip(kind).map(|(id, order)| {
            (
                id,
                order,
                sim.fire.time_s(),
                p.x.round() as i32,
                p.y.round() as i32,
            )
        });
        if key != tool.preview_key || key.is_none() {
            let preview = target_preview(&sim, p, kind, unit);
            tool.preview_route = preview.route;
            tool.preview_road = preview.road;
            tool.preview_reason = preview.reason;
            tool.preview_key = key;
        }
        let ok = tool.preview_reason.is_none();
        (p, ok)
    });
}

/// Would an order at `p` do anything?
///
/// Answered per unit kind, because "useless" means something different for each,
/// and answering it in the cursor rather than after the click is the whole point
/// — the same reasoning as the ignition ring turning red on non-burnable fuel.
///
/// Asked of the *selected* unit, not of its kind in general: whether a road is
/// within hose reach depends on which engine is being sent, because reachability
/// is per road component and one engine's network is not another's.
#[derive(Default)]
struct TargetPreview {
    route: Vec<Pos>,
    road: Option<Pos>,
    reason: Option<&'static str>,
}

// A first drop requests the aircraft through Referee::order; it must be
// previewable before request_air changes Unavailable to Inbound.
fn can_target(kind: UnitKind, state: abm::suppression::UnitState, order: OrderKind) -> bool {
    use abm::suppression::UnitState;
    order.allowed_for(kind)
        && state != UnitState::Lost
        && (state != UnitState::Unavailable || (kind.is_air() && order == OrderKind::Drop))
}

fn target_preview(
    sim: &Sim,
    p: Pos,
    order: Option<OrderKind>,
    unit: Option<usize>,
) -> TargetPreview {
    let mut preview = TargetPreview::default();
    preview.reason = Some("Select an available unit and an order first.");
    let Some(u) = unit.and_then(|id| sim.crews.units.get(id)) else {
        return preview;
    };
    let Some(order) = order else { return preview };
    if !can_target(u.kind, u.state, order) {
        return preview;
    }
    if !sim.scenario.world.contains(p) {
        preview.reason = Some("Target is outside the scenario.");
        return preview;
    }
    if !u.kind.is_air() {
        let net = &sim.agents.network;
        let driving = u.kind == UnitKind::Engine;
        let endpoints = net
            .nearest(u.pos, driving)
            .and_then(|from| net.nearest_reachable(p, driving, from).map(|to| (from, to)));
        let Some((from, to)) = endpoints else {
            preview.reason = Some("No connected road or path. Choose another target or unit.");
            return preview;
        };
        let road = net.pos(to);
        preview.road = Some(road);
        if driving && distance(road, p) > ENGINE_REACH_M {
            preview.reason = Some("Outside hose reach. Target inside the road coverage ring.");
            return preview;
        }
        let Some(route) = abm::network::route(net, from, to, sim.fire.threat(), driving) else {
            preview.reason =
                Some("Approach blocked by fire. Choose another target or request aircraft.");
            return preview;
        };
        preview.route.push(u.pos);
        preview.route.push(net.pos(from));
        preview
            .route
            .extend(route.into_iter().map(|node| net.pos(node)));
        if !driving {
            preview.route.push(p);
        }
    }
    let reach = match u.kind {
        UnitKind::Engine => ENGINE_REACH_M,
        UnitKind::HandCrew => 120.0,
        UnitKind::AirTanker => abm::suppression::DROP_LENGTH_M * 0.5,
    };
    preview.reason = if fire::cells_in_radius(&sim.scenario.world, p, reach)
        .into_iter()
        .any(|c| sim.fire.is_suppressible(c, &sim.scenario))
    {
        None
    } else {
        Some("No suppressible fuel here. Choose unburnt fuel near the fire edge.")
    };
    preview
}

fn distance(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Turn a click into an order.
pub fn place(
    mut sim: ResMut<Sim>,
    mut tool: ResMut<OrderTool>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        return;
    }
    if !tool.is_armed() || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some((p, _)) = tool.hover else { return };
    tool.click_consumed = true;
    let Some(id) = tool.selected else {
        tool.refusal = Some("Select a unit first.".into());
        return;
    };
    let preview = target_preview(&sim, p, tool.armed, tool.selected);
    tool.preview_reason = preview.reason;
    if preview.reason.is_some() {
        tool.refusal = Some(
            tool.preview_reason
                .unwrap_or("Target is not workable.")
                .into(),
        );
        return;
    }

    // Map orders use the same ledger and session activity as district buttons.
    let issued = match tool.armed {
        Some(OrderKind::Drop) => Some(demo::Order::Drop { at: p }),
        Some(OrderKind::Attack) => Some(demo::Order::Attack { kind: sim.crews.units[id].kind, at: p }),
        _ => None,
    };
    if let Some(order) = issued {
        tool.issued = Some(order);
        tool.refusal = None;
        tool.confirmation = Some(String::new());
        tool.disarm();
        return;
    }
    let task = match tool.armed {
        Some(OrderKind::Attack) => Some(Task::Attack { at: p }),
        Some(OrderKind::Drop) => Some(Task::Drop { at: p }),
        Some(OrderKind::Line) => match tool.line_from.take() {
            // First click anchors the alignment; the order waits for the second.
            None => {
                tool.line_from = Some(p);
                tool.refusal = None;
                return;
            }
            Some(from) => Some(Task::Line { from, to: p }),
        },
        None => None,
    };
    let Some(task) = task else { return };

    match sim.crews.assign(id, task) {
        Ok(()) => {
            let u = &sim.crews.units[id];
            info!("{} ordered: {:?}", u.callsign, task);
            tool.refusal = None;
            tool.confirmation = Some(format!(
                "{}: {} ordered at {:.0}, {:.0} m.{}",
                u.callsign,
                tool.armed.unwrap().label_for(u.kind),
                p.x,
                p.y,
                if sim.playing {
                    ""
                } else {
                    " Press Play to execute."
                }
            ));
            tool.disarm();
        }
        Err(why) => {
            tool.refusal = Some(format!("{}: {why}", sim.crews.units[id].callsign));
        }
    }
}

/// Draw the cursor ring, and the anchor of a half-placed line.
pub fn update_cursor(
    mut commands: Commands,
    sim: Res<Sim>,
    tool: Res<OrderTool>,
    assets: Res<CursorAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    existing: Query<Entity, With<OrderCursor>>,
) {
    // Rebuilt per frame while armed, like the ignition hover ring: the ring
    // reads as lying on the hillside only because its vertices are draped on
    // the terrain, and a transform cannot do that. 96 segments is free next to
    // the vegetation it is drawn over.
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !tool.is_armed() {
        return;
    }
    if let Some(from) = tool.line_from {
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(ring_mesh(&sim.scenario, from, 25.0)),
                material: assets.anchor.clone(),
                ..default()
            },
            OrderCursor,
        ));
        // And the alignment as it would be if the player clicked now.
        if let Some((p, _)) = tool.hover {
            commands.spawn((
                MaterialMeshBundle::<RetroMaterial> {
                    mesh: meshes.add(crate::units::ribbon(&sim.scenario, from, p, 5.0)),
                    material: assets.anchor.clone(),
                    ..default()
                },
                OrderCursor,
            ));
        }
    }
    let Some((p, ok)) = tool.hover else { return };
    // A road-centered ring shows actual hose coverage; the approach shows how
    // the selected crew gets there. These are plans, not guarantees about future fire.
    let mut segments = tool
        .preview_route
        .windows(2)
        .filter(|pair| distance(pair[0], pair[1]) > 0.1)
        .map(|pair| crate::units::ribbon(&sim.scenario, pair[0], pair[1], 5.0));
    if let Some(mut route_mesh) = segments.next() {
        for segment in segments {
            route_mesh.merge(&segment);
        }
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: meshes.add(route_mesh),
                material: assets.anchor.clone(),
                ..default()
            },
            OrderCursor,
        ));
    }
    if tool
        .selected
        .is_some_and(|id| sim.crews.units[id].kind == UnitKind::Engine)
    {
        if let Some(road) = tool.preview_road {
            commands.spawn((
                MaterialMeshBundle::<RetroMaterial> {
                    mesh: meshes.add(ring_mesh(&sim.scenario, road, ENGINE_REACH_M)),
                    material: assets.anchor.clone(),
                    ..default()
                },
                OrderCursor,
            ));
        }
    }
    commands.spawn((
        MaterialMeshBundle::<RetroMaterial> {
            mesh: meshes.add(ring_mesh(&sim.scenario, p, CURSOR_R_M)),
            material: if ok {
                assets.ok.clone()
            } else {
                assets.blocked.clone()
            },
            ..default()
        },
        OrderCursor,
    ));
}

/// Drop the selection and any half-placed order when the sim restarts.
///
/// Unit ids survive a restart (the roster is rebuilt identically), so the
/// selection *could* be kept — but a half-placed line anchored in the previous
/// run, and a refusal explaining a fire that no longer exists, could not.
pub fn reset(
    mut restarted: EventReader<crate::sim::SimRestarted>,
    mut tool: ResMut<OrderTool>,

) {
    if restarted.is_empty() {
        return;
    }
    restarted.clear();
    tool.disarm();
    tool.refusal = None;
    tool.confirmation = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use abm::suppression::UnitState;

    #[test]
    fn map_click_requests_air_and_assigns_its_first_drop() -> anyhow::Result<()> {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let spec = demo::draw(demo::ALL[0], 42).unwrap().spec;
        let sim = Sim::at_ignition(
            scenario::Scenario::load_by_id(&data, spec.id)?, spec.weather,
            spec.ignition, spec.radius_m, 42, behavior::defaults::default_library(),
        )?;
        let mut referee = demo::Referee::new(spec, &sim.scenario, &sim.agents, demo::Variant::default());
        let air = sim.crews.units.iter().find(|u| u.kind.is_air()).unwrap().id;
        let p = sim.scenario.world.centre_of(fire::cells_in_radius(&sim.scenario.world, spec.ignition, 200.0)
            .into_iter().find(|c| sim.fire.is_suppressible(*c, &sim.scenario)).unwrap());
        let mut app = App::new();
        app.insert_resource(sim)
            .insert_resource(OrderTool { selected: Some(air), armed: Some(OrderKind::Drop), hover: Some((p, true)), ..default() })
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, place);
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        app.update();
        let order = app.world_mut().resource_mut::<OrderTool>().issued.take().expect("first request emits a drop");
        let mut sim = app.world_mut().resource_mut::<Sim>();
        let Sim { scenario, fire, agents, crews, .. } = &mut *sim;
        referee.order(order, demo::Parts { scn: scenario, fire, agents, crews });
        assert_eq!(crews.units[air].state, UnitState::Inbound);
        assert!(matches!(crews.units[air].task, Task::Drop { .. }));
        Ok(())
    }

    #[test]
    fn engine_map_click_is_logged_and_shift_drag_does_not_place() -> anyhow::Result<()> {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let spec = demo::draw(demo::ALL[0], 42).unwrap().spec;
        let sim = Sim::at_ignition(
            scenario::Scenario::load_by_id(&data, spec.id)?, spec.weather,
            spec.ignition, spec.radius_m, 42, behavior::defaults::default_library(),
        )?;
        let id = demo::run::best_unit(&sim.crews, UnitKind::Engine).unwrap();
        let p = sim.agents.network.nodes.iter().copied()
            .find(|p| target_preview(&sim, *p, Some(OrderKind::Attack), Some(id)).reason.is_none())
            .expect("a reachable road with fuel in hose range");
        let mut referee = demo::Referee::new(spec, &sim.scenario, &sim.agents, demo::Variant::default());
        let mut app = App::new();
        app.insert_resource(sim)
            .insert_resource(OrderTool { selected: Some(id), armed: Some(OrderKind::Attack), hover: Some((p, true)), ..default() })
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, place);
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
        app.update();
        assert!(app.world().resource::<OrderTool>().issued.is_none());
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::ShiftLeft);
        app.update();
        let order = app.world_mut().resource_mut::<OrderTool>().issued.take().expect("map attack emitted");
        assert!(matches!(order, demo::Order::Attack { kind: UnitKind::Engine, .. }));
        let mut sim = app.world_mut().resource_mut::<Sim>();
        let Sim { scenario, fire, agents, crews, .. } = &mut *sim;
        referee.order(order, demo::Parts { scn: scenario, fire, agents, crews });
        assert!(matches!(crews.units[id].task, Task::Attack { .. }));
        assert!(!referee.log.entries.is_empty());
        Ok(())
    }

    #[test]
    fn first_air_request_can_be_placed_but_lost_units_cannot() {
        assert!(can_target(UnitKind::AirTanker, UnitState::Unavailable, OrderKind::Drop));
        assert!(can_target(UnitKind::AirTanker, UnitState::Inbound, OrderKind::Drop));
        assert!(!can_target(UnitKind::AirTanker, UnitState::Lost, OrderKind::Drop));
        assert!(!can_target(UnitKind::Engine, UnitState::Unavailable, OrderKind::Attack));
        assert!(can_target(UnitKind::Engine, UnitState::Staged, OrderKind::Attack));
        assert!(!can_target(UnitKind::Engine, UnitState::Staged, OrderKind::Drop));
    }
}
