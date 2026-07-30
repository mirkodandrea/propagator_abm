//! Prepare the small terrain asset embedded in the web build.
//!
//! The source terrain is 5 m posting (2048² samples).  That is excellent for
//! desktop close-ups but expensive to download and draw in a browser.  The
//! simulation itself remains on its native 20 m fire grid, so selecting every
//! fourth height sample gives the web renderer a matching 20 m terrain mesh.

use std::{env, fs, path::PathBuf};

fn main() {
    let data = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../data");
    let source = data.join("spotorno_render_terrain.f32");
    println!("cargo:rerun-if-changed={}", source.display());

    let bytes = fs::read(&source).expect("read source terrain for web asset");
    let samples: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(samples.len(), 2048 * 2048, "unexpected source terrain dimensions");

    let mut reduced = Vec::with_capacity(512 * 512 * 4);
    for row in (0..2048).step_by(4) {
        for col in (0..2048).step_by(4) {
            reduced.extend_from_slice(&samples[row * 2048 + col].to_le_bytes());
        }
    }
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("web_terrain.f32"), reduced)
        .expect("write reduced web terrain");
}
