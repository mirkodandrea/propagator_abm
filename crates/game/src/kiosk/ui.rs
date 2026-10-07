//! The kiosk screens, drawn with egui painters rather than stock widgets: big
//! rounded tiles, a fire-orange accent on deep navy, pictograms painted from
//! primitives (egui's bundled font has no usable emoji), and a little motion so
//! the screen invites a click from across the gazebo.
//!
//! The controls that matter live **on the map**: each district carries a chip
//! with its name, how far the fire is, and two buttons -- warn it, defend it.
//! The HUD around the edge is kept slim so the town is the screen.

use abm::suppression::{UnitKind, UnitState};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Rounding, Shape, Stroke};
use bevy_egui::{EguiContexts, EguiSettings};
use demo::district::{Level, Report};
use demo::Order;

use super::strings_it as t;
use super::{Cmd, Kiosk, Phase, Speaker, OPERATOR_HOLD_S};
use crate::command::{OrderKind, OrderTool};
use crate::sim::Sim;

// --- palette -----------------------------------------------------------------
// CIMA Foundation palette (cimafoundation.org): deep navy #001E31, blue #004070,
// orange #DD7500, pale grey #D4DBDE. Fire and status colours stay functional.
const NAVY: Color32 = Color32::from_rgb(0, 30, 49);
const PANEL: Color32 = Color32::from_rgba_premultiplied(0, 27, 44, 232);
const PANEL_HI: Color32 = Color32::from_rgb(0, 64, 112);
const INK: Color32 = Color32::from_rgb(255, 255, 255);
const MUTED: Color32 = Color32::from_rgb(160, 176, 186);
const GREY: Color32 = Color32::from_rgb(212, 219, 222);
const FLAME: Color32 = Color32::from_rgb(221, 117, 0);
const AMBER: Color32 = Color32::from_rgb(252, 185, 0);
const GREEN: Color32 = Color32::from_rgb(52, 214, 140);
const RED: Color32 = Color32::from_rgb(255, 82, 90);
const SKY: Color32 = Color32::from_rgb(64, 152, 222);
const GOLD: Color32 = Color32::from_rgb(255, 210, 90);

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

/// A horizontal gradient (square corners).
fn hgradient(p: &egui::Painter, r: Rect, left: Color32, right: Color32) {
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top(), left);
    m.colored_vertex(r.right_top(), right);
    m.colored_vertex(r.right_bottom(), right);
    m.colored_vertex(r.left_bottom(), left);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

/// A rounded card: soft shadow, glass body, bright rim.
fn card(p: &egui::Painter, r: Rect, accent: Color32, radius: f32) {
    for i in 1..=4 {
        p.rect_filled(r.expand(i as f32 * 2.5).translate(vec2(0.0, 5.0)), Rounding::same(radius + i as f32 * 2.5), Color32::from_black_alpha(14));
    }
    p.rect_filled(r, Rounding::same(radius), PANEL);
    p.rect_stroke(r, Rounding::same(radius), Stroke::new(1.5, alpha(accent, 150)));
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

/// A megaphone: "warn".
fn megaphone(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    let q = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    p.add(Shape::convex_polygon(vec![q(-0.55, -0.22), q(0.5, -0.7), q(0.5, 0.7), q(-0.55, 0.22)], col, Stroke::NONE));
    p.rect_filled(Rect::from_min_max(q(-0.85, -0.25), q(-0.5, 0.25)), Rounding::same(s * 0.08), col);
    p.line_segment([q(-0.45, 0.2), q(-0.3, 0.75)], Stroke::new(s * 0.18, col));
    for (i, r) in [0.95f32, 1.2].iter().enumerate() {
        let a0 = -0.5f32;
        let pts: Vec<Pos2> = (0..=6).map(|j| {
            let a = a0 + j as f32 / 6.0;
            pos2(c.x + 0.2 * s + a.cos() * r * s * 0.7, c.y + a.sin() * r * s * 0.7)
        }).collect();
        p.add(Shape::line(pts, Stroke::new(s * 0.1, alpha(col, 220 - i as u8 * 80))));
    }
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

fn fast_icon(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    for dx in [-0.45f32, 0.35] {
        p.add(Shape::convex_polygon(vec![pos2(c.x + (dx - 0.4) * s, c.y - s * 0.6), pos2(c.x + (dx + 0.45) * s, c.y), pos2(c.x + (dx - 0.4) * s, c.y + s * 0.6)], col, Stroke::NONE));
    }
}

fn house(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.rect_filled(Rect::from_center_size(pos2(c.x, c.y + s * 0.3), vec2(s * 1.2, s * 0.9)), Rounding::same(s * 0.08), col);
    p.add(Shape::convex_polygon(vec![pos2(c.x - s * 0.8, c.y - s * 0.1), pos2(c.x, c.y - s * 0.8), pos2(c.x + s * 0.8, c.y - s * 0.1)], mix(col, Color32::BLACK, 0.25), Stroke::NONE));
}

fn shield(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.add(Shape::convex_polygon(vec![pos2(c.x - s * 0.8, c.y - s * 0.7), pos2(c.x + s * 0.8, c.y - s * 0.7), pos2(c.x + s * 0.8, c.y + s * 0.1), pos2(c.x, c.y + s * 0.95), pos2(c.x - s * 0.8, c.y + s * 0.1)], col, Stroke::NONE));
    tick(p, c, s * 0.8, NAVY);
}

fn tick(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    let w = Stroke::new(s * 0.25, col);
    p.line_segment([pos2(c.x - s * 0.45, c.y), pos2(c.x - s * 0.08, c.y + s * 0.38)], w);
    p.line_segment([pos2(c.x - s * 0.08, c.y + s * 0.38), pos2(c.x + s * 0.5, c.y - s * 0.38)], w);
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

fn clock(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.circle_stroke(c, s * 0.8, Stroke::new(s * 0.18, col));
    p.line_segment([c, c + vec2(0.0, -s * 0.5)], Stroke::new(s * 0.16, col));
    p.line_segment([c, c + vec2(s * 0.35, s * 0.1)], Stroke::new(s * 0.16, col));
}

/// The speaker's face in an advisor bubble: a helmet, a sash, a weather sun.
fn portrait(p: &egui::Painter, c: Pos2, s: f32, who: Speaker) {
    let (bg, hat) = match who {
        Speaker::Fire => (Color32::from_rgb(180, 40, 40), AMBER),
        Speaker::Mayor => (PANEL_HI, Color32::from_rgb(40, 150, 80)),
        Speaker::Weather => (Color32::from_rgb(40, 110, 170), GOLD),
    };
    p.circle_filled(c, s, bg);
    p.circle_stroke(c, s, Stroke::new(2.0, alpha(INK, 180)));
    // face and shoulders
    p.circle_filled(c + vec2(0.0, -s * 0.12), s * 0.38, Color32::from_rgb(240, 200, 160));
    p.rect_filled(Rect::from_center_size(c + vec2(0.0, s * 0.62), vec2(s * 1.1, s * 0.6)), Rounding::same(s * 0.3), mix(bg, NAVY, 0.4));
    match who {
        Speaker::Fire => {
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, -s * 0.45), vec2(s * 0.95, s * 0.32)), Rounding { nw: s * 0.4, ne: s * 0.4, sw: 0.0, se: 0.0 }, hat);
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, -s * 0.3), vec2(s * 1.1, s * 0.08)), Rounding::same(s * 0.04), mix(hat, NAVY, 0.3));
        }
        Speaker::Mayor => {
            p.line_segment([c + vec2(-s * 0.45, s * 0.35), c + vec2(s * 0.45, s * 0.85)], Stroke::new(s * 0.18, hat));
            p.line_segment([c + vec2(-s * 0.45, s * 0.42), c + vec2(s * 0.45, s * 0.92)], Stroke::new(s * 0.08, INK));
        }
        Speaker::Weather => {
            p.circle_filled(c + vec2(s * 0.5, -s * 0.5), s * 0.28, hat);
        }
    }
}

/// The CIMA Foundation mark (white-on-transparent) with its name beside it.
fn logo(p: &egui::Painter, tex: Option<egui::TextureId>, at: Pos2, h: f32, caption: bool) {
    let Some(tex) = tex else { return };
    let w = h * 289.0 / 360.0;
    p.image(tex, Rect::from_min_size(at, vec2(w, h)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    if caption {
        txt(p, at + vec2(w + 12.0, h * 0.38), Align2::LEFT_CENTER, "CIMA Foundation", h * 0.30, INK);
        txt(p, at + vec2(w + 12.0, h * 0.68), Align2::LEFT_CENTER, "PROPAGATOR · modello di incendio", h * 0.19, GREY);
    }
}

// --- text helpers ------------------------------------------------------------
fn txt(p: &egui::Painter, pos: Pos2, a: Align2, s: &str, size: f32, col: Color32) -> Rect {
    p.text(pos, a, s, FontId::proportional(size), col)
}

fn big(p: &egui::Painter, pos: Pos2, a: Align2, s: &str, size: f32, col: Color32) -> Rect {
    // Stand-in for a bold face: a darker offset copy under the text.
    p.text(pos + vec2(0.0, size * 0.05), a, s, FontId::proportional(size), Color32::from_black_alpha(120));
    p.text(pos, a, s, FontId::proportional(size), col)
}

fn wrapped(p: &egui::Painter, at: Pos2, s: &str, size: f32, col: Color32, width: f32) -> f32 {
    let g = p.layout(s.to_string(), FontId::proportional(size), col, width);
    let h = g.size().y;
    p.galley(at, g, col);
    h
}

fn pill(p: &egui::Painter, centre: Pos2, label: &str, size: f32, fill: Color32, ink: Color32) -> Rect {
    let galley = p.layout_no_wrap(label.to_string(), FontId::proportional(size), ink);
    let r = Rect::from_center_size(centre, galley.size() + vec2(size * 1.6, size * 0.9));
    p.rect_filled(r.translate(vec2(0.0, 3.0)), Rounding::same(r.height() * 0.5), Color32::from_black_alpha(70));
    p.rect_filled(r, Rounding::same(r.height() * 0.5), fill);
    p.rect_filled(Rect::from_min_size(r.min + vec2(r.height() * 0.4, 3.0), vec2(r.width() - r.height() * 0.8, r.height() * 0.28)), Rounding::same(r.height() * 0.14), Color32::from_white_alpha(22));
    p.galley(r.center() - galley.size() * 0.5, galley, ink);
    r
}

/// A clickable pill button on an interactive area; returns whether it was clicked.
fn pill_button(ui: &mut egui::Ui, label: &str, size: f32, fill: Color32, ink: Color32) -> bool {
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(size), ink);
    let desired = galley.size() + vec2(size * 2.2, size * 1.0);
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
    let lift = if resp.hovered() { -2.0 } else { 0.0 };
    let r = rect.translate(vec2(0.0, lift));
    let p = ui.painter();
    p.rect_filled(rect.translate(vec2(0.0, 4.0)), Rounding::same(r.height() * 0.5), Color32::from_black_alpha(80));
    p.rect_filled(r, Rounding::same(r.height() * 0.5), if resp.hovered() { mix(fill, Color32::WHITE, 0.15) } else { fill });
    p.rect_filled(Rect::from_min_size(r.min + vec2(r.height() * 0.4, 3.0), vec2(r.width() - r.height() * 0.8, r.height() * 0.26)), Rounding::same(r.height() * 0.13), Color32::from_white_alpha(24));
    p.galley(r.center() - galley.size() * 0.5, galley, ink);
    resp.clicked()
}

// --- wind ---------------------------------------------------------------------
fn compass(p: &egui::Painter, c: Pos2, r: f32, to_deg: f32, yaw: f32, ghost_to: Option<f32>, k: f32) {
    p.circle_filled(c + vec2(0.0, 4.0), r, Color32::from_black_alpha(80));
    p.circle_filled(c, r, PANEL);
    p.circle_stroke(c, r, Stroke::new(2.0, alpha(SKY, 160)));
    // Tick marks turn with the camera; the N tells you which way is north.
    for i in 0..12 {
        let a = i as f32 * std::f32::consts::TAU / 12.0 + yaw;
        let (s, co) = a.sin_cos();
        let major = i % 3 == 0;
        let (r0, r1) = (r * if major { 0.78 } else { 0.86 }, r * 0.94);
        p.line_segment([c + vec2(s * r0, -co * r0), c + vec2(s * r1, -co * r1)], Stroke::new(if major { 2.5 } else { 1.2 }, alpha(MUTED, 160)));
    }
    let (sn, cn) = yaw.sin_cos();
    txt(p, c + vec2(sn * r * 0.6, -cn * r * 0.6), Align2::CENTER_CENTER, "N", r * 0.28, AMBER);
    let arrow = |to: f32, col: Color32, len: f32, dashed: bool| {
        let a = super::view::screen_angle(to, yaw);
        let (s, co) = a.sin_cos();
        let dir = vec2(s, -co);
        let side = vec2(co, s);
        let tip = c + dir * r * 0.72 * len;
        let tail = c - dir * r * 0.5 * len;
        if dashed {
            p.add(Shape::dashed_line(&[tail, c + dir * r * 0.3], Stroke::new(r * 0.07, col), r * 0.12, r * 0.08));
            p.add(Shape::convex_polygon(vec![tip, c + dir * r * 0.3 + side * r * 0.18, c + dir * r * 0.3 - side * r * 0.18], Color32::TRANSPARENT, Stroke::new(2.0, col)));
        } else {
            p.add(Shape::convex_polygon(vec![tip, c + dir * r * 0.18 + side * r * 0.28, c + dir * r * 0.18 - side * r * 0.28], FLAME, Stroke::new(1.5, AMBER)));
            p.add(Shape::convex_polygon(vec![c + dir * r * 0.2 + side * r * 0.1, tail + side * r * 0.1, tail - side * r * 0.1, c + dir * r * 0.2 - side * r * 0.1], mix(FLAME, NAVY, 0.3), Stroke::NONE));
        }
    };
    if let Some(g) = ghost_to {
        arrow(g, alpha(GREY, 170), 0.92, true);
    }
    arrow(to_deg, FLAME, 1.0 + (k * 4.0).sin() * 0.04, false);
    p.circle_filled(c, r * 0.07, INK);
}

/// The forecast, as a card: the wind with its uncertainty and the chance of a
/// change. Honest by layout -- it is labelled a forecast and shows odds, not a
/// verdict -- and kept apart from the compass, which shows what the wind *is*.
fn forecast_card(p: &egui::Painter, rect: Rect, f: &demo::Forecast, fresh: f32) {
    p.rect_filled(rect.translate(vec2(0.0, 4.0)), Rounding::same(16.0), Color32::from_black_alpha(80));
    p.rect_filled(rect, Rounding::same(16.0), PANEL);
    let rim = if fresh > 0.0 { mix(SKY, AMBER, fresh) } else { SKY };
    p.rect_stroke(rect, Rounding::same(16.0), Stroke::new(1.5 + fresh * 2.5, alpha(rim, 180)));
    let x = rect.left() + 16.0;
    let title = if f.issue > 1 { t::FORECAST_UPDATED } else { t::FORECAST };
    txt(p, pos2(x, rect.top() + 18.0), Align2::LEFT_CENTER, title, 16.0, rim);
    txt(p, pos2(x, rect.top() + 42.0), Align2::LEFT_CENTER, &t::forecast_wind(bearing_name(f.wind_from_deg), f.wind_kmh, f.cone_deg), 15.0, INK);
    txt(p, pos2(x, rect.top() + 66.0), Align2::LEFT_CENTER, t::SHIFT_CHANCE, 13.0, MUTED);
    let bar = Rect::from_min_size(pos2(x, rect.top() + 78.0), vec2(rect.width() - 86.0, 10.0));
    p.rect_filled(bar, Rounding::same(5.0), Color32::from_white_alpha(26));
    p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * f.shift_p, bar.height())), Rounding::same(5.0), AMBER);
    big(p, pos2(bar.right() + 10.0, bar.center().y), Align2::LEFT_CENTER, &format!("{:.0}%", f.shift_p * 100.0), 20.0, INK);
    txt(p, pos2(x, rect.top() + 102.0), Align2::LEFT_CENTER, &t::forecast_shift(bearing_name(f.shift_to_deg), f.shift_eta_min.0, f.shift_eta_min.1), 13.0, GREY);
    txt(p, pos2(x, rect.bottom() - 13.0), Align2::LEFT_CENTER, t::FORECAST_CAVEAT, 12.0, MUTED);
}

/// Moved to `crates/text`, shared with the `play` binary.
pub(crate) use text::bearing_name;

/// Width of the wind-and-forecast column on the right; map chips keep out of it.
const RIGHT_COL: f32 = 270.0;

/// Height of the action bar band at the bottom of the play screen.
const BAR_H: f32 = 104.0;

// --- state of one frame --------------------------------------------------------
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut settings: ResMut<EguiSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut kiosk: ResMut<Kiosk>,
    sim: Res<Sim>,
    mut tool: ResMut<OrderTool>,
    mut focus: ResMut<crate::ui::UiFocus>,
    cams: Query<(&Camera, &GlobalTransform, &crate::camera::OrbitCamera)>,
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
    let scale = settings.scale_factor;
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
    let cam = cams.get_single().ok();
    let yaw = cam.map_or(0.0, |c| c.2.yaw);
    // World -> egui points, for anything pinned to the map.
    let project = |p: scenario::Pos, lift: f32| -> Option<Pos2> {
        let (camera, gt, _) = cam?;
        let h = sim.scenario.terrain.height_at(p);
        let v = camera.world_to_viewport(gt, crate::frame::to_bevy(p, h + lift))?;
        Some(pos2(v.x / scale, v.y / scale))
    };

    // An order the map tool placed (the Canadair) goes through the books too.
    if let Some(o) = tool.issued.take() {
        kiosk.cmd.push_back(Cmd::Order(o));
    }

    // Hidden operator corner: hold the top-right corner for a few seconds.
    let corner = Rect::from_min_size(pos2(screen.right() - 70.0, screen.top()), vec2(70.0, 70.0));
    let held = ctx.input(|i| i.pointer.primary_down() && i.pointer.hover_pos().is_some_and(|p| corner.contains(p)));
    kiosk.corner_hold = if held { kiosk.corner_hold + time.delta_seconds() } else { 0.0 };
    if kiosk.corner_hold >= OPERATOR_HOLD_S {
        kiosk.operator_open = true;
        kiosk.corner_hold = 0.0;
    }
    let clicked_anywhere = ctx.input(|i| i.pointer.primary_clicked()) && !held;

    match kiosk.phase {
        Phase::Attract => attract(ctx, &mut kiosk, screen, k, clicked_anywhere),
        Phase::Briefing => {
            district_chips(ctx, &mut kiosk, &sim, &project, screen, k);
            briefing(ctx, &mut kiosk, &sim, screen, k, yaw);
        }
        Phase::Play => {
            district_chips(ctx, &mut kiosk, &sim, &project, screen, k);
            play(ctx, &mut kiosk, &sim, &mut tool, screen, k, yaw);
        }
        Phase::Outcome => outcome(ctx, &mut kiosk, &sim, screen, k),
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
    gradient(&p, Rect::from_min_size(screen.min, vec2(screen.width(), screen.height() * 0.36)), Color32::from_black_alpha(200), Color32::TRANSPARENT);
    gradient(&p, Rect::from_min_size(pos2(screen.left(), screen.bottom() - screen.height() * 0.34), vec2(screen.width(), screen.height() * 0.34)), Color32::TRANSPARENT, Color32::from_black_alpha(210));
    logo(&p, kiosk.logo, screen.left_top() + vec2(22.0, 18.0), 64.0, true);
    let c = pos2(screen.center().x, screen.top() + screen.height() * 0.2);
    let half = p.layout_no_wrap(t::TITLE.to_string(), FontId::proportional(54.0), INK).size().x * 0.5;
    flame(&p, c - vec2(half + 44.0, 0.0), 32.0, k);
    flame(&p, c + vec2(half + 44.0, 0.0), 32.0, k + 1.3);
    big(&p, c, Align2::CENTER_CENTER, t::TITLE, 54.0, INK);
    let sub = c + vec2(0.0, 52.0);
    let sw = p.layout_no_wrap(t::SUBTITLE.to_string(), FontId::proportional(21.0), AMBER).size().x;
    p.rect_filled(Rect::from_center_size(sub, vec2(sw + 40.0, 34.0)), Rounding::same(17.0), alpha(NAVY, 200));
    txt(&p, sub, Align2::CENTER_CENTER, t::SUBTITLE, 21.0, AMBER);
    let town = t::town(kiosk.spec.id);
    let cp = pos2(screen.center().x, screen.bottom() - screen.height() * 0.27);
    let cap = format!("{} · {}", town.name, town.place);
    let cw = p.layout_no_wrap(cap.clone(), FontId::proportional(22.0), INK).size().x;
    p.rect_filled(Rect::from_center_size(cp, vec2(cw + 40.0, 36.0)), Rounding::same(18.0), alpha(NAVY, 200));
    txt(&p, cp, Align2::CENTER_CENTER, &cap, 22.0, INK);
    txt(&p, cp + vec2(0.0, 36.0), Align2::CENTER_CENTER, t::TAGLINE, 17.0, GREY);
    let bob = (k * 3.0).sin() * 4.0;
    let grow = 1.0 + (k * 3.0).sin() * 0.03;
    pill(&p, pos2(screen.center().x, screen.bottom() - screen.height() * 0.11 + bob), t::START, 32.0 * grow, FLAME, Color32::WHITE);
    if clicked {
        kiosk.cmd.push_back(Cmd::Begin);
    }
}

fn briefing(ctx: &egui::Context, kiosk: &mut Kiosk, sim: &Sim, screen: Rect, k: f32, yaw: f32) {
    let town = t::town(kiosk.spec.id);
    let slide = 1.0 - ((kiosk.phase_t / 0.5).min(1.0) - 1.0).powi(2);
    // Wind and forecast, top right, where they will stay during play.
    wind_panel(&layer(ctx, "brief_wind"), kiosk, sim, screen, yaw, k, (1.0 - slide) * 60.0);
    // Town card, bottom left, clear of the districts at the centre of the frame.
    let size = vec2(430.0f32.min(screen.width() - 32.0), 236.0);
    let rect = Rect::from_min_size(pos2(16.0 - (1.0 - slide) * 80.0, screen.bottom() - size.y - 16.0), size);
    egui::Area::new("briefing".into()).fixed_pos(rect.min).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.set_min_size(size);
        let p = ui.painter().clone();
        card(&p, rect, FLAME, 20.0);
        let strip = Rect::from_min_size(rect.min, vec2(rect.width(), 58.0));
        p.rect_filled(strip.shrink(1.5), Rounding { nw: 19.0, ne: 19.0, sw: 0.0, se: 0.0 }, mix(FLAME, NAVY, 0.55));
        flame(&p, strip.left_center() + vec2(32.0, 0.0), 18.0, k);
        big(&p, strip.left_center() + vec2(60.0, -9.0), Align2::LEFT_CENTER, town.name, 26.0, INK);
        txt(&p, strip.left_center() + vec2(60.0, 15.0), Align2::LEFT_CENTER, town.place, 14.0, AMBER);
        let x = rect.left() + 20.0;
        let w = rect.width() - 40.0;
        let mut y = rect.top() + 68.0;
        y += wrapped(&p, pos2(x, y), town.brief, 15.0, INK, w) + 8.0;
        y += wrapped(&p, pos2(x, y), t::BRIEF_HOW, 13.5, GREY, w) + 6.0;
        let _ = y;
        let mut b = ui.child_ui(Rect::from_min_size(pos2(rect.right() - 170.0, rect.bottom() - 58.0), vec2(150.0, 46.0)), egui::Layout::right_to_left(egui::Align::Center), None);
        let pulse = 1.0 + (k * 4.0).sin().max(0.0) * 0.04;
        if pill_button(&mut b, t::GO, 24.0 * pulse, FLAME, Color32::WHITE) {
            b.ctx().request_repaint();
            kiosk.cmd.push_back(Cmd::Go);
        }
    });
    // The tip, as a thin strip along the bottom-right.
    let tip = layer(ctx, "brief_tip");
    let tr = Rect::from_min_max(pos2(rect.right() + 16.0, screen.bottom() - 60.0), pos2(screen.right() - 16.0, screen.bottom() - 16.0));
    if tr.width() > 240.0 {
        tip.rect_filled(tr, Rounding::same(14.0), alpha(NAVY, 200));
        tip.rect_filled(Rect::from_min_size(tr.min, vec2(6.0, tr.height())), Rounding { nw: 14.0, sw: 14.0, ne: 0.0, se: 0.0 }, AMBER);
        wrapped(&tip, tr.min + vec2(18.0, 7.0), t::BRIEF_TIP, 13.0, GREY, tr.width() - 30.0);
    }
}

/// Compass, the wind in words, and the forecast card under it.
fn wind_panel(p: &egui::Painter, kiosk: &Kiosk, sim: &Sim, screen: Rect, yaw: f32, k: f32, slide_x: f32) {
    let w = sim.fire.weather();
    let to = ((w.wind_dir_deg as f32) + 180.0) % 360.0;
    let now = sim.time_s();
    let f = kiosk.forecast_at(now);
    // A faint second arrow where the wind would turn, while a shift is forecast.
    let ghost = (kiosk.referee.as_ref().is_some_and(|r| !r.events.iter().any(|e| matches!(e.kind, demo::EventKind::WindShifted { .. }))) && f.shift_p >= 0.15)
        .then_some((f.shift_to_deg + 180.0) % 360.0);
    let r = 50.0;
    let c = pos2(screen.right() - 16.0 - r + slide_x, screen.top() + 84.0 + r);
    compass(p, c, r, to, yaw, ghost, k);
    let label = format!("{} {} · {:.0} km/h", t::WIND_FROM, bearing_name(w.wind_dir_deg as f32), w.wind_speed_kmh);
    let lw = p.layout_no_wrap(label.clone(), FontId::proportional(15.0), INK).size().x;
    let lr = Rect::from_center_size(c + vec2(0.0, r + 18.0), vec2(lw + 22.0, 26.0));
    p.rect_filled(lr, Rounding::same(13.0), PANEL);
    txt(p, lr.center(), Align2::CENTER_CENTER, &label, 15.0, INK);
    let fresh = if f.issue > 1 { (1.0 - ((now - demo::ISSUE_2_AT_S) as f32 / 300.0)).clamp(0.0, 1.0) * (0.5 + 0.5 * (k * 6.0).sin()) } else { 0.0 };
    let fc = Rect::from_min_size(pos2(screen.right() - 16.0 - RIGHT_COL + slide_x, lr.bottom() + 10.0), vec2(RIGHT_COL, 140.0));
    forecast_card(p, fc, &f, fresh);
}

/// Colour and words for a district's state.
fn level_style(r: &Report) -> (Color32, String) {
    match r.level() {
        Level::Reached => (RED, t::FIRE_HERE.to_string()),
        Level::Threatened => (FLAME, t::fire_at(r.fire_now_m)),
        Level::Watch => (AMBER, t::fire_at(r.fire_now_m)),
        Level::Calm => (GREEN, if r.fire_now_m.is_finite() { t::fire_at(r.fire_now_m) } else { t::CALM.to_string() }),
    }
}

/// The district chips: the game's main controls, pinned over each district.
fn district_chips(ctx: &egui::Context, kiosk: &mut Kiosk, sim: &Sim, project: &dyn Fn(scenario::Pos, f32) -> Option<Pos2>, screen: Rect, k: f32) {
    let Some(referee) = kiosk.referee.as_ref() else { return };
    let free_engines = sim.crews.units.iter().filter(|u| u.kind == UnitKind::Engine && u.assignable() && matches!(u.state, UnitState::Staged | UnitState::Inbound)).count();
    let mut orders: Vec<Order> = vec![];
    // The fire gets a label too: the first thing a newcomer has to find.
    {
        // The middle of what is burning, not the head: the head is often a spot
        // fire ahead of the front, which has its own ring and "!".
        let active = sim.fire.active_cells();
        let head = if active.is_empty() {
            kiosk.spec.ignition
        } else {
            let n = active.len() as f32;
            let (sx, sy) = active.iter().map(|c| sim.scenario.world.centre_of(*c)).fold((0.0, 0.0), |(a, b), p| (a + p.x, b + p.y));
            scenario::Pos { x: sx / n, y: sy / n }
        };
        if let Some(a) = project(head, 40.0) {
            let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("fire_tag")));
            let tag = pos2(a.x, (a.y - 54.0).max(screen.top() + 70.0));
            p.line_segment([tag, a], Stroke::new(2.5, alpha(FLAME, 220)));
            let r = pill(&p, tag, t::FIRE_TAG, 15.0, mix(RED, FLAME, 0.4), INK);
            flame(&p, r.left_center() + vec2(-2.0, 0.0), 11.0, k);
        }
    }
    let n = referee.districts.len();
    for i in 0..n {
        let d = &referee.districts[i];
        let r = referee.reports[i];
        let posted = referee.posted(i);
        let Some(anchor) = project(d.centre, 30.0) else { continue };
        let size = vec2(206.0, 100.0);
        // The chip floats above its district, held inside the play area.
        // Kept above the advisor band, which owns the bottom-left of the map.
        let top = (anchor.y - size.y - 46.0).clamp(screen.top() + 70.0, screen.bottom() - BAR_H - ADVISOR_BAND - size.y - 16.0);
        let left = (anchor.x - size.x * 0.5).clamp(screen.left() + 8.0, screen.right() - RIGHT_COL - size.x - 28.0);
        let rect = Rect::from_min_size(pos2(left, top), size);
        let (col, status) = level_style(&r);
        let pulse = if r.level() >= Level::Threatened { 0.5 + 0.5 * (k * 6.0).sin() } else { 0.0 };
        egui::Area::new(egui::Id::new(("district", i))).fixed_pos(rect.min).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_min_size(size);
            let p = ui.painter().clone();
            // Stem down to the district, and a dot on it.
            p.line_segment([pos2(rect.center().x.clamp(anchor.x - 60.0, anchor.x + 60.0), rect.bottom()), anchor], Stroke::new(2.5, alpha(col, 200)));
            p.circle_filled(anchor, 6.0 + pulse * 3.0, alpha(col, 220));
            p.circle_stroke(anchor, 9.0 + pulse * 6.0, Stroke::new(2.0, alpha(col, (120.0 * (1.0 - pulse * 0.5)) as u8)));
            card(&p, rect, col, 14.0);
            if pulse > 0.0 {
                p.rect_stroke(rect.expand(2.0 + pulse * 2.0), Rounding::same(16.0), Stroke::new(2.5, alpha(col, (90.0 + 120.0 * pulse) as u8)));
            }
            p.rect_filled(Rect::from_min_size(rect.min, vec2(7.0, rect.height())), Rounding { nw: 14.0, sw: 14.0, ne: 0.0, se: 0.0 }, col);
            big(&p, rect.min + vec2(16.0, 16.0), Align2::LEFT_CENTER, &d.name, 17.0, INK);
            txt(&p, rect.right_top() + vec2(-12.0, 16.0), Align2::RIGHT_CENTER, &t::households(r.households), 12.0, MUTED);
            txt(&p, rect.min + vec2(16.0, 37.0), Align2::LEFT_CENTER, &status, 14.0, col);
            // Two buttons: warn, defend.
            let bw = (size.x - 16.0 - 12.0 - 8.0) / 2.0;
            let b1 = Rect::from_min_size(rect.min + vec2(16.0, 52.0), vec2(bw, 38.0));
            let b2 = Rect::from_min_size(b1.right_top() + vec2(8.0, 0.0), vec2(bw, 38.0));
            let warned = r.warned_at_s.is_some();
            let r1 = ui.interact(b1, egui::Id::new(("warn", i)), if warned { egui::Sense::hover() } else { egui::Sense::click() });
            if warned {
                p.rect_filled(b1, Rounding::same(10.0), alpha(SKY, 40));
                let out = r.safe + r.moving;
                let frac = out as f32 / r.households.max(1) as f32;
                p.rect_filled(Rect::from_min_size(b1.min, vec2(b1.width() * frac, b1.height())), Rounding::same(10.0), alpha(SKY, 110));
                tick(&p, b1.left_center() + vec2(14.0, -1.0), 12.0, INK);
                txt(&p, b1.left_center() + vec2(26.0, -8.0), Align2::LEFT_CENTER, t::WARNED, 12.5, INK);
                txt(&p, b1.left_center() + vec2(26.0, 8.0), Align2::LEFT_CENTER, &t::leaving(out, r.households), 11.5, GREY);
            } else {
                let hot = r1.hovered();
                let fill = if hot { mix(RED, Color32::WHITE, 0.15) } else { mix(RED, NAVY, 0.15) };
                p.rect_filled(b1.translate(vec2(0.0, 3.0)), Rounding::same(10.0), Color32::from_black_alpha(70));
                p.rect_filled(b1.translate(vec2(0.0, if hot { -1.5 } else { 0.0 })), Rounding::same(10.0), fill);
                megaphone(&p, b1.left_center() + vec2(18.0, 0.0), 10.0, INK);
                txt(&p, b1.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, t::WARN, 16.0, INK);
                if r1.clicked() {
                    orders.push(Order::EvacuateDistrict(i));
                }
            }
            let can_defend = free_engines > 0;
            let r2 = ui.interact(b2, egui::Id::new(("defend", i)), if can_defend { egui::Sense::click() } else { egui::Sense::hover() });
            let hot = r2.hovered() && can_defend;
            let fill = if !can_defend { Color32::from_rgb(40, 46, 60) } else if hot { mix(FLAME, Color32::WHITE, 0.15) } else { mix(FLAME, NAVY, 0.2) };
            p.rect_filled(b2.translate(vec2(0.0, 3.0)), Rounding::same(10.0), Color32::from_black_alpha(70));
            p.rect_filled(b2.translate(vec2(0.0, if hot { -1.5 } else { 0.0 })), Rounding::same(10.0), fill);
            engine(&p, b2.left_center() + vec2(18.0, 0.0), 9.0, if can_defend { INK } else { MUTED });
            txt(&p, b2.left_center() + vec2(34.0, if posted > 0 { -8.0 } else { 0.0 }), Align2::LEFT_CENTER, t::DEFEND, 15.0, if can_defend { INK } else { MUTED });
            if posted > 0 {
                txt(&p, b2.left_center() + vec2(34.0, 9.0), Align2::LEFT_CENTER, &t::engines_posted(posted), 11.0, AMBER);
            }
            if r2.clicked() {
                orders.push(Order::Defend { kind: UnitKind::Engine, district: i });
            }
        });
    }
    for o in orders {
        kiosk.cmd.push_back(Cmd::Order(o));
    }
}

/// One compact counter: icon, number, label.
fn counter(p: &egui::Painter, at: Pos2, w: f32, label: &str, value: usize, col: Color32, icon: impl Fn(&egui::Painter, Pos2, f32), pulse: f32) {
    let r = Rect::from_min_size(at, vec2(w, 46.0));
    p.rect_filled(r, Rounding::same(14.0), PANEL);
    if pulse > 0.0 {
        p.rect_stroke(r, Rounding::same(14.0), Stroke::new(2.5, alpha(col, (pulse * 255.0) as u8)));
    }
    p.circle_filled(r.left_center() + vec2(23.0, 0.0), 16.0, alpha(col, 45));
    icon(p, r.left_center() + vec2(23.0, 0.0), 10.5);
    big(p, r.left_center() + vec2(46.0, -7.0), Align2::LEFT_CENTER, &value.to_string(), 22.0, INK);
    txt(p, r.left_center() + vec2(46.0, 13.0), Align2::LEFT_CENTER, label, 12.0, col);
}

#[allow(clippy::too_many_arguments)]
fn play(ctx: &egui::Context, kiosk: &mut Kiosk, sim: &Sim, tool: &mut OrderTool, screen: Rect, k: f32, yaw: f32) {
    let p = layer(ctx, "hud");
    let Some(referee) = kiosk.referee.as_ref() else { return };
    let stats = sim.agents.stats();
    let outcome = referee.tally.outcome(&sim.agents, &sim.fire, &sim.scenario.world);

    // Top bar: town and clock, then the four counters, then money and trust.
    let town = t::town(kiosk.spec.id);
    let tl = Rect::from_min_size(screen.left_top() + vec2(16.0, 12.0), vec2(250.0, 46.0));
    p.rect_filled(tl, Rounding::same(14.0), PANEL);
    big(&p, tl.left_center() + vec2(14.0, -8.0), Align2::LEFT_CENTER, town.name, 17.0, INK);
    let now = sim.time_s();
    let left_s = (kiosk.spec.duration_s - now).max(0);
    clock(&p, tl.left_center() + vec2(20.0, 12.0), 7.0, AMBER);
    txt(&p, tl.left_center() + vec2(32.0, 12.0), Align2::LEFT_CENTER, &format!("T+{} min", now / 60), 13.0, AMBER);
    txt(&p, tl.right_center() + vec2(-14.0, 12.0), Align2::RIGHT_CENTER, &format!("{} {:02}:{:02}", t::TIME_LEFT, left_s / 60, left_s % 60), 12.0, MUTED);
    let frac = (now as f32 / kiosk.spec.duration_s as f32).clamp(0.0, 1.0);
    let bar = Rect::from_min_size(tl.left_bottom() + vec2(14.0, -5.0), vec2(tl.width() - 28.0, 3.0));
    p.rect_filled(bar, Rounding::same(1.5), Color32::from_white_alpha(24));
    p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * frac, bar.height())), Rounding::same(1.5), mix(AMBER, FLAME, frac));

    let cw = 132.0;
    let x0 = tl.right() + 10.0;
    counter(&p, pos2(x0, tl.top()), cw, t::SAFE, stats.safe, GREEN, |p, c, s| shield(p, c, s, GREEN), 0.0);
    counter(&p, pos2(x0 + cw + 8.0, tl.top()), cw, t::MOVING, stats.moving, SKY, |p, c, s| runner(p, c, s, SKY, k), 0.0);
    let danger_pulse = if outcome.in_danger > 0 { 0.5 + 0.5 * (k * 6.0).sin() } else { 0.0 };
    counter(&p, pos2(x0 + 2.0 * (cw + 8.0), tl.top()), cw, t::DANGER, outcome.in_danger, AMBER, |p, c, s| warning(p, c, s, AMBER), danger_pulse * 0.9);
    counter(&p, pos2(x0 + 3.0 * (cw + 8.0), tl.top()), cw, t::HOMES_LOST, outcome.homes_lost, RED, |p, c, s| house(p, c, s, RED), 0.0);

    // Money and trust, under the clock.
    let spent = referee.ledger(now).total_eur();
    let mr = Rect::from_min_size(tl.left_bottom() + vec2(0.0, 8.0), vec2(250.0, 30.0));
    p.rect_filled(mr, Rounding::same(12.0), alpha(NAVY, 200));
    txt(&p, mr.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, &format!("{} {}", t::SPENT, t::spent_eur(spent)), 13.0, GREY);
    logo(&p, kiosk.logo, pos2(16.0, mr.bottom() + 10.0), 40.0, false);
    wind_panel(&p, kiosk, sim, screen, yaw, k, 0.0);

    // Advisor bubble, bottom left above the bar.
    if let Some((who, text, left)) = kiosk.advisor.clone() {
        let fade = (left.min(ADVISOR_FADE) / ADVISOR_FADE).clamp(0.0, 1.0);
        let w = 470.0f32.min(screen.width() * 0.45);
        let a = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("advisor")));
        let g = a.layout(text.clone(), FontId::proportional(15.5), alpha(NAVY, (255.0 * fade) as u8), w - 92.0);
        let h = g.size().y + 40.0;
        let r = Rect::from_min_size(pos2(16.0, screen.bottom() - BAR_H - h - 18.0), vec2(w, h.max(64.0)));
        a.rect_filled(r.translate(vec2(0.0, 4.0)), Rounding::same(16.0), Color32::from_black_alpha((70.0 * fade) as u8));
        a.rect_filled(r, Rounding::same(16.0), alpha(Color32::from_rgb(250, 248, 240), (245.0 * fade) as u8));
        portrait(&a, r.left_center() + vec2(36.0, 0.0), 24.0, who);
        txt(&a, r.min + vec2(72.0, 14.0), Align2::LEFT_CENTER, t::speaker(who), 13.0, alpha(Color32::from_rgb(0, 64, 112), (255.0 * fade) as u8));
        a.galley(r.min + vec2(72.0, 26.0), g, alpha(NAVY, (255.0 * fade) as u8));
    }

    // Banner: a pending drop, or the last refusal.
    if let Some(msg) = tool.refusal.take() {
        kiosk.say(t::refusal(&msg));
    }
    if tool.confirmation.take().is_some() {
        kiosk.banner = None;
    }
    let banner_y = screen.bottom() - BAR_H - 46.0;
    let mut cancel = false;
    egui::Area::new("banner".into()).fixed_pos(pos2(screen.center().x - 300.0, banner_y - 24.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.set_width(600.0);
        let (text, col) = if tool.is_armed() {
            (Some(t::PENDING_DROP.to_string()), SKY)
        } else if let Some((m, _)) = &kiosk.banner {
            (Some(m.clone()), AMBER)
        } else {
            (None, FLAME)
        };
        if let Some(text) = text {
            let r = pill(ui.painter(), pos2(screen.center().x, banner_y), &text, 18.0, col, NAVY);
            if tool.is_armed() {
                let x = Rect::from_center_size(pos2(r.right() + 28.0, r.center().y), vec2(40.0, 40.0));
                let resp = ui.interact(x, egui::Id::new("cancel"), egui::Sense::click());
                ui.painter().circle_filled(x.center(), 17.0, if resp.hovered() { RED } else { mix(RED, NAVY, 0.4) });
                let s = Stroke::new(3.0, Color32::WHITE);
                ui.painter().line_segment([x.center() + vec2(-6.0, -6.0), x.center() + vec2(6.0, 6.0)], s);
                ui.painter().line_segment([x.center() + vec2(6.0, -6.0), x.center() + vec2(-6.0, 6.0)], s);
                cancel = resp.clicked();
            }
        }
    });
    if cancel {
        tool.disarm();
    }

    action_bar(ctx, kiosk, sim, tool, screen, k);

    // "Are you still there?"
    if kiosk.idle_s >= super::play_idle_warn_s() && kiosk.ordered_at_s.is_none() && !kiosk.paused {
        let left = (super::play_idle_warn_s() + super::play_idle_grace_s() - kiosk.idle_s).max(0.0);
        let q = layer(ctx, "still");
        q.rect_filled(screen, Rounding::ZERO, Color32::from_black_alpha(150));
        big(&q, screen.center() - vec2(0.0, 30.0), Align2::CENTER_CENTER, t::STILL_THERE, 56.0, INK);
        let ring = left / super::play_idle_grace_s();
        let c = screen.center() + vec2(0.0, 60.0);
        q.circle_stroke(c, 26.0, Stroke::new(5.0, Color32::from_white_alpha(40)));
        let pts: Vec<Pos2> = (0..=48).map(|j| {
            let a = -std::f32::consts::FRAC_PI_2 + ring * std::f32::consts::TAU * j as f32 / 48.0;
            c + vec2(a.cos(), a.sin()) * 26.0
        }).collect();
        q.add(Shape::line(pts, Stroke::new(5.0, AMBER)));
        txt(&q, c + vec2(0.0, 50.0), Align2::CENTER_CENTER, t::STILL_THERE_SUB, 20.0, AMBER);
    }
}

/// Height kept clear above the action bar for the advisor bubble.
const ADVISOR_BAND: f32 = 96.0;

/// Seconds an advisor bubble takes to fade out at the end of its time.
const ADVISOR_FADE: f32 = 0.6;

#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Ready,
    Armed,
    Off,
    Done,
}

#[allow(clippy::too_many_arguments)]
fn action_button(ui: &mut egui::Ui, w: f32, title: &str, sub: &str, state: Btn, accent: Color32, k: f32, icon: impl Fn(&egui::Painter, Pos2, f32)) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 76.0), egui::Sense::click());
    let hot = resp.hovered() && matches!(state, Btn::Ready | Btn::Armed);
    let r = rect.translate(vec2(0.0, if hot { -3.0 } else { 0.0 }));
    let p = ui.painter();
    p.rect_filled(rect.translate(vec2(0.0, 4.0)), Rounding::same(16.0), Color32::from_black_alpha(90));
    let body = match state {
        Btn::Off | Btn::Done => Color32::from_rgb(28, 34, 54),
        _ => mix(accent, NAVY, 0.35),
    };
    p.rect_filled(r, Rounding::same(16.0), body);
    if matches!(state, Btn::Ready | Btn::Armed) {
        p.rect_filled(Rect::from_min_size(r.min + vec2(10.0, 3.0), vec2(r.width() - 20.0, 10.0)), Rounding::same(5.0), Color32::from_white_alpha(28));
    }
    if state == Btn::Armed {
        let pulse = 0.5 + 0.5 * (k * 7.0).sin();
        p.rect_stroke(r.expand(3.0 + pulse * 2.0), Rounding::same(19.0), Stroke::new(3.0, alpha(AMBER, (120.0 + pulse * 135.0) as u8)));
    } else if hot {
        p.rect_stroke(r, Rounding::same(16.0), Stroke::new(2.0, INK));
    }
    let dim = matches!(state, Btn::Off | Btn::Done);
    let tint = if dim { MUTED } else { INK };
    p.circle_filled(r.left_center() + vec2(30.0, 0.0), 21.0, Color32::from_black_alpha(if dim { 40 } else { 60 }));
    icon(p, r.left_center() + vec2(30.0, 0.0), 14.0);
    txt(p, r.left_center() + vec2(58.0, -9.0), Align2::LEFT_CENTER, title, 17.0, tint);
    txt(p, r.left_center() + vec2(58.0, 12.0), Align2::LEFT_CENTER, sub, 12.5, if dim { alpha(MUTED, 180) } else { alpha(INK, 210) });
    if state == Btn::Done {
        p.circle_filled(r.right_top() + vec2(-14.0, 14.0), 10.0, GREEN);
        tick(p, r.right_top() + vec2(-14.0, 14.0), 9.0, NAVY);
    }
    resp.clicked() && matches!(state, Btn::Ready | Btn::Armed)
}

fn action_bar(ctx: &egui::Context, kiosk: &mut Kiosk, sim: &Sim, tool: &mut OrderTool, screen: Rect, k: f32) {
    let widths = [196.0, 190.0, 176.0, 150.0, 150.0];
    let gap = 10.0;
    let total = widths.iter().sum::<f32>() + gap * (widths.len() as f32 - 1.0) + 28.0;
    let pos = pos2(screen.center().x - total * 0.5, screen.bottom() - BAR_H);
    let all_warned = kiosk.referee.as_ref().is_some_and(|r| r.reports.iter().all(|r| r.warned_at_s.is_some()));
    egui::Area::new("actions".into()).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
        let bar = Rect::from_min_size(pos, vec2(total, 96.0));
        card(ui.painter(), bar, SKY, 20.0);
        let mut inner = ui.child_ui(bar.shrink2(vec2(14.0, 10.0)), egui::Layout::left_to_right(egui::Align::Min), None);
        inner.spacing_mut().item_spacing = vec2(gap, 0.0);
        let ui = &mut inner;

        // General alarm: every district at once (with its false alarms).
        let state = if all_warned { Btn::Done } else { Btn::Ready };
        if action_button(ui, widths[0], t::ACT_EVACUATE, if all_warned { t::ACT_EVACUATE_DONE } else { t::ACT_EVACUATE_SUB }, state, RED, k, |p, c, s| siren(p, c, s, if all_warned { MUTED } else { Color32::WHITE }, k)) {
            kiosk.cmd.push_back(Cmd::Order(Order::EvacuateAll));
            tool.disarm();
        }

        // Engines: the count; the order itself is "Difendi" on a district.
        let free = sim.crews.units.iter().filter(|u| u.kind == UnitKind::Engine && u.assignable() && matches!(u.state, UnitState::Staged | UnitState::Inbound)).count();
        let state = if free > 0 { Btn::Ready } else { Btn::Off };
        if action_button(ui, widths[1], t::ACT_ENGINE, &t::free_engines(free), state, FLAME, k, |p, c, s| engine(p, c, s, Color32::WHITE)) {
            kiosk.say(t::ACT_ENGINE_HINT);
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
        if action_button(ui, widths[2], t::ACT_AIR, &sub, state, SKY, k, |p, c, s| plane(p, c, s, Color32::WHITE)) {
            if armed {
                tool.disarm();
            } else if let Some(id) = demo::run::best_unit(&sim.crews, UnitKind::AirTanker).or_else(|| sim.crews.units.iter().find(|u| u.kind.is_air()).map(|u| u.id)) {
                tool.disarm();
                tool.selected = Some(id);
                tool.toggle(OrderKind::Drop);
            }
        }

        let paused = kiosk.paused;
        let (title, sub) = if paused { (t::ACT_RESUME, t::PAUSED) } else { (t::ACT_PAUSE, t::ACT_PAUSE_SUB) };
        if action_button(ui, widths[3], title, sub, Btn::Ready, MUTED, k, |p, c, s| pause_icon(p, c, s, Color32::WHITE, paused)) {
            kiosk.paused = !paused;
        }
        let fast = kiosk.fast;
        if action_button(ui, widths[4], t::ACT_FAST, if fast { t::ACT_FAST_ON } else { t::ACT_FAST_SUB }, if fast { Btn::Armed } else { Btn::Ready }, PANEL_HI, k, |p, c, s| fast_icon(p, c, s, Color32::WHITE)) {
            kiosk.fast = !fast;
        }
    });
}

fn outcome(ctx: &egui::Context, kiosk: &mut Kiosk, sim: &Sim, screen: Rect, k: f32) {
    let Some(res) = kiosk.result else { return };
    let Some(referee) = kiosk.referee.as_ref() else { return };
    let appear = ((kiosk.phase_t - 0.2) / 0.5).clamp(0.0, 1.0);
    let w = 500.0f32.min(screen.width() * 0.48);
    let rect = Rect::from_min_max(pos2(screen.right() - w - 16.0 + (1.0 - appear) * 60.0, 16.0), pos2(screen.right() - 16.0 + (1.0 - appear) * 60.0, screen.bottom() - 16.0));
    // A soft shade on the panel side only, so the burnt town stays in view.
    hgradient(&layer(ctx, "dim"), Rect::from_min_max(pos2(screen.right() - w * 1.6, screen.top()), screen.max), Color32::TRANSPARENT, Color32::from_black_alpha(110));
    let names = kiosk.district_names();
    let reports = referee.reports.clone();
    let badges = demo::district::badges(&reports);
    let spent = referee.ledger(sim.time_s()).total_eur();
    let twin = kiosk.twin.clone();
    let saved = twin.as_ref().map(|tw| tw.outcome.caught as i64 - res.caught as i64);
    let mut cmd: Option<Cmd> = None;
    egui::Area::new("outcome".into()).fixed_pos(rect.min).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.set_min_size(rect.size());
        let p = ui.painter().clone();
        card(&p, rect, GREEN, 22.0);
        let x = rect.left() + 22.0;
        let iw = rect.width() - 44.0;
        // Headline.
        let head = Rect::from_min_size(rect.min, vec2(rect.width(), 64.0));
        p.rect_filled(head.shrink(1.5), Rounding { nw: 21.0, ne: 21.0, sw: 0.0, se: 0.0 }, mix(GREEN, NAVY, 0.62));
        shield(&p, head.left_center() + vec2(34.0, 0.0), 16.0, GREEN);
        big(&p, head.left_center() + vec2(62.0, -8.0), Align2::LEFT_CENTER, t::headline(res.caught, saved), 25.0, INK);
        txt(&p, head.left_center() + vec2(62.0, 16.0), Align2::LEFT_CENTER, &format!("{} · {}", t::OUTCOME_TITLE, t::town(kiosk.spec.id).name), 13.0, GREY);
        flame(&p, head.right_center() - vec2(30.0, 0.0), 16.0, k);

        // The big number and the twin beside it.
        let mut y = head.bottom() + 16.0;
        let nr = big(&p, pos2(x, y + 16.0), Align2::LEFT_CENTER, &format!("{}/{}", res.secure(), res.households), 32.0, INK);
        txt(&p, pos2(nr.right() + 10.0, y + 20.0), Align2::LEFT_CENTER, t::FAMILIES_SAFE, 15.0, GREEN);
        y += 38.0;
        let bar = Rect::from_min_size(pos2(x, y), vec2(iw, 8.0));
        p.rect_filled(bar, Rounding::same(4.0), Color32::from_white_alpha(26));
        p.rect_filled(Rect::from_min_size(bar.min, vec2(iw * res.secure() as f32 / res.households.max(1) as f32 * appear, 8.0)), Rounding::same(4.0), GREEN);
        y += 18.0;
        let caught_col = if res.caught == 0 { GREEN } else { AMBER };
        txt(&p, pos2(x, y + 9.0), Align2::LEFT_CENTER, &t::caught_line(res.caught), 15.0, caught_col);
        match (&twin, kiosk.twin_failed) {
            (Some(tw), _) => {
                txt(&p, pos2(x + iw, y + 9.0), Align2::RIGHT_CENTER, &format!("{}: {}", t::WITHOUT_ORDERS, tw.outcome.caught), 15.0, MUTED);
                y += 24.0;
                let s = saved.unwrap_or(0);
                let col = if s > 0 { GREEN } else if s == 0 { GREY } else { RED };
                big(&p, pos2(x, y + 10.0), Align2::LEFT_CENTER, &t::saved_vs_none(s), 17.0, col);
            }
            (None, true) => {
                txt(&p, pos2(x + iw, y + 9.0), Align2::RIGHT_CENTER, t::TWIN_NA, 13.0, MUTED);
                y += 24.0;
            }
            (None, false) => {
                y += 24.0;
                for j in 0..8 {
                    let a = k * 5.0 + j as f32 * 0.785;
                    p.circle_filled(pos2(x + 10.0, y + 10.0) + vec2(a.cos(), a.sin()) * 8.0, 2.5 - j as f32 * 0.2, alpha(INK, 255 - j * 25));
                }
                txt(&p, pos2(x + 28.0, y + 10.0), Align2::LEFT_CENTER, t::TWIN_WAIT, 13.0, MUTED);
            }
        }
        y += 30.0;

        // One line per district: what happened there and why.
        for (i, r) in reports.iter().enumerate() {
            let (line, mood) = t::district_story(r);
            let col = match mood {
                t::Mood::Good => GREEN,
                t::Mood::Meh => AMBER,
                t::Mood::Bad => RED,
            };
            let row = Rect::from_min_size(pos2(x, y), vec2(iw, 52.0));
            p.rect_filled(row, Rounding::same(12.0), alpha(col, 22));
            p.rect_filled(Rect::from_min_size(row.min, vec2(5.0, row.height())), Rounding { nw: 12.0, sw: 12.0, ne: 0.0, se: 0.0 }, col);
            big(&p, row.min + vec2(16.0, 14.0), Align2::LEFT_CENTER, names.get(i).map_or("", |s| s.as_str()), 15.0, INK);
            let twin_c = twin.as_ref().and_then(|tw| tw.districts.get(i)).map(|d| d.caught);
            let right = match twin_c {
                Some(c) => format!("{} · {} {}", t::caught_line(r.caught), t::WITHOUT_ORDERS, c),
                None => t::caught_line(r.caught),
            };
            txt(&p, row.right_top() + vec2(-12.0, 14.0), Align2::RIGHT_CENTER, &right, 12.0, if r.caught > 0 { AMBER } else { MUTED });
            txt(&p, row.min + vec2(16.0, 36.0), Align2::LEFT_CENTER, &line, 12.5, GREY);
            y += 58.0;
        }
        y += 4.0;

        // Medals: facts about the session, earned or not.
        txt(&p, pos2(x, y + 8.0), Align2::LEFT_CENTER, t::BADGES, 13.0, MUTED);
        y += 20.0;
        let medals = [
            (badges.in_time, t::BADGE_IN_TIME, t::BADGE_IN_TIME_HINT, 0),
            (badges.no_false_alarm, t::BADGE_NO_FALSE, t::BADGE_NO_FALSE_HINT, 1),
            (badges.homes_defended, t::BADGE_DEFENDED, t::BADGE_DEFENDED_HINT, 2),
        ];
        let mw = iw / 3.0;
        for (j, (got, name, hint, icon)) in medals.iter().enumerate() {
            let c = pos2(x + mw * (j as f32 + 0.5), y + 26.0);
            let pop = (appear * 3.0 - j as f32 * 0.4).clamp(0.0, 1.0);
            let r = 22.0 * (0.6 + 0.4 * pop);
            if *got {
                p.circle_filled(c, r + 4.0 + (k * 3.0 + j as f32).sin() * 1.5, alpha(GOLD, 60));
                p.circle_filled(c, r, GOLD);
                p.circle_stroke(c, r, Stroke::new(2.0, Color32::from_rgb(200, 140, 30)));
            } else {
                p.circle_filled(c, r, Color32::from_white_alpha(18));
                p.circle_stroke(c, r, Stroke::new(1.5, alpha(MUTED, 120)));
            }
            let ink = if *got { NAVY } else { alpha(MUTED, 160) };
            match icon {
                0 => clock(&p, c, r * 0.7, ink),
                1 => megaphone(&p, c, r * 0.55, ink),
                _ => house(&p, c, r * 0.6, ink),
            }
            txt(&p, c + vec2(0.0, r + 12.0), Align2::CENTER_CENTER, name, 13.0, if *got { GOLD } else { MUTED });
            if !*got {
                let g = p.layout(hint.to_string(), FontId::proportional(10.5), alpha(MUTED, 200), mw - 8.0);
                let gs = g.size();
                p.galley(c + vec2(-gs.x * 0.5, r + 22.0), g, alpha(MUTED, 200));
            }
        }
        y += 108.0;

        // The lesson, and the bill.
        let any_reached = reports.iter().any(|r| r.reached_at_s.is_some());
        let any_needless = reports.iter().any(|r| r.needless());
        y += wrapped(&p, pos2(x, y), t::lesson(badges, any_reached, any_needless), 15.0, AMBER, iw) + 6.0;
        txt(&p, pos2(x, y + 8.0), Align2::LEFT_CENTER, &format!("{} {} · {}", t::SPENT, t::spent_eur(spent), t::EVAC_SAVES_PEOPLE), 11.5, MUTED);

        logo(&p, kiosk.logo, rect.left_bottom() + vec2(20.0, -58.0), 40.0, false);
        let mut b = ui.child_ui(Rect::from_min_size(pos2(rect.right() - 380.0, rect.bottom() - 62.0), vec2(360.0, 50.0)), egui::Layout::right_to_left(egui::Align::Center), None);
        if pill_button(&mut b, t::ANOTHER_TOWN, 18.0, PANEL_HI, INK) {
            cmd = Some(Cmd::NextTown);
        }
        if pill_button(&mut b, t::RETRY, 18.0, FLAME, Color32::WHITE) {
            cmd = Some(Cmd::Retry);
        }
    });
    if let Some(c) = cmd {
        kiosk.cmd.push_back(c);
    }
}

fn operator(ctx: &egui::Context, kiosk: &mut Kiosk, screen: Rect) {
    egui::Window::new("Operatore").anchor(Align2::RIGHT_TOP, vec2(-16.0, 16.0)).collapsible(false).show(ctx, |ui| {
        ui.checkbox(&mut kiosk.pinned, "Resta su questo paese");
        if ui.button("Prossimo paese").clicked() {
            kiosk.cmd.push_back(Cmd::NextTown);
        }
        if ui.button("Ricomincia").clicked() {
            kiosk.cmd.push_back(Cmd::Begin);
        }
        if ui.button("Chiudi").clicked() {
            kiosk.operator_open = false;
        }
        ui.separator();
        if ui.button("Esci dal programma").clicked() {
            kiosk.cmd.push_back(Cmd::Quit);
        }
        let _ = screen;
    });
}
