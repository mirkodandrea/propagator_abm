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

use abm::suppression::{Task, UnitKind, UnitState, ENGINE_REACH_M};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;
use scenario::Pos;

use crate::camera::OrbitCamera;
use crate::ignition_edit::{ring_mesh, EditMode, IgnitionTool};
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

/// Keyboard: cycle the selection, arm the orders, stand down.
///
/// `Tab` cycles units, `A`/`L`/`D` arm an order, `X` sends the selected unit
/// back to staging, `C` calls for air support. `A` and `D` used to *also* pan
/// the camera, because the camera panned on WASD — arming an attack slid the
/// map west, which looked like the order having some mysterious side effect.
/// The camera now pans on the arrow keys and these letters are unambiguous;
/// see `crate::camera::controls`.
///
/// Every branch is behind [`UiFocus::typing`]: `d` is a perfectly ordinary
/// letter to type into a search box.
pub fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<crate::ui::UiFocus>,
    mut tool: ResMut<OrderTool>,
    mut ignition: ResMut<IgnitionTool>,
    mut sim: ResMut<Sim>,
    mut panels: ResMut<crate::ui::PanelState>,
    mut selected: ResMut<crate::inspect::Selected>,
) {
    if focus.typing() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        tool.disarm();
    }
    // The invariant: at most one tool owns left-click. Arming an order disarms
    // the ignition tool below; this is the other direction, so whichever the
    // player reached for most recently is the one that is live.
    if ignition.mode != EditMode::Off && tool.is_armed() {
        tool.disarm();
    }
    if keys.just_pressed(KeyCode::Tab) {
        // Cycling the units is only useful if you can see them, so it also
        // brings the tab that lists them forward.
        panels.focus_tab(crate::ui::DockTab::Units);
        let n = sim.crews.units.len();
        // Only through the units that can actually be given an order, so Tab
        // never parks the selection on an aircraft that has not been called.
        let start = tool.selected.map(|s| s + 1).unwrap_or(0);
        tool.disarm();
        tool.selected = (0..n)
            .map(|k| (start + k) % n)
            .find(|id| sim.crews.units[*id].assignable());
        selected.target = tool.selected.map(crate::inspect::Target::Unit);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        let n = sim.crews.request_air();
        if n > 0 {
            info!(
                "air support requested: {n} aircraft, on station in {:.0} min",
                abm::suppression::AIR_RESPONSE_S / 60.0
            );
        }
    }

    let armed = [
        (KeyCode::KeyA, OrderKind::Attack),
        (KeyCode::KeyL, OrderKind::Line),
        (KeyCode::KeyD, OrderKind::Drop),
    ];
    for (key, kind) in armed {
        if keys.just_pressed(key)
            && tool
                .selected
                .and_then(|id| sim.crews.units.get(id))
                .is_some_and(|u| kind.allowed_for(u.kind) && u.assignable())
        {
            // Arming an order takes left-click off the ignition tool: two tools
            // fighting for the same click is how a control stops being trusted.
            if ignition.mode != EditMode::Off {
                ignition.mode = EditMode::Off;
            }
            tool.toggle(kind);
            panels.focus_tab(crate::ui::DockTab::Units);
        }
    }
    if keys.just_pressed(KeyCode::KeyX) {
        if let Some(id) = tool.selected {
            let _ = sim.crews.assign(id, Task::Return);
        }
    }
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
#[cfg(test)]
pub fn workable(sim: &Sim, p: Pos, order: Option<OrderKind>, unit: Option<usize>) -> bool {
    target_preview(sim, p, order, unit).reason.is_none()
}

#[derive(Default)]
struct TargetPreview {
    route: Vec<Pos>,
    road: Option<Pos>,
    reason: Option<&'static str>,
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
    if !u.assignable() || !order.allowed_for(u.kind) {
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
) {
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
    mut panels: ResMut<crate::ui::PanelState>,
) {
    if restarted.is_empty() {
        return;
    }
    restarted.clear();
    panels.evacuation_notice = None;
    panels.preview_evacuation = false;
    tool.disarm();
    tool.refusal = None;
    tool.confirmation = None;
}

/// The scrollable response roster and air-support controls. Orders live in
/// `orders_body`, pinned outside this scroll area.
pub fn units_body(
    ui: &mut egui::Ui,
    sim: &mut Sim,
    tool: &mut OrderTool,
    _ignition: &mut IgnitionTool,
    selected_entity: &mut crate::inspect::Selected,
    camera: &mut Query<&mut crate::camera::OrbitCamera>,
) -> bool {
    let stats = sim.crews.stats();
    let air_eta = sim.crews.air_eta_s();
    let mut select: Option<usize> = tool.selected;
    let mut request_air = false;
    let mut show_inspector = false;

    ui.add_space(8.0);
    crate::ui::section(ui, "Air support");
    ui.horizontal(|ui| {
        let none_left = stats.unrequested == 0;
        if ui
            .add_enabled(!none_left, egui::Button::new("✈ Request air support  (C)"))
            .on_hover_text(format!(
                "Aircraft come from the national fleet, not from {}. \
                 Ask early: they are 25 minutes out.",
                sim.scenario.metadata.location,
            ))
            .clicked()
        {
            request_air = true;
        }
        if let Some(eta) = air_eta {
            ui.colored_label(
                egui::Color32::from_rgb(240, 180, 60),
                format!("● {:.0} min out", (eta / 60.0).max(0.0)),
            );
        }
    });
    ui.add_space(6.0);
    ui.small("Tab next unit · A attack · L cut line · D drop · X stand down · Esc cancel");

    ui.collapsing("Response statistics", |ui| {
        crate::ui::section(ui, "Effort");
        egui::Grid::new("supp").num_columns(2).show(ui, |ui| {
            ui.label("Working");
            ui.label(format!("{} of {}", stats.working, sim.crews.units.len()));
            ui.end_row();
            ui.label("Water used");
            ui.label(format!(
                "{:.1} kL · {} drops",
                stats.water_l / 1000.0,
                stats.drops
            ));
            ui.end_row();
            ui.label("Line cut");
            ui.label(format!("{:.0} m", stats.line_m));
            ui.end_row();
            if stats.lost > 0 {
                ui.colored_label(egui::Color32::from_rgb(255, 90, 70), "Units lost");
                ui.colored_label(
                    egui::Color32::from_rgb(255, 90, 70),
                    format!("{}", stats.lost),
                );
                ui.end_row();
            }
        });
    });

    ui.add_space(8.0);
    crate::ui::section(ui, "Roster");
    egui::CollapsingHeader::new("Crew roster").default_open(true).show(ui, |ui| {
    for u in &sim.crews.units {
        let selected = tool.selected == Some(u.id);
        let c = crate::units::colour(u.kind, u.state);
        let srgb = c.to_srgba();
        let colour = egui::Color32::from_rgb(
            (srgb.red * 255.0).max(130.0) as u8,
            (srgb.green * 255.0).max(130.0) as u8,
            (srgb.blue * 255.0).max(130.0) as u8,
        );
        let mut text =
            egui::RichText::new(format!("{}  —  {}", u.callsign, status_line(&sim, u.id)))
                .color(colour);
        if selected {
            text = text.strong();
        }
        let row = ui.selectable_label(selected, text);
        if row.clicked() {
            select = Some(u.id);
            selected_entity.target = Some(crate::inspect::Target::Unit(u.id));
        }
        if row.double_clicked() {
            selected_entity.target = Some(crate::inspect::Target::Unit(u.id));
            if let Ok(mut orbit) = camera.get_single_mut() {
                let ground = sim.scenario.terrain.height_at(u.pos);
                orbit.focus = crate::frame::to_bevy(u.pos, ground);
                orbit.distance = orbit.distance.clamp(110.0, 220.0);
            }
            show_inspector = true;
        }
        // Everything the unit knows about itself, on hover: the numbers
        // that explain why it is or is not achieving anything.
        let detail = match u.kind {
            UnitKind::Engine => format!(
                "{}\nTank {:.0} of {:.0} L · {:.0} L delivered\nWorks within {:.0} m of a road it can reach.",
                u.kind.label(),
                u.water_l,
                u.tank_l,
                u.water_used_l,
                ENGINE_REACH_M,
            ),
            UnitKind::HandCrew => format!(
                "{}\n{:.0} m of line cut · {:.0} m/h in this fuel\nGoes where vehicles cannot; cannot outpace the fire.",
                u.kind.label(),
                u.line_cut_m,
                abm::suppression::LINE_M_PER_H,
            ),
            UnitKind::AirTanker => format!(
                "{}\n{} drops · {:.0} L delivered\n{:.0} L a load, {:.0} s to scoop and return.",
                u.kind.label(),
                u.drops,
                u.water_used_l,
                u.tank_l,
                abm::suppression::SCOOP_S,
            ),
        };
        row.on_hover_text(format!(
            "{detail}\n\nDouble-click to locate and inspect this unit."
        ));
        if selected && !u.note.is_empty() {
            ui.small(
                egui::RichText::new(format!("    {}", u.note))
                    .color(egui::Color32::from_rgb(240, 180, 60)),
            );
        }
    }

    });

    if select != tool.selected {
        tool.disarm();
        tool.confirmation = None;
        tool.selected = select;
        // A new selection cannot inherit a half-placed line from the old one.
        tool.line_from = None;
        tool.refusal = None;
    }
    if request_air {
        let n = sim.crews.request_air();
        info!("air support requested: {n} aircraft");
    }
    show_inspector
}

/// Text for the panel: what this unit is doing, in one line.
pub fn status_line(sim: &Sim, id: usize) -> String {
    let Some(u) = sim.crews.units.get(id) else {
        return String::new();
    };
    let mut s = u.state.label().to_string();
    if u.state == UnitState::Inbound {
        let eta = (u.arrives_at_s - sim.crews.time_s()).max(0.0);
        s = format!("inbound, {:.0} min", eta / 60.0);
    }
    match u.task {
        Task::Attack { .. } => s.push_str(if u.kind == UnitKind::HandCrew {
            " · preparing defensive line"
        } else {
            " · road suppression"
        }),
        Task::Line { from, to } => {
            let total = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
            s.push_str(&format!(" · line {:.0}/{:.0} m", u.line_done_m, total));
        }
        Task::Drop { .. } => s.push_str(" · drop run"),
        Task::Return => s.push_str(" · returning"),
        Task::Hold => {}
    }
    s
}

/// The inspector, roster, and map share one selected crew.
pub fn sync_selection(selected: Res<crate::inspect::Selected>, mut tool: ResMut<OrderTool>) {
    let id = match selected.target {
        Some(crate::inspect::Target::Unit(id)) => Some(id),
        _ => None,
    };
    if selected.is_changed() && tool.selected != id {
        tool.disarm();
        tool.selected = id;
        tool.refusal = None;
        tool.confirmation = None;
    }
}

#[derive(Component)]
pub struct EvacuationPreview;

pub fn evacuation_preview(
    mut commands: Commands,
    sim: Res<Sim>,
    panels: Res<crate::ui::PanelState>,
    assets: Res<CursorAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    existing: Query<Entity, With<EvacuationPreview>>,
) {
    if panels.preview_evacuation && panels.dock.visible() {
        if existing.is_empty() {
            let centre = sim.scenario.world.centre_of(sim.ignition.centre);
            commands.spawn((
                MaterialMeshBundle::<RetroMaterial> {
                    mesh: meshes.add(ring_mesh(&sim.scenario, centre, 2000.0)),
                    material: assets.anchor.clone(),
                    ..default()
                },
                EvacuationPreview,
            ));
        }
    } else {
        for e in &existing {
            commands.entity(e).despawn();
        }
    }
}

#[cfg(test)]
mod ux_tests {
    use super::*;

    fn simulation() -> Sim {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let scenario = scenario::Scenario::load_by_id(data, "suppression_access").unwrap();
        let (weather, radius) = crate::sim::opening_conditions("suppression_access");
        Sim::new(
            scenario,
            weather,
            radius,
            42,
            behavior::defaults::default_library(),
        )
        .unwrap()
    }

    #[test]
    fn placing_an_order_disarms_and_acknowledges() {
        let sim = simulation();
        let id = sim
            .crews
            .units
            .iter()
            .find(|u| u.kind == UnitKind::HandCrew)
            .unwrap()
            .id;
        let target = (0..sim.scenario.world.fire_rows)
            .flat_map(|y| {
                (0..sim.scenario.world.fire_cols).map(move |x| scenario::Cell { row: y, col: x })
            })
            .map(|c| sim.scenario.world.centre_of(c))
            .find(|&p| workable(&sim, p, Some(OrderKind::Attack), Some(id)))
            .expect("workable crew target");
        let mut buttons = ButtonInput::<MouseButton>::default();
        buttons.press(MouseButton::Left);
        let mut app = App::new();
        app.insert_resource(sim)
            .insert_resource(buttons)
            .insert_resource(OrderTool {
                selected: Some(id),
                armed: Some(OrderKind::Attack),
                hover: Some((target, true)),
                ..default()
            })
            .add_systems(Update, place);
        app.update();
        let tool = app.world().resource::<OrderTool>();
        assert!(!tool.is_armed());
        assert!(tool
            .confirmation
            .as_ref()
            .unwrap()
            .contains("Build defensive line"));
        assert!(matches!(
            app.world().resource::<Sim>().crews.units[id].task,
            Task::Attack { .. }
        ));
        // A second click must not replace the accepted order.
        app.update();
        assert!(!app.world().resource::<OrderTool>().is_armed());
    }

    #[test]
    fn text_focus_acquired_this_frame_blocks_game_shortcuts() {
        fn focus_search(mut contexts: bevy_egui::EguiContexts) {
            let ctx = contexts.ctx_mut();
            ctx.begin_frame(egui::RawInput::default());
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut search = String::new();
                ui.text_edit_singleline(&mut search).request_focus();
            });
        }
        let sim = simulation();
        let id = sim
            .crews
            .units
            .iter()
            .find(|u| u.kind == UnitKind::Engine)
            .unwrap()
            .id;
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyA);
        let mut app = App::new();
        app.insert_resource(sim)
            .insert_resource(keys)
            .init_resource::<bevy_egui::EguiUserTextures>()
            .init_resource::<crate::ui::UiFocus>()
            .init_resource::<crate::ui::PanelState>()
            .init_resource::<crate::interview::Interview>()
            .init_resource::<crate::inspect::Selected>()
            .init_resource::<IgnitionTool>()
            .insert_resource(OrderTool {
                selected: Some(id),
                ..default()
            })
            .add_systems(
                Update,
                (focus_search, crate::ui::finalize_input_focus, controls).chain(),
            );
        app.world_mut().spawn((
            Window::default(),
            PrimaryWindow,
            bevy_egui::EguiContext::default(),
        ));
        app.update();
        assert!(app.world().resource::<crate::ui::UiFocus>().typing());
        assert!(!app.world().resource::<OrderTool>().is_armed());
    }

    #[test]
    fn invalid_or_incompatible_targets_do_not_pass_validation() {
        let sim = simulation();
        let engine = sim
            .crews
            .units
            .iter()
            .find(|u| u.kind == UnitKind::Engine)
            .unwrap();
        assert!(!workable(
            &sim,
            engine.pos,
            Some(OrderKind::Drop),
            Some(engine.id)
        ));
        assert!(!workable(
            &sim,
            Pos {
                x: -100.0,
                y: -100.0
            },
            Some(OrderKind::Attack),
            Some(engine.id)
        ));
        assert!(!workable(&sim, engine.pos, Some(OrderKind::Attack), None));
    }

    #[test]
    fn selecting_an_entity_cancels_stale_order_and_syncs_crew() {
        let mut app = App::new();
        let mut selected = crate::inspect::Selected::default();
        selected.target = Some(crate::inspect::Target::Unit(2));
        app.insert_resource(selected)
            .insert_resource(OrderTool {
                selected: Some(1),
                armed: Some(OrderKind::Attack),
                ..default()
            })
            .add_systems(Update, sync_selection);
        app.update();
        let tool = app.world().resource::<OrderTool>();
        assert_eq!(tool.selected, Some(2));
        assert!(!tool.is_armed());
    }
}

/// Pinned below the response scroll area so commands never disappear behind the roster.
pub fn orders_body(
    ui: &mut egui::Ui,
    sim: &mut Sim,
    tool: &mut OrderTool,
    ignition: &mut IgnitionTool,
    selected_entity: &mut crate::inspect::Selected,
    camera: &mut Query<&mut OrbitCamera>,
) {
    let select = tool.selected;
    let mut arm = None;
    let mut recall = None;
    ui.add_space(8.0);
    crate::ui::section(ui, "Orders");
    match select.and_then(|id| sim.crews.units.get(id)) {
        None => {
            ui.label("Select a crew in the roster to give it an order.");
        }
        Some(u) => {
            ui.horizontal(|ui| {
                ui.strong(format!("For {}", u.callsign));
                if ui.button("Locate").clicked() {
                    selected_entity.target = Some(crate::inspect::Target::Unit(u.id));
                    if let Ok(mut orbit) = camera.get_single_mut() {
                        orbit.focus =
                            crate::frame::to_bevy(u.pos, sim.scenario.terrain.height_at(u.pos));
                        orbit.distance = 220.0;
                    }
                }
            });
            ui.label(status_line(sim, u.id));
            if !u.note.is_empty() {
                ui.colored_label(egui::Color32::YELLOW, u.note);
            }
            if u.kind == UnitKind::HandCrew {
                ui.small("Builds a line across the fire’s approach; does not spray water.");
            }
            ui.horizontal_wrapped(|ui| {
                for kind in [OrderKind::Attack, OrderKind::Line, OrderKind::Drop] {
                    // Inbound aircraft included: briefing one is the
                    // right move, not a mistake to grey out.
                    let allowed = kind.allowed_for(u.kind) && u.assignable();
                    let armed = tool.armed == Some(kind);
                    let label = if armed {
                        format!("▶ {}", kind.label_for(u.kind))
                    } else {
                        kind.label_for(u.kind).to_string()
                    };
                    let b = ui.add_enabled(allowed, egui::SelectableLabel::new(armed, label));
                    let b = match kind {
                        OrderKind::Attack => b.on_hover_text(
                            "Click the ground. An engine wets the fuel within \
                             hose reach of the nearest road it can reach; a crew \
                             cuts line across the fire's approach.",
                        ),
                        OrderKind::Line => b.on_hover_text(
                            "Two clicks: where the line starts, and where it \
                             ends. Permanent — cut fuel does not grow back \
                             during the incident.",
                        ),
                        OrderKind::Drop => b.on_hover_text(
                            "Click the ground. One load, then back to the water \
                             and round again until you re-task it.",
                        ),
                    };
                    if b.clicked() {
                        arm = Some(kind);
                    }
                }
                if ui
                    .button("Stand down")
                    .on_hover_text("Return to staging and wait. (X)")
                    .clicked()
                {
                    recall = Some(u.id);
                }
            });
            if tool.armed == Some(OrderKind::Line) && tool.line_from.is_none() {
                ui.small("Click where the line should start.");
            } else if tool.line_from.is_some() {
                ui.small("Now click where it should end.");
            }
        }
    }

    if let Some(message) = &tool.confirmation {
        ui.colored_label(egui::Color32::from_rgb(130, 230, 180), message);
    }
    if let Some(why) = &tool.refusal {
        ui.colored_label(egui::Color32::from_rgb(255, 140, 110), why);
    }

    if let Some(kind) = arm {
        ignition.mode = EditMode::Off;
        tool.toggle(kind);
    }
    if let Some(id) = recall {
        if sim.crews.assign(id, Task::Return).is_ok() {
            tool.disarm();
            tool.refusal = None;
            tool.confirmation = Some(format!(
                "{}: returning to staging.",
                sim.crews.units[id].callsign
            ));
        }
    }
}
