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
use rocca::State;
use scenario::Pos;

use super::{board, new_visit, start_game, start_tutorial, Kiosk, Phase, Selection};
use crate::sim::{Sim, SimRestarted};

const PANEL: Color32 = Color32::from_rgba_premultiplied(18, 20, 24, 245);
pub(super) const AMBER: Color32 = Color32::from_rgb(250, 195, 70);
const RED: Color32 = Color32::from_rgb(255, 105, 85);
const BLUE: Color32 = Color32::from_rgb(110, 185, 255);
const GREEN: Color32 = Color32::from_rgb(110, 230, 150);
const GREY: Color32 = Color32::from_rgb(205, 208, 212);
/// The fire brigade's red and the volunteers' yellow-green, as on the map.
pub const VVF: Color32 = Color32::from_rgb(225, 60, 50);
pub const AIB: Color32 = Color32::from_rgb(200, 215, 60);
const SIDE_W: f32 = 340.0;

fn clock(s: i64) -> String {
    rocca::words::clock(s)
}

fn secondary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).size(17.0).color(Color32::WHITE))
        .fill(Color32::from_gray(62))
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(110)))
        .rounding(8.0)
        .min_size(egui::vec2(44.0, 40.0))
}

fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).size(20.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(10.0).min_size(egui::vec2(140.0, 46.0))
}

fn panel() -> egui::Frame {
    egui::Frame::none().fill(PANEL).rounding(8.0).inner_margin(egui::Margin::symmetric(12.0, 8.0))
}

fn card(tint: Color32) -> egui::Frame {
    egui::Frame::none().fill(PANEL).stroke(egui::Stroke::new(1.5, tint.gamma_multiply(0.8))).rounding(14.0).inner_margin(18.0)
}

/// World point to screen point, `lift_m` above the ground.
fn screen(cam: (&Camera, &GlobalTransform), sim: &Sim, p: Pos, lift_m: f32) -> Option<egui::Pos2> {
    let v = crate::frame::to_bevy(p, sim.scn.terrain.height_at(p) + lift_m);
    let s = cam.0.world_to_viewport(cam.1, v)?;
    Some(egui::pos2(s.x, s.y))
}

fn team_colour(sim: &Sim, t: usize) -> Color32 {
    if sim.teams[t].volunteers { AIB } else { VVF }
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
        if ui.add(secondary(if k.operator { "Chiudi menu" } else { "Menu" })).clicked() {
            k.operator = !k.operator;
        }
    });
    if k.operator {
        operator(ctx, k, &mut sim, &mut restarted, &mut gfx);
    }

    match k.phase {
        Phase::Intro => {
            k.side_w = 0.0;
            intro(ctx, k, &mut sim, &mut restarted);
        }
        Phase::Giorno2 => {
            k.side_w = 0.0;
            day_two(ctx, k, &mut sim, &mut restarted);
        }
        Phase::Fine => {
            k.side_w = 0.0;
            end(ctx, k, &mut sim, &mut restarted);
        }
        Phase::Tutorial | Phase::Gioco => {
            k.side_w = SIDE_W;
            top_bar(ctx, k, &mut sim);
            side_bar(ctx, k, &mut sim);
            map_badges(ctx, k, &mut sim, cam);
            map_click(k, &mut sim);
            if k.phase == Phase::Tutorial {
                tutorial(ctx, k, &mut sim, &mut restarted);
            }
            notice(ctx, k);
        }
    }
    focus.pointer = ctx.is_pointer_over_area();
    k.click = None;
}

// --- the pause and the weather ------------------------------------------------

fn top_bar(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim) {
    egui::Area::new(egui::Id::new("barra")).anchor(Align2::LEFT_TOP, [10.0, 10.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.horizontal(|ui| {
                let day = if k.phase == Phase::Tutorial { "Giorno 1" } else { "Giorno 2" };
                ui.label(RichText::new(format!("{day} · {}", clock(sim.time_s()))).size(22.0).strong().color(Color32::WHITE));
                ui.separator();
                let w = sim.weather();
                ui.label(
                    RichText::new(format!("vento da {} {:.0} km/h · combustibile secco: umidità {:.0} %", rocca::words::compass(w.wind_dir_deg), w.wind_speed_kmh, w.moisture_pct))
                        .size(16.0)
                        .color(GREY),
                );
                ui.separator();
                if let Some(left) = sim.pause_left_s() {
                    ui.label(RichText::new(format!("PAUSA · ancora {} min", (left + 59) / 60)).size(20.0).strong().color(AMBER));
                    let ok = k.allows(Expect::Resume);
                    let r = ui.add_enabled(ok, primary("Riprendi"));
                    crate::playtest::control("Riprendi", &r, ok);
                    if r.clicked() {
                        sim.resume();
                        k.clear_selection();
                        k.done(Expect::Resume);
                    }
                } else {
                    let ready = sim.can_pause() && k.allows(Expect::Pause);
                    let label = if sim.pause_ready_in_s() > 0 {
                        format!("Esperto tra {} min", (sim.pause_ready_in_s() + 59) / 60)
                    } else {
                        "Pausa: chiedi all'esperto".into()
                    };
                    let r = ui.add_enabled(ready, primary(&label));
                    crate::playtest::control("Pausa", &r, ready);
                    if r.clicked() {
                        match sim.pause() {
                            Ok(()) => k.done(Expect::Pause),
                            Err(e) => k.notify(e),
                        }
                    }
                }
            });
            if sim.paused() {
                ui.label(
                    RichText::new("Previsione dell'esperto, senza interventi: dove il fuoco può arrivare tra 1 ora (rosso) e tra 3 ore (viola).")
                        .size(15.0)
                        .color(Color32::WHITE),
                );
                match sim.weather_ahead() {
                    Some((at, w)) => ui.label(
                        RichText::new(format!("Meteo: alle {} il vento girerà da {}, {:.0} km/h.", clock(at), rocca::words::compass(w.wind_dir_deg), w.wind_speed_kmh))
                            .size(15.0)
                            .color(AMBER),
                    ),
                    None => ui.label(RichText::new("Meteo: vento stabile nelle prossime ore.").size(15.0).color(GREY)),
                };
            }
        });
    });
}

// --- the side bar: teams, districts, what is selected ---------------------------

fn side_bar(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim) {
    egui::SidePanel::right("lato").exact_width(SIDE_W).resizable(false).frame(egui::Frame::none().fill(PANEL).inner_margin(12.0)).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            selected(ui, k, sim);
            ui.add_space(8.0);
            ui.label(RichText::new("Squadre").size(20.0).strong().color(Color32::WHITE));
            for t in 0..sim.teams.len() {
                let on = k.selection == Selection::Team(t);
                let text = format!("{}  ·  {}", sim.teams[t].name, sim.team_status(t));
                let b = egui::Button::new(RichText::new(text).size(16.0).color(Color32::WHITE))
                    .fill(if on { team_colour(sim, t).gamma_multiply(0.45) } else { Color32::from_gray(45) })
                    .stroke(egui::Stroke::new(if on { 2.0 } else { 1.0 }, team_colour(sim, t)))
                    .min_size(egui::vec2(SIDE_W - 30.0, 36.0));
                let r = ui.add(b);
                crate::playtest::control(&sim.teams[t].name.clone(), &r, true);
                if r.clicked() {
                    select_team(k, sim, t);
                }
            }
            ui.add_space(10.0);
            ui.label(RichText::new("Borghi").size(20.0).strong().color(Color32::WHITE));
            for d in 0..sim.districts.len() {
                let on = k.selection == Selection::District(d);
                let text = format!("{}  ·  {}", sim.districts[d].name, district_state(sim, d));
                let b = egui::Button::new(RichText::new(text).size(16.0).color(Color32::WHITE))
                    .fill(if on { BLUE.gamma_multiply(0.4) } else { Color32::from_gray(45) })
                    .stroke(egui::Stroke::new(if on { 2.0 } else { 1.0 }, BLUE.gamma_multiply(0.7)))
                    .min_size(egui::vec2(SIDE_W - 30.0, 36.0));
                let r = ui.add(b);
                crate::playtest::control(&sim.districts[d].name.clone(), &r, true);
                if r.clicked() {
                    select_district(k, sim, d);
                }
            }
            ui.add_space(10.0);
            let o = sim.outcome();
            let (safe, caught, road): (u32, u32, u32) = (o.districts.iter().map(|d| d.safe).sum(), o.caught(), o.districts.iter().map(|d| d.on_the_road).sum());
            ui.label(RichText::new(format!("Al sicuro {safe} · in strada {road} · colpite dal fuoco {caught}")).size(15.0).color(GREY));
            ui.label(RichText::new(format!("Case colpite {} · bruciati {:.0} ha", o.homes_hit(), o.hectares)).size(15.0).color(GREY));
        });
    });
}

fn district_state(sim: &Sim, d: usize) -> String {
    let people: u32 = sim.districts[d].households.iter().map(|&i| sim.evac.families[i].people).sum();
    let caught = sim.evac.count(Some(d), State::Caught);
    let mut s = match sim.evac.of(d) {
        Some(e) if e.ended_s.is_none() => format!("evacuazione: ancora {} famiglie", e.waiting()),
        Some(e) if e.interrupted => "evacuazione interrotta".into(),
        Some(_) => format!("evacuati {}", sim.evac.count(Some(d), State::Safe) + sim.evac.count(Some(d), State::Road)),
        None => format!("{people} persone"),
    };
    if caught > 0 {
        s += &format!(" · {caught} colpite");
    }
    s
}

fn select_team(k: &mut Kiosk, sim: &Sim, t: usize) {
    if !k.allows(Expect::Select(t)) {
        k.notify(tutorial_hint(k));
        return;
    }
    k.clear_selection();
    k.selection = Selection::Team(t);
    k.done(Expect::Select(t));
    if k.phase == Phase::Gioco && !sim.paused() {
        k.notify("Per dare ordini metti in pausa: chiedi all'esperto.");
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

/// The selected team or district, its preview and «Conferma».
fn selected(ui: &mut egui::Ui, k: &mut Kiosk, sim: &mut Sim) {
    match k.selection {
        Selection::None => {
            let hint = if sim.paused() {
                "Clicca una squadra per spostarla, o un borgo per evacuarlo."
            } else {
                "Il tempo scorre. Per dare ordini: Pausa."
            };
            ui.label(RichText::new(hint).size(16.0).color(GREY));
        }
        Selection::Team(t) => {
            let name = sim.teams[t].name.clone();
            ui.label(RichText::new(format!("{name}: autobotte e {} {}", sim.teams[t].crew, if sim.teams[t].volunteers { "volontari" } else { "vigili del fuoco" })).size(18.0).strong().color(team_colour(sim, t)));
            match k.move_plan.clone() {
                None => {
                    ui.label(RichText::new("Clicca un esagono sulla mappa: lì la squadra attacca il fuoco e protegge le case vicine.").size(16.0).color(Color32::WHITE));
                }
                Some(Err(e)) => {
                    ui.colored_label(RED, RichText::new(e).size(16.0));
                }
                Some(Ok(p)) => {
                    let near = sim.place_name(p.hex.target).to_string();
                    ui.label(RichText::new(format!("Verso l'esagono vicino a {near}: {:.0} min di strada.", p.minutes.ceil())).size(17.0).color(Color32::WHITE));
                    let ok = sim.paused() && k.allows(Expect::Move(t, p.hex.at));
                    ui.horizontal(|ui| {
                        let r = ui.add_enabled(ok, primary("Conferma"));
                        crate::playtest::control("Conferma", &r, ok);
                        if r.clicked() {
                            let at = p.hex.at;
                            match sim.move_team(t, at) {
                                Ok(_) => {
                                    k.done(Expect::Move(t, at));
                                    k.clear_selection();
                                }
                                Err(e) => k.notify(e),
                            }
                        }
                        if ui.add(secondary("Annulla")).clicked() {
                            k.clear_selection();
                        }
                    });
                    if !sim.paused() {
                        ui.label(RichText::new("Gli ordini si danno solo in pausa.").color(AMBER));
                    }
                }
            }
        }
        Selection::District(d) => {
            ui.label(RichText::new(sim.districts[d].name.clone()).size(18.0).strong().color(BLUE));
            match k.evac_plan.clone() {
                None => {}
                Some(Err(e)) => {
                    ui.colored_label(RED, RichText::new(e).size(16.0));
                }
                Some(Ok(p)) => {
                    ui.label(
                        RichText::new(format!(
                            "Evacuare {} persone verso {}: circa {:.0} min (preparazione {:.0}, partenze {:.0}, viaggio {:.0}).",
                            p.people,
                            p.path.to,
                            p.total_min().ceil(),
                            p.prep_min,
                            p.leaving_min.ceil(),
                            p.drive_min.ceil()
                        ))
                        .size(16.0)
                        .color(Color32::WHITE),
                    );
                    ui.label(RichText::new(format!("Al massimo {:.0} persone al minuto, un borgo alla volta.", sim.evac.rules.per_min)).size(14.0).color(GREY));
                    if let Some(e) = sim.evac.active() {
                        ui.label(RichText::new(format!("Si sta già evacuando {}.", sim.districts[e.district].name)).color(AMBER));
                    }
                    let ok = sim.paused() && sim.evac.active().is_none() && k.allows(Expect::Evacuate(d));
                    ui.horizontal(|ui| {
                        let r = ui.add_enabled(ok, primary("Evacua"));
                        crate::playtest::control("Evacua", &r, ok);
                        if r.clicked() {
                            match sim.evacuate(d) {
                                Ok(_) => {
                                    k.done(Expect::Evacuate(d));
                                    k.clear_selection();
                                }
                                Err(e) => k.notify(e),
                            }
                        }
                        if ui.add(secondary("Annulla")).clicked() {
                            k.clear_selection();
                        }
                    });
                }
            }
        }
    }
}

/// A click on the map with a team selected: the hex under it, previewed.
fn map_click(k: &mut Kiosk, sim: &mut Sim) {
    let Some(p) = k.click else { return };
    if let Selection::Team(t) = k.selection {
        let at = rocca::Axial::of(p);
        if let Some(Expect::Move(_, want)) = k.expect() {
            if at != want {
                k.notify("Clicca l'esagono evidenziato.");
                return;
            }
        }
        k.move_hex = Some(at);
        k.move_plan = Some(sim.plan_move(t, at));
    }
}

/// Team and district badges on the map: clicking one selects it.
fn map_badges(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, cam: (&Camera, &GlobalTransform)) {
    for d in 0..sim.districts.len() {
        let c = sim.districts[d].centre;
        let Some(s) = screen(cam, sim, c, 40.0) else { continue };
        let name = sim.districts[d].name.clone();
        let on = k.selection == Selection::District(d);
        egui::Area::new(egui::Id::new(("borgo", d))).fixed_pos(s - egui::vec2(60.0, 14.0)).order(egui::Order::Middle).show(ctx, |ui| {
            let b = egui::Button::new(RichText::new(&name).size(15.0).strong().color(Color32::WHITE))
                .fill(if on { BLUE.gamma_multiply(0.6) } else { Color32::from_black_alpha(170) })
                .stroke(egui::Stroke::new(1.0, BLUE))
                .rounding(6.0);
            if ui.add(b).clicked() {
                select_district(k, sim, d);
            }
        });
    }
    for t in 0..sim.teams.len() {
        let u = &sim.crews.units[sim.teams[t].unit];
        let Some(s) = screen(cam, sim, u.pos, 60.0) else { continue };
        let on = k.selection == Selection::Team(t);
        let col = team_colour(sim, t);
        let name = sim.teams[t].name.clone();
        // several teams in the station: side by side
        let same = (0..t).filter(|&o| {
            let q = sim.crews.units[sim.teams[o].unit].pos;
            (q.x - u.pos.x).hypot(q.y - u.pos.y) < 30.0
        });
        let dx = same.count() as f32 * 64.0;
        egui::Area::new(egui::Id::new(("squadra", t))).fixed_pos(s + egui::vec2(dx - 30.0, -36.0)).order(egui::Order::Middle).show(ctx, |ui| {
            let b = egui::Button::new(RichText::new(&name).size(14.0).strong().color(Color32::BLACK)).fill(if on { Color32::WHITE } else { col }).rounding(6.0);
            if ui.add(b).clicked() {
                select_team(k, sim, t);
            }
        });
    }
}

fn notice(ctx: &egui::Context, k: &Kiosk) {
    let Some((text, _)) = &k.notice else { return };
    egui::Area::new(egui::Id::new("avviso")).anchor(Align2::CENTER_BOTTOM, [-SIDE_W / 2.0, -80.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.label(RichText::new(text).size(18.0).color(Color32::WHITE));
        });
    });
}

// --- the tutorial -------------------------------------------------------------

fn tutorial(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    let Some(step) = k.script.as_ref().and_then(|s| s.steps.get(k.step)).cloned() else { return };
    let mut skip = false;
    egui::Area::new(egui::Id::new("tutorial")).anchor(Align2::LEFT_BOTTOM, [10.0, -64.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        card(AMBER).show(ui, |ui| {
            ui.set_max_width(560.0);
            ui.label(RichText::new(format!("Tutorial · {}/{}", k.step + 1, k.script.as_ref().map_or(0, |s| s.steps.len()))).size(14.0).color(AMBER));
            ui.label(RichText::new(&step.text).size(20.0).color(Color32::WHITE));
            ui.horizontal(|ui| {
                if step.expect == Expect::Next {
                    let r = ui.add(primary("Avanti"));
                    crate::playtest::control("Avanti", &r, true);
                    if r.clicked() {
                        k.done(Expect::Next);
                    }
                }
                let r = ui.add(secondary("Salta il tutorial"));
                crate::playtest::control("Salta il tutorial", &r, true);
                skip = r.clicked();
            });
        });
    });
    if skip || (!ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Escape))) {
        start_game(k, sim, restarted);
    }
}

// --- the screens ---------------------------------------------------------------

fn intro(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    let mut go = None;
    egui::Area::new(egui::Id::new("intro")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        card(AMBER).show(ui, |ui| {
            ui.set_width(560.0);
            ui.label(RichText::new(super::TITLE).size(36.0).strong().color(Color32::WHITE));
            ui.label(
                RichText::new("Un incendio sull'isola. Sposta le squadre, evacua i borghi, decidi in pausa con l'aiuto dell'esperto. Prima una prova guidata, poi tocca a te.")
                    .size(18.0)
                    .color(GREY),
            );
            ui.add_space(10.0);
            ui.label(RichText::new("Come ti chiami?").size(18.0).color(Color32::WHITE));
            let r = ui.add(egui::TextEdit::singleline(&mut k.player).font(egui::FontId::proportional(22.0)).desired_width(300.0).char_limit(rocca::score::NAME_MAX));
            crate::playtest::control("Nome", &r, true);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let r = ui.add(primary("Inizia"));
                crate::playtest::control("Inizia", &r, true);
                if r.clicked() {
                    go = Some(true);
                }
                let r = ui.add(secondary("Salta la prova guidata"));
                crate::playtest::control("Salta la prova guidata", &r, true);
                if r.clicked() {
                    go = Some(false);
                }
            });
            ui.add_space(10.0);
            ranking(ui, k);
        });
    });
    match go {
        Some(true) => start_tutorial(k, sim, restarted),
        Some(false) => start_game(k, sim, restarted),
        None => {}
    }
}

fn day_two(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
    let mut go = false;
    egui::Area::new(egui::Id::new("giorno2")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        card(AMBER).show(ui, |ui| {
            ui.set_width(520.0);
            ui.label(RichText::new("Ben fatto: il fuoco è sotto controllo.").size(26.0).strong().color(GREEN));
            ui.label(RichText::new("Giorno 2. Un nuovo incendio, più grande, e il vento può cambiare. Ora decidi tu.").size(20.0).color(Color32::WHITE));
            ui.add_space(10.0);
            let r = ui.add(primary("Inizia"));
            crate::playtest::control("Inizia giorno 2", &r, true);
            go = r.clicked();
        });
    });
    if go {
        start_game(k, sim, restarted);
    }
}

fn stars_text(n: Option<u8>) -> String {
    match n {
        Some(n) => "★".repeat(n as usize) + &"☆".repeat(5 - n as usize),
        None => "—".into(),
    }
}

fn end(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>) {
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
    let mut again = false;
    egui::Area::new(egui::Id::new("fine")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        card(AMBER).show(ui, |ui| {
            ui.set_width(600.0);
            ui.label(RichText::new("Fine dell'incendio").size(30.0).strong().color(Color32::WHITE));
            let people = mine.people();
            ui.label(
                RichText::new(format!(
                    "Persone colpite dal fuoco: {} su {people}. Case colpite: {}. Bruciati {:.0} ettari.",
                    mine.caught(),
                    mine.homes_hit(),
                    mine.hectares
                ))
                .size(18.0)
                .color(Color32::WHITE),
            );
            match &base {
                None => {
                    ui.label(RichText::new("Calcolo il confronto con lo stesso incendio senza ordini…").color(GREY));
                }
                Some(Err(e)) => {
                    ui.colored_label(RED, e);
                }
                Some(Ok(none)) => {
                    let st = rocca::score::stars(&mine, none);
                    for (label, s) in [("Persone", st.persone), ("Case", st.case), ("Ettari", st.ettari)] {
                        ui.label(RichText::new(format!("{label:<8} {}", stars_text(s))).size(26.0).color(AMBER).monospace());
                    }
                    ui.label(
                        RichText::new(format!(
                            "Senza ordini lo stesso incendio avrebbe colpito {} persone e {} case, bruciando {:.0} ettari.",
                            none.caught(),
                            none.homes_hit(),
                            none.hectares
                        ))
                        .size(15.0)
                        .color(GREY),
                    );
                }
            }
            ui.add_space(8.0);
            ranking(ui, k);
            ui.add_space(8.0);
            let r = ui.add(primary("Nuova partita"));
            crate::playtest::control("Nuova partita", &r, true);
            again = r.clicked();
        });
    });
    if again {
        new_visit(k, sim, restarted);
    }
}

fn ranking(ui: &mut egui::Ui, k: &Kiosk) {
    let (day, _) = board::now();
    let rows = k.board.ranking(&k.territory.game, &day);
    if rows.is_empty() {
        return;
    }
    ui.label(RichText::new("I migliori di oggi").size(18.0).strong().color(Color32::WHITE));
    for (i, e) in rows.iter().take(rocca::score::BOARD_SHOWN).enumerate() {
        let me = k.saved.as_ref().is_some_and(|(_, at)| *at == e.at);
        let s: Vec<String> = e.stars.iter().map(|s| s.map_or("—".into(), |n| n.to_string())).collect();
        let text = format!("{}. {}  · persone {} · case {} · ettari {}", i + 1, e.name, s[0], s[1], s[2]);
        ui.label(RichText::new(text).size(15.0).color(if me { AMBER } else { GREY }));
    }
}

/// The operator menu (F2): a new visit, the game straight away, the world
/// stopped, the view, the graphics, the board.
fn operator(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>, gfx: &mut crate::quality::Graphics) {
    egui::Area::new(egui::Id::new("operatore")).anchor(Align2::LEFT_BOTTOM, [10.0, -64.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.label(RichText::new("Menu (F2 per chiudere)").strong().color(Color32::WHITE));
            ui.label(RichText::new(format!("{} · {} · seme {}", sim.case.name, clock(sim.time_s()), k.seed)).color(GREY));
            ui.horizontal(|ui| {
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
            ui.horizontal(|ui| {
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
