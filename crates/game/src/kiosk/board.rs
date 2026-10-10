//! Where the leaderboard is kept: a file on the kiosk, the browser's local
//! storage on the web. The board itself and the score are `rocca::score`;
//! this only reads and writes it. «2»: scores out of 1000 since
//! 2026-10-10; the first board's points are not comparable.

use rocca::score::Board;

#[cfg(not(target_arch = "wasm32"))]
fn path() -> std::path::PathBuf {
    // KIOSK_SCORES moves it, e.g. to a stick the operator keeps
    if let Ok(p) = std::env::var("KIOSK_SCORES") {
        return p.into();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".rocca_ventosa").join("classifica2.json")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load() -> Board {
    std::fs::read_to_string(path()).map(|s| Board::from_json(&s)).unwrap_or_default()
}

/// Save the board; a failure is reported, never fatal.
#[cfg(not(target_arch = "wasm32"))]
pub fn save(b: &Board) -> Result<(), String> {
    let p = path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("classifica: {e}"))?;
    }
    std::fs::write(&p, b.to_json()).map_err(|e| format!("classifica: {e}"))
}

#[cfg(target_arch = "wasm32")]
const KEY: &str = "rocca_ventosa_classifica2";

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn load() -> Board {
    storage().and_then(|s| s.get_item(KEY).ok().flatten()).map(|s| Board::from_json(&s)).unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
pub fn save(b: &Board) -> Result<(), String> {
    let s = storage().ok_or("classifica: il browser non salva dati")?;
    s.set_item(KEY, &b.to_json()).map_err(|_| "classifica: salvataggio non riuscito".to_string())
}
