//! The kiosk screens, drawn with egui painters rather than stock widgets: big
//! rounded tiles, a fire-orange accent on deep navy, pictograms painted from
//! primitives (egui's bundled font has no usable emoji), and a little motion so
//! the screen invites a click from across the gazebo.

use abm::suppression::{Task, UnitKind, UnitState};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Rounding, Shape, Stroke};
use bevy_egui::{EguiContexts, EguiSettings};

use super::strings_it as t;
use super::{Cmd, Kiosk, Phase, OPERATOR_HOLD_S, PLAY_IDLE_GRACE_S, PLAY_IDLE_WARN_S};
use crate::command::{OrderKind, OrderTool};
use crate::sim::Sim;

// --- palette -----------------------------------------------------------------
// CIMA Foundation palette (cimafoundation.org): deep navy #001E31, blue #004070,
// orange #DD7500, pale grey #D4DBDE. Fire and status colours stay functional.
const NAVY: Color32 = Color32::from_rgb(0, 30, 49);
const PANEL: Color32 = Color32::from_rgba_premultiplied(0, 27, 44, 235);
const PANEL_HI: Color32 = Color32::from_rgb(0, 64, 112);
const INK: Color32 = Color32::from_rgb(255, 255, 255);
const MUTED: Color32 = Color32::from_rgb(160, 176, 186);
const GREY: Color32 = Color32::from_rgb(212, 219, 222);
const FLAME: Color32 = Color32::from_rgb(221, 117, 0);
const AMBER: Color32 = Color32::from_rgb(252, 185, 0);
const GREEN: Color32 = Color32::from_rgb(52, 214, 140);
const RED: Color32 = Color32::from_rgb(255, 82, 90);
const SKY: Color32 = Color32::from_rgb(64, 152, 222);

fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// A vertical two-colour gradient inside a rectangle (square corners).
fn gradient(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.right_bottom(), bottom);
    m.colored_vertex(r.left_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

/// A rounded card: soft shadow, glass body, bright rim.
fn card(p: &egui::Painter, r: Rect, accent: Color32) {
    for i in 1..=5 {
        p.rect_filled(r.expand(i as f32 * 3.0).translate(vec2(0.0, 6.0)), Rounding::same(26.0 + i as f32 * 3.0), Color32::from_black_alpha(14));
    }
    p.rect_filled(r, Rounding::same(26.0), PANEL);
    p.rect_stroke(r, Rounding::same(26.0), Stroke::new(2.0, alpha(accent, 150)));
}

// --- pictograms ---------------------------------------------------------------
fn flame(p: &egui::Painter, c: Pos2, s: f32, k: f32) {
    let w = (k * 6.0).sin() * 0.06;
    let pts = |scale: f32, dy: f32| -> Vec<Pos2> {
        [
            (0.0, -1.0 + w), (0.35, -0.45), (0.62, -0.1), (0.55, 0.45), (0.28, 0.85), (0.0, 0.95),
            (-0.28, 0.85), (-0.55, 0.45), (-0.5, 0.0), (-0.28, -0.3), (-0.2, -0.65),
        ]
        .iter()
        .map(|(x, y)| pos2(c.x + x * s * scale, c.y + (y * scale + dy) * s))
        .collect()
    };
    p.add(Shape::convex_polygon(pts(1.0, 0.0), FLAME, Stroke::NONE));
    p.add(Shape::convex_polygon(pts(0.68, 0.22), AMBER, Stroke::NONE));
    p.add(Shape::convex_polygon(pts(0.36, 0.42), Color32::from_rgb(255, 244, 190), Stroke::NONE));
}

fn siren(p: &egui::Painter, c: Pos2, s: f32, col: Color32, k: f32) {
    p.rect_filled(Rect::from_center_size(pos2(c.x, c.y + s * 0.62), vec2(s * 1.6, s * 0.36)), Rounding::same(s * 0.1), mix(col, NAVY, 0.55));
    let dome = Rect::from_center_size(pos2(c.x, c.y + s * 0.1), vec2(s * 1.2, s * 1.0));
    p.rect_filled(dome, Rounding { nw: s * 0.6, ne: s * 0.6, sw: s * 0.08, se: s * 0.08 }, col);
    p.rect_filled(Rect::from_center_size(pos2(c.x - s * 0.22, c.y + s * 0.1), vec2(s * 0.16, s * 0.6)), Rounding::same(s * 0.08), Color32::from_white_alpha(120));
    for i in 0..3 {
        let a = -2.6 + i as f32 * 0.9;
        let (d0, d1) = (s * 0.95, s * (1.25 + 0.12 * ((k * 5.0 + i as f32).sin())));
        p.line_segment([pos2(c.x + a.cos() * d0, c.y - s * 0.1 + a.sin() * d0), pos2(c.x + a.cos() * d1, c.y - s * 0.1 + a.sin() * d1)], Stroke::new(s * 0.12, alpha(col, 200)));
    }
}

fn crew(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.circle_filled(pos2(c.x - s * 0.15, c.y - s * 0.5), s * 0.28, col);
    p.rect_filled(Rect::from_center_size(pos2(c.x - s * 0.15, c.y - s * 0.76), vec2(s * 0.66, s * 0.16)), Rounding::same(s * 0.08), AMBER);
    p.rect_filled(Rect::from_center_size(pos2(c.x - s * 0.15, c.y + s * 0.15), vec2(s * 0.62, s * 0.8)), Rounding::same(s * 0.2), col);
    p.line_segment([pos2(c.x + s * 0.35, c.y + s * 0.7), pos2(c.x + s * 0.7, c.y - s * 0.45)], Stroke::new(s * 0.14, Color32::from_rgb(160, 120, 80)));
    p.add(Shape::convex_polygon(
        vec![pos2(c.x + s * 0.5, c.y - s * 0.4), pos2(c.x + s * 0.95, c.y - s * 0.62), pos2(c.x + s * 0.86, c.y - s * 0.2)],
        Color32::from_rgb(190, 200, 215),
        Stroke::NONE,
    ));
}

fn engine(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.rect_filled(Rect::from_min_size(pos2(c.x - s * 1.0, c.y - s * 0.45), vec2(s * 1.35, s * 0.9)), Rounding::same(s * 0.12), col);
    p.rect_filled(Rect::from_min_size(pos2(c.x + s * 0.35, c.y - s * 0.2), vec2(s * 0.65, s * 0.65)), Rounding::same(s * 0.12), mix(col, Color32::BLACK, 0.2));
    p.rect_filled(Rect::from_min_size(pos2(c.x + s * 0.5, c.y - s * 0.1), vec2(s * 0.32, s * 0.24)), Rounding::same(s * 0.05), SKY);
    p.rect_filled(Rect::from_center_size(pos2(c.x - s * 0.3, c.y - s * 0.62), vec2(s * 0.5, s * 0.14)), Rounding::same(s * 0.05), AMBER);
    for x in [-0.55, 0.55] {
        p.circle_filled(pos2(c.x + s * x, c.y + s * 0.5), s * 0.25, NAVY);
        p.circle_filled(pos2(c.x + s * x, c.y + s * 0.5), s * 0.11, MUTED);
    }
}

fn plane(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    let q = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    p.add(Shape::convex_polygon(vec![q(-1.0, 0.1), q(-0.8, -0.1), q(0.7, -0.1), q(1.0, 0.1), q(0.7, 0.28), q(-0.7, 0.28)], col, Stroke::NONE));
    p.add(Shape::convex_polygon(vec![q(-0.15, -0.05), q(0.2, -0.05), q(0.5, -0.8), q(0.12, -0.8)], mix(col, Color32::WHITE, 0.25), Stroke::NONE));
    p.add(Shape::convex_polygon(vec![q(-0.15, 0.2), q(0.2, 0.2), q(0.5, 0.9), q(0.12, 0.9)], mix(col, Color32::BLACK, 0.15), Stroke::NONE));
    p.add(Shape::convex_polygon(vec![q(-0.9, -0.05), q(-0.65, -0.05), q(-0.75, -0.5), q(-0.95, -0.5)], col, Stroke::NONE));
    for i in 0..3 {
        p.circle_filled(q(-0.15 + i as f32 * 0.35, 0.62 + i as f32 * 0.06), s * 0.07, alpha(SKY, 200));
    }
}

fn pause_icon(p: &egui::Painter, c: Pos2, s: f32, col: Color32, paused: bool) {
    if paused {
        p.add(Shape::convex_polygon(vec![pos2(c.x - s * 0.5, c.y - s * 0.7), pos2(c.x + s * 0.7, c.y), pos2(c.x - s * 0.5, c.y + s * 0.7)], col, Stroke::NONE));
    } else {
        for x in [-0.4, 0.4] {
            p.rect_filled(Rect::from_center_size(pos2(c.x + s * x, c.y), vec2(s * 0.34, s * 1.3)), Rounding::same(s * 0.1), col);
        }
    }
}

fn house(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.rect_filled(Rect::from_center_size(pos2(c.x, c.y + s * 0.3), vec2(s * 1.2, s * 0.9)), Rounding::same(s * 0.08), col);
    p.add(Shape::convex_polygon(vec![pos2(c.x - s * 0.8, c.y - s * 0.1), pos2(c.x, c.y - s * 0.8), pos2(c.x + s * 0.8, c.y - s * 0.1)], mix(col, Color32::BLACK, 0.25), Stroke::NONE));
}

fn shield(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.add(Shape::convex_polygon(vec![pos2(c.x - s * 0.8, c.y - s * 0.7), pos2(c.x + s * 0.8, c.y - s * 0.7), pos2(c.x + s * 0.8, c.y + s * 0.1), pos2(c.x, c.y + s * 0.95), pos2(c.x - s * 0.8, c.y + s * 0.1)], col, Stroke::NONE));
    let w = Stroke::new(s * 0.2, NAVY);
    p.line_segment([pos2(c.x - s * 0.35, c.y), pos2(c.x - s * 0.05, c.y + s * 0.3)], w);
    p.line_segment([pos2(c.x - s * 0.05, c.y + s * 0.3), pos2(c.x + s * 0.4, c.y - s * 0.3)], w);
}

fn runner(p: &egui::Painter, c: Pos2, s: f32, col: Color32, k: f32) {
    let bob = (k * 8.0).sin() * s * 0.06;
    p.circle_filled(pos2(c.x + s * 0.1, c.y - s * 0.65 + bob), s * 0.22, col);
    p.line_segment([pos2(c.x, c.y - s * 0.35 + bob), pos2(c.x - s * 0.15, c.y + s * 0.2)], Stroke::new(s * 0.26, col));
    p.line_segment([pos2(c.x - s * 0.15, c.y + s * 0.2), pos2(c.x + s * 0.3, c.y + s * 0.8)], Stroke::new(s * 0.2, col));
    p.line_segment([pos2(c.x - s * 0.15, c.y + s * 0.2), pos2(c.x - s * 0.55, c.y + s * 0.75)], Stroke::new(s * 0.2, col));
    p.line_segment([pos2(c.x, c.y - s * 0.2), pos2(c.x + s * 0.55, c.y - s * 0.05)], Stroke::new(s * 0.16, col));
}

fn warning(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.add(Shape::convex_polygon(vec![pos2(c.x, c.y - s * 0.85), pos2(c.x + s * 0.95, c.y + s * 0.75), pos2(c.x - s * 0.95, c.y + s * 0.75)], col, Stroke::NONE));
    p.line_segment([pos2(c.x, c.y - s * 0.3), pos2(c.x, c.y + s * 0.28)], Stroke::new(s * 0.2, NAVY));
    p.circle_filled(pos2(c.x, c.y + s * 0.52), s * 0.11, NAVY);
}

/// The CIMA Foundation mark (white-on-transparent) with its name beside it.
fn logo(p: &egui::Painter, tex: Option<egui::TextureId>, at: Pos2, h: f32, caption: bool) {
    let Some(tex) = tex else { return };
    let w = h * 289.0 / 360.0;
    p.image(tex, Rect::from_min_size(at, vec2(w, h)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    if caption {
        txt(p, at + vec2(w + 14.0, h * 0.38), Align2::LEFT_CENTER, "CIMA Foundation", h * 0.30, INK);
        txt(p, at + vec2(w + 14.0, h * 0.68), Align2::LEFT_CENTER, "PROPAGATOR · modello di incendio", h * 0.19, GREY);
    }
}

// --- text helpers ------------------------------------------------------------
fn txt(p: &egui::Painter, pos: Pos2, a: Align2, s: &str, size: f32, col: Color32) {
    p.text(pos, a, s, FontId::proportional(size), col);
}

fn big(p: &egui::Painter, pos: Pos2, a: Align2, s: &str, size: f32, col: Color32) {
    // Stand-in for a bold face: a darker offset copy under the text.
    p.text(pos + vec2(0.0, size * 0.05), a, s, FontId::proportional(size), Color32::from_black_alpha(120));
    p.text(pos, a, s, FontId::proportional(size), col);
}

fn pill(p: &egui::Painter, centre: Pos2, label: &str, size: f32, fill: Color32, ink: Color32) -> Rect {
    let galley = p.layout_no_wrap(label.to_string(), FontId::proportional(size), ink);
    let r = Rect::from_center_size(centre, galley.size() + vec2(size * 1.6, size * 0.9));
    p.rect_filled(r.translate(vec2(0.0, 4.0)), Rounding::same(r.height() * 0.5), Color32::from_black_alpha(70));
    p.rect_filled(r, Rounding::same(r.height() * 0.5), fill);
    p.rect_filled(Rect::from_min_size(r.min + vec2(r.height() * 0.4, 3.0), vec2(r.width() - r.height() * 0.8, r.height() * 0.28)), Rounding::same(r.height() * 0.14), Color32::from_white_alpha(22));
    p.galley(r.center() - galley.size() * 0.5, galley, ink);
    r
}

/// A clickable pill button on an interactive area; returns whether it was clicked.
fn pill_button(ui: &mut egui::Ui, label: &str, size: f32, fill: Color32, ink: Color32) -> bool {
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(size), ink);
    let desired = galley.size() + vec2(size * 2.4, size * 1.1);
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
    let lift = if resp.hovered() { -3.0 } else { 0.0 };
    let r = rect.translate(vec2(0.0, lift));
    let p = ui.painter();
    p.rect_filled(rect.translate(vec2(0.0, 5.0)), Rounding::same(r.height() * 0.5), Color32::from_black_alpha(80));
    p.rect_filled(r, Rounding::same(r.height() * 0.5), if resp.hovered() { mix(fill, Color32::WHITE, 0.15) } else { fill });
    p.rect_filled(Rect::from_min_size(r.min + vec2(r.height() * 0.4, 4.0), vec2(r.width() - r.height() * 0.8, r.height() * 0.26)), Rounding::same(r.height() * 0.13), Color32::from_white_alpha(24));
    p.galley(r.center() - galley.size() * 0.5, galley, ink);
    resp.clicked()
}

// --- wind ---------------------------------------------------------------------
fn compass(p: &egui::Painter, c: Pos2, r: f32, to_deg: f32, yaw: f32, kmh: f32, k: f32) {
    p.circle_filled(c + vec2(0.0, 5.0), r, Color32::from_black_alpha(80));
    p.circle_filled(c, r, PANEL);
    p.circle_stroke(c, r, Stroke::new(2.0, alpha(SKY, 160)));
    // Tick marks turn with the camera; the N tells you which way is north.
    for i in 0..12 {
        let a = i as f32 * std::f32::consts::TAU / 12.0 + yaw;
        let (s, co) = a.sin_cos();
        let major = i % 3 == 0;
        let (r0, r1) = (r * if major { 0.78 } else { 0.86 }, r * 0.94);
        p.line_segment([c + vec2(s * r0, -co * r0), c + vec2(s * r1, -co * r1)], Stroke::new(if major { 3.0 } else { 1.5 }, alpha(MUTED, 160)));
    }
    let (sn, cn) = yaw.sin_cos();
    txt(p, c + vec2(sn * r * 0.62, -cn * r * 0.62), Align2::CENTER_CENTER, "N", r * 0.26, AMBER);
    // The arrow points where the fire is being driven.
    let a = super::view::screen_angle(to_deg, yaw);
    let (s, co) = a.sin_cos();
    let dir = vec2(s, -co);
    let side = vec2(co, s);
    let pulse = 1.0 + (k * 4.0).sin() * 0.05;
    let tip = c + dir * r * 0.72 * pulse;
    let tail = c - dir * r * 0.5;
    p.add(Shape::convex_polygon(vec![tip, c + dir * r * 0.18 + side * r * 0.28, c + dir * r * 0.18 - side * r * 0.28], FLAME, Stroke::new(1.5, AMBER)));
    p.add(Shape::convex_polygon(vec![c + dir * r * 0.2 + side * r * 0.1, tail + side * r * 0.1, tail - side * r * 0.1, c + dir * r * 0.2 - side * r * 0.1], mix(FLAME, NAVY, 0.3), Stroke::NONE));
    p.circle_filled(c, r * 0.07, INK);
    let _ = kmh;
}

/// The forecast, as a card: the wind with its uncertainty and the chance of a
/// change. Honest by layout -- it is labelled a forecast and shows odds, not a
/// verdict -- and kept apart from the compass, which shows what the wind *is*.
fn forecast_card(p: &egui::Painter, rect: Rect, f: &demo::Forecast) {
    p.rect_filled(rect.translate(vec2(0.0, 5.0)), Rounding::same(20.0), Color32::from_black_alpha(80));
    p.rect_filled(rect, Rounding::same(20.0), PANEL);
    p.rect_stroke(rect, Rounding::same(20.0), Stroke::new(2.0, alpha(SKY, 160)));
    let x = rect.left() + 20.0;
    let title = if f.issue > 1 { t::FORECAST_UPDATED } else { t::FORECAST };
    txt(p, pos2(x, rect.top() + 24.0), Align2::LEFT_CENTER, title, 22.0, SKY);
    txt(p, pos2(x, rect.bottom() - 18.0), Align2::LEFT_CENTER, t::FORECAST_CAVEAT, 18.0, MUTED);
    txt(p, pos2(x, rect.top() + 56.0), Align2::LEFT_CENTER, &t::forecast_wind(bearing_name(f.wind_from_deg), f.wind_kmh, f.cone_deg), 20.0, INK);
    txt(p, pos2(x, rect.top() + 90.0), Align2::LEFT_CENTER, t::SHIFT_CHANCE, 17.0, MUTED);
    let bar = Rect::from_min_size(pos2(x, rect.top() + 106.0), vec2(rect.width() - 110.0, 14.0));
    p.rect_filled(bar, Rounding::same(7.0), Color32::from_white_alpha(26));
    p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * f.shift_p, bar.height())), Rounding::same(7.0), AMBER);
    big(p, pos2(bar.right() + 12.0, bar.center().y), Align2::LEFT_CENTER, &format!("{:.0}%", f.shift_p * 100.0), 26.0, INK);
    txt(p, pos2(x, rect.top() + 134.0), Align2::LEFT_CENTER, &t::forecast_shift(bearing_name(f.shift_to_deg), f.shift_eta_min.0, f.shift_eta_min.1), 17.0, GREY);
}

/// Distance from the screen's bottom edge to the top of the action bar.
const ACTION_BAR_TOP: f32 = 148.0 + 56.0;

fn bearing_name(deg: f32) -> &'static str {
    const N: [&str; 8] = ["nord", "nord-est", "est", "sud-est", "sud", "sud-ovest", "ovest", "nord-ovest"];
    N[(((deg % 360.0 + 360.0) % 360.0 + 22.5) / 45.0) as usize % 8]
}

// --- state of one frame --------------------------------------------------------
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut settings: ResMut<EguiSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut kiosk: ResMut<Kiosk>,
    mut sim: ResMut<Sim>,
    mut tool: ResMut<OrderTool>,
    mut focus: ResMut<crate::ui::UiFocus>,
    orbit: Query<&crate::camera::OrbitCamera>,
    time: Res<Time>,
    mut images: ResMut<Assets<Image>>,
) {
    if kiosk.logo.is_none() {
        let bytes = include_bytes!("../../../../assets/brand/cima_logo_white.png");
        if let Ok(img) = Image::from_buffer(
            bytes,
            bevy::render::texture::ImageType::Extension("png"),
            bevy::render::texture::CompressedImageFormats::NONE,
            true,
            bevy::render::texture::ImageSampler::linear(),
            bevy::render::render_asset::RenderAssetUsages::default(),
        ) {
            let handle = images.add(img);
            kiosk.logo = Some(contexts.add_image(handle.clone_weak()));
            kiosk.logo_handle = Some(handle);
        }
    }
    let ctx = contexts.ctx_mut();
    if let Ok(w) = windows.get_single() {
        settings.scale_factor = (w.height() / 720.0).clamp(1.0, 2.0);
    }
    if !kiosk.styled {
        let mut style = (*ctx.style()).clone();
        style.visuals.override_text_color = Some(INK);
        ctx.set_style(style);
        kiosk.styled = true;
    }
    // Held for the whole session: no shortcut system will read the keyboard.
    focus.keyboard = true;
    focus.pointer = false;

    let k = time.elapsed_seconds();
    let screen = ctx.screen_rect();
    let yaw = orbit.get_single().map_or(0.0, |o| o.yaw);

    // Hidden operator corner: hold the top-right corner for a few seconds.
    let corner = Rect::from_min_size(pos2(screen.right() - 90.0, screen.top()), vec2(90.0, 90.0));
    let held = ctx.input(|i| i.pointer.primary_down() && i.pointer.hover_pos().is_some_and(|p| corner.contains(p)));
    kiosk.corner_hold = if held { kiosk.corner_hold + time.delta_seconds() } else { 0.0 };
    if kiosk.corner_hold >= OPERATOR_HOLD_S {
        kiosk.operator_open = true;
        kiosk.corner_hold = 0.0;
    }
    let clicked_anywhere = ctx.input(|i| i.pointer.primary_clicked()) && !held;

    match kiosk.phase {
        Phase::Attract => attract(ctx, &mut kiosk, screen, k, clicked_anywhere),
        Phase::Briefing => briefing(ctx, &mut kiosk, screen, k),
        Phase::Play => play(ctx, &mut kiosk, &mut sim, &mut tool, screen, k, yaw),
        Phase::Outcome => outcome(ctx, &mut kiosk, screen, k),
        Phase::Compare => compare(ctx, &mut kiosk, screen, k),
    }
    if kiosk.operator_open {
        operator(ctx, &mut kiosk, screen);
    }
    focus.pointer = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}

fn layer(ctx: &egui::Context, name: &str) -> egui::Painter {
    ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new(name)))
}

fn attract(ctx: &egui::Context, kiosk: &mut Kiosk, screen: Rect, k: f32, clicked: bool) {
    let p = layer(ctx, "attract");
    // Vignette: dark at the top and bottom so the title and prompt read over any map.
    gradient(&p, Rect::from_min_size(screen.min, vec2(screen.width(), screen.height() * 0.32)), Color32::from_black_alpha(190), Color32::TRANSPARENT);
    gradient(&p, Rect::from_min_size(pos2(screen.left(), screen.bottom() - screen.height() * 0.3), vec2(screen.width(), screen.height() * 0.3)), Color32::TRANSPARENT, Color32::from_black_alpha(200));
    logo(&p, kiosk.logo, screen.left_top() + vec2(28.0, 22.0), 96.0, true);
    let c = pos2(screen.center().x, screen.top() + screen.height() * 0.2);
    let half = p.layout_no_wrap(t::TITLE.to_string(), FontId::proportional(64.0), INK).size().x * 0.5;
    flame(&p, c - vec2(half + 56.0, 0.0), 40.0, k);
    flame(&p, c + vec2(half + 56.0, 0.0), 40.0, k + 1.3);
    big(&p, c, Align2::CENTER_CENTER, t::TITLE, 64.0, INK);
    txt(&p, c + vec2(0.0, 52.0), Align2::CENTER_CENTER, t::SUBTITLE, 26.0, AMBER);
    let town = t::town(kiosk.spec.id);
    txt(&p, pos2(screen.center().x, screen.bottom() - screen.height() * 0.2), Align2::CENTER_CENTER, &format!("{} · {}", town.name, town.place), 28.0, INK);
    let bob = (k * 3.0).sin() * 5.0;
    let grow = 1.0 + (k * 3.0).sin() * 0.03;
    pill(&p, pos2(screen.center().x, screen.bottom() - screen.height() * 0.12 + bob), t::START, 40.0 * grow, FLAME, Color32::WHITE);
    if clicked {
        kiosk.cmd = Some(Cmd::Begin);
    }
}

fn briefing(ctx: &egui::Context, kiosk: &mut Kiosk, screen: Rect, k: f32) {
    let town = t::town(kiosk.spec.id);
    let slide = 1.0 - ((kiosk.phase_t / 0.5).min(1.0) - 1.0).powi(2);
    let size = vec2(860.0f32.min(screen.width() - 40.0), 330.0);
    let rect = Rect::from_center_size(pos2(screen.center().x, screen.bottom() - size.y * 0.5 - 40.0 + (1.0 - slide) * 80.0), size);
    let fc = Rect::from_min_size(pos2(rect.left(), rect.top() - 202.0 + (1.0 - slide) * 80.0), vec2(rect.width().min(560.0), 186.0));
    forecast_card(&layer(ctx, "forecast_brief"), fc, &kiosk.forecast_at(0));
    egui::Area::new("briefing".into()).fixed_pos(rect.min).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.set_min_size(size);
        let p = ui.painter().clone();
        card(&p, rect, FLAME);
        // Banner strip.
        let strip = Rect::from_min_size(rect.min + vec2(0.0, 0.0), vec2(rect.width(), 86.0));
        p.rect_filled(strip.shrink(2.0), Rounding { nw: 24.0, ne: 24.0, sw: 0.0, se: 0.0 }, mix(FLAME, NAVY, 0.55));
        flame(&p, strip.left_center() + vec2(54.0, 0.0), 26.0, k);
        big(&p, strip.left_center() + vec2(96.0, -12.0), Align2::LEFT_CENTER, town.name, 40.0, INK);
        txt(&p, strip.left_center() + vec2(96.0, 22.0), Align2::LEFT_CENTER, town.place, 22.0, AMBER);
        // Body.
        let body = Rect::from_min_max(rect.min + vec2(36.0, 104.0), rect.max - vec2(36.0, 100.0));
        let g = p.layout(town.brief.to_string(), FontId::proportional(26.0), INK, body.width());
        p.galley(body.min, g, INK);
        p.rect_filled(Rect::from_min_size(pos2(rect.left() + 36.0, rect.bottom() - 92.0), vec2(8.0, 30.0)), Rounding::same(4.0), GREEN);
        txt(&p, pos2(rect.left() + 58.0, rect.bottom() - 77.0), Align2::LEFT_CENTER, town.mission, 26.0, GREEN);
        let mut ui2 = ui.child_ui(Rect::from_min_size(pos2(rect.right() - 300.0, rect.bottom() - 84.0), vec2(280.0, 64.0)), egui::Layout::right_to_left(egui::Align::Center), None);
        if pill_button(&mut ui2, t::GO, 30.0, FLAME, Color32::WHITE) {
            kiosk.cmd = Some(Cmd::Go);
        }
    });
}

fn counter_tile(p: &egui::Painter, at: Pos2, label: &str, value: usize, col: Color32, icon: impl Fn(&egui::Painter, Pos2, f32), pulse: f32) {
    let r = Rect::from_min_size(at, vec2(210.0, 84.0));
    p.rect_filled(r.translate(vec2(0.0, 5.0)), Rounding::same(20.0), Color32::from_black_alpha(80));
    p.rect_filled(r, Rounding::same(20.0), PANEL);
    p.rect_filled(Rect::from_min_size(r.min, vec2(10.0, r.height())), Rounding { nw: 20.0, sw: 20.0, ne: 0.0, se: 0.0 }, col);
    if pulse > 0.0 {
        p.rect_stroke(r, Rounding::same(20.0), Stroke::new(3.0, alpha(col, (pulse * 255.0) as u8)));
    }
    p.circle_filled(r.left_center() + vec2(48.0, 0.0), 28.0, alpha(col, 40));
    icon(p, r.left_center() + vec2(48.0, 0.0), 19.0);
    big(p, r.min + vec2(88.0, 36.0), Align2::LEFT_CENTER, &value.to_string(), 44.0, INK);
    txt(p, r.min + vec2(90.0, 67.0), Align2::LEFT_CENTER, label, 20.0, alpha(col, 255));
}

#[allow(clippy::too_many_arguments)]
fn play(ctx: &egui::Context, kiosk: &mut Kiosk, sim: &mut Sim, tool: &mut OrderTool, screen: Rect, k: f32, yaw: f32) {
    let p = layer(ctx, "hud");
    let stats = sim.agents.stats();
    let outcome = kiosk.tally.outcome(&sim.agents, &sim.fire, &sim.scenario.world);

    // Counters, top-left.
    let origin = screen.left_top() + vec2(20.0, 20.0);
    counter_tile(&p, origin, t::SAFE, stats.safe, GREEN, |p, c, s| shield(p, c, s, GREEN), 0.0);
    counter_tile(&p, origin + vec2(222.0, 0.0), t::MOVING, stats.moving, SKY, |p, c, s| runner(p, c, s, SKY, k), 0.0);
    let danger_pulse = if outcome.in_danger > 0 { 0.5 + 0.5 * (k * 6.0).sin() } else { 0.0 };
    counter_tile(&p, origin + vec2(444.0, 0.0), t::DANGER, outcome.in_danger, AMBER, |p, c, s| warning(p, c, s, AMBER), danger_pulse * 0.9);
    counter_tile(&p, origin + vec2(666.0, 0.0), t::HOMES_LOST, outcome.homes_lost, RED, |p, c, s| house(p, c, s, RED), 0.0);

    // Mission strip, under the counters: town, clock, time left.
    let town = t::town(kiosk.spec.id);
    let strip = Rect::from_min_size(origin + vec2(0.0, 100.0), vec2(876.0, 56.0));
    p.rect_filled(strip, Rounding::same(28.0), PANEL);
    let frac = (sim.time_s() as f32 / kiosk.spec.duration_s as f32).clamp(0.0, 1.0);
    let bar = Rect::from_min_size(strip.min + vec2(20.0, 38.0), vec2(strip.width() - 40.0, 8.0));
    p.rect_filled(bar, Rounding::same(4.0), Color32::from_white_alpha(24));
    p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * frac, bar.height())), Rounding::same(4.0), mix(AMBER, FLAME, frac));
    txt(&p, strip.min + vec2(24.0, 19.0), Align2::LEFT_CENTER, &format!("{} · {}", town.name, town.mission), 22.0, INK);
    let left_s = (kiosk.spec.duration_s - sim.time_s()).max(0);
    txt(&p, strip.right_top() + vec2(-24.0, 19.0), Align2::RIGHT_CENTER, &format!("{} {:02}:{:02}", t::TIME_LEFT, left_s / 60, left_s % 60), 24.0, AMBER);

    // Wind, top-right, under the operator corner.
    let w = sim.fire.weather();
    let to = ((w.wind_dir_deg as f32) + 180.0) % 360.0;
    let wc = pos2(screen.right() - 110.0, screen.top() + 190.0);
    compass(&p, wc, 80.0, to, yaw, w.wind_speed_kmh as f32, k);
    txt(&p, wc + vec2(0.0, 98.0), Align2::CENTER_CENTER, &format!("{} {} · {:.0} km/h", t::WIND_FROM, bearing_name(w.wind_dir_deg as f32), w.wind_speed_kmh), 20.0, INK);

    let fc = Rect::from_min_size(pos2(screen.right() - 20.0 - 380.0, wc.y + 124.0), vec2(380.0, 186.0));
    forecast_card(&p, fc, &kiosk.forecast_at(sim.time_s()));

    // Banner: a pending order, or the last refusal.
    let banner_y = screen.bottom() - ACTION_BAR_TOP - 70.0;
    let action_label = |kind: UnitKind| match kind {
        UnitKind::HandCrew => t::ACT_CREW,
        UnitKind::Engine => t::ACT_ENGINE,
        UnitKind::AirTanker => t::ACT_AIR,
    };
    if let Some(msg) = tool.refusal.take() {
        kiosk.say(t::refusal(&msg));
    }
    if tool.confirmation.take().is_some() {
        kiosk.banner = None;
    }
    let mut cancel = false;
    egui::Area::new("banner".into()).fixed_pos(pos2(screen.center().x - 380.0, banner_y)).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.set_width(760.0);
        let (text, col) = if let (Some(id), true) = (tool.selected, tool.is_armed()) {
            let kind = sim.crews.units[id].kind;
            let what = if tool.armed == Some(OrderKind::Line) { t::PENDING_LINE.to_string() } else { t::pending_order(action_label(kind)) };
            (Some(what), FLAME)
        } else if let Some((m, _)) = &kiosk.banner {
            (Some(m.clone()), AMBER)
        } else {
            (None, FLAME)
        };
        if let Some(text) = text {
            let r = pill(ui.painter(), pos2(screen.center().x - 40.0, banner_y + 24.0), &text, 26.0, col, NAVY);
            if tool.is_armed() {
                let x = Rect::from_center_size(pos2(r.right() + 40.0, r.center().y), vec2(60.0, 60.0));
                let resp = ui.interact(x, egui::Id::new("cancel"), egui::Sense::click());
                ui.painter().circle_filled(x.center(), 24.0, if resp.hovered() { RED } else { mix(RED, NAVY, 0.4) });
                let s = Stroke::new(4.0, Color32::WHITE);
                ui.painter().line_segment([x.center() + vec2(-8.0, -8.0), x.center() + vec2(8.0, 8.0)], s);
                ui.painter().line_segment([x.center() + vec2(8.0, -8.0), x.center() + vec2(-8.0, 8.0)], s);
                cancel = resp.clicked();
            }
        }
    });
    if cancel {
        tool.disarm();
    }

    logo(&p, kiosk.logo, screen.left_top() + vec2(24.0, 176.0), 60.0, false);
    action_bar(ctx, kiosk, sim, tool, screen, k);

    // "Are you still there?"
    if kiosk.idle_s >= PLAY_IDLE_WARN_S && kiosk.ordered_at_s.is_none() && !kiosk.paused {
        let left = (PLAY_IDLE_WARN_S + PLAY_IDLE_GRACE_S - kiosk.idle_s).max(0.0);
        let q = layer(ctx, "still");
        q.rect_filled(screen, Rounding::ZERO, Color32::from_black_alpha(150));
        big(&q, screen.center() - vec2(0.0, 30.0), Align2::CENTER_CENTER, t::STILL_THERE, 72.0, INK);
        txt(&q, screen.center() + vec2(0.0, 40.0), Align2::CENTER_CENTER, &format!("{} ({:.0})", t::STILL_THERE_SUB, left), 28.0, AMBER);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Ready,
    Armed,
    Off,
    Done,
}

#[allow(clippy::too_many_arguments)]
fn action_button(
    ui: &mut egui::Ui,
    size: Vec2x,
    title: &str,
    sub: &str,
    state: Btn,
    accent: Color32,
    k: f32,
    icon: impl Fn(&egui::Painter, Pos2, f32),
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(size.0, size.1), egui::Sense::click());
    let hot = resp.hovered() && matches!(state, Btn::Ready | Btn::Armed);
    let r = rect.translate(vec2(0.0, if hot { -5.0 } else { 0.0 }));
    let p = ui.painter();
    p.rect_filled(rect.translate(vec2(0.0, 6.0)), Rounding::same(22.0), Color32::from_black_alpha(90));
    let body = match state {
        Btn::Off | Btn::Done => Color32::from_rgb(28, 34, 54),
        _ => PANEL_HI,
    };
    p.rect_filled(r, Rounding::same(22.0), body);
    if matches!(state, Btn::Ready | Btn::Armed) {
        gradient_round(p, r, mix(accent, Color32::WHITE, 0.1), mix(accent, NAVY, 0.55), 22.0, 0.34);
    }
    if state == Btn::Armed {
        let pulse = 0.5 + 0.5 * (k * 7.0).sin();
        p.rect_stroke(r.expand(4.0 + pulse * 3.0), Rounding::same(26.0), Stroke::new(4.0, alpha(AMBER, (120.0 + pulse * 135.0) as u8)));
    } else if hot {
        p.rect_stroke(r, Rounding::same(22.0), Stroke::new(3.0, INK));
    }
    let dim = matches!(state, Btn::Off | Btn::Done);
    let tint = if dim { MUTED } else { INK };
    p.circle_filled(r.center_top() + vec2(0.0, 40.0), 31.0, Color32::from_black_alpha(if dim { 40 } else { 70 }));
    icon(p, r.center_top() + vec2(0.0, 40.0), 23.0);
    txt(p, r.center_top() + vec2(0.0, 90.0), Align2::CENTER_CENTER, title, 26.0, tint);
    txt(p, r.center_top() + vec2(0.0, 116.0), Align2::CENTER_CENTER, sub, 18.0, if dim { alpha(MUTED, 180) } else { alpha(INK, 210) });
    if state == Btn::Done {
        p.circle_filled(r.right_top() + vec2(-18.0, 18.0), 13.0, GREEN);
        let w = Stroke::new(3.0, NAVY);
        p.line_segment([r.right_top() + vec2(-24.0, 18.0), r.right_top() + vec2(-19.0, 23.0)], w);
        p.line_segment([r.right_top() + vec2(-19.0, 23.0), r.right_top() + vec2(-11.0, 12.0)], w);
    }
    resp.clicked() && matches!(state, Btn::Ready | Btn::Armed)
}

type Vec2x = (f32, f32);

/// Gradient confined to the top `frac` of a rounded rectangle.
fn gradient_round(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32, radius: f32, frac: f32) {
    // Layer thin rounded slices so the corners stay round.
    let n = 36;
    let h = r.height();
    for i in 0..n {
        let f = i as f32 / n as f32;
        let y0 = r.top() + f * h;
        let y1 = r.top() + (f + 1.0 / n as f32) * h + 1.0;
        let c = mix(top, bottom, (f / (frac.max(0.01) * 1.0 + 0.0)).min(1.0).max(f));
        let slice = Rect::from_min_max(pos2(r.left(), y0), pos2(r.right(), y1.min(r.bottom())));
        let rr = Rounding {
            nw: if i == 0 { radius } else { 0.0 },
            ne: if i == 0 { radius } else { 0.0 },
            sw: if i == n - 1 { radius } else { 0.0 },
            se: if i == n - 1 { radius } else { 0.0 },
        };
        p.rect_filled(slice, rr, c);
    }
}

fn best_unit(sim: &Sim, kind: UnitKind) -> Option<usize> {
    sim.crews
        .units
        .iter()
        .filter(|u| u.kind == kind && u.assignable())
        .min_by_key(|u| match u.state {
            UnitState::Staged => 0,
            UnitState::Inbound => 1,
            UnitState::Moving | UnitState::Working | UnitState::Refilling => 2,
            _ => 3,
        })
        .map(|u| u.id)
}

fn free_count(sim: &Sim, kind: UnitKind) -> usize {
    sim.crews.units.iter().filter(|u| u.kind == kind && matches!(u.state, UnitState::Staged | UnitState::Inbound)).count()
}

fn action_bar(ctx: &egui::Context, kiosk: &mut Kiosk, sim: &mut Sim, tool: &mut OrderTool, screen: Rect, k: f32) {
    let btn = (190.0, 148.0);
    let total = 5.0 * btn.0 + 4.0 * 16.0 + 48.0;
    let pos = pos2(screen.center().x - total * 0.5, screen.bottom() - ACTION_BAR_TOP);
    egui::Area::new("actions".into()).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
        let bar = Rect::from_min_size(pos, vec2(total, btn.1 + 40.0));
        card(ui.painter(), bar, SKY);
        let mut inner = ui.child_ui(bar.shrink2(vec2(24.0, 20.0)), egui::Layout::left_to_right(egui::Align::Min), None);
        inner.spacing_mut().item_spacing = vec2(16.0, 0.0);
        let ui = &mut inner;

        // Evacuazione: one order to everyone, once.
        let done = kiosk.ordered_at_s.is_some();
        let state = if done { Btn::Done } else { Btn::Ready };
        if action_button(ui, btn, t::ACT_EVACUATE, if done { t::ACT_EVACUATE_DONE } else { t::ACT_EVACUATE_SUB }, state, RED, k, |p, c, s| siren(p, c, s, if done { MUTED } else { Color32::WHITE }, k)) {
            sim.agents.order_evacuation_all();
            kiosk.ordered_at_s = Some(sim.time_s());
            tool.disarm();
        }

        for (kind, title, accent) in [(UnitKind::HandCrew, t::ACT_CREW, AMBER), (UnitKind::Engine, t::ACT_ENGINE, FLAME)] {
            let n = free_count(sim, kind);
            let armed = tool.is_armed() && tool.selected.is_some_and(|id| sim.crews.units[id].kind == kind);
            let state = if armed { Btn::Armed } else if best_unit(sim, kind).is_some() { Btn::Ready } else { Btn::Off };
            let sub = if state == Btn::Off { t::ACT_NONE_LEFT.to_string() } else { t::free_units(n) };
            if action_button(ui, btn, title, &sub, state, accent, k, |p, c, s| {
                if kind == UnitKind::HandCrew { crew(p, c, s, Color32::WHITE) } else { engine(p, c, s, Color32::WHITE) }
            }) {
                if armed {
                    tool.disarm();
                } else if let Some(id) = best_unit(sim, kind) {
                    tool.disarm();
                    tool.selected = Some(id);
                    tool.toggle(OrderKind::Attack);
                }
            }
        }

        // Canadair: asking and tasking are one gesture, and the 25 minutes are the cost.
        let air = sim.crews.air_eta_s();
        let requested = sim.crews.units.iter().any(|u| u.kind.is_air() && u.state != UnitState::Unavailable);
        let armed = tool.is_armed() && tool.armed == Some(OrderKind::Drop);
        let state = if armed { Btn::Armed } else { Btn::Ready };
        let sub = match (requested, air) {
            (false, _) => t::ACT_AIR_ASK.to_string(),
            (true, Some(s)) => t::air_inbound((s / 60.0) as u32, (s % 60.0) as u32),
            (true, None) => t::ACT_AIR_READY.to_string(),
        };
        if action_button(ui, btn, t::ACT_AIR, &sub, state, SKY, k, |p, c, s| plane(p, c, s, Color32::WHITE)) {
            if armed {
                tool.disarm();
            } else {
                sim.crews.request_air();
                if let Some(id) = best_unit(sim, UnitKind::AirTanker) {
                    tool.disarm();
                    tool.selected = Some(id);
                    tool.toggle(OrderKind::Drop);
                }
            }
        }

        let paused = kiosk.paused;
        let (title, sub) = if paused { (t::ACT_RESUME, t::PAUSED) } else { (t::ACT_PAUSE, t::ACT_PAUSE_SUB) };
        if action_button(ui, btn, title, sub, Btn::Ready, MUTED, k, |p, c, s| pause_icon(p, c, s, Color32::WHITE, paused)) {
            kiosk.paused = !paused;
        }
    });
    let _ = Task::Return;
}

fn stat_row(p: &egui::Painter, at: Pos2, width: f32, label: &str, value: String, frac: Option<f32>, col: Color32) {
    big(p, at, Align2::LEFT_CENTER, &value, 42.0, INK);
    txt(p, at + vec2(0.0, 36.0), Align2::LEFT_CENTER, label, 20.0, col);
    if let Some(f) = frac {
        let bar = Rect::from_min_size(at + vec2(0.0, 58.0), vec2(width, 10.0));
        p.rect_filled(bar, Rounding::same(5.0), Color32::from_white_alpha(26));
        p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * f.clamp(0.0, 1.0), bar.height())), Rounding::same(5.0), col);
    }
}

fn outcome(ctx: &egui::Context, kiosk: &mut Kiosk, screen: Rect, k: f32) {
    let Some(res) = kiosk.result else { return };
    let size = vec2(900.0f32.min(screen.width() - 40.0), 520.0);
    let rect = Rect::from_center_size(screen.center(), size);
    layer(ctx, "dim").rect_filled(screen, Rounding::ZERO, Color32::from_black_alpha(120));
    egui::Area::new("outcome".into()).fixed_pos(rect.min).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.set_min_size(size);
        let p = ui.painter().clone();
        card(&p, rect, GREEN);
        let head = Rect::from_min_size(rect.min, vec2(size.x, 90.0));
        p.rect_filled(head.shrink(2.0), Rounding { nw: 24.0, ne: 24.0, sw: 0.0, se: 0.0 }, mix(GREEN, NAVY, 0.6));
        shield(&p, head.left_center() + vec2(56.0, 0.0), 26.0, GREEN);
        big(&p, head.left_center() + vec2(106.0, 0.0), Align2::LEFT_CENTER, t::OUTCOME_TITLE, 44.0, INK);

        let col_w = (size.x - 120.0) / 2.0;
        let left = rect.min + vec2(40.0, 150.0);
        let right = left + vec2(col_w + 40.0, 0.0);
        let frac = res.secure() as f32 / res.households.max(1) as f32;
        // The headline pops in with a little overshoot.
        let pop = 1.0 + (1.0 - (kiosk.phase_t * 3.0).min(1.0)).powi(2) * 0.25;
        big(&p, rect.min + vec2(size.x * 0.5, 120.0), Align2::CENTER_CENTER, &t::safe_of(res.secure(), res.households), 44.0 * pop, GREEN);
        stat_row(&p, left + vec2(0.0, 30.0), col_w, t::FAMILIES_SAFE, t::safe_of(res.secure(), res.households), Some(frac), GREEN);
        stat_row(&p, right + vec2(0.0, 30.0), col_w, t::STILL_IN_DANGER, res.in_danger.to_string(), None, AMBER);
        stat_row(&p, left + vec2(0.0, 150.0), col_w, t::HOMES_LOST_LC, res.homes_lost.to_string(), None, RED);
        stat_row(&p, right + vec2(0.0, 150.0), col_w, t::HECTARES, format!("{:.0}", res.hectares), None, FLAME);
        flame(&p, rect.right_top() + vec2(-60.0, 45.0), 24.0, k);
        logo(&p, kiosk.logo, rect.left_bottom() + vec2(36.0, -76.0), 56.0, true);
        let mut b = ui.child_ui(Rect::from_min_size(pos2(rect.right() - 480.0, rect.bottom() - 84.0), vec2(450.0, 64.0)), egui::Layout::right_to_left(egui::Align::Center), None);
        if pill_button(&mut b, t::SEE_COMPARE, 26.0, FLAME, Color32::WHITE) {
            kiosk.enter(Phase::Compare);
        }
    });
}

fn compare(ctx: &egui::Context, kiosk: &mut Kiosk, screen: Rect, k: f32) {
    let Some(you) = kiosk.result else { return };
    let size = vec2(960.0f32.min(screen.width() - 40.0), 620.0);
    let rect = Rect::from_center_size(screen.center(), size);
    layer(ctx, "dim").rect_filled(screen, Rounding::ZERO, Color32::from_black_alpha(140));
    egui::Area::new("compare".into()).fixed_pos(rect.min).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.set_min_size(size);
        let p = ui.painter().clone();
        card(&p, rect, SKY);
        let head = Rect::from_min_size(rect.min, vec2(size.x, 84.0));
        p.rect_filled(head.shrink(2.0), Rounding { nw: 24.0, ne: 24.0, sw: 0.0, se: 0.0 }, mix(SKY, NAVY, 0.65));
        big(&p, head.center(), Align2::CENTER_CENTER, t::COMPARE_TITLE, 40.0, INK);
        let cf = kiosk.counterfactual;
        let cols = [(t::YOU, Some(you), GREEN), (t::NO_ORDERS, cf, MUTED)];
        let w = (size.x - 120.0) / 2.0;
        for (i, (name, o, col)) in cols.iter().enumerate() {
            let x = rect.min.x + 40.0 + i as f32 * (w + 40.0);
            let col_rect = Rect::from_min_size(pos2(x, rect.min.y + 104.0), vec2(w, 340.0));
            p.rect_filled(col_rect, Rounding::same(20.0), alpha(*col, 18));
            p.rect_stroke(col_rect, Rounding::same(20.0), Stroke::new(2.0, alpha(*col, 110)));
            big(&p, col_rect.center_top() + vec2(0.0, 28.0), Align2::CENTER_CENTER, name, 32.0, *col);
            match o {
                Some(o) => {
                    let f = o.secure() as f32 / o.households.max(1) as f32;
                    let hw = (w - 72.0) / 2.0;
                    stat_row(&p, col_rect.min + vec2(28.0, 90.0), w - 56.0, t::FAMILIES_SAFE, t::safe_of(o.secure(), o.households), Some(f), *col);
                    stat_row(&p, col_rect.min + vec2(28.0, 190.0), hw, t::CAUGHT_SHORT, o.caught.to_string(), None, AMBER);
                    stat_row(&p, col_rect.min + vec2(44.0 + hw, 190.0), hw, t::HECTARES_SHORT, format!("{:.0}", o.hectares), None, FLAME);
                    stat_row(&p, col_rect.min + vec2(28.0, 270.0), w - 56.0, t::HOMES_LOST_LC, o.homes_lost.to_string(), None, RED);
                }
                None => {
                    // The twin is still running: a spinner, not a blank.
                    let c = col_rect.center();
                    for j in 0..8 {
                        let a = k * 5.0 + j as f32 * 0.785;
                        p.circle_filled(c + vec2(a.cos(), a.sin()) * 30.0, 6.0 - j as f32 * 0.5, alpha(INK, 255 - j * 25));
                    }
                }
            }
        }
        if let Some(cf) = cf {
            let msg = t::takeaway(you.caught, cf.caught);
            let g = p.layout(msg, FontId::proportional(23.0), AMBER, size.x - 80.0);
            p.galley(pos2(rect.min.x + 40.0, rect.min.y + 462.0), g, AMBER);
        }
        logo(&p, kiosk.logo, rect.left_bottom() + vec2(36.0, -76.0), 56.0, false);
        let mut b = ui.child_ui(Rect::from_min_size(pos2(rect.right() - 640.0, rect.bottom() - 76.0), vec2(610.0, 60.0)), egui::Layout::right_to_left(egui::Align::Center), None);
        if pill_button(&mut b, t::ANOTHER_TOWN, 24.0, PANEL_HI, INK) {
            kiosk.cmd = Some(Cmd::NextTown);
        }
        if pill_button(&mut b, t::RETRY, 24.0, FLAME, Color32::WHITE) {
            kiosk.cmd = Some(Cmd::Retry);
        }
    });
}

fn operator(ctx: &egui::Context, kiosk: &mut Kiosk, screen: Rect) {
    egui::Window::new("Operatore").anchor(Align2::RIGHT_TOP, vec2(-16.0, 16.0)).collapsible(false).show(ctx, |ui| {
        ui.checkbox(&mut kiosk.pinned, "Resta su questo paese");
        if ui.button("Prossimo paese").clicked() {
            kiosk.cmd = Some(Cmd::NextTown);
        }
        if ui.button("Ricomincia").clicked() {
            kiosk.cmd = Some(Cmd::Begin);
            kiosk.pinned = true;
        }
        if ui.button("Chiudi").clicked() {
            kiosk.operator_open = false;
        }
        let _ = screen;
    });
}
