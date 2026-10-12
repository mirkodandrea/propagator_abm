fn main() {
    // Also allow workspace checks on wasm without compiling any native C.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    let mut build = cc::Build::new();
    build.include("vendor/src/decomp/include")
        .define("SM64_LIB_EXPORT", None)
        .define("GBI_FLOATS", None)
        .define("VERSION_US", None)
        .define("NO_SEGMENTED_MEMORY", None)
        .flag_if_supported("-fno-strict-aliasing")
        .warnings(false);
    let dirs = ["", "decomp", "decomp/engine", "decomp/include/PR", "decomp/game",
        "decomp/pc", "decomp/pc/audio", "decomp/mario", "decomp/tools", "decomp/audio"];
    let mut files = Vec::new();
    for dir in dirs {
        let path = std::path::Path::new("vendor/src").join(dir);
        if !path.exists() { continue; }
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "c") { files.push(path); }
        }
    }
    files.sort();
    build.files(files).compile("sm64");
    println!("cargo:rerun-if-changed=vendor/src");
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
        println!("cargo:rustc-link-lib=m");
    }
}
