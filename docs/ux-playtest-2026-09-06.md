# UX playtest — 6 September 2026

Tested the current native release using the actual game UI, in Spotorno. Started from the scenario chooser; ordered general evacuation, tried an engine attack, assigned a hand crew, requested aircraft, placed a drop, searched entities, and used the incident report and help. Left the simulation paused at T+02:55:14: 478/750 households safe, 2 casualties, 67 structures lost. These are playtest outcomes, not a model-validation result. No gameplay code was changed.

## Findings, in recommended fix order

### 1. Crew selection and entity selection disagree — high

Reproduce: single-click Autobotte 1 or Squadra A in the left roster, then try the toolbar's Selection button. The roster highlights the crew and enables its orders, but Selection remains disabled when no entity has previously been inspected. Selecting the same crew through Entities enables it.

The roster updates the order-tool selection on single-click, but only updates the inspected entity on double-click (`crates/game/src/command.rs:478–483`); the camera button depends on the inspected entity (`crates/game/src/menu.rs:487`). A double-click workaround exists in the hover tooltip, but the two meanings of “selected” are not visible.

Fix: synchronize selection on single-click. Keep double-click as a camera shortcut and provide a visible Locate action.

### 2. Evacuation has no immediate acknowledgment in the main panel — high

Reproduce: click Evacuate everyone early in the run. The main panel continues to show a near-zero safe count and unchanged buttons. Opening Incident report reveals Ordered: 750 households and the preparation/movement breakdown.

The action works, but its main feedback measures completion rather than receipt. This makes normal preparation time look like an ignored click. The response panel calls the order method without presenting its result (`crates/game/src/ui.rs:891–927`).

Fix: acknowledge “Evacuation ordered for N households” immediately, and show ordered/preparing/moving/safe counts beside the action. Label the nearby action with its 2 km radius and show its area; its center is the opening ignition, which is only explained on hover.

### 3. Targeting requires trial and error; acceptance does not establish reachability — high

Reproduce: select an engine, choose Attack here, click the visible fire. My first attempt was rejected with “No road within hose reach of there.” A hand-crew attack was accepted at the fire edge, but after resuming the crew displayed “no road to there that is still open.” It eventually made progress later in the run.

A validity-colored cursor ring exists, but it does not reveal where an engine can work or a crew's route. Ground-crew placement checks suppressible fuel rather than travel feasibility (`crates/game/src/command.rs:250–277`). The engine refusal also conflates failed reach and absent suppressible fuel (`crates/game/src/command.rs:297–305`).

Fix: highlight reachable roads/hose coverage while targeting; preview travel feasibility and route. If an accepted order cannot currently be reached, show a persistent blocked status with a recovery action. Make rejection text match the failed condition.

### 4. “Attack here” means different things for different crews — medium

Reproduce: give Squadra A an attack. The roster initially says direct attack, then switches to line construction, including a line-length counter. To a new player this looks like the order changed. Source inspection confirms that a hand crew cutting line is intended behavior and is explained in the attack tooltip (`crates/game/src/command.rs:550–555`).

Fix: use unit-specific labels such as “Suppress from road” and “Build defensive line here,” with a brief preview of the resulting work. Keep requested objective and current activity distinguishable.

### 5. Completed attack/drop placement leaves the command tool armed — medium

Reproduce: place a valid attack or aircraft drop. The roster changes state, but “click to order” remains active. Subsequent map clicks continue placing orders until Escape. Line placement, by contrast, disarms after completion (`crates/game/src/command.rs:331–336`).

Fix: disarm after a successful order by default, or make repeat-placement mode explicit. Provide a short acceptance message identifying the unit and target.

### 6. Onboarding is hidden and some help describes an older interface — medium

The first scenario opens with Quick start collapsed, the report hidden, and an 8x speed ready when Play is pressed. The guide refers to “Intervention” and a left “Command” panel, while the visible panel is “Incident response,” with Response and Fire setup tabs (`crates/game/src/ui.rs:640–647`). The brief quick start says to press Play first; the longer guide puts Play after initial orders.

Fix: expand a short first-run briefing, use current panel names, and give one consistent sequence: assess the incident, issue initial orders while paused, then run. Surface the initial hazards and how progress will be evaluated.

### 7. Dense panels reduce readability and hide operational information — medium

At the tested window size, text and controls are small and subdued. Expanding Quick start pushes orders and air support toward the bottom of the response panel. Crew notes grow the roster inside a nested scroll area. Opening Entities and Incident report leaves a substantially smaller map; important crew rows can fall below the visible roster.

Fix: increase default type/contrast, keep selected-unit orders fixed and visible, and compress nonessential roster details before adding nested scrolling. Keep a compact incident summary visible without requiring the full report.

## Additional observations

- The native accessibility tree exposed only the window/title and macOS window buttons, not the game controls. All gameplay interaction required coordinates. Screen-reader and keyboard-navigation accessibility needs a dedicated pass.
- While searching for Squadra A, a drop command became armed unexpectedly. A later typing attempt did not reproduce it reliably, so this is a follow-up investigation rather than a confirmed shortcut bug.
- Aircraft request feedback was comparatively clear: the roster showed an inbound ETA, then staged status after arrival.
- This pass did not cover other scenarios, the browser build, mobile layouts, interviews, or behavior editing.

## Fix verification

Implemented the seven findings plus the native accessibility and typing follow-ups:

- Shared roster/map/entity selection, explicit Locate, and selection cleanup on scenario changes.
- Evacuation acknowledgments, ordered/preparing/moving counts, compact structure-risk counts, and a labeled 2 km action with an area preview.
- Road-route validation before placement, distinct rejection reasons, and approach/hose-coverage overlays. Hover pathfinding is cached between simulation ticks and metre-scale pointer changes. Fire can still invalidate a previously valid approach; the selected unit's current note and retasking controls stay visible.
- Unit-specific order names and a visible explanation of hand-crew line construction.
- Single-use order placement with persistent acknowledgment. The matching mouse release cannot accidentally inspect a different entity.
- An initially expanded paused-planning briefing, 1x default speed, initial ignition markers, and updated English/Italian help.
- Larger type and controls, a wider resizable response panel, no nested crew scroll area, and selected-unit orders pinned below the scrollable briefing and roster.
- Native egui widget trees and actions forwarded to macOS through Bevy's AccessKit adapter, including translation of legacy toggle-button roles.
- Keyboard ownership finalized after all panels, preventing newly focused text fields from also firing game shortcuts.

Validation: 19 game tests passed, including same-frame text focus, selection synchronization, target compatibility, successful placement/disarming, accessibility node conversion, and native action delivery. The native release build and wasm32 compile check passed. After expanding accessibility test coverage to selectable controls, both accessibility tests passed again and the release was rebuilt successfully.

Hands-on verification in the rebuilt native game confirmed the expanded briefing and ignition marker, immediate “750 newly notified households” feedback, enabled Selection after one roster click, pinned crew orders, accessible Launch/evacuation/crew controls after scenario loading, and an accepted Squadra A order with confirmation and automatic disarming. The game was left paused. This verifies the exposed native control tree and activation; it is not a comprehensive VoiceOver audit of the 3D map.
