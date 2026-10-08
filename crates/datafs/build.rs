//! The browser has no filesystem: for wasm32 the game's data directory
//! (`data/`, as the game reads it) is compiled into the binary. Natively
//! nothing is embedded.

use std::{env, fs, path::Path};

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(root, &p, out);
        } else {
            out.push(p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
        }
    }
}

fn main() {
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("embedded.rs");
    let wasm = env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32");
    let repo = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..").canonicalize().unwrap();
    let mut files = vec![];
    if wasm {
        // only what the game loads: the one scenario, the behaviours, the fuels
        for sub in ["data/scenarios/rocca_ventosa", "data/behaviours"] {
            walk(&repo, &repo.join(sub), &mut files);
            println!("cargo:rerun-if-changed={}", repo.join(sub).display());
        }
        files.push("data/fuels_eu12.json".into());
        files.retain(|f| !f.ends_with("params.json"));
    }
    let body: String = files
        .iter()
        .map(|f| format!("    ({f:?}, include_bytes!({:?})),\n", repo.join(f).display().to_string()))
        .collect();
    fs::write(out, format!("pub static FILES: &[(&str, &[u8])] = &[\n{body}];\n")).unwrap();
}
