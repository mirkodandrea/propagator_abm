//! The people who talk to the player: the volunteer who guides (opening,
//! tutorial, day 2, the end), the expert who gives the forecast, the fire
//! brigade. Faces and speech bubbles only: what they say comes from the
//! screens, which take it from the game.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Who {
    Volontaria,
    Esperto,
    Vigili,
    Sindaca,
}

impl Who {
    pub fn name(self) -> &'static str {
        match self {
            Who::Volontaria => "La volontaria di protezione civile",
            Who::Esperto => "L'esperto di previsioni",
            Who::Vigili => "Il caposquadra dei vigili del fuoco",
            Who::Sindaca => "La sindaca",
        }
    }

    /// A short label for the team row of the opening screen.
    pub fn short(self) -> &'static str {
        match self {
            Who::Volontaria => "La volontaria",
            Who::Esperto => "L'esperto",
            Who::Vigili => "I vigili del fuoco",
            Who::Sindaca => "La sindaca",
        }
    }

    /// The colour of their frame, so they are told apart without reading.
    pub fn colour(self) -> egui::Color32 {
        match self {
            Who::Volontaria => egui::Color32::from_rgb(250, 195, 70),
            Who::Esperto => egui::Color32::from_rgb(110, 185, 255),
            Who::Vigili => egui::Color32::from_rgb(235, 80, 60),
            Who::Sindaca => egui::Color32::from_rgb(120, 205, 140),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    Calmo,
    Preoccupato,
}

/// The portraits, compiled in so the browser build has them too.
#[derive(Resource)]
pub struct Portraits(Vec<((Who, Mood), egui::TextureId)>);

impl Portraits {
    pub fn get(&self, who: Who, mood: Mood) -> Option<egui::TextureId> {
        self.0.iter().find(|(k, _)| *k == (who, mood)).map(|(_, t)| *t)
    }
}

/// The CIMA logo, as an egui texture.
#[derive(Resource)]
pub struct Logo(pub egui::TextureId);

/// The QR code of the browser version, for the «Provami» banner.
#[derive(Resource)]
pub struct Qr(pub egui::TextureId);

fn texture(bytes: &[u8], images: &mut Assets<Image>, contexts: &mut EguiContexts) -> Option<egui::TextureId> {
    let img = Image::from_buffer(
        bytes,
        bevy::render::texture::ImageType::Extension("png"),
        bevy::render::texture::CompressedImageFormats::NONE,
        true,
        bevy::render::texture::ImageSampler::linear(),
        // Not RENDER_WORLD alone: Bevy then drops the image from the main world
        // after upload, and bevy_egui forgets the texture on that removal.
        bevy::render::render_asset::RenderAssetUsages::all(),
    )
    .ok()?;
    Some(contexts.add_image(images.add(img)))
}

pub fn load_portraits(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut contexts: EguiContexts) {
    macro_rules! png {
        ($f:literal) => {
            include_bytes!(concat!("../../../../assets/characters/", $f, ".png")).as_slice()
        };
    }
    let all: [((Who, Mood), &[u8]); 8] = [
        ((Who::Volontaria, Mood::Calmo), png!("volontaria_calmo")),
        ((Who::Volontaria, Mood::Preoccupato), png!("volontaria_preoccupato")),
        ((Who::Esperto, Mood::Calmo), png!("meteo_calmo")),
        ((Who::Esperto, Mood::Preoccupato), png!("meteo_preoccupato")),
        ((Who::Vigili, Mood::Calmo), png!("dos_calmo")),
        ((Who::Vigili, Mood::Preoccupato), png!("dos_preoccupato")),
        ((Who::Sindaca, Mood::Calmo), png!("sindaca_calmo")),
        ((Who::Sindaca, Mood::Preoccupato), png!("sindaca_preoccupato")),
    ];
    let out = all.into_iter().filter_map(|(key, bytes)| texture(bytes, &mut images, &mut contexts).map(|t| (key, t))).collect();
    commands.insert_resource(Portraits(out));
    let logo = include_bytes!("../../../../assets/brand/cima_logo_white.png");
    if let Some(t) = texture(logo, &mut images, &mut contexts) {
        commands.insert_resource(Logo(t));
    }
    if let Some(t) = texture(include_bytes!("../../../../assets/brand/qr_provami.png"), &mut images, &mut contexts) {
        commands.insert_resource(Qr(t));
    }
}

/// A portrait in a coloured ring.
pub fn portrait(ui: &mut egui::Ui, portraits: Option<&Portraits>, who: Who, mood: Mood, size: f32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    ui.painter().circle_filled(r.center(), size * 0.5, egui::Color32::from_rgb(29, 36, 48));
    if let Some(t) = portraits.and_then(|x| x.get(who, mood)) {
        egui::Image::new(egui::load::SizedTexture::new(t, r.size())).rounding(size * 0.5).paint_at(ui, r.shrink(3.0));
    }
    ui.painter().circle_stroke(r.center(), size * 0.5 - 1.5, egui::Stroke::new(4.0, who.colour()));
}

/// The speech bubble: a frame with a tail pointing left at the portrait.
pub fn bubble<R>(ui: &mut egui::Ui, who: Who, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let fill = egui::Color32::from_rgb(245, 245, 240);
    let inner = egui::Frame::none().fill(fill).rounding(14.0).inner_margin(egui::Margin::symmetric(16.0, 12.0)).stroke(egui::Stroke::new(3.0, who.colour())).show(ui, add);
    let r = inner.response.rect;
    let y = r.bottom() - 34.0;
    let tail = vec![egui::pos2(r.left() + 1.5, y - 10.0), egui::pos2(r.left() - 14.0, y + 6.0), egui::pos2(r.left() + 1.5, y + 10.0)];
    ui.painter().add(egui::Shape::convex_polygon(tail.clone(), fill, egui::Stroke::NONE));
    ui.painter().line_segment([tail[0], tail[1]], egui::Stroke::new(3.0, who.colour()));
    ui.painter().line_segment([tail[1], tail[2]], egui::Stroke::new(3.0, who.colour()));
    inner.inner
}

/// Text inside a bubble: dark on light, with the speaker's role on top.
/// `**x**` marks an action (bold, orange), `__x__` a place or thing (bold, blue).
pub fn says(ui: &mut egui::Ui, who: Who, text: &str, width: f32) {
    ui.vertical(|ui| {
        ui.set_max_width(width);
        ui.label(egui::RichText::new(who.name()).size(16.0).strong().color(egui::Color32::from_gray(90)));
        ui.add(egui::Label::new(marked(text, 27.0)).wrap());
    });
}

/// The text as a layout job, with the `**` and `__` marks turned into colour.
fn marked(text: &str, size: f32) -> egui::text::LayoutJob {
    use egui::{text::TextFormat, Color32, FontId};
    let base = Color32::from_rgb(20, 22, 26);
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let mut mode = 0; // 0 plain, 1 action, 2 place
    let mut rest = text;
    while !rest.is_empty() {
        let next = [("**", 1), ("__", 2)].iter().filter_map(|&(m, k)| rest.find(m).map(|i| (i, k))).min();
        let (chunk, tail, toggle) = match next {
            Some((i, k)) => (&rest[..i], &rest[i + 2..], Some(k)),
            None => (rest, "", None),
        };
        let (colour, bold) = match mode {
            1 => (Color32::from_rgb(196, 78, 0), true),
            2 => (Color32::from_rgb(20, 80, 170), true),
            _ => (base, false),
        };
        let mut f = TextFormat { font_id: FontId::proportional(if bold { size + 1.0 } else { size }), color: colour, ..Default::default() };
        if bold {
            f.underline = egui::Stroke::new(1.5, colour.gamma_multiply(0.45));
        }
        job.append(chunk, 0.0, f);
        if let Some(k) = toggle {
            mode = if mode == k { 0 } else { k };
        }
        rest = tail;
    }
    job
}

/// A line said to the player by name, if they gave one:
/// «Il fuoco è sotto controllo» → «Giulia, il fuoco è sotto controllo».
/// The first letter is lowered: pass only lines opening with a common word.
pub fn to(player: &str, text: &str) -> String {
    let mut c = text.chars();
    match c.next() {
        Some(f) if !player.is_empty() => format!("{player}, {}", f.to_lowercase().chain(c).collect::<String>()),
        _ => text.to_string(),
    }
}
