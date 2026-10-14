//! The screens around the game, in the style of the first kiosk: the opening
//! (the volunteer, the people you work with, the name, today's board), the
//! tutorial's bubble, the day-2 card and the end (big stars, a verdict, your
//! place on the board). Faces speak, words are few, buttons big.
//!
//! What they say comes from the game (the tutorial script, the outcome, the
//! stars); no rule lives here.

use bevy::prelude::*;
use bevy_egui::egui::{self, Align2, Color32, RichText};
use rocca::tutorial::Expect;

use super::characters::{self, Logo, Mood, Portraits, Who};
use super::ui::{card, primary, secondary, AMBER, GREY, RED};
use super::{board, new_visit, start_game, start_tutorial, Kiosk, Shot};
use scenario::Pos;
use crate::sim::{Sim, SimRestarted};

const INK: Color32 = Color32::from_rgb(18, 20, 24);
/// Widest the volunteer's text gets.
const TALK_W: f32 = 560.0;

/// The dim behind a full-screen card.
fn backdrop(ctx: &egui::Context, id: &str, alpha: u8) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new(id)).fixed_pos(screen.min).order(egui::Order::Middle).interactable(false).show(ctx, |ui| {
        ui.painter().rect_filled(screen, 0.0, Color32::from_rgba_premultiplied(8, 10, 14, alpha));
    });
}

/// The big card of the opening and the end.
fn big_card() -> egui::Frame {
    egui::Frame::none()
        .fill(INK)
        .stroke(egui::Stroke::new(1.5, AMBER.gamma_multiply(0.55)))
        .shadow(egui::epaint::Shadow { offset: egui::vec2(0.0, 10.0), blur: 40.0, spread: 4.0, color: Color32::from_black_alpha(160) })
        .rounding(18.0)
        .inner_margin(egui::Margin { left: 36.0, right: 36.0, top: 28.0, bottom: 24.0 })
}

/// The volunteer speaking, with her portrait.
fn volunteer(ui: &mut egui::Ui, faces: Option<&Portraits>, mood: Mood, size: f32, text: &str, width: f32) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.add_space(6.0);
            characters::portrait(ui, faces, Who::Volontaria, mood, size);
        });
        ui.add_space(18.0);
        characters::bubble(ui, Who::Volontaria, |ui| characters::says(ui, Who::Volontaria, text, width));
    });
}

// --- the opening --------------------------------------------------------------

pub fn intro(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>, logo: Option<&Logo>, faces: Option<&Portraits>) {
    let screen = ctx.screen_rect();
    backdrop(ctx, "intro_fondo", 190);
    // today's board stands on the right of the card, the two centred as a pair
    let (panel_w, gap) = (300.0, 16.0);
    let card_w = (screen.width() - 72.0 - panel_w - gap - 24.0).min(820.0);
    let pair = card_w + 72.0 + gap + panel_w;
    egui::Area::new(egui::Id::new("intro_classifica")).anchor(Align2::CENTER_CENTER, [(pair - panel_w) / 2.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::none()
            .fill(INK)
            .stroke(egui::Stroke::new(1.5, AMBER.gamma_multiply(0.55)))
            .rounding(18.0)
            .inner_margin(egui::Margin { left: 16.0, right: 16.0, top: 16.0, bottom: 12.0 })
            .show(ui, |ui| {
                ui.set_width(panel_w - 32.0);
                ui.label(RichText::new("Classifica di oggi").size(22.0).strong().color(Color32::WHITE));
                ui.label(RichText::new("le stelle di Persone, Case, Ettari").size(13.0).color(GREY));
                ui.add_space(8.0);
                let rows = if screen.height() >= 900.0 { rocca::score::BOARD_SHOWN } else { 4 };
                board_rows(ui, k, None, rows);
            });
    });
    let mut go = None;
    egui::Area::new(egui::Id::new("intro")).anchor(Align2::CENTER_CENTER, [-(pair - card_w - 72.0) / 2.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        big_card().show(ui, |ui| {
            ui.set_width(card_w);
            ui.horizontal(|ui| {
                if let Some(l) = logo {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(l.0, egui::vec2(77.0, 96.0))));
                    ui.add_space(14.0);
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new(super::TITLE).size(44.0).strong().color(Color32::WHITE));
                    ui.label(RichText::new("Aiutaci a gestire questa emergenza.").size(22.0).color(AMBER));
                });
            });
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(10.0);
            let volunteers = |v: bool| k.territory.roster.iter().filter(|s| k.territory.stations[s.station].volunteers == v).count();
            let (vvf, aib) = (volunteers(false), volunteers(true));
            let text = format!(
                "Un incendio minaccia i borghi dell'isola e ci serve il tuo aiuto! Abbiamo {vvf} squadre dei vigili del fuoco e {aib} di volontari, per tanti borghi: non bastano per tutti. Il tempo scorre veloce; per decidere chiedi una pausa all'esperto. Prima proviamo insieme con un piccolo incendio, poi tocca a te.",
            );
            volunteer(ui, faces, Mood::Preoccupato, 110.0, &text, TALK_W.min(card_w - 200.0));
            ui.add_space(12.0);
            ui.label(RichText::new("LE PERSONE CON CUI LAVORI").size(13.0).strong().color(GREY.gamma_multiply(0.7)));
            ui.add_space(6.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                for who in [Who::Volontaria, Who::Esperto, Who::Vigili] {
                    egui::Frame::none()
                        .fill(who.colour().gamma_multiply(0.10))
                        .stroke(egui::Stroke::new(1.0, who.colour().gamma_multiply(0.35)))
                        .rounding(40.0)
                        .inner_margin(egui::Margin { left: 6.0, right: 14.0, top: 6.0, bottom: 6.0 })
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                characters::portrait(ui, faces, who, Mood::Calmo, 48.0);
                                ui.label(RichText::new(who.short()).size(16.0).strong().color(who.colour()));
                            });
                        });
                }
            });
            ui.add_space(12.0);
            ui.label(RichText::new("Alla fine vedrai com'è andata, confrontata con lo stesso incendio senza ordini.").size(15.0).italics().color(GREY));
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Come ti chiami?").size(20.0).strong().color(Color32::WHITE));
                ui.add_space(8.0);
                let edit = egui::TextEdit::singleline(&mut k.player)
                    .hint_text("Il tuo nome")
                    .font(egui::FontId::proportional(24.0))
                    .char_limit(rocca::score::NAME_MAX)
                    .desired_width(320.0)
                    .margin(egui::vec2(12.0, 8.0));
                let r = ui.add(edit);
                crate::playtest::control("Nome", &r, true);
                // nothing else on this screen takes typing
                if !r.has_focus() {
                    r.request_focus();
                }
            });
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let b = egui::Button::new(RichText::new("Inizia").size(30.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(12.0).min_size(egui::vec2(280.0, 64.0));
                let r = ui.add(b);
                crate::playtest::control("Inizia", &r, true);
                if super::icons::pointing(r).clicked() {
                    go = Some(true);
                }
                ui.add_space(8.0);
                let r = ui.add(secondary("Salta la prova guidata"));
                crate::playtest::control("Salta la prova guidata", &r, true);
                if super::icons::pointing(r).clicked() {
                    go = Some(false);
                }
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new("Territorio immaginario, fuoco simulato con PROPAGATOR di Fondazione CIMA. Nella realtà segui sempre le indicazioni delle autorità.")
                    .size(13.0)
                    .color(GREY.gamma_multiply(0.75)),
            );
        });
    });
    if go.is_some() {
        k.player = rocca::score::name(&k.player);
    }
    match go {
        Some(true) => start_tutorial(k, sim, restarted),
        Some(false) => start_game(k, sim, restarted),
        None => {}
    }
}

// --- the tutorial -------------------------------------------------------------

/// The volunteer's bubble at the foot of the map: the step's text, «Avanti»
/// when the step only asks to read, «Salta» always. The map stays usable:
/// the step asks the player to act on it.
pub fn tutorial(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>, faces: Option<&Portraits>, cam: (&Camera, &GlobalTransform)) {
    let Some(step) = k.script.as_ref().and_then(|s| s.steps.get(k.step)).cloned() else { return };
    if k.shot_for != Some(k.step) {
        fly_to(k, sim, step.expect);
    }
    highlight(ctx, k, step.expect);
    let total = k.script.as_ref().map_or(0, |s| s.steps.len());
    let mut skip = false;
    let t = ctx.input(|i| i.time) as f32;
    egui::Area::new(egui::Id::new("tutorial")).anchor(Align2::CENTER_BOTTOM, [0.0, -64.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.add_space(8.0);
                characters::portrait(ui, faces, Who::Volontaria, Mood::Calmo, 100.0);
            });
            ui.add_space(18.0);
            characters::bubble(ui, Who::Volontaria, |ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(560.0);
                    // «benvenuto/a» would guess a gender: greet by the name
                    let hello = if k.player.is_empty() { "Ciao, grazie dell'aiuto!".to_string() } else { format!("Ciao {}, grazie dell'aiuto!", k.player) };
                    let text = if k.step == 0 { format!("{hello} {}", step.text) } else { step.text.clone() };
                    characters::says(ui, Who::Volontaria, &text, 560.0);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let r = ui.add(secondary("Salta il tutorial"));
                        crate::playtest::control("Salta il tutorial", &r, true);
                        skip = r.clicked();
                        ui.label(RichText::new(format!("{}/{total}", k.step + 1)).size(15.0).color(Color32::from_gray(110)));
                        if step.expect == Expect::Next {
                            let label = "Avanti";
                            let w = 180.0;
                            ui.add_space((ui.available_width() - w).max(0.0));
                            let r = ui.add(primary(label).min_size(egui::vec2(w, 48.0)));
                            crate::playtest::control(label, &r, true);
                            if r.clicked() {
                                k.done(Expect::Next);
                            }
                        } else {
                            // the step waits for the player's own action
                            let glow = 0.55 + 0.45 * (t * 3.0).sin();
                            let hint = if matches!(step.expect, Expect::Run(_)) { "guarda" } else { "tocca a te" };
                            ui.label(RichText::new(hint).size(16.0).strong().color(AMBER.gamma_multiply(glow)));
                        }
                    });
                });
            });
        });
    });
    ctx.request_repaint();
    if skip || (!ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Escape))) {
        start_game(k, sim, restarted);
    }
}

/// The camera flies to what the step is about; when the fire runs by itself
/// it follows it again.
fn fly_to(k: &mut Kiosk, sim: &Sim, expect: Expect) {
    k.shot_for = Some(k.step);
    k.step_t = 0.0;
    let ignition = Some(sim.case.ignition());
    let mid = |a: Pos, b: Pos| Some(Pos { x: (a.x + b.x) * 0.5, y: (a.y + b.y) * 0.5 });
    k.follow = false;
    k.shot = Some(match expect {
        // the first step: where the fire started
        _ if k.step == 0 => Shot { at: ignition, zoom: 0.5 },
        Expect::Move(t, hex) => {
            let team = &sim.teams[t];
            let home = Pos::from(sim.territory.stations[team.station].pos);
            let from = team.hex.map_or(home, |h| h.centre());
            Shot { at: mid(from, hex.centre()), zoom: 0.7 }
        }
        Expect::Evacuate(d) => Shot { at: Some(sim.districts[d].centre), zoom: 0.55 },
        Expect::Over | Expect::Run(_) => {
            k.follow = true;
            return k.shot = None;
        }
        _ => Shot { at: None, zoom: 1.0 },
    });
}

/// Real seconds a highlight glows after its step begins.
const GLOW_S: f32 = 7.0;

/// A glowing frame on what the step asks the player to touch, for a few
/// seconds: the pause button, the team's flag and the hex, the district.
fn highlight(ctx: &egui::Context, k: &Kiosk, expect: Expect) {
    if k.step_t > GLOW_S || k.step == 0 {
        return;
    }
    let area = |id: egui::Id| ctx.memory(|m| m.area_rect(id));
    let targets: Vec<Option<egui::Rect>> = match expect {
        Expect::Pause | Expect::Resume => vec![area(egui::Id::new("pausa"))],
        // no box on a drag: the cone and the hex on the map are signs enough
        Expect::Move(..) => vec![],
        Expect::Evacuate(d) => vec![area(egui::Id::new(("borgo", d)))],
        _ => vec![],
    };
    let fade = ((GLOW_S - k.step_t) / 1.5).clamp(0.0, 1.0);
    let t = ctx.input(|i| i.time) as f32;
    let glow = (0.85 + 0.15 * (t * 4.0).sin()) * fade;
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("tutorial_evidenza")));
    for r in targets.into_iter().flatten() {
        let r = r.expand(8.0);
        // a dark edge under the amber one, so it reads on any ground
        p.rect_stroke(r, 12.0, egui::Stroke::new(8.0, Color32::from_black_alpha((160.0 * fade) as u8)));
        p.rect_stroke(r, 12.0, egui::Stroke::new(5.0, AMBER.gamma_multiply(glow)));
        p.rect_stroke(r.expand(8.0), 18.0, egui::Stroke::new(2.0, AMBER.gamma_multiply(glow * 0.6)));
    }
}

// --- day 2 --------------------------------------------------------------------

pub fn day_two(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>, faces: Option<&Portraits>) {
    backdrop(ctx, "giorno2_fondo", 150);
    let mut go = false;
    egui::Area::new(egui::Id::new("giorno2")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        big_card().show(ui, |ui| {
            ui.set_width(640.0);
            ui.label(RichText::new("Giorno 2").size(40.0).strong().color(Color32::WHITE));
            ui.add_space(8.0);
            let text = characters::to(&k.player, "Ben fatto, il fuoco è sotto controllo! Ma domani c'è un nuovo incendio, più grande, e il vento può cambiare. Ora decidi tu: ti aiuto solo se me lo chiedi.");
            volunteer(ui, faces, Mood::Calmo, 110.0, &text, 420.0);
            ui.add_space(14.0);
            ui.label(RichText::new("La partita parte in pausa: guarda la previsione e dai i primi ordini.").size(16.0).color(GREY));
            ui.add_space(12.0);
            let b = egui::Button::new(RichText::new("Inizia").size(30.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(12.0).min_size(egui::vec2(280.0, 64.0));
            let r = ui.add_sized([280.0, 64.0], b);
            crate::playtest::control("Inizia giorno 2", &r, true);
            go = super::icons::pointing(r).clicked();
        });
    });
    if go {
        start_game(k, sim, restarted);
    }
}

// --- the end ------------------------------------------------------------------

/// One star, filled or empty, `size` across.
fn star(ui: &mut egui::Ui, fill: Color32, size: f32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let c = r.center() + egui::vec2(0.0, size * 0.05);
    let pts: Vec<egui::Pos2> = (0..10)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let rad = if i % 2 == 0 { size * 0.5 } else { size * 0.2 };
            c + egui::vec2(a.cos(), a.sin()) * rad
        })
        .collect();
    let p = ui.painter();
    // a star is not convex: the inner pentagon, then a triangle per point
    p.add(egui::Shape::convex_polygon((0..5).map(|i| pts[2 * i + 1]).collect(), fill, egui::Stroke::NONE));
    for i in 0..5 {
        p.add(egui::Shape::convex_polygon(vec![pts[2 * i], pts[(2 * i + 1) % 10], pts[(2 * i + 9) % 10]], fill, egui::Stroke::NONE));
    }
}

/// The three star rows, big: the category, its stars, what it counts.
fn big_stars(ui: &mut egui::Ui, st: &rocca::score::Stars, size: f32) {
    for (name, n, what) in [
        ("Persone", st.persone, "persone che il fuoco non ha raggiunto"),
        ("Case", st.case, "case che il fuoco non ha colpito"),
        ("Ettari", st.ettari, "bosco che non è bruciato"),
    ] {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(120.0, size), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(120.0);
                ui.label(RichText::new(name).size(24.0).strong().color(Color32::WHITE));
            });
            match n {
                Some(n) => {
                    for i in 0..5 {
                        star(ui, if i < n as usize { AMBER } else { Color32::from_gray(70) }, size);
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new(what).size(14.0).color(GREY));
                }
                None => {
                    ui.label(RichText::new("non conta: senza ordini non sarebbe successo").size(15.0).color(GREY));
                }
            }
        });
        ui.add_space(4.0);
    }
}

/// The volunteer's verdict on the people, from the two outcomes.
fn verdict(name: &str, mine: &rocca::Outcome, none: &rocca::Outcome) -> (Mood, String) {
    let (caught, saved) = (mine.caught(), none.caught().saturating_sub(mine.caught()));
    let (mood, text) = if caught == 0 && saved > 0 {
        (Mood::Calmo, format!("Il fuoco non ha raggiunto nessuno: senza i tuoi ordini avrebbe colpito {} persone. Ottimo lavoro!", none.caught()))
    } else if caught == 0 {
        (Mood::Calmo, "Nessuno è rimasto in pericolo. Ben fatto!".to_string())
    } else if saved > 0 {
        (Mood::Preoccupato, format!("Hai messo in salvo {saved} persone in più rispetto a nessun ordine, ma il fuoco ne ha raggiunte {caught}. Si poteva fare meglio?"))
    } else {
        (Mood::Preoccupato, format!("Il fuoco ha raggiunto {caught} persone. Quando conviene dare l'ordine di evacuare?"))
    };
    (mood, characters::to(name, &text))
}

/// A level's board, the best `rows`; the player's own row (`mine`: name and
/// time stamp) lit, and appended with its place when it ranks lower.
fn board_rows(ui: &mut egui::Ui, k: &Kiosk, mine: Option<(&str, u64)>, rows: usize) {
    let (day, _) = board::now();
    let ranking = k.board.ranking(&k.territory.game, &day);
    if ranking.is_empty() {
        ui.label(RichText::new("Nessuna partita oggi").size(14.0).color(GREY.gamma_multiply(0.8)));
        return;
    }
    let is_mine = |e: &rocca::score::Played| mine.is_some_and(|(n, at)| e.at == at && e.name == n);
    let mut shown: Vec<(usize, &rocca::score::Played)> = ranking.iter().copied().enumerate().take(rows).collect();
    if let Some(p) = ranking.iter().position(|e| is_mine(e)).filter(|&p| p >= rows) {
        shown.push((p, ranking[p]));
    }
    for (i, e) in shown {
        let me = is_mine(e);
        let ink = if me { Color32::BLACK } else { Color32::WHITE };
        let s: Vec<String> = e.stars.iter().map(|s| s.map_or("–".into(), |n| n.to_string())).collect();
        egui::Frame::none().fill(if me { AMBER } else { Color32::TRANSPARENT }).rounding(6.0).inner_margin(egui::Margin::symmetric(6.0, 1.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add_sized([22.0, 20.0], egui::Label::new(RichText::new(format!("{}", i + 1)).size(15.0).strong().color(if me { ink } else { GREY })));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(s.join(" ")).size(16.0).strong().monospace().color(ink));
                    star(ui, if me { Color32::BLACK } else { AMBER }, 15.0);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(RichText::new(&e.name).size(16.0).strong().color(ink)).truncate());
                    });
                });
            });
        });
    }
}

/// «Sei al 3º posto su 12 oggi».
fn place_words(place: usize, of: usize) -> String {
    match place {
        0 if of == 1 => "Sei il primo di oggi".to_string(),
        0 => format!("Sei al 1º posto su {of} oggi"),
        p => format!("Sei al {}º posto su {of} oggi", p + 1),
    }
}

pub fn end(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>, faces: Option<&Portraits>) {
    let mine = sim.outcome();
    let base = k.baseline();
    // on the board once, when the yardstick is ready
    if let (Some(Ok(none)), None) = (&base, &k.saved) {
        let st = rocca::score::stars(&mine, none);
        let (day, at) = board::now();
        let name = if k.player.trim().is_empty() { "Anonimo".to_string() } else { rocca::score::name(&k.player) };
        k.board.push(rocca::score::Played { name, level: sim.case.name.clone(), stars: st.all(), day: day.clone(), at });
        if let Err(e) = board::save(&k.board) {
            k.error = Some(e);
        }
        k.saved = Some((day, at));
    }
    backdrop(ctx, "fine_fondo", 200);
    let screen = ctx.screen_rect();
    let side = 340.0;
    let inner = (screen.width() - 48.0 - 72.0).min(1060.0);
    let left = (inner - side - 24.0).max(420.0);
    let name = if k.player.trim().is_empty() { "Anonimo".to_string() } else { rocca::score::name(&k.player) };
    let mut again = false;
    egui::Area::new(egui::Id::new("fine")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        big_card().show(ui, |ui| {
            ui.set_width(inner);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(egui::vec2(left, 10.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(left);
                    let title = if k.player.trim().is_empty() { "Com'è andata".to_string() } else { format!("Com'è andata, {}", k.player.trim()) };
                    ui.label(RichText::new(title).size(34.0).strong().color(Color32::WHITE));
                    ui.add_space(12.0);
                    match &base {
                        None => {
                            ui.label(RichText::new("Confronto con lo stesso incendio senza ordini…").size(20.0).color(GREY));
                        }
                        Some(Err(e)) => {
                            ui.colored_label(RED, e);
                        }
                        Some(Ok(none)) => {
                            big_stars(ui, &rocca::score::stars(&mine, none), 36.0);
                            ui.add_space(10.0);
                            let (mood, text) = verdict(&k.player, &mine, none);
                            volunteer(ui, faces, mood, 90.0, &text, left - 190.0);
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(format!(
                                    "Tu: {} persone e {} case colpite, {:.0} ettari bruciati. Senza ordini: {} persone, {} case, {:.0} ettari.",
                                    mine.caught(),
                                    mine.homes_hit(),
                                    mine.hectares,
                                    none.caught(),
                                    none.homes_hit(),
                                    none.hectares
                                ))
                                .size(15.0)
                                .color(GREY),
                            );
                        }
                    }
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.set_width(side);
                    card(AMBER).show(ui, |ui| {
                        ui.set_width(side - 36.0);
                        let stamp = k.saved.as_ref().map(|(_, at)| (name.as_str(), *at));
                        if let Some((day, at)) = &k.saved {
                            let (place, of) = (k.board.place(&k.territory.game, day, &name, *at), k.board.count(&k.territory.game, day));
                            if let Some(p) = place {
                                ui.add(egui::Label::new(RichText::new(place_words(p, of)).size(22.0).strong().color(Color32::WHITE)).wrap());
                            }
                        }
                        ui.label(RichText::new("Classifica di oggi").size(14.0).color(GREY));
                        ui.add_space(6.0);
                        board_rows(ui, k, stamp, rocca::score::BOARD_SHOWN);
                    });
                    ui.add_space(12.0);
                    ui.add(egui::Label::new(RichText::new("Nella realtà, quando arriva l'ordine di evacuazione si parte subito. Aspettare di vedere il fuoco è uno degli errori più pericolosi.").size(13.0).italics().color(AMBER)).wrap());
                    ui.add_space(12.0);
                    let big = egui::Button::new(RichText::new("Nuova partita").size(26.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(12.0);
                    let r = ui.add_sized([side, 64.0], big);
                    crate::playtest::control("Nuova partita", &r, true);
                    again = super::icons::pointing(r).clicked();
                });
            });
        });
    });
    if again {
        new_visit(k, sim, restarted);
    }
}
