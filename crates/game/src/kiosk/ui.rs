//! The kiosk's interface: the map is the screen.
//!
//! - **On the map**, one chip per district: its rank, how far the fire is,
//!   the units on it now and after confirmation, the controls to rank it and
//!   the two civil orders. Each ground unit carries a label of what it is
//!   doing. Routes and posts are drawn in 3D by `overlays::update_routes`.
//! - **Top**: wind, clock and phase; the crisis banner with its countdown.
//! - **Bottom centre**: the one button (Conferma / Continua).
//! - **Bottom left**: what the coordinator proposes and why; **bottom right**:
//!   the latest events.
//! - **Fine**: the debrief, against the same fire with no orders, with
//!   «Riprova» and «Altro incendio».
//! - **F2**: the operator bar (case, seed, speed, new game). Hidden by default.
//!
//! Reads the game and the plan being composed; every change goes through
//! `kiosk::commit`, `kiosk::close_crisis` or `kiosk::new_game`.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui::{Align2, Color32, RichText};
use rocca::words::{compass, km};
use rocca::Civil;
use scenario::population::Status;
use scenario::Pos;

use super::{close_crisis, commit, new_game, Kiosk, Phase};
use crate::sim::{Sim, SimRestarted};

const PANEL: Color32 = Color32::from_rgba_premultiplied(18, 20, 24, 242);
const ORANGE: Color32 = Color32::from_rgb(240, 140, 50);
const RED: Color32 = Color32::from_rgb(255, 105, 85);
const AMBER: Color32 = Color32::from_rgb(250, 195, 70);
const BLUE: Color32 = Color32::from_rgb(110, 185, 255);
const GREEN: Color32 = Color32::from_rgb(110, 230, 150);
/// What the two civil orders do, as the model does it (`Abm::prealert_of`,
/// `Abm::order_evacuation_of`).
const PREALLERTA: &str = "Preallerta: le famiglie vengono avvisate e si preparano, ma restano a casa (qualcuna può decidere di partire da sola). Se poi ordini l'evacuazione, partono prima.";
const EVACUA: &str = "Evacua: le famiglie ricevono l'ordine di partire subito verso un'area sicura. Non si può annullare.";
/// Room a district chip is given on screen, for placing chips apart.
const CHIP_W: f32 = 290.0;
const CHIP_H: f32 = 190.0;
/// What the simulated families do is not what to do: said where it shows.
const REAL_LIFE: &str = "Nella realtà, quando arriva l'ordine di evacuazione si parte subito, seguendo le indicazioni. Aspettare di vedere il fuoco o restare a difendere la casa sono tra gli errori più pericolosi.";
const GREY: Color32 = Color32::from_rgb(205, 208, 212);

fn clock(s: i64) -> String {
    format!("T+{}:{:02}", s / 3600, (s / 60) % 60)
}


/// One family of controls: a secondary action (grey), an order that can be
/// on (filled with its colour) or off, and the one primary action (yellow).
fn secondary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).size(15.0).color(Color32::WHITE))
        .fill(Color32::from_gray(62))
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(110)))
        .rounding(6.0)
        .min_size(egui::vec2(0.0, 28.0))
}

fn toggle(text: &str, on: bool, colour: Color32) -> egui::Button<'static> {
    let fg = if on { Color32::BLACK } else { Color32::WHITE };
    egui::Button::new(RichText::new(text.to_string()).size(16.0).strong().color(fg))
        .fill(if on { colour } else { Color32::from_gray(48) })
        .stroke(egui::Stroke::new(1.5, colour))
        .rounding(6.0)
        .min_size(egui::vec2(104.0, 30.0))
}

fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).size(24.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(10.0).min_size(egui::vec2(300.0, 54.0))
}

/// A coloured tag: the fire's distance, a unit in trouble.
fn pill(ui: &mut egui::Ui, text: &str, fill: Color32) {
    egui::Frame::none().fill(fill).rounding(5.0).inner_margin(egui::Margin::symmetric(6.0, 1.0)).show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(text.to_string()).size(15.0).strong().color(Color32::BLACK)).extend());
    });
}

fn panel() -> egui::Frame {
    egui::Frame::none().fill(PANEL).rounding(8.0).inner_margin(egui::Margin::symmetric(12.0, 8.0))
}

/// World point to screen point, `lift_m` above the ground.
fn screen(cam: (&Camera, &GlobalTransform), sim: &Sim, p: Pos, lift_m: f32) -> Option<egui::Pos2> {
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
) {
    let ctx = contexts.ctx_mut();
    let k = &mut *kiosk;
    let Ok(cam) = cameras.get_single() else { return };
    if ctx.input(|i| i.key_pressed(egui::Key::F2)) {
        k.operator = !k.operator;
    }

    let mut top = top_bar(ctx, k, &sim, cam);
    view_controls(ctx, k);
    if k.phase == Phase::Fine {
        debrief(ctx, k, &mut sim, &mut restarted);
    } else {
        legend(ctx);
        if k.phase == Phase::Crisi {
            top = crisis_banner(ctx, k, &mut sim, top);
        }
        unit_labels(ctx, &sim, cam);
        district_chips(ctx, k, &sim, cam, top);
        proposal(ctx, k, &sim);
        action(ctx, k, &mut sim);
        events(ctx, &sim);
    }
    if k.operator {
        operator(ctx, k, &mut sim, &mut restarted);
    }
    focus.pointer = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

/// Wind, clock and phase; under it, the crisis or the planning hint.
/// Returns where the band ends, for the chips to stay clear of it.
fn top_bar(ctx: &egui::Context, k: &Kiosk, sim: &Sim, cam: (&Camera, &GlobalTransform)) -> f32 {
    egui::Area::new(egui::Id::new("barra")).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, [0.0, 10.0]).interactable(false).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.horizontal(|ui| {
                let w = sim.fire.weather();
                // an arrow the way the wind pushes the fire, as seen on screen
                let (rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
                let from = (w.wind_dir_deg as f32).to_radians();
                let at = sim.case.ignition();
                let to = Pos { x: at.x - from.sin() * 400.0, y: at.y - from.cos() * 400.0 };
                if let (Some(a), Some(b)) = (screen(cam, sim, at, 0.0), screen(cam, sim, to, 0.0)) {
                    let d = (b - a).normalized() * 14.0;
                    let c = rect.center();
                    let stroke = egui::Stroke::new(3.0, Color32::WHITE);
                    ui.painter().line_segment([c - d, c + d], stroke);
                    let side = egui::vec2(-d.y, d.x) * 0.45;
                    ui.painter().line_segment([c + d, c + d * 0.4 + side], stroke);
                    ui.painter().line_segment([c + d, c + d * 0.4 - side], stroke);
                }
                ui.label(RichText::new(format!("Vento da {}, {:.0} km/h", compass(w.wind_dir_deg), w.wind_speed_kmh)).size(20.0).color(Color32::WHITE));
                ui.separator();
                ui.label(RichText::new(format!("{} di {}", clock(sim.time_s()), clock(sim.case.duration_s()).trim_start_matches("T+"))).size(20.0).color(Color32::WHITE).monospace());
                ui.separator();
                let (text, colour) = match k.phase {
                    Phase::Pianifica if sim.time_s() > rocca::STEP_S => ("IN PAUSA · puoi cambiare il piano".to_string(), AMBER),
                    Phase::Pianifica => ("PIANIFICA · tempo fermo".to_string(), GREY),
                    Phase::Esegui if sim.speed > k.speed => (format!("IN CORSO · niente di nuovo, tempo veloce ×{:.0}", sim.speed), GREEN),
                    Phase::Esegui => (format!("IN CORSO · tempo accelerato ×{:.0}", k.speed), GREEN),
                    Phase::Crisi => (format!("CRISI · ×1 · {:.0} s", (k.crisis_s - k.phase_t).max(0.0).ceil()), ORANGE),
                    Phase::Fine => ("FINE".to_string(), GREY),
                };
                ui.label(RichText::new(text).size(20.0).strong().color(colour));
            });
        });
        match k.phase {
            Phase::Pianifica if sim.time_s() <= rocca::STEP_S => {
                ui.add_space(6.0);
                panel().show(ui, |ui| {
                    ui.set_max_width(760.0);
                    ui.label(RichText::new(format!("Nuovo incendio, vicino a {}. Il tempo è fermo.", sim.case.near)).size(19.0).strong().color(Color32::WHITE));
                    for (n, step) in ["Scegli quali paesi difendere: «Difendi». La sala operativa ci manda i mezzi.", "Avvisa (Preallerta) o fai partire (Evacua) gli abitanti.", "Premi «Conferma e avvia»."].iter().enumerate() {
                        ui.label(RichText::new(format!("{}. {step}", n + 1)).size(18.0).color(Color32::WHITE));
                    }
                    ui.label(RichText::new("Passa il mouse su Preallerta ed Evacua per la differenza.").size(14.0).color(GREY));
                });
            }
            _ => {}
        }
    })
    .response
    .rect
    .bottom()
}

/// The district a crisis is about, if any.
fn crisis_district(k: &Kiosk) -> Option<usize> {
    use rocca::crisis::Kind;
    match k.crisis.as_ref()?.kind {
        Kind::Scoperto { district } | Kind::Previsione { district } | Kind::Vento { district } => Some(district),
        Kind::MezzoPerso { .. } => None,
    }
}

/// The crisis in one place: what is happening, the actions that answer it,
/// what the change would do to the units, the countdown and the button.
/// Returns where it ends.
fn crisis_banner(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, top: f32) -> f32 {
    let Some(c) = k.crisis.clone() else { return top };
    let d = crisis_district(k);
    let mut close = false;
    let r = egui::Area::new(egui::Id::new("crisi")).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, [0.0, top + 6.0]).show(ctx, |ui| {
        egui::Frame::none().fill(Color32::from_rgb(64, 30, 10)).stroke(egui::Stroke::new(2.0, ORANGE)).rounding(8.0).inner_margin(12.0).show(ui, |ui| {
            ui.set_max_width(780.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Decisione critica").size(22.0).strong().color(ORANGE));
                ui.label(RichText::new(format!("{:.0} s", (k.crisis_s - k.phase_t).max(0.0).ceil())).size(22.0).strong().color(Color32::WHITE));
            });
            ui.add(egui::ProgressBar::new((1.0 - k.phase_t / k.crisis_s).clamp(0.0, 1.0)).desired_height(8.0).fill(ORANGE));
            ui.label(RichText::new(&c.text).size(19.0).color(Color32::WHITE));
            if let Some(d) = d {
                let name = sim.districts[d].name.clone();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let first = k.proposed.priorities.first() == Some(&d);
                    if ui.add_enabled(!first, secondary(&format!("Metti {name} per primo"))).clicked() {
                        k.proposed.priorities.retain(|&x| x != d);
                        k.proposed.priorities.insert(0, d);
                        k.dirty = true;
                    }
                    let active = sim.active.civil[d];
                    for (o, label, colour) in [(Civil::Preallerta, "Preallerta", AMBER), (Civil::Evacua, "Evacua", BLUE)] {
                        let on = k.proposed.civil[d] == o;
                        if ui.add_enabled(active < o, toggle(&format!("{label} {name}"), on, colour)).clicked() {
                            k.proposed.civil[d] = if on { active } else { o };
                            k.dirty = true;
                        }
                    }
                });
            }
            // what the change does to the units, district by district
            if let Some(p) = &k.preview {
                let changes: Vec<String> = (0..sim.districts.len())
                    .filter_map(|x| {
                        let now = sim.posts.iter().flatten().filter(|q| q.district == x).count();
                        let next = p.units_on(x);
                        (now != next).then(|| format!("{} da {now} a {next} mezzi", sim.districts[x].name))
                    })
                    .collect();
                if !changes.is_empty() {
                    ui.label(RichText::new(format!("Con il nuovo piano: {}", changes.join(" · "))).size(17.0).strong().color(AMBER));
                }
            }
            let pending = k.proposed != sim.active;
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let label = if pending { "Conferma" } else { "Continua con il piano attuale" };
                if ui.add(primary(label)).clicked() {
                    close = true;
                }
                ui.label(RichText::new("Il fuoco non si ferma. Se il tempo scade si applica quello che hai scelto.").size(14.0).color(GREY));
            });
        });
    });
    if close {
        close_crisis(k, sim);
    }
    r.response.rect.bottom()
}

/// Back to the home view, zoom in and out, and how to move the map.
fn view_controls(ctx: &egui::Context, k: &mut Kiosk) {
    egui::Area::new(egui::Id::new("vista")).anchor(Align2::RIGHT_TOP, [-12.0, 12.0]).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add(secondary("+")).on_hover_text("avvicina").clicked() {
                    k.zoom *= 0.8;
                }
                if ui.add(secondary("−")).on_hover_text("allontana").clicked() {
                    k.zoom *= 1.25;
                }
                if ui.add(secondary("Vista iniziale")).clicked() {
                    k.reset_view = true;
                }
            });
            ui.label(RichText::new("trascina: sposta · rotella: zoom · tasto destro: ruota").size(13.0).color(GREY));
        });
    });
}

/// What the marks on the map mean.
fn legend(ctx: &egui::Context) {
    // below the operator bar when that is open; closed at first on a small screen
    let below = ctx.memory(|m| m.area_rect(egui::Id::new("operatore"))).filter(|_| ctx.memory(|m| m.areas().visible_last_frame(&egui::LayerId::new(egui::Order::Foreground, egui::Id::new("operatore"))))).map_or(12.0, |r| r.bottom() + 8.0);
    let roomy = false;
    egui::Area::new(egui::Id::new("legenda")).anchor(Align2::LEFT_TOP, [12.0, below]).show(ctx, |ui| {
        panel().show(ui, |ui| {
            egui::CollapsingHeader::new(RichText::new("Legenda").size(15.0).strong().color(Color32::WHITE)).default_open(roomy).show(ui, |ui| {
                #[derive(Clone, Copy)]
                enum Mark {
                    Fill(Color32),
                    Ring(Color32),
                    Line(Color32, bool),
                    Arrow,
                    Diamond,
                }
                let items = [
                    (Mark::Fill(Color32::from_rgb(255, 120, 40)), "fuoco attivo"),
                    (Mark::Fill(Color32::from_rgb(70, 50, 40)), "area già bruciata"),
                    (Mark::Arrow, "dove il vento spinge il fuoco"),
                    (Mark::Ring(GREEN), "case difese da un mezzo"),
                    (Mark::Line(GREEN, false), "strada che un mezzo sta facendo"),
                    (Mark::Line(Color32::WHITE, true), "spostamento proposto, da confermare"),
                    (Mark::Ring(AMBER), "preallerta"),
                    (Mark::Ring(BLUE), "evacuazione"),
                    (Mark::Diamond, "famiglia in casa con il fuoco vicino"),
                    (Mark::Fill(Color32::from_rgb(60, 120, 220)), "cartello blu: area di attesa sicura"),
                ];
                for (m, text) in items {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(26.0, 16.0), egui::Sense::hover());
                        let p = ui.painter();
                        let c = r.center();
                        match m {
                            Mark::Fill(col) => {
                                p.rect_filled(r.shrink2(egui::vec2(4.0, 2.0)), 3.0, col);
                            }
                            Mark::Ring(col) => {
                                p.circle_stroke(c, 6.5, egui::Stroke::new(2.5, col));
                            }
                            Mark::Line(col, dashed) => {
                                let (a, b) = (egui::pos2(r.left() + 2.0, c.y), egui::pos2(r.right() - 2.0, c.y));
                                if dashed {
                                    p.add(egui::Shape::dashed_line(&[a, b], egui::Stroke::new(3.0, col), 5.0, 3.0));
                                } else {
                                    p.line_segment([a, b], egui::Stroke::new(3.0, col));
                                }
                            }
                            Mark::Arrow => {
                                let st = egui::Stroke::new(3.0, Color32::from_gray(230));
                                p.line_segment([egui::pos2(r.left() + 3.0, c.y), egui::pos2(r.right() - 3.0, c.y)], st);
                                p.line_segment([egui::pos2(r.right() - 3.0, c.y), egui::pos2(r.right() - 9.0, c.y - 5.0)], st);
                                p.line_segment([egui::pos2(r.right() - 3.0, c.y), egui::pos2(r.right() - 9.0, c.y + 5.0)], st);
                            }
                            Mark::Diamond => {
                                let pts = vec![c + egui::vec2(0.0, -7.0), c + egui::vec2(6.0, 0.0), c + egui::vec2(0.0, 7.0), c + egui::vec2(-6.0, 0.0)];
                                p.add(egui::Shape::convex_polygon(pts, RED, egui::Stroke::NONE));
                            }
                        }
                        ui.label(RichText::new(text).size(14.0).color(GREY));
                    });
                }
            });
        });
    });
}

/// Where a district's chip stands: over the home nearest the district's
/// middle, so a district of scattered hamlets is labelled on a hamlet and not
/// on the woods between them.
fn chip_anchor(sim: &Sim, d: usize) -> Pos {
    let c = sim.districts[d].centre;
    sim.districts[d]
        .households
        .iter()
        .map(|&i| sim.agents.households[i].home)
        .min_by(|a, b| rocca::district::dist(*a, c).total_cmp(&rocca::district::dist(*b, c)))
        .unwrap_or(c)
}

/// One chip over each district: rank, fire distance, units, controls.
fn district_chips(ctx: &egui::Context, k: &mut Kiosk, sim: &Sim, cam: (&Camera, &GlobalTransform), top: f32) {
    let screen_rect = ctx.screen_rect();
    let n = sim.districts.len();
    let mut order = k.proposed.priorities.clone();
    let mut civil = k.proposed.civil.clone();
    let mut changed = false;
    // Where each chip stands (bottom centre) and how big it was last frame.
    let mut place: Vec<Option<(egui::Pos2, egui::Vec2)>> = (0..n)
        .map(|d| {
            let at = screen(cam, sim, chip_anchor(sim, d), 40.0)?;
            // A fixed size, not last frame's: a chip that gains a line of text
            // must not move its buttons from under the pointer.
            let size = egui::vec2(CHIP_W, CHIP_H);
            Some((at, size))
        })
        .collect();
    // Keep the whole chip on screen and clear of the top band and the button.
    let clamp = |at: &mut egui::Pos2, size: egui::Vec2| {
        at.x = at.x.clamp(screen_rect.left() + size.x * 0.5 + 8.0, screen_rect.right() - size.x * 0.5 - 8.0);
        at.y = at.y.clamp(top + size.y + 8.0, screen_rect.bottom() - 100.0);
    };
    for (at, size) in place.iter_mut().flatten() {
        clamp(at, *size);
    }
    // the fixed panels (last frame's), which a chip must not cover
    let obstacles: Vec<egui::Rect> = ["proposta", "eventi", "azione", "legenda", "vista"]
        .iter()
        .filter_map(|n| ctx.memory(|m| m.area_rect(egui::Id::new(*n))))
        .map(|r| r.expand(6.0))
        .collect();
    let avoid = |at: &mut egui::Pos2, size: egui::Vec2| {
        for o in &obstacles {
            let chip = egui::Rect::from_min_size(egui::pos2(at.x - size.x * 0.5, at.y - size.y), size);
            if !chip.intersects(*o) {
                continue;
            }
            // above the panel if there is room, else beside it
            if o.top() - size.y > top + 8.0 {
                at.y = o.top();
            } else if o.center().x < screen_rect.center().x {
                at.x = o.right() + size.x * 0.5;
            } else {
                at.x = o.left() - size.x * 0.5;
            }
        }
    };
    // Two places close on screen: push their chips apart along the shorter
    // overlap, a few passes.
    for _ in 0..6 {
        for a in 0..n {
            for b in a + 1..n {
                let (Some((pa, sa)), Some((pb, sb))) = (place[a], place[b]) else { continue };
                let ox = (sa.x + sb.x) * 0.5 + 8.0 - (pa.x - pb.x).abs();
                let oy = (sa.y + sb.y) * 0.5 + 8.0 - (pa.y - pb.y).abs();
                if ox <= 0.0 || oy <= 0.0 {
                    continue;
                }
                let (mut pa, mut pb) = (pa, pb);
                if ox < oy {
                    let s = if pa.x <= pb.x { -0.5 } else { 0.5 };
                    pa.x += s * ox;
                    pb.x -= s * ox;
                } else {
                    let s = if pa.y <= pb.y { -0.5 } else { 0.5 };
                    pa.y += s * oy;
                    pb.y -= s * oy;
                }
                clamp(&mut pa, sa);
                clamp(&mut pb, sb);
                avoid(&mut pa, sa);
                avoid(&mut pb, sb);
                place[a] = Some((pa, sa));
                place[b] = Some((pb, sb));
            }
        }
    }
    for (at, size) in place.iter_mut().flatten() {
        avoid(at, *size);
    }
    let alarm = crisis_district(k);
    // A chip moved off its place (by another chip, a panel, the screen edge)
    // keeps a thin line to it, so it is never a label of nowhere.
    let leaders = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("guide")));
    for d in 0..n {
        let (Some((at, _)), Some(p)) = (place[d], screen(cam, sim, chip_anchor(sim, d), 0.0)) else { continue };
        if at.distance(p) > 30.0 {
            leaders.line_segment([at, p], egui::Stroke::new(2.0, Color32::from_white_alpha(170)));
            leaders.circle_filled(p, 4.0, Color32::from_white_alpha(220));
        }
    }
    for d in 0..n {
        let dist = &sim.districts[d];
        let Some((at, _)) = place[d] else { continue };
        let id = egui::Id::new(("quartiere", d));
        let rank = order.iter().position(|&x| x == d);
        egui::Area::new(id).fixed_pos(at).pivot(Align2::CENTER_BOTTOM).show(ctx, |ui| {
            let (border, width) = if alarm == Some(d) { (ORANGE, 4.0) } else if rank == Some(0) { (AMBER, 1.5) } else { (Color32::from_gray(90), 1.5) };
            egui::Frame::none().fill(PANEL).stroke(egui::Stroke::new(width, border)).rounding(8.0).inner_margin(8.0).show(ui, |ui| {
                ui.set_min_width(250.0);
                ui.horizontal(|ui| {
                    let badge = rank.map_or("·".to_string(), |r| (r + 1).to_string());
                    ui.label(RichText::new(badge).size(30.0).strong().color(if rank.is_some() { AMBER } else { GREY }));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&dist.name).size(20.0).strong().color(Color32::WHITE));
                        let (risk, colour) = match k.risk.get(d).copied().flatten() {
                            Some(e) if e.distance_m < 60.0 => ("fuoco tra le case".to_string(), Some(RED)),
                            Some(e) if e.distance_m < 1000.0 => (format!("fuoco a {}", km(e.distance_m)), Some(RED)),
                            Some(e) if e.distance_m < 2500.0 => (format!("fuoco a {}", km(e.distance_m)), Some(ORANGE)),
                            Some(e) => (format!("fuoco a {}", km(e.distance_m)), None),
                            None => ("nessun fuoco".to_string(), None),
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{} famiglie", dist.households.len())).size(15.0).color(GREY));
                            match colour {
                                Some(c) => pill(ui, &risk, c),
                                None => {
                                    ui.label(RichText::new(risk).size(15.0).color(GREY));
                                }
                            }
                        });
                    });
                });
                let now = sim.posts.iter().flatten().filter(|p| p.district == d).count();
                let next = k.preview.as_ref().map_or(now, |p| p.units_on(d));
                ui.label(RichText::new(format!("mezzi assegnati: {now}")).size(16.0).color(if now > 0 { GREEN } else { GREY }));
                ui.horizontal(|ui| match rank {
                    Some(r) => {
                        if r > 0 && ui.add(secondary("metti per primo")).clicked() {
                            order.retain(|&x| x != d);
                            order.insert(0, d);
                            changed = true;
                        }
                        if r > 1 && ui.add(secondary("più importante")).clicked() {
                            order.swap(r, r - 1);
                            changed = true;
                        }
                        if ui.add(secondary("non difendere")).clicked() {
                            order.retain(|&x| x != d);
                            changed = true;
                        }
                    }
                    None => {
                        let b = egui::Button::new(RichText::new("Difendi").size(17.0).strong().color(Color32::WHITE))
                            .fill(Color32::from_gray(62))
                            .stroke(egui::Stroke::new(2.0, AMBER))
                            .rounding(6.0)
                            .min_size(egui::vec2(120.0, 30.0));
                        if ui.add(b).clicked() {
                            order.push(d);
                            changed = true;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    let active = sim.active.civil[d];
                    for (c, label, colour) in [(Civil::Preallerta, "Preallerta", AMBER), (Civil::Evacua, "Evacua", BLUE)] {
                        // the order chosen, not every order it includes
                        let on = civil[d] == c;
                        // orders already given cannot be taken back
                        let locked = active >= c;
                        let hint = if c == Civil::Preallerta { PREALLERTA } else { EVACUA };
                        // an order already given shows as on and cannot be undone
                        let shown = on || (locked && civil[d] == c);
                        if ui.add_enabled(!locked, toggle(label, shown, colour)).on_hover_text(hint).clicked() {
                            civil[d] = if on { active } else { c.max(active) };
                            changed = true;
                        }
                    }
                });
                // what is chosen but not confirmed yet, right where it was chosen
                let mut pending: Vec<String> = vec![];
                if civil[d] != sim.active.civil[d] {
                    pending.push(if civil[d] == Civil::Evacua { "evacuazione".into() } else { "preallerta".into() });
                }
                if next < now {
                    pending.push(format!("perde {} mezzi", now - next));
                } else if next > now {
                    pending.push(format!("riceve {} mezzi", next - now));
                }
                if !pending.is_empty() && k.phase != Phase::Pianifica {
                    pill(ui, &format!("da confermare: {}", pending.join(", ")), AMBER);
                } else if !pending.is_empty() {
                    ui.label(RichText::new(format!("con il piano: {}", pending.join(", "))).size(15.0).strong().color(AMBER));
                }
                if sim.active.civil[d] != Civil::Nessuno {
                    let hh = &dist.households;
                    let safe = hh.iter().filter(|&&i| sim.agents.households[i].status == Status::Evacuated).count();
                    let road = hh.iter().filter(|&&i| sim.agents.households[i].status == Status::Evacuating).count();
                    let count = |st: Status| hh.iter().filter(|&&i| sim.agents.households[i].status == st).count();
                    let what = if sim.active.civil[d] == Civil::Evacua { "evacuazione ordinata" } else { "preallertati" };
                    ui.label(RichText::new(format!("{what}: {safe} in salvo, {road} in strada")).size(15.0).color(GREY));
                    // the families still at home, by what they are doing
                    let parts: Vec<String> = [
                        (count(Status::Preparing), "si preparano a partire"),
                        (count(Status::Warned), "avvisate, aspettano di vedere il fuoco"),
                        (count(Status::Defending), "restano a difendere la casa"),
                        (count(Status::Normal), "non ancora raggiunte dall'avviso"),
                        (count(Status::Trapped), "bloccate dal fuoco"),
                        (count(Status::Casualty), "raggiunte dal fuoco"),
                    ]
                    .into_iter()
                    .filter(|(n, _)| *n > 0)
                    .map(|(n, w)| format!("{n} {w}"))
                    .collect();
                    if !parts.is_empty() {
                        ui.label(RichText::new(format!("a casa: {}", parts.join(", "))).size(15.0).color(GREY)).on_hover_text(REAL_LIFE);
                    }
                }
            });
        });
    }
    if changed {
        k.proposed.priorities = order;
        k.proposed.civil = civil;
        k.dirty = true;
    }
}

/// A label over each ground unit: its name and what it is doing.
fn unit_labels(ctx: &egui::Context, sim: &Sim, cam: (&Camera, &GlobalTransform)) {
    // Units standing together (at the base, on one post) stack their labels.
    let mut shown: Vec<egui::Pos2> = vec![];
    for (i, u) in sim.crews.units.iter().enumerate() {
        if u.kind.is_air() {
            continue;
        }
        let Some(ground) = screen(cam, sim, u.pos, 35.0) else { continue };
        // Under a district's chip (last frame's): read it just below the chip.
        let ground = (0..sim.districts.len())
            .filter_map(|d| ctx.memory(|m| m.area_rect(egui::Id::new(("quartiere", d)))))
            .find(|r| r.expand(4.0).intersects(egui::Rect::from_center_size(ground - egui::vec2(0.0, 9.0), egui::vec2(200.0, 18.0))))
            .map_or(ground, |r| egui::pos2(ground.x, r.bottom() + 24.0));
        // a label is about 200 px wide and 18 tall
        let below = shown.iter().filter(|p| (p.x - ground.x).abs() < 200.0 && (p.y - ground.y).abs() < 18.0).count();
        shown.push(ground);
        let at = ground + egui::vec2(0.0, 20.0 * below as f32);
        let status = sim.unit_status(i);
        let trouble = if status.starts_with("bloccato") || status.starts_with("fuori") {
            Some(RED)
        } else if status.starts_with("si ritira") || status.contains(", fuoco a") {
            Some(ORANGE)
        } else {
            None
        };
        egui::Area::new(egui::Id::new(("mezzo", i))).fixed_pos(at).pivot(Align2::CENTER_BOTTOM).interactable(false).order(egui::Order::Background).show(ctx, |ui| {
            egui::Frame::none().fill(Color32::from_rgb(14, 16, 18)).stroke(egui::Stroke::new(1.0, trouble.unwrap_or(GREEN))).rounding(4.0).inner_margin(egui::Margin::symmetric(6.0, 2.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(RichText::new(u.callsign.clone()).size(15.0).strong().color(Color32::WHITE)).extend());
                    match trouble {
                        Some(c) => pill(ui, &status, c),
                        None => {
                            ui.add(egui::Label::new(RichText::new(status).size(15.0).color(GREEN)).extend());
                        }
                    }
                });
            });
        });
    }
}

/// What the coordinator would do with the plan being composed, and why.
fn proposal(ctx: &egui::Context, k: &Kiosk, sim: &Sim) {
    let Some(p) = &k.preview else { return };
    let pending = k.proposed != sim.active || k.phase == Phase::Pianifica;
    egui::Area::new(egui::Id::new("proposta")).anchor(Align2::LEFT_BOTTOM, [12.0, -12.0]).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.set_max_width(430.0);
            let title = if pending { "Il coordinatore propone" } else { "I mezzi" };
            egui::CollapsingHeader::new(RichText::new(title).size(15.0).strong().color(Color32::WHITE)).id_source("proposta_h").default_open(true).show(ui, |ui| {
            if !pending {
                // Nothing to confirm: what the units are doing now, not the
                // reasons given when they were sent (those are in the log).
                ui.label(RichText::new(format!("I mezzi adesso ({})", clock(sim.time_s()))).size(16.0).strong().color(Color32::WHITE));
                for (i, u) in sim.crews.units.iter().enumerate().filter(|(_, u)| !u.kind.is_air()) {
                    ui.label(RichText::new(format!("{}: {}", u.callsign, sim.unit_status(i))).size(15.0).color(GREY));
                }
                for (_, why) in &p.uncovered {
                    ui.label(RichText::new(format!("Senza mezzi: {why}")).size(15.0).color(ORANGE));
                }
                return;
            }
            ui.label(RichText::new("anteprima, non è garantito").size(14.0).color(GREY));
            for post in p.posts.iter().flatten() {
                ui.label(RichText::new(&post.reason).size(15.0).color(GREY));
            }
            for (_, why) in &p.idle {
                ui.label(RichText::new(why).size(15.0).color(GREY));
            }
            for (_, why) in &p.uncovered {
                ui.label(RichText::new(format!("Senza mezzi: {why}")).size(15.0).color(ORANGE));
            }
            for &d in &p.quiet {
                ui.label(RichText::new(format!("{}: il fuoco ora non lo minaccia, nessun mezzo", sim.districts[d].name)).size(15.0).color(GREY));
            }
            if p.posts.iter().all(|x| x.is_none()) && p.uncovered.is_empty() && p.quiet.is_empty() && p.idle.is_empty() {
                ui.label(RichText::new("Nessun luogo da difendere: i mezzi restano alla base.").size(15.0).color(ORANGE));
            }
            });
        });
    });
}

/// The one button.
fn action(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim) {
    let pending = k.proposed != sim.active;
    let label = match k.phase {
        Phase::Pianifica if sim.time_s() > rocca::STEP_S => Some("Conferma e riprendi"),
        Phase::Pianifica => Some("Conferma e avvia"),
        Phase::Esegui if pending => Some("Conferma il nuovo piano"),
        Phase::Crisi => return,
        _ => None,
    };
    egui::Area::new(egui::Id::new("azione")).anchor(Align2::CENTER_BOTTOM, [0.0, -18.0]).show(ctx, |ui| {
        ui.set_min_width(480.0);
        ui.vertical_centered(|ui| {
            match label {
                Some(l) => {
                    let b = egui::Button::new(RichText::new(l).size(26.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(10.0).min_size(egui::vec2(320.0, 58.0));
                    if ui.add(b).clicked() {
                        match k.phase {
                            Phase::Pianifica => {
                                commit(k, sim);
                                k.enter(Phase::Esegui);
                            }
                            Phase::Esegui => commit(k, sim),
                            Phase::Crisi => close_crisis(k, sim),
                            Phase::Fine => {}
                        }
                    }
                }
                None => {
                    panel().show(ui, |ui| {
                        ui.label(RichText::new("Il piano è in corso. Puoi cambiarlo quando vuoi dalla mappa.").size(15.0).color(GREY));
                    });
                }
            }
        });
    });
}

/// The latest events.
fn events(ctx: &egui::Context, sim: &Sim) {
    if sim.log.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::new("eventi")).anchor(Align2::RIGHT_BOTTOM, [-12.0, -12.0]).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.set_max_width(400.0);
            egui::CollapsingHeader::new(RichText::new("Eventi").size(15.0).strong().color(Color32::WHITE)).default_open(true).show(ui, |ui| {
            for e in sim.log.iter().rev().take(6).collect::<Vec<_>>().into_iter().rev() {
                let colour = if e.text.starts_with("CRISI") { ORANGE } else { GREY };
                ui.label(RichText::new(format!("{}  {}", clock(e.at_s), e.text)).size(15.0).color(colour));
            }
            });
        });
    });
}

/// The end: facts per district against the same fire with no orders.
fn debrief(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    let o = sim.outcome();
    let base = k.baseline();
    let mut again = None;
    egui::Area::new(egui::Id::new("fine")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        egui::Frame::none().fill(Color32::from_rgb(18, 20, 24)).stroke(egui::Stroke::new(1.0, Color32::from_gray(80))).rounding(12.0).inner_margin(22.0).show(ui, |ui| {
            ui.set_max_width(720.0);
            ui.label(RichText::new("Com'è andata").size(30.0).strong().color(Color32::WHITE));
            ui.label(RichText::new(format!("Dopo {} ore di incendio. A confronto: lo stesso incendio, con lo stesso vento, senza nessun ordine.", sim.time_s() / 3600)).size(16.0).color(GREY));
            ui.add_space(10.0);
            let b = base.as_ref().and_then(|r| r.as_ref().ok());
            // the two numbers that matter, big, before the detail
            ui.horizontal(|ui| {
                for (what, mine, theirs) in [("famiglie colte in casa dal fuoco", o.caught(), b.map(|b| b.caught())), ("case colpite", o.homes_hit(), b.map(|b| b.homes_hit()))] {
                    egui::Frame::none().fill(Color32::from_gray(34)).rounding(8.0).inner_margin(12.0).show(ui, |ui| {
                        ui.set_min_width(320.0);
                        ui.label(RichText::new(what).size(16.0).color(GREY));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(mine.to_string()).size(40.0).strong().color(Color32::WHITE));
                            ui.label(RichText::new(theirs.map_or("senza ordini: …".into(), |t| format!("senza ordini: {t}"))).size(18.0).color(GREY));
                        });
                    });
                }
            });
            ui.add_space(8.0);
            egui::Grid::new("esito").spacing([22.0, 8.0]).show(ui, |ui| {
                for h in ["", "case colpite", "", "famiglie colte in casa", "", "famiglie"] {
                    ui.label(RichText::new(h).size(16.0).strong().color(Color32::WHITE));
                }
                ui.end_row();
                for h in ["", "tu", "senza ordini", "tu", "senza ordini", "evacuate"] {
                    ui.label(RichText::new(h).size(15.0).color(GREY));
                }
                ui.end_row();
                for (i, (d, x)) in sim.districts.iter().zip(&o.districts).enumerate() {
                    let bx = b.map(|b| &b.districts[i]);
                    ui.label(RichText::new(&d.name).size(18.0).color(Color32::WHITE));
                    ui.label(RichText::new(x.homes_hit.to_string()).size(20.0).strong().color(Color32::WHITE));
                    ui.label(RichText::new(bx.map_or("…".into(), |b| b.homes_hit.to_string())).size(18.0).color(GREY));
                    ui.label(RichText::new(x.caught.to_string()).size(20.0).strong().color(Color32::WHITE));
                    ui.label(RichText::new(bx.map_or("…".into(), |b| b.caught.to_string())).size(18.0).color(GREY));
                    ui.label(RichText::new(format!("{} su {}", x.evacuated, x.households)).size(18.0).color(GREY));
                    ui.end_row();
                }
            });
            ui.add_space(10.0);
            match &base {
                Some(Ok(b)) => {
                    let homes = b.homes_hit() as i64 - o.homes_hit() as i64;
                    let people = b.caught() as i64 - o.caught() as i64;
                    let say = |n: i64, what: &str| match n {
                        0 => format!("lo stesso numero di {what}"),
                        n if n > 0 => format!("{n} {what} in meno"),
                        n => format!("{} {what} in più", -n),
                    };
                    ui.label(RichText::new(format!("Con il tuo piano: {}, {}.", say(homes, "case colpite"), say(people, "famiglie colte in casa dal fuoco"))).size(20.0).strong().color(AMBER));
                }
                Some(Err(e)) => {
                    ui.label(RichText::new(format!("Confronto non disponibile: {e}")).size(15.0).color(RED));
                }
                None => {
                    ui.label(RichText::new("Calcolo il confronto con lo stesso incendio senza ordini…").size(16.0).color(GREY));
                }
            }
            // why, district by district: what the orders changed and what they could not
            if let Some(b) = b {
                for (i, (d, x)) in sim.districts.iter().zip(&o.districts).enumerate() {
                    let y = &b.districts[i];
                    let mut why = vec![];
                    if y.caught > x.caught {
                        why.push(format!("{} famiglie in meno colte in casa: sono partite prima che arrivasse il fuoco", y.caught - x.caught));
                    }
                    if y.homes_hit > x.homes_hit {
                        why.push(format!("{} case in meno colpite: i mezzi le hanno difese", y.homes_hit - x.homes_hit));
                    } else if x.homes_hit > 0 && x.homes_hit == y.homes_hit {
                        let ranked = sim.log.iter().any(|e| e.text.starts_with("priorità:") && e.text.contains(d.name.as_str()));
                        why.push(if ranked {
                            "le case colpite sono le stesse: i mezzi non sono arrivati in tempo o non c'era una postazione sicura (vedi gli eventi)".into()
                        } else {
                            "le case colpite sono le stesse: non era tra i luoghi da difendere".into()
                        });
                    }
                    let ordered = sim.log.iter().any(|e| e.text.starts_with("evacuazione:") && e.text.contains(d.name.as_str()));
                    if y.homes_hit == 0 && y.caught == 0 && ordered {
                        why.push("qui il fuoco non è arrivato: l'evacuazione è stata una precauzione".into());
                    }
                    if !why.is_empty() {
                        ui.label(RichText::new(format!("{}: {}.", d.name, why.join("; "))).size(15.0).color(Color32::WHITE));
                    }
                }
            }
            // the evacuation orders, with when they were given: the timing is the lesson
            let evac: Vec<String> = sim.log.iter().filter(|e| e.text.starts_with("evacuazione:")).map(|e| format!("{} {}", e.text.trim_start_matches("evacuazione: "), clock(e.at_s))).collect();
            if !evac.is_empty() {
                ui.label(RichText::new(format!("Evacuazioni ordinate: {}. Prima si parte, meno famiglie il fuoco trova in casa.", evac.join(", "))).size(15.0).color(GREY));
            }
            if o.units_lost > 0 {
                let lost: Vec<&str> = sim.crews.units.iter().filter(|u| u.state == abm::suppression::UnitState::Lost).map(|u| u.callsign.as_str()).collect();
                ui.label(RichText::new(format!("Mezzi persi, raggiunti dal fuoco: {}", lost.join(", "))).size(16.0).color(RED));
            }
            // the player's own orders, in order
            let orders: Vec<String> = sim
                .log
                .iter()
                .filter(|e| ["priorità:", "preallerta:", "evacuazione:"].iter().any(|p| e.text.starts_with(p)) || e.text.contains("fuori servizio"))
                .map(|e| format!("{}  {}", clock(e.at_s), e.text))
                .collect();
            if !orders.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("Le tue decisioni (e i mezzi persi)").size(16.0).strong().color(Color32::WHITE));
                for o in orders {
                    ui.label(RichText::new(o).size(15.0).color(GREY));
                }
            }
            egui::Frame::none().fill(Color32::from_rgb(20, 40, 70)).rounding(6.0).inner_margin(8.0).show(ui, |ui| {
                ui.label(RichText::new(format!("Da ricordare: {REAL_LIFE}")).size(15.0).color(Color32::WHITE));
            });
            {
                ui.label(
                    RichText::new("Casa «colpita»: raggiunta dal fuoco nella simulazione, non per forza distrutta. Famiglia «colta in casa»: era ancora in casa quando il fuoco è arrivato. «Evacuate»: famiglie arrivate in un'area sicura, anche dove il fuoco poi non è arrivato.")
                        .size(14.0)
                        .color(GREY),
                );
            }
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let big = |t: &str| egui::Button::new(RichText::new(t).size(22.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(10.0).min_size(egui::vec2(220.0, 50.0));
                if ui.add(big("Riprova")).clicked() {
                    again = Some(k.case.clone());
                }
                if ui.add(big("Altro incendio")).clicked() {
                    again = Some(k.next_case());
                }
            });
        });
    });
    if let Some(case) = again {
        k.case = case;
        new_game(k, sim, restarted);
    }
}

/// The operator bar (F2): any case, seed and speed, a new game, a pause.
fn operator(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    let now = ctx.input(|i| i.time);
    if k.reset_armed.is_some_and(|t| now - t > 5.0) {
        k.reset_armed = None;
    }
    egui::Area::new(egui::Id::new("operatore")).anchor(Align2::LEFT_TOP, [10.0, 10.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.label(RichText::new("Operatore (F2 per chiudere)").strong().color(Color32::WHITE));
            ui.label(RichText::new(format!("Partita in corso: {} · {}", sim.case.name, clock(sim.time_s()))).color(GREY));
            ui.horizontal(|ui| {
                ui.label("Prossima partita:");
                egui::ComboBox::from_id_source("caso").selected_text(k.pick.clone()).show_ui(ui, |ui| {
                    let featured = k.territory.playlist();
                    for c in k.territory.cases.clone() {
                        let star = if featured.contains(&c.name) { " (chiosco)" } else { "" };
                        ui.selectable_value(&mut k.pick, c.name.clone(), format!("{} (vicino a {}){star}", c.name, c.near));
                    }
                });
                ui.label("seme");
                ui.add(egui::DragValue::new(&mut k.seed).range(1..=999));
            });
            ui.horizontal(|ui| {
                // a game under way is thrown away only on a second press
                let started = sim.time_s() > rocca::STEP_S && k.phase != Phase::Fine;
                let label = if k.reset_armed.is_some() { "Sicuro? Premi di nuovo: la partita in corso si perde" } else { "Nuova partita" };
                if ui.add(secondary(label)).clicked() {
                    if started && k.reset_armed.is_none() {
                        k.reset_armed = Some(now);
                    } else {
                        k.reset_armed = None;
                        k.case = k.pick.clone();
                        new_game(k, sim, restarted);
                    }
                }
                if k.phase == Phase::Esegui && ui.add(secondary("Pausa")).clicked() {
                    k.enter(Phase::Pianifica);
                }
            });
            ui.horizontal(|ui| {
                ui.label("velocità ×");
                ui.add(egui::DragValue::new(&mut k.speed).range(1.0..=300.0));
                ui.label("secondi per le crisi");
                for s in [25.0, 40.0, 60.0] {
                    ui.selectable_value(&mut k.crisis_s, s, format!("{s:.0}"));
                }
            });
            if let Some(e) = &k.error {
                ui.colored_label(RED, e);
            }
        });
    });
}
