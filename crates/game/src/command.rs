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
use bevy_egui::{egui, EguiContexts};
use scenario::Pos;

use crate::camera::OrbitCamera;
use crate::ignition_edit::{ring_mesh, EditMode, IgnitionTool};
use crate::pick;
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
        }
    }

    pub fn disarm(&mut self) {
        self.armed = None;
        self.hover = None;
        self.line_from = None;
    }
}

/// Marks the cursor ring and the pending line's first-point ring.
#[derive(Component)]
pub struct OrderCursor;

#[derive(Resource)]
pub struct CursorAssets {
    ok: Handle<StandardMaterial>,
    blocked: Handle<StandardMaterial>,
    anchor: Handle<StandardMaterial>,
}

/// Radius of the cursor ring, metres. Sized to the thing being ordered: an
/// engine's is its hose reach, so the ring *is* the area it can work.
const CURSOR_R_M: f32 = 60.0;

pub fn setup(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut ring = |r: f32, g: f32, b: f32, a: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgba(r, g, b, a),
            emissive: LinearRgba::rgb(r * 1.6, g * 1.6, b * 1.6),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    };
    commands.insert_resource(CursorAssets {
        ok: ring(0.35, 0.95, 1.00, 0.80),
        blocked: ring(0.95, 0.20, 0.20, 0.65),
        anchor: ring(1.00, 0.85, 0.30, 0.85),
    });
}

/// Keyboard: cycle the selection, arm the orders, stand down.
///
/// Deliberately not overlapping the existing bindings: `i` is the ignition tool,
/// `e` the evacuation order, `r` restart. `Tab` cycles units, `a`/`l`/`d` arm,
/// `x` sends the selected unit back to staging, `c` calls for air support.
pub fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut tool: ResMut<OrderTool>,
    mut ignition: ResMut<IgnitionTool>,
    mut sim: ResMut<Sim>,
) {
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
        let n = sim.crews.units.len();
        // Only through the units that can actually be given an order, so Tab
        // never parks the selection on an aircraft that has not been called.
        let start = tool.selected.map(|s| s + 1).unwrap_or(0);
        tool.selected = (0..n)
            .map(|k| (start + k) % n)
            .find(|id| sim.crews.units[*id].assignable());
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
        if keys.just_pressed(key) {
            // Arming an order takes left-click off the ignition tool: two tools
            // fighting for the same click is how a control stops being trusted.
            if ignition.mode != EditMode::Off {
                ignition.mode = EditMode::Off;
            }
            tool.toggle(kind);
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
    ui_focus: Res<crate::ui::UiFocus>,
    mut tool: ResMut<OrderTool>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<OrbitCamera>>,
) {
    if !tool.is_armed() || ui_focus.0 {
        if tool.hover.is_some() {
            tool.hover = None;
        }
        return;
    }
    let (Ok(window), Ok((camera, cam_tf))) = (windows.get_single(), camera.get_single()) else {
        return;
    };
    let (kind, unit) = (tool.armed, tool.selected);
    tool.hover = pick::cursor_ground(&sim.scenario, camera, cam_tf, window)
        .map(|p| (p, workable(&sim, p, kind, unit)));
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
pub fn workable(sim: &Sim, p: Pos, order: Option<OrderKind>, unit: Option<usize>) -> bool {
    let world = &sim.scenario.world;
    if !world.contains(p) {
        return false;
    }
    let Some(unit) = unit.and_then(|id| sim.crews.units.get(id)) else {
        return false;
    };
    let has_fuel = |reach: f32| {
        fire::cells_in_radius(world, p, reach)
            .into_iter()
            .any(|c| sim.fire.is_suppressible(c, &sim.scenario))
    };
    match (order, unit.kind) {
        // An engine works from the road, so the question is whether there is a
        // road *this* engine can get to within hose reach of the point -- and
        // whether there is anything beside it worth wetting.
        (Some(OrderKind::Attack), UnitKind::Engine) => {
            let net = &sim.agents.network;
            let road = net
                .nearest(unit.pos, true)
                .and_then(|a| net.nearest_reachable(p, true, a))
                .map(|n| net.pos(n));
            road.map_or(false, |r| {
                (r.x - p.x).powi(2) + (r.y - p.y).powi(2) <= ENGINE_REACH_M.powi(2)
            }) && has_fuel(ENGINE_REACH_M)
        }
        (Some(OrderKind::Attack | OrderKind::Line), _) => has_fuel(120.0),
        (Some(OrderKind::Drop), _) => has_fuel(abm::suppression::DROP_LENGTH_M * 0.5),
        _ => false,
    }
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
    let Some((p, ok)) = tool.hover else { return };
    let Some(id) = tool.selected else {
        tool.refusal = Some("Select a unit first.".into());
        return;
    };
    if !ok {
        // The ring is already red; the panel says why in words.
        tool.refusal = Some(match tool.armed {
            Some(OrderKind::Drop) => "Nothing unburnt to drop on there.".into(),
            Some(OrderKind::Attack) if sim.crews.units[id].kind == UnitKind::Engine => {
                "No road within hose reach of there.".into()
            }
            _ => "No fuel worth working there.".into(),
        });
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
            // Line orders disarm: the alignment is placed, and leaving the tool
            // armed would make the next click start another one by accident.
            if matches!(task, Task::Line { .. }) {
                tool.disarm();
            }
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
            PbrBundle {
                mesh: meshes.add(ring_mesh(&sim.scenario, from, 25.0)),
                material: assets.anchor.clone(),
                ..default()
            },
            OrderCursor,
        ));
        // And the alignment as it would be if the player clicked now.
        if let Some((p, _)) = tool.hover {
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(crate::units::ribbon(&sim.scenario, from, p, 5.0)),
                    material: assets.anchor.clone(),
                    ..default()
                },
                OrderCursor,
            ));
        }
    }
    let Some((p, ok)) = tool.hover else { return };
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(ring_mesh(&sim.scenario, p, CURSOR_R_M)),
            material: if ok { assets.ok.clone() } else { assets.blocked.clone() },
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
}

/// The **Resources** panel: the roster, the selection, and the orders.
///
/// Its own dock rather than a section of "Incident" for the reason those two
/// are already split: this is where the player *acts*, and it is read while
/// pointing at the map. Docked along the bottom, outside the 3D viewport, so
/// giving an order never means a panel is sitting on top of the ground the
/// order is about.
pub fn panel(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    mut tool: ResMut<OrderTool>,
    mut ignition: ResMut<IgnitionTool>,
    mut focus: ResMut<crate::ui::UiFocus>,
    mut panels: ResMut<crate::ui::PanelState>,
) {
    let ctx = contexts.ctx_mut();
    let mut open = panels.resources;
    let stats = sim.crews.stats();
    let air_eta = sim.crews.air_eta_s();
    let mut select: Option<usize> = tool.selected;
    let mut arm: Option<OrderKind> = None;
    let mut request_air = false;
    let mut recall: Option<usize> = None;

    egui::TopBottomPanel::bottom("resources_dock")
        .resizable(open)
        .default_height(260.0)
        .height_range(if open { 140.0..=420.0 } else { 26.0..=26.0 })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                open = crate::ui::collapse_button(ui, open, "⏷", "⏶");
                if open {
                    ui.heading("Resources");
                }
            });
            if !open {
                return;
            }
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("supp").num_columns(2).show(ui, |ui| {
                ui.label("Working");
                ui.label(format!("{} of {}", stats.working, sim.crews.units.len()));
                ui.end_row();
                ui.label("Water used");
                ui.label(format!("{:.1} kL · {} drops", stats.water_l / 1000.0, stats.drops));
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

            ui.separator();
            for u in &sim.crews.units {
                let selected = tool.selected == Some(u.id);
                let c = crate::units::colour(u.kind, u.state);
                let srgb = c.to_srgba();
                let colour = egui::Color32::from_rgb(
                    (srgb.red * 255.0) as u8,
                    (srgb.green * 255.0) as u8,
                    (srgb.blue * 255.0) as u8,
                );
                let mut text = egui::RichText::new(format!(
                    "{}  —  {}",
                    u.callsign,
                    status_line(&sim, u.id)
                ))
                .color(colour);
                if selected {
                    text = text.strong();
                }
                let row = ui.selectable_label(selected, text);
                if row.clicked() {
                    select = Some(u.id);
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
                row.on_hover_text(detail);
                if !u.note.is_empty() {
                    ui.small(
                        egui::RichText::new(format!("    {}", u.note))
                            .color(egui::Color32::from_rgb(240, 180, 60)),
                    );
                }
            }

            ui.separator();
            match select.and_then(|id| sim.crews.units.get(id)) {
                None => {
                    ui.label("Select a unit to give it an order.");
                }
                Some(u) => {
                    ui.label(format!("Orders for {}", u.callsign));
                    ui.horizontal_wrapped(|ui| {
                        for kind in [OrderKind::Attack, OrderKind::Line, OrderKind::Drop] {
                            // Inbound aircraft included: briefing one is the
                            // right move, not a mistake to grey out.
                            let allowed = kind.allowed_for(u.kind) && u.assignable();
                            let armed = tool.armed == Some(kind);
                            let label = if armed {
                                format!("◉ {}", kind.label())
                            } else {
                                kind.label().to_string()
                            };
                            let b = ui.add_enabled(
                                allowed,
                                egui::SelectableLabel::new(armed, label),
                            );
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
                            .on_hover_text("Return to staging and wait. (x)")
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

            if let Some(why) = &tool.refusal {
                ui.colored_label(egui::Color32::from_rgb(255, 140, 110), why);
            }

            ui.separator();
            ui.horizontal(|ui| {
                let none_left = stats.unrequested == 0;
                if ui
                    .add_enabled(!none_left, egui::Button::new("✈ Request air support  (c)"))
                    .on_hover_text(
                        "Aircraft come from the national fleet, not from Spotorno. \
                         Ask early: they are 25 minutes out.",
                    )
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
            ui.small("tab next unit · a attack · l cut line · d drop · x stand down · esc cancel");
            });
        });

    if select != tool.selected {
        tool.selected = select;
        // A new selection cannot inherit a half-placed line from the old one.
        tool.line_from = None;
        tool.refusal = None;
    }
    if let Some(kind) = arm {
        if ignition.mode != EditMode::Off {
            ignition.mode = EditMode::Off;
        }
        tool.toggle(kind);
    }
    if let Some(id) = recall {
        let _ = sim.crews.assign(id, Task::Return);
    }
    if request_air {
        let n = sim.crews.request_air();
        info!("air support requested: {n} aircraft");
    }
    panels.resources = open;

    focus.0 |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
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
        Task::Attack { .. } => s.push_str(" · direct attack"),
        Task::Line { from, to } => {
            let total = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
            s.push_str(&format!(
                " · line {:.0}/{:.0} m",
                u.line_done_m, total
            ));
        }
        Task::Drop { .. } => s.push_str(" · drop run"),
        Task::Return => s.push_str(" · returning"),
        Task::Hold => {}
    }
    s
}
