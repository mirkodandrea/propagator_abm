//! The language of the game (user, 2026-10-21): chosen by `ROCCA_LANG` (or
//! `?lang=` on the web page), else the player's last choice, else the system's
//! or browser's language, else Italian. The title and intro screens have a
//! picker; Klingon is not in it: Ctrl+Shift+K switches it on and off.
//! The texts themselves are in `rocca::i18n` and `data/i18n/`.

use bevy_egui::egui::{self, Align2};

/// What the player chose before, if anything.
#[cfg(not(target_arch = "wasm32"))]
fn saved() -> Option<String> {
    std::fs::read_to_string(path()).ok().map(|s| s.trim().to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".rocca_ventosa").join("lingua.txt")
}

#[cfg(not(target_arch = "wasm32"))]
fn save(code: &str) {
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, code);
}

#[cfg(target_arch = "wasm32")]
fn saved() -> Option<String> {
    web_sys::window()?.local_storage().ok()??.get_item("rocca_ventosa_lingua").ok()?
}

#[cfg(target_arch = "wasm32")]
fn save(code: &str) {
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.set_item("rocca_ventosa_lingua", code);
    }
}

/// The system's language tag (`de-AT`), or the browser's.
#[cfg(not(target_arch = "wasm32"))]
fn system() -> Option<String> {
    sys_locale::get_locale()
}

#[cfg(target_arch = "wasm32")]
fn system() -> Option<String> {
    let nav = js_sys::Reflect::get(&js_sys::global(), &"navigator".into()).ok()?;
    js_sys::Reflect::get(&nav, &"language".into()).ok()?.as_string()
}

/// The page may force a language (`?lang=de`): `window.roccaLang`.
#[cfg(target_arch = "wasm32")]
fn forced() -> Option<String> {
    js_sys::Reflect::get(&js_sys::global(), &"roccaLang".into()).ok()?.as_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn forced() -> Option<String> {
    std::env::var("ROCCA_LANG").ok()
}

/// Picks the language at start. The easter egg is only taken when forced; a
/// saved Klingon is never kept (see [`toggle_klingon`]).
pub fn init(data: &std::path::Path) {
    rocca::i18n::set_root(data);
    let code = [forced(), saved(), system()]
        .into_iter()
        .flatten()
        .filter_map(|tag| rocca::i18n::parse(&tag))
        .find(|c| *c != rocca::i18n::EASTER_EGG.0 || forced().is_some());
    if let Some(code) = code {
        rocca::i18n::set(code);
    }
}

/// The player picks a language: used from now on and remembered.
pub fn choose(code: &str) {
    if rocca::i18n::set(code) == code {
        save(code);
    }
}

/// Ctrl+Shift+K: Klingon on, and off again back to the language chosen before.
pub fn toggle_klingon() {
    if rocca::i18n::code() == rocca::i18n::EASTER_EGG.0 {
        rocca::i18n::set(saved().as_deref().and_then(rocca::i18n::parse).or_else(|| system().as_deref().and_then(rocca::i18n::parse)).unwrap_or("it"));
    } else {
        rocca::i18n::set(rocca::i18n::EASTER_EGG.0);
    }
}

/// The language picker, top left of the title and intro screens.
pub fn picker(ctx: &egui::Context) {
    egui::Area::new(egui::Id::new("lingua")).anchor(Align2::LEFT_TOP, [10.0, 10.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        let now = rocca::i18n::code();
        let name = |c: &'static str| rocca::i18n::LANGS.iter().chain([&rocca::i18n::EASTER_EGG]).find(|(x, _)| *x == c).map_or(c, |(_, n)| n);
        let mut picked = None;
        egui::ComboBox::from_id_salt("lingua_scelta").selected_text(name(now)).height(420.0).show_ui(ui, |ui| {
            for (code, native) in rocca::i18n::LANGS.iter().chain((now == rocca::i18n::EASTER_EGG.0).then_some(&rocca::i18n::EASTER_EGG)) {
                if ui.selectable_label(*code == now, *native).clicked() {
                    picked = Some(*code);
                }
            }
        });
        crate::playtest::control("Lingua", &ui.response(), true);
        if let Some(code) = picked {
            choose(code);
        }
    });
}
