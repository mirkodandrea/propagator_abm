//! The kiosk's interface: the map is the screen.
//!
//! - **On the map**, one chip per district: its rank, how far the fire is,
//!   the units on it now and after confirmation, the controls to rank it and
//!   the two civil orders. Each ground unit carries a label of what it is
//!   doing. Routes and posts are drawn in 3D by `overlays::update_routes`.
//! - **Top**: wind, clock and phase.
//! - **Bottom left**: someone speaking (`characters`): the mayor, the fire
//!   chief, the forecaster or a volunteer, with one line about what just
//!   happened; at a crisis, the crisis with its countdown and answers.
//! - **Bottom centre**: the one button (Avvia / Conferma).
//! - **Bottom right**: what the coordinator proposes and why, and the
//!   latest events, both closed by default.
//! - **Fine**: the debrief, against the same fire with no orders, with
//!   «Riprova» and «Altro incendio».
//! - **F2**: the operator bar (case, seed, speed, new game). Hidden by default.
//!
//! Reads the game and the plan being composed; every change goes through
//! `kiosk::commit`, `kiosk::close_crisis` or `kiosk::new_game`.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui::{Align2, Color32, RichText};
use rocca::words::km;
use rocca::Civil;
use scenario::population::Status;
use scenario::Pos;

use super::characters::{self, Line, Mood, Portraits, Speech, Who};
use super::icons::{self, Icon};
use super::{close_crisis, commit, new_game, Kiosk, Phase};
use crate::sim::{Sim, SimRestarted};

const PANEL: Color32 = Color32::from_rgba_premultiplied(18, 20, 24, 250);
const ORANGE: Color32 = Color32::from_rgb(240, 140, 50);
const RED: Color32 = Color32::from_rgb(255, 105, 85);
const AMBER: Color32 = Color32::from_rgb(250, 195, 70);
const BLUE: Color32 = Color32::from_rgb(110, 185, 255);
const GREEN: Color32 = Color32::from_rgb(110, 230, 150);
/// What the two civil orders do, as the model does it (`Abm::prealert_of`,
/// `Abm::order_evacuation_of`).
/// Room a district chip is given on screen, for placing chips apart.
const CHIP_W: f32 = 320.0;
const CHIP_H: f32 = 350.0;
/// A chip while the plan runs untouched: name, families, defence. It opens
/// downward to the full chip under the pointer.
const CHIP_H_COMPACT: f32 = 175.0;
/// What the simulated families do is not what to do: said where it shows.
const REAL_LIFE: &str = "Nella realtà, quando arriva l'ordine di evacuazione si parte subito. Aspettare di vedere il fuoco è uno degli errori più pericolosi.";
const GREY: Color32 = Color32::from_rgb(205, 208, 212);

fn clock(s: i64) -> String {
    format!("T+{}:{:02}", s / 3600, (s / 60) % 60)
}

/// «Autobotte 1» as «A1», «Squadra A» as «SQ»: a badge, not a sentence.
fn short(callsign: &str) -> String {
    if let Some(n) = callsign.strip_prefix("Autobotte ") {
        format!("A{n}")
    } else if callsign.starts_with("Squadra") {
        "SQ".into()
    } else {
        callsign.chars().take(3).collect()
    }
}

/// Engine icons in a row: `now` solid; what the new plan adds, outlined in
/// amber; what it takes away, crossed in red.
fn trucks(ui: &mut egui::Ui, now: usize, next: usize) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for k in 0..now.max(next).max(1) {
            let (r, _) = ui.allocate_exact_size(egui::vec2(30.0, 26.0), egui::Sense::hover());
            let p = ui.painter();
            if k < now.min(next) {
                icons::draw(p, r, Icon::Engine, GREEN);
            } else if k < next {
                icons::draw(p, r, Icon::Engine, AMBER.gamma_multiply(0.5));
                p.rect_stroke(r.shrink(1.0), 4.0, egui::Stroke::new(2.0, AMBER));
            } else if k < now {
                icons::draw(p, r, Icon::Engine, GREEN.gamma_multiply(0.4));
                icons::draw(p, r.shrink(4.0), Icon::Cross, RED);
            } else {
                icons::draw(p, r, Icon::Engine, Color32::from_gray(70));
            }
        }
    });
}


/// One family of controls: a secondary action (grey), an order that can be
/// on (filled with its colour) or off, and the one primary action (yellow).
fn secondary(text: &str) -> egui::Button<'static> {
    // 44 px: a finger, not a mouse pointer
    egui::Button::new(RichText::new(text.to_string()).size(17.0).color(Color32::WHITE))
        .fill(Color32::from_gray(62))
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(110)))
        .rounding(8.0)
        .min_size(egui::vec2(44.0, 44.0))
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
    logo: Option<Res<Logo>>,
    portraits: Option<Res<Portraits>>,
    mut speech: ResMut<Speech>,
) {
    let faces = portraits.as_deref();
    let ctx = contexts.ctx_mut();
    let k = &mut *kiosk;
    let Ok(cam) = cameras.get_single() else { return };
    if ctx.input(|i| i.key_pressed(egui::Key::F2)) {
        k.operator = !k.operator;
    }

    if k.intro {
        intro(ctx, k, &sim, logo.as_deref().map(|l| l.0), faces);
        if k.operator {
            operator(ctx, k, &mut sim, &mut restarted);
        }
        focus.pointer = true;
        return;
    }
    let top = top_bar(ctx, k, &sim, cam);
    view_controls(ctx, k);
    speech.update(&sim, ctx.input(|i| i.time));
    if k.phase == Phase::Fine {
        debrief(ctx, k, &mut sim, &mut restarted);
    } else {
        legend(ctx);
        if k.phase == Phase::Crisi {
            crisis(ctx, k, &mut sim, faces);
        } else {
            speaker(ctx, k, &sim, &speech, faces);
        }
        unit_labels(ctx, &sim, cam);
        district_chips(ctx, k, &sim, cam, top);
        side(ctx, k, &sim);
        action(ctx, k, &mut sim);
    }
    if k.operator {
        operator(ctx, k, &mut sim, &mut restarted);
    }
    focus.pointer = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

/// Wind, time as a bar, and the pace as an icon; under it at the start, one
/// short line of what to do. Returns where the band ends.
fn top_bar(ctx: &egui::Context, k: &Kiosk, sim: &Sim, cam: (&Camera, &GlobalTransform)) -> f32 {
    egui::Area::new(egui::Id::new("barra")).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, [0.0, 10.0]).interactable(false).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.horizontal(|ui| {
                let w = sim.fire.weather();
                // an arrow the way the wind pushes the fire, as seen on screen
                let (rect, _) = ui.allocate_exact_size(egui::vec2(38.0, 38.0), egui::Sense::hover());
                let from = (w.wind_dir_deg as f32).to_radians();
                let at = sim.case.ignition();
                let to = Pos { x: at.x - from.sin() * 400.0, y: at.y - from.cos() * 400.0 };
                if let (Some(a), Some(b)) = (screen(cam, sim, at, 0.0), screen(cam, sim, to, 0.0)) {
                    let d = (b - a).normalized() * 15.0;
                    let c = rect.center();
                    let stroke = egui::Stroke::new(4.0, Color32::WHITE);
                    ui.painter().line_segment([c - d, c + d], stroke);
                    let side = egui::vec2(-d.y, d.x) * 0.5;
                    ui.painter().line_segment([c + d, c + d * 0.35 + side], stroke);
                    ui.painter().line_segment([c + d, c + d * 0.35 - side], stroke);
                }
                ui.label(RichText::new(format!("{:.0} km/h", w.wind_speed_kmh)).size(22.0).strong().color(Color32::WHITE));
                ui.add_space(14.0);
                // the three hours as a bar
                let frac = (sim.time_s() as f32 / sim.case.duration_s() as f32).clamp(0.0, 1.0);
                let (r, _) = ui.allocate_exact_size(egui::vec2(170.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 5.0, Color32::from_gray(60));
                ui.painter().rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(r.width() * frac, r.height())), 5.0, ORANGE);
                ui.label(RichText::new(format!("{} / {} h", clock(sim.time_s()), sim.case.duration_s() / 3600)).size(22.0).color(Color32::WHITE).monospace());
                ui.add_space(10.0);
                let (icon, colour) = match k.phase {
                    Phase::Pianifica => (Icon::Pause, if sim.time_s() > rocca::STEP_S { AMBER } else { GREY }),
                    Phase::Esegui if sim.speed > k.speed => (Icon::Fast, GREEN),
                    Phase::Esegui => (Icon::Play, GREEN),
                    Phase::Crisi => (Icon::Alert, ORANGE),
                    Phase::Fine => (Icon::Check, GREY),
                };
                icons::show(ui, icon, 30.0, colour);
                let pace = match k.phase {
                    Phase::Pianifica => "In pausa".into(),
                    Phase::Esegui => format!("×{:.0}", sim.speed),
                    Phase::Crisi => "Decisione · ×1".into(),
                    Phase::Fine => "Concluso".into(),
                };
                ui.label(RichText::new(pace).size(17.0).color(colour));
            });
        });
    })
    .response
    .rect
    .bottom()
}

/// The CIMA logo, as an egui texture.
#[derive(Resource)]
pub struct Logo(pub egui::TextureId);

/// Load the logo once: compiled in, so the browser build has it too.
pub fn load_logo(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut contexts: EguiContexts) {
    let bytes = include_bytes!("../../../../assets/brand/cima_logo_white.png");
    let Ok(img) = Image::from_buffer(
        bytes,
        bevy::render::texture::ImageType::Extension("png"),
        bevy::render::texture::CompressedImageFormats::NONE,
        true,
        bevy::render::texture::ImageSampler::linear(),
        bevy::render::render_asset::RenderAssetUsages::all(),
    ) else {
        return;
    };
    let id = contexts.add_image(images.add(img));
    commands.insert_resource(Logo(id));
}

/// The opening screen: who made it, the situation, how to play, «Inizia».
fn intro(ctx: &egui::Context, k: &mut Kiosk, sim: &Sim, logo: Option<egui::TextureId>, faces: Option<&Portraits>) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("intro_fondo")).fixed_pos(screen.min).order(egui::Order::Middle).interactable(false).show(ctx, |ui| {
        ui.painter().rect_filled(screen, 0.0, Color32::from_rgba_premultiplied(8, 10, 14, 200));
    });
    egui::Area::new(egui::Id::new("intro")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::none().fill(Color32::from_rgb(18, 20, 24)).stroke(egui::Stroke::new(1.0, Color32::from_gray(80))).rounding(14.0).inner_margin(28.0).show(ui, |ui| {
            ui.set_width((screen.width() - 64.0).min(860.0));
            ui.horizontal(|ui| {
                if let Some(t) = logo {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(t, egui::vec2(77.0, 96.0))));
                    ui.add_space(14.0);
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new("Rocca Ventosa").size(42.0).strong().color(Color32::WHITE));
                    ui.label(RichText::new("Un incendio, tre paesi, poche squadre. Decidi tu le priorità.").size(20.0).color(AMBER));
                });
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                characters::portrait(ui, faces, Who::Volontaria, Mood::Preoccupato, 110.0);
                ui.add_space(18.0);
                characters::bubble(ui, Who::Volontaria, |ui| {
                    let text = format!("Un incendio è partito vicino a {}! Abbiamo 2 autobotti e 1 squadra per 3 paesi: non bastano per tutti. Decidi tu come usarle.", sim.case.near);
                    characters::says(ui, Who::Volontaria, &text, 600.0);
                });
            });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                for (icon, colour, title) in [
                    (Icon::Shield, AMBER, "Scegli chi difendere"),
                    (Icon::Exit, BLUE, "Avvisa o fai evacuare"),
                    (Icon::Play, GREEN, "Conferma e osserva"),
                ] {
                    egui::Frame::none().fill(Color32::from_gray(32)).rounding(10.0).inner_margin(14.0).show(ui, |ui| {
                        ui.set_width(230.0);
                        ui.vertical_centered(|ui| {
                            icons::show(ui, icon, 64.0, colour);
                            ui.label(RichText::new(title).size(19.0).strong().color(Color32::WHITE));
                        });
                    });
                }
            });
            ui.add_space(6.0);
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                let b = egui::Button::new(RichText::new("Inizia").size(30.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(12.0).min_size(egui::vec2(320.0, 70.0));
                if ui.add(b).clicked() {
                    k.intro = false;
                }
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new("Territorio immaginario, fuoco simulato con PROPAGATOR di Fondazione CIMA. Nella realtà segui sempre le indicazioni delle autorità.")
                    .size(14.0)
                    .color(GREY),
            );
        });
    });
}

/// The district a crisis is about, if any.
fn crisis_district(k: &Kiosk) -> Option<usize> {
    use rocca::crisis::Kind;
    match k.crisis.as_ref()?.kind {
        Kind::Scoperto { district } | Kind::Previsione { district } | Kind::Vento { district } => Some(district),
        Kind::MezzoPerso { .. } => None,
    }
}

/// Size of the portraits bottom left.
const FACE: f32 = 132.0;
/// Widest a speech bubble gets.
const BUBBLE_W: f32 = 380.0;

/// Someone speaking, bottom left: the portrait and the bubble, and under
/// them whatever answers it (the crisis controls).
fn speaking<R>(ctx: &egui::Context, faces: Option<&Portraits>, who: Who, mood: Mood, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Area::new(egui::Id::new("personaggio")).order(egui::Order::Foreground).anchor(Align2::LEFT_BOTTOM, [12.0, -12.0]).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.add_space(8.0);
                characters::portrait(ui, faces, who, mood, FACE);
            });
            ui.add_space(18.0);
            ui.vertical(|ui| add(ui)).inner
        })
        .inner
    })
    .inner
}

/// Outside a crisis: the latest line, or at the start how to begin.
fn speaker(ctx: &egui::Context, k: &Kiosk, sim: &Sim, speech: &Speech, faces: Option<&Portraits>) {
    let tip;
    let line = match &speech.now {
        Some((l, _)) => l,
        None if k.phase == Phase::Pianifica => {
            let text = if sim.time_s() <= rocca::STEP_S {
                format!("{} Scegli chi difendere per primo e chi avvisare, poi premi «Avvia».", the_fire(k, sim))
            } else {
                "Siamo in pausa: puoi cambiare il piano, poi premi «Conferma e riprendi».".into()
            };
            tip = Line { who: Who::Volontaria, mood: Mood::Calmo, text };
            &tip
        }
        // how to play only at first; then something new from the game
        None if k.phase == Phase::Esegui && sim.time_s() < TUTORIAL_S => {
            tip = Line { who: Who::Volontaria, mood: Mood::Calmo,
                text: "Il piano è in corso. Guarda arrivi ed evacuazioni nelle schede: puoi cambiare gli ordini e premere Conferma, oppure mettere in pausa.".into() };
            &tip
        }
        None if k.phase == Phase::Esegui => {
            let facts = news(k, sim);
            if facts.is_empty() {
                return;
            }
            let (text, worried) = facts[(ctx.input(|i| i.time) / NEWS_S) as usize % facts.len()].clone();
            tip = Line { who: Who::Volontaria, mood: if worried { Mood::Preoccupato } else { Mood::Calmo }, text };
            &tip
        }
        None => return,
    };
    speaking(ctx, faces, line.who, line.mood, |ui| {
        characters::bubble(ui, line.who, |ui| characters::says(ui, line.who, &line.text, BUBBLE_W));
    });
}

/// Simulated seconds the volunteer explains how to play, after which she
/// says what is new instead (playtest 2: the tutorial repeated all game).
const TUTORIAL_S: i64 = 10 * 60;
/// Real seconds each piece of news stays before the next.
const NEWS_S: f64 = 8.0;

/// Where this fire starts and whom the wind drives it at: the case's `near`,
/// its wind, and the districts the fire is downwind of now.
fn the_fire(k: &Kiosk, sim: &Sim) -> String {
    let w = sim.fire.weather();
    let wind = format!("il vento da {} a {:.0} km/h", rocca::words::compass(w.wind_dir_deg), w.wind_speed_kmh);
    let mut toward: Vec<(f32, &str)> = k.risk.iter().enumerate()
        .filter_map(|(d, e)| e.filter(|e| e.downwind > 0.5).map(|e| (e.distance_m, sim.districts[d].name.as_str())))
        .collect();
    toward.sort_by(|a, b| a.0.total_cmp(&b.0));
    match toward.first() {
        Some((m, name)) if *name == sim.case.near => format!("Il fuoco è partito a {} da {name}, e {wind} lo spinge proprio lì.", rocca::words::km(*m)),
        Some((m, name)) => format!("Il fuoco è partito vicino a {}: {wind} lo spinge verso {name}, a {}.", sim.case.near, rocca::words::km(*m)),
        None => format!("Il fuoco è partito vicino a {}: {wind} per ora lo spinge lontano dai paesi.", sim.case.near),
    }
}

/// What the volunteer can tell that the player may not have seen: the next
/// unit to arrive, the nearest front, families still at home near the fire.
/// Facts of the game, each with whether it is worrying.
fn news(k: &Kiosk, sim: &Sim) -> Vec<(String, bool)> {
    let mut out = vec![];
    let next = (0..sim.districts.len())
        .flat_map(|d| sim.arrivals(d).into_iter().map(move |(u, a)| (d, u, a)))
        .filter_map(|(d, u, a)| if let rocca::Arrival::InMin(m) = a { Some((m, d, u)) } else { None })
        .min();
    if let Some((m, d, u)) = next {
        out.push((format!("{} arriva a {} tra circa {m} min.", sim.crews.units[u].callsign, sim.districts[d].name), false));
    }
    let nearest = k.risk.iter().enumerate().filter_map(|(d, e)| e.map(|e| (d, e))).min_by(|a, b| a.1.distance_m.total_cmp(&b.1.distance_m));
    if let Some((d, e)) = nearest {
        let push = if e.downwind > 0.5 { ", e il vento lo spinge proprio lì" } else { "" };
        if e.distance_m >= 60.0 {
            out.push((format!("Il fronte più vicino è a {} da {}{push}.", rocca::words::km(e.distance_m), sim.districts[d].name), e.distance_m < 1000.0));
        }
    }
    for (d, e) in k.risk.iter().enumerate() {
        let Some(e) = e.filter(|e| e.distance_m < 2500.0) else { continue };
        let home = sim.districts[d].households.iter()
            .filter(|&&i| matches!(sim.agents.households[i].status, Status::Normal | Status::Warned | Status::Defending | Status::Preparing))
            .count();
        if home == 0 {
            continue;
        }
        let who = if home == 1 { "1 famiglia è ancora in casa".to_string() } else { format!("{home} famiglie sono ancora in casa") };
        let order = if sim.active.civil[d] == Civil::Nessuno { " Non hanno ricevuto ordini." } else { "" };
        let fire = if e.distance_m < 60.0 { "il fuoco già tra le case".to_string() } else { format!("il fuoco a {}", rocca::words::km(e.distance_m)) };
        out.push((format!("A {}, con {fire}, {who}.{order}", sim.districts[d].name), true));
    }
    out
}

/// The crisis, told by the person it concerns: what is happening, the
/// actions that answer it, what the change would do to the units, the
/// countdown and the button.
fn crisis(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, faces: Option<&Portraits>) {
    let Some(c) = k.crisis.clone() else { return };
    let d = crisis_district(k);
    let (who, mood) = characters::for_crisis(c.kind);
    let mut close = false;
    speaking(ctx, faces, who, mood, |ui| {
        characters::bubble(ui, who, |ui| characters::says(ui, who, &c.text, BUBBLE_W));
        ui.add_space(8.0);
        egui::Frame::none().fill(Color32::from_rgb(64, 30, 10)).stroke(egui::Stroke::new(3.0, ORANGE)).rounding(10.0).inner_margin(12.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                // the countdown as a shrinking ring around the seconds
                let (r, _) = ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::hover());
                let left = (1.0 - k.phase_t / k.crisis_s).clamp(0.0, 1.0);
                let n = 48;
                let pts: Vec<egui::Pos2> = (0..=((n as f32 * left) as usize))
                    .map(|i| {
                        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
                        r.center() + egui::vec2(a.cos(), a.sin()) * 24.0
                    })
                    .collect();
                ui.painter().circle_stroke(r.center(), 24.0, egui::Stroke::new(5.0, Color32::from_gray(70)));
                if pts.len() > 1 {
                    ui.painter().add(egui::Shape::line(pts, egui::Stroke::new(5.0, ORANGE)));
                }
                ui.painter().text(r.center(), Align2::CENTER_CENTER, format!("{:.0}", (k.crisis_s - k.phase_t).max(0.0).ceil()), egui::FontId::proportional(22.0), Color32::WHITE);
                ui.add_space(6.0);
                if let Some(d) = d {
                    let name = sim.districts[d].name.clone();
                    let first = k.proposed.priorities.first() == Some(&d);
                    if icons::button(ui, Icon::Up, &format!("{name} per primo"), first, AMBER, !first).clicked() {
                        k.proposed.priorities.retain(|&x| x != d);
                        k.proposed.priorities.insert(0, d);
                        k.dirty = true;
                    }
                    let active = sim.active.civil[d];
                    for (o, icon, colour) in [(Civil::Preallerta, Icon::Bell, AMBER), (Civil::Evacua, Icon::Exit, BLUE)] {
                        let on = k.proposed.civil[d] == o || active >= o;
                        let label = if o == Civil::Preallerta { "Preallerta" } else { "Evacua" };
                        if icons::button(ui, icon, label, on, colour, active < o).clicked() {
                            k.proposed.civil[d] = if k.proposed.civil[d] == o { active } else { o };
                            k.dirty = true;
                        }
                    }
                }
                ui.add_space(12.0);
                let pending = k.proposed != sim.active;
                if ui.add(primary(if pending { "Conferma" } else { "Continua" })).clicked() {
                    close = true;
                }
            });
            // what the change does to the units, as engine icons
            if let Some(p) = &k.preview {
                for x in 0..sim.districts.len() {
                    let now = sim.posts.iter().flatten().filter(|q| q.district == x).count();
                    let next = p.units_on(x);
                    if now != next {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&sim.districts[x].name).size(17.0).color(Color32::WHITE));
                            trucks(ui, now, next);
                        });
                    }
                }
            }
        });
    });
    if close {
        close_crisis(k, sim);
    }
}

/// Back to the home view, zoom in and out.
fn view_controls(ctx: &egui::Context, k: &mut Kiosk) {
    egui::Area::new(egui::Id::new("vista")).anchor(Align2::RIGHT_TOP, [-12.0, 12.0]).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.horizontal(|ui| {
                if icons::button(ui, Icon::Plus, "", false, GREY, true).clicked() {
                    k.zoom *= 0.8;
                }
                if icons::button(ui, Icon::Minus, "", false, GREY, true).clicked() {
                    k.zoom *= 1.25;
                }
                if icons::button(ui, Icon::Home, "", false, GREY, true).clicked() {
                    k.reset_view = true;
                }
            });
        });
    });
}

/// What the marks on the map mean.
fn legend(ctx: &egui::Context) {
    // below the operator bar when that is open
    let below = ctx.memory(|m| m.area_rect(egui::Id::new("operatore"))).filter(|_| ctx.memory(|m| m.areas().visible_last_frame(&egui::LayerId::new(egui::Order::Foreground, egui::Id::new("operatore"))))).map_or(12.0, |r| r.bottom() + 8.0);
    // closed at first; `KIOSK_LEGEND=1` opens it, for screenshots
    let roomy = std::env::var("KIOSK_LEGEND").is_ok();
    egui::Area::new(egui::Id::new("legenda")).order(egui::Order::Foreground).anchor(Align2::LEFT_TOP, [12.0, below]).show(ctx, |ui| {
        panel().fill(Color32::from_rgb(18, 20, 24)).show(ui, |ui| {
            egui::CollapsingHeader::new(RichText::new("Legenda e tempi").size(15.0).strong().color(Color32::WHITE)).default_open(roomy).show(ui, |ui| {
                ui.set_width(340.0);
                ui.set_height(ctx.screen_rect().height() * 0.65);
                egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ctx.screen_rect().height() * 0.65).show(ui, |ui| {
                #[derive(Clone, Copy)]
                enum Mark {
                    Fill(Color32),
                    Ring(Color32),
                    Line(Color32, bool),
                    Arrow,
                    Diamond,
                }
                // Most asked first: what the plants on the map are and why they
                // matter, then the families bar; the rest folds away.
                section(ui, "Vegetazione: come brucia", true, |ui| {
                    for (kind, text) in [
                        (Plant::Grass, "Prato secco: il fuoco corre veloce, con fiamme basse."),
                        (Plant::Shrub, "Macchia: cespugli fitti, brucia forte e lancia faville."),
                        (Plant::Pine, "Pineta di pino marittimo: fiamme alte e faville lontane; può correre veloce."),
                        (Plant::Chestnut, "Castagneto (latifoglie): il fuoco avanza più lento."),
                    ] {
                        ui.horizontal(|ui| {
                            plant_sample(ui, kind);
                            ui.add(egui::Label::new(RichText::new(text).size(14.0).color(GREY)).wrap());
                        });
                    }
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(40.0, 26.0), egui::Sense::hover());
                        ui.painter().rect_filled(r.shrink(2.0), 4.0, Color32::from_rgb(214, 207, 189));
                        ui.label(RichText::new("Strade, case, orti: non bruciano.").size(14.0).color(GREY));
                    });
                    ui.label(RichText::new("Anche vento e pendenza spingono il fuoco.").size(14.0).color(GREY));
                });
                section(ui, "Barra delle famiglie", true, |ui| {
                    for (colour, text) in [(GREEN, "Evacuate: in sicurezza"), (BLUE, "In viaggio"), (AMBER, "In preparazione"), (Color32::from_gray(150), "A casa"), (RED, "Intrappolate / vittime")] {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 12.0), egui::Sense::hover());
                            ui.painter().rect_filled(r, 2.0, colour);
                            ui.label(RichText::new(text).size(14.0).color(GREY));
                        });
                    }
                });
                section(ui, "Segni sulla mappa", true, |ui| {
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
                section(ui, "Schede dei paesi", false, |ui| {
                    for text in [
                        "1, 2, 3: priorità di difesa. Cerchio vuoto: escluso dalla difesa.",
                        "Fiamma + km: distanza del fronte dalla casa più vicina, non tempo d’arrivo.",
                        "Famiglia + numero: famiglie del paese. Evacuate X/Y: già arrivate in sicurezza su totale.",
                        "Mezzo verde: assegnato, può essere in viaggio. Giallo: da assegnare alla conferma. Barrato: da trasferire.",
                        "Preallerta prepara; Evacua ordina di partire. Gli ordini confermati non si possono ritirare.",
                        "Tutti i tempi sono minuti nel mondo simulato. A ×40, 40 minuti passano in 1 minuto reale; quando non succede nulla il gioco accelera.",
                        "Evacuazione: avviso, preparazione, viaggio. La stima indica solo il viaggio dalla casa, senza code né rallentamenti; qualcuno può tardare o restare.",
                        "I nuclei con lo stesso nome condividono gli ordini della stessa scheda.",
                    ] { ui.label(RichText::new(text).size(14.0).color(GREY)); }
                });
                });
            });
        });
    });
}

/// «~3 min» or «~2–6 min».
fn minutes(m: &[u32]) -> String {
    let (a, b) = (m.iter().min().copied().unwrap_or(0), m.iter().max().copied().unwrap_or(0));
    if a == b { format!("~{a} min") } else { format!("~{a}–{b} min") }
}

/// The card's defence line. While the plan in force runs untouched, it is the
/// units' real state ([`rocca::Game::arrivals`]); while a plan is being
/// composed, the coordinator's preview of it. With no unit coming, it says
/// why, in the coordinator's words.
fn defence_line(k: &Kiosk, sim: &Sim, d: usize) -> String {
    let name = &sim.districts[d].name;
    let units = |n: usize| if n == 1 { "1 mezzo".to_string() } else { format!("{n} mezzi") };
    let composing = k.phase != Phase::Esegui || k.proposed != sim.active;
    // why nothing is coming: not chosen, no unit could be given, or no threat yet
    let reason = |plan: &rocca::Plan, uncovered: &[(usize, String)]| -> String {
        if plan.rank(d).is_none() {
            return "Difesa: non è tra le priorità".into();
        }
        match uncovered.iter().find(|(x, _)| *x == d) {
            Some((_, why)) => format!("Difesa: {}", why.strip_prefix(&format!("{name}: ")).unwrap_or(why)),
            None => "Difesa: nessun mezzo ora, il fuoco non minaccia ancora".into(),
        }
    };
    if composing {
        let Some(p) = k.preview.as_ref() else { return String::new() };
        let mins: Vec<u32> = p.posts.iter().flatten().filter(|x| x.district == d).map(|x| (x.eta_s / 60.0).ceil() as u32).collect();
        // units that hold a post here now and the new plan sends home
        let leaving: Vec<&str> = p.idle.iter().filter(|(u, _)| sim.posts.get(*u).and_then(|x| x.as_ref()).is_some_and(|x| x.district == d)).map(|(u, _)| sim.crews.units[*u].callsign.as_str()).collect();
        let mut line = if mins.is_empty() {
            reason(&k.proposed, &p.uncovered)
        } else if mins.iter().all(|&m| m == 0) {
            format!("Difesa: {} in postazione", units(mins.len()))
        } else {
            format!("Difesa: {} in arrivo {}", units(mins.len()), minutes(&mins))
        };
        if !leaving.is_empty() {
            line += &format!(" · {} tornerà alla base", leaving.join(", "));
        }
        return line;
    }
    let arrivals = sim.arrivals(d);
    if arrivals.is_empty() {
        return reason(&sim.active, &sim.uncovered);
    }
    let on_post = arrivals.iter().filter(|(_, a)| *a == rocca::Arrival::OnPost).count();
    let coming: Vec<u32> = arrivals.iter().filter_map(|(_, a)| if let rocca::Arrival::InMin(m) = a { Some(*m) } else { None }).collect();
    let blocked = arrivals.iter().filter(|(_, a)| *a == rocca::Arrival::Blocked).count();
    let mut parts = vec![];
    if on_post > 0 {
        parts.push(format!("{} in postazione", units(on_post)));
    }
    if !coming.is_empty() {
        parts.push(format!("{} in arrivo {}", units(coming.len()), minutes(&coming)));
    }
    if blocked > 0 {
        parts.push(format!("{} bloccato dal fuoco", units(blocked)));
    }
    for (u, a) in &arrivals {
        if let rocca::Arrival::Away(what) = a {
            parts.push(format!("{} {what}", sim.crews.units[*u].callsign));
        }
    }
    format!("Difesa: {}", parts.join(" · "))
}

/// A sub-section of the legend that can be folded.
fn section(ui: &mut egui::Ui, title: &str, open: bool, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).size(15.0).strong().color(Color32::WHITE)).default_open(open).show(ui, add);
}

#[derive(Clone, Copy)]
enum Plant {
    Grass,
    Shrub,
    Pine,
    Chestnut,
}

/// A small drawing of one vegetation type on its ground, in the colours the
/// map renders them with (sampled from kiosk screenshots, iteration 5b):
/// the same silhouettes as `vegetation.rs`.
fn plant_sample(ui: &mut egui::Ui, kind: Plant) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(40.0, 26.0), egui::Sense::hover());
    let p = ui.painter();
    let ground = match kind {
        Plant::Grass => Color32::from_rgb(213, 186, 104),
        Plant::Shrub => Color32::from_rgb(156, 157, 90),
        Plant::Pine => Color32::from_rgb(92, 112, 84),
        Plant::Chestnut => Color32::from_rgb(120, 150, 70),
    };
    p.rect_filled(r.shrink(2.0), 4.0, ground);
    let (base, c) = (r.bottom() - 4.0, r.center().x);
    match kind {
        Plant::Grass => {
            let tuft = Color32::from_rgb(150, 130, 70);
            for x in [-12.0, -3.0, 7.0, 14.0] {
                for (dx, h) in [(-2.0, 6.0), (0.0, 8.0), (2.0, 6.0)] {
                    p.line_segment([egui::pos2(c + x, base), egui::pos2(c + x + dx, base - h)], egui::Stroke::new(1.5, tuft));
                }
            }
        }
        Plant::Shrub => {
            let leaf = Color32::from_rgb(111, 122, 59);
            for (dx, rad) in [(-9.0, 6.0), (0.0, 8.0), (9.0, 6.5)] {
                p.circle_filled(egui::pos2(c + dx, base - rad * 0.6), rad, leaf);
            }
        }
        Plant::Pine => {
            let (bark, leaf) = (Color32::from_rgb(150, 70, 40), Color32::from_rgb(50, 75, 54));
            p.line_segment([egui::pos2(c, base), egui::pos2(c + 1.5, r.top() + 9.0)], egui::Stroke::new(2.5, bark));
            p.add(egui::Shape::ellipse_filled(egui::pos2(c + 1.0, r.top() + 8.0), egui::vec2(14.0, 4.5), leaf));
        }
        Plant::Chestnut => {
            let (bark, leaf) = (Color32::from_rgb(80, 60, 40), Color32::from_rgb(95, 119, 42));
            p.line_segment([egui::pos2(c, base), egui::pos2(c, base - 7.0)], egui::Stroke::new(3.5, bark));
            p.circle_filled(egui::pos2(c, base - 13.0), 9.0, leaf);
            p.circle_filled(egui::pos2(c - 7.0, base - 10.0), 5.5, leaf);
            p.circle_filled(egui::pos2(c + 7.0, base - 10.0), 5.5, leaf);
        }
    }
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
    // While the plan in force runs and nothing is being composed, chips are
    // compact, so three of them fit near each other; the one under the
    // pointer opens (downward, so what was under the pointer stays put).
    let compact = k.phase == Phase::Esegui && k.proposed == sim.active;
    let open_id = |d: usize| egui::Id::new(("quartiere aperto", d));
    // Where each chip stands (bottom centre) and how big it was last frame.
    let mut place: Vec<Option<(egui::Pos2, egui::Vec2)>> = (0..n)
        .map(|d| {
            let at = screen(cam, sim, chip_anchor(sim, d), 40.0)?;
            // A fixed size, not last frame's: a chip that gains a line of text
            // must not move its buttons from under the pointer.
            let size = egui::vec2(CHIP_W, if compact { CHIP_H_COMPACT } else { CHIP_H });
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
    let obstacles: Vec<egui::Rect> = ["lato", "personaggio", "azione", "vista"]
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
        let Some((at, _)) = place[d] else { continue };
        let dist = &sim.districts[d];
        if dist.nuclei.len() < 2 { continue; }
        for (i, &home) in dist.nuclei.iter().enumerate() {
            let Some(p) = screen(cam, sim, home, 0.0) else { continue };
            if !screen_rect.contains(p) { continue; }
            leaders.line_segment([at, p], egui::Stroke::new(1.0, AMBER.gamma_multiply(0.65)));
            leaders.circle_stroke(p, 7.0, egui::Stroke::new(2.0, AMBER));
            let text = format!("{} · nucleo {}", dist.name, i + 1);
            let galley = leaders.layout_no_wrap(text, egui::FontId::proportional(14.0), Color32::WHITE);
            let mut candidates = vec![
                p + egui::vec2(10.0, 8.0),
                p - egui::vec2(galley.size().x + 10.0, -8.0),
                p - egui::vec2(galley.size().x * 0.5, galley.size().y + 12.0),
                p + egui::vec2(-galley.size().x * 0.5, 18.0),
            ];
            for (at, size) in place.iter().flatten() {
                let chip = egui::Rect::from_min_size(*at - egui::vec2(size.x * 0.5, size.y), *size);
                candidates.extend([
                    egui::pos2(chip.left() - galley.size().x - 8.0, p.y),
                    egui::pos2(chip.right() + 8.0, p.y),
                    egui::pos2(p.x - galley.size().x * 0.5, chip.bottom() + 8.0),
                    egui::pos2(p.x - galley.size().x * 0.5, chip.top() - galley.size().y - 8.0),
                ]);
            }
            let score = |pos: egui::Pos2| {
                let r = egui::Rect::from_min_size(pos, galley.size()).expand(4.0);
                let chips = place.iter().flatten().filter(|(at, size)| {
                    r.intersects(egui::Rect::from_min_size(*at - egui::vec2(size.x * 0.5, size.y), *size))
                }).count();
                chips + obstacles.iter().filter(|o| r.intersects(**o)).count()
                    + usize::from(!screen_rect.contains_rect(r)) * 10
            };
            let pos = candidates.into_iter().min_by(|a, b| score(*a).cmp(&score(*b))
                .then_with(|| a.distance(p).total_cmp(&b.distance(p)))).unwrap();
            if pos.distance(p) > 35.0 {
                leaders.line_segment([p, pos + galley.size() * 0.5], egui::Stroke::new(1.0, AMBER));
            }
            leaders.rect_filled(egui::Rect::from_min_size(pos - egui::vec2(3.0, 2.0), galley.size() + egui::vec2(6.0, 4.0)), 4.0, PANEL);
            leaders.galley(pos, galley, Color32::WHITE);
        }
    }
    for d in 0..n {
        let dist = &sim.districts[d];
        let Some((at, _)) = place[d] else { continue };
        let id = egui::Id::new(("quartiere", d));
        let rank = order.iter().position(|&x| x == d);
        let open = compact && ctx.memory(|m| m.data.get_temp::<bool>(open_id(d))).unwrap_or(false);
        let small = compact && !open;
        // compact chips hang from their top edge, so opening grows downward
        let area = if compact {
            egui::Area::new(id).fixed_pos(at - egui::vec2(CHIP_W * 0.5, CHIP_H_COMPACT)).pivot(Align2::LEFT_TOP).order(if open { egui::Order::Foreground } else { egui::Order::Middle })
        } else {
            egui::Area::new(id).fixed_pos(at).pivot(Align2::CENTER_BOTTOM)
        };
        let shown = area.show(ctx, |ui| {
            let (border, width) = if alarm == Some(d) { (ORANGE, 4.0) } else if rank == Some(0) { (AMBER, 1.5) } else { (Color32::from_gray(90), 1.5) };
            egui::Frame::none().fill(PANEL).stroke(egui::Stroke::new(width, border)).rounding(10.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(CHIP_W - 20.0);
                if small {
                    ui.set_min_height(CHIP_H_COMPACT - 22.0);
                }
                // rank, name, fire
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
                    match rank {
                        Some(n) => {
                            ui.painter().circle_filled(r.center(), 17.0, AMBER);
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, (n + 1).to_string(), egui::FontId::proportional(22.0), Color32::BLACK);
                        }
                        None => {
                            ui.painter().circle_stroke(r.center(), 16.0, egui::Stroke::new(2.0, Color32::from_gray(110)));
                        }
                    }
                    ui.label(RichText::new(&dist.name).size(22.0).strong().color(Color32::WHITE));
                    if let Some(e) = k.risk.get(d).copied().flatten() {
                        let colour = if e.distance_m < 1000.0 { RED } else if e.distance_m < 2500.0 { ORANGE } else { GREY };
                        let text = if e.distance_m < 60.0 { "qui".to_string() } else { km(e.distance_m) };
                        egui::Frame::none().fill(colour).rounding(6.0).inner_margin(egui::Margin::symmetric(5.0, 2.0)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 3.0;
                                icons::show(ui, Icon::Fire, 18.0, Color32::from_rgb(120, 20, 0));
                                ui.label(RichText::new(text).size(17.0).strong().color(Color32::BLACK));
                            });
                        });
                    }
                });
                if !small {
                    ui.label(RichText::new(match rank {
                        Some(r) => format!("Priorità {}{}", r + 1, if dist.nuclei.len() > 1 { " · tutti i nuclei" } else { "" }),
                        None => "Nessuna priorità di difesa".into(),
                    }).size(14.0).color(GREY));
                }
                // families and engines
                let now = sim.posts.iter().flatten().filter(|p| p.district == d).count();
                let next = k.preview.as_ref().map_or(now, |p| p.units_on(d));
                ui.horizontal(|ui| {
                    icons::show(ui, Icon::Family, 24.0, GREY);
                    ui.label(RichText::new(format!("{} famiglie", dist.households.len())).size(16.0).color(Color32::WHITE));
                    ui.add_space(10.0);
                    trucks(ui, now, next);
                });
                // the families, as a bar: safe, on the road, getting ready, at home, reached
                {
                    let hh = &dist.households;
                    let count = |st: &[Status]| hh.iter().filter(|&&i| st.contains(&sim.agents.households[i].status)).count();
                    let safe = count(&[Status::Evacuated]);
                    ui.horizontal(|ui| {
                        icons::stack_bar(
                            ui,
                            CHIP_W - 160.0,
                            &[
                                (safe, GREEN),
                                (count(&[Status::Evacuating]), BLUE),
                                (count(&[Status::Preparing]), AMBER),
                                (count(&[Status::Warned, Status::Defending, Status::Normal]), Color32::from_gray(150)),
                                (count(&[Status::Trapped, Status::Casualty]), RED),
                            ],
                        );
                        ui.label(RichText::new(format!("{safe}/{} evacuate", hh.len())).size(16.0).color(GREEN));
                    });
                }
                if small {
                    ui.label(RichText::new(defence_line(k, sim, d)).size(14.0).color(GREEN));
                    return;
                }
                let hh = &dist.households;
                let count = |states: &[Status]| hh.iter().filter(|&&i| states.contains(&sim.agents.households[i].status)).count();
                ui.label(RichText::new(format!("{} a casa · {} si preparano · {} in viaggio",
                    count(&[Status::Normal, Status::Warned, Status::Defending]),
                    count(&[Status::Preparing]), count(&[Status::Evacuating]))).size(14.0).color(GREY));
                let trapped = count(&[Status::Trapped, Status::Casualty]);
                if trapped > 0 {
                    ui.label(RichText::new(format!("{trapped} intrappolate / vittime")).size(13.0).color(RED));
                } else {
                    ui.allocate_space(egui::vec2(1.0, 16.0));
                }
                // Reserve two lines so choosing an order cannot move controls.
                ui.allocate_ui(egui::vec2(CHIP_W - 20.0, 42.0), |ui| {
                    ui.label(RichText::new(defence_line(k, sim, d)).size(14.0).color(GREEN));
                    let journey = match k.evacuation.get(d) {
                        Some((Some((a, b)), 0)) => format!("Evacua: viaggio ~{a}–{b} min + attesa"),
                        Some((Some((a, b)), blocked)) => format!("Viaggio ~{a}–{b} min · {blocked} famiglie senza via"),
                        _ => "Evacua: viaggio non disponibile".into(),
                    };
                    ui.label(RichText::new(journey).size(14.0).color(BLUE));
                });
                // controls
                ui.horizontal(|ui| match rank {
                    Some(r) => {
                        if r == 0 { ui.allocate_space(egui::vec2(44.0, 44.0)); }
                        if r > 0 && icons::button(ui, Icon::Up, "", false, AMBER, true).clicked() {
                            order.retain(|&x| x != d);
                            order.insert(0, d);
                            changed = true;
                        }
                        if icons::button(ui, Icon::Cross, "", false, GREY, true).clicked() {
                            order.retain(|&x| x != d);
                            changed = true;
                        }
                    }
                    None => {
                        if icons::button(ui, Icon::Shield, "Difendi", false, AMBER, true).clicked() {
                            order.push(d);
                            changed = true;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    let active = sim.active.civil[d];
                    for (c, icon, label, colour) in [(Civil::Preallerta, Icon::Bell, "Preallerta", AMBER), (Civil::Evacua, Icon::Exit, "Evacua", BLUE)] {
                        let on = civil[d] == c || (active >= c && civil[d] <= c);
                        if icons::button(ui, icon, label, on, colour, active < c).clicked() {
                            civil[d] = if civil[d] == c { active } else { c.max(active) };
                            changed = true;
                        }
                    }
                });
                if (civil[d] != sim.active.civil[d] || next != now) && k.phase != Phase::Pianifica {
                    pill(ui, "da confermare", AMBER);
                }
            });
        });
        // open while the pointer is on it
        let hovered = ctx.pointer_hover_pos().is_some_and(|p| shown.response.rect.contains(p));
        ctx.memory_mut(|m| m.data.insert_temp(open_id(d), compact && hovered));
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
            .find(|r| r.expand(4.0).intersects(egui::Rect::from_center_size(ground - egui::vec2(0.0, 12.0), egui::vec2(110.0, 24.0))))
            .map_or(ground, |r| egui::pos2(ground.x, r.bottom() + 24.0));
        // a label is about 200 px wide and 18 tall
        let below = shown.iter().filter(|p| (p.x - ground.x).abs() < 110.0 && (p.y - ground.y).abs() < 26.0).count();
        shown.push(ground);
        let mut at = ground + egui::vec2(0.0, 28.0 * below as f32);
        // egui keeps an area on screen; do the same here so the panel test
        // below sees where the label will really be drawn
        let sr = ctx.screen_rect();
        at.x = at.x.clamp(sr.left() + 60.0, sr.right() - 60.0);
        at.y = at.y.clamp(sr.top() + 24.0, sr.bottom());
        // Behind a fixed panel (updates, speaker, button): just above it, with
        // a thin line down to where the unit is.
        for name in ["lato", "personaggio", "azione"] {
            let Some(panel) = ctx.memory(|m| m.area_rect(egui::Id::new(name))) else { continue };
            if panel.expand(4.0).intersects(egui::Rect::from_center_size(at - egui::vec2(0.0, 12.0), egui::vec2(110.0, 24.0))) {
                let lifted = egui::pos2(at.x, panel.top() - 6.0 - 28.0 * below as f32);
                ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("guide"))).line_segment([lifted, at], egui::Stroke::new(1.5, Color32::from_white_alpha(150)));
                at = lifted;
            }
        }
        let status = sim.unit_status(i);
        let trouble = if status.starts_with("bloccato") {
            Some((RED, "bloccata"))
        } else if status.starts_with("fuori") {
            Some((RED, "persa"))
        } else if status.starts_with("si ritira") {
            Some((ORANGE, "si ritira"))
        } else if status.starts_with("va a rifornirsi") {
            Some((BLUE, "acqua"))
        } else {
            None
        };
        let fire_near = status.contains(", fuoco a");
        let colour = trouble.map_or(if fire_near { ORANGE } else { GREEN }, |t| t.0);
        egui::Area::new(egui::Id::new(("mezzo", i))).fixed_pos(at).pivot(Align2::CENTER_BOTTOM).interactable(false).order(egui::Order::Background).show(ctx, |ui| {
            egui::Frame::none().fill(Color32::from_rgb(14, 16, 18)).stroke(egui::Stroke::new(2.0, colour)).rounding(6.0).inner_margin(egui::Margin::symmetric(5.0, 2.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    icons::show(ui, if u.kind == abm::suppression::UnitKind::Engine { Icon::Engine } else { Icon::Crew }, 20.0, colour);
                    ui.add(egui::Label::new(RichText::new(short(&u.callsign)).size(15.0).strong().color(Color32::WHITE)).extend());
                    if u.kind == abm::suppression::UnitKind::Engine {
                        // water left
                        let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 18.0), egui::Sense::hover());
                        ui.painter().rect_filled(r, 2.0, Color32::from_gray(60));
                        let h = r.height() * u.water_frac().clamp(0.0, 1.0);
                        ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(r.left(), r.bottom() - h), r.max), 2.0, BLUE);
                    }
                    if fire_near {
                        icons::show(ui, Icon::Fire, 16.0, ORANGE);
                    }
                    if let Some((c, word)) = trouble {
                        ui.add(egui::Label::new(RichText::new(word).size(14.0).strong().color(c)).extend());
                    }
                });
            });
        });
    }
}

/// What the coordinator would do with the plan being composed, and why.
fn proposal(ui: &mut egui::Ui, k: &Kiosk, sim: &Sim) {
    let Some(p) = &k.preview else { return };
    let pending = k.proposed != sim.active || k.phase == Phase::Pianifica;
        panel().show(ui, |ui| {
            ui.set_max_width(430.0);
            let title = if pending { "Il coordinatore propone" } else { "I mezzi" };
            egui::CollapsingHeader::new(RichText::new(title).size(15.0).strong().color(Color32::WHITE)).id_source("proposta_h").default_open(false).show(ui, |ui| {
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
}

/// The one button.
fn action(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim) {
    let pending = k.proposed != sim.active;
    let label = match k.phase {
        Phase::Pianifica if sim.time_s() > rocca::STEP_S => Some("Conferma e riprendi"),
        Phase::Pianifica => Some("Avvia"),
        Phase::Esegui if pending => Some("Conferma"),
        Phase::Crisi => return,
        _ => None,
    };
    egui::Area::new(egui::Id::new("azione")).anchor(Align2::CENTER_BOTTOM, [FACE, -18.0]).show(ctx, |ui| {
        ui.set_width(320.0);
        ui.vertical_centered(|ui| {
            if k.phase == Phase::Esegui {
                ui.label(RichText::new(if pending {
                    "Modifiche pronte: premi Conferma"
                } else {
                    "Passa sopra una scheda per gli ordini"
                }).size(16.0).color(Color32::WHITE));
                if !pending && ui.add(secondary("Pausa e modifica piano")).clicked() {
                    k.enter(Phase::Pianifica);
                }
            }
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
                None => {}
            }
        });
    });
}

/// Recent updates stay visible; the full history remains available.
fn events(ui: &mut egui::Ui, sim: &Sim) {
    panel().show(ui, |ui| {
        ui.set_width(300.0);
        ui.label(RichText::new("Ultimi aggiornamenti").size(16.0).strong().color(Color32::WHITE));
        if sim.log.is_empty() {
            ui.label(RichText::new("In attesa del primo piano").color(GREY));
        }
        let show = |ui: &mut egui::Ui, e: &rocca::game::LogEntry| {
            let (icon, colour) = if e.text.starts_with("CRISI") || e.text.contains("fuori servizio") || e.text.contains("si ritira") {
                (Icon::Alert, ORANGE)
            } else if e.text.starts_with("evacuazione") { (Icon::Exit, BLUE) }
            else if e.text.starts_with("preallerta") { (Icon::Bell, AMBER) }
            else { (Icon::Engine, GREEN) };
            ui.horizontal_top(|ui| {
                icons::show(ui, icon, 20.0, colour);
                ui.vertical(|ui| {
                    ui.label(RichText::new(clock(e.at_s)).size(12.0).color(colour));
                    ui.label(RichText::new(&e.text).size(14.0).color(Color32::WHITE));
                });
            });
        };
        for e in sim.log.iter().rev().take(2) {
            show(ui, e);
            ui.add_space(4.0);
        }
        egui::CollapsingHeader::new(format!("Cronologia completa ({})", sim.log.len())).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for e in sim.log.iter().rev() { show(ui, e); ui.separator(); }
            });
        });
    });
}

/// Bottom right, one above the other: what the coordinator proposes and the
/// latest events, both closed until asked for.
fn side(ctx: &egui::Context, k: &Kiosk, sim: &Sim) {
    egui::Area::new(egui::Id::new("lato")).anchor(Align2::RIGHT_BOTTOM, [-12.0, -12.0]).show(ctx, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
            proposal(ui, k, sim);
            ui.add_space(6.0);
            events(ui, sim);
        });
    });
}

/// Two bars to compare: yours (solid) and the same fire with no orders
/// (outlined), on the same scale.
fn compare_bars(ui: &mut egui::Ui, mine: usize, theirs: Option<usize>, scale: usize, colour: Color32) {
    let w = 150.0;
    let (r, _) = ui.allocate_exact_size(egui::vec2(w + 40.0, 30.0), egui::Sense::hover());
    let p = ui.painter();
    let len = |n: usize| w * n as f32 / scale.max(1) as f32;
    let top = egui::Rect::from_min_size(r.min, egui::vec2(len(mine).max(2.0), 13.0));
    p.rect_filled(top, 3.0, colour);
    p.text(egui::pos2(top.right() + 4.0, top.center().y), Align2::LEFT_CENTER, mine.to_string(), egui::FontId::proportional(15.0), Color32::WHITE);
    if let Some(t) = theirs {
        let bot = egui::Rect::from_min_size(r.min + egui::vec2(0.0, 16.0), egui::vec2(len(t).max(2.0), 12.0));
        p.rect_stroke(bot, 3.0, egui::Stroke::new(1.5, Color32::from_gray(150)));
        p.text(egui::pos2(bot.right() + 4.0, bot.center().y), Align2::LEFT_CENTER, t.to_string(), egui::FontId::proportional(13.0), GREY);
    }
}

/// The game on a line: the three hours, with what the player decided and
/// what was lost, as marks.
fn timeline(ui: &mut egui::Ui, sim: &Sim, width: f32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(width, 52.0), egui::Sense::hover());
    let p = ui.painter();
    let y = r.top() + 20.0;
    p.line_segment([egui::pos2(r.left(), y), egui::pos2(r.right(), y)], egui::Stroke::new(3.0, Color32::from_gray(90)));
    let dur = sim.case.duration_s().max(1) as f32;
    let x = |t: i64| r.left() + r.width() * (t as f32 / dur).clamp(0.0, 1.0);
    for h in 0..=(sim.case.duration_s() / 3600) {
        let px = x(h * 3600);
        p.line_segment([egui::pos2(px, y - 5.0), egui::pos2(px, y + 5.0)], egui::Stroke::new(2.0, Color32::from_gray(120)));
        p.text(egui::pos2(px, y + 18.0), Align2::CENTER_CENTER, format!("{h} h"), egui::FontId::proportional(13.0), GREY);
    }
    for e in &sim.log {
        let mark = if e.text.starts_with("priorità:") {
            Some((Icon::Shield, AMBER))
        } else if e.text.starts_with("preallerta:") {
            Some((Icon::Bell, AMBER))
        } else if e.text.starts_with("evacuazione:") {
            Some((Icon::Exit, BLUE))
        } else if e.text.contains("fuori servizio") {
            Some((Icon::Engine, RED))
        } else if e.text.starts_with("il vento gira") {
            Some((Icon::Fast, Color32::WHITE))
        } else {
            None
        };
        if let Some((icon, c)) = mark {
            let at = egui::Rect::from_center_size(egui::pos2(x(e.at_s), y), egui::vec2(22.0, 22.0));
            p.circle_filled(at.center(), 13.0, Color32::from_gray(25));
            icons::draw(p, at.shrink(2.0), icon, c);
        }
    }
}

/// The end: what your plan changed, against the same fire with no orders.
fn debrief(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    let o = sim.outcome();
    let base = k.baseline();
    let mut again = None;
    egui::Area::new(egui::Id::new("fine")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        egui::Frame::none().fill(Color32::from_rgb(18, 20, 24)).stroke(egui::Stroke::new(1.0, Color32::from_gray(80))).rounding(14.0).inner_margin(24.0).show(ui, |ui| {
            ui.set_width(760.0);
            let b = base.as_ref().and_then(|r| r.as_ref().ok());
            // the two numbers that matter, big
            ui.horizontal(|ui| {
                for (icon, what, mine, theirs, colour) in [
                    (Icon::Family, "famiglie colte in casa", o.caught(), b.map(|b| b.caught()), BLUE),
                    (Icon::House, "case colpite", o.homes_hit(), b.map(|b| b.homes_hit()), ORANGE),
                ] {
                    egui::Frame::none().fill(Color32::from_gray(32)).rounding(10.0).inner_margin(14.0).show(ui, |ui| {
                        ui.set_width(340.0);
                        ui.horizontal(|ui| {
                            icons::show(ui, icon, 56.0, colour);
                            ui.vertical(|ui| {
                                ui.label(RichText::new(mine.to_string()).size(46.0).strong().color(Color32::WHITE));
                                ui.label(RichText::new(what).size(16.0).color(GREY));
                            });
                            if let Some(t) = theirs {
                                let saved = t as i64 - mine as i64;
                                if saved > 0 {
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(format!("−{saved}")).size(30.0).strong().color(GREEN));
                                        ui.label(RichText::new("grazie a te").size(15.0).color(GREEN));
                                    });
                                }
                            }
                        });
                    });
                }
            });
            ui.add_space(10.0);
            // per place: two bars each, yours against no orders
            let scale = sim.districts.iter().map(|d| d.households.len()).max().unwrap_or(1).min(60);
            egui::Grid::new("esito").spacing([18.0, 6.0]).show(ui, |ui| {
                ui.label("");
                ui.horizontal(|ui| {
                    icons::show(ui, Icon::Family, 20.0, BLUE);
                    ui.label(RichText::new("in casa").size(15.0).color(GREY));
                });
                ui.horizontal(|ui| {
                    icons::show(ui, Icon::House, 20.0, ORANGE);
                    ui.label(RichText::new("colpite").size(15.0).color(GREY));
                });
                ui.horizontal(|ui| {
                    icons::show(ui, Icon::Exit, 20.0, GREEN);
                    ui.label(RichText::new("evacuate").size(15.0).color(GREY));
                });
                ui.end_row();
                for (i, (d, x)) in sim.districts.iter().zip(&o.districts).enumerate() {
                    let bx = b.map(|b| &b.districts[i]);
                    ui.label(RichText::new(&d.name).size(19.0).strong().color(Color32::WHITE));
                    compare_bars(ui, x.caught, bx.map(|b| b.caught), scale, BLUE);
                    compare_bars(ui, x.homes_hit, bx.map(|b| b.homes_hit), scale, ORANGE);
                    ui.horizontal(|ui| {
                        icons::stack_bar(ui, 120.0, &[(x.evacuated, GREEN), (x.households - x.evacuated, Color32::from_gray(80))]);
                        ui.label(RichText::new(format!("{}/{}", x.evacuated, x.households)).size(15.0).color(GREEN));
                    });
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 3.0, Color32::WHITE);
                ui.label(RichText::new("tu").size(14.0).color(GREY));
                let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, 12.0), egui::Sense::hover());
                ui.painter().rect_stroke(r, 3.0, egui::Stroke::new(1.5, Color32::from_gray(150)));
                ui.label(RichText::new("lo stesso incendio senza ordini").size(14.0).color(GREY));
                if base.is_none() {
                    ui.label(RichText::new("(calcolo…)").size(14.0).color(GREY));
                }
            });
            ui.add_space(10.0);
            timeline(ui, sim, 740.0);
            // why each place ended as it did: what the game recorded
            ui.label(RichText::new("Perché").size(17.0).strong().color(Color32::WHITE));
            for d in 0..sim.districts.len() {
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(egui::vec2(130.0, 18.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.set_min_width(130.0);
                        ui.label(RichText::new(&sim.districts[d].name).size(15.0).strong().color(Color32::WHITE));
                    });
                    ui.vertical(|ui| {
                        for line in sim.story(d) {
                            ui.add(egui::Label::new(RichText::new(line).size(14.0).color(GREY)).wrap());
                        }
                    });
                });
                ui.add_space(4.0);
            }
            // a real-world note, not advice on the game
            ui.label(RichText::new(REAL_LIFE).size(13.0).italics().color(AMBER));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let big = |t: &str| egui::Button::new(RichText::new(t).size(24.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(12.0).min_size(egui::vec2(240.0, 60.0));
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
                        k.intro = true;
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
