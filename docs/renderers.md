# Renderers

Use **View → Renderer → Simplified 2D** to switch to a north-up tactical map.
Choose **3D scene** in the same menu to return to the orbit view. The running
incident, selection, and camera focus/zoom are retained; switching does not
restart the simulation. A first-person camera becomes a following map camera.

Camera controls:

| Input | 2D | 3D |
|---|---|---|
| Left-drag | Pan | Orbit |
| Right/middle-drag or Shift-left-drag | Pan | Pan |
| Arrows | Pan north/south/east/west | Pan relative to camera heading |
| Shift+Arrows | Pan faster | Pan faster |
| Scroll | Zoom at pointer | Zoom toward focus |
| + / − (including numpad) | Zoom | Zoom |
| Q / E | — | Rotate |
| Page Up / Page Down | — | Tilt |
| V | Switch to 3D | Switch to 2D |
| F / Shift+F / Home | Focus selection / fire / overview | Same |

Manual panning releases follow mode. Follow keeps updating while the pointer is
over a panel. Navigation commands leave follow/first-person mode, and Escape
cancels it. Both Shift keys work. Keyboard navigation pauses while a UI widget
owns keyboard focus; Ctrl/Command/Alt chords do not trigger plain game shortcuts.
Drags must begin on the map, and active order/ignition tools retain left-click.

Operations retain A/L/D for attack/line/drop, C for air support, X for stand down,
and Shift+E for evacuation. Tab cycles forward through available units;
Shift+Tab cycles backward. See **Help → Keyboard shortcuts** for all bindings.
Inspection, ignition placement, suppression orders, focus shortcuts, and the four
fire layers use the same simulation data and map tools.

The map shows road lines, building outlines, household status colors, moving
people/vehicles, cyan refuge squares, and white-bordered suppression units.
Fire is drawn at its native cell resolution. Selection and operational overlays
are shared with the 3D view. It uses unlit mesh batches without scene geometry,
terrain relief, vegetation, smoke, or flame particles. The 3D scene remains loaded
to allow switching back, so this mode does not reduce initial asset loading.

For native launches or unattended screenshots, set `SPOTORNO_RENDERER=2d`.
The default remains 3D.
