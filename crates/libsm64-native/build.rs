fn main() {
    let wasm = std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32");
    let mut build = cc::Build::new();
    if wasm {
        // wasm32-unknown-unknown has no libc: freestanding, with the small
        // headers in wasm/shim and the stubs in wasm/libc.c. Apple's clang
        // cannot target wasm; use an LLVM one (CC_wasm32_unknown_unknown wins).
        if std::env::var_os("CC_wasm32_unknown_unknown").is_none() {
            for dir in ["/opt/homebrew/opt/llvm/bin", "/usr/local/opt/llvm/bin"] {
                let clang = std::path::Path::new(dir).join("clang");
                if clang.exists() {
                    build.compiler(clang);
                    build.archiver(std::path::Path::new(dir).join("llvm-ar"));
                    break;
                }
            }
        }
        build
            .flag("-ffreestanding")
            .include("wasm/shim")
            .include("vendor/src/decomp")
            .file("wasm/libc.c");
        println!("cargo:rerun-if-changed=wasm");
    }
    build
        .include("vendor/src/decomp/include")
        .define("SM64_LIB_EXPORT", None)
        .define("GBI_FLOATS", None)
        .define("VERSION_US", None)
        .define("NO_SEGMENTED_MEMORY", None)
        .flag_if_supported("-fno-strict-aliasing")
        .warnings(false);
    let dirs = [
        "",
        "decomp",
        "decomp/engine",
        "decomp/include/PR",
        "decomp/game",
        "decomp/pc",
        "decomp/pc/audio",
        "decomp/mario",
        "decomp/tools",
        "decomp/audio",
    ];
    let mut files = Vec::new();
    for dir in dirs {
        let path = std::path::Path::new("vendor/src").join(dir);
        if !path.exists() {
            continue;
        }
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "c") {
                files.push(path);
            }
        }
    }
    if wasm {
        // ROM-extraction tooling that needs a filesystem; wasm/libc.c has the
        // one helper (`read_f32_be`) the rest uses.
        files.retain(|f| f.file_name().is_some_and(|n| n != "utils.c"));
    }
    files.sort();
    build.files(files).compile("sm64");
    println!("cargo:rerun-if-changed=vendor/src");
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
        println!("cargo:rustc-link-lib=m");
    }
}
