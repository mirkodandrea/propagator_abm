//! Rocca Ventosa's data, compiled into the binary so `play` runs from a
//! sandbox with no path into the repository (playtest spec §2). Written out
//! once to a cache directory the scenario loader can read.

use std::path::PathBuf;

use anyhow::{Context, Result};

const FILES: &[(&str, &[u8])] = &[
    ("fuels_eu12.json", include_bytes!("../../../data/fuels_eu12.json")),
    ("scenarios/demo_borgo/scenario.json", include_bytes!("../../../data/scenarios/demo_borgo/scenario.json")),
    ("scenarios/demo_borgo/dem.f64", include_bytes!("../../../data/scenarios/demo_borgo/dem.f64")),
    ("scenarios/demo_borgo/fuel.i32", include_bytes!("../../../data/scenarios/demo_borgo/fuel.i32")),
    ("scenarios/demo_borgo/osm.json", include_bytes!("../../../data/scenarios/demo_borgo/osm.json")),
    ("scenarios/demo_borgo/population.json", include_bytes!("../../../data/scenarios/demo_borgo/population.json")),
    ("scenarios/demo_borgo/render_terrain.f32", include_bytes!("../../../data/scenarios/demo_borgo/render_terrain.f32")),
    ("scenarios/demo_borgo/render_terrain.json", include_bytes!("../../../data/scenarios/demo_borgo/render_terrain.json")),
];

/// A content hash, so a rebuilt binary with new data never reads stale files.
fn tag() -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (name, bytes) in FILES {
        for b in name.bytes().chain(bytes.iter().copied()) {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

/// The data directory: `PLAY_DATA` if set (development), else the embedded
/// copy written under the system temp directory.
pub fn dir() -> Result<PathBuf> {
    if let Ok(d) = std::env::var("PLAY_DATA") {
        return Ok(PathBuf::from(d));
    }
    let root = std::env::temp_dir().join(format!("rocca-ventosa-{:016x}", tag()));
    let done = root.join(".completo");
    if !done.exists() {
        for (name, bytes) in FILES {
            let p = root.join(name);
            std::fs::create_dir_all(p.parent().expect("a parent")).context("cartella dei dati")?;
            std::fs::write(&p, bytes).context("dati del gioco")?;
        }
        std::fs::write(&done, b"ok").context("dati del gioco")?;
    }
    Ok(root)
}
