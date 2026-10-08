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

use super::{close_crisis, commit, new_game, Kiosk, Phase, CRISIS_S};
use crate::sim::{Sim, SimRestarted};

const PANEL: Color32 = Color32::from_rgba_premultiplied(18, 20, 24, 215);
const ORANGE: Color32 = Color32::from_rgb(240, 140, 50);
const RED: Color32 = Color32::from_rgb(235, 80, 60);
const AMBER: Color32 = Color32::from_rgb(250, 195, 70);
const BLUE: Color32 = Color32::from_rgb(110, 185, 255);
const GREEN: Color32 = Color32::from_rgb(110, 230, 150);
/// What the two civil orders do, as the model does it (`Abm::prealert_of`,
/// `Abm::order_evacuation_of`).
const PREALLERTA: &str = "Preallerta: le famiglie vengono avvisate e si preparano, ma restano a casa. Se poi ordini l'evacuazione, partono prima.";
const EVACUA: &str = "Evacua: le famiglie ricevono l'ordine di partire subito verso un'area sicura. Non si può annullare.";
const GREY: Color32 = Color32::from_rgb(205, 208, 212);

fn clock(s: i64) -> String {
    format!("T+{}:{:02}", s / 3600, (s / 60) % 60)
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

    let top = top_bar(ctx, k, &sim, cam);
    if k.phase == Phase::Fine {
        debrief(ctx, k, &mut sim, &mut restarted);
    } else {
        unit_labels(ctx, &sim, cam);
        district_chips(ctx, k, &sim, cam, top);
        proposal(ctx, k, &sim);
        action(ctx, k, &mut sim);
    }
    events(ctx, &sim);
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
                ui.label(RichText::new(clock(sim.time_s())).size(20.0).color(Color32::WHITE).monospace());
                ui.separator();
                let (text, colour) = match k.phase {
                    Phase::Pianifica => ("PIANIFICA · tempo fermo".to_string(), GREY),
                    Phase::Esegui => (format!("IN CORSO · tempo accelerato ×{:.0}", k.speed), GREEN),
                    Phase::Crisi => (format!("CRISI · ×1 · {:.0} s", (CRISIS_S - k.phase_t).max(0.0).ceil()), ORANGE),
                    Phase::Fine => ("FINE".to_string(), GREY),
                };
                ui.label(RichText::new(text).size(20.0).strong().color(colour));
            });
        });
        match (k.phase, &k.crisis) {
            (Phase::Crisi, Some(c)) => {
                ui.add_space(6.0);
                egui::Frame::none().fill(Color32::from_rgba_premultiplied(70, 30, 8, 235)).stroke(egui::Stroke::new(2.0, ORANGE)).rounding(8.0).inner_margin(12.0).show(ui, |ui| {
                    ui.set_max_width(760.0);
                    ui.label(RichText::new("Decisione critica").size(22.0).strong().color(ORANGE));
                    ui.label(RichText::new(&c.text).size(19.0).color(Color32::WHITE));
                    ui.add_space(4.0);
                    let left = (1.0 - k.phase_t / CRISIS_S).clamp(0.0, 1.0);
                    ui.add(egui::ProgressBar::new(left).desired_height(8.0).fill(ORANGE));
                    ui.label(RichText::new("Il fuoco non si ferma. Puoi cambiare priorità e ordini ai cittadini, poi premere Conferma. Se non fai nulla resta il piano attuale.").size(15.0).color(GREY));
                });
            }
            (Phase::Pianifica, _) if sim.time_s() <= rocca::STEP_S => {
                ui.add_space(6.0);
                panel().show(ui, |ui| {
                    ui.set_max_width(760.0);
                    ui.label(RichText::new(format!("Nuovo incendio, vicino a {}. Il tempo è fermo.", sim.case.near)).size(19.0).strong().color(Color32::WHITE));
                    ui.label(RichText::new("Sulla mappa scegli quali luoghi difendere e in che ordine: la sala operativa manda lì i mezzi (autobotti e squadre), tu non li guidi. Il fuoco decide il resto.").size(16.0).color(GREY));
                    ui.label(RichText::new(PREALLERTA).size(16.0).color(AMBER));
                    ui.label(RichText::new(EVACUA).size(16.0).color(BLUE));
                });
            }
            _ => {}
        }
    })
    .response
    .rect
    .bottom()
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
            let size = ctx.memory(|m| m.area_rect(egui::Id::new(("quartiere", d)))).map_or(egui::vec2(260.0, 150.0), |r| r.size());
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
                place[a] = Some((pa, sa));
                place[b] = Some((pb, sb));
            }
        }
    }
    for d in 0..n {
        let dist = &sim.districts[d];
        let Some((at, _)) = place[d] else { continue };
        let id = egui::Id::new(("quartiere", d));
        let rank = order.iter().position(|&x| x == d);
        egui::Area::new(id).fixed_pos(at).pivot(Align2::CENTER_BOTTOM).show(ctx, |ui| {
            let border = if rank == Some(0) { AMBER } else { Color32::from_gray(90) };
            egui::Frame::none().fill(PANEL).stroke(egui::Stroke::new(1.5, border)).rounding(8.0).inner_margin(8.0).show(ui, |ui| {
                ui.set_min_width(250.0);
                ui.horizontal(|ui| {
                    let badge = rank.map_or("·".to_string(), |r| (r + 1).to_string());
                    ui.label(RichText::new(badge).size(30.0).strong().color(if rank.is_some() { AMBER } else { GREY }));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&dist.name).size(20.0).strong().color(Color32::WHITE));
                        let (risk, colour) = match k.risk.get(d).copied().flatten() {
                            Some(e) if e.distance_m < 1000.0 => (format!("fuoco a {}", km(e.distance_m)), RED),
                            Some(e) if e.distance_m < 2500.0 => (format!("fuoco a {}", km(e.distance_m)), ORANGE),
                            Some(e) => (format!("fuoco a {}", km(e.distance_m)), GREY),
                            None => ("nessun fuoco".to_string(), GREY),
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{} famiglie ·", dist.households.len())).size(15.0).color(GREY));
                            ui.label(RichText::new(risk).size(15.0).strong().color(colour));
                        });
                    });
                });
                let now = sim.posts.iter().flatten().filter(|p| p.district == d).count();
                let next = k.preview.as_ref().map_or(now, |p| p.units_on(d));
                ui.label(RichText::new(format!("mezzi assegnati: {now}")).size(16.0).color(if now > 0 { GREEN } else { GREY }));
                if next < now {
                    ui.label(RichText::new(format!("con il nuovo piano ne perde {}", now - next)).size(16.0).strong().color(ORANGE));
                } else if next > now {
                    ui.label(RichText::new(format!("con il nuovo piano ne riceve {}", next - now)).size(16.0).strong().color(GREEN));
                }
                ui.horizontal(|ui| match rank {
                    Some(r) => {
                        if r > 0 && ui.button(RichText::new("più importante").size(15.0)).clicked() {
                            order.swap(r, r - 1);
                            changed = true;
                        }
                        if ui.button(RichText::new("non difendere").size(15.0)).clicked() {
                            order.retain(|&x| x != d);
                            changed = true;
                        }
                    }
                    None => {
                        if ui.button(RichText::new("Difendi").size(17.0).strong()).clicked() {
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
                        let text = RichText::new(label).size(16.0).color(if on { colour } else { Color32::WHITE });
                        let hint = if c == Civil::Preallerta { PREALLERTA } else { EVACUA };
                        if ui.add_enabled(!locked, egui::SelectableLabel::new(on, text)).on_hover_text(hint).clicked() {
                            civil[d] = if on { active } else { c.max(active) };
                            changed = true;
                        }
                    }
                });
                if sim.active.civil[d] != Civil::Nessuno {
                    let hh = &dist.households;
                    let safe = hh.iter().filter(|&&i| sim.agents.households[i].status == Status::Evacuated).count();
                    let road = hh.iter().filter(|&&i| sim.agents.households[i].status == Status::Evacuating).count();
                    let what = if sim.active.civil[d] == Civil::Evacua { "evacuazione ordinata" } else { "preallertati" };
                    ui.label(RichText::new(format!("{what}: {safe} in salvo, {road} in strada")).size(15.0).color(GREY));
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
        let colour = if status.starts_with("bloccato") || status.starts_with("fuori") { RED } else if status.starts_with("si ritira") { ORANGE } else { GREEN };
        egui::Area::new(egui::Id::new(("mezzo", i))).fixed_pos(at).pivot(Align2::CENTER_BOTTOM).interactable(false).order(egui::Order::Background).show(ctx, |ui| {
            egui::Frame::none().fill(Color32::from_rgba_premultiplied(10, 12, 14, 190)).rounding(4.0).inner_margin(egui::Margin::symmetric(6.0, 2.0)).show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(format!("{} · {status}", u.callsign)).size(15.0).color(colour)).extend());
            });
        });
    }
}

/// What the coordinator would do with the plan being composed, and why.
fn proposal(ctx: &egui::Context, k: &Kiosk, sim: &Sim) {
    let Some(p) = &k.preview else { return };
    let pending = k.proposed != sim.active || k.phase == Phase::Pianifica;
    egui::Area::new(egui::Id::new("proposta")).anchor(Align2::LEFT_BOTTOM, [12.0, -12.0]).interactable(false).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.set_max_width(430.0);
            let title = if pending { "Il coordinatore propone (anteprima, non è garantito)" } else { "Il coordinatore" };
            ui.label(RichText::new(title).size(16.0).strong().color(Color32::WHITE));
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
}

/// The one button.
fn action(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim) {
    let pending = k.proposed != sim.active;
    let label = match k.phase {
        Phase::Pianifica => Some("Conferma e avvia"),
        Phase::Esegui if pending => Some("Conferma il nuovo piano"),
        Phase::Crisi if pending => Some("Conferma"),
        Phase::Crisi => Some("Continua con il piano attuale"),
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
    egui::Area::new(egui::Id::new("eventi")).anchor(Align2::RIGHT_BOTTOM, [-12.0, -12.0]).interactable(false).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.set_max_width(400.0);
            ui.label(RichText::new("Eventi").size(16.0).strong().color(Color32::WHITE));
            for e in sim.log.iter().rev().take(6).collect::<Vec<_>>().into_iter().rev() {
                let colour = if e.text.starts_with("CRISI") { ORANGE } else { GREY };
                ui.label(RichText::new(format!("{}  {}", clock(e.at_s), e.text)).size(15.0).color(colour));
            }
        });
    });
}

/// The end: facts per district against the same fire with no orders.
fn debrief(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    let o = sim.outcome();
    let base = k.baseline();
    let mut again = None;
    egui::Area::new(egui::Id::new("fine")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        egui::Frame::none().fill(Color32::from_rgba_premultiplied(18, 20, 24, 240)).rounding(12.0).inner_margin(22.0).show(ui, |ui| {
            ui.set_max_width(720.0);
            ui.label(RichText::new("Com'è andata").size(30.0).strong().color(Color32::WHITE));
            ui.label(RichText::new(format!("Dopo {} ore di incendio. A confronto: lo stesso incendio, con lo stesso vento, senza nessun ordine.", sim.time_s() / 3600)).size(16.0).color(GREY));
            ui.add_space(10.0);
            let b = base.as_ref().and_then(|r| r.as_ref().ok());
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
            if o.units_lost > 0 {
                ui.label(RichText::new(format!("Mezzi persi, raggiunti dal fuoco: {}", o.units_lost)).size(16.0).color(RED));
            }
            ui.label(
                RichText::new("Casa «colpita»: raggiunta dal fuoco nella simulazione, non per forza distrutta. Famiglia «colta in casa»: era ancora in casa quando il fuoco è arrivato. «Evacuate»: famiglie arrivate in un'area sicura, anche dove il fuoco poi non è arrivato.")
                    .size(15.0)
                    .color(GREY),
            );
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
    egui::Area::new(egui::Id::new("operatore")).anchor(Align2::LEFT_TOP, [10.0, 10.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.label(RichText::new("Operatore (F2)").strong().color(Color32::WHITE));
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_source("caso").selected_text(k.case.clone()).show_ui(ui, |ui| {
                    let featured = k.territory.playlist();
                    for c in k.territory.cases.clone() {
                        let star = if featured.contains(&c.name) { " (chiosco)" } else { "" };
                        ui.selectable_value(&mut k.case, c.name.clone(), format!("{} (vicino a {}){star}", c.name, c.near));
                    }
                });
                ui.label("seme");
                ui.add(egui::DragValue::new(&mut k.seed).range(1..=999));
                if ui.button("Nuova partita").clicked() {
                    new_game(k, sim, restarted);
                }
            });
            ui.horizontal(|ui| {
                ui.label("velocità ×");
                ui.add(egui::DragValue::new(&mut k.speed).range(1.0..=300.0));
                match k.phase {
                    Phase::Esegui if ui.button("Pausa").clicked() => k.enter(Phase::Pianifica),
                    _ => {}
                }
            });
            if let Some(e) = &k.error {
                ui.colored_label(RED, e);
            }
        });
    });
}
