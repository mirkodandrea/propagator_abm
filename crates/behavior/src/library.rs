//! Loading and saving what the scientist authored.
//!
//! One directory, two folders of small JSON files, one file per thing:
//!
//! ```text
//! data/behaviours/
//!   graphs/four-stage-evacuation.json
//!   subtypes/committed-defender.json
//! ```
//!
//! One file per object rather than one big document, because these are meant
//! to be diffed and shared. A scientist who changes one threshold should be
//! able to send a colleague a three-line patch, and two people editing
//! different subtypes should not conflict.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::eval::{CompiledGraph, Overrides};
use crate::graph::BehaviorGraph;
use crate::subtype::AgentSubtype;
use crate::validate::Report;

/// Everything authored, in memory.
#[derive(Debug, Clone, Default)]
pub struct Library {
    pub graphs: BTreeMap<String, BehaviorGraph>,
    pub subtypes: BTreeMap<String, AgentSubtype>,
}

/// The conventional location under a scenario data directory.
pub const DEFAULT_DIR: &str = "behaviours";

impl Library {
    /// Read every graph and subtype under `root`.
    ///
    /// A missing directory is not an error — it means nothing has been
    /// authored yet, and the caller falls back to
    /// [`crate::defaults::default_library`]. A *malformed* file is an error,
    /// named: silently skipping it would lose someone's work without saying so.
    pub fn load_dir(root: &Path) -> Result<Library> {
        let mut lib = Library::default();
        for (sub, is_graph) in [("graphs", true), ("subtypes", false)] {
            let dir = root.join(sub);
            if !dir.is_dir() {
                continue;
            }
            let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
                .with_context(|| format!("reading {}", dir.display()))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
                .collect();
            entries.sort();
            for path in entries {
                let text = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                if is_graph {
                    let g: BehaviorGraph = serde_json::from_str(&text)
                        .with_context(|| format!("parsing graph {}", path.display()))?;
                    lib.graphs.insert(g.id.clone(), g);
                } else {
                    let s: AgentSubtype = serde_json::from_str(&text)
                        .with_context(|| format!("parsing subtype {}", path.display()))?;
                    lib.subtypes.insert(s.id.clone(), s);
                }
            }
        }
        Ok(lib)
    }

    /// Load from `root`, or fall back to the shipped default library when
    /// there is nothing there.
    pub fn load_or_default(root: &Path) -> Library {
        match Library::load_dir(root) {
            Ok(lib) if !lib.graphs.is_empty() => lib,
            Ok(_) => crate::defaults::default_library(),
            Err(e) => {
                // Returning the defaults rather than propagating: a broken
                // authored file should not stop the game launching, and the
                // caller logs this.
                eprintln!("behaviour library: {e:#}; using the shipped default");
                crate::defaults::default_library()
            }
        }
    }

    pub fn save_dir(&self, root: &Path) -> Result<()> {
        std::fs::create_dir_all(root.join("graphs"))?;
        std::fs::create_dir_all(root.join("subtypes"))?;
        for g in self.graphs.values() {
            self.save_graph(root, g)?;
        }
        for s in self.subtypes.values() {
            self.save_subtype(root, s)?;
        }
        Ok(())
    }

    pub fn save_graph(&self, root: &Path, g: &BehaviorGraph) -> Result<PathBuf> {
        let dir = root.join("graphs");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", slug(&g.id)));
        std::fs::write(&path, serde_json::to_string_pretty(g)? + "\n")
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(path)
    }

    pub fn save_subtype(&self, root: &Path, s: &AgentSubtype) -> Result<PathBuf> {
        let dir = root.join("subtypes");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", slug(&s.id)));
        std::fs::write(&path, serde_json::to_string_pretty(s)? + "\n")
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(path)
    }

    pub fn delete_subtype(&mut self, root: &Path, id: &str) -> Result<()> {
        self.subtypes.remove(id);
        let path = root.join("subtypes").join(format!("{}.json", slug(id)));
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// An id not already taken, derived from `base`.
    pub fn free_id(&self, base: &str, graphs: bool) -> String {
        let taken = |id: &str| {
            if graphs {
                self.graphs.contains_key(id)
            } else {
                self.subtypes.contains_key(id)
            }
        };
        let base = slug(base);
        if !taken(&base) {
            return base;
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|c| !taken(c))
            .expect("unbounded")
    }

    /// Compile one subtype: its graph, with its overrides baked in.
    pub fn compile(&self, subtype_id: &str) -> Result<CompiledGraph, CompileError> {
        let s = self
            .subtypes
            .get(subtype_id)
            .ok_or_else(|| CompileError::NoSubtype(subtype_id.to_string()))?;
        let g = self
            .graphs
            .get(&s.graph)
            .ok_or_else(|| CompileError::NoGraph(s.graph.clone(), s.id.clone()))?;
        CompiledGraph::compile(g, &s.overrides).map_err(|r| CompileError::Invalid(s.id.clone(), r))
    }

    /// Compile a graph with no overrides — what the editor's test bench runs
    /// when no subtype is selected.
    pub fn compile_graph(&self, graph_id: &str) -> Result<CompiledGraph, CompileError> {
        let g = self
            .graphs
            .get(graph_id)
            .ok_or_else(|| CompileError::NoGraph(graph_id.to_string(), "(editor)".into()))?;
        CompiledGraph::compile(g, &Overrides::new())
            .map_err(|r| CompileError::Invalid(graph_id.to_string(), r))
    }

    /// Subtypes with a non-zero share, and their shares normalised to sum to
    /// one. Empty when nothing has a share, which the caller reads as "do not
    /// use authored behaviour at all".
    pub fn assignment(&self) -> Vec<(String, f32)> {
        let total: f32 = self.subtypes.values().map(|s| s.share.max(0.0)).sum();
        if total <= 0.0 {
            return Vec::new();
        }
        self.subtypes
            .values()
            .filter(|s| s.share > 0.0)
            .map(|s| (s.id.clone(), s.share / total))
            .collect()
    }
}

#[derive(Debug)]
pub enum CompileError {
    NoSubtype(String),
    NoGraph(String, String),
    Invalid(String, Report),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::NoSubtype(id) => write!(f, "no subtype \"{id}\""),
            CompileError::NoGraph(g, s) => write!(f, "subtype \"{s}\" wants graph \"{g}\", which is not loaded"),
            CompileError::Invalid(id, r) => {
                write!(f, "\"{id}\" does not validate:")?;
                for e in r.errors() {
                    write!(f, "\n  - {}", e.message)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for CompileError {}

/// Filesystem-safe form of an id. Ids are kebab-case by convention, but the
/// editor lets a scientist type anything into the field.
pub fn slug(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .to_lowercase();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed
    }
}
