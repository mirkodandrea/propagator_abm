//! The group chat of the operation: what `rocca::Game::chat` says, drawn as
//! small bubbles with a tiny portrait and the time. On a desktop it is a panel
//! at the top left; on a phone it is a notification at the top, for a few
//! seconds. Nothing here decides anything: it only shows what the game said.

use bevy_egui::egui::{self, Align2, Color32, RichText};
use rocca::{ChatMsg, Kind, Voice};

use super::characters::{Mood, Portraits, Who};
use super::ui::compact;
use crate::sim::Sim;

/// Lines kept in the panel.
const SHOWN: usize = 4;
/// Real seconds a notification stays up on a phone, and how many at once.
const TOAST_S: f64 = 6.0;
const TOASTS: usize = 2;
const PORTRAIT: f32 = 34.0;

/// What this view remembers between frames (in egui's memory, so a restart of
/// the game only has to notice that the chat got shorter).
#[derive(Clone, Default)]
struct Seen {
    count: usize,
    /// (index in the chat, real time it arrived)
    fresh: Vec<(usize, f64)>,
}

fn who(sim: &Sim, voice: Voice) -> (Who, String) {
    match voice {
        Voice::Team(t) => (Who::Vigili, sim.teams.get(t).map_or_else(|| "Squadra".into(), |t| t.name.clone())),
        Voice::Mayor => (Who::Sindaca, "Sindaca".into()),
        Voice::Expert => (Who::Esperto, "Esperto".into()),
    }
}

fn mood(kind: Kind) -> Mood {
    match kind {
        Kind::Withdraw | Kind::Lost | Kind::EvacStuck | Kind::HomesHit | Kind::Wind => Mood::Preoccupato,
        _ => Mood::Calmo,
    }
}

/// One line: the portrait, and a bubble with «name · time» above the text.
/// `age` is 0 for the newest line and 1 for the oldest shown: older bubbles
/// get duller, never see-through.
fn line(ui: &mut egui::Ui, sim: &Sim, faces: Option<&Portraits>, m: &ChatMsg, age: f32, width: f32) {
    let (face, name) = who(sim, m.who);
    let dim = |c: Color32, to: Color32| Color32::from_rgb(
        (c.r() as f32 + (to.r() as f32 - c.r() as f32) * age * 0.35) as u8,
        (c.g() as f32 + (to.g() as f32 - c.g() as f32) * age * 0.35) as u8,
        (c.b() as f32 + (to.b() as f32 - c.b() as f32) * age * 0.35) as u8,
    );
    let night = Color32::from_rgb(29, 36, 48);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let (r, _) = ui.allocate_exact_size(egui::vec2(PORTRAIT, PORTRAIT), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), PORTRAIT * 0.5, night);
        if let Some(t) = faces.and_then(|f| f.get(face, mood(m.kind))) {
            egui::Image::new(egui::load::SizedTexture::new(t, r.size())).corner_radius(PORTRAIT * 0.5).paint_at(ui, r.shrink(2.0));
        }
        ui.painter().circle_stroke(r.center(), PORTRAIT * 0.5 - 1.0, egui::Stroke::new(2.0, dim(face.colour(), night)));
        egui::Frame::NONE
            .fill(dim(Color32::from_rgb(247, 247, 242), night))
            .corner_radius(egui::CornerRadius { nw: 3, ne: 10, sw: 10, se: 10 })
            .inner_margin(egui::Margin::symmetric(10, 6))
            .shadow(egui::epaint::Shadow { offset: [0, 2], blur: 6, spread: 0, color: Color32::from_black_alpha(90) })
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.set_max_width(width);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(RichText::new(name).size(13.0).strong().color(Color32::from_rgb((face.colour().r() as f32 * 0.55) as u8, (face.colour().g() as f32 * 0.55) as u8, (face.colour().b() as f32 * 0.55) as u8)));
                        ui.label(RichText::new(rocca::words::clock(m.at_s)).size(13.0).color(Color32::from_gray(95)));
                    });
                    ui.add(egui::Label::new(RichText::new(&m.text).size(16.0).color(Color32::from_rgb(25, 30, 40))).wrap());
                });
            });
    });
}

/// Draw the chat. Call every frame of the game; it knows by itself whether the
/// chat is a panel or a notification.
pub(super) fn show(ctx: &egui::Context, sim: &Sim, faces: Option<&Portraits>, notice_up: bool) {
    let now = ctx.input(|i| i.time);
    let id = egui::Id::new("chat_seen");
    let mut seen: Seen = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    let len = sim.chat.len();
    if len < seen.count {
        seen = Seen::default(); // a new game
    }
    for i in seen.count..len {
        seen.fresh.push((i, now));
    }
    seen.count = len;
    seen.fresh.retain(|&(_, t)| now - t < TOAST_S);
    ctx.data_mut(|d| d.insert_temp(id, seen.clone()));

    if compact(ctx) {
        let top = if notice_up { 176.0 } else { 120.0 };
        let latest: Vec<usize> = seen.fresh.iter().rev().take(TOASTS).map(|&(i, _)| i).collect::<Vec<_>>().into_iter().rev().collect();
        if latest.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("chat_toast")).anchor(Align2::CENTER_TOP, [0.0, top]).order(egui::Order::Foreground).interactable(false).show(ctx, |ui| {
            let width = (ctx.viewport_rect().width() - 48.0 - PORTRAIT).clamp(160.0, 360.0);
            for i in latest {
                line(ui, sim, faces, &sim.chat[i], 0.0, width);
            }
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        return;
    }
    if sim.chat.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::new("chat_panel")).anchor(Align2::LEFT_TOP, [10.0, 10.0]).order(egui::Order::Middle).interactable(false).show(ctx, |ui| {
        let from = len.saturating_sub(SHOWN);
        for (n, m) in sim.chat[from..].iter().enumerate() {
            // the oldest lines get duller
            let age = 1.0 - (n + SHOWN - (len - from)) as f32 / (SHOWN - 1) as f32;
            line(ui, sim, faces, m, age.clamp(0.0, 1.0), 260.0);
        }
    });
}
