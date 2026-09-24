//! Bounded draft history. Disk and runtime snapshots are separate from undo.
use behavior::Library;

#[derive(Clone)]
pub(super) struct Snapshot {
    pub lib: Library,
    pub graph_id: String,
    pub subtype: Option<String>,
}

pub(super) fn normalized(lib: &Library) -> Library {
    let mut lib = lib.clone();
    for graph in lib.graphs.values_mut() {
        graph.nodes.sort_by_key(|n| n.id);
        graph
            .wires
            .sort_by_key(|w| (w.from_node, w.from_port, w.to_node, w.to_port));
    }
    lib
}

#[derive(Default)]
pub(super) struct History {
    pub current: Option<Snapshot>,
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    grouping: bool,
}

impl History {
    pub fn record(&mut self, snapshot: Snapshot, grouping: bool) {
        if let Some(previous) = self.current.take() {
            if previous.lib != snapshot.lib {
                if !self.grouping {
                    self.undo.push(previous);
                    if self.undo.len() > 100 {
                        self.undo.remove(0);
                    }
                }
                self.redo.clear();
                self.grouping = grouping;
            } else if !grouping {
                self.grouping = false;
            }
        }
        self.current = Some(snapshot);
    }

    pub fn undo(&mut self) -> Option<Snapshot> {
        let previous = self.undo.pop()?;
        if let Some(current) = self.current.replace(previous.clone()) {
            self.redo.push(current);
        }
        self.grouping = false;
        Some(previous)
    }

    pub fn redo(&mut self) -> Option<Snapshot> {
        let next = self.redo.pop()?;
        if let Some(current) = self.current.replace(next.clone()) {
            self.undo.push(current);
        }
        self.grouping = false;
        Some(next)
    }
}
