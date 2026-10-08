//! Reading the game's data files: from disk natively, and in the browser from
//! the copy of `data/` compiled into the binary (`build.rs`), because there is
//! no filesystem there. Paths are the same in both: `data/...`.

use std::io;
use std::path::{Path, PathBuf};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

/// `./data/x/../y` as `data/y`, with forward slashes: the embedded key.
fn key(p: &Path) -> String {
    let mut parts: Vec<String> = vec![];
    for c in p.components() {
        match c {
            std::path::Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            std::path::Component::ParentDir => {
                parts.pop();
            }
            _ => {}
        }
    }
    parts.join("/")
}

fn embedded(p: &Path) -> Option<&'static [u8]> {
    let k = key(p);
    embedded::FILES.iter().find(|(f, _)| *f == k).map(|(_, b)| *b)
}

fn not_found(p: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{} not found", p.display()))
}

/// Whether data files are compiled in (the browser build).
pub fn is_embedded() -> bool {
    !embedded::FILES.is_empty()
}

pub fn read(p: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let p = p.as_ref();
    if is_embedded() {
        return embedded(p).map(|b| b.to_vec()).ok_or_else(|| not_found(p));
    }
    std::fs::read(p)
}

pub fn read_to_string(p: impl AsRef<Path>) -> io::Result<String> {
    String::from_utf8(read(p)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn is_dir(p: impl AsRef<Path>) -> bool {
    let p = p.as_ref();
    if is_embedded() {
        let k = key(p) + "/";
        return embedded::FILES.iter().any(|(f, _)| f.starts_with(&k));
    }
    p.is_dir()
}

/// The files directly inside `dir`, sorted.
pub fn files_in(dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let mut out: Vec<PathBuf> = if is_embedded() {
        let k = key(dir) + "/";
        embedded::FILES
            .iter()
            .filter_map(|(f, _)| f.strip_prefix(&k).filter(|rest| !rest.contains('/')).map(|rest| dir.join(rest)))
            .collect()
    } else {
        std::fs::read_dir(dir)?.filter_map(|e| e.ok()).filter(|e| e.path().is_file()).map(|e| e.path()).collect()
    };
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_normalised() {
        assert_eq!(key(Path::new("./data/scenarios/../behaviours/graphs/a.json")), "data/behaviours/graphs/a.json");
    }
}
