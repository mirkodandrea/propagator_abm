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

use crate::domain::Domain;
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

/// What happened to one file on the way in.
///
/// The editor lists these, which is the whole reason they exist: a scientist who
/// hand-edited a graph and got a comma wrong needs to be told *which file* and
/// *what about it*, next to the ones that loaded — not to have the whole library
/// refuse and fall back to the shipped defaults with one line on stderr.
#[derive(Debug, Clone)]
pub struct FileReport {
    pub path: PathBuf,
    /// A graph file, as opposed to a subtype file.
    pub is_graph: bool,
    /// The id it declared, when it parsed.
    pub id: Option<String>,
    /// Why it did not load, when it did not.
    pub error: Option<String>,
}

impl FileReport {
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }

    /// The file's own name, which is what a person recognises.
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

/// A directory read, file by file.
#[derive(Debug, Clone, Default)]
pub struct LoadReport {
    pub library: Library,
    /// Every `.json` seen, in the order read, whether or not it loaded.
    pub files: Vec<FileReport>,
}

impl LoadReport {
    pub fn failures(&self) -> impl Iterator<Item = &FileReport> {
        self.files.iter().filter(|f| !f.ok())
    }

    pub fn failure_count(&self) -> usize {
        self.failures().count()
    }

    pub fn ok(&self) -> bool {
        self.failure_count() == 0
    }

    /// One line naming what went wrong, for a status strip.
    pub fn summary(&self) -> String {
        let bad = self.failure_count();
        if bad == 0 {
            format!(
                "{} graphs, {} profiles",
                self.library.graphs.len(),
                self.library.subtypes.len()
            )
        } else {
            format!(
                "{} graphs, {} profiles, {bad} file{} would not load",
                self.library.graphs.len(),
                self.library.subtypes.len(),
                if bad == 1 { "" } else { "s" }
            )
        }
    }
}

impl Library {
    /// Read every graph and subtype under `root`, reporting on each file.
    ///
    /// Never fails on the *content* of a file. A missing directory means nothing
    /// has been authored yet; a malformed file is recorded in
    /// [`LoadReport::files`] with its error and skipped, so one bad file costs
    /// that file rather than the whole library. Only a directory that cannot be
    /// listed at all is an error.
    pub fn load_dir_reported(root: &Path) -> Result<LoadReport> {
        let mut report = LoadReport::default();
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
                let mut file =
                    FileReport { path: path.clone(), is_graph, id: None, error: None };
                match std::fs::read_to_string(&path) {
                    Err(e) => file.error = Some(format!("{e}")),
                    Ok(text) if is_graph => match serde_json::from_str::<BehaviorGraph>(&text) {
                        Ok(g) => {
                            file.id = Some(g.id.clone());
                            report.library.graphs.insert(g.id.clone(), g);
                        }
                        Err(e) => file.error = Some(format!("{e}")),
                    },
                    Ok(text) => match serde_json::from_str::<AgentSubtype>(&text) {
                        Ok(s) => {
                            file.id = Some(s.id.clone());
                            report.library.subtypes.insert(s.id.clone(), s);
                        }
                        Err(e) => file.error = Some(format!("{e}")),
                    },
                }
                report.files.push(file);
            }
        }
        Ok(report)
    }

    /// Read every graph and subtype under `root`, refusing a directory that
    /// contains anything malformed.
    ///
    /// The strict form, for tests and for anything that wants an all-or-nothing
    /// answer. The editor uses [`Library::load_dir_reported`].
    pub fn load_dir(root: &Path) -> Result<Library> {
        let report = Library::load_dir_reported(root)?;
        if let Some(bad) = report.failures().next() {
            anyhow::bail!(
                "{}: {}",
                bad.path.display(),
                bad.error.as_deref().unwrap_or("would not load")
            );
        }
        Ok(report.library)
    }

    /// Read one graph file from anywhere on disk.
    ///
    /// The import half of "custom behaviours can be saved to and loaded from
    /// disk": a file someone was sent, rather than one already in the library
    /// directory. The caller decides what to do about an id that is already
    /// taken — see [`Library::free_id`].
    pub fn import_graph(path: &Path) -> Result<BehaviorGraph> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("parsing graph {}", path.display()))
    }

    pub fn import_subtype(path: &Path) -> Result<AgentSubtype> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text)
            .with_context(|| format!("parsing profile {}", path.display()))
    }

    /// Read one file without being told which kind it is.
    ///
    /// A behaviour file and a profile file are both JSON objects with an `id`
    /// and a `name`, so the discriminator is `nodes`: only a graph has one. It
    /// is tried first, and the error reported is the one from whichever shape
    /// the file looked more like — reporting "missing field `nodes`" for a file
    /// that was plainly a profile with a typo would send the reader the wrong
    /// way.
    pub fn import_file(path: &Path) -> Result<Imported> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let looks_like_graph = serde_json::from_str::<serde_json::Value>(&text)
            .map(|v| v.get("nodes").is_some())
            .unwrap_or(true);
        if looks_like_graph {
            serde_json::from_str::<BehaviorGraph>(&text)
                .map(Imported::Graph)
                .with_context(|| format!("parsing behaviour {}", path.display()))
        } else {
            serde_json::from_str::<AgentSubtype>(&text)
                .map(Imported::Subtype)
                .with_context(|| format!("parsing profile {}", path.display()))
        }
    }

    /// Write one graph to an arbitrary path, for sending to someone else.
    pub fn export_graph(g: &BehaviorGraph, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(g)? + "\n")
            .with_context(|| format!("writing {}", path.display()))
    }

    pub fn export_subtype(s: &AgentSubtype, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(s)? + "\n")
            .with_context(|| format!("writing {}", path.display()))
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

    /// Check the whole runnable library before starting or replacing an incident.
    /// Missing graph references must not silently remove profiles from assignment.
    pub fn validate_runtime(&self) -> Result<()> {
        for profile in self.subtypes.values() {
            anyhow::ensure!(self.graphs.contains_key(&profile.graph),
                "Profile \"{}\" needs missing behaviour \"{}\". Load that behaviour or update the profile.",
                profile.name, profile.graph);
            anyhow::ensure!(profile.share.is_finite() && profile.share >= 0.0,
                "Profile \"{}\" needs a finite, non-negative population share.", profile.name);
        }
        for domain in Domain::ALL {
            let assigned: Vec<String> = if domain == Domain::SuppressionUnit {
                self.unit_assignment()
            } else {
                self.share_assignment(domain).into_iter().map(|(id, _)| id).collect()
            };
            anyhow::ensure!(!assigned.is_empty(),
                "{} has no active profiles. Open Profiles and enable a profile or give it a positive share.", domain.label());
            for id in assigned {
                self.compile(&id)?;
            }
        }
        Ok(())
    }

    /// Which domain a subtype runs in, from the graph it points at.
    ///
    /// A subtype does not carry its own domain: it would be a second place for
    /// the answer to live, and the two could disagree. `None` means it names a
    /// graph that is not loaded, which `compile` reports properly.
    pub fn domain_of(&self, subtype: &AgentSubtype) -> Option<Domain> {
        self.graphs.get(&subtype.graph).map(|g| g.domain)
    }

    /// Subtypes of one share-assigned domain with a non-zero share, and their
    /// shares normalised to sum to one. Empty when nothing has a share, which
    /// makes the library incomplete for a simulation run.
    pub fn share_assignment(&self, domain: Domain) -> Vec<(String, f32)> {
        let mine: Vec<&AgentSubtype> = self
            .subtypes
            .values()
            .filter(|s| self.domain_of(s) == Some(domain))
            .collect();
        let total: f32 = mine.iter().map(|s| s.share.max(0.0)).sum();
        if total <= 0.0 {
            return Vec::new();
        }
        mine.iter()
            .filter(|s| s.share > 0.0)
            .map(|s| (s.id.clone(), s.share / total))
            .collect()
    }

    /// Household profiles in play, by share.
    pub fn assignment(&self) -> Vec<(String, f32)> {
        self.share_assignment(Domain::Household)
    }

    /// Separated-person profiles in play, by share.
    ///
    /// Shares rather than an on/off list, like the households and unlike the
    /// units, because these are anonymous too: there are hundreds of them and
    /// no one of them is a named individual whose behaviour a player would ask
    /// about by name.
    pub fn person_assignment(&self) -> Vec<(String, f32)> {
        self.share_assignment(Domain::Person)
    }

    /// Whether anything at all is assigned, in any domain.
    ///
    /// What the caller checks before deciding a library is worth applying. Only
    /// looking at the households — which is what this used to do — quietly
    /// discarded a library whose only live profile was a unit policy or a
    /// person behaviour.
    pub fn has_assignment(&self) -> bool {
        !self.assignment().is_empty()
            || !self.person_assignment().is_empty()
            || !self.unit_assignment().is_empty()
    }

    /// Suppression profiles that are in play, in id order.
    ///
    /// No shares and no normalisation: a unit takes the first profile in this
    /// list that governs its kind. Empty makes the library incomplete for a
    /// simulation run.
    pub fn unit_assignment(&self) -> Vec<String> {
        self.subtypes
            .values()
            .filter(|s| self.domain_of(s) == Some(Domain::SuppressionUnit) && s.enabled)
            .map(|s| s.id.clone())
            .collect()
    }
}

/// What came out of [`Library::import_file`].
#[derive(Debug, Clone)]
pub enum Imported {
    Graph(BehaviorGraph),
    Subtype(AgentSubtype),
}

impl Imported {
    pub fn id(&self) -> &str {
        match self {
            Imported::Graph(g) => &g.id,
            Imported::Subtype(s) => &s.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Imported::Graph(g) => &g.name,
            Imported::Subtype(s) => &s.name,
        }
    }

    /// What to call it in a status line.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Imported::Graph(_) => "behaviour",
            Imported::Subtype(_) => "profile",
        }
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
