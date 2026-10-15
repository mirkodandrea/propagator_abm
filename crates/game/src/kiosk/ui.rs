//! The kiosk's interface (egui): few words, big buttons (docs/01-GIOCO.md §6).
//!
//! - top bar: day, clock, wind and fuel moisture, the pause («Chiedi
//!   all'esperto») with its countdown or its recharge;
//! - side bar: the teams and the districts, and what is selected: the move or
//!   the evacuation with its time, to confirm;
//! - on the map: a badge per team and per district (clickable), the notice
//!   of the last event;
//! - the tutorial's step, the opening, day 2 and the end screens.
//!
//! Every order goes through the game (`rocca::Game::move_team`,
//! `rocca::Game::evacuate`), which refuses what its rules do not allow; the
//! tutorial also refuses what its step does not ask (`Kiosk::allows`).

use bevy::prelude::*;
use bevy_egui::egui::{self, Align2, Color32, RichText};
use bevy_egui::EguiContexts;
use rocca::tutorial::Expect;
use scenario::Pos;

use super::characters::{Logo, Portraits, Qr};
use super::{board, new_visit, screens, start_game, Kiosk, Phase, Selection};
use crate::sim::{Sim, SimRestarted};

const PANEL: Color32 = Color32::from_rgba_premultiplied(18, 20, 24, 245);
pub(super) const AMBER: Color32 = Color32::from_rgb(250, 195, 70);
pub(super) const RED: Color32 = Color32::from_rgb(255, 105, 85);
const BLUE: Color32 = Color32::from_rgb(110, 185, 255);
const GREEN: Color32 = Color32::from_rgb(110, 230, 150);
/// The evacuation's colour, the same as its route on the map (`marks::EVAC`).
const EVAC: Color32 = Color32::from_rgb((super::marks::EVAC[0] * 255.0) as u8, (super::marks::EVAC[1] * 255.0) as u8, (super::marks::EVAC[2] * 255.0) as u8);
pub(super) const GREY: Color32 = Color32::from_rgb(205, 208, 212);
/// The fire brigade's red and the volunteers' yellow-green, as on the map.
pub const VVF: Color32 = Color32::from_rgb(225, 60, 50);
pub const AIB: Color32 = Color32::from_rgb(200, 215, 60);

/// Use the same layout in either orientation, including short phone viewports.
pub(super) fn compact(ctx: &egui::Context) -> bool {
    let r = ctx.screen_rect();
    r.width() < 1000.0 || r.height() < 600.0
}

fn clock(s: i64) -> String {
    rocca::words::clock(s)
}

pub(super) fn secondary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).size(17.0).color(Color32::WHITE))
        .fill(Color32::from_gray(62))
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(110)))
        .rounding(8.0)
        .min_size(egui::vec2(44.0, 40.0))
}

pub(super) fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).size(20.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(10.0).min_size(egui::vec2(140.0, 46.0))
}

fn panel() -> egui::Frame {
    egui::Frame::none().fill(PANEL).rounding(8.0).inner_margin(egui::Margin::symmetric(12.0, 8.0))
}

pub(super) fn card(tint: Color32) -> egui::Frame {
    egui::Frame::none().fill(PANEL).stroke(egui::Stroke::new(1.5, tint.gamma_multiply(0.8))).rounding(14.0).inner_margin(18.0)
}

/// World point to screen point, `lift_m` above the ground.
pub(super) fn screen(cam: (&Camera, &GlobalTransform), sim: &Sim, p: Pos, lift_m: f32) -> Option<egui::Pos2> {
    let v = crate::frame::to_bevy(p, sim.scn.terrain.height_at(p) + lift_m);
    let s = cam.0.world_to_viewport(cam.1, v)?;
    Some(egui::pos2(s.x, s.y))
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut kiosk: ResMut<Kiosk>,
    mut sim: ResMut<Sim>,
    mut focus: ResMut<crate::ui::UiFocus>,
    mut restarted: EventWriter<SimRestarted>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    mut gfx: ResMut<crate::quality::Graphics>,
    faces: Option<Res<Portraits>>,
    logo: Option<Res<Logo>>,
    qr: Option<Res<Qr>>,
) {
    crate::playtest::begin();
    let ctx = contexts.ctx_mut();
    let k = &mut *kiosk;
    let Ok(cam) = cameras.get_single() else { return };
    if ctx.style().visuals.interact_cursor.is_none() {
        ctx.style_mut(|s| {
            s.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
            s.animation_time = 0.0;
            s.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(3.0, Color32::WHITE);
        });
    }
    if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::F2)) {
        k.operator = !k.operator;
    }
    egui::Area::new(egui::Id::new("menu_btn")).anchor(Align2::LEFT_BOTTOM, [10.0, -10.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        let r = ui.add(secondary(if k.operator { "Chiudi menu" } else { "Menu" }));
        crate::playtest::control("Menu", &r, true);
        if r.clicked() {
            k.operator = !k.operator;
        }
    });
    if k.operator {
        operator(ctx, k, &mut sim, &mut restarted, &mut gfx);
    }

    match k.phase {
        Phase::Intro => {
            k.side_w = 0.0;
            if !compact(ctx) { banner(ctx, qr.as_deref()); }
            if k.title {
                if screens::title(ctx, k, logo.as_deref()) {
                    k.title = false;
                }
            } else {
                screens::intro(ctx, k, &mut sim, &mut restarted, logo.as_deref(), faces.as_deref());
            }
        }
        Phase::Giorno2 => {
            k.side_w = 0.0;
            screens::day_two(ctx, k, &mut sim, &mut restarted, faces.as_deref());
        }
        Phase::Fine => {
            k.side_w = 0.0;
            if cfg!(target_arch = "wasm32") && !compact(ctx) {
                banner(ctx, None);
            }
            screens::end(ctx, k, &mut sim, &mut restarted, faces.as_deref());
        }
        Phase::Tutorial | Phase::Gioco => {
            let r = ctx.screen_rect();
            k.side_w = if compact(ctx) && r.width() > r.height() && k.phase == Phase::Tutorial { 316.0 } else { 0.0 };
            top_bar(ctx, k, &mut sim, cam);
            map_badges(ctx, k, &mut sim, cam);
            map_click(k, &mut sim);
            if k.phase == Phase::Tutorial {
                screens::tutorial(ctx, k, &mut sim, &mut restarted, faces.as_deref(), cam);
            }
            notice(ctx, k);
            if compact(ctx) {
                egui::Area::new(egui::Id::new("map_tools")).anchor(Align2::RIGHT_BOTTOM, [-8.0, -8.0]).order(egui::Order::Foreground).show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        for (label, zoom) in [("−", -1.0), ("+", 1.0)] {
                            let r = ui.add(secondary(label).min_size(egui::vec2(44.0, 44.0)));
                            crate::playtest::control(label, &r, true);
                            if r.clicked() { k.zoom_steps += zoom; }
                        }
                        let r = ui.add(secondary("Isola"));
                        crate::playtest::control("Isola", &r, true);
                        if r.clicked() { k.reset_view = true; }
                    });
                });
            }
        }
    }
    focus.pointer = ctx.is_pointer_over_area();
    k.click = None;
}

/// The Google Form for the feedback of those who play in the browser, with the
/// «Versione del gioco» answer filled in by the build's commit.
fn feedback_url() -> String {
    format!("https://docs.google.com/forms/d/e/1FAIpQLSfHWmJHJwLLzqLmjomK-HA3YikUQfw4wz5L9cSjqpgS4i1ipw/viewform?usp=pp_url&entry.2126742218={}", super::BUILD)
}

/// The banner at the top right. In the kiosk, «Provami!» with the QR code of
/// the browser version, on the title and the opening, where a passer-by is
/// looking at the screen. In the browser, which is already the thing the QR
/// points to, a button to the feedback form, on the same screens and at the end.
fn banner(ctx: &egui::Context, qr: Option<&Qr>) {
    egui::Area::new(egui::Id::new("provami")).anchor(Align2::RIGHT_TOP, [-14.0, 14.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        card(AMBER).inner_margin(10.0).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                if cfg!(target_arch = "wasm32") {
                    ui.label(RichText::new("Dicci cosa ne pensi").size(20.0).strong().color(AMBER));
                    ui.add_space(4.0);
                    if ui.add(primary("Dai un feedback")).clicked() {
                        ctx.open_url(egui::OpenUrl::new_tab(feedback_url()));
                    }
                } else if let Some(qr) = qr {
                    ui.label(RichText::new("Provami!").size(26.0).strong().color(AMBER));
                    ui.label(RichText::new("Gioca dal tuo computer").size(15.0).color(GREY));
                    ui.add_space(4.0);
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(qr.0, egui::vec2(150.0, 150.0))));
                }
            });
        });
    });
}

// --- the pause and the weather ------------------------------------------------

fn top_bar(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, cam: (&Camera, &GlobalTransform)) {
    if compact(ctx) {
        compact_top(ctx, k, sim, cam);
        return;
    }
    egui::Area::new(egui::Id::new("barra")).anchor(Align2::LEFT_TOP, [10.0, 10.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.horizontal(|ui| {
                pause_dial(ui, k, sim);
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.set_min_width(300.0);
                    let day = if k.phase == Phase::Tutorial { "Giorno 1" } else { "Giorno 2" };
                    ui.label(RichText::new(format!("{day} · {}", clock(sim.time_s()))).size(34.0).strong().color(Color32::WHITE));
                    // the scenario's progress: elapsed time as a bar, with the end hour
                    let frac = (sim.time_s() as f32 / sim.case.duration_s().max(1) as f32).clamp(0.0, 1.0);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(clock(0)).size(16.0).color(GREY));
                        let w = (ui.available_width() - 70.0).max(40.0);
                        let (r, _) = ui.allocate_exact_size(egui::vec2(w, 18.0), egui::Sense::hover());
                        ui.painter().rect_filled(r, 7.0, Color32::from_gray(60));
                        ui.painter().rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(r.width() * frac, r.height())), 7.0, AMBER);
                        ui.label(RichText::new(clock(sim.case.duration_s())).size(16.0).color(GREY));
                    });
                });
            });
        });
    });
    conditions(ctx, sim, cam);
}

/// Wind and fuel moisture, big, bottom right. The wind's arrow and the north
/// mark turn with the camera, so they match what the map shows.
fn conditions(ctx: &egui::Context, sim: &Sim, cam: (&Camera, &GlobalTransform)) {
    use super::icons::{self, Icon};
    let w = sim.weather();
    egui::Area::new(egui::Id::new("condizioni")).anchor(Align2::RIGHT_BOTTOM, [-12.0, -12.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.horizontal(|ui| {
                let size = 84.0;
                let (r, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                let c = r.center();
                ui.painter().circle(c, size * 0.5, Color32::from_black_alpha(90), egui::Stroke::new(2.0, Color32::from_gray(110)));
                // north on the screen, from the camera
                let right: Vec3 = cam.1.right().into();
                let fwd: Vec3 = cam.1.forward().into();
                let fwd = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero();
                let north = Vec3::new(0.0, 0.0, -1.0);
                let n = egui::vec2(north.dot(right), -north.dot(fwd)).normalized();
                ui.painter().text(c + n * size * 0.40, Align2::CENTER_CENTER, "N", egui::FontId::proportional(15.0), GREY);
                let mut inner = ui.child_ui(r.shrink(size * 0.17), *ui.layout(), None);
                wind_arrow(&mut inner, cam, w.wind_dir_deg, size * 0.66, Color32::WHITE);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("{:.0} km/h", w.wind_speed_kmh)).size(32.0).strong().color(Color32::WHITE));
                    ui.label(RichText::new(format!("da {}", rocca::words::compass(w.wind_dir_deg))).size(15.0).color(GREY));
                });
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        icons::show(ui, Icon::Drop, 30.0, BLUE);
                        ui.label(RichText::new(format!("{:.0} %", w.moisture_pct)).size(32.0).strong().color(Color32::WHITE));
                    });
                    ui.label(RichText::new("umidità").size(15.0).color(GREY));
                });
            });
            // the forecast, while paused: the lines' legend and the wind to come
            if sim.paused() {
                ui.separator();
                ui.horizontal(|ui| {
                    for (col, text) in [(crate::forecast_view::NEAR_COLOUR, "1 h"), (crate::forecast_view::FAR_COLOUR, "3 h")] {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(30.0, 24.0), egui::Sense::hover());
                        ui.painter().line_segment([r.left_center(), r.right_center()], egui::Stroke::new(7.0, col));
                        ui.label(RichText::new(text).size(20.0).strong().color(col));
                        ui.add_space(6.0);
                    }
                    if let Some((at, w2)) = sim.weather_ahead() {
                        ui.add_space(10.0);
                        icons::show(ui, Icon::Clock, 24.0, AMBER);
                        ui.label(RichText::new(clock(at)).size(20.0).strong().color(AMBER));
                        wind_arrow(ui, cam, w2.wind_dir_deg, 30.0, AMBER);
                        ui.label(RichText::new(format!("{:.0} km/h", w2.wind_speed_kmh)).size(20.0).strong().color(AMBER));
                    }
                });
            }
        });
    });
}

/// An arrow the way the wind blows, as seen on the map from the camera.
fn wind_arrow(ui: &mut egui::Ui, cam: (&Camera, &GlobalTransform), from_deg: f64, size: f32, col: Color32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let to = (from_deg as f32 + 180.0).to_radians();
    // world east/north to Bevy (x, -z), then onto the screen's right and up
    let d = Vec3::new(to.sin(), 0.0, -to.cos());
    let right: Vec3 = cam.1.right().into();
    let fwd: Vec3 = cam.1.forward().into();
    let fwd = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero();
    let v = egui::vec2(d.dot(right), -d.dot(fwd)).normalized();
    let c = r.center();
    let (tip, tail) = (c + v * size * 0.45, c - v * size * 0.45);
    let n = egui::vec2(-v.y, v.x);
    let p = ui.painter();
    p.line_segment([tail, tip - v * size * 0.2], egui::Stroke::new(size * 0.12, col));
    p.add(egui::Shape::convex_polygon(vec![tip, tip - v * size * 0.35 + n * size * 0.22, tip - v * size * 0.35 - n * size * 0.22], col, egui::Stroke::NONE));
}

/// An arc from the top, clockwise, over `frac` of the circle.
fn arc(p: &egui::Painter, c: egui::Pos2, radius: f32, from: f32, frac: f32, stroke: egui::Stroke) {
    let n = ((frac * 64.0).ceil() as usize).max(1);
    let pts: Vec<egui::Pos2> = (0..=n)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * (from + frac.clamp(0.0, 1.0) * i as f32 / n as f32);
            c + egui::vec2(a.cos(), a.sin()) * radius
        })
        .collect();
    p.add(egui::Shape::line(pts, stroke));
}

const DIAL: f32 = 112.0;

/// The pause, a big round button: ready (amber ring, pause symbol), in pause
/// (no time limit: play symbol, green ring) or recharging (the ring fills
/// and the number counts down the seconds to the next pause).
fn pause_dial(ui: &mut egui::Ui, k: &mut Kiosk, sim: &mut Sim) {
    let dial = if compact(ui.ctx()) { 56.0 } else { DIAL };
    use super::icons::{self, Icon};
    let paused = sim.paused();
    let enabled = if paused { k.allows(Expect::Resume) } else { sim.can_pause() && k.allows(Expect::Pause) };
    let (r, resp) = ui.allocate_exact_size(egui::vec2(dial, dial), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let p = ui.painter();
    let c = r.center();
    let radius = dial * 0.5 - 7.0;
    let hot = resp.hovered() && enabled;
    p.circle_filled(c, dial * 0.5, if hot { Color32::from_gray(60) } else { Color32::from_black_alpha(90) });
    p.circle_stroke(c, radius, egui::Stroke::new(10.0, Color32::from_gray(55)));
    let (frac, ring, icon, icon_col, secs) = if paused {
        (1.0, GREEN, Icon::Play, GREEN, None)
    } else if sim.pause_ready_in_s() > 0 {
        let left = sim.pause_ready_in_s() as f32 / rocca::RUN_X;
        (1.0 - left / rocca::PAUSE_COOLDOWN_REAL_S, AMBER, Icon::Pause, Color32::from_gray(150), Some(left))
    } else {
        (1.0, AMBER, Icon::Pause, AMBER, None)
    };
    arc(p, c, radius, 0.0, frac, egui::Stroke::new(10.0, ring));
    let icon_c = if secs.is_some() { c - egui::vec2(0.0, if compact(ui.ctx()) { 8.0 } else { 20.0 }) } else { c };
    icons::draw(p, egui::Rect::from_center_size(icon_c, egui::vec2(dial * if secs.is_some() { 0.26 } else { 0.36 }, dial * if secs.is_some() { 0.26 } else { 0.36 })), icon, icon_col);
    if let Some(sec) = secs {
        let g = p.layout_no_wrap(format!("{} s", sec.ceil() as i64), egui::FontId::proportional(if compact(ui.ctx()) { 16.0 } else { 30.0 }), Color32::WHITE);
        p.galley(c + egui::vec2(-g.size().x / 2.0, 4.0), g, Color32::WHITE);
    }
    if hot {
        p.circle_stroke(c, dial * 0.5, egui::Stroke::new(2.5, Color32::WHITE));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    crate::playtest::control(if paused { "Riprendi" } else { "Pausa" }, &resp, enabled);
    if resp.clicked() {
        if paused {
            sim.resume();
            k.clear_selection();
            k.done(Expect::Resume);
        } else {
            match sim.pause() {
                Ok(()) => k.done(Expect::Pause),
                Err(e) => k.notify(e),
            }
        }
    }
}

/// A small icon and a short text, side by side.
fn chip(ui: &mut egui::Ui, icon: super::icons::Icon, text: &str, col: Color32) {
    super::icons::show(ui, icon, 22.0, col);
    ui.add(egui::Label::new(RichText::new(text).size(18.0).strong().color(col)).wrap());
}

fn select_team(k: &mut Kiosk, sim: &Sim, t: usize) {
    if !k.allows(Expect::Select(t)) {
        k.notify(tutorial_hint(k));
        return;
    }
    k.clear_selection();
    k.selection = Selection::Team(t);
    k.follow = false;
    k.shot = None;
    k.done(Expect::Select(t));
    if sim.paused() && k.phase == Phase::Gioco { k.notify("Tocca l'esagono di destinazione, oppure trascina il cono."); }
    if k.phase == Phase::Gioco && !sim.paused() {
        k.notify("Per dare ordini premi «Pausa».");
    }
}

fn select_district(k: &mut Kiosk, sim: &Sim, d: usize) {
    if k.phase == Phase::Tutorial && !matches!(k.expect(), Some(Expect::Evacuate(e)) if e == d) {
        k.notify(tutorial_hint(k));
        return;
    }
    k.clear_selection();
    k.selection = Selection::District(d);
    k.evac_plan = Some(sim.plan_evacuation(d));
}

fn tutorial_hint(k: &Kiosk) -> String {
    match k.expect() {
        Some(Expect::Select(_)) | Some(Expect::Move(..)) => "Segui l'istruzione: la squadra e l'esagono indicati.".into(),
        Some(Expect::Evacuate(_)) => "Segui l'istruzione: il borgo indicato.".into(),
        _ => "Segui l'istruzione in basso.".into(),
    }
}

/// Preview the move of team `t` to the hex `at` (under the dragged flag, or
/// clicked): the tutorial refuses a hex its step does not ask.
fn preview_move(k: &mut Kiosk, sim: &Sim, t: usize, at: rocca::Axial) {
    if k.move_hex == Some(at) {
        return;
    }
    k.move_hex = Some(at);
    k.move_plan = Some(match k.expect() {
        Some(Expect::Move(_, want)) if at != want => Err("Porta la squadra sull'esagono evidenziato.".into()),
        _ => sim.plan_move(t, at),
    });
}

/// Send team `t` to hex `at` now: there is no confirmation (user,
/// 2026-10-14); an order the game refuses is told and forgotten.
fn give_move(k: &mut Kiosk, sim: &mut Sim, t: usize, at: rocca::Axial) {
    preview_move(k, sim, t, at);
    let result = match k.move_plan.clone() {
        Some(Ok(_)) if !k.allows(Expect::Move(t, at)) => Err(tutorial_hint(k)),
        Some(Ok(_)) => sim.move_team(t, at).map(|_| ()),
        Some(Err(e)) => Err(e),
        None => Ok(()),
    };
    match result {
        Ok(()) => k.done(Expect::Move(t, at)),
        Err(e) => k.notify(e),
    }
    k.clear_selection();
}

/// A click on the map with a team selected: the team goes to the hex under it.
fn map_click(k: &mut Kiosk, sim: &mut Sim) {
    let Some(p) = k.click else { return };
    if let Selection::Team(t) = k.selection {
        if sim.paused() {
            give_move(k, sim, t, rocca::Axial::of(p));
        }
    }
}

/// The ground under a screen point (egui logical px).
fn ground_at(sim: &Sim, cam: (&Camera, &GlobalTransform), p: egui::Pos2) -> Option<Pos> {
    let vp = crate::pick::window_to_viewport(cam.0, Vec2::new(p.x, p.y))?;
    let ray = cam.0.viewport_to_world(cam.1, vp)?;
    crate::pick::ray_ground(&sim.scn, ray.origin, *ray.direction)
}

/// A thin bar: dark track, `frac` (0-1) filled in `col`.
fn bar_fill(p: &egui::Painter, r: egui::Rect, frac: f32, col: Color32) {
    p.rect_filled(r, 3.0, Color32::from_black_alpha(200));
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        p.rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(r.width() * f, r.height())), 3.0, col);
    }
    p.rect_stroke(r, 3.0, egui::Stroke::new(1.0, Color32::from_white_alpha(120)));
}

/// District badges (clicked; the pointer over one shows its evacuation's
/// route and time), the teams' flags (dragged onto a hex, the order given on
/// release), the evacuation's «Evacua», and the evacuations' timers.
fn map_badges(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, cam: (&Camera, &GlobalTransform)) {
    let mut hovered = None;
    let mut rects = vec![None; sim.districts.len()];
    let mut occupied: Vec<egui::Rect> = vec![];
    let mut order: Vec<_> = (0..sim.districts.len()).collect();
    order.sort_by_key(|&d| !(k.selection == Selection::District(d) || k.expect() == Some(Expect::Evacuate(d))));
    for d in order {
        // A chosen team's destination takes priority over labels: town badges
        // must not intercept a tap or hide the hex under the finger.
        if compact(ctx) && matches!(k.selection, Selection::Team(_)) { continue; }
        let c = sim.districts[d].centre;
        let Some(mut s) = screen(cam, sim, c, 40.0) else { continue };
        let name = sim.districts[d].name.clone();
        if compact(ctx) {
            let anchor = s;
            let bounds = drop_rect(ctx, k).shrink(4.0);
            if !bounds.contains(anchor) { continue; }
            let width = ctx.fonts(|f| f.layout_no_wrap(name.clone(), egui::FontId::proportional(16.0), Color32::WHITE)).size().x + 18.0;
            let size = egui::vec2(width, 52.0);
            let mut placed = None;
            for dy in [26.0, 78.0, -30.0, -82.0, 130.0] {
                for dx in [0.0, -width * 0.55, width * 0.55] {
                    let center = egui::pos2((anchor.x + dx).clamp(bounds.left() + width * 0.5, (bounds.right() - width * 0.5).max(bounds.left() + width * 0.5)), anchor.y + dy);
                    let r = egui::Rect::from_center_size(center, size);
                    if bounds.contains_rect(r) && occupied.iter().all(|o| !o.intersects(r.expand(3.0))) {
                        placed = Some(r); break;
                    }
                }
                if placed.is_some() { break; }
            }
            let Some(r) = placed else { continue };
            s = r.center();
            occupied.push(r);
            ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("town_leaders")))
                .line_segment([anchor, s], egui::Stroke::new(1.5, BLUE));
        }
        let on = k.selection == Selection::District(d);
        let inner = egui::Area::new(egui::Id::new(("borgo", d))).fixed_pos(s).constrain(false).pivot(Align2::CENTER_CENTER).order(egui::Order::Middle).show(ctx, |ui| {
            let b = egui::Button::new(RichText::new(&name).size(16.0).strong().color(Color32::WHITE))
                .fill(if on { BLUE.gamma_multiply(0.6) } else { Color32::from_black_alpha(170) })
                .stroke(egui::Stroke::new(if on { 3.0 } else { 1.0 }, if on { Color32::WHITE } else { BLUE }))
                .rounding(6.0)
                .min_size(egui::vec2(44.0, if compact(ctx) { 44.0 } else { 0.0 }));
            let r = ui.add(b);
            // damage: the share of the homes the fire has reached
            let (hit, homes) = sim.district_damage(d);
            let bar = egui::Rect::from_min_size(r.rect.left_bottom() + egui::vec2(0.0, 2.0), egui::vec2(r.rect.width(), 6.0));
            let share = if homes == 0 { 0.0 } else { hit as f32 / homes as f32 };
            bar_fill(ui.painter(), bar, share, RED);
            ui.allocate_rect(bar, egui::Sense::hover());
            r
        });
        let r = inner.inner;
        rects[d] = Some(inner.response.rect);
        crate::playtest::control(&name, &r, true);
        if r.clicked() {
            select_district(k, sim, d);
        }
        if r.hovered() && ctx.dragged_id().is_none() && sim.evac.of(d).is_none() {
            hovered = Some(d);
        }
        evac_timer(ctx, sim, d, s - egui::vec2(88.0, 6.0));
    }
    // the evacuation under the pointer: its route (on the map) and its time
    match hovered {
        Some(d) => {
            if !matches!(&k.evac_hover, Some((h, g, _)) if *h == d && *g + 10 > sim.generation) {
                k.evac_hover = Some((d, sim.generation, sim.plan_evacuation(d)));
            }
        }
        // a still picture (`snap`) keeps the one it was given
        None if k.frozen => {}
        None => k.evac_hover = None,
    }
    if let Some(d) = k.evac_hover.as_ref().map(|h| h.0) {
        if let (Some(rect), true) = (rects[d], k.selection != Selection::District(d)) {
            use super::icons::Icon;
            egui::Area::new(egui::Id::new(("evacua_su", d))).fixed_pos(rect.left_bottom() + egui::vec2(0.0, 6.0)).order(egui::Order::Tooltip).interactable(false).show(ctx, |ui| {
                egui::Frame::none().fill(Color32::from_black_alpha(215)).rounding(6.0).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
                    ui.horizontal(|ui| match &k.evac_hover {
                        Some((_, _, Ok(p))) => {
                            chip(ui, Icon::Family, &p.people.to_string(), Color32::WHITE);
                            ui.add_space(6.0);
                            chip(ui, Icon::Clock, &format!("{:.0} min", p.total_min().ceil()), EVAC);
                        }
                        Some((_, _, Err(_))) => chip(ui, Icon::Alert, "strade chiuse", RED),
                        None => {}
                    });
                });
            });
        }
    }
    team_flags(ctx, k, sim, cam);
    confirm_bubble(ctx, k, sim, cam);
}

/// The handles of the teams' 3D flags (`pins`): in pause a flag is dragged
/// onto a hex and the team goes there when it is let go; a click on a flag
/// and then on a hex does the same.
fn team_flags(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, cam: (&Camera, &GlobalTransform)) {
    use super::pins;
    let right: Vec3 = cam.1.right().into();
    let feet = pins::feet(k, sim, cam.1.translation(), right);
    for t in 0..sim.teams.len() {
        let Some((foot, sc)) = feet[t] else { continue };
        // the cone's lid: the handle, with the team's engine and number on it
        let lift = crate::rings::RING_LIFT_M + (pins::LIFT_M + pins::CONE_H_M) * sc;
        let lid = crate::frame::to_bevy(foot, sim.scn.terrain.height_at(foot) + lift);
        let tip = crate::frame::to_bevy(foot, sim.scn.terrain.height_at(foot) + lift - pins::CONE_H_M * sc);
        let (Some(a), Some(b), Some(e)) =
            (cam.0.world_to_viewport(cam.1, lid), cam.0.world_to_viewport(cam.1, tip), cam.0.world_to_viewport(cam.1, lid + right * pins::CONE_R_M * sc))
        else {
            continue;
        };
        // the cone bobs and spins, so the handle covers the whole cone down to
        // the ground ring, with a margin: the point is as good a grip as the badge
        let ground = cam.0.world_to_viewport(cam.1, crate::frame::to_bevy(foot, sim.scn.terrain.height_at(foot) + crate::rings::RING_LIFT_M));
        let bob = pins::BOB_M * sc;
        let half_w = ((e.x - a.x).abs() + 6.0).max(22.0);
        let bottom = b.y.max(a.y + 10.0).max(ground.map_or(0.0, |g| g.y)) + 8.0;
        let flag = egui::Rect::from_min_max(egui::pos2(a.x - half_w, a.y - 18.0 - bob), egui::pos2(a.x + half_w, bottom));
        let col = if sim.teams[t].volunteers { AIB } else { VVF };
        let number = sim.teams[t].name.split_whitespace().last().unwrap_or("").to_string();
        let on = k.selection == Selection::Team(t);
        let lost = sim.crews.units[sim.teams[t].unit].state == abm::suppression::UnitState::Lost;
        let multi = ctx.input(|i| i.multi_touch().is_some_and(|m| m.num_touches > 1));
        let canceled = ctx.input(|i| i.raw.events.iter().any(|e| matches!(e, egui::Event::Touch { phase: egui::TouchPhase::Cancel, .. })));
        let may_drag = sim.paused() && !lost && !multi && !canceled;
        if (multi || canceled) && k.dragging.is_some() { k.clear_selection(); }
        let fire_d = if lost { None } else { sim.team_fire_distance_m(t) };
        if compact(ctx) && !drop_rect(ctx, k).intersects(flag) { continue; }
        let inner = egui::Area::new(egui::Id::new(("bandiera", t))).fixed_pos(flag.min).constrain(false).order(egui::Order::Middle).show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(flag.size(), egui::Sense::click_and_drag());
            let p = ui.painter();
            let badge = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 18.0 + bob), egui::vec2(56.0, 28.0));
            p.rect_filled(badge, 14.0, Color32::from_black_alpha(200));
            p.rect_stroke(badge, 14.0, egui::Stroke::new(if on || resp.hovered() { 2.5 } else { 1.5 }, if on || resp.hovered() { Color32::WHITE } else { col }));
            super::icons::draw(p, egui::Rect::from_center_size(badge.left_center() + egui::vec2(17.0, 0.0), egui::vec2(22.0, 22.0)), super::icons::Icon::Engine, col);
            let g = p.layout_no_wrap(number.clone(), egui::FontId::proportional(17.0), Color32::WHITE);
            p.galley(egui::pos2(badge.right() - 10.0 - g.size().x, badge.center().y - g.size().y / 2.0), g, Color32::WHITE);
            // distance from the fire: full while far, shrinking under the critical distance
            let bar = egui::Rect::from_min_size(badge.left_bottom() + egui::vec2(0.0, 3.0), egui::vec2(badge.width(), 6.0));
            if let Some(dm) = fire_d {
                let frac = (dm / rocca::TEAM_FIRE_CRIT_M).min(1.0);
                let col = if dm < rocca::TEAM_DANGER_M { RED } else if frac < 1.0 { AMBER } else { GREEN };
                bar_fill(p, bar, frac, col);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(if may_drag { egui::CursorIcon::Grab } else { egui::CursorIcon::PointingHand });
            }
            resp
        });
        let resp = inner.inner;
        crate::playtest::control(&sim.teams[t].name.clone(), &resp, true);
        if resp.drag_started() || resp.clicked() {
            if may_drag {
                select_team(k, sim, t);
            } else if !lost {
                k.notify("Per dare ordini premi «Pausa».");
            }
        }
        if resp.dragged() && may_drag && k.selection == Selection::Team(t) {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(g) = ctx.pointer_latest_pos().and_then(|p| ground_at(sim, cam, p)) {
                preview_move(k, sim, t, rocca::Axial::of(g));
                k.dragging = Some((t, g));
            }
        }
        // where it would go: the time on the road, next to the cone
        if k.dragging.is_some_and(|(d, _)| d == t) {
            use super::icons::Icon;
            egui::Area::new(egui::Id::new("trascina")).fixed_pos(if compact(ctx) {
                egui::pos2(12.0, 122.0)
            } else { flag.right_top() + egui::vec2(8.0, 4.0) }).order(egui::Order::Tooltip).interactable(false).show(ctx, |ui| {
                egui::Frame::none().fill(Color32::from_black_alpha(215)).rounding(6.0).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
                    if compact(ctx) { ui.set_width((ctx.screen_rect().width() - 48.0).min(300.0)); }
                    ui.horizontal_wrapped(|ui| match &k.move_plan {
                        Some(Ok(plan)) => chip(ui, Icon::Clock, &format!("{:.0} min", plan.minutes.ceil()), AMBER),
                        Some(Err(e)) => chip(ui, Icon::Cross, e, RED),
                        None => {}
                    });
                });
            });
        }
        if resp.drag_stopped() && k.selection == Selection::Team(t) {
            // Touch release also emits PointerGone; latest_pos is then None.
            // The release event still carries the actual drop location.
            let release = ctx.input(|i| i.raw.events.iter().rev().find_map(|e| match e {
                egui::Event::Touch { phase: egui::TouchPhase::End, pos, .. } => Some(*pos),
                egui::Event::PointerButton { pressed: false, pos, .. } => Some(*pos),
                _ => None,
            })).or_else(|| ctx.pointer_latest_pos());
            let on_map = release.is_some_and(|p| drop_rect(ctx, k).contains(p));
            match k.dragging {
                Some((_, g)) if on_map && may_drag => give_move(k, sim, t, rocca::Axial::of(g)),
                _ => { k.clear_selection(); k.notify("Spostamento annullato: rilascia il cono sulla mappa."); }
            }
        }
    }
}

/// The evacuation being chosen, on its district: who, where to and how
/// long (a dial: preparation, departures, the road), «Evacua» / «Annulla».
/// In the same pause the choice can be changed: another district replaces
/// it, and the district itself offers to take it back.
fn confirm_bubble(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, cam: (&Camera, &GlobalTransform)) {
    use super::icons::Icon;
    if ctx.dragged_id().is_some() {
        return;
    }
    let Selection::District(d) = k.selection else { return };
    let Some(plan) = k.evac_plan.clone() else { return };
    let s = if compact(ctx) {
        egui::pos2(if k.side_w > 0.0 { ctx.screen_rect().right() - k.side_w * 0.5 } else { ctx.screen_rect().center().x }, if k.side_w > 0.0 { 8.0 } else { 100.0 })
    } else {
        let Some(s) = screen(cam, sim, sim.districts[d].centre, 40.0) else { return };
        s
    };
    let this_pause = sim.can_cancel_evacuation();
    let ordered_here = this_pause && sim.evac.active().is_some_and(|e| e.district == d);
    let mut close = false;
    egui::Area::new(egui::Id::new("conferma")).fixed_pos(s + egui::vec2(0.0, 24.0)).pivot(Align2::CENTER_TOP).order(egui::Order::Foreground).show(ctx, |ui| {
        card(BLUE).inner_margin(if compact(ctx) { 10.0 } else { 18.0 }).show(ui, |ui| {
            if compact(ctx) { ui.set_width((if k.side_w > 0.0 { k.side_w - 40.0 } else { ctx.screen_rect().width() - 48.0 }).min(340.0)); }
            ui.label(RichText::new(&sim.districts[d].name).size(19.0).strong().color(BLUE));
            if ordered_here {
                ui.horizontal(|ui| chip(ui, Icon::Exit, "evacuazione ordinata", GREEN));
                ui.horizontal(|ui| {
                    let r = ui.add(secondary("Annulla l'evacuazione"));
                    crate::playtest::control("Annulla l'evacuazione", &r, true);
                    if r.clicked() {
                        if let Err(e) = sim.cancel_evacuation() {
                            k.notify(e);
                        }
                        close = true;
                    }
                    let r = ui.add(secondary("Chiudi"));
                    close |= r.clicked();
                });
                return;
            }
            match &plan {
                Err(e) => {
                    ui.horizontal(|ui| chip(ui, Icon::Alert, e, RED));
                }
                Ok(p) => {
                    ui.horizontal(|ui| {
                        chip(ui, Icon::Family, &p.people.to_string(), Color32::WHITE);
                        ui.add_space(10.0);
                        chip(ui, Icon::Clock, &format!("{:.0} min", p.total_min().ceil()), EVAC);
                    });
                }
            }
            let other = sim.evac.active().map(|e| e.district).filter(|&e| e != d);
            if let (Some(e), true) = (other, this_pause) {
                ui.horizontal(|ui| chip(ui, Icon::Up, &format!("al posto di {}", sim.districts[e].name), AMBER));
            }
            let free = other.is_none() || this_pause;
            let ok = plan.is_ok() && sim.paused() && free && k.allows(Expect::Evacuate(d));
            ui.horizontal(|ui| {
                let r = ui.add_enabled(ok, primary("Evacua"));
                crate::playtest::control("Evacua", &r, ok);
                if r.clicked() {
                    match sim.evacuate(d) {
                        Ok(_) => {
                            k.done(Expect::Evacuate(d));
                            close = true;
                        }
                        Err(e) => k.notify(e),
                    }
                }
                let r = ui.add(secondary("Annulla"));
                crate::playtest::control("Annulla", &r, true);
                close |= r.clicked();
            });
            if let (Some(e), false) = (other, this_pause) {
                ui.horizontal(|ui| chip(ui, Icon::Alert, &format!("in corso: {}", sim.districts[e].name), AMBER));
            }
        });
    });
    if close {
        k.clear_selection();
    }
}

/// The timer on a district being evacuated: a ring that empties with the time
/// left until the last family reaches the refuge (`Evacuations::remaining_min`,
/// the rule of the time announced); a check when it is done, an alert when
/// the fire cut it short.
fn evac_timer(ctx: &egui::Context, sim: &Sim, d: usize, at: egui::Pos2) {
    use super::icons::{self, Icon};
    let Some(e) = sim.evac.of(d) else { return };
    let left = sim.evac.remaining_min(d, sim.time_s());
    let (frac, minutes, col, icon) = match left {
        _ if e.interrupted => (1.0, None, RED, Some(Icon::Alert)),
        Some(left) => {
            let elapsed = (sim.time_s() - e.started_s).max(0) as f32 / 60.0;
            (left / (elapsed + left).max(0.1), Some(left.ceil() as i64), EVAC, None)
        }
        None => (1.0, None, GREEN, Some(Icon::Check)),
    };
    let size = if minutes.is_some() { 60.0 } else { 40.0 };
    egui::Area::new(egui::Id::new(("timer", d))).fixed_pos(at).pivot(Align2::CENTER_CENTER).order(egui::Order::Middle).interactable(false).show(ctx, |ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
        let p = ui.painter();
        let c = r.center();
        let radius = size * 0.5 - 4.0;
        p.circle_filled(c, size * 0.5, Color32::from_black_alpha(215));
        p.circle_stroke(c, radius, egui::Stroke::new(5.0, Color32::from_gray(60)));
        // the time left: an arc from the top, clockwise, that shrinks
        let n = ((frac * 64.0).ceil() as usize).max(1);
        let pts: Vec<egui::Pos2> = (0..=n)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * frac.clamp(0.0, 1.0) * i as f32 / n as f32;
                c + egui::vec2(a.cos(), a.sin()) * radius
            })
            .collect();
        p.add(egui::Shape::line(pts, egui::Stroke::new(5.0, col)));
        if let Some(m) = minutes {
            let g = p.layout_no_wrap(format!("{m}"), egui::FontId::proportional(20.0), Color32::WHITE);
            p.galley(c - egui::vec2(g.size().x / 2.0, g.size().y / 2.0 + 4.0), g, Color32::WHITE);
            let g = p.layout_no_wrap("min".into(), egui::FontId::proportional(11.0), GREY);
            p.galley(c + egui::vec2(-g.size().x / 2.0, 7.0), g, GREY);
        }
        if let Some(icon) = icon {
            icons::draw(p, egui::Rect::from_center_size(c, egui::vec2(size * 0.5, size * 0.5)), icon, col);
        }
    });
}

fn notice(ctx: &egui::Context, k: &Kiosk) {
    let Some((text, _)) = &k.notice else { return };
    egui::Area::new(egui::Id::new("avviso")).anchor(Align2::CENTER_TOP, [0.0, if compact(ctx) { 116.0 } else { 170.0 }]).order(egui::Order::Foreground).show(ctx, |ui| {
        panel().show(ui, |ui| {
            if compact(ctx) { ui.set_width((ctx.screen_rect().width() - 48.0).min(480.0)); }
            ui.add(egui::Label::new(RichText::new(text).size(16.0).color(Color32::WHITE)).wrap());
        });
    });
}

/// The operator menu (F2): a new visit, the game straight away, the world
/// stopped, the view, the graphics, the board.
fn operator(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>, gfx: &mut crate::quality::Graphics) {
    egui::Window::new("Menu").id(egui::Id::new("operatore")).anchor(Align2::LEFT_BOTTOM, [8.0, -60.0]).default_width((ctx.screen_rect().width() - 48.0).min(380.0)).max_height(ctx.screen_rect().height() - 90.0).resizable(false).collapsible(false).vscroll(true).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.label(RichText::new("Menu (F2 per chiudere)").strong().color(Color32::WHITE));
            ui.label(RichText::new(format!("{} · {} · seme {}", sim.case.name, clock(sim.time_s()), k.seed)).color(GREY));
            ui.horizontal_wrapped(|ui| {
                if ui.add(secondary("Nuova visita")).clicked() {
                    new_visit(k, sim, restarted);
                }
                if ui.add(secondary("Vai al giorno 2")).clicked() {
                    start_game(k, sim, restarted);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Seme:");
                for s in 1..=3u64 {
                    ui.selectable_value(&mut k.seed, s, s.to_string());
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut k.frozen, "Ferma il mondo");
                if ui.add(secondary("Vista iniziale")).clicked() {
                    k.reset_view = true;
                }
            });
            if !cfg!(target_arch = "wasm32") { ui.horizontal(|ui| {
                use crate::quality::Graphics;
                ui.label("Grafica:");
                let now_level = gfx.level();
                for (name, label, preset) in [("alta", "Alta", Graphics::ALTA), ("media", "Media", Graphics::MEDIA), ("bassa", "Bassa", Graphics::BASSA)] {
                    if ui.selectable_label(now_level == Some(name), label).clicked() && now_level != Some(name) {
                        *gfx = Graphics { vegetation: gfx.vegetation, ..preset };
                        if let Err(e) = crate::quality::save(name) {
                            k.error = Some(e);
                        }
                    }
                }
            });
            }
            if ui.add(secondary("Azzera classifica")).clicked() {
                k.board = rocca::score::Board::default();
                if let Err(e) = board::save(&k.board) {
                    k.error = Some(e);
                }
            }
            if let Some(e) = &k.error {
                ui.colored_label(RED, e);
            }
        });
    });
}


fn compact_top(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, cam: (&Camera, &GlobalTransform)) {
    egui::Area::new(egui::Id::new("barra_mobile")).anchor(Align2::LEFT_TOP, [8.0, 8.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        panel().inner_margin(8.0).show(ui, |ui| {
            ui.set_width((ctx.screen_rect().width() - k.side_w - 32.0).min(460.0));
            ui.horizontal(|ui| {
                pause_dial(ui, k, sim);
                ui.vertical(|ui| {
                    let day = if k.phase == Phase::Tutorial { 1 } else { 2 };
                    ui.label(RichText::new(format!("Giorno {day} · {}", clock(sim.time_s()))).size(20.0).strong());
                    let w = sim.weather();
                    ui.horizontal(|ui| {
                        wind_arrow(ui, cam, w.wind_dir_deg, 22.0, Color32::WHITE);
                        ui.label(format!("{:.0} km/h · umidità {:.0}%", w.wind_speed_kmh, w.moisture_pct));
                    });
                    let frac = sim.time_s() as f32 / sim.case.duration_s().max(1) as f32;
                    ui.add(egui::ProgressBar::new(frac).desired_height(5.0));
                });
            });
            if sim.paused() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Pausa").strong().color(GREEN));
                    for (color, label) in [(crate::forecast_view::NEAR_COLOUR, "1 h"), (crate::forecast_view::FAR_COLOUR, "3 h")] {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                        ui.painter().line_segment([r.left_center(), r.right_center()], egui::Stroke::new(3.0, color));
                        ui.colored_label(color, label);
                    }
                    if let Some((at, w)) = sim.weather_ahead() {
                        ui.label(format!("{}: {:.0} km/h da {}", clock(at), w.wind_speed_kmh, rocca::words::compass(w.wind_dir_deg)));
                    }
                });
            }
        });
    });
}


fn drop_rect(ctx: &egui::Context, k: &Kiosk) -> egui::Rect {
    let mut r = ctx.screen_rect();
    if compact(ctx) {
        r.min.y += 116.0;
        r.max.x -= k.side_w;
        r.max.y -= if k.phase == Phase::Tutorial && k.side_w == 0.0 { (ctx.screen_rect().height() * 0.20).clamp(42.0, 120.0) + 126.0 } else { 58.0 };
    }
    r
}
