//! Pictograms drawn with the egui painter, so the interface can say with a
//! shape what it used to say with a sentence. No font glyphs: the bundled
//! font has no emoji or arrows, and a missing glyph draws as a box.

use bevy_egui::egui::{self, pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Fire,
    House,
    Family,
    Engine,
    Crew,
    Shield,
    Bell,
    Exit,
    Cross,
    Up,
    Home,
    Plus,
    Minus,
    Pause,
    Play,
    Fast,
    Alert,
    Check,
}

fn poly(p: &Painter, r: Rect, pts: &[(f32, f32)], fill: Color32) {
    let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| pos2(r.left() + x * r.width(), r.top() + y * r.height())).collect();
    p.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
}

fn at(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.left() + x * r.width(), r.top() + y * r.height())
}

/// Draw `icon` filling the square `r`, in `col`.
pub fn draw(p: &Painter, r: Rect, icon: Icon, col: Color32) {
    let s = r.width().min(r.height());
    let r = Rect::from_center_size(r.center(), vec2(s, s));
    let w = (s * 0.11).max(2.0);
    let st = Stroke::new(w, col);
    match icon {
        Icon::Fire => {
            poly(p, r, &[(0.5, 0.02), (0.78, 0.38), (0.86, 0.66), (0.68, 0.95), (0.32, 0.95), (0.14, 0.66), (0.22, 0.38)], col);
            poly(p, r, &[(0.5, 0.42), (0.66, 0.66), (0.6, 0.9), (0.4, 0.9), (0.34, 0.66)], Color32::from_rgb(255, 225, 120));
        }
        Icon::House => {
            poly(p, r, &[(0.5, 0.08), (0.95, 0.48), (0.05, 0.48)], col);
            p.rect_filled(Rect::from_min_max(at(r, 0.17, 0.46), at(r, 0.83, 0.92)), 1.0, col);
        }
        Icon::Family => {
            for (x, h) in [(0.32, 1.0), (0.7, 0.75)] {
                let head = at(r, x, 0.62 - 0.4 * h);
                p.circle_filled(head, s * 0.11 * h.max(0.8), col);
                p.rect_filled(Rect::from_min_max(at(r, x - 0.14 * h, 0.72 - 0.38 * h + 0.12), at(r, x + 0.14 * h, 0.95)), s * 0.08, col);
            }
        }
        Icon::Engine => {
            p.rect_filled(Rect::from_min_max(at(r, 0.04, 0.3), at(r, 0.66, 0.74)), 2.0, col);
            p.rect_filled(Rect::from_min_max(at(r, 0.68, 0.44), at(r, 0.96, 0.74)), 2.0, col);
            for x in [0.22, 0.78] {
                p.circle_filled(at(r, x, 0.8), s * 0.11, col);
                p.circle_filled(at(r, x, 0.8), s * 0.05, Color32::from_gray(20));
            }
        }
        Icon::Crew => {
            p.circle_filled(at(r, 0.5, 0.22), s * 0.15, col);
            p.rect_filled(Rect::from_min_max(at(r, 0.3, 0.4), at(r, 0.7, 0.95)), s * 0.1, col);
        }
        Icon::Shield => {
            poly(p, r, &[(0.15, 0.1), (0.85, 0.1), (0.85, 0.5), (0.5, 0.95), (0.15, 0.5)], col);
        }
        Icon::Bell => {
            poly(p, r, &[(0.5, 0.08), (0.72, 0.2), (0.78, 0.68), (0.9, 0.78), (0.1, 0.78), (0.22, 0.68), (0.28, 0.2)], col);
            p.circle_filled(at(r, 0.5, 0.88), s * 0.09, col);
        }
        Icon::Exit => {
            p.rect_stroke(Rect::from_min_max(at(r, 0.08, 0.1), at(r, 0.5, 0.9)), 1.0, st);
            p.line_segment([at(r, 0.32, 0.5), at(r, 0.92, 0.5)], st);
            p.line_segment([at(r, 0.92, 0.5), at(r, 0.74, 0.32)], st);
            p.line_segment([at(r, 0.92, 0.5), at(r, 0.74, 0.68)], st);
        }
        Icon::Cross => {
            p.line_segment([at(r, 0.2, 0.2), at(r, 0.8, 0.8)], st);
            p.line_segment([at(r, 0.8, 0.2), at(r, 0.2, 0.8)], st);
        }
        Icon::Up => {
            p.line_segment([at(r, 0.2, 0.62), at(r, 0.5, 0.28)], st);
            p.line_segment([at(r, 0.5, 0.28), at(r, 0.8, 0.62)], st);
            p.line_segment([at(r, 0.2, 0.86), at(r, 0.8, 0.86)], st);
        }
        Icon::Home => {
            p.line_segment([at(r, 0.1, 0.5), at(r, 0.5, 0.12)], st);
            p.line_segment([at(r, 0.5, 0.12), at(r, 0.9, 0.5)], st);
            p.rect_stroke(Rect::from_min_max(at(r, 0.22, 0.48), at(r, 0.78, 0.9)), 1.0, st);
        }
        Icon::Plus => {
            p.line_segment([at(r, 0.2, 0.5), at(r, 0.8, 0.5)], st);
            p.line_segment([at(r, 0.5, 0.2), at(r, 0.5, 0.8)], st);
        }
        Icon::Minus => {
            p.line_segment([at(r, 0.2, 0.5), at(r, 0.8, 0.5)], st);
        }
        Icon::Pause => {
            p.rect_filled(Rect::from_min_max(at(r, 0.22, 0.18), at(r, 0.42, 0.82)), 1.0, col);
            p.rect_filled(Rect::from_min_max(at(r, 0.58, 0.18), at(r, 0.78, 0.82)), 1.0, col);
        }
        Icon::Play => {
            poly(p, r, &[(0.25, 0.15), (0.85, 0.5), (0.25, 0.85)], col);
        }
        Icon::Fast => {
            poly(p, r, &[(0.08, 0.2), (0.5, 0.5), (0.08, 0.8)], col);
            poly(p, r, &[(0.5, 0.2), (0.92, 0.5), (0.5, 0.8)], col);
        }
        Icon::Alert => {
            poly(p, r, &[(0.5, 0.05), (0.97, 0.92), (0.03, 0.92)], col);
            p.line_segment([at(r, 0.5, 0.35), at(r, 0.5, 0.65)], Stroke::new(w, Color32::BLACK));
            p.circle_filled(at(r, 0.5, 0.78), w * 0.6, Color32::BLACK);
        }
        Icon::Check => {
            p.line_segment([at(r, 0.15, 0.55), at(r, 0.4, 0.8)], st);
            p.line_segment([at(r, 0.4, 0.8), at(r, 0.88, 0.25)], st);
        }
    }
}

/// An icon on its own, `size` px square.
pub fn show(ui: &mut egui::Ui, icon: Icon, size: f32, col: Color32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    draw(ui.painter(), r, icon, col);
    resp
}

/// A touch-sized button: an icon and an optional short word. `on` fills it
/// with `colour`; off it is dark with a `colour` outline.
pub fn button(ui: &mut egui::Ui, icon: Icon, text: &str, on: bool, colour: Color32, enabled: bool) -> egui::Response {
    const H: f32 = 46.0;
    let fg = if on { Color32::BLACK } else { Color32::WHITE };
    let galley = (!text.is_empty()).then(|| ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(18.0), fg));
    let w = H + galley.as_ref().map_or(0.0, |g| g.size().x + 14.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, H), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let fill = if on { colour } else if resp.hovered() && enabled { Color32::from_gray(70) } else { Color32::from_gray(46) };
    let p = ui.painter();
    p.rect_filled(r, 9.0, fill);
    p.rect_stroke(r, 9.0, Stroke::new(2.0, colour));
    let icon_rect = Rect::from_min_size(r.min, vec2(H, H)).shrink(11.0);
    draw(p, icon_rect, icon, if on { Color32::BLACK } else { colour });
    if let Some(g) = galley {
        p.galley(pos2(r.left() + H - 4.0, r.center().y - g.size().y / 2.0), g, fg);
    }
    if !enabled {
        p.rect_filled(r, 9.0, Color32::from_black_alpha(120));
    }
    resp
}

/// A bar split into coloured parts, proportional to the counts.
pub fn stack_bar(ui: &mut egui::Ui, width: f32, parts: &[(usize, Color32)]) {
    let (r, _) = ui.allocate_exact_size(vec2(width, 14.0), egui::Sense::hover());
    let total: usize = parts.iter().map(|(n, _)| n).sum();
    let p = ui.painter();
    p.rect_filled(r, 4.0, Color32::from_gray(50));
    if total == 0 {
        return;
    }
    let mut x = r.left();
    for &(n, c) in parts {
        let w = r.width() * n as f32 / total as f32;
        if w > 0.0 {
            p.rect_filled(Rect::from_min_size(pos2(x, r.top()), vec2(w, r.height())), 0.0, c);
        }
        x += w;
    }
    p.rect_stroke(r, 4.0, Stroke::new(1.0, Color32::from_gray(90)));
}
