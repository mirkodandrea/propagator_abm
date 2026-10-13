//! Where the day's board is kept (`rocca::score::Board`, 2026-10-13): a file
//! on the kiosk, the browser's local storage on the web. This only reads and
//! writes it, and tells the local date and time.

use rocca::score::Board;

#[cfg(not(target_arch = "wasm32"))]
fn path() -> std::path::PathBuf {
    // KIOSK_SCORES moves it, e.g. to a stick the operator keeps
    if let Ok(p) = std::env::var("KIOSK_SCORES") {
        return p.into();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".rocca_ventosa").join("classifica_giorno.json")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load() -> Board {
    std::fs::read_to_string(path()).map(|s| Board::from_json(&s)).unwrap_or_default()
}

/// Save the list; a failure is reported, never fatal. Never from the
/// scripted game (`KIOSK_SHOT`) or the screenshots (`KIOSK_SNAP`): they play
/// as «Giulia», and would enter the kiosk's real board.
#[cfg(not(target_arch = "wasm32"))]
pub fn save(b: &Board) -> Result<(), String> {
    if std::env::var_os("KIOSK_SHOT").is_some() || std::env::var_os("KIOSK_SNAP").is_some() {
        return Ok(());
    }
    let p = path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("classifica del giorno: {e}"))?;
    }
    std::fs::write(&p, b.to_json()).map_err(|e| format!("classifica del giorno: {e}"))
}

#[cfg(target_arch = "wasm32")]
const KEY: &str = "rocca_ventosa_classifica_giorno";

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
    let s = storage().ok_or("classifica del giorno: il browser non salva dati")?;
    s.set_item(KEY, &b.to_json()).map_err(|_| "classifica del giorno: salvataggio non riuscito".to_string())
}

/// The local date (`YYYY-MM-DD`) and the seconds since the epoch: what a
/// game is stamped with, and what «today» means for the board.
#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> (String, u64) {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    // the kiosk's own time zone, not UTC: the day ends at the local midnight
    let t = secs as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let day = if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        String::new()
    } else {
        format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday)
    };
    (day, secs)
}

#[cfg(target_arch = "wasm32")]
pub fn now() -> (String, u64) {
    let d = js_sys::Date::new_0();
    let day = format!("{:04}-{:02}-{:02}", d.get_full_year(), d.get_month() + 1, d.get_date());
    (day, (d.get_time() / 1000.0) as u64)
}
