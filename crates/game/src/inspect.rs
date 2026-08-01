//! Click-to-inspect: households, people and vehicles.
//!
//! Picking is screen-space, not a mesh raycast — the same reasoning as
//! [`crate::pick`], but here the candidates are agents, not the terrain, so
//! the cheapest correct test is projecting every live candidate's world
//! position with [`Camera::world_to_viewport`] and taking whichever lands
//! nearest the cursor. That is a few thousand projections on the frame of a
//! click, not per frame, and it is naturally zoom-invariant: a household
//! beacon a thousand metres out and one fifty metres out compete on screen
//! pixels, which is what the player is actually pointing at.
//!
//! A "house" here is a household: the beacon [`crate::agents`] already spawns
//! at every house is the pick target, and the inspector cross-references it
//! against the structure it lives in ([`crate::buildings::Buildings`]) for the
//! building's own damage state. There is no separate building entity to pick
//! — the beacon already marks the house.

use abm::{Mode, TravelState};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use scenario::population::Status;

use crate::buildings::Buildings;
use crate::camera::OrbitCamera;
use crate::frame;
use crate::retro;
use crate::retro::RetroMaterial;
use crate::ignition_edit::EditMode;
use crate::sim::Sim;

/// Screen-space pick radius, logical pixels. Generous: these are small
/// figures at commander altitude, and a miss reads as a broken control.
pub(crate) const PICK_PX: f32 = 26.0;

/// A press-then-release further apart than this on screen is a drag (camera
/// orbit), not a click.
const CLICK_SLOP_PX: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Household(usize),
    Person(usize),
    /// Index into `Abm::travellers`. Not just cars: a traveller can abandon
    /// its car mid-route and continue on foot, and the inspector should keep
    /// following the same group rather than losing the selection.
    Traveller(usize),
    /// Index into `Suppression::units` — an engine, hand crew or air tanker.
    Unit(usize),
}

#[derive(Resource, Default)]
pub struct Selected {
    pub target: Option<Target>,
    /// A selection asked for on the command line, restored after a restart.
    ///
    /// Only ever set by `SPOTORNO_WATCH`. In play a restart clears the
    /// selection and should; an unattended run that asked to watch one agent
    /// asked to watch it for the whole run, and applying a behaviour restarts.
    pinned: Option<Target>,
}

impl Selected {
    /// `SPOTORNO_WATCH=household:12`, and the same for `person:` and `unit:`.
    ///
    /// Selecting an agent is a click on the 3D view, so it is the one input the
    /// composer's live execution view needs and the one an unattended run
    /// cannot produce — exactly the reason `SPOTORNO_COMPOSER` picks a tab and
    /// `SPOTORNO_TAB` picks a dock tab. Without it the live view can only ever
    /// be screenshotted empty.
    pub fn from_env() -> Selected {
        let Ok(spec) = std::env::var("SPOTORNO_WATCH") else {
            return Selected::default();
        };
        let (kind, id) = spec.split_once(':').unwrap_or((spec.as_str(), "0"));
        let id: usize = id.trim().parse().unwrap_or(0);
        let target = match kind.trim() {
            "household" | "hh" => Some(Target::Household(id)),
            "person" => Some(Target::Person(id)),
            "unit" => Some(Target::Unit(id)),
            other => {
                eprintln!("SPOTORNO_WATCH: no such agent kind {other:?}");
                None
            }
        };
        Selected { target, pinned: target }
    }
}

#[derive(Resource, Default)]
pub struct ClickTracker {
    down_at: Option<Vec2>,
}

#[derive(Component)]
pub struct SelectionRing;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RetroMaterial>>,
) {
    let mesh = meshes.add(Cylinder::new(1.0, 0.4));
    let material = materials.add(retro::material(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.55),
        emissive: LinearRgba::rgb(2.2, 2.2, 2.4),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }, true));
    commands.spawn((
        MaterialMeshBundle::<RetroMaterial> {
            mesh,
            material,
            visibility: Visibility::Hidden,
            ..default()
        },
        SelectionRing,
    ));
}

/// Clear the selection across a restart: household ids and person ids are
/// stable, but `Target::Traveller` indexes the rebuilt, append-only
/// `travellers` list, and a stale index there would silently point at
/// whatever unrelated group ends up at that slot.
pub fn reset(mut restarted: EventReader<crate::sim::SimRestarted>, mut selected: ResMut<Selected>) {
    if restarted.is_empty() {
        return;
    }
    restarted.clear();
    selected.target = selected.pinned;
}

/// Turn a click into a selection. Screen-space nearest-candidate, gated on a
/// click (not a drag) landing outside the egui panels and outside ignition
/// placement, both of which already own left-click.
pub fn pick_click(
    mut selected: ResMut<Selected>,
    mut tracker: ResMut<ClickTracker>,
    ui_focus: Res<crate::ui::UiFocus>,
    tool: Res<crate::ignition_edit::IgnitionTool>,
    // Suppression orders also own left-click while armed: see `crate::command`
    // for the rule that at most one of the three tools ever is.
    order: Res<crate::command::OrderTool>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<OrbitCamera>>,
    sim: Res<Sim>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };

    if buttons.just_pressed(MouseButton::Left) {
        tracker.down_at = window.cursor_position();
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    let down = tracker.down_at.take();
    if tool.mode != EditMode::Off || order.is_armed() || ui_focus.pointer {
        return;
    }
    let (Some(down), Some(cur), Ok((camera, cam_tf))) =
        (down, window.cursor_position(), camera.get_single())
    else {
        return;
    };
    if down.distance(cur) > CLICK_SLOP_PX {
        return;
    }

    let mut best: Option<(f32, Target)> = None;
    let mut try_pick = |world: Vec3, target: Target| {
        let Some(screen) = camera.world_to_viewport(cam_tf, world) else {
            return;
        };
        let d = screen.distance(cur);
        if d < PICK_PX && best.map_or(true, |(bd, _)| d < bd) {
            best = Some((d, target));
        }
    };

    // Households are picked at the building itself now, not a floating
    // marker — mid-wall height reads as "click the house", and an evacuated
    // household's building has nobody left to select for.
    for h in &sim.agents.households {
        if h.status == Status::Evacuated {
            continue;
        }
        let ground = sim.scenario.terrain.height_at(h.home);
        try_pick(
            frame::to_bevy(h.home, ground + 4.0),
            Target::Household(h.id),
        );
    }

    // People currently drawn as their own figure — indoors, or riding a car,
    // they're picked as the household or the vehicle instead.
    for p in &sim.agents.people {
        let in_vehicle = p
            .traveller
            .and_then(|t| sim.agents.travellers.get(t))
            .map(|t| t.mode == Mode::Car && t.state != TravelState::Cutoff)
            .unwrap_or(false);
        let outside = matches!(p.status, Status::Evacuating | Status::Trapped) && !in_vehicle;
        if !outside {
            continue;
        }
        let ground = sim.scenario.terrain.height_at(p.pos);
        let scale = crate::people::figure_scale(sim.scenario.vr_palette().is_some());
        try_pick(
            frame::to_bevy(p.pos, ground + scale * 0.5),
            Target::Person(p.id),
        );
    }

    // Cars actually on the move — see `people::update_vehicles`'s visibility
    // rule, matched here so the pick target is exactly what's drawn.
    for (i, t) in sim.agents.travellers.iter().enumerate() {
        let driving = matches!(t.state, TravelState::Approaching | TravelState::OnNetwork)
            && t.mode == Mode::Car;
        if !driving {
            continue;
        }
        let ground = sim.scenario.terrain.height_at(t.pos);
        let scale = crate::people::figure_scale(sim.scenario.vr_palette().is_some());
        try_pick(
            frame::to_bevy(t.pos, ground + scale * 0.6),
            Target::Traveller(i),
        );
    }

    // Suppression units, at the altitude they're actually drawn at — see
    // `units::update_units`'s visibility rule, matched here so the pick
    // target is exactly what's drawn.
    for u in &sim.crews.units {
        if !(u.on_scene() || u.state == abm::suppression::UnitState::Lost) {
            continue;
        }
        let ground = sim.scenario.terrain.height_at(u.pos);
        let lift = if u.kind.is_air() {
            crate::units::AIR_ALTITUDE_M
        } else {
            crate::units::symbol_scale(sim.scenario.vr_palette().is_some()) * 1.3
        };
        try_pick(frame::to_bevy(u.pos, ground + lift), Target::Unit(u.id));
    }

    // A click that hit nothing deselects, same as clicking empty ground in
    // any RTS.
    selected.target = best.map(|(_, t)| t);
}

/// Where a target actually is right now, in world space. Shared by the
/// selection ring and [`crate::browser`]'s "zoom to" — the model position, not
/// wherever the entity happens to be drawn (a person indoors has no figure on
/// the map, but still has a `pos`).
pub(crate) fn target_pos(sim: &Sim, target: Target) -> Option<scenario::Pos> {
    match target {
        Target::Household(id) => sim.agents.households.get(id).map(|h| h.home),
        Target::Person(id) => sim.agents.people.get(id).map(|p| p.pos),
        Target::Traveller(i) => sim.agents.travellers.get(i).map(|t| t.pos),
        Target::Unit(id) => sim.crews.units.get(id).map(|u| u.pos),
    }
}

/// Which way a target is heading, world bearing in radians — only meaningful
/// for the entities that actually travel continuously. `None` for anything
/// else (a household never moves; a person on foot is tracked as a
/// traveller). Shared with [`crate::camera`]'s first-person view, which needs
/// a facing direction and not just a point.
pub(crate) fn target_heading(sim: &Sim, target: Target) -> Option<f32> {
    match target {
        Target::Traveller(i) => sim.agents.travellers.get(i).map(|t| t.heading),
        Target::Unit(id) => sim.crews.units.get(id).map(|u| u.heading),
        _ => None,
    }
}

/// Position the highlight ring on whatever is selected, or hide it.
pub fn update_ring(
    sim: Res<Sim>,
    selected: Res<Selected>,
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Visibility), With<SelectionRing>>,
) {
    let Ok((mut tf, mut vis)) = query.get_single_mut() else {
        return;
    };
    let Some(target) = selected.target else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    };

    let Some(pos) = target_pos(&sim, target) else {
        *vis = Visibility::Hidden;
        return;
    };

    if *vis != Visibility::Inherited {
        *vis = Visibility::Inherited;
    }
    let ground = sim.scenario.terrain.height_at(pos);
    // A slow pulse, big enough to find at a glance, never so big it reads as
    // a hazard marker rather than a selection.
    let pulse = 1.0 + 0.12 * (time.elapsed_seconds() * 2.2).sin();
    let radius = match target {
        Target::Household(_) => 9.0,
        Target::Person(_) => 3.0,
        Target::Traveller(_) => 5.0,
        Target::Unit(_) => 8.0,
    } * pulse;
    tf.translation = frame::to_bevy(pos, ground + 0.5);
    tf.scale = Vec3::new(radius, 1.0, radius);
}

/// The inspector panel: everything the model knows about whatever is
/// selected. Only shown when something is.
pub fn panel(
    mut contexts: EguiContexts,
    mut selected: ResMut<Selected>,
    mut focus: ResMut<crate::ui::UiFocus>,
    mut mode: ResMut<crate::camera::CameraMode>,
    mut panels: ResMut<crate::ui::PanelState>,
    mut composer: ResMut<crate::composer::Composer>,
    sim: Res<Sim>,
    buildings: Res<Buildings>,
) {
    let Some(target) = selected.target else {
        return;
    };
    let ctx = contexts.ctx_mut();
    let mut open = panels.inspector;
    let mut close = false;
    let mut jump_to: Option<Target> = None;
    // Set by the behaviour section's one button.
    let mut open_composer = false;

    egui::TopBottomPanel::bottom("inspector_dock")
        .resizable(open)
        .default_height(240.0)
        .height_range(if open { 140.0..=420.0 } else { 26.0..=26.0 })
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                open = crate::ui::collapse_button(ui, open, "⏷", "⏶");
                if open {
                    ui.heading(title(target));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").on_hover_text("Deselect").clicked() {
                        close = true;
                    }
                });
            });
            if !open {
                return;
            }
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    match target {
                        Target::Household(id) => {
                            household_panel(ui, &sim, &buildings, id, &mut jump_to)
                        }
                        Target::Person(id) => person_panel(ui, &sim, id, &mut jump_to),
                        Target::Traveller(i) => {
                            traveller_panel(ui, &sim, i, &mut jump_to, &mut mode)
                        }
                        Target::Unit(id) => unit_panel(ui, &sim, id, &mut mode),
                    }
                    behaviour_panel(ui, &sim, &mut open_composer, target);
                    history_panel(ui, &sim, target);
                });
        });

    if close {
        selected.target = None;
    } else if let Some(t) = jump_to {
        selected.target = Some(t);
    }
    if open_composer {
        composer.open = true;
        composer.right = crate::composer::RightTab::Live;
    }
    panels.inspector = open;
    focus.pointer |= ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

fn title(target: Target) -> String {
    match target {
        Target::Household(id) => format!("Household #{id}"),
        Target::Person(id) => format!("Person #{id}"),
        Target::Traveller(_) => "On the move".to_string(),
        Target::Unit(id) => format!("Unit #{id}"),
    }
}

fn household_panel(
    ui: &mut egui::Ui,
    sim: &Sim,
    buildings: &Buildings,
    id: usize,
    jump_to: &mut Option<Target>,
) {
    let Some(h) = sim.agents.households.get(id) else {
        ui.label("(gone)");
        return;
    };

    egui::Grid::new("hh").num_columns(2).show(ui, |ui| {
        ui.label("Status");
        ui.label(status_text(h.status));
        ui.end_row();

        ui.label("Structure");
        ui.label(buildings.status_of(id).unwrap_or("undamaged"));
        ui.end_row();

        ui.label("Intent");
        ui.label(match h.intent {
            scenario::population::Intent::LeaveEarly => "leave early",
            scenario::population::Intent::WaitAndSee => "wait and see",
            scenario::population::Intent::StayDefend => "stay and defend",
        });
        ui.end_row();

        ui.label("Perceived threat");
        ui.label(format!("{:.0}%", h.cue * 100.0));
        ui.end_row();

        ui.label("Order issued");
        ui.label(if h.ordered { "yes" } else { "no" });
        ui.end_row();

        ui.label("Order received");
        ui.label(if h.warning_received {
            "yes"
        } else if h.ordered {
            "in transit"
        } else {
            "—"
        });
        ui.end_row();

        ui.label("Warning channel");
        ui.label(channel_text(h.channel));
        ui.end_row();

        ui.label("Risk perception");
        ui.label(format!("{:.0}%", h.risk_perception * 100.0));
        ui.end_row();

        ui.label("Trust in authority");
        ui.label(format!("{:.0}%", h.trust_authority * 100.0));
        ui.end_row();

        ui.label("Vehicles");
        ui.label(format!("{}", h.vehicles));
        ui.end_row();

        ui.label("Defensible space");
        ui.label(format!("{:.0}%", h.defensible_space * 100.0));
        ui.end_row();

        ui.label("Baked prep time");
        ui.label(format!("{:.0} min", h.prep_time_min));
        ui.end_row();

        if h.status == Status::Preparing {
            ui.label("Prep remaining");
            ui.label(format!("{:.0} s", h.prep_remaining_s.max(0.0)));
            ui.end_row();
        }
        if h.status == Status::Trapped {
            ui.label("Sheltering");
            ui.label(format!("{:.0} s", h.shelter_s));
            ui.end_row();
        }
        if h.aware_at_s.is_finite() {
            ui.label("First aware at");
            ui.label(mmss(h.aware_at_s));
            ui.end_row();
        }
    });

    ui.separator();
    ui.label(format!("Members ({})", h.members.len()));
    egui::Grid::new("hh_members").num_columns(3).show(ui, |ui| {
        for &pid in &h.members {
            let Some(p) = sim.agents.people.get(pid) else {
                continue;
            };
            ui.label(format!("#{pid}"));
            ui.label(format!("age {}", p.age));
            if ui.small_button(status_text(p.status)).clicked() {
                *jump_to = Some(Target::Person(pid));
            }
            ui.end_row();
        }
    });

    if let Some(ti) = h.traveller {
        ui.separator();
        if ui.button("Inspect the group on the move").clicked() {
            *jump_to = Some(Target::Traveller(ti));
        }
    }
}

/// Why this agent is doing what it is doing, when it is running an authored
/// behaviour.
///
/// Absent entirely under the shipped hand-written models, rather than showing
/// an empty section: there is no graph to explain, and a panel that always
/// appeared would imply there was.
///
/// One function for all three kinds of agent, because the answer has the same
/// shape in each: a profile, a decision, and the branches that produced it. The
/// only per-kind part is which of the model's `explain` calls to make, and that
/// is three lines at the top.
fn behaviour_panel(ui: &mut egui::Ui, sim: &Sim, open: &mut bool, target: Target) {
    let found = match target {
        Target::Household(id) => sim.agents.behaviour_of(id),
        Target::Person(id) => sim.agents.person_behaviour_of(id),
        // A unit's roster does not cache the last decision the way the civilian
        // ones do, so the profile comes from the roster and the decision from
        // the trace below.
        Target::Unit(id) => sim
            .crews
            .policy_of(id)
            .map(|(sid, name)| (sid, name, behavior::Decision::default())),
        Target::Traveller(_) => None,
    };
    let Some((subtype_id, subtype_name, decision)) = found else {
        return;
    };
    let (subtype_id, subtype_name) = (subtype_id.to_string(), subtype_name.to_string());

    // Re-evaluated against the current fire rather than cached from the last
    // decision tick, so what it shows is what the agent would decide right now.
    // It is one graph on one agent at UI rate.
    let explained = match target {
        Target::Household(id) => sim.agents.explain(id, &sim.fire),
        Target::Person(id) => sim.agents.explain_person(id, &sim.fire),
        Target::Unit(id) => sim.crews.explain(id, &sim.agents.network, &sim.fire, &sim.scenario),
        Target::Traveller(_) => None,
    };
    let decision = explained.as_ref().map(|(d, _)| *d).unwrap_or(decision);

    ui.separator();
    ui.horizontal(|ui| {
        ui.strong("Behaviour");
        ui.label(&subtype_name).on_hover_text(&subtype_id);
        // The one door from the map into the editor. Everything else about
        // watching a behaviour run is in there; without this the feature is
        // reachable only by someone who already knows it exists.
        if ui
            .small_button("Open in the composer")
            .on_hover_text(
                "Open the Agent Behaviour Composer on this agent's graph and watch it decide.",
            )
            .clicked()
        {
            *open = true;
        }
    });
    ui.label(format!(
        "{}  ·  priority {:.2}",
        decision.action.label(),
        decision.priority
    ));
    if decision.prep_scale != 1.0 {
        ui.small(format!("preparation ×{:.2}", decision.prep_scale));
    }
    if decision.urgency > 0.0 {
        ui.small(format!("urgency readout {:.2}", decision.urgency));
    }

    egui::CollapsingHeader::new("Why").default_open(false).show(ui, |ui| {
        let Some((_, trace)) = &explained else {
            ui.small("(no explanation available)");
            return;
        };
        if trace.proposals.is_empty() {
            ui.small("No branch of the behaviour fired: the agent is doing its default.");
        } else {
            for (i, (node, kind, prio)) in trace.proposals.iter().enumerate() {
                ui.small(format!(
                    "{} {} @ {prio:.2} (node #{node})",
                    if i == 0 { "▶" } else { " " },
                    kind.label()
                ));
            }
        }
        ui.separator();
        // Which boxes actually produced the answer, rather than all of them.
        // The full list is in the composer's Live tab; here it is the short
        // version, and the short version is the slice.
        let active = trace.active();
        for n in &trace.nodes {
            // Observations are already on the panel above; what is worth
            // reading here is what the behaviour *made* of them.
            if n.category == behavior::Category::Observation {
                continue;
            }
            let values =
                n.outputs.iter().map(behavior::Value::display).collect::<Vec<_>>().join(", ");
            let mark = if active.contains(&n.node) { "▸" } else { " " };
            ui.small(format!("{mark} {} = {values}", n.name));
        }
        ui.small("▸ marks the boxes that fed the decision that was taken.");
    });
}

/// What this agent's own row in the run's [`crate::history::History`] says
/// happened to it, oldest first. Always shown, unlike the Behaviour section
/// above: it has something to say for hand-written agents too, and it is the
/// same log an "interview an agent" feature would read from later.
fn history_panel(ui: &mut egui::Ui, sim: &Sim, target: Target) {
    let subject = crate::history::subject_of(target);
    let events = sim.history.log.events_for(subject);

    ui.separator();
    egui::CollapsingHeader::new(format!("History ({})", events.len()))
        .default_open(false)
        .show(ui, |ui| {
            if events.is_empty() {
                ui.small("Nothing recorded yet.");
                return;
            }
            const SHOWN: usize = 40;
            let skip = events.len().saturating_sub(SHOWN);
            if skip > 0 {
                ui.small(format!("… {skip} earlier"));
            }
            for e in &events[skip..] {
                ui.small(format!(
                    "{}  {}",
                    mmss(e.sim_time_s as f32),
                    crate::history::summarize(&e.kind, &e.detail)
                ));
            }
        });
}

fn person_panel(ui: &mut egui::Ui, sim: &Sim, id: usize, jump_to: &mut Option<Target>) {
    let Some(p) = sim.agents.people.get(id) else {
        ui.label("(gone)");
        return;
    };

    egui::Grid::new("person").num_columns(2).show(ui, |ui| {
        ui.label("Status");
        ui.label(status_text(p.status));
        ui.end_row();
        ui.label("Age");
        ui.label(format!("{}", p.age));
        ui.end_row();
        ui.label("Walk speed");
        ui.label(format!("{:.1} m/s", p.walk_speed));
        ui.end_row();
        ui.label("Needs assistance");
        ui.label(if p.needs_assistance { "yes" } else { "no" });
        ui.end_row();

        // The one thing that decides whether this person is an agent at all.
        // Everyone else leaves with the family, and the household is what
        // decides; only someone away from it has choices of their own.
        ui.label("With the household");
        ui.label(if p.away { "no — away from home" } else { "yes" });
        ui.end_row();

        if p.away {
            ui.label("Own alarm");
            ui.label(format!("{:.0}%", p.cue * 100.0));
            ui.end_row();

            if let Some(t) = p.traveller.and_then(|ti| sim.agents.travellers.get(ti)) {
                ui.label("Heading");
                ui.label(match t.goal {
                    abm::Goal::Refuge => "out, to the nearest refuge",
                    abm::Goal::Home => "back to the house",
                });
                ui.end_row();
            }
        }
    });

    ui.separator();
    if ui
        .button(format!("Inspect household #{}", p.household))
        .clicked()
    {
        *jump_to = Some(Target::Household(p.household));
    }
    if let Some(ti) = p.traveller {
        if ui.button("Inspect the group on the move").clicked() {
            *jump_to = Some(Target::Traveller(ti));
        }
    }
}

fn traveller_panel(
    ui: &mut egui::Ui,
    sim: &Sim,
    i: usize,
    jump_to: &mut Option<Target>,
    mode: &mut crate::camera::CameraMode,
) {
    let Some(t) = sim.agents.travellers.get(i) else {
        ui.label("(gone)");
        return;
    };

    egui::Grid::new("traveller").num_columns(2).show(ui, |ui| {
        ui.label("Party");
        ui.label(if t.solo {
            "one person, caught away from home".to_string()
        } else {
            format!("household #{} ({} people)", t.household, t.members.len())
        });
        ui.end_row();

        ui.label("Mode");
        ui.label(match t.mode {
            Mode::Car => "car",
            Mode::Foot => "on foot",
        });
        ui.end_row();

        ui.label("State");
        ui.label(travel_state_text(t.state));
        ui.end_row();

        ui.label("Speed");
        ui.label(format!("{:.1} m/s", t.free_speed));
        ui.end_row();

        ui.label("Distance so far");
        ui.label(format!("{:.0} m", t.distance_m));
        ui.end_row();

        ui.label("Departed");
        ui.label(format!(
            "{} ago ({})",
            mmss(sim.agents.time_s() - t.departed_s),
            mmss(t.departed_s)
        ));
        ui.end_row();

        if t.heat_s > 0.0 {
            ui.label("Heat exposure");
            ui.label(format!("{:.0} s", t.heat_s));
            ui.end_row();
        }
    });

    if !t.solo {
        ui.separator();
        if ui
            .button(format!("Inspect household #{}", t.household))
            .clicked()
        {
            *jump_to = Some(Target::Household(t.household));
        }
    }

    camera_controls(ui, mode, Target::Traveller(i));
}

/// A suppression unit's own account of itself: what it is, what it has been
/// told to do, and what it has actually achieved — the same numbers as the
/// Resources panel's hover text (`crate::command::panel`), but reachable by
/// clicking the unit on the map rather than finding it in the roster list.
fn unit_panel(ui: &mut egui::Ui, sim: &Sim, id: usize, mode: &mut crate::camera::CameraMode) {
    use abm::suppression::{Task, UnitKind, UnitState};

    let Some(u) = sim.crews.units.get(id) else {
        ui.label("(gone)");
        return;
    };

    egui::Grid::new("unit").num_columns(2).show(ui, |ui| {
        ui.label("Callsign");
        ui.label(&u.callsign);
        ui.end_row();

        ui.label("Kind");
        ui.label(u.kind.label());
        ui.end_row();

        ui.label("State");
        ui.label(crate::command::status_line(sim, id));
        ui.end_row();

        ui.label("Order");
        ui.label(match u.task {
            Task::Hold => "hold position".to_string(),
            Task::Return => "returning to base".to_string(),
            Task::Attack { .. } => "direct attack".to_string(),
            Task::Drop { .. } => "drop run".to_string(),
            Task::Line { from, to } => {
                let total = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
                format!("cutting line — {:.0}/{:.0} m done", u.line_done_m, total)
            }
        });
        ui.end_row();

        match u.kind {
            UnitKind::Engine => {
                ui.label("Tank");
                ui.label(format!("{:.0} of {:.0} L", u.water_l, u.tank_l));
                ui.end_row();
                ui.label("Delivered");
                ui.label(format!("{:.0} L", u.water_used_l));
                ui.end_row();
                ui.label("Hose reach");
                ui.label(format!(
                    "{:.0} m of a road it can reach",
                    abm::suppression::ENGINE_REACH_M
                ));
                ui.end_row();
            }
            UnitKind::HandCrew => {
                ui.label("Line cut");
                ui.label(format!("{:.0} m", u.line_cut_m));
                ui.end_row();
                ui.label("Production rate");
                ui.label(format!("{:.0} m/h", abm::suppression::LINE_M_PER_H));
                ui.end_row();
            }
            UnitKind::AirTanker => {
                ui.label("Drops");
                ui.label(format!("{}", u.drops));
                ui.end_row();
                ui.label("Delivered");
                ui.label(format!("{:.0} L", u.water_used_l));
                ui.end_row();
                ui.label("Load size");
                ui.label(format!("{:.0} L", u.tank_l));
                ui.end_row();
                if u.state == UnitState::Inbound {
                    ui.label("Arrives");
                    ui.label(mmss((u.arrives_at_s - sim.crews.time_s()).max(0.0)));
                    ui.end_row();
                }
            }
        }

        if u.heat_s > 0.0 {
            ui.label("Heat exposure");
            ui.label(format!("{:.0} s", u.heat_s));
            ui.end_row();
        }
    });

    if !u.note.is_empty() {
        ui.separator();
        ui.colored_label(egui::Color32::from_rgb(240, 180, 60), u.note);
    }

    camera_controls(ui, mode, Target::Unit(id));
}

/// Follow / first-person toggle, shared by the two targets that actually
/// travel continuously. A household never moves and a lone person is tracked
/// through its traveller, so those two panels have nothing to hand this.
fn camera_controls(ui: &mut egui::Ui, mode: &mut crate::camera::CameraMode, target: Target) {
    use crate::camera::CameraMode;

    ui.separator();
    ui.horizontal(|ui| {
        let following = *mode == CameraMode::Follow(target);
        if ui
            .selectable_label(following, "🎥 Follow")
            .on_hover_text(
                "Keep the camera's focus on this as it moves. Orbit and zoom still work.",
            )
            .clicked()
        {
            *mode = if following {
                CameraMode::Free
            } else {
                CameraMode::Follow(target)
            };
        }
        let riding = *mode == CameraMode::FirstPerson(target);
        if ui
            .selectable_label(riding, "👁 First person")
            .on_hover_text("Ride along, looking the way it's heading. Left-drag to look around.")
            .clicked()
        {
            *mode = if riding {
                CameraMode::Free
            } else {
                CameraMode::FirstPerson(target)
            };
        }
    });
    if *mode == CameraMode::FirstPerson(target) {
        ui.small("left-drag to look around · click again, or esc, to return to the orbit view");
    }
}

pub(crate) fn status_text(s: Status) -> &'static str {
    match s {
        Status::Normal => "normal",
        Status::Warned => "warned",
        Status::Preparing => "preparing",
        Status::Evacuating => "evacuating",
        Status::Evacuated => "evacuated / safe",
        Status::Defending => "defending",
        Status::Trapped => "trapped",
        Status::Casualty => "casualty",
    }
}

pub(crate) fn travel_state_text(s: TravelState) -> &'static str {
    match s {
        TravelState::Approaching => "walking to the road",
        TravelState::OnNetwork => "on the network",
        TravelState::Safe => "at a refuge / off the map",
        TravelState::Cutoff => "cut off — no route out",
        TravelState::Arrived => "reached home",
        TravelState::Dead => "casualty",
    }
}

fn channel_text(c: scenario::population::WarningChannel) -> &'static str {
    use scenario::population::WarningChannel::*;
    match c {
        MobileAlert => "mobile alert",
        Siren => "siren",
        Neighbour => "neighbour",
        SelfObserved => "self-observed",
        None => "none",
    }
}

fn mmss(s: f32) -> String {
    let s = s.max(0.0) as i64;
    format!("{}:{:02}", s / 60, s % 60)
}
