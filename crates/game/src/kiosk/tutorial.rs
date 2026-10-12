//! A short tour of the screen when a visitor starts playing: the camera
//! flies to what each step is about, the rest of the screen dims, and the
//! volunteer says what it is. Not interactive (user, 2026-10-12): only
//! «Avanti» and «Salta»; the map and the panels are shown, not used. The
//! world stands still meanwhile (×0, before «Avvia»).

use bevy::prelude::*;
use bevy_egui::egui;
use egui::{Color32, RichText};

use super::characters::{self, Mood, Portraits, Who};
use super::ui::{chip_anchor, primary, screen, secondary, section_rect, the_fire, AMBER, FACE, ORANGE};
use super::{Kiosk, Shot, MAX_PAUSES};
use crate::sim::Sim;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    /// Where the fire started and where the wind drives it.
    Fuoco,
    /// The districts' badges.
    Paesi,
    /// A district's opened chip: defend, pre-alert, evacuate.
    Scheda,
    /// «Il piano», top left.
    Piano,
    /// The button that starts the time.
    Avvia,
}

const STEPS: [Step; 5] = [Step::Fuoco, Step::Paesi, Step::Scheda, Step::Piano, Step::Avvia];

/// Widest the tour's bubble gets.
const TOUR_W: f32 = 460.0;

/// The district the tour opens: the one the fire was picked for.
fn near(sim: &Sim) -> usize {
    sim.district_index(&sim.case.near).unwrap_or(0)
}

/// Show step `i` of the tour, moving the camera to it; past the last step,
/// end the tour and fly back to the home view.
pub fn go(k: &mut Kiosk, sim: &Sim, i: usize) {
    k.selected = None;
    let Some(&step) = STEPS.get(i) else {
        k.tour = None;
        k.toured = true;
        k.shot = Some(Shot { at: None, zoom: 1.0 });
        return;
    };
    k.tour = Some(i);
    k.shot = Some(match step {
        Step::Fuoco => Shot { at: Some(sim.case.ignition()), zoom: 0.45 },
        Step::Scheda => {
            k.selected = Some(near(sim));
            Shot { at: Some(chip_anchor(sim, near(sim))), zoom: 0.65 }
        }
        Step::Paesi | Step::Piano | Step::Avvia => Shot { at: None, zoom: 1.0 },
    });
}

fn text(step: Step, k: &Kiosk, sim: &Sim) -> String {
    match step {
        Step::Fuoco => {
            // «benvenuto/a» would guess the player's gender from a name
            let hello = if k.player.is_empty() { "Ciao, grazie dell'aiuto!".to_string() } else { format!("Ciao {}, grazie dell'aiuto!", k.player) };
            format!("{hello} {} Il cerchio arancione segna il punto in cui è partito.", the_fire(k, sim))
        }
        Step::Paesi => "Questi sono i paesi da proteggere. Su ogni etichetta vedi quanto è lontano il fuoco, i mezzi che ci lavorano e gli ordini dati alle famiglie.".into(),
        Step::Scheda => "Cliccando su un paese si apre la sua scheda. «Difendi» ci manda autobotti e squadre. «Preallerta» fa preparare le famiglie in casa, «Evacua» le fa preparare e partire. Un ordine alle famiglie non si ritira.".into(),
        Step::Piano => "Nella barra a destra c'è il tuo piano: i paesi da difendere, in ordine. Le squadre vanno dove indichi, se ci arrivano in tempo: essere primi non garantisce di essere salvati.".into(),
        Step::Avvia => format!(
            "Quando sei pronto premi «Avvia»: il tempo comincia a scorrere. Per cambiare gli ordini servono le pause, e ne hai solo {MAX_PAUSES}. Se succede qualcosa di grave il tempo rallenta, ma non si ferma: avrai {:.0} secondi per decidere.",
            k.crisis_s
        ),
    }
}

/// What the step points at, on screen: the areas the interface drew last
/// frame, or the fire's start projected.
fn targets(step: Step, ctx: &egui::Context, sim: &Sim, cam: (&Camera, &GlobalTransform)) -> Vec<egui::Rect> {
    let area = |id: egui::Id| ctx.memory(|m| m.area_rect(id));
    match step {
        Step::Fuoco => screen(cam, sim, sim.case.ignition(), 0.0)
            .map(|p| egui::Rect::from_min_max(p - egui::vec2(110.0, 90.0), p + egui::vec2(110.0, 60.0)))
            .into_iter()
            .collect(),
        Step::Paesi => (0..sim.districts.len()).filter_map(|d| area(egui::Id::new(("badge", d)))).collect(),
        Step::Scheda => {
            let d = near(sim);
            let both = [area(egui::Id::new(("badge", d))), area(egui::Id::new(("quartiere", d)))];
            both.into_iter().flatten().reduce(|a, b| a.union(b)).into_iter().collect()
        }
        Step::Piano => section_rect(ctx, "piano").into_iter().collect(),
        Step::Avvia => section_rect(ctx, "azione").into_iter().collect(),
    }
}

/// The screen less the holes, as rectangles: the grid cut by the holes'
/// edges, keeping the cells outside every hole.
fn veil(screen: egui::Rect, holes: &[egui::Rect]) -> Vec<egui::Rect> {
    let cuts = |a: f32, b: f32, edges: &mut dyn Iterator<Item = f32>| {
        let mut v: Vec<f32> = std::iter::once(a).chain(edges.filter(|x| *x > a && *x < b)).chain(std::iter::once(b)).collect();
        v.sort_by(f32::total_cmp);
        v.dedup();
        v
    };
    let xs = cuts(screen.left(), screen.right(), &mut holes.iter().flat_map(|h| [h.left(), h.right()]));
    let ys = cuts(screen.top(), screen.bottom(), &mut holes.iter().flat_map(|h| [h.top(), h.bottom()]));
    let mut out = vec![];
    for x in xs.windows(2) {
        for y in ys.windows(2) {
            let cell = egui::Rect::from_min_max(egui::pos2(x[0], y[0]), egui::pos2(x[1], y[1]));
            if !holes.iter().any(|h| h.contains(cell.center())) {
                out.push(cell);
            }
        }
    }
    out
}

/// Where the bubble goes on the map: at the foot if free, else at the top,
/// the right or the left, the first place clear of what the step points at.
fn place(screen: egui::Rect, holes: &[egui::Rect], size: egui::Vec2) -> egui::Pos2 {
    let mid = screen.center();
    let spots = [
        egui::pos2(mid.x - size.x * 0.5, screen.bottom() - size.y - 40.0),
        egui::pos2(mid.x - size.x * 0.5, screen.top() + 80.0),
        egui::pos2(screen.right() - size.x - 24.0, mid.y - size.y * 0.5),
        egui::pos2(screen.left() + 290.0, mid.y - size.y * 0.5),
    ];
    spots.into_iter().find(|p| !holes.iter().any(|h| h.expand(16.0).intersects(egui::Rect::from_min_size(*p, size)))).unwrap_or(spots[0])
}

/// The tour over everything else: the dimmed screen with the step's holes,
/// the volunteer's bubble with «Avanti» and «Salta», an arrow to the hole.
/// The veil takes every click, so nothing under it can be used.
pub fn draw(ctx: &egui::Context, k: &mut Kiosk, sim: &Sim, cam: (&Camera, &GlobalTransform), faces: Option<&Portraits>) {
    let Some(i) = k.tour else { return };
    let Some(&step) = STEPS.get(i) else { return };
    let whole = ctx.screen_rect();
    let holes: Vec<egui::Rect> = targets(step, ctx, sim, cam).into_iter().map(|r| r.expand(10.0)).collect();
    let size_id = egui::Id::new("tutorial_fumetto");
    let size = ctx.data(|d| d.get_temp::<egui::Vec2>(size_id)).unwrap_or(egui::vec2(TOUR_W + FACE * 0.7 + 60.0, 220.0));
    // the bubble on the map, clear of the side bar
    let at = place(ctx.available_rect(), &holes, size);
    let t = ctx.input(|i| i.time) as f32;
    let mut next = None;
    egui::Area::new(egui::Id::new("tutorial")).fixed_pos(whole.min).order(egui::Order::Tooltip).show(ctx, |ui| {
        ui.allocate_rect(whole, egui::Sense::click_and_drag());
        let p = ui.painter().clone();
        for cell in veil(whole, &holes) {
            p.rect_filled(cell, 0.0, Color32::from_black_alpha(215));
        }
        let glow = 0.55 + 0.45 * (t * 3.0).sin();
        for h in &holes {
            p.rect_stroke(*h, 10.0, egui::Stroke::new(4.0, AMBER.gamma_multiply(glow)));
        }
        let shown = ui.allocate_ui_at_rect(egui::Rect::from_min_size(at, egui::vec2(size.x, whole.height())), |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    characters::portrait(ui, faces, Who::Volontaria, Mood::Calmo, FACE * 0.7);
                });
                ui.add_space(18.0);
                characters::bubble(ui, Who::Volontaria, |ui| ui.vertical(|ui| {
                    ui.set_max_width(TOUR_W);
                    characters::says(ui, Who::Volontaria, &text(step, k, sim), TOUR_W);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let last = i + 1 == STEPS.len();
                        if !last && ui.add(secondary("Salta")).clicked() {
                            next = Some(STEPS.len());
                        }
                        if ui.add(secondary("Esci")).clicked() {
                            k.leave = true;
                        }
                        ui.label(RichText::new(format!("{}/{}", i + 1, STEPS.len())).size(15.0).color(Color32::from_gray(110)));
                        // pushed right by a space, not by a right-to-left
                        // layout: that one also puts the text at the
                        // button's right edge (playtest colleghi: «Ho capito»
                        // aligned at random)
                        let label = if last { "Ho capito, gioca" } else { "Avanti" };
                        let text_w = ui.painter().layout_no_wrap(label.into(), egui::FontId::proportional(24.0), Color32::BLACK).size().x;
                        let w = (text_w + 2.0 * ui.spacing().button_padding.x).max(180.0);
                        let b = primary(label).min_size(egui::vec2(w, 48.0));
                        ui.add_space((ui.available_width() - w).max(0.0));
                        if ui.add(b).clicked() {
                            next = Some(i + 1);
                        }
                    });
                }));
            });
        });
        let bubble = shown.response.rect;
        ctx.data_mut(|d| d.insert_temp(size_id, bubble.size()));
        // an arrow from the bubble to the nearest hole
        if let Some(h) = holes.iter().min_by(|a, b| a.distance_to_pos(bubble.center()).total_cmp(&b.distance_to_pos(bubble.center()))) {
            let to = h.clamp(bubble.center());
            let from = bubble.clamp(to);
            let v = to - from;
            if v.length() > 40.0 {
                let v = v - v.normalized() * 8.0;
                p.arrow(from, v, egui::Stroke::new(4.0, ORANGE));
            }
        }
    });
    if let Some(n) = next {
        go(k, sim, n);
    }
    ctx.request_repaint();
}
