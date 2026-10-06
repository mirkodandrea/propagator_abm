//! The behaviour library the simulation runs on, read once from `data/behaviours/`.
//!
//! This is all that is left of the authoring tools: the graphs still *are* the
//! decision layer (`abm` has no second implementation), so something has to
//! load them, but nothing edits them in the game any more.

use bevy::prelude::*;
use behavior::Library;

#[derive(Resource)]
pub struct BehaviourLibrary {
    pub lib: Library,
    /// One entry per file, so a graph that failed to parse is a refused launch
    /// with a path in the message rather than a silently smaller library.
    pub load_report: Vec<behavior::FileReport>,
}

impl BehaviourLibrary {
    #[cfg(not(target_arch = "wasm32"))]
    fn load() -> Self {
        let root = std::path::PathBuf::from(std::env::var("SPOTORNO_DATA").unwrap_or_else(|_| "data".into()))
            .join(behavior::library::DEFAULT_DIR);
        match Library::load_dir_reported(&root) {
            Ok(r) => Self { lib: r.library, load_report: r.files },
            Err(e) => {
                eprintln!("behaviour library: {e:#}");
                Self { lib: Library::default(), load_report: Vec::new() }
            }
        }
    }

    /// Browser builds have no filesystem; the built-in form is generated from
    /// the same shipped graphs.
    #[cfg(target_arch = "wasm32")]
    fn load() -> Self {
        Self { lib: behavior::defaults::default_library(), load_report: Vec::new() }
    }
}

pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BehaviourLibrary::load());
    }
}
