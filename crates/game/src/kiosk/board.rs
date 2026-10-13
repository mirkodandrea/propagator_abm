//! Where the list of the last players is kept (`rocca::score::Recent`, no
//! leaderboard since 2026-10-13): a file on the kiosk, the browser's local
//! storage on the web. This only reads and writes it.

use rocca::score::Recent;

#[cfg(not(target_arch = "wasm32"))]
fn path() -> std::path::PathBuf {
    // KIOSK_SCORES moves it, e.g. to a stick the operator keeps
    if let Ok(p) = std::env::var("KIOSK_SCORES") {
        return p.into();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".rocca_ventosa").join("ultimi.json")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load() -> Recent {
    std::fs::read_to_string(path()).map(|s| Recent::from_json(&s)).unwrap_or_default()
}

/// Save the list; a failure is reported, never fatal. Never from the
/// scripted game (`KIOSK_SHOT`) or the screenshots (`KIOSK_SNAP`): they play
/// as «Giulia», and would enter the kiosk's real list.
#[cfg(not(target_arch = "wasm32"))]
pub fn save(b: &Recent) -> Result<(), String> {
    if std::env::var_os("KIOSK_SHOT").is_some() || std::env::var_os("KIOSK_SNAP").is_some() {
        return Ok(());
    }
    let p = path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("ultimi giocatori: {e}"))?;
    }
    std::fs::write(&p, b.to_json()).map_err(|e| format!("ultimi giocatori: {e}"))
}

#[cfg(target_arch = "wasm32")]
const KEY: &str = "rocca_ventosa_ultimi";

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn load() -> Recent {
    storage().and_then(|s| s.get_item(KEY).ok().flatten()).map(|s| Recent::from_json(&s)).unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
pub fn save(b: &Recent) -> Result<(), String> {
    let s = storage().ok_or("ultimi giocatori: il browser non salva dati")?;
    s.set_item(KEY, &b.to_json()).map_err(|_| "ultimi giocatori: salvataggio non riuscito".to_string())
}
