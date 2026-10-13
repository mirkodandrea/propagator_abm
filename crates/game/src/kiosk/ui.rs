//! The kiosk's interface: the map is the screen.
//!
//! - **On the map**, one small badge per district: its rank, how far the
//!   fire is, the units on it and the civil order. A click opens the full
//!   chip (families, defence, the controls to rank it and the two civil
//!   orders); one is open at a time. Each ground unit carries a label of
//!   what it is doing.
//! - **Top left**, «Il piano»: the priorities in order with units and civil
//!   orders, the one in force or the one being composed. Routes and posts are drawn in 3D by `overlays::update_routes`.
//! - **Top**: wind, clock and phase.
//! - **Bottom left**: someone speaking (`characters`): the mayor, the fire
//!   chief, the forecaster or a volunteer, with one line about what just
//!   happened; at a crisis, the crisis with its countdown and answers.
//! - **Bottom centre**: the one button (Avvia / Conferma).
//! - **Bottom right**: what the coordinator proposes and why, and the
//!   latest events, both closed by default.
//! - **Fine**: the debrief, against the same fire with no orders, with
//!   «Riprova», back to the opening screen to pick a level.
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
use super::tutorial;
use super::{board, close_crisis, commit, new_game, to_title, Kiosk, Phase};
use crate::sim::{Sim, SimRestarted};

const PANEL: Color32 = Color32::from_rgba_premultiplied(18, 20, 24, 250);
pub(super) const ORANGE: Color32 = Color32::from_rgb(240, 140, 50);
const RED: Color32 = Color32::from_rgb(255, 105, 85);
pub(super) const AMBER: Color32 = Color32::from_rgb(250, 195, 70);
const BLUE: Color32 = Color32::from_rgb(110, 185, 255);
const GREEN: Color32 = Color32::from_rgb(110, 230, 150);
/// What the two civil orders do, as the model does it (`Abm::prealert_of`,
/// `Abm::order_evacuation_of`).
/// Room a district chip is given on screen, for placing chips apart.
const CHIP_W: f32 = 320.0;
const CHIP_H: f32 = 400.0;
/// A chip while the plan runs untouched: name, families, defence. It opens
/// downward to the full chip under the pointer.
const CHIP_H_COMPACT: f32 = 201.0;
/// A district's badge on the map: rank, name, fire, units, order. Clicking it
/// opens the full chip beside it.
const BADGE_W: f32 = 150.0;
/// The messenger patrols on the map (`units::PATROL`), for the legend.
const PATROL: Color32 = Color32::from_rgb(40, 110, 230);
const BADGE_H: f32 = 46.0;
/// What the simulated families do is not what to do: said where it shows.
const REAL_LIFE: &str = "Nella realtà, quando arriva l'ordine di evacuazione si parte subito. Aspettare di vedere il fuoco è uno degli errori più pericolosi.";
const GREY: Color32 = Color32::from_rgb(205, 208, 212);

fn clock(s: i64) -> String {
    rocca::words::clock(s)
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
pub(super) fn secondary(text: &str) -> egui::Button<'static> {
    // 44 px: a finger, not a mouse pointer
    egui::Button::new(RichText::new(text.to_string()).size(17.0).color(Color32::WHITE))
        .fill(Color32::from_gray(62))
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(110)))
        .rounding(8.0)
        .min_size(egui::vec2(44.0, 44.0))
}

pub(super) fn primary(text: &str) -> egui::Button<'static> {
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
    logo: Option<Res<Logo>>,
    portraits: Option<Res<Portraits>>,
    mut speech: ResMut<Speech>,
    mut gfx: ResMut<crate::quality::Graphics>,
    mut mario: ResMut<crate::mario::Playground>,
) {
    crate::playtest::begin();
    let faces = portraits.as_deref();
    let ctx = contexts.ctx_mut();
    if mario.active {
        focus.pointer = ctx.is_pointer_over_area();
        return;
    }
    let k = &mut *kiosk;
    let Ok(cam) = cameras.get_single() else { return };
    if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::F2)) {
        k.operator = !k.operator;
    }
    // the same menu for a mouse or a touch (and the web page, which has no F2)
    egui::Area::new(egui::Id::new("menu_btn")).anchor(Align2::LEFT_TOP, [10.0, 10.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        let label = if k.operator { "Chiudi menu" } else { "Menu" };
        if ui.add(secondary(label)).clicked() {
            k.operator = !k.operator;
        }
    });
    // A hand over every button, and the hover shown at once with a white
    // edge (playtest colleghi: a late, faint hover over the level buttons
    // did not say they were buttons). Drawn buttons: `icons::pointing`.
    if ctx.style().visuals.interact_cursor.is_none() {
        ctx.style_mut(|s| {
            s.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
            s.animation_time = 0.0;
            s.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(3.0, Color32::WHITE);
            s.visuals.widgets.active.bg_stroke = egui::Stroke::new(3.0, Color32::WHITE);
        });
    }
    speech.player.clone_from(&k.player);

    // abandoned, or finished and sent back: the title screen
    if std::mem::take(&mut k.leave) {
        to_title(k, &mut sim, &mut restarted);
    }
    if k.title {
        k.side_w = 0.0;
        let (play, _mario_requested) = title(ctx, k, logo.as_deref().map(|l| l.0));
        if _mario_requested { mario.request_start(); }
        if let Some(error) = mario.error_message() {
            egui::Area::new(egui::Id::new("mario_title_error"))
                .anchor(Align2::CENTER_BOTTOM, [0.0, -52.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| { ui.colored_label(Color32::LIGHT_RED, error); });
        }
        if play {
            // a new game from the start; its no-orders run begins now, while
            // the visitor reads the opening screen and types a name
            k.title = false;
            k.case = k.pick.clone();
            new_game(k, &mut sim, &mut restarted);
            k.intro = true;
        }
        if k.operator {
            operator(ctx, k, &mut sim, &mut restarted, &mut gfx);
        }
        focus.pointer = true;
        return;
    }
    if k.intro {
        intro(ctx, k, &sim, logo.as_deref().map(|l| l.0), faces);
        // a level picked: play its case (a new game if it changes, or if
        // the choice came from a finished game's «Riprova»)
        if let Some(i) = k.picked_level.take() {
            if let Some(l) = k.territory.levels.get(i).cloned() {
                if l.case != k.case || k.phase == Phase::Fine {
                    k.case = l.case;
                    new_game(k, &mut sim, &mut restarted);
                }
                k.level = Some(i);
            }
            k.intro = false;
            k.started = true;
        }
        // after the intro closes (a level picked, or «Inizia»), a new visitor: show them the screen first
        if !k.intro && k.tour_on && !k.toured {
            tutorial::go(k, &sim, 0);
        }
        if k.operator {
            operator(ctx, k, &mut sim, &mut restarted, &mut gfx);
        }
        focus.pointer = true;
        return;
    }
    speech.update(&sim, ctx.input(|i| i.time));
    if k.phase == Phase::Fine {
        k.side_w = 0.0;
        // one panel: the old top bar (wind, hour) and the separate score are gone
        debrief(ctx, k, &mut sim, &mut restarted, faces);
    } else {
        // everything but the map in the side bar; on the map only what
        // points at a place, the opened chip and the speaker (user, 2026-10-12)
        sidebar(ctx, k, &mut sim, cam);
        let map = ctx.available_rect();
        k.side_w = ctx.screen_rect().width() - map.width();
        legend_corner(ctx, map);
        if k.phase == Phase::Crisi {
            crisis_bubble(ctx, k, faces);
        } else if k.tour.is_none() {
            speaker(ctx, k, &sim, &speech, faces);
        }
        // everything at ×1 has the same frame: the opening plan, a crisis,
        // an evacuation waiting for its confirmation
        if (k.phase == Phase::Pianifica && k.started) || k.phase == Phase::Crisi || k.confirm_evac.is_some() {
            slow_motion_frame(ctx, map);
        }
        evacuation_routes(ctx, k, &sim, cam);
        fire_marker(ctx, &sim, cam);
        unit_labels(ctx, &sim, cam);
        district_chips(ctx, k, &sim, cam, map.top());
        tutorial::draw(ctx, k, &sim, cam, faces);
    }
    if k.operator {
        operator(ctx, k, &mut sim, &mut restarted, &mut gfx);
    }
    focus.pointer = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

/// An arrow the way the wind pushes the fire, as seen on screen.
fn wind_arrow(ui: &mut egui::Ui, sim: &Sim, cam: (&Camera, &GlobalTransform)) {
    let w = sim.fire.weather();
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

/// The title screen: a game plays behind it (`kiosk::demo`), here only the
/// name and «Gioca». True when «Gioca» was pressed.
fn title(ctx: &egui::Context, k: &Kiosk, logo: Option<egui::TextureId>) -> (bool, bool) {
    let screen = ctx.screen_rect();
    let mut play = false;
    // Mario is an easter egg: no button, only the M key / gamepad Start (`mario::drive`)
    let mario_requested = false;
    egui::Area::new(egui::Id::new("titolo_fondo")).fixed_pos(screen.min).order(egui::Order::Middle).interactable(false).show(ctx, |ui| {
        let p = ui.painter();
        p.rect_filled(screen, 0.0, Color32::from_black_alpha(70));
        // darker toward the middle, where the words are: one mesh whose
        // vertex opacity follows a bell, so the band has no edge and no seams
        let (h, n) = (640.0, 32);
        let mut mesh = egui::Mesh::default();
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let bell = (std::f32::consts::PI * t).sin().powi(2);
            let y = screen.center().y - h / 2.0 + h * t;
            let c = Color32::from_black_alpha((170.0 * bell) as u8);
            mesh.colored_vertex(egui::pos2(screen.left(), y), c);
            mesh.colored_vertex(egui::pos2(screen.right(), y), c);
            if i > 0 {
                let k = 2 * i as u32;
                mesh.add_triangle(k - 2, k - 1, k);
                mesh.add_triangle(k - 1, k + 1, k);
            }
        }
        p.add(egui::Shape::mesh(mesh));
    });
    egui::Area::new(egui::Id::new("titolo")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.set_width(screen.width().min(900.0));
        ui.vertical_centered(|ui| {
            if let Some(t) = logo {
                ui.add(egui::Image::new(egui::load::SizedTexture::new(t, egui::vec2(64.0, 80.0))));
                ui.add_space(6.0);
            }
            ui.label(RichText::new("Isola di Roccavento").size(84.0).strong().color(Color32::WHITE));
            ui.label(RichText::new("Aiutaci a gestire un incendio.").size(26.0).color(AMBER));
            ui.add_space(34.0);
            let b = egui::Button::new(RichText::new("Gioca").size(44.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(16.0).min_size(egui::vec2(360.0, 96.0));
            let r = ui.add(b);
            crate::playtest::control("Gioca", &r, true);
            play = icons::pointing(r).clicked();
        });
    });
    egui::Area::new(egui::Id::new("titolo_nota")).anchor(Align2::LEFT_BOTTOM, [18.0, -14.0]).order(egui::Order::Foreground).interactable(false).show(ctx, |ui| {
        ui.label(RichText::new(format!("Simulazione dimostrativa · {}", k.case)).size(14.0).color(GREY.gamma_multiply(0.75)));
    });
    ctx.request_repaint();
    (play, mario_requested)
}

/// The opening screen: who made it, the situation, how to play, «Inizia».
fn intro(ctx: &egui::Context, k: &mut Kiosk, sim: &Sim, logo: Option<egui::TextureId>, faces: Option<&Portraits>) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("intro_fondo")).fixed_pos(screen.min).order(egui::Order::Middle).interactable(false).show(ctx, |ui| {
        ui.painter().rect_filled(screen, 0.0, Color32::from_rgba_premultiplied(8, 10, 14, 200));
    });
    // today's ranking stands on the right of the card, both centred as a
    // pair; the card gives up width when the window is narrow
    let (panel_w, gap) = (300.0, 16.0);
    let card_w = (screen.width() - 72.0 - panel_w - gap - 24.0).min(900.0);
    let pair = card_w + 72.0 + gap + panel_w;
    egui::Area::new(egui::Id::new("intro_classifica"))
        .anchor(Align2::CENTER_CENTER, [(pair - panel_w) / 2.0, 0.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let rows = if screen.height() >= 900.0 { rocca::score::BOARD_SHOWN } else { 4 };
            let day = board::now().0;
            egui::Frame::none()
                .fill(Color32::from_rgb(18, 20, 24))
                .stroke(egui::Stroke::new(1.5, AMBER.gamma_multiply(0.55)))
                .rounding(18.0)
                .inner_margin(egui::Margin { left: 16.0, right: 16.0, top: 16.0, bottom: 12.0 })
                .show(ui, |ui| {
                    ui.set_width(panel_w - 32.0);
                    ui.label(RichText::new("Classifica di oggi").size(22.0).strong().color(Color32::WHITE));
                    ui.label(RichText::new("le stelle di tutte le categorie").size(13.0).color(GREY));
                    for l in &k.territory.levels {
                        ui.add_space(8.0);
                        pill(ui, &l.name, level_colour(&l.name));
                        ui.add_space(2.0);
                        board_rows(ui, &k.board, &l.name, &day, None, rows);
                    }
                    if k.territory.levels.is_empty() {
                        ui.label(RichText::new("Nessuna partita oggi").size(14.0).color(GREY));
                    }
                });
        });
    egui::Area::new(egui::Id::new("intro")).anchor(Align2::CENTER_CENTER, [-(pair - card_w - 72.0) / 2.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::none()
            .fill(Color32::from_rgb(18, 20, 24))
            .stroke(egui::Stroke::new(1.5, AMBER.gamma_multiply(0.55)))
            .shadow(egui::epaint::Shadow { offset: egui::vec2(0.0, 10.0), blur: 40.0, spread: 4.0, color: Color32::from_black_alpha(160) })
            .rounding(18.0)
            .inner_margin(egui::Margin { left: 36.0, right: 36.0, top: 30.0, bottom: 24.0 })
            .show(ui, |ui| {
            ui.set_width(card_w);
            ui.horizontal(|ui| {
                if let Some(t) = logo {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(t, egui::vec2(77.0, 96.0))));
                    ui.add_space(14.0);
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new("Isola di Roccavento").size(44.0).strong().color(Color32::WHITE));
                    ui.label(RichText::new("Aiutaci a gestire questa emergenza.").size(22.0).color(AMBER));
                });
            });
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                characters::portrait(ui, faces, Who::Volontaria, Mood::Preoccupato, 110.0);
                ui.add_space(18.0);
                characters::bubble(ui, Who::Volontaria, |ui| {
                    // the fire is not chosen yet: say what is at stake, from the data
                    let roster = &k.territory.roster;
                    let engines = roster.iter().filter(|s| s.kind == rocca::case::Kind::Engine).count();
                    let crews = roster.len() - engines;
                    let n = |k: usize, one: &str, many: &str| if k == 1 { format!("1 {one}") } else { format!("{k} {many}") };
                    let text = format!(
                        "Un incendio minaccia i nostri borghi e ci serve il tuo aiuto! Abbiamo {} e {} per {} borghi: non bastano per tutti. Decidi tu come usarli. Il tempo scorre da solo e non si ferma: gli ordini li cambi mentre scorre, e rallenta solo quando c'è da decidere in fretta.",
                        n(engines, "autobotte", "autobotti"), n(crews, "squadra", "squadre"), sim.districts.len()
                    );
                    characters::says(ui, Who::Volontaria, &text, 600.0);
                });
            });
            ui.add_space(10.0);
            // the people the player works with, who speak during the game: a
            // team photo, faces and names only
            ui.label(RichText::new("LA SQUADRA CON CUI LAVORI").size(13.0).strong().color(GREY.gamma_multiply(0.7)));
            ui.add_space(6.0);
            let team = [Who::Sindaca, Who::Dos, Who::Meteo, Who::Volontaria];
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                for who in team {
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
            // how to play is shown on the map once the game starts (`tutorial`)
            ui.add_space(12.0);
            ui.label(RichText::new("Alla fine vedrai com'è andata, confrontata con lo stesso incendio senza ordini.").size(15.0).italics().color(GREY));
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            // the difficulty, as three fires (user, 2026-10-10); without
            // levels in game.json, the single button of before
            let levels = k.territory.levels.clone();
            if levels.is_empty() {
                ui.vertical_centered(|ui| {
                    let b = egui::Button::new(RichText::new("Inizia").size(30.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(12.0).min_size(egui::vec2(320.0, 70.0));
                    if ui.add(b).clicked() {
                        k.intro = false;
                        k.started = true;
                    }
                });
            } else {
                // the name first, so the characters can speak to the player
                // (user, 2026-10-12); a level starts only with one
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
                    // always the field's: a click elsewhere (a button, the map)
                    // may have moved the focus, and nothing else on this
                    // screen takes typing
                    if !r.has_focus() {
                        r.request_focus();
                    }
                });
                let named = !rocca::score::name(&k.player).is_empty();
                ui.add_space(14.0);
                let ask = if named { "Scegli la difficoltà e inizia" } else { "Scrivi il tuo nome, poi scegli la difficoltà" };
                ui.label(RichText::new(ask).size(20.0).strong().color(if named { Color32::WHITE } else { GREY }));
                ui.add_space(6.0);
                // three cards, green to red, as wide as the panel; the whole
                // card is the button
                let tints = [GREEN, AMBER, RED];
                ui.columns(levels.len(), |cols| {
                    for ((i, l), ui) in levels.iter().enumerate().zip(cols.iter_mut()) {
                        let tint = tints[i.min(tints.len() - 1)];
                        let tint = if named { tint } else { tint.gamma_multiply(0.7) };
                        let id = ui.id().with(("livello", i));
                        let hover = ui.ctx().memory(|m| m.data.get_temp::<bool>(id)).unwrap_or(false) && named;
                        let card = egui::Frame::none()
                            .fill(if hover { tint.gamma_multiply(0.30) } else { tint.gamma_multiply(0.12) })
                            .stroke(egui::Stroke::new(if hover { 2.5 } else { 1.5 }, tint))
                            .rounding(14.0)
                            .inner_margin(14.0)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.set_min_height(96.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(RichText::new(&l.name).size(30.0).strong().color(tint));
                                });
                                ui.add_space(4.0);
                                ui.vertical_centered(|ui| {
                                    ui.add(egui::Label::new(RichText::new(&l.about).size(15.0).color(if named { GREY } else { GREY.gamma_multiply(0.8) })).wrap());
                                });
                            });
                        let r = ui.interact(card.response.rect, id.with("clic"), egui::Sense::click());
                        crate::playtest::control(&l.name, &r, named);
                        ui.ctx().memory_mut(|m| m.data.insert_temp(id, r.hovered()));
                        if named {
                            let r = icons::pointing(r);
                            if r.clicked() {
                                k.player = rocca::score::name(&k.player);
                                k.picked_level = Some(i);
                            }
                        }
                    }
                });
            }
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if icons::pointing(ui.add(secondary("Indietro"))).clicked() {
                    k.leave = true;
                }
                ui.add_space(8.0);
            ui.label(
                RichText::new("Territorio immaginario, fuoco simulato con PROPAGATOR di Fondazione CIMA. Nella realtà segui sempre le indicazioni delle autorità.")
                    .size(13.0)
                    .color(GREY.gamma_multiply(0.75)),
            );
            });
        });
    });
}

/// The district a crisis is about, if any.
fn crisis_district(k: &Kiosk) -> Option<usize> {
    k.crisis.as_ref()?.kind.district()
}

/// Size of the portraits bottom left.
pub(super) const FACE: f32 = 132.0;
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
fn speaker(ctx: &egui::Context, k: &mut Kiosk, sim: &Sim, speech: &Speech, faces: Option<&Portraits>) {
    let tip;
    let line = match &speech.now {
        Some((l, _)) => l,
        None if k.phase == Phase::Pianifica && !k.started => {
            let text = format!("{} Guarda la mappa: dove sono i paesi e dove va il fuoco. Poi scegli la difficoltà.", the_fire(k, sim));
            tip = Line { who: Who::Volontaria, mood: Mood::Preoccupato, text };
            &tip
        }
        None if k.phase == Phase::Pianifica => {
            let text = format!("{} {}", the_fire(k, sim), characters::to(&k.player, "Scegli chi difendere per primo e chi avvisare, poi premi «Avvia». Dopo, gli ordini si cambiano mentre il tempo scorre."));
            tip = Line { who: Who::Volontaria, mood: Mood::Calmo, text };
            &tip
        }
        // what is new in the game (how to play was told by the tour)
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

/// Real seconds each piece of news stays before the next.
const NEWS_S: f64 = 8.0;

/// Where this fire starts and whom the wind drives it at: the case's `near`,
/// its wind, and the districts the fire is downwind of now.
pub(super) fn the_fire(k: &Kiosk, sim: &Sim) -> String {
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
fn crisis_bubble(ctx: &egui::Context, k: &Kiosk, faces: Option<&Portraits>) {
    let Some(c) = &k.crisis else { return };
    let (who, mood) = characters::for_crisis(c.kind);
    speaking(ctx, faces, who, mood, |ui| {
        characters::bubble(ui, who, |ui| {
            characters::says(ui, who, &c.text, BUBBLE_W);
            ui.label(RichText::new("Decidi nella barra a destra.").size(16.0).strong().color(Color32::from_rgb(170, 80, 10)));
        });
    });
}

/// In the side bar at a crisis, in place of the time's button: the
/// countdown, the answers, what the change does to the units, and the
/// button.
fn crisis_choice(ui: &mut egui::Ui, k: &mut Kiosk, sim: &mut Sim) {
    let Some(c) = k.crisis.clone() else { return };
    let d = crisis_district(k);
    let mut close = false;
    {
        egui::Frame::none().fill(Color32::from_rgb(64, 30, 10)).stroke(egui::Stroke::new(3.0, ORANGE)).rounding(10.0).inner_margin(12.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Alert, 24.0, ORANGE);
                ui.add(egui::Label::new(RichText::new(&c.title).size(20.0).strong().color(ORANGE)).wrap());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                });
            });
            // the why is the speaker's (`crisis_bubble`); here, what is at stake
            ui.label(RichText::new("Il tempo è rallentato: il fuoco avanza ancora.").size(14.0).color(GREY));
            // the other districts the mayor has authorised and nobody has evacuated yet
            let more: Vec<usize> = (0..sim.districts.len())
                .filter(|&x| Some(x) != d && sim.mayor.is_some() && sim.may_evacuate(x) && sim.active.civil[x] != Civil::Evacua)
                .collect();
            for x in more {
                let on = k.proposed.civil[x] == Civil::Evacua;
                if icons::button(ui, Icon::Exit, &format!("Evacua anche {}", sim.districts[x].name), on, BLUE, true).clicked() {
                    k.proposed.civil[x] = if on { sim.active.civil[x] } else { Civil::Evacua };
                    k.dirty = true;
                }
            }
            ui.horizontal_wrapped(|ui| {
                // A unit lost: the choice is who comes first with the units
                // left (playtest 3: a countdown with only «Continua» was a
                // decision without options).
                if d.is_none() && matches!(c.kind, rocca::crisis::Kind::MezzoPerso { .. }) {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Con i mezzi rimasti, chi difendere per primo?").size(15.0).color(Color32::WHITE));
                        ui.horizontal_wrapped(|ui| {
                            for x in 0..sim.districts.len() {
                                let first = k.proposed.priorities.first() == Some(&x);
                                if icons::button(ui, Icon::Up, &sim.districts[x].name, first, AMBER, !first).clicked() {
                                    k.proposed.priorities.retain(|&y| y != x);
                                    k.proposed.priorities.insert(0, x);
                                    k.dirty = true;
                                }
                            }
                        });
                    });
                }
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
                        let allowed = o != Civil::Evacua || sim.may_evacuate(d);
                        if icons::button(ui, icon, label, on, colour, active < o && allowed).clicked() {
                            k.proposed.civil[d] = if k.proposed.civil[d] == o { active } else { o };
                            k.dirty = true;
                        }
                    }
                }
            });
            ui.add_space(6.0);
            let pending = k.proposed != sim.active;
            if big_button(ui, None, if pending { "Conferma" } else { "Continua" }, AMBER).clicked() {
                close = true;
            }
            // what the change does to the units, as engine icons
            if let Some(p) = &k.preview {
                let moves = (0..sim.districts.len()).any(|x| sim.posts.iter().flatten().filter(|q| q.district == x).count() != p.units_on(x));
                if moves {
                    ui.label(RichText::new("Con questa scelta i mezzi si spostano così:").size(15.0).color(Color32::WHITE));
                }
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
    }
    if close {
        close_crisis(k, sim);
    }
}

/// What the marks on the map mean, in the side bar, closed at first
/// (`KIOSK_LEGEND=1` opens it, for screenshots).
fn legend(ui: &mut egui::Ui) {
    #[derive(Clone, Copy)]
    enum Mark {
        Fill(Color32),
        Ring(Color32),
        Line(Color32, bool),
        Arrow,
        Diamond,
        Stem,
        Patrol,
    }
    // Only what is on the map or on the cards, short (playtest colleghi:
    // too much, and some of it seen only in the debrief). The cards say
    // what each order does; the plants come first, the most asked.
    let line = |ui: &mut egui::Ui, text: &str| {
        ui.add(egui::Label::new(RichText::new(text).size(14.0).color(GREY)).wrap());
    };
    section(ui, "Vegetazione: come brucia", true, |ui| {
        for (kind, text) in [
            (Plant::Grass, "Prato secco: corre veloce, fiamme basse."),
            (Plant::Shrub, "Macchia: brucia forte, lancia faville."),
            (Plant::Pine, "Pineta: fiamme alte, faville lontane."),
            (Plant::Chestnut, "Castagneto: il fuoco avanza più lento."),
        ] {
            ui.horizontal(|ui| {
                plant_sample(ui, kind);
                line(ui, text);
            });
        }
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(40.0, 26.0), egui::Sense::hover());
            ui.painter().rect_filled(r.shrink(2.0), 4.0, Color32::from_rgb(214, 207, 189));
            line(ui, "Strade, case, orti: non bruciano.");
        });
    });
    section(ui, "Etichette dei paesi e mezzi", true, |ui| {
        for text in [
            "Fiammella e «0,6 km»: quanto è lontano il fuoco dalla casa più vicina.",
            "Numero nel cerchio: posto del paese nella difesa.",
            "Camion e numero: mezzi che difendono il paese.",
            "17/190: famiglie avvisate su tutte (in verde: già in salvo).",
            "A1, A2: autobotti. SQ: squadra a piedi.",
            "Cornice arancione intorno alla mappa: tempo rallentato, è il momento di decidere.",
        ] {
            line(ui, text);
        }
    });
    section(ui, "Segni sulla mappa", true, |ui| {
        let items = [
            (Mark::Fill(Color32::from_rgb(255, 120, 40)), "fuoco attivo"),
            (Mark::Fill(Color32::from_rgb(70, 50, 40)), "area già bruciata"),
            (Mark::Arrow, "dove il vento spinge il fuoco"),
            (Mark::Stem, "un mezzo: il punto è dove si trova"),
            (Mark::Patrol, "pattuglia che porta gli avvisi casa per casa"),
            (Mark::Ring(GREEN), "case difese da un mezzo"),
            (Mark::Line(GREEN, false), "strada che un mezzo sta facendo"),
            (Mark::Line(Color32::WHITE, true), "spostamento proposto, da confermare"),
            (Mark::Line(Color32::from_rgb(255, 92, 61), false), "previsione del fuoco tra 1 ora (viola tratteggiato: tra 3 ore), senza interventi"),
            (Mark::Ring(AMBER), "paese in preallerta"),
            (Mark::Ring(BLUE), "paese in evacuazione"),
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
                    Mark::Stem => {
                        // as `unit_labels` draws it: the label, the stem, the dot
                        p.rect_stroke(egui::Rect::from_center_size(egui::pos2(c.x, r.top() + 2.0), egui::vec2(16.0, 6.0)), 2.0, egui::Stroke::new(1.5, GREEN));
                        p.line_segment([egui::pos2(c.x, r.top() + 5.0), egui::pos2(c.x, r.bottom() - 3.0)], egui::Stroke::new(2.0, GREEN));
                        p.circle_filled(egui::pos2(c.x, r.bottom() - 3.0), 3.5, GREEN);
                    }
                    Mark::Patrol => {
                        // as `patrol_labels` draws it: a bell on a stem
                        icons::draw(p, egui::Rect::from_center_size(egui::pos2(c.x, r.top() + 4.0), egui::vec2(10.0, 10.0)), Icon::Bell, PATROL);
                        p.line_segment([egui::pos2(c.x, r.top() + 9.0), egui::pos2(c.x, r.bottom() - 3.0)], egui::Stroke::new(2.0, PATROL));
                        p.circle_filled(egui::pos2(c.x, r.bottom() - 3.0), 3.5, PATROL);
                    }
                }
                line(ui, text);
            });
        }
    });
    section(ui, "Famiglie e tempi", true, |ui| {
        for (colour, text) in [(GREEN, "in salvo"), (BLUE, "in viaggio"), (AMBER, "si preparano"), (Color32::from_gray(150), "a casa"), (RED, "raggiunte dal fuoco")] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 2.0, colour);
                line(ui, text);
            });
        }
        line(ui, "Dall'ordine alla partenza passano minuti: l'avviso deve arrivare e le famiglie devono prepararsi. La scheda stima preparazione e strada, senza le code. La preallerta accorcia la preparazione.");
        line(ui, "Le ore sono quelle del gioco: a ×40, 40 minuti passano in 1 minuto vero.");
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
            return "Difesa: nessun mezzo mandato qui".into();
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

/// The legend, folded, in the map's top right corner (just left of the side
/// bar, clear of the «Menu» button top left and the speaker bottom left).
/// `KIOSK_LEGEND=1` opens it, for screenshots.
fn legend_corner(ctx: &egui::Context, map: egui::Rect) {
    let right = ctx.screen_rect().right() - map.right() + 10.0;
    egui::Area::new(egui::Id::new("legenda")).anchor(Align2::RIGHT_TOP, [-right, 10.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::none().fill(Color32::from_rgba_unmultiplied(14, 16, 18, 235)).stroke(egui::Stroke::new(1.5, Color32::from_gray(90))).rounding(8.0).inner_margin(8.0).show(ui, |ui| {
            ui.set_max_width(340.0);
            let title = RichText::new("Legenda").size(16.0).strong().color(Color32::WHITE);
            egui::CollapsingHeader::new(title).id_source("legenda_h").default_open(std::env::var("KIOSK_LEGEND").is_ok()).show(ui, |ui| {
                legend(ui);
            });
        });
    });
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
pub(super) fn chip_anchor(sim: &Sim, d: usize) -> Pos {
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
    let screen_rect = ctx.available_rect();
    let n = sim.districts.len();
    let mut order = k.proposed.priorities.clone();
    let mut civil = k.proposed.civil.clone();
    let mut changed = false;
    let can_order = k.can_order();
    // While the plan in force runs and nothing is being composed, chips are
    // compact, so three of them fit near each other; the one under the
    // pointer opens (downward, so what was under the pointer stays put).
    let compact = k.phase == Phase::Esegui && k.proposed == sim.active;
    // Where each chip stands (bottom centre) and how big it was last frame.
    let mut place: Vec<Option<(egui::Pos2, egui::Vec2)>> = (0..n)
        .map(|d| {
            let at = screen(cam, sim, chip_anchor(sim, d), 26.0)?;
            // A fixed size, not last frame's, so a badge never shifts.
            Some((at, egui::vec2(BADGE_W, BADGE_H)))
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
    let obstacles: Vec<egui::Rect> = ["personaggio"]
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
    // Clipped to the map: a district behind the side bar draws no line across it.
    let leaders = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("guide"))).with_clip_rect(screen_rect);
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
        if dist.nuclei.len() < 2 || k.selected != Some(d) { continue; }
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
    // The badges: always on the map, small; a click opens the chip.
    let mut clicked: Option<usize> = None;
    for d in 0..n {
        let dist = &sim.districts[d];
        let Some((at, _)) = place[d] else { continue };
        let rank = order.iter().position(|&x| x == d);
        let now = sim.posts.iter().flatten().filter(|p| p.district == d).count();
        let next = k.preview.as_ref().map_or(now, |p| p.units_on(d));
        let selected = k.selected == Some(d);
        let (border, width) = if alarm == Some(d) { (ORANGE, 4.0) } else if selected { (Color32::WHITE, 3.0) } else if rank == Some(0) { (AMBER, 2.0) } else { (Color32::from_gray(90), 1.5) };
        egui::Area::new(egui::Id::new(("badge", d))).fixed_pos(at).pivot(Align2::CENTER_BOTTOM).order(egui::Order::Middle).show(ctx, |ui| {
            let framed = egui::Frame::none().fill(Color32::from_rgba_unmultiplied(18, 20, 24, 150)).stroke(egui::Stroke::new(width, border)).rounding(9.0).inner_margin(5.0).show(ui, |ui| {
                ui.set_width(BADGE_W - 10.0);
                ui.set_min_height(BADGE_H - 10.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                    match rank {
                        Some(n) => {
                            ui.painter().circle_filled(r.center(), 9.5, AMBER);
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, (n + 1).to_string(), egui::FontId::proportional(13.0), Color32::BLACK);
                        }
                        None => {
                            ui.painter().circle_stroke(r.center(), 9.0, egui::Stroke::new(2.0, Color32::from_gray(110)));
                        }
                    }
                    ui.label(RichText::new(&dist.name).size(13.0).strong().color(Color32::WHITE));
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if let Some(e) = k.risk.get(d).copied().flatten() {
                        let colour = if e.distance_m < 1000.0 { RED } else if e.distance_m < 2500.0 { ORANGE } else { GREY };
                        let text = if e.distance_m < 60.0 { "qui".to_string() } else { km(e.distance_m) };
                        egui::Frame::none().fill(colour).rounding(5.0).inner_margin(egui::Margin::symmetric(4.0, 0.0)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                icons::show(ui, Icon::Fire, 11.0, Color32::from_rgb(120, 20, 0));
                                ui.label(RichText::new(text).size(11.0).strong().color(Color32::BLACK));
                            });
                        });
                    }
                    // trucks on it (now, or after the confirm), and the civil order
                    if now.max(next) > 0 {
                        icons::show(ui, Icon::Engine, 14.0, if next >= now { GREEN } else { AMBER });
                        ui.label(RichText::new(format!("{next}")).size(11.0).color(Color32::WHITE));
                    }
                    match civil[d] {
                        Civil::Evacua => { icons::show(ui, Icon::Exit, 14.0, BLUE); }
                        Civil::Preallerta => { icons::show(ui, Icon::Bell, 14.0, AMBER); }
                        Civil::Nessuno => {}
                    }
                    // the families: how many have heard the order while the
                    // patrols are still knocking, then how many are out
                    let hh = &dist.households;
                    let safe = hh.iter().filter(|&&i| sim.agents.households[i].status == Status::Evacuated).count();
                    let reached = hh.iter().filter(|&&i| matches!(sim.agents.households[i].status, Status::Trapped | Status::Casualty)).count();
                    let telling = sim.alerted(d).filter(|&(heard, all)| heard < all);
                    if reached > 0 {
                        icons::show(ui, Icon::Alert, 14.0, RED);
                    } else if let Some((heard, all)) = telling {
                        let colour = if sim.active.civil[d] == Civil::Evacua { BLUE } else { AMBER };
                        ui.label(RichText::new(format!("{heard}/{all}")).size(11.0).color(colour));
                    } else if safe > 0 {
                        ui.label(RichText::new(format!("{safe}/{}", hh.len())).size(11.0).color(GREEN));
                    }
                });
            });
            let hit = icons::pointing(ui.interact(framed.response.rect, egui::Id::new(("badge_hit", d)), egui::Sense::click()));
            crate::playtest::control(&format!("Mappa:{}", dist.name), &hit, k.can_order());
            if hit.clicked() {
                clicked = Some(d);
            }
        });
    }
    if let Some(d) = clicked {
        k.selected = if k.selected == Some(d) { None } else { Some(d) };
    }
    // The opened chip, beside its badge: the whole card, and the orders.
    let mut close = false;
    for d in 0..n {
        if k.selected != Some(d) {
            continue;
        }
        let dist = &sim.districts[d];
        let Some((at, _)) = place[d] else { continue };
        let id = egui::Id::new(("quartiere", d));
        let rank = order.iter().position(|&x| x == d);
        let small = false;
        let height = if compact { CHIP_H_COMPACT } else { CHIP_H };
        let left = (at.x - CHIP_W * 0.5).clamp(8.0, screen_rect.right() - CHIP_W - 8.0);
        // below the badge if it fits, else above
        let area = if at.y + 6.0 + height < screen_rect.bottom() - 90.0 {
            egui::Area::new(id).fixed_pos(egui::pos2(left, at.y + 6.0)).pivot(Align2::LEFT_TOP).order(egui::Order::Foreground)
        } else {
            egui::Area::new(id).fixed_pos(egui::pos2(left, (at.y - BADGE_H - 6.0).max(top + height + 8.0))).pivot(Align2::LEFT_BOTTOM).order(egui::Order::Foreground)
        };
        let shown = area.show(ctx, |ui| {
            let (border, width) = if alarm == Some(d) { (ORANGE, 4.0) } else if rank == Some(0) { (AMBER, 1.5) } else { (Color32::from_gray(90), 1.5) };
            egui::Frame::none().fill(Color32::from_rgb(18, 20, 24)).stroke(egui::Stroke::new(width, border)).rounding(10.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(CHIP_W - 20.0);
                if small {
                    ui.set_min_height(CHIP_H_COMPACT - 22.0);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let b = egui::Button::new(RichText::new("Chiudi").size(14.0).color(Color32::WHITE)).fill(Color32::from_gray(52)).rounding(6.0).min_size(egui::vec2(60.0, 24.0));
                    if ui.add(b).clicked() {
                        close = true;
                    }
                });
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
                // families and engines
                let now = sim.posts.iter().flatten().filter(|p| p.district == d).count();
                let next = k.preview.as_ref().map_or(now, |p| p.units_on(d));
                if !compact {
                    // a place ranked but given no unit says so before the
                    // commit (playtest 3: «Difendi» promised more than it gave)
                    let (text, colour) = match rank {
                        Some(r) if next == 0 => (format!("Priorità {} · 0 mezzi ora", r + 1), AMBER),
                        Some(r) => (format!("Priorità {}{}", r + 1, if dist.nuclei.len() > 1 { " · tutti i nuclei" } else { "" }), GREY),
                        None => ("Non difeso".into(), GREY),
                    };
                    ui.label(RichText::new(text).size(14.0).color(colour));
                }
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
                // Compact: the defence line only; opened, the orders under it
                // and nothing else, so it covers little of the scene.
                if compact {
                    ui.label(RichText::new(defence_line(k, sim, d)).size(14.0).color(GREEN));
                    if small {
                        return;
                    }
                }
                let hh = &dist.households;
                let count = |states: &[Status]| hh.iter().filter(|&&i| states.contains(&sim.agents.households[i].status)).count();
                if !compact {
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
                    ui.allocate_ui(egui::vec2(CHIP_W - 20.0, 60.0), |ui| {
                        ui.label(RichText::new(defence_line(k, sim, d)).size(14.0).color(GREEN));
                        let road = |a: u32, b: u32| if b > a { format!("~{a} min di strada, le ultime ~{b} min") } else { format!("~{a} min di strada") };
                        // how long before they leave, too (playtest colleghi:
                        // when to order it was guesswork)
                        let ready = match k.readying.get(d).copied().flatten() {
                            Some(m) => format!("~{m} min per prepararsi, poi "),
                            None => String::new(),
                        };
                        let journey = match k.evacuation.get(d) {
                            Some((Some((a, b)), 0)) => format!("Evacua: {ready}{}", road(*a, *b)),
                            Some((Some((a, b)), blocked)) => format!("Evacua: {ready}{} · {blocked} con la strada tagliata", road(*a, *b)),
                            _ => "Evacua: viaggio non disponibile".into(),
                        };
                        ui.add(egui::Label::new(RichText::new(journey).size(14.0).color(BLUE)).wrap());
                    });
                }
                // Orders are given while the time runs and apply at once
                // (user, 2026-10-13); an evacuation first shows its way out.
                if k.phase == Phase::Esegui {
                    if let Some((heard, all)) = sim.alerted(d).filter(|&(heard, all)| heard < all) {
                        ui.label(RichText::new(format!("Avvisate {heard} famiglie su {all}: le pattuglie bussano porta a porta")).size(14.0).color(GREY));
                    }
                    let queued = sim.agents.waiting_to_leave().iter().filter(|i| dist.households.contains(i)).count();
                    if queued > 0 {
                        ui.label(RichText::new(format!("{queued} famiglie pronte aspettano il turno per uscire")).size(14.0).color(BLUE));
                    }
                }
                // The three orders on one row, icon over word, what each does
                // under them (playtest colleghi); greyed until the game has started.
                ui.add_space(10.0);
                ui.add_enabled_ui(can_order, |ui| {
                    ui.horizontal(|ui| {
                        let gap = ui.spacing().item_spacing.x;
                        let w = ((CHIP_W - 20.0) - 2.0 * gap) / 3.0;
                        // «Difendi» on: ranked; pressed again, not defended
                        if icons::tall_button(ui, Icon::Shield, "Difendi", rank.is_some(), AMBER, true, w).clicked() {
                            if rank.is_some() {
                                order.retain(|&x| x != d);
                            } else {
                                order.push(d);
                            }
                            changed = true;
                        }
                        let active = sim.active.civil[d];
                        for (c, icon, label, colour) in [(Civil::Preallerta, Icon::Bell, "Preallerta", AMBER), (Civil::Evacua, Icon::Exit, "Evacua", BLUE)] {
                            let on = civil[d] == c || (active >= c && civil[d] <= c);
                            // the evacuation is the mayor's to authorise (spec §1)
                            let allowed = c != Civil::Evacua || sim.may_evacuate(d);
                            if icons::tall_button(ui, icon, label, on, colour, active < c && allowed, w).clicked() {
                                if c == Civil::Evacua && k.phase == Phase::Esegui && k.begun && civil[d] != c {
                                    // the way out first, then the confirmation
                                    k.confirm_evac = Some((d, 0.0));
                                } else {
                                    civil[d] = if civil[d] == c { active } else { c.max(active) };
                                    changed = true;
                                }
                            }
                        }
                    });
                    if let Some(r) = rank.filter(|&r| r > 0) {
                        ui.add_space(4.0);
                        if icons::button(ui, Icon::Up, &format!("È {}° nella difesa: portalo al 1° posto", r + 1), false, AMBER, true).clicked() {
                            order.retain(|&x| x != d);
                            order.insert(0, d);
                            changed = true;
                        }
                    }
                });
                ui.add_space(10.0);
                if !can_order {
                    // playtest 6: grey buttons with no reason given
                    ui.label(RichText::new("Scegli la difficoltà per cominciare.").size(14.0).strong().color(AMBER));
                } else {
                    if !sim.may_evacuate(d) {
                        ui.label(RichText::new("«Evacua» serve l'autorizzazione della sindaca: arriva quando il fuoco minaccia il paese.").size(14.0).strong().color(BLUE));
                    }
                    if (civil[d] != sim.active.civil[d] || next != now) && k.phase == Phase::Crisi {
                        pill(ui, "da confermare", AMBER);
                    }
                    // as the model does it (`Abm::prealert_of`): ready at home,
                    // a few leave on their own, the order then reaches them sooner
                    for (colour, text) in [
                        (AMBER, "Difendi: manda i mezzi a proteggere le case. Ripremi per toglierlo."),
                        (AMBER, "Preallerta: si preparano a casa e qualcuno parte da sé; un'evacuazione dopo li raggiunge prima."),
                        (BLUE, "Evacua: si preparano e partono; qualcuno aspetta di vedere il fuoco. Gli ordini alle famiglie non si ritirano."),
                    ] {
                        ui.add(egui::Label::new(RichText::new(text).size(13.0).color(colour.gamma_multiply(0.85))).wrap());
                    }
                    if let Some(m) = sim.messengers {
                        let text = format!("Gli avvisi li portano {} pattuglie, casa per casa; chi ha l'allerta sul telefono la riceve subito. Più paesi avvisi insieme, più tardi arrivano a ciascuno.", m.teams);
                        ui.add(egui::Label::new(RichText::new(text).size(13.0).color(GREY)).wrap());
                    }
                }
            });
        });
        let _ = shown;
    }
    if close {
        k.selected = None;
    }
    if changed {
        k.proposed.priorities = order;
        k.proposed.civil = civil;
        k.dirty = true;
    }
}

/// The plan, in one place: the priorities in order, each with its units and
/// civil order, then what is ordered without a defence, who stays at base and
/// who is left out. The one in force while it runs; the proposal while it is
/// composed. A row opens that district's chip.
/// A district dragged in the plan's columns.
#[derive(Clone, Copy)]
struct Dragged(usize);

/// Where a drop lands.
#[derive(Clone, Copy, PartialEq)]
enum Column {
    /// Defended, at this rank.
    Difesa(usize),
    Preallerta,
    Evacua,
    /// Not defended (from the defence column only: civil orders are not
    /// taken back).
    Nessuno,
}

/// The plan as three columns (user, 2026-10-13): Difesa, ranked, and the two
/// civil orders. A district is dragged into a column, or up and down the
/// defence to change its rank; the ones with no order wait in a row below.
/// While the time runs a change applies at once, an evacuation after its
/// confirmation; in a crisis it is proposed until «Conferma». A name opens
/// that district's chip.
fn plan(ui: &mut egui::Ui, k: &mut Kiosk, sim: &Sim) {
    let pending = k.phase == Phase::Crisi && k.proposed != sim.active;
    let plan = k.proposed.clone();
    let n = sim.districts.len();
    let units_on = |d: usize| -> usize {
        match (&k.preview, pending || k.phase == Phase::Pianifica) {
            (Some(p), true) => p.units_on(d),
            _ => sim.posts.iter().flatten().filter(|p| p.district == d).count(),
        }
    };
    let mut open: Option<usize> = None;
    let mut drop: Option<(usize, Column)> = None;
    heading(ui, "Il piano");
    let (tag, colour) = if !k.started {
        ("nessun ordine ancora", Color32::from_gray(150))
    } else if pending {
        ("da confermare", AMBER)
    } else if k.phase == Phase::Pianifica {
        ("parte con «Avvia»", AMBER)
    } else {
        ("in vigore: trascina i paesi per cambiarlo", GREEN)
    };
    ui.label(RichText::new(tag).size(14.0).color(colour));
    ui.add_space(4.0);
    let enabled = k.can_order();
    let gap = 6.0;
    // the defence carries ranks and engines: the widest
    let full = ui.available_width() - 2.0 * gap;
    let widths = [full * 0.42, full * 0.29, full * 0.29];
    let dragging = egui::DragAndDrop::has_payload_of_type::<Dragged>(ui.ctx());
    // one name, draggable; a click opens the chip
    let item = |ui: &mut egui::Ui, d: usize, rank: Option<usize>, tint: Color32, col_w: f32, open: &mut Option<usize>| -> egui::Rect {
        let id = egui::Id::new(("piano", d, rank.is_some(), tint.to_array()));
        let r = ui.dnd_drag_source(id, Dragged(d), |ui| {
            egui::Frame::none().fill(tint.gamma_multiply(0.18)).stroke(egui::Stroke::new(1.0, tint)).rounding(6.0).inner_margin(egui::Margin::symmetric(5.0, 3.0)).show(ui, |ui| {
                ui.set_width(col_w - 12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if let Some(r) = rank {
                        ui.label(RichText::new(format!("{}", r + 1)).size(14.0).strong().color(AMBER));
                    }
                    let name = RichText::new(&sim.districts[d].name).size(14.0).color(Color32::WHITE);
                    let name = if k.selected == Some(d) { name.underline() } else { name };
                    ui.add(egui::Label::new(name).truncate());
                    if rank.is_some() {
                        let u = units_on(d);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(u.to_string()).size(13.0).color(if u > 0 { Color32::WHITE } else { ORANGE }));
                            icons::show(ui, Icon::Engine, 14.0, if u > 0 { GREEN } else { Color32::from_gray(90) });
                        });
                    }
                });
            });
        });
        let resp = r.response;
        crate::playtest::control(&format!("Borgo:{}", sim.districts[d].name), &resp, enabled);
        if resp.clicked() {
            *open = Some(d);
        }
        resp.rect
    };
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for ((title, icon, tint, which), col_w) in [
                ("Difesa", Icon::Shield, AMBER, Column::Difesa(0)),
                ("Preallerta", Icon::Bell, AMBER, Column::Preallerta),
                ("Evacuazione", Icon::Exit, BLUE, Column::Evacua),
            ].into_iter().zip(widths) {
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        icons::show(ui, icon, 16.0, tint);
                        ui.label(RichText::new(title).size(14.0).strong().color(tint));
                    });
                    let frame = egui::Frame::none()
                        .fill(Color32::from_gray(30))
                        .stroke(egui::Stroke::new(1.0, if dragging { tint } else { Color32::from_gray(60) }))
                        .rounding(6.0)
                        .inner_margin(4.0);
                    let (resp, payload) = ui.dnd_drop_zone::<Dragged, _>(frame, |ui| {
                        ui.set_min_size(egui::vec2(col_w - 8.0, 120.0));
                        ui.spacing_mut().item_spacing.y = 4.0;
                        let mut rects = vec![];
                        match which {
                            Column::Difesa(_) => {
                                for (r, &d) in plan.priorities.iter().enumerate() {
                                    rects.push(item(ui, d, Some(r), AMBER, col_w, &mut open));
                                }
                            }
                            Column::Preallerta | Column::Evacua => {
                                let c = if which == Column::Evacua { Civil::Evacua } else { Civil::Preallerta };
                                for d in (0..n).filter(|&d| plan.civil[d] == c) {
                                    item(ui, d, None, tint, col_w, &mut open);
                                }
                                if which == Column::Evacua && sim.mayor.is_some() {
                                    let ok: Vec<&str> = (0..n).filter(|&d| sim.may_evacuate(d) && plan.civil[d] != Civil::Evacua).map(|d| sim.districts[d].name.as_str()).collect();
                                    let text = if ok.is_empty() { "Serve l'autorizzazione della sindaca".to_string() } else { format!("La sindaca permette di evacuare: {} (trascina qui per dare l'ordine)", ok.join(", ")) };
                                    ui.add(egui::Label::new(RichText::new(text).size(12.0).color(GREY)).wrap());
                                }
                            }
                            Column::Nessuno => {}
                        }
                        rects
                    });
                    if let Some(Dragged(d)) = payload.map(|p| *p) {
                        let at = match which {
                            Column::Difesa(_) => {
                                // the rank under the pointer
                                let y = ui.ctx().pointer_interact_pos().map_or(f32::INFINITY, |p| p.y);
                                Column::Difesa(resp.inner.iter().filter(|r| r.center().y < y).count())
                            }
                            w => w,
                        };
                        drop = Some((d, at));
                    }
                });
            }
        });
        // the districts with no order, and the way out of the defence
        let free: Vec<usize> = (0..n).filter(|&d| plan.rank(d).is_none() && plan.civil[d] == Civil::Nessuno).collect();
        ui.add_space(4.0);
        let frame = egui::Frame::none().fill(Color32::from_gray(24)).stroke(egui::Stroke::new(1.0, Color32::from_gray(60))).rounding(6.0).inner_margin(4.0);
        let (_, payload) = ui.dnd_drop_zone::<Dragged, _>(frame, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(if free.is_empty() { "Senza ordini: nessuno (trascina qui per togliere la difesa)" } else { "Senza ordini (trascina qui per togliere la difesa)" }).size(12.0).color(GREY));
            // whole chips only, in as many rows as they need (playtest: «San Roc…» cut at the edge)
            let per_row = ((ui.available_width() + gap) / (widths[1] + gap)).floor().max(1.0) as usize;
            for row in free.chunks(per_row) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for &d in row {
                        item(ui, d, None, Color32::from_gray(150), widths[1], &mut open);
                    }
                });
            }
        });
        if let Some(Dragged(d)) = payload.map(|p| *p) {
            drop = Some((d, Column::Nessuno));
        }
    });
    if !enabled {
        ui.label(RichText::new("Scegli la difficoltà per cominciare.").size(14.0).color(AMBER));
    }
    if let Some((d, at)) = drop {
        let active = sim.active.civil[d];
        match at {
            Column::Difesa(r) => {
                let was = k.proposed.priorities.iter().position(|&x| x == d);
                k.proposed.priorities.retain(|&x| x != d);
                // dragging down past itself: the ranks above shift up
                let r = if was.is_some_and(|w| w < r) { r - 1 } else { r };
                k.proposed.priorities.insert(r.min(k.proposed.priorities.len()), d);
            }
            Column::Nessuno => k.proposed.priorities.retain(|&x| x != d),
            Column::Preallerta if active < Civil::Preallerta => k.proposed.civil[d] = Civil::Preallerta,
            Column::Evacua if active < Civil::Evacua && sim.may_evacuate(d) => {
                if k.phase == Phase::Esegui && k.begun {
                    k.confirm_evac = Some((d, 0.0));
                } else {
                    k.proposed.civil[d] = Civil::Evacua;
                }
            }
            Column::Evacua if !sim.may_evacuate(d) => {
                k.error = None;
                k.selected = Some(d);
            }
            // orders to the families are not taken back
            _ => {}
        }
        k.dirty = true;
    }
    if let Some(d) = open {
        k.selected = Some(d);
    }
}

/// A label over each ground unit: its name and what it is doing.
fn unit_labels(ctx: &egui::Context, sim: &Sim, cam: (&Camera, &GlobalTransform)) {
    // Units standing together (at the base, on one post) stack their labels.
    let mut shown: Vec<egui::Pos2> = vec![];
    let mut badges: Vec<egui::Rect> = (0..sim.districts.len()).filter_map(|d| ctx.memory(|m| m.area_rect(egui::Id::new(("badge", d))))).collect();
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
        // the label's width, roughly: icon, name, water, then the words
        let label_w = 80.0 + if fire_near { 20.0 } else { 0.0 } + trouble.map_or(0.0, |t| 10.0 + 8.0 * t.1.chars().count() as f32);
        // a label is about 200 px wide and 18 tall
        let below = shown.iter().filter(|p| (p.x - ground.x).abs() < 110.0 && (p.y - ground.y).abs() < 26.0).count();
        shown.push(ground);
        let mut at = ground + egui::vec2(0.0, 28.0 * below as f32);
        // egui keeps an area on screen; do the same here so the panel test
        // below sees where the label will really be drawn
        let sr = ctx.available_rect();
        at.x = at.x.clamp(sr.left() + 60.0, sr.right() - 60.0);
        at.y = at.y.clamp(sr.top() + 24.0, sr.bottom());
        // nor under a badge (playtest 6: A2 and SQ hidden under two of them):
        // just below it, a few times in case that is under the next one
        for _ in 0..3 {
            let label = egui::Rect::from_center_size(at - egui::vec2(0.0, 12.0), egui::vec2(label_w, 24.0));
            let Some(r) = badges.iter().find(|r| r.intersects(label)) else { break };
            at.y = r.bottom() + 26.0;
        }
        // Behind the speaker: just above it; the stem still reaches the unit.
        for name in ["personaggio"] {
            let Some(panel) = ctx.memory(|m| m.area_rect(egui::Id::new(name))) else { continue };
            if panel.expand(4.0).intersects(egui::Rect::from_center_size(at - egui::vec2(0.0, 12.0), egui::vec2(110.0, 24.0))) {
                let lifted = egui::pos2(at.x, panel.top() - 6.0 - 28.0 * below as f32);
                at = lifted;
            }
        }
        // lifted back under a badge (a panel just below it): beside the badge
        for _ in 0..3 {
            let label = egui::Rect::from_center_size(at - egui::vec2(0.0, 12.0), egui::vec2(label_w, 24.0));
            let Some(r) = badges.iter().find(|r| r.intersects(label)) else { break };
            let half = label_w * 0.5 + 4.0;
            at.x = if r.left() - half > sr.left() + 60.0 { r.left() - half } else { r.right() + half };
        }
        badges.push(egui::Rect::from_center_size(at - egui::vec2(0.0, 12.0), egui::vec2(label_w - 8.0, 22.0)));
        let colour = trouble.map_or(if fire_near { ORANGE } else { GREEN }, |t| t.0);
        // Always a stem from the label down to a dot where the unit is, in
        // its colour (playtest colleghi: labels "flew in the void", and the
        // white line drawn only when a label was lifted said nothing).
        // Not for a unit off the map (behind the side bar): its label stays
        // at the map's edge, without a stem across the bar.
        if let Some(foot) = screen(cam, sim, u.pos, 0.0).filter(|f| sr.contains(*f)) {
            let guide = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("guide")));
            if (foot - at).length() > 8.0 {
                guide.line_segment([at, foot], egui::Stroke::new(2.0, colour.gamma_multiply(0.8)));
            }
            guide.circle_filled(foot, 5.0, colour);
            guide.circle_stroke(foot, 5.0, egui::Stroke::new(1.5, Color32::BLACK));
        }
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
    patrol_labels(ctx, sim, cam, &badges);
}

/// The messenger patrols' labels: a bell and «P1», or how many when several
/// knock in the same place, on a stem down to the cars, as the units'; none
/// while a patrol is parked at its station (`units::update_teams` hides it
/// there too).
fn patrol_labels(ctx: &egui::Context, sim: &Sim, cam: (&Camera, &GlobalTransform), taken: &[egui::Rect]) {
    use rocca::messengers::Task as T;
    let sr = ctx.available_rect();
    let mut taken = taken.to_vec();
    // (where on screen, who, any lost), patrols within 40 px together
    let mut groups: Vec<(egui::Pos2, Vec<usize>, bool)> = vec![];
    for (i, t) in sim.teams.iter().enumerate() {
        let parked = matches!(t.task, T::Idle { .. }) && (t.pos.x - t.base.x).hypot(t.pos.y - t.base.y) < 40.0;
        if parked {
            continue;
        }
        let Some(foot) = screen(cam, sim, t.pos, 0.0).filter(|f| sr.contains(*f)) else { continue };
        let lost = t.task == T::Lost;
        match groups.iter_mut().find(|g| g.2 == lost && (g.0 - foot).length() < 40.0) {
            Some(g) => g.1.push(i),
            None => groups.push((foot, vec![i], lost)),
        }
    }
    for (foot, who, lost) in groups {
        let colour = if lost { RED } else { PATROL };
        let mut at = foot - egui::vec2(0.0, 34.0);
        // not on a badge or another label: a little higher, a few times
        for _ in 0..4 {
            let label = egui::Rect::from_center_size(at - egui::vec2(0.0, 12.0), egui::vec2(64.0, 24.0));
            let Some(r) = taken.iter().find(|r| r.intersects(label)) else { break };
            at.y = r.top() - 4.0;
        }
        taken.push(egui::Rect::from_center_size(at - egui::vec2(0.0, 12.0), egui::vec2(60.0, 22.0)));
        let guide = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("guide")));
        guide.line_segment([at, foot], egui::Stroke::new(2.0, colour.gamma_multiply(0.8)));
        guide.circle_filled(foot, 5.0, colour);
        guide.circle_stroke(foot, 5.0, egui::Stroke::new(1.5, Color32::BLACK));
        let text = if who.len() == 1 { format!("P{}", who[0] + 1) } else { format!("×{}", who.len()) };
        egui::Area::new(egui::Id::new(("pattuglia", who[0]))).fixed_pos(at).pivot(Align2::CENTER_BOTTOM).interactable(false).order(egui::Order::Background).show(ctx, |ui| {
            egui::Frame::none().fill(Color32::from_rgb(14, 16, 18)).stroke(egui::Stroke::new(2.0, colour)).rounding(6.0).inner_margin(egui::Margin::symmetric(5.0, 2.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    icons::show(ui, Icon::Bell, 18.0, colour);
                    ui.add(egui::Label::new(RichText::new(text).size(15.0).strong().color(Color32::WHITE)).extend());
                });
            });
        });
    }
}

/// «I mezzi»: what the units are doing now (not the reasons given when they
/// were sent: those are in the log), and the districts left without units.
fn proposal(ui: &mut egui::Ui, k: &Kiosk, sim: &Sim) {
    let Some(p) = &k.preview else { return };
    egui::CollapsingHeader::new(RichText::new("I mezzi").size(16.0).strong().color(Color32::WHITE)).id_source("proposta_h").default_open(false).show(ui, |ui| {
        ui.label(RichText::new(format!("I mezzi adesso ({})", clock(sim.time_s()))).size(16.0).strong().color(Color32::WHITE));
        for (i, u) in sim.crews.units.iter().enumerate().filter(|(_, u)| !u.kind.is_air()) {
            ui.label(RichText::new(format!("{}: {}", u.callsign, sim.unit_status(i))).size(15.0).color(GREY));
        }
        for (_, why) in &p.uncovered {
            ui.label(RichText::new(format!("Senza mezzi: {why}")).size(15.0).color(ORANGE));
        }
    });
}

/// The time's button and the evacuation's confirmation (user, 2026-10-13:
/// no pauses, orders apply as given). «Avvia» confirms the first plan; while
/// the time runs, an «Evacua» pressed on a chip shows its way out on the map
/// at ×1 and waits here for «Conferma» or «Annulla», [`super::CONFIRM_EVAC_S`]
/// real seconds at most.
fn time_button(ui: &mut egui::Ui, k: &mut Kiosk, sim: &mut Sim) {
    if let Some((d, t)) = k.confirm_evac {
        confirm_evacuation(ui, k, sim, d, t);
        return;
    }
    if k.phase != Phase::Pianifica {
        if k.phase == Phase::Esegui {
            ui.label(RichText::new("Gli ordini valgono subito: tocca un paese sulla mappa per cambiarli.").size(15.0).color(GREY));
        }
        return;
    }
    ui.vertical_centered(|ui| {
        if big_button(ui, Some(Icon::Play), "Avvia", AMBER).clicked() {
            commit(k, sim);
            k.begun = true;
            k.enter(Phase::Esegui);
        }
    });
}

/// «Evacua X?»: where the families would go and how long it would take, the
/// way out drawn on the map ([`evacuation_routes`]), and the countdown.
fn confirm_evacuation(ui: &mut egui::Ui, k: &mut Kiosk, sim: &mut Sim, d: usize, t: f32) {
    let routes = sim.evacuation_routes(d);
    let name = sim.districts[d].name.clone();
    let left = (super::CONFIRM_EVAC_S - t).max(0.0).ceil();
    let (mut yes, mut no) = (false, false);
    egui::Frame::none().fill(Color32::from_rgb(12, 34, 60)).stroke(egui::Stroke::new(3.0, BLUE)).rounding(10.0).inner_margin(12.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            icons::show(ui, Icon::Exit, 24.0, BLUE);
            ui.label(RichText::new(format!("Evacuare {name}?")).size(21.0).strong().color(Color32::WHITE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{left:.0} s")).size(20.0).strong().color(BLUE));
            });
        });
        let to = if routes.to.is_empty() { "nessuna area sicura raggiungibile".to_string() } else { routes.to.join(" e ") };
        let mut text = format!("Le famiglie in casa andrebbero verso {to}, lungo le strade in blu sulla mappa");
        if let Some((a, b)) = routes.minutes {
            text += &format!(": circa {a} min di viaggio, i più lontani {b}");
        }
        text += ".";
        if routes.no_way > 0 {
            text += &format!(" {} famiglie non hanno una strada libera.", routes.no_way);
        }
        if let Some(r) = sim.agents.exit_capacity() {
            text += &format!(" Escono al massimo {r:.0} famiglie al minuto da tutta l'isola.");
        }
        ui.add(egui::Label::new(RichText::new(text).size(15.0).color(Color32::WHITE)).wrap());
        ui.label(RichText::new("Il tempo è rallentato. Allo scadere l'ordine non parte.").size(14.0).color(GREY));
        ui.add_space(6.0);
        yes = big_button(ui, Some(Icon::Check), "Conferma evacuazione", BLUE).clicked();
        ui.add_space(4.0);
        no = ui.add_sized([ui.available_width(), 36.0], secondary("Annulla")).clicked();
    });
    if yes {
        k.proposed = sim.active.clone().with_civil(d, Civil::Evacua);
        commit(k, sim);
        k.confirm_evac = None;
    } else if no {
        k.confirm_evac = None;
    }
}

/// The ways out of the districts about to be evacuated, on the map: the one
/// waiting for its confirmation, and in a crisis those the proposed plan
/// would evacuate.
fn evacuation_routes(ctx: &egui::Context, k: &Kiosk, sim: &Sim, cam: (&Camera, &GlobalTransform)) {
    let mut ds: Vec<usize> = k.confirm_evac.map(|(d, _)| d).into_iter().collect();
    if k.phase == Phase::Crisi {
        ds.extend((0..sim.districts.len()).filter(|&d| k.proposed.civil[d] == Civil::Evacua && sim.active.civil[d] != Civil::Evacua));
    }
    if ds.is_empty() {
        return;
    }
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("vie_di_fuga")));
    // a moving dash says which way they go
    let phase = (ctx.input(|i| i.time) as f32 * 60.0) % 40.0;
    for d in ds {
        let routes = sim.evacuation_routes(d);
        let mut end = None;
        for path in &routes.paths {
            // draped: a point every ~30 m, lifted over the ground
            let mut pts = vec![];
            for w in path.windows(2) {
                let (a, b) = (w[0], w[1]);
                let n = ((((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt() / 30.0).ceil() as usize).max(1);
                for s in 0..n {
                    let f = s as f32 / n as f32;
                    pts.push(scenario::Pos { x: a.x + (b.x - a.x) * f, y: a.y + (b.y - a.y) * f });
                }
            }
            pts.extend(path.last().copied());
            end = path.last().copied();
            let screen: Vec<egui::Pos2> = pts.iter().filter_map(|&q| screen(cam, sim, q, 6.0)).collect();
            if screen.len() < 2 {
                continue;
            }
            p.add(egui::Shape::line(screen.clone(), egui::Stroke::new(7.0, Color32::from_rgba_unmultiplied(10, 20, 40, 160))));
            p.add(egui::Shape::line(screen.clone(), egui::Stroke::new(4.0, BLUE)));
            // dashes along the way, moving toward the end
            let mut run = -phase;
            for w in screen.windows(2) {
                let seg = w[1] - w[0];
                let len = seg.length();
                let mut at = run.rem_euclid(40.0);
                while at < len {
                    let a = w[0] + seg * (at / len);
                    let b = w[0] + seg * ((at + 10.0).min(len) / len);
                    p.line_segment([a, b], egui::Stroke::new(3.0, Color32::WHITE));
                    at += 40.0;
                }
                run -= len;
            }
        }
        if let Some(c) = end.and_then(|e| screen(cam, sim, e, 6.0)) {
            p.circle_filled(c, 12.0, BLUE);
            icons::draw(&p, egui::Rect::from_center_size(c, egui::vec2(16.0, 16.0)), Icon::Exit, Color32::WHITE);
        }
    }
}

/// A wide button with an optional icon before the word.
fn big_button(ui: &mut egui::Ui, icon: Option<Icon>, label: &str, fill: Color32) -> egui::Response {
    let size = egui::vec2(ui.available_width(), 58.0);
    let (r, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = ui.painter();
    p.rect_filled(r, 10.0, if resp.hovered() { fill.gamma_multiply(0.85) } else { fill });
    let icon_w = if icon.is_some() { 40.0 } else { 0.0 };
    // a long label shrinks to fit
    let mut g = p.layout_no_wrap(label.to_string(), egui::FontId::proportional(26.0), Color32::BLACK);
    if g.size().x + icon_w > size.x - 24.0 {
        let fit = 26.0 * (size.x - 24.0 - icon_w) / g.size().x;
        g = p.layout_no_wrap(label.to_string(), egui::FontId::proportional(fit.max(14.0)), Color32::BLACK);
    }
    let x = r.center().x - (g.size().x + icon_w) / 2.0;
    if let Some(i) = icon {
        icons::draw(p, egui::Rect::from_min_size(egui::pos2(x, r.center().y - 15.0), egui::vec2(30.0, 30.0)), i, Color32::BLACK);
    }
    p.galley(egui::pos2(x + icon_w, r.center().y - g.size().y / 2.0), g, Color32::BLACK);
    if resp.hovered() {
        p.rect_stroke(r.expand(2.0), 11.0, egui::Stroke::new(3.0, Color32::WHITE));
    }
    crate::playtest::control(label, &resp, true);
    icons::pointing(resp)
}

/// Simulated seconds the start of the fire stays marked on the map.
const FIRE_MARK_S: i64 = 30 * 60;

/// Where the fire started, ringed and named, until it has grown enough to
/// be seen (playtests 5 and 6: at the start the fire was a few red pixels,
/// often under a panel).
fn fire_marker(ctx: &egui::Context, sim: &Sim, cam: (&Camera, &GlobalTransform)) {
    if sim.time_s() > FIRE_MARK_S {
        return;
    }
    let Some(p) = screen(cam, sim, sim.case.ignition(), 0.0) else { return };
    if !ctx.available_rect().contains(p) {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("innesco")));
    let t = ctx.input(|i| i.time) as f32;
    // a ring that keeps widening, as on a radar, round a steady one
    let grow = (t * 0.8).fract();
    painter.circle_stroke(p, 22.0 + 30.0 * grow, egui::Stroke::new(4.0, Color32::from_rgba_unmultiplied(255, 120, 40, (230.0 * (1.0 - grow)) as u8)));
    painter.circle_stroke(p, 20.0, egui::Stroke::new(3.0, ORANGE));
    let galley = painter.layout_no_wrap("Il fuoco è partito qui".into(), egui::FontId::proportional(16.0), Color32::BLACK);
    let at = p + egui::vec2(-galley.size().x * 0.5, -galley.size().y - 30.0);
    painter.rect_filled(egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(8.0, 4.0)), 6.0, ORANGE);
    painter.galley(at, galley, Color32::BLACK);
    ctx.request_repaint();
}


/// In a decision moment the time slows but does not stop: an orange frame
/// round the map, like a slow-motion shot.
fn slow_motion_frame(ctx: &egui::Context, map: egui::Rect) {
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("rallentatore")));
    for (w, a) in [(18.0, 60u8), (8.0, 120)] {
        p.rect_stroke(map.shrink(w * 0.5), 0.0, egui::Stroke::new(w, Color32::from_rgba_unmultiplied(255, 140, 30, a)));
    }
}

/// Recent updates stay visible; the full history remains available.
fn events(ui: &mut egui::Ui, sim: &Sim) {
    {
        heading(ui, "Ultimi aggiornamenti");
        if sim.log.is_empty() {
            ui.label(RichText::new("In attesa del primo piano").color(GREY));
        }
        let show = |ui: &mut egui::Ui, e: &rocca::game::LogEntry| {
            let (icon, colour) = if e.text.starts_with("CRISI") || e.text.contains("fuori servizio") || e.text.contains("si ritira") {
                (Icon::Alert, ORANGE)
            } else if e.text.contains("nessun mezzo") || e.text.contains("mezzi insufficienti") || e.text.contains("non possono avvicinarsi") {
                (Icon::Engine, ORANGE)
            } else if e.text.starts_with("evacuazione") || e.text.starts_with("la sindaca autorizza") { (Icon::Exit, BLUE) }
            else if e.text.starts_with("preallerta") { (Icon::Bell, AMBER) }
            else { (Icon::Engine, GREEN) };
            ui.horizontal_top(|ui| {
                icons::show(ui, icon, 20.0, colour);
                ui.vertical(|ui| {
                    ui.label(RichText::new(clock(e.at_s)).size(12.0).color(colour));
                    ui.add(egui::Label::new(RichText::new(&e.text).size(14.0).color(Color32::WHITE)).wrap());
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
    }
}

/// Width of the side bar during the game.
const SIDE_W: f32 = 400.0;

/// A section's title in the side bar.
fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(19.0).strong().color(Color32::WHITE));
}

/// Remember where a part of the side bar was drawn, for the tour.
fn mark(ui: &egui::Ui, name: &str, r: egui::Rect) {
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("sezione", name)), r));
}

/// Where a part of the side bar («piano», «azione») was drawn last frame.
pub(super) fn section_rect(ctx: &egui::Context, name: &str) -> Option<egui::Rect> {
    ctx.data(|d| d.get_temp(egui::Id::new(("sezione", name))))
}

/// The state of the game as a coloured strip: a decision moment (amber),
/// running (green), the mayor's authorisation (orange, blinking in the last
/// seconds), stopped by the operator (grey).
fn pace(ui: &mut egui::Ui, k: &Kiosk, sim: &Sim) {
    let left = (k.crisis_s - k.phase_t).max(0.0).ceil();
    let (icon, colour, text) = match k.phase {
        _ if k.frozen => (Icon::Pause, GREY, "Fermato dall'operatore".to_string()),
        Phase::Pianifica if !k.started => (Icon::Pause, AMBER, "Il fuoco aspetta: scegli la difficoltà.".to_string()),
        Phase::Pianifica => (Icon::Pause, AMBER, "Tempo rallentato ×1: prepara gli ordini, poi premi «Avvia».".to_string()),
        Phase::Esegui if k.confirm_evac.is_some() => (Icon::Exit, BLUE, "Tempo rallentato ×1: conferma l'evacuazione".to_string()),
        Phase::Esegui if sim.speed > k.speed => (Icon::Fast, GREEN, format!("Il tempo scorre · non succede nulla, ×{:.0}", sim.speed)),
        Phase::Esegui => (Icon::Play, GREEN, format!("Il tempo scorre · ×{:.0}", if sim.speed > 0.0 { sim.speed } else { k.speed })),
        Phase::Crisi => (Icon::Alert, ORANGE, format!("LA SINDACA ASPETTA · {left:.0} s · tempo rallentato")),
        Phase::Fine => (Icon::Check, GREY, "Concluso".to_string()),
    };
    // blinking in the last seconds of a crisis (playtest 6: one ran out unseen)
    let loud = k.phase == Phase::Crisi && (left > 8.0 || (ui.input(|i| i.time) * 2.0).fract() < 0.6);
    let (fill, ink) = if loud { (ORANGE, Color32::BLACK) } else { (colour.gamma_multiply(0.15), colour) };
    egui::Frame::none().fill(fill).stroke(egui::Stroke::new(1.5, colour)).rounding(8.0).inner_margin(egui::Margin::symmetric(10.0, 6.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            icons::show(ui, icon, 24.0, ink);
            ui.add(egui::Label::new(RichText::new(text).size(16.0).strong().color(ink)).wrap());
        });
    });
}

/// Level, wind, the clock with the hours as a bar, and the pace.
fn status(ui: &mut egui::Ui, k: &mut Kiosk, sim: &Sim, cam: (&Camera, &GlobalTransform)) {
    ui.horizontal(|ui| {
        if let Some(l) = k.level.and_then(|i| k.territory.levels.get(i)) {
            pill(ui, &l.name, Color32::from_gray(200));
            ui.add_space(6.0);
        }
        let w = sim.fire.weather();
        wind_arrow(ui, sim, cam);
        ui.label(RichText::new(format!("vento da {} {:.0} km/h", rocca::words::compass(w.wind_dir_deg), w.wind_speed_kmh)).size(19.0).strong().color(Color32::WHITE));
    });
    // the arrow points where the fire is pushed, not where the wind comes
    // from (playtest: «da sud-est» and an arrow to the north-west)
    ui.label(RichText::new(format!("la freccia: spinge il fuoco verso {}", rocca::words::compass((sim.fire.weather().wind_dir_deg + 180.0) % 360.0))).size(13.0).color(GREY));
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("ore {}", clock(sim.time_s()))).size(26.0).strong().color(Color32::WHITE).monospace());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("fine {}", clock(sim.case.duration_s()))).size(15.0).color(GREY));
        });
    });
    let frac = (sim.time_s() as f32 / sim.case.duration_s() as f32).clamp(0.0, 1.0);
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 10.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 4.0, Color32::from_gray(60));
    ui.painter().rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(r.width() * frac, r.height())), 4.0, ORANGE);
    ui.add_space(8.0);
    // the game chooses the speed (spec §2, 2026-10-13); the operator can
    // change it from the menu
    pace(ui, k, sim);
}

/// During the game, everything but the map, top to bottom (user,
/// 2026-10-12): the state, the time's button or the crisis's choice, the
/// plan, what the coordinator proposes, the latest events and the legend;
/// the view buttons at the foot. Only the opened chip and the speaker stay
/// on the map.
fn sidebar(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, cam: (&Camera, &GlobalTransform)) {
    let frame = egui::Frame::none().fill(Color32::from_rgb(18, 20, 24)).inner_margin(egui::Margin::symmetric(16.0, 14.0));
    egui::SidePanel::right("barra_laterale").exact_width(SIDE_W).resizable(false).show_separator_line(false).frame(frame).show(ctx, |ui| {
        status(ui, k, sim, cam);
        ui.add_space(10.0);
        let r = ui.scope(|ui| if k.phase == Phase::Crisi { crisis_choice(ui, k, sim) } else { time_button(ui, k, sim) }).response.rect;
        mark(ui, "azione", r);
        ui.add_space(6.0);
        ui.separator();
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 40.0), egui::Layout::left_to_right(egui::Align::Center), |ui| abort_button(ui, k));
            ui.separator();
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let r = ui.scope(|ui| plan(ui, k, sim)).response.rect;
                    mark(ui, "piano", r);
                    ui.add_space(6.0);
                    ui.separator();
                    proposal(ui, k, sim);
                    ui.separator();
                    events(ui, sim);
                    ui.separator();
                });
            });
        });
    });
}

/// «Abbandona la partita» at the foot of the side bar: asks once more, then
/// goes back to the title screen (`Kiosk::leave`).
fn abort_button(ui: &mut egui::Ui, k: &mut Kiosk) {
    let now = ui.input(|i| i.time);
    if k.abort_armed.is_some_and(|t| now - t > 6.0) {
        k.abort_armed = None;
    }
    {
        if k.abort_armed.is_none() {
            if icons::pointing(ui.add(secondary("Abbandona la partita"))).clicked() {
                k.abort_armed = Some(now);
            }
        } else {
            ui.label(RichText::new("Abbandoni?").size(16.0).color(Color32::WHITE));
            let yes = egui::Button::new(RichText::new("Sì, esci").size(18.0).strong().color(Color32::WHITE)).fill(RED.gamma_multiply(0.8)).rounding(8.0);
            if icons::pointing(ui.add(yes)).clicked() {
                k.abort_armed = None;
                k.leave = true;
            }
            if icons::pointing(ui.add(secondary("No"))).clicked() {
                k.abort_armed = None;
            }
        }
    }
}

/// What the mayor says first: the result in a sentence, against no orders.
fn verdict(name: &str, o: &rocca::Outcome, b: Option<&rocca::Outcome>) -> (Mood, String) {
    let Some(b) = b else {
        return (Mood::Calmo, "Sto confrontando la tua partita con lo stesso incendio senza ordini…".into());
    };
    let caught = o.caught();
    let saved = b.caught().saturating_sub(caught);
    let (mood, text) = if caught == 0 && saved > 0 {
        (Mood::Calmo, format!("Nessuna famiglia è stata colta in casa: senza i tuoi ordini ce ne sarebbero state {}. Ottimo lavoro!", b.caught()))
    } else if caught == 0 {
        (Mood::Calmo, "Nessuna famiglia è rimasta in pericolo. Ben fatto!".to_string())
    } else if saved > 0 {
        let f = if saved == 1 { "una famiglia".to_string() } else { format!("{saved} famiglie") };
        (Mood::Preoccupato, format!("Hai messo in salvo {f} in più rispetto a nessun ordine, ma il fuoco ne ha raggiunte {caught} ancora in casa."))
    } else {
        (Mood::Preoccupato, format!("Il fuoco ha raggiunto {caught} famiglie ancora in casa. Quando conviene dare l'ordine di evacuare?"))
    };
    (mood, characters::to(name, &text))
}

/// One star, filled or empty, `size` across.
fn star(ui: &mut egui::Ui, filled: bool, size: f32) {
    star_in(ui, if filled { AMBER } else { Color32::from_gray(70) }, size);
}

/// A star of the given colour.
fn star_in(ui: &mut egui::Ui, fill: Color32, size: f32) {
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

/// The colour that tells the levels apart on the board.
fn level_colour(name: &str) -> Color32 {
    match name {
        "Facile" => GREEN,
        "Medio" => AMBER,
        "Esperto" => RED,
        _ => Color32::from_gray(200),
    }
}

/// One level of today's board (`rocca::score::Board`): the best `rows`,
/// the player's own row (`mine`: name and time stamp) highlighted, and
/// appended with its place if it ranks below the rows shown.
fn board_rows(ui: &mut egui::Ui, board: &rocca::score::Board, level: &str, day: &str, mine: Option<(&str, u64)>, rows: usize) {
    let ranking = board.ranking(level, day);
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
        let colour = if me { Color32::BLACK } else { Color32::WHITE };
        egui::Frame::none().fill(if me { AMBER } else { Color32::TRANSPARENT }).rounding(6.0).inner_margin(egui::Margin::symmetric(6.0, 1.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add_sized([22.0, 20.0], egui::Label::new(RichText::new(format!("{}", i + 1)).size(15.0).strong().color(if me { colour } else { GREY })));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(e.total().to_string()).size(16.0).strong().monospace().color(colour));
                    star_in(ui, if me { Color32::BLACK } else { AMBER }, 15.0);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(RichText::new(&e.name).size(16.0).strong().color(colour)).truncate());
                    });
                });
            });
        });
    }
}

/// «1º», «2º»…: the place in words for the end screen.
fn place_words(place: usize, of: usize) -> String {
    match place {
        0 if of == 1 => "Sei il primo di oggi".to_string(),
        0 => format!("Sei al 1º posto su {of} oggi"),
        p => format!("Sei al {}º posto su {of} oggi", p + 1),
    }
}

/// The four star rows, big: the category and its stars, nothing else.
fn big_stars(ui: &mut egui::Ui, st: &rocca::score::Stars, size: f32) {
    // what each row counts, beside it (playtest: «Tempismo» and «Misura» said nothing)
    for (name, n, what) in [
        ("Persone", st.persone, "famiglie che il fuoco non ha colto in casa"),
        ("Case", st.case, "case che il fuoco non ha colpito"),
        ("Tempismo", st.tempismo, "quanto prima del fuoco hai avvisato"),
        ("Misura", Some(st.misura), "nessuno in strada senza bisogno"),
    ] {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(130.0, size), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(130.0);
                ui.label(RichText::new(name).size(22.0).strong().color(Color32::WHITE));
            });
            match n {
                Some(n) => {
                    for i in 0..5 {
                        star(ui, i < n as usize, size);
                    }
                }
                None => {
                    ui.label(RichText::new("non conta: il fuoco non è arrivato alle case").size(15.0).color(GREY));
                }
            }
            if n.is_some() {
                ui.add_space(8.0);
                ui.label(RichText::new(what).size(14.0).color(GREY));
            }
        });
        ui.add_space(4.0);
    }
}

/// The end (spec §5, 2026-10-13): the four stars, big; the place in today's
/// ranking of the level, with the ranking beside it; three short lines on
/// how it went, all from facts the game already gives.
fn debrief(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>, _faces: Option<&Portraits>) {
    let o = sim.outcome();
    let base = k.baseline();
    let mut again = None;
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("fine_fondo")).fixed_pos(screen.min).order(egui::Order::Middle).interactable(false).show(ctx, |ui| {
        ui.painter().rect_filled(screen, 0.0, Color32::from_rgba_premultiplied(8, 10, 14, 200));
    });
    // the stars, from the same facts as ever; the game joins the board once
    let b = base.as_ref().and_then(|r| r.as_ref().ok());
    let scored = b.map(|b| {
        let s = rocca::score::score(&o, b);
        (s, rocca::score::stars(&s, sim.timing(), sim.measure()))
    });
    let level = k.level.and_then(|i| k.territory.levels.get(i)).map_or_else(|| sim.case.name.clone(), |l| l.name.clone());
    let name = if k.player.is_empty() { "Anonimo".to_string() } else { k.player.clone() };
    if let (Some((_, st)), false) = (&scored, k.saved) {
        let (day, at) = board::now();
        k.board.push(rocca::score::Played { name: name.clone(), level: level.clone(), stars: st.all(), day: day.clone(), at });
        k.saved = true;
        k.saved_at = (day, at);
        if let Err(e) = board::save(&k.board) {
            k.error = Some(e);
        }
    }
    let (day, at) = k.saved_at.clone();
    // three short lines: the sentence on the families, the weakest of the
    // stars (the best, if none is weak) with its reason, one order review
    let mut lines: Vec<(Color32, String)> = Vec::new();
    if let Some((s, st)) = &scored {
        lines.push((Color32::WHITE, verdict(&k.player, &o, b).1));
        let fam = |n: usize| if n == 1 { "1 famiglia".to_string() } else { format!("{n} famiglie") };
        let needless = sim.needless_evacuations();
        let rows = [
            ("Persone", st.persone, if s.families_at_risk > 0 { format!("senza ordini il fuoco avrebbe colto in casa {}: ne hai salvate {}.", fam(s.families_at_risk), s.families) } else { "nessuna famiglia era in pericolo in casa.".to_string() }),
            ("Case", st.case, if s.homes_at_risk > 0 { format!("senza difesa il fuoco avrebbe colpito {} case: ne hai risparmiate {}.", s.homes_at_risk, s.homes) } else { "nessuna casa era in pericolo.".to_string() }),
            ("Tempismo", st.tempismo, "quanto prima del fuoco hai avvisato i paesi che ha raggiunto.".to_string()),
            ("Misura", Some(st.misura), if needless == 0 { "nessuna famiglia in strada senza che il fuoco arrivasse.".to_string() } else { format!("{} in strada da paesi dove il fuoco non è arrivato alle case.", fam(needless)) }),
        ];
        let weakest = rows.iter().filter_map(|(n, s, why)| s.map(|s| (s, n, why))).min_by_key(|(s, _, _)| *s);
        if let Some((s, n, why)) = weakest {
            lines.push((if s <= 2 { AMBER } else { GREEN }, format!("{n}: {why}")));
        }
        let worst = (0..sim.districts.len()).map(|d| sim.order_review(d)).min_by_key(|r| match r.verdict {
            rocca::Verdict::Male => 0,
            rocca::Verdict::Forse => 1,
            rocca::Verdict::Bene => 2,
        });
        if let Some(r) = worst {
            let colour = match r.verdict {
                rocca::Verdict::Male => RED,
                rocca::Verdict::Forse => AMBER,
                rocca::Verdict::Bene => GREEN,
            };
            lines.push((colour, r.text));
        }
    }
    let side = 340.0;
    let inner = (screen.width() - 48.0 - 72.0).min(1100.0);
    let left = (inner - side - 24.0).max(420.0);
    let height = screen.height() - 48.0;
    egui::Area::new(egui::Id::new("fine")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::none()
            .fill(Color32::from_rgb(18, 20, 24))
            .stroke(egui::Stroke::new(1.5, AMBER.gamma_multiply(0.55)))
            .shadow(egui::epaint::Shadow { offset: egui::vec2(0.0, 10.0), blur: 40.0, spread: 4.0, color: Color32::from_black_alpha(160) })
            .rounding(18.0)
            .inner_margin(egui::Margin { left: 36.0, right: 36.0, top: 24.0, bottom: 22.0 })
            .show(ui, |ui| {
                ui.set_width(inner);
                ui.horizontal_top(|ui| {
                    // the stars and the three lines
                    ui.allocate_ui_with_layout(egui::vec2(left, height - 48.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(left);
                        let title = if k.player.is_empty() { "Com'è andata".to_string() } else { format!("Com'è andata, {}", k.player) };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(title).size(34.0).strong().color(Color32::WHITE));
                            ui.add_space(8.0);
                            pill(ui, &level, level_colour(&level));
                        });
                        ui.add_space(14.0);
                        match &scored {
                            Some((_, st)) => big_stars(ui, st, 34.0),
                            None => {
                                ui.label(RichText::new("Stelle: calcolo…").size(20.0).color(GREY));
                            }
                        }
                        ui.add_space(10.0);
                        for (colour, text) in &lines {
                            ui.horizontal_top(|ui| {
                                let (dot, _) = ui.allocate_exact_size(egui::vec2(20.0, 22.0), egui::Sense::hover());
                                ui.painter().circle_filled(dot.center(), 5.0, *colour);
                                ui.add(egui::Label::new(RichText::new(text).size(16.0).color(Color32::WHITE)).wrap());
                            });
                            ui.add_space(4.0);
                        }
                    });
                    ui.add_space(24.0);
                    // today's ranking of the level, the player's row lit
                    ui.vertical(|ui| {
                        ui.set_width(side);
                        card(level_colour(&level)).show(ui, |ui| {
                            ui.set_width(side - 28.0);
                            if k.saved {
                                let place = k.board.place(&level, &day, &name, at);
                                let of = k.board.count(&level, &day);
                                if let Some(p) = place {
                                    ui.add(egui::Label::new(RichText::new(place_words(p, of)).size(22.0).strong().color(Color32::WHITE)).wrap());
                                }
                            }
                            ui.label(RichText::new(format!("Classifica di oggi · {level}")).size(14.0).color(GREY));
                            ui.add_space(6.0);
                            board_rows(ui, &k.board, &level, &day, Some((name.as_str(), at)).filter(|_| k.saved), rocca::score::BOARD_SHOWN);
                        });
                        ui.add_space(12.0);
                        // a real-world note, not advice on the game
                        ui.add(egui::Label::new(RichText::new(REAL_LIFE).size(13.0).italics().color(AMBER)).wrap());
                        ui.add_space(10.0);
                        let big = |t: &str| egui::Button::new(RichText::new(t).size(26.0).strong().color(Color32::BLACK)).fill(AMBER).rounding(12.0).min_size(egui::vec2(side, 64.0));
                        // one way on (playtest colleghi): «Riprova» goes back to
                        // the levels, the name kept; without levels, the same fire
                        let retry = ui.add_sized([side, 64.0], big("Riprova"));
                        crate::playtest::control("Riprova", &retry, true);
                        if icons::pointing(retry).clicked() {
                            if k.territory.levels.is_empty() {
                                again = Some(k.case.clone());
                            } else {
                                k.intro = true;
                            }
                        }
                        if k.territory.levels.is_empty() && icons::pointing(ui.add_sized([side, 64.0], big("Altro incendio"))).clicked() {
                            again = Some(k.next_case());
                        }
                        // for the next visitor: the title screen, the name forgotten
                        if icons::pointing(ui.add_sized([side, 44.0], secondary("Schermata iniziale"))).clicked() {
                            k.leave = true;
                        }
                    });
                });
            });
    });
    if let Some(case) = again {
        k.case = case;
        new_game(k, sim, restarted);
    }
}

/// A rounded card tinted with `tint`, as the levels on the first screen.
fn card(tint: Color32) -> egui::Frame {
    egui::Frame::none().fill(tint.gamma_multiply(0.12)).stroke(egui::Stroke::new(1.5, tint.gamma_multiply(0.8))).rounding(14.0).inner_margin(14.0)
}

/// The operator bar (F2): any case, seed and speed, a new game, a pause.
fn operator(ctx: &egui::Context, k: &mut Kiosk, sim: &mut Sim, restarted: &mut EventWriter<SimRestarted>, gfx: &mut crate::quality::Graphics) {
    let now = ctx.input(|i| i.time);
    if k.reset_armed.is_some_and(|t| now - t > 5.0) {
        k.reset_armed = None;
    }
    if k.board_reset_armed.is_some_and(|t| now - t > 5.0) {
        k.board_reset_armed = None;
    }
    egui::Area::new(egui::Id::new("operatore")).anchor(Align2::LEFT_TOP, [10.0, 56.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        panel().show(ui, |ui| {
            ui.label(RichText::new("Menu (F2 per chiudere)").strong().color(Color32::WHITE));
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
                ui.checkbox(&mut k.tour_on, "Tutorial a inizio partita");
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
                        // a new visitor: the title screen, a new name; the
                        // case picked here is the one «Gioca» starts
                        to_title(k, sim, restarted);
                    }
                }
                if ui.add(secondary("Vista iniziale (Home)")).clicked() {
                    k.reset_view = true;
                    k.follow = false;
                }
                // the one way the world stands still (spec §2)
                if k.phase != Phase::Fine && ui.add(secondary(if k.frozen { "Riprendi il mondo" } else { "Ferma il mondo" })).clicked() {
                    k.frozen = !k.frozen;
                }
            });
            ui.horizontal(|ui| {
                ui.label("velocità ×");
                ui.add(egui::DragValue::new(&mut k.speed).range(1.0..=300.0));
                ui.label("secondi per la sindaca");
                for s in [30.0, 45.0, 60.0] {
                    ui.selectable_value(&mut k.crisis_s, s, format!("{s:.0}"));
                }
            });
            ui.horizontal(|ui| {
                // the board is emptied only on a second press
                let label = if k.board_reset_armed.is_some() { "Sicuro? Premi di nuovo: la classifica si cancella" } else { "Azzera classifica" };
                if ui.add(secondary(label)).clicked() {
                    if k.board_reset_armed.is_none() {
                        k.board_reset_armed = Some(now);
                    } else {
                        k.board_reset_armed = None;
                        k.board = rocca::score::Board::default();
                        if let Err(e) = board::save(&k.board) {
                            k.error = Some(e);
                        }
                    }
                }
                ui.label(RichText::new(format!("{} partite oggi", { let day = board::now().0; k.board.entries.iter().filter(|e| e.day == day).count() })).color(GREY));
            });
            ui.horizontal(|ui| {
                use crate::quality::Graphics;
                ui.label("Grafica:");
                let now_level = gfx.level();
                for (name, label, preset) in [("alta", "Alta", Graphics::ALTA), ("media", "Media", Graphics::MEDIA), ("bassa", "Bassa", Graphics::BASSA)] {
                    if ui.selectable_label(now_level == Some(name), label).clicked() && now_level != Some(name) {
                        // the plant density waits for the next launch
                        *gfx = Graphics { vegetation: gfx.vegetation, ..preset };
                        if let Err(e) = crate::quality::save(name) {
                            k.error = Some(e);
                        }
                    }
                }
                if now_level.is_none() {
                    ui.label(RichText::new("personalizzata").color(GREY));
                }
                ui.label(RichText::new("(la densità del bosco cambia al riavvio)").color(GREY));
            });
            if let Some(e) = &k.error {
                ui.colored_label(RED, e);
            }
        });
    });
}
