//! Prepare small assets embedded in the web build.
//!
//! The source terrain is 5 m posting (2048² samples).  That is excellent for
//! desktop close-ups but expensive to download and draw in a browser.  The
//! simulation itself remains on its native 20 m fire grid, so selecting every
//! fourth height sample gives the web renderer a matching 20 m terrain mesh.

use std::{env, fs, path::PathBuf};

fn main() {
    let data = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../data");

    // Support scenario selection via env var for web builds
    let scenario_id = env::var("SPOTORNO_WEB_SCENARIO").unwrap_or_else(|_| "spotorno".to_string());
    println!("cargo:rustc-env=SPOTORNO_SCENARIO_ID={}", scenario_id);

    let scenario_dir = data.join("scenarios").join(&scenario_id);

    // Process terrain
    let source_terrain = scenario_dir.join("render_terrain.f32");
    println!("cargo:rerun-if-changed={}", source_terrain.display());

    let bytes = fs::read(&source_terrain)
        .expect(&format!("read source terrain from scenarios/{}/render_terrain.f32", scenario_id));
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

    // Copy fuel raster
    let source_fuel = scenario_dir.join("fuel.i32");
    println!("cargo:rerun-if-changed={}", source_fuel.display());
    let fuel_bytes = fs::read(&source_fuel)
        .expect(&format!("read fuel from scenarios/{}/fuel.i32", scenario_id));
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("web_fuel.i32"), fuel_bytes)
        .expect("write web fuel");

    // Copy DEM raster
    let source_dem = scenario_dir.join("dem.f64");
    println!("cargo:rerun-if-changed={}", source_dem.display());
    let dem_bytes = fs::read(&source_dem)
        .expect(&format!("read DEM from scenarios/{}/dem.f64", scenario_id));
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("web_dem.f64"), dem_bytes)
        .expect("write web DEM");

    // Copy population
    let source_pop = scenario_dir.join("population.json");
    println!("cargo:rerun-if-changed={}", source_pop.display());
    let pop_bytes = fs::read(&source_pop)
        .expect(&format!("read population from scenarios/{}/population.json", scenario_id));
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("web_population.json"), pop_bytes)
        .expect("write web population");
}
