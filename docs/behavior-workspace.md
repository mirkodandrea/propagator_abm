# Behavior workspace

Open **Behavior → Open behavior workspace** or press **G**. **F2** opens the
same workspace in Debug. G, Escape, or Back to map closes it without clearing
the selected agent.

- **Edit:** compact output cards with expandable input dependencies. Click a
  node for settings. Choose shared defaults or a profile to set the edit scope.
- **Debug:** the selected agent's applied graph, proposals, node values, used
  parameters and decision history. Run and Next decision control the incident.
- **Test:** evaluate a hypothetical situation using the draft.
- **Profiles:** manage assignments and parameter overrides.

Structure is generated from actual wires in both Edit and Debug. It is a
projection of a dataflow graph: shared nodes can appear under several consumers,
and nesting shows dependencies, not execution order. Disconnected components
remain visible; cycles are marked. Wiring provides connection editing and a
zoomable canvas when needed.

Debug always uses the applied graph with profile overrides materialized, even
when a draft with the same graph ID has changed. Opening Debug never loads over
the current draft. Author notes remain available in a collapsed section labeled
non-executable; they are not used as a behavior explanation.

Save/reload/rename/duplicate live under Manage. Apply and restart validates the
library and restarts the incident using it. Edits do not change the running model
until applied.

## Editing safety

Undo/Redo restores graph and profile edits, including deleted nodes and their
profile overrides. Continuous drags and text edits are grouped into one step;
history keeps up to 100 steps. Command/Ctrl-Z and Command/Ctrl-Shift-Z operate
on the workspace outside text fields, which retain their own text undo.

Selecting a profile opens its graph. All parameter editing uses the inspector's
selected scope, including constants; wiring edits affect the shared structure.
Deleting a node removes overrides attached to it before its ID can be reused.
Deleting a profile changes only the draft until Save.

The status distinguishes unsaved edits from unapplied edits. Undo never rewinds
the running incident or writes to disk: save or apply the restored draft when
ready. Apply checks active profiles across the library, not only the open graph.
