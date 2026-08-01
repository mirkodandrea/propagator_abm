//! The Entities browser: a searchable, filterable roster of everything
//! inspectable, and a way to jump the camera to any of it.
//!
//! `inspect::pick_click` only reaches what is actually drawn on screen — a
//! person indoors, or a household that has already evacuated, has no figure
//! to click. This panel is built from the model directly instead of from
//! screen-space picking, using the same [`Target`] enum and the same
//! [`Selected`] resource `inspect` already has, so nothing in the roster is
//! ever unreachable and picking either way opens the same Inspector.

use bevy::prelude::*;
use bevy_egui::egui;

use crate::camera::OrbitCamera;
use crate::frame;
use crate::inspect::{target_pos, Selected, Target};
use crate::sim::Sim;

/// How close the camera lands on "jump to". Close enough to make out a
/// person-scale figure, far enough not to end up inside a building or a
/// tree.
const ZOOM_DISTANCE_M: f32 = 220.0;

#[derive(Resource)]
pub struct BrowserUi {
    pub search: String,
    pub focus_search: bool,
    pub show_households: bool,
    /// Off by default: 1,577 people is most of the roster, and the household
    /// they belong to is usually the more useful unit to browse.
    pub show_people: bool,
    pub show_travellers: bool,
    pub show_units: bool,
}

impl Default for BrowserUi {
    fn default() -> Self {
        Self {
            search: String::new(),
            focus_search: false,
            show_households: true,
            show_people: false,
            show_travellers: true,
            show_units: true,
        }
    }
}

/// `B` brings the Entities tab forward. It used to *also* open the behaviour
/// composer, which shared the binding — both fired, so the composer opened over
/// a browser that had just changed state underneath it. The composer is now `G`.
pub fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<crate::ui::UiFocus>,
    mut panels: ResMut<crate::ui::PanelState>,
    mut browser: ResMut<BrowserUi>,
) {
    if focus.typing() {
        return;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        if panels.tab == crate::ui::DockTab::Entities && panels.dock.visible() {
            panels.dock = crate::ui::PanelPlacement::Hidden;
        } else {
            panels.focus_tab(crate::ui::DockTab::Entities);
            browser.focus_search = true;
        }
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if keys.just_pressed(KeyCode::Slash) && !shift {
        panels.focus_tab(crate::ui::DockTab::Entities);
        browser.focus_search = true;
    }
}

/// The **Entities** tab: filters, a virtualised list (`show_rows`, so a roster
/// of a couple of thousand people costs only as many egui widgets as are
/// actually on screen), and a click that both selects and recentres the
/// camera — the two things "find this entity" always means together.
pub fn entities_body(
    ui: &mut egui::Ui,
    browser: &mut BrowserUi,
    selected: &mut Selected,
    sim: &Sim,
    camera: &mut Query<&mut OrbitCamera>,
) -> bool {
    let mut jump_to: Option<Target> = None;

    ui.horizontal(|ui| {
        ui.label("🔍");
        // Takes the width it can get: this is the control the tab exists for,
        // and a search box you have to aim at is one you stop using.
        let search = ui.add(
            egui::TextEdit::singleline(&mut browser.search)
                .hint_text("id, status, callsign…")
                .desired_width(ui.available_width() - 28.0),
        );
        if browser.focus_search {
            search.request_focus();
            browser.focus_search = false;
        }
        if ui.small_button("×").on_hover_text("Clear search").clicked() {
            browser.search.clear();
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut browser.show_households, "Households");
        ui.checkbox(&mut browser.show_people, "People");
        ui.checkbox(&mut browser.show_travellers, "On the move");
        ui.checkbox(&mut browser.show_units, "Units");
    });

    let rows = build_rows(sim, browser);
    ui.add_space(4.0);
    crate::ui::section(ui, &format!("{} of {}", rows.len(), total_count(sim)));

    let row_h = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
    egui::ScrollArea::vertical()
        .id_source("entities_rows")
        .auto_shrink([false, false])
        .show_rows(ui, row_h, rows.len(), |ui, range| {
            for &t in &rows[range] {
                let is_selected = selected.target == Some(t);
                let label = row_label(sim, t);
                let resp = ui.selectable_label(is_selected, label);
                if resp.clicked() {
                    jump_to = Some(t);
                }
            }
        });

    if let Some(t) = jump_to {
        selected.target = Some(t);
        if let (Some(pos), Ok(mut orbit)) = (target_pos(sim, t), camera.get_single_mut()) {
            let ground = sim.scenario.terrain.height_at(pos);
            orbit.focus = frame::to_bevy(pos, ground);
            orbit.distance = orbit.distance.clamp(ZOOM_DISTANCE_M * 0.5, ZOOM_DISTANCE_M);
        }
        true
    } else {
        false
    }
}

fn total_count(sim: &Sim) -> usize {
    sim.agents.households.len()
        + sim.agents.people.len()
        + sim.agents.travellers.len()
        + sim.crews.units.len()
}

/// The filtered roster, cheapest test first: the kind checkboxes gate whole
/// collections before the search string is even looked at.
fn build_rows(sim: &Sim, browser: &BrowserUi) -> Vec<Target> {
    let q = browser.search.trim().to_lowercase();
    let mut rows = Vec::new();

    if browser.show_households {
        rows.extend(
            sim.agents
                .households
                .iter()
                .filter(|h| matches_household(h, &q))
                .map(|h| Target::Household(h.id)),
        );
    }
    if browser.show_people {
        rows.extend(
            sim.agents
                .people
                .iter()
                .filter(|p| matches_person(p, &q))
                .map(|p| Target::Person(p.id)),
        );
    }
    if browser.show_travellers {
        rows.extend(
            sim.agents
                .travellers
                .iter()
                .enumerate()
                .filter_map(|(i, t)| matches_traveller(t, i, &q).then_some(Target::Traveller(i))),
        );
    }
    if browser.show_units {
        rows.extend(
            sim.crews
                .units
                .iter()
                .filter(|u| matches_unit(u, &q))
                .map(|u| Target::Unit(u.id)),
        );
    }
    rows
}

fn matches_household(h: &abm::HouseholdAgent, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    h.id.to_string().contains(q) || crate::inspect::status_text(h.status).contains(q)
}

fn matches_person(p: &abm::PersonAgent, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    p.id.to_string().contains(q) || crate::inspect::status_text(p.status).contains(q)
}

fn matches_traveller(t: &abm::Traveller, i: usize, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    i.to_string().contains(q)
        || t.household.to_string().contains(q)
        || crate::inspect::travel_state_text(t.state).contains(q)
        || matches!(t.mode, abm::Mode::Car) && "car".contains(q)
        || matches!(t.mode, abm::Mode::Foot) && "foot".contains(q)
}

fn matches_unit(u: &abm::Unit, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    u.callsign.to_lowercase().contains(q)
        || u.kind.label().contains(q)
        || u.state.label().contains(q)
}

fn row_label(sim: &Sim, t: Target) -> String {
    match t {
        Target::Household(id) => match sim.agents.households.get(id) {
            Some(h) => format!("🏠 #{id}  {}", crate::inspect::status_text(h.status)),
            None => format!("🏠 #{id}  (gone)"),
        },
        Target::Person(id) => match sim.agents.people.get(id) {
            Some(p) => {
                format!(
                    "🚶 #{id}  {}, age {}",
                    crate::inspect::status_text(p.status),
                    p.age
                )
            }
            None => format!("🚶 #{id}  (gone)"),
        },
        Target::Traveller(i) => match sim.agents.travellers.get(i) {
            Some(t) => {
                let who = if t.solo {
                    "solo".to_string()
                } else {
                    format!("household #{}", t.household)
                };
                format!("🚗 {who}  {}", crate::inspect::travel_state_text(t.state))
            }
            None => "🚗 (gone)".to_string(),
        },
        Target::Unit(id) => match sim.crews.units.get(id) {
            Some(u) => format!(
                "🚒 {}  {}",
                u.callsign,
                crate::command::status_line(sim, id)
            ),
            None => format!("🚒 #{id}  (gone)"),
        },
    }
}
