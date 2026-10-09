//! The people who speak to the player, bottom left: the mayor, the fire
//! chief directing the units (DOS), the forecaster and a civil-protection
//! volunteer. Each says one short line about something the game already
//! produced -- a log entry, a crisis, an outcome count -- so no rule lives
//! here: this only chooses who says it, and when.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use rocca::crisis::Kind;

use crate::sim::Sim;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Who {
    Sindaca,
    Dos,
    Meteo,
    Volontaria,
}

impl Who {
    pub fn name(self) -> &'static str {
        match self {
            Who::Sindaca => "La sindaca",
            Who::Dos => "Il caposquadra dei vigili del fuoco",
            Who::Meteo => "Il previsore meteo",
            Who::Volontaria => "La volontaria di protezione civile",
        }
    }

    /// The colour of their frame, so they are told apart without reading.
    pub fn colour(self) -> egui::Color32 {
        match self {
            Who::Sindaca => egui::Color32::from_rgb(80, 190, 110),
            Who::Dos => egui::Color32::from_rgb(235, 80, 60),
            Who::Meteo => egui::Color32::from_rgb(110, 185, 255),
            Who::Volontaria => egui::Color32::from_rgb(250, 195, 70),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    Calmo,
    Preoccupato,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub who: Who,
    pub mood: Mood,
    pub text: String,
}

impl Line {
    fn new(who: Who, mood: Mood, text: impl Into<String>) -> Line {
        Line { who, mood, text: text.into() }
    }
}

/// The portraits, compiled in so the browser build has them too.
#[derive(Resource)]
pub struct Portraits(Vec<((Who, Mood), egui::TextureId)>);

impl Portraits {
    pub fn get(&self, who: Who, mood: Mood) -> Option<egui::TextureId> {
        self.0.iter().find(|(k, _)| *k == (who, mood)).map(|(_, t)| *t)
    }
}

pub fn load_portraits(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut contexts: EguiContexts) {
    macro_rules! png {
        ($f:literal) => {
            include_bytes!(concat!("../../../../assets/characters/", $f, ".png")).as_slice()
        };
    }
    let all: [((Who, Mood), &[u8]); 8] = [
        ((Who::Sindaca, Mood::Calmo), png!("sindaca_calmo")),
        ((Who::Sindaca, Mood::Preoccupato), png!("sindaca_preoccupato")),
        ((Who::Dos, Mood::Calmo), png!("dos_calmo")),
        ((Who::Dos, Mood::Preoccupato), png!("dos_preoccupato")),
        ((Who::Meteo, Mood::Calmo), png!("meteo_calmo")),
        ((Who::Meteo, Mood::Preoccupato), png!("meteo_preoccupato")),
        ((Who::Volontaria, Mood::Calmo), png!("volontaria_calmo")),
        ((Who::Volontaria, Mood::Preoccupato), png!("volontaria_preoccupato")),
    ];
    let mut out = vec![];
    for (key, bytes) in all {
        let Ok(img) = Image::from_buffer(
            bytes,
            bevy::render::texture::ImageType::Extension("png"),
            bevy::render::texture::CompressedImageFormats::NONE,
            true,
            bevy::render::texture::ImageSampler::linear(),
            bevy::render::render_asset::RenderAssetUsages::all(),
        ) else {
            continue;
        };
        out.push((key, contexts.add_image(images.add(img))));
    }
    commands.insert_resource(Portraits(out));
}

/// Real seconds a line stays up, and the least before the next replaces it.
const SHOW_S: f64 = 7.0;
const MIN_S: f64 = 2.5;
/// Lines waiting; older ones are dropped when the game runs faster than
/// anyone can read.
const QUEUE: usize = 2;

/// Who is speaking now and what is waiting.
#[derive(Resource, Default)]
pub struct Speech {
    pub now: Option<(Line, f64)>,
    queue: VecDeque<Line>,
    /// Log entries already turned into lines.
    seen: usize,
    /// Per district, families caught at home already told.
    caught: Vec<usize>,
    caught_checked: f64,
    /// Simulated time last frame: going back means a new game.
    last_s: i64,
}

impl Speech {
    pub fn clear(&mut self) {
        *self = Speech::default();
    }

    fn say(&mut self, l: Line) {
        if self.queue.len() >= QUEUE {
            self.queue.pop_front();
        }
        self.queue.push_back(l);
    }

    /// Turn what happened since the last frame into lines, and move the queue.
    pub fn update(&mut self, sim: &Sim, t: f64) {
        if sim.log.len() < self.seen || sim.time_s() < self.last_s {
            self.clear(); // a new game
        }
        self.last_s = sim.time_s();
        for e in &sim.log[self.seen..] {
            if let Some(l) = from_log(&e.text) {
                self.say(l);
            }
        }
        self.seen = sim.log.len();
        // families the fire reached at home: the outcome, about once a second
        if t - self.caught_checked > 1.0 {
            self.caught_checked = t;
            let o = sim.outcome();
            self.caught.resize(o.districts.len(), 0);
            for (d, x) in o.districts.iter().enumerate() {
                if x.caught > self.caught[d] {
                    let n = x.caught - self.caught[d];
                    let name = &sim.districts[d].name;
                    let who = if n == 1 { "una famiglia ancora in casa".to_string() } else { format!("{n} famiglie ancora in casa") };
                    self.say(Line::new(Who::Sindaca, Mood::Preoccupato, format!("A {name} il fuoco ha raggiunto {who}!")));
                    self.caught[d] = x.caught;
                }
            }
        }
        let up = self.now.as_ref().map_or(f64::INFINITY, |(_, at)| t - at);
        if up >= SHOW_S || (up >= MIN_S && !self.queue.is_empty()) {
            self.now = self.queue.pop_front().map(|l| (l, t));
        }
    }
}

/// Who says a log entry, and how. Entries the player caused by their own
/// plan (priorities, the coordinator's reasons) stay in the events list.
fn from_log(text: &str) -> Option<Line> {
    if let Some(rest) = text.strip_prefix("il vento gira: ") {
        return Some(Line::new(Who::Meteo, Mood::Preoccupato, format!("Il vento è girato: {rest}.")));
    }
    // "evacuazione: Le Ghiande (12 famiglie)": the place is enough
    // (playtest 3: the long lines were skipped).
    let place = |rest: &str| rest.split(" (").next().unwrap_or(rest).to_string();
    if let Some(rest) = text.strip_prefix("evacuazione: ") {
        return Some(Line::new(Who::Sindaca, Mood::Calmo, format!("Evacuazione di {} avviata.", place(rest))));
    }
    if let Some(rest) = text.strip_prefix("preallerta: ") {
        return Some(Line::new(Who::Sindaca, Mood::Calmo, format!("Preallerta a {}: tenersi pronti.", place(rest))));
    }
    if text.contains(" si ritira vicino a ") || text.contains("fuori servizio") {
        return Some(Line::new(Who::Dos, Mood::Preoccupato, format!("{text}.")));
    }
    None
}

/// Who brings a crisis to the player.
pub fn for_crisis(kind: Kind) -> (Who, Mood) {
    match kind {
        Kind::Scoperto { .. } => (Who::Sindaca, Mood::Preoccupato),
        Kind::Previsione { .. } | Kind::Vento { .. } => (Who::Meteo, Mood::Preoccupato),
        Kind::MezzoPerso { .. } => (Who::Dos, Mood::Preoccupato),
    }
}

/// A portrait in a coloured ring.
pub fn portrait(ui: &mut egui::Ui, portraits: Option<&Portraits>, who: Who, mood: Mood, size: f32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let p = ui.painter();
    p.circle_filled(r.center(), size * 0.5, egui::Color32::from_rgb(29, 36, 48));
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
pub fn says(ui: &mut egui::Ui, who: Who, text: &str, width: f32) {
    ui.vertical(|ui| {
        ui.set_max_width(width);
        ui.label(egui::RichText::new(who.name()).size(14.0).strong().color(egui::Color32::from_gray(90)));
        ui.add(egui::Label::new(egui::RichText::new(text).size(19.0).color(egui::Color32::from_rgb(20, 22, 26))).wrap());
    });
}
