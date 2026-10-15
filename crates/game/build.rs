//! Records the commit the game is built from (`GAME_COMMIT`), shown on the
//! title screen and sent with the feedback, so a report names its version.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    let commit = git(&["rev-parse", "--short=8", "HEAD"])
        .or_else(|| std::env::var("GITHUB_SHA").ok().map(|s| s.chars().take(8).collect()))
        .unwrap_or_else(|| "sconosciuto".into());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    println!("cargo:rustc-env=GAME_COMMIT={commit}{}", if dirty { "-modificato" } else { "" });
    if let Some(dir) = git(&["rev-parse", "--git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
        if let Some(r) = git(&["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={dir}/{r}");
        }
    }
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
}
