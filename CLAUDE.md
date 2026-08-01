# propagator_abm — Spotorno wildfire serious game

A 3D serious game about the interaction between people and wildfire: the
player is an **incident commander**, civilians are **individual simulated
humans**, and the fire is a real second-scale wildfire model running on real
data for a real place.

It mixes two existing projects:

- **`~/dev/fire/propagator/propagator_sim`** — CIMA's PROPAGATOR wildfire
  simulator. We use its **Rust core** (`rust/propagator-core`), which is
  std-only and dependency-free, so it drops into a Bevy workspace as a plain
  path dependency: no FFI, no Python at runtime.
- **`~/dev/experiments/igad-to-rust`** — a Bevy 0.14 flood-displacement ABM.
  We reuse its *shape* (terrain meshing, orbit camera, egui panels, asset
  baking, click-to-inspect, workspace replay) but **not** its flood agent
  model, which does not transfer to fire.

---

## Decisions already made

These were settled explicitly with Mirko. Do not relitigate them without
asking.

| Topic | Decision |
|---|---|
| Player role | Incident commander, RTS-style |
| Time model | Real-time play, plus a turn-based after-action debrief (debrief deferred) |
| Place | **Multiple scenarios supported** — Spotorno, Liguria is the default (44.2265 N, 8.4176 E, UTM 32N). Others can be added. |
| Agent types | Civilians (individual humans), ground crews on foot, engines/vehicles. **No aircraft.** |
| Population | ~500–2,000 people, each individually inspectable |
| Scenario length | 1–3 h simulated (initial attack) |
| Civilian model | Wildfire-specific, **built from scratch** — not a port of igad's flood sentiment model |
| Scoring | **None.** But surface information on everything |
| Architecture | New cargo workspace here; `propagator-core` as a path dependency |
| Deferred | Web/wasm build, record/replay debrief |
| Engine | Bevy **0.14** + `bevy_egui` **0.28**, matching igad so its UI ports directly |
| Scenarios | **Multi-scenario support** — load different scenarios for ABM testing and validation |
| Agent behaviour | Authorable in-game as a node graph (`crates/behavior`) for **three** kinds of agent — households, separated people, suppression units — **opt-in**: the hand-written layers stay the default. One graph, one kind of agent; the editor works on one at a time |
| Separated people | A person who is **not with their household** is an agent in their own right (`Domain::Person`). Everyone else evacuates as a family, and the household is the decision-making unit — see `crates/behavior/src/domain.rs` |
| Agent subtypes | Composition and flat parameter overrides. **No inheritance** — see the note in `crates/behavior/src/subtype.rs` |

---

## Architecture

```
crates/scenario/   baked assets + coordinate frames   (no Bevy, no fire core)
crates/fire/       PROPAGATOR integration, exposure, threat, interventions
crates/behavior/   authored agent behaviour: domains, node registry, graphs, subtypes
crates/abm/        civilians: perception, decision, road network, evacuation
crates/game/       Bevy app
scripts/           offline asset baking (Python) — never runs at game time
data/              the baked scenario, and the behaviour library
```

The dependency direction is strict: `scenario` and `behavior` are both leaves,
knowing nothing about Bevy, the fire core, or each other; `fire` depends on
`scenario`; `abm` depends on all three; `game` depends on everything. Keep it
that way — it is what makes the headless tests fast and the model testable
without a window. In particular `behavior` restates `Intent` and `UnitKind`
rather than importing them, so the editor and its tests run with no scenario,
no fire model and no road network loaded at all; `abm::behaviour::intent_of` and
`unit_kind_of` are the only two places the enums meet.

### Coordinate frames — read this before touching rendering

**Three resolutions coexist deliberately.** Conflating them is the single
easiest way to break this project.

| Concern | Resolution | Why |
|---|---|---|
| Fire model | **20 m**, 512×512 | Fixed by the PROPAGATOR input rasters. Not negotiable. |
| Render terrain | **5 m**, 2048×2048 (4.19 M verts) | Purely visual. Freely changeable. |
| Vectors + agents | **unquantised metres** | OSM and agent positions carry far more precision than 20 m |

The **world frame** is metric: origin at the scenario window's south-west
corner, **+x east, +y north**, metres. Everything outside the fire core lives
here. `World::cell_of()` converts to fire cells; nothing else should need to
know the raster spacing exists.

Bevy is Y-up with −Z forward, so **north becomes −Z** and elevation becomes
+Y. That sign flip lives in exactly one place: `crates/game/src/frame.rs`.
Use `to_bevy` / `to_world`, do not hand-roll it.

`Terrain::height_at()` is bilinear on the 5 m field — use it for anything
placed on the ground (agents, buildings, road ribbons, camera focus).
Sampling the 20 m fire DEM instead makes everything visibly step.

---

## Data

Real: fuel (`eu_fuel12` 12-class), DEM, OSM buildings/roads/hydrants.
Synthetic: the people, and the weather.

Window: 512×512 @ 20 m = 10.24 × 10.24 km, EPSG:32632, SW corner UTM
`(448360, 4892080)`. Covers Spotorno, Bergeggi, Noli and the ridges behind.

**7,629 buildings · 1,793 drivable roads · 1,863 tracks/paths · 102 hydrants ·
13 open water bodies · 750 households · 1,577 people.**

Full provenance table and per-file inventory: **`data/README.md`**.

### Regenerating

```bash
export AWS_PROFILE=return          # the COG bucket needs this specific profile
python scripts/fetch_osm.py                                   # cached; rm data/osm_raw.json to refetch
python scripts/build_render_terrain.py --factor 4 --smooth 1.0 # 5 m render heightfield
python scripts/generate_population.py --people 1500 --seed 42
python scripts/bake_fire_rasters.py                           # GeoTIFF -> raw arrays for Rust
python scripts/bake_fuels.py                                  # eu12 fuel table -> JSON
```

Use `propagator_sim`'s venv: `/Users/mirko/dev/fire/propagator/propagator_sim/.venv/bin/python`.

The COG URLs are **not** in the propagator_sim repo — its docs redact them.
They are recorded in `data/README.md`. Bucket is `cima-propagator-return`,
layers `eu_dem_utm_{26..39}.tif` and `eu_fuel12_utm_{26..37}.tif`, all on one
grid (origin `(0, 7960000)`, 20 m). Liguria is zone 32.

---

## Hard-won findings — do not rediscover these

Each of these cost real debugging time and is invisible from the code.

**1. `w_dir` is the bearing the wind blows FROM** (meteorological). The kernel
reads `w_proj = cos(w_dir - angle)`, which looks like the opposite convention,
and nothing in the pipeline flips it. Verified empirically on flat uniform
fuel: `w_dir=0` drives the fire south. The Rust core's own docstring agrees.

**2. Houses can never burn in the CA.** Buildings sit on non-vegetated fuel
cells, which are non-burnable by definition, so a house cell never enters the
fire mask — a 48 ha fire produced *zero* burning household cells at every
timestep. This is a silent always-negative, not an error. Structure threat is
therefore its own layer (`crates/fire/src/exposure.rs`). The same applies to
any "did the fire reach X" query: roads and people are on non-burnable cells
too.

**3. Single-cell ignitions fizzle ~20% of the time** at `realizations=1`. Over
seeds 1–20, four never established. Seed 42 was one of them, which made a
correct integration look broken. Always use `FireSim::ignite_patch`. Regression
test: `crates/fire/tests/seeds.rs`. A 20-realization Python ensemble hides this
completely.

**4. A fire travels only ~500–800 m in a two-hour window.** So the scenario
cannot start small and far away and still threaten anyone. Ignition placement
went through four wrong versions (documented in `crates/fire/src/ignition.rs`),
each producing a fire that ran fine and made a useless scenario. Sized
empirically in `crates/fire/tests/sizing.rs`: **250 m starting radius (≈21 ha)
threatens 137 households**.

**5. Anything accumulated per update call is a bug.** Damage originally
accrued per `update()`, making structure loss depend on the caller's step size
— the game steps every 2 s, batch tests every 300 s, a 150× difference for the
same fire. Integrate over simulated time. Test:
`damage_is_independent_of_step_size`.

**6. Exposure reach must scale with intensity.** A creeping grass fire and a
crowning conifer run threaten completely different radii. Driven off the
core's per-cell fireline intensity (kW/m, Byram) via `get_fireline_int()`:
flame length `L = 0.0775·I^0.46`, radiant reach = 4L (Butler & Cohen safe
separation), ember reach ∝ √I × wind.

**7. There are two threat layers, and they are not interchangeable.**
`fire::exposure` answers "is this *building* being destroyed" — slow,
integrated, at ~750 fixed points, ember reach out to 2.5 km.
`fire::threat::ThreatField` answers "is it survivable to *stand here*" —
instantaneous, sampled anywhere, ember reach capped at 400 m. Firebrands
destroy houses hours later at distances that do not threaten a pedestrian;
using one field for both makes either the evacuation absurdly panicky or the
structure loss absurdly local.

**8. Routing is a field, not a search.** Every agent wants the same thing —
the nearest reachable refuge — so one multi-source Dijkstra from all refuges
gives every node its distance to safety and its next hop. 61,431 nodes, both
travel modes, **6.4 ms** per refresh (`routing_cost`), once per simulated
minute, versus 750 per-agent searches. Rerouting around a road the fire has
cut then falls out of the field instead of being something each agent has to
discover.

**9. Refuges have to be measured too.** The same failure as ignition
placement: a plausible-looking assembly point sitting in continuous macchia is
a death trap the model will happily route everyone into. `abm::refuge` picks
road nodes with <12% burnable fuel within 300 m, which selects the waterfront,
the town core and the port — all at 0–16 m elevation, which is the check that
they are real.

**10. OSM ways run past the window edge, and that is useful.** Some road
vertices sit a few metres outside the 10.24 km frame. Rather than clamping
them, driving off the map counts as evacuated — the A10 and the Aurelia both
leave the window, and someone who takes them has left the incident.

**11. A back-facing ribbon is invisible, not inside-out.** The road network
existed, was built correctly, logged nothing wrong and drew *nothing at all*
for two commits: the quad strip alternates right/left across the centreline, so
the obvious index order (`a, a+1, a+2`) winds it clockwise seen from above, the
faces point at the ground, and `StandardMaterial`'s back-face culling discards
the lot. Wind ground-facing ribbons `a, a+2, a+1` — verified for the roads and
the ignition rings. Any new draped strip needs the same check, and the check is
"can I see it", because nothing else will tell you.

It came back on the far-terrain skirt, and copying working code is what did it:
`terrain_mesh` walks its lattice **north to south** (row 0 is the DEM's north
edge) while `far_terrain` walks south to north, so the same index order winds
the two meshes opposite ways. The whole skirt, and every distant house, was
culled — the world outside the window was sky — while the logs happily reported
314 k triangles. The only survivors were the cardboard trees, whose material
sets `cull_mode: None`. **A lattice's winding depends on which way its rows
run, not on the index expression that produced it.**

**12. Draping only samples the terrain where there is a vertex.** OSM ways carry
vertices where the road *bends*, so a straight run over a ridge can be a single
200 m segment — which drapes as a chord straight through the hill and
disappears into it. Resample to the render posting (5 m) before offsetting.
Same trap for any polyline laid on the ground.

**13. Anything drawn on the ground is under the canopy.** The vegetation is
5–15 m of actual plants, so a marker lifted the half-metre that clears
z-fighting is rendered perfectly and seen never. The ignition rings sit at
+20 m, next to the household beacons (+22 m) and refuge markers (+30 m). A
"correct" overlay that nobody can see looks exactly like a broken one.

**14. `vegetation_changes` is sparse *by NaN*, and getting that wrong is
silent.** The core writes every **non-NaN** cell of that grid into the fuel map.
The first version of `flush_interventions` built it with `Grid2::filled(rows,
cols, 0.0)` and wrote the line into it, which reclassified the entire 512×512
window as non-vegetated — so *any* intervention stopped the fire everywhere at
once. It survived review because the only test asserted the fire got smaller,
which it certainly did. Regression: `a_fireline_is_local`.

**15. `additional_moisture` is percentage points, and a litre is worth ~30 of
them.** The core takes the field in percent, accumulates it, decays it 1% per
minute, and stops spread past its 30-point moisture of extinction. The original
`1 mm ≈ 1 point` guess made a full Canadair load worth about half a percent —
aircraft that changed nothing. Fine fuel load is ~1 kg/m² and moisture is water
mass over dry fuel mass, so with ~⅓ of a drop reaching the fine fuel, 1 L/m² is
~30 points (`MOISTURE_POINTS_PER_LITRE`). Measured on the shipped scenario over
2 h: a 60 m cut line 300 m ahead of the front saves 40% of the area, a light
drop (0.56 L/m², +17 pts) 8%, a saturating one (+42 pts) 34%. Water buys time,
cut fuel holds ground — which is the operationally correct answer, and it is
what makes both unit types worth having.

**16. Suppression works on the fuel ahead of the front, never on the flames.**
The kernel never un-lights a cell — burn-out is `fire`'s own ageing layer — so
wetting the burning edge does exactly nothing. Every unit targets
`FireSim::is_suppressible` (burnable, unburnt, not already cut). This is the same
class of always-negative as houses never burning: the intuitive action reads as
fighting the fire and is measurably worthless.

**17. The nearest drivable node is usually the wrong one.** OSM tags plenty of
inland farm track as drivable, and those stubs connect to nothing, so
`net.nearest(p, true)` for a point up in the macchia routinely returns an island.
A* then explores the whole component and fails, which is how the first engine
dispatch left every engine parked at staging with no error anywhere. Road
components are now labelled once at build (`RoadNetwork::component`) and units
ask `nearest_reachable`, which is O(1) and means "drive as close as the road
network gets" — the hose then bridges the rest, or the note says it cannot.

**18. An A* per sub-step is not free, and "route is empty" is not "needs a
route".** A unit that has arrived has an empty route for the rest of the
incident. Re-planning on that condition ran a 61 k-node search per unit per
4 s sub-step and took the model from ~1 ms to minutes per test. The re-plan
trigger is the *target* moving or `REROUTE_S` elapsing.

**19. Closing the last metres by fractions never terminates.** The crew walk-in
was `while on_foot > 0.0 { f = on_foot/d; move f; on_foot -= d*f }`, which in f32
leaves a rounding residue every iteration and spins forever — a hang, in a model
whose tests otherwise finish in a second. One straight move per sub-step covers
at most 4.4 m and needs no loop at all.

**20. "Attack the head of the fire" puts the click *on* a burning cell.** Both
the crew's line alignment and the drop run are derived from the direction of the
nearest active cell, which degenerates to a zero vector in exactly the most
common case. Unguarded, every crew reported "that line is too short to cut" and
did nothing — visible only in a screenshot, because nothing errored.

**21. A restart has to clear the *latched* view state, and only that.** Almost
every view here is recomputed from `Sim` each frame and needs no help — the
`generation` bump does it, which is why that counter is monotonic across
restarts rather than reset. The exceptions are the things that deliberately
remember: `buildings::Structure::alight_at_s` (a latch, so a house keeps burning
down after the front passes), the smoke and ember particles (simulated in the
view), and the vehicle entities (indexed into an append-only `travellers` list).
Miss one and the new run opens with the old run's charred buildings, drifting
plume, or cars parked on roads that never burnt. `SimRestarted` fans out to the
three `reset` systems; `SPOTORNO_SELFTEST=1` is what checks they ran.

**22. A custom `Material` gets no fog, and the sea is too big to get away with
it.** `StandardMaterial` applies `FogSettings` at the end of its own fragment
shader; nothing applies it for you. The water shader went without, so the one
surface in the scene that ignores the atmosphere was also the widest: it held
the same saturated blue out to the edge of its mesh while the coast beside it
hazed away properly, and the straight line where it stopped read as a slab of
blue laid over the horizon. `shaders/water.wgsl::apply_scene_fog` transcribes
`bevy_pbr::pbr_functions::apply_fog` — transcribes, because importing that
module also pulls in `pbr_bindings`, which redeclares `StandardMaterial`'s
`@group(2)` on top of the material's own.

**23. Any boundary where one surface hands off to another is a straight line
on the horizon.** A straight world-space line viewed obliquely is straight on
screen from every angle, so two surfaces pretending to be one sea can never be
made to agree — the sea/`far_terrain` handoff was rebuilt twice before the
answer turned out to be one sea: `crates/game/src/sea.rs` meshes the near water
at 20 m and continues it in four coarse bands to the same 25 km the skirt
reaches, each band's spacing derived from its own span so it lands exactly on
the inner mesh's edge. The outer boundary is trimmed to a **disc**, not the box
the lattice is built on: a horizon the same distance away in every direction
cannot resolve into a ruled line.

**24. A node graph editor's ids are the editor's, not the file's.** `egui_snarl`
hands out its own `NodeId` on insert and there is no way to ask it to keep the
ones a file carried. Subtype overrides are keyed `<node id>.<param>`, so the
first version loaded a graph, got a fresh set of ids, and every override in
every profile silently stopped matching anything — the graph ran, the profiles
compiled, and all four behaved identically. The first fix remapped the overrides
at load; the general shape it taught is **an override keyed on an identity the
editor is free to reassign has to be remapped at the moment of reassignment**,
because every later opportunity looks like a valid graph.

It is now fixed rather than patched, and the reason is the second thing that
wanted the same identity. `EditorNode` carries a `behavior::NodeId` of its own
and `to_graph` writes *that*; snarl's handles never leave `composer/`. Remapping
was correct for overrides and could not extend to the live execution view, which
matches a trace's node ids against the canvas: a trace comes from the *running*
library, the canvas from the edited one, and no single remap exists between two
things that renumber independently. The lesson underneath the lesson: **an
identity the editor is free to reassign is not an identity**, and the second
thing that needs it is what makes that obvious.

**25. A keyboard shortcut collides silently, and the loser is whichever system
happens to read the key second — or neither, because both run.** Three of these
shipped at once. `b` was bound to the Entities browser *and* the behaviour
composer, and both systems fired on the same press, so opening the composer also
toggled the panel behind it. `a` and `d` armed an attack and a drop *and* panned
the camera west and east, because the camera panned on WASD — which reads as the
order having a mysterious side effect, not as two bindings. And nothing was
gated on egui's keyboard focus at all, so typing a household id into the
Entities search box armed an attack, dropped a load, ordered a general
evacuation and restarted the incident, one letter at a time. Bevy's
`ButtonInput` has no notion of a binding table or of who has focus: every system
that reads it is reading the raw keyboard, so **nothing warns you, nothing
errors, and there is no place a conflict shows up except by playing**. The
answers are all structural rather than clever: one system (`menu::menubar`)
decides input ownership for the frame, every shortcut system is scheduled after
it and returns early on `UiFocus::typing()`, and the camera moved to the arrow
keys because it was the binding with a full mouse gesture already covering it.
The menu bar then makes the whole table *visible*, which is the only thing that
stops the next collision being found the same way.

**26. A default that is also a legitimate value is an always-negative waiting
to happen.** `block.unit_resupply` ships with `refill_below = 0.0`, meaning
"break off when the tank is dry" — and the first version tested
`water_fraction < refill_below`, which at 0.0 is *never true*. The block
validated, appeared on the canvas wired to a live action, showed up in the
override list, and did nothing at all; the shipped policy quietly lost its
resupply rule while looking exactly like a working one. Inclusive comparison
(`<=`), because the shipped setting sits on the boundary. The general shape is
the same one as houses never burning and wetting the flames: **when a
parameter's default is the edge of its own range, the comparison at that edge is
the behaviour, not a corner case** — and the only thing that catches it is a
test that asserts the branch *fires*, not that the graph runs.

**27. A projection loses whatever it was not told to carry.** The composer keeps
`egui_snarl` authoritative and derives `BehaviorGraph` from it every frame
(`Composer::to_graph`), which is what stops the canvas disagreeing with the
validator. Adding a domain to the graph made that a trap: `to_graph` built its
output with `BehaviorGraph::new`, which defaults to `Household`, so the first
sync after opening a unit policy silently rewrote it into a civilian behaviour —
and a civilian behaviour containing only unit nodes fails validation with six
errors about nodes that were perfectly correct a frame earlier. The domain lives
in `Composer::graph_domain` because `self.graph` is the *output* of the
projection and cannot be its input. **Anything a projection does not explicitly
carry is reset to a default on every rebuild, and a default that validates is
worse than one that does not.**

**28. A dataflow graph has no active node, and drawing one anyway is a lie
that reads as a feature.** The obvious way to visualise a behaviour running is a
token moving through it, and there is nothing for the token to be: a
`BehaviorGraph` evaluates *every* node on *every* decision tick, in topological
order, between two instants of simulated time. Nothing is ever "reached" and
nothing is ever "skipped". The honest quantity is a **backward slice** — from
the winning proposal, and from every output sink, to the observations that
produced them (`Trace::active`) — and the difference matters because the failure
it replaces is not "no picture" but "a confident wrong picture": the same
highlight, drawn from a guess, is indistinguishable on screen from one drawn
from the trace.

Two things fell out of building it that would not have been guessed. **The
decision sink must be on the slice and never expanded**: every proposal in the
graph wires into it, so one backward step from it reaches every branch there is
and lights the whole canvas — the answer that looks most complete and says
least. And **the other sinks must be seeded**: a `Decision` is four numbers, only
one of them is the action, and leaving `out.prep_scale` out of the slice drew
`block.preparation` — the largest single lever in the evacuation model — as a
box that had never mattered. The general rule: **the slice is everything that
fed something the model reads back, and the sink everything converges on is a
junction rather than a cause.**

**29. Four states in one hue is three states.** "On the path", "checked and
declined", "was on the path recently" and "never on the path" are not degrees of
one thing — a rule that ran and said no is a completely different report from a
box nothing reached — and the first version drew them as four brightnesses of
green, which reads as one continuum with a bright end. They are four hues now
(`viewer::LiveRole`), and the legend lives next to the canvas rather than in a
doc comment, because a colour whose meaning is written down somewhere else is a
colour nobody reads. The same reason the pin *shapes* carry the port type: in
live mode the fill is repurposed for the role, and without the shapes the type
would simply have vanished.

---

## Current state

**Working:** terrain mesh (256 chunks), procedural vegetation from the fuel
raster, road ribbons with the drivable/track split, orbit camera, fire
rendered as an age-coloured mesh rebuilt only on sim generation change.

**The screen** (`crates/game/src/menu.rs`, `ui.rs`): a menu bar and three
regions, and nothing else.

- **Menu bar** — every action the game has, reachable in at most two clicks,
  each row carrying its own shortcut in grey. Scenario (including **Load
  scenario…**, a real round trip back to the selector), Simulation, Orders,
  View, Tools, Help. Its right-hand end is the status strip: the clock, the
  play button, the speed, and — in orange — what the next left-click will do
  when a map tool is armed.
- **Incident**, left — what the commander *reads*: the map legend, the fire
  numbers, the evacuation breakdown behind a progress bar, and the two
  evacuation orders. No transport, no layer buttons: those moved to the menu.
- **The dock**, right — one panel, three tabs, because its three jobs are
  mutually exclusive in practice: **Fire** (wind, moisture, ignition, seed,
  restart), **Units** (roster, orders, air support), **Entities** (search
  everything inspectable). `ui::DockTab`; `PanelState::focus_tab` is what makes
  a shortcut or a menu item bring its tab forward rather than select it behind
  a closed chevron.
- **Inspector**, bottom — only when something is selected. Ends with a
  **Behaviour** section for any agent running an authored one — household,
  person or unit, one function for all three — carrying the decision, the
  branches that produced it, and the one door from the map into the composer.

This replaced five independent docks (two left/right side panels, two bottom
panels, plus a floating dev window), which between them left the 3D view a
strip in the middle. `ui::sync_viewport` still points the camera at whatever is
left over, so the render never draws under an opaque panel.

**Roads** (`crates/game/src/roads.rs`): 1,793 drivable ways and 1,676 tracks as
casing-plus-surface ribbons draped on the 5 m field, resampled to 5 m and
mitred at the joints, in 234 chunks / 656 k triangles. Asphalt dark, tracks
pale dirt — the split is also the drive/walk distinction the ABM routes on.

**Wildfire controls** (`crates/game/src/ui.rs` `wildfire_body`,
`ignition_edit.rs`, `pick.rs`): the dock's **Fire** tab. Wind direction (with a
compass that spells out *both* the from-bearing and the direction the fire is
driven), wind speed, fuel moisture — staged and applied on release as a
boundary condition, so a shift changes what the front does next without
rewriting the scar. Click-to-place ignitions with a draped cursor ring that
turns red where there is no burnable fuel, radius 60–600 m. Restart, which
rebuilds `FireSim` and `Abm` and replays the ignition list. `Sim::ignitions`
carries an `at_s` per patch, so a fire lit mid-run comes back at its own time
rather than becoming part of the opening fire — a restart is a genuinely clean
comparison, not a new roll of the dice.

**Buildings** (`crates/game/src/buildings.rs`): all 7,611 drawable OSM
footprints extruded — walls on the traced outline, plinth course, overhanging
eave, hipped or flat roof by building kind, Ligurian palette hashed per
building. Merged into 195 chunks, 0.49 M triangles. Storey counts take the
population bake as a *floor* only: it sits at 2 for 98% of dwellings, which
draws the old town as bungalows. Structures recolour through
`fire::StructureExposure` — threatened, alight, charred — never through the
fire mask.

**Agents** (`crates/abm`): the four-stage evacuation model — perception,
decision, preparation, movement. Households perceive through the threat field,
structure exposure and a coarse 200 m distance-to-fire field (what they can
*see*); decide on `intent`, `risk_perception` and `trust_authority`; mill for
`prep_time_min`; then move on the real road graph by car or on foot, with
congestion, slope, rerouting and abandonment of vehicles on a cut road. The
commander's order is a lever, not a teleport: it still arrives over each
household's own channel (90 s mobile alert → 20 min for no channel at all).
Rendered as one capsule per person plus one vehicle per driving household, drawn
at 3× life size (`people::FIGURE_SCALE`) because at command altitude a person is
sub-pixel.

**People away from home are their own agents.** ~400 of 1,577 start out and
`PersonAgent::away` marks them; they launch their own walk out at T+0, from the
constructor, which is the shipped behaviour and stays there rather than falling
out of the decision layer's first tick — moving it would shift every one of
those departures by an interval. `Abm::decide_people` runs only when a person
behaviour is loaded, and every branch of it changes a *destination* rather than
a pace, which is what keeps an authored person behaviour step-size invariant.
`Traveller::goal` is `Refuge` for everyone except someone walking home, who
carries a planned path instead — the one place the civilian model runs a
per-agent A* rather than reading the route field, because it is the one case the
field cannot answer.

A representative run — general order at T+5 min, 2 h incident:

```
        aware  prep  moving  safe  defend  cutoff  dead
 30 min     3    66      39   166      51       0     0
 60 min     3     2       5   264      51       0     0
120 min     3     0       1   272      49       0     0
```

**Suppression** (`crates/abm/src/suppression.rs`, `crates/game/src/command.rs`,
`units.rs`): crews, engines and aircraft as agents, and the commander's second
lever. Three kinds, deliberately not interchangeable — and the three constraints
below are the whole game:

| | Moves on | Acts by | Limited by |
|---|---|---|---|
| **Hand crew** ×3 (`Squadra A–C`) | roads, tracks, then on foot | cutting line — permanent | 120 m/h in macchia: slower than the fire |
| **Engine** ×3 (`Autobotte 1–3`) | drivable roads only | water | 2,500 L (6 min of pumping), 60 m of hose |
| **Air tanker** ×2 (`Canadair 1–2`) | straight lines | 6,137 L a load | arriving at all: 25 min after you ask |

Air support has to be **requested** and can be **briefed while inbound**, so it
goes to work the moment it is on station. Every refusal is a sentence in the
panel, not a silent no-op: "no road within hose reach of there", "an engine
cannot cut line — send a hand crew", "pulled back: not survivable here". An
engine sent past the end of the tarmac works the roadside where it stopped and
says so, rather than refusing. Safety overrides orders: a unit ordered into
lethal threat withdraws (`WORK_LIMIT` = 0.35, below the civilians' 0.55 — they
disengage while they still can), though it can still be burnt over if the fire
comes to it.

Interventions go through the core's own boundary conditions rather than being
bolted on: `Fireline` → `vegetation_changes`, `Water` → `additional_moisture`.
Both are calibrated and measured — see findings 14–16 — and the model rewards
flying the aircraft properly rather than parking them:

```
2 h, seed 42, tramontana 35 km/h, 6% moisture
  no suppression               49.0 ha
  everything at one point      45.4 ha   (487 m line, 648 kL)
  aircraft re-tasked every 5 min  38.7 ha   (491 m line, 648 kL)
```

**The shipped scenario** (seed 42, tramontana 35 km/h from N, 6% moisture,
ignition at cell (153, 246), r=250 m):

```
  15 min   23.4 ha   front 586   FLI 80,908 kW/m   threatened  27
  90 min   38.7 ha   front 103   FLI 17,047        threatened  18
 105 min   42.8 ha   front 128   FLI 66,596        threatened 107
 120 min   49.0 ha   front 200   FLI 66,596        threatened 137
```

**Agent Behaviour Composer** (`crates/behavior`, `crates/game/src/composer/`):
a node editor for the agent decision layers, so the behavioural assumptions can
be changed without writing Rust. `g` opens it.

It covers **three kinds of agent**, and works on one at a time. The domain
selector at the top of the window scopes everything below it — the graph list,
the palette, the profiles, the test bench, the live view — because the three
share nothing but the arithmetic:

| | Replaces | Action set |
|---|---|---|
| **Households** (a family) | the departure decision in `Abm::decide` | prepare · evacuate now · defend · shelter |
| **Separated people** (one person away from home) | the T+0 bootstrap in `Abm::with_behaviours` | walk out · shelter where they are · head home |
| **Suppression units** (a crew, engine or aircraft) | the safety and resupply policy in `abm::suppression` | withdraw · break off for water · hold · return to staging |

A unit policy decides **when a unit stops**, not where it goes: pull back
because the ground is not survivable, break off because the tank is low, hold or
go home because the order was a bad one. Where a unit is sent stays the
commander's job — a graph cannot produce a map position, exactly as a household
graph cannot choose a destination.

A **person** is the one domain that is about a destination, and it is why it is
narrow: a household decides *when* to go and a separated person decides *where
to*. Only people who are not with their household run it — everyone else leaves
with the family, at the pace of the slowest member, and giving each of them a
graph would be a per-person assumption written as a per-family one.
`PersonAgent::away` is the flag that decides which they are.

**Reunification is now expressible, and ships off.** `block.person_reunite` is
on the shipped person graph with `enabled = false`, and the `family-first`
profile at share 0 turns it on. Going home is the behaviour every post-fire
study finds and no evacuation plan assumes; turning it on changes the casualty
figures, so every measurement in `crates/fire/tests` was taken with it off and
saying so is the whole reason it ships that way. A person who makes it home
rejoins the household and stops being an agent (`Abm::arrive_home`), which is
what makes it reunification rather than a detour — and if the family has already
left, they are on their own again, which is the outcome the profile exists to
make visible.

The developer-facing surface is one macro. A `behavior_node!` invocation
anywhere in any linked crate is collected by `inventory` at link time and the
node is in the palette, the validator, the inspector and the test bench with no
further edit — there is no central list, which means there is no way to add a
node the editor does not know about. A node declares `domain:` or is
domain-free; `anything_that_reads_the_world_declares_a_domain` is what stops a
node reading a default observation in the wrong graph and looking like a branch
that never fires. 117 ship — 60 offered to a household graph, 50 to a person
one, 49 to a unit one, and the 21 in all three are the arithmetic. The counts
come from `palette_report` (`cargo test -p behavior --release -- --ignored
palette_report --nocapture`), which is a report rather than an assertion: a test
that pinned them would fail every time somebody added a node.

**Blocks, not arithmetic.** The node set is two tiers, and the top one is where
authoring is meant to happen. A `Category::Block` is one whole behavioural
assumption — "how much alarm before they act", "when does a unit pull back" —
reading what it needs off the observation itself and exposing the numbers that
assumption turns on as parameters. That is also exactly what a subtype
overrides, so a profile stops being a list of node ids and becomes a list of
named quantities. The shipped evacuation behaviour was **31 nodes** written in
observations and arithmetic; on blocks it is **13**, and the unit policy is 6.
The primitives are all still in the palette — a block's *structure* is fixed,
and rebuilding one out of `Logic` nodes is the supported way to change it.

The scientist-facing surface is a graph. Four port types (`number`, `bool`,
`intent`, `action`), and a wire is **refused while being dragged** rather than
reported afterwards — `viewer::connect` asks `behavior` the same question the
validator does, so the canvas cannot disagree with the report under it.
Validation is live and every issue names its node; clicking one selects it.

An **agent subtype** is a graph plus a flat map of parameter overrides plus some
starting traits — no inheritance, deliberately, so "why did this agent do that"
is answered by reading one file. How a profile is *assigned* is the one place
the domains differ, and it follows from what is being assigned to:

- **Households** and **separated people** carry a relative **share**, hashed
  onto 750 anonymous families and ~400 anonymous individuals. Different hash
  salts, so a person and their household do not correlate — sharing one would
  put every member of a low-hash family in the same person profile, which looks
  like a finding and is an artefact.
- **Units** carry a list of **kinds** and an on/off switch. There are eight of
  them and they are named individuals; a hash would make "why did Autobotte 2
  do that" a question about arithmetic rather than about a file.

Eight profiles ship: four household, on the one graph, differing only in
numbers — which is the pattern the whole thing is for — plus `walk-out` and
`family-first` (the latter at share 0), plus `standing-orders` (the shipped unit
policy, written down) and `cautious-engines` (off by default). The inspector
edits the *override* when a profile is selected and says so, because a scientist
who moves a threshold and finds they moved it for everyone has been badly
served.

**A node's identity is the file's, and the editor preserves it.** `egui_snarl`
hands out its own `NodeId` on insert; `EditorNode` carries a `behavior::NodeId`
of its own and `to_graph` writes *that*, so a load, a save and a rebuild all
name the same boxes. Override keys therefore need no remapping, and the live
execution view can match a trace's node ids against the canvas — which it could
not if the two renumbered independently. See finding 24, which this replaces
with a fix.

The **test bench** puts a made-up agent in a situation and reads back the answer
node by node, plus a sweep that varies one field across its range and reports
where the decision actually changes — a threshold here is never a single number,
so the alternative is guessing. Situations, editable fields and sweep fields all
follow the open graph's domain; eight household situations ship, eight person
ones and nine unit ones, each chosen because it is a moment the hand-written
layer either handled or visibly did not.

The **Live tab** is the other half of debugging, and it is the one that works on
the real incident rather than a made-up one. Click an agent on the map — a
household, a person, a unit — and the canvas starts showing what their behaviour
is doing: every node's outputs on the box, every input value on its pin, and the
nodes coloured by how they relate to the decision that was taken.

**The graph is dataflow, not a flowchart, and the view says so.** Every node runs
on every decision tick, so "the currently executing node" has no referent. What
does is `Trace::active` — the backward slice from the winning proposal, and from
every output sink, to the observations that produced them. Four states, and they
are not shades of one thing:

| | |
|---|---|
| **on the path taken** | fed a value into the decision that won |
| **checked, did not apply** | an action node that ran and withheld its proposal |
| **was on the path recently** | in the slice on one of the last 60 ticks |
| **not on the path** | ran, as everything does, and has never mattered |

Two details in the slice are load-bearing. The **decision sink is on it but
never expanded** — every proposal in the graph wires into it, so walking back
through its inputs would light the whole canvas, which is the useless answer. And
the **other sinks are seeded**: a `Decision` is four numbers and only one is the
action, so `block.preparation` — the largest single lever in the evacuation
model — is on the path even when the branch it feeds is not.

It does **not** guess which arm of an `Or` mattered. That needs per-node
semantics the registry does not carry, and a highlight that guesses is worse
than one that includes an extra box. It also says out loud when the canvas has
edits the incident has not been given, because a bright node the model has never
seen is otherwise indistinguishable from one it ignored.

`.` steps one decision interval (`sim::STEP_S` = 6 s: `DECISION_S` rounded up to
the fire's own quantum), running or paused, so a decision can be walked forward
one tick at a time. `Sim::advance` is the shared body, extracted so a step and a
play frame take the same path through the scheduled ignitions and the auto-order
— a single-step facility that took a different one would step something other
than what plays. History is per subject and dropped the moment the selection
changes, so one agent's traversed path is never shown over another's graph.

The **Help tab** is the editor's own documentation, next to the thing it is
about: what a graph *is* (the dataflow point, first and open by default), adding
and wiring nodes, conditions and priorities, profiles, running and debugging,
and the file list — which names every `.json` in `data/behaviours/` **and the
ones that would not load, with their parse errors**. `Library::load_dir_reported`
is lenient per file, so one graph with a stray comma costs that graph and not
the other nine; the report is what turns a skipped file from a silent loss into
a line someone can act on. Import and export of a single behaviour or profile
live there too.

`Apply and restart` rebuilds both agent models and replays the ignition list, so
"same fire, different behaviour" is a controlled comparison. Measured through
the self-test, 15 minutes after a general order on an identical fire:

```
  shipped hand-written model   576 households departed
  shipped behaviour library    473          (longer baked prep times per profile)
```

And the unit policy's first interesting knob, measured on an engine attacking
300 m downwind for 25 minutes (`refill_threshold_report`):

```
  refill_below 0.00   pump dry then go       3,140 L delivered
  refill_below 0.33   go with a third left   3,387 L
  refill_below 0.60   go with most of it     2,200 L
```

Breaking off early wins a little and then loses badly: the hydrant round trip is
longer than six minutes of pumping, so past about a third the engine spends the
incident shuttling. That number has not been swept properly, and it is the
question `block.unit_resupply` exists to let someone answer.

All three are **off by default**: `Sim::behaviour` is `None` until the composer
applies something, and a library with no profile in play leaves each model on
its own hand-written layer — so every measurement in `crates/fire/tests` still
describes the model it was taken on. The shipped graphs are transcriptions of
those layers and are pinned to reproduce them exactly
(`the_shipped_policy_reproduces_the_hand_written_one`,
`the_shipped_person_behaviour_reproduces_the_hand_written_one`). Whether a
library is worth applying is `Library::has_assignment`, which asks all three:
checking only the households — which is what it used to do — silently discarded
a library whose one live profile was a unit policy, and the symptom was an Apply
that reported success and changed nothing.

**Not built yet:** no debrief. No wasm. No dozers (the only line-cutting
resource is a hand crew, which is why line production is the binding
constraint). Units are
selected from the Resources panel, not by clicking them on the map — the
screen-space picker in `inspect` would do it, but three tools already contend for
left-click and a fourth needs a rule, not a patch.

### Commands

```bash
cargo run --release -p game              # play with scenario selector UI
SPOTORNO_SCENARIO=spotorno cargo run --release -p game    # skip UI, load scenario directly
SPOTORNO_SCENARIO=test_small cargo run --release -p game  # test with small synthetic scenario
SPOTORNO_AUTOPLAY=1 cargo run --release -p game   # start running immediately (with selector)
SPOTORNO_ORDER_AT=600 cargo run --release -p game  # auto-order evacuation at T+10 min
SPOTORNO_ATTACK_AT=300 cargo run --release -p game # commit every unit to the head at T+5 min
cargo test --release                     # everything, ~4 s
cargo test -p abm --release -- --ignored --nocapture     # evacuation timeline,
       # routing cost, and the engine refill-threshold sweep
cargo test -p fire --release -- --ignored --nocapture    # slow calibration sweeps

# the wildfire controls, driven without a keyboard: place an ignition mid-run,
# shift the wind, restart, and check each one actually did what it claims.
# Exits non-zero on failure. The controls are Bevy behaviour -- resources,
# events, the reset systems -- so this is the only place they can be tested.
SPOTORNO_SELFTEST=1 cargo run --release -p game

# screenshots without a human at the keyboard
SPOTORNO_SHOT=/tmp/shots SPOTORNO_SHOT_AT=1800 SPOTORNO_SHOT_DIST=400 \
  SPOTORNO_SHOT_FOCUS=4875,2875 SPOTORNO_SHOT_LAYER=Flames \
  SPOTORNO_AUTOPLAY=1 cargo run --release -p game
SPOTORNO_SHOT_YAW=270 SPOTORNO_SHOT_PITCH=-14 ...  # orbit angle, degrees --
       # the only way to review something that is wrong from one direction
       # (a seam on the horizon), which the default three-quarter view misses
SPOTORNO_PLACE=1 ...   # open with the ignition tool armed, so the rings show
SPOTORNO_TAB=fire ...  # open on a chosen dock tab: fire | units | entities --
       # the only way to screenshot one of them unattended

# the behaviour composer, opened with the scenario -- the value picks the
# right-hand tab, which is the only way to screenshot a tab unattended.
SPOTORNO_COMPOSER=1 cargo run --release -p game          # node inspector
SPOTORNO_COMPOSER=subtypes ...                           # profiles and compare
SPOTORNO_COMPOSER=test ...                               # the test bench
SPOTORNO_COMPOSER=live ...                               # the live execution view
SPOTORNO_COMPOSER=help ...                               # the help tab
SPOTORNO_COMPOSER_DOMAIN=units ...   # open on another kind of agent:
       # households | people | units -- the editor shows one at a time, so this
       # is the only way to screenshot the others unattended

# the three inputs the live execution view needs and an unattended run cannot
# produce: an applied library, a selected agent, and a running clock. Without
# the first two the Live tab can only ever be screenshotted empty.
SPOTORNO_BEHAVIOUR=1 ...             # apply the library on the first frame
SPOTORNO_WATCH=household:0 ...       # select an agent: household | person | unit
       # survives the restart an Apply causes, unlike a click

# regenerate data/behaviours/ after editing crates/behavior/src/defaults.rs
cargo test -p behavior --release -- --ignored write_shipped_library

# what the palette offers per domain, for the counts quoted above
cargo test -p behavior --release -- --ignored palette_report --nocapture
```

Controls: `space` play/pause · `.` step one decision (paused or not) · `[` `]`
speed · `1`–`4` fire layer · `e` general evacuation order · `i` arm the ignition
tool (then left-click the map) · `esc` disarm · `r` restart · `b` the Entities
tab · `g` the Agent Behaviour Composer · `f1` help · `f12` screenshot · drag
orbit · right-drag pan · scroll zoom · **arrow keys** pan.

Suppression: `tab` next unit · `a` attack here · `l` cut line (two clicks) ·
`d` drop here · `x` stand down · `c` request air support. Units are selected in
the dock's **Units** tab or with `tab`; the order is then placed by clicking the
ground.

Everything above is also a row in the menu bar, with the key printed beside it.
That is the only reason a player ever learns any of it.

**Three tools contend for left-click** — ignition placement, agent inspection,
and suppression orders — and the invariant is that at most one is armed. Arming
either tool disarms the other, `inspect::pick_click` stands down while either is
armed, and `esc` returns to plain inspect-and-orbit. While a tool is armed
left-drag no longer orbits; right-drag pan, scroll and the arrow keys always
work, so the view is never stuck.

**One system owns input arbitration.** `menu::menubar` runs first each frame
and writes both halves of `ui::UiFocus`: `pointer` (assigned there, OR-ed into
by every panel after it) and `keyboard` (egui's own `wants_keyboard_input`).
Every shortcut system reads `UiFocus::typing()` as its first act and every one
of them is scheduled `.after(menu::menubar)`. `esc` is the single exception,
because it is what gets you *out* of a state.

---

## Working agreements

- **Two agents are active.** One (this line of work) owns the model:
  `crates/scenario`, `crates/fire`, `crates/abm`, `scripts/`, `data/`. Another owns
  rendering. The contract between them is `crates/scenario` — the world frame,
  `Terrain::height_at()`, and the asset formats. Changing those needs
  coordination; changing materials, meshes, shaders or camera does not.
- The 20 m grid binds the **fire model only**. Rendering should use the 5 m
  field and is free to go finer — swapping in a real 5–10 m DTM (Tinitaly,
  Regione Liguria geoportale) means changing only `load_dem` in
  `scripts/build_render_terrain.py`.
- Prefer measuring to reasoning about the fire. Every scenario-tuning question
  so far has been settled by running candidates headlessly, and the heuristics
  were wrong every time — notably, the best ignitions are *inland* in
  continuous fuel, not at the WUI edge.
- Keep the model testable without a window. The headless tests in
  `crates/fire/tests/` run a full 2 h simulation in well under a second.
- Exposing a new thing an agent can know is a field on `behavior::HouseholdObs`,
  `PersonObs` or `UnitObs` plus one `obs_number!` / `person_bool!` /
  `unit_bool!` line. Exposing a new thing it can *do* is harder on purpose: each
  domain's action set is closed, and adding to it means teaching
  `abm::behaviour::outcome_of`, `person_outcome_of` or `unit_outcome_of` what
  the movement or suppression layer should do about it.
- Adding a **domain** touches more than it looks: the enum, an observation, a
  decision sink, an action set, a runtime and a call site in `abm`, plus every
  `match` on `Domain` in `crates/game/src/composer/`. The compiler finds all of
  them except the last two, which are behavioural. `Library::has_assignment` and
  `Sim::restart` are the ones that fail *silently* when a domain is forgotten —
  the model runs, reports success, and ignores the new agents entirely.
- **Prefer a block to a cluster of primitives.** A new behavioural assumption
  should arrive as one `Category::Block` node with its numbers as parameters,
  not as six `Logic` nodes a scientist has to wire in the right order. If it
  cannot be expressed as one box, that is usually a sign the assumption has not
  been decided yet.
- A node that reads the observation **must** declare its `domain:`. A
  domain-free one reads a default observation in whichever graph it lands in,
  which behaves exactly like a branch that never fires. Pinned by
  `anything_that_reads_the_world_declares_a_domain`.
- An authored graph must not be able to break determinism or step-size
  invariance. There is no random node and no state between calls; per-agent
  variation is `jitter`, hashed from the household or unit id. Pinned by
  `authored_behaviour_is_step_size_invariant` and
  `an_authored_policy_is_step_size_invariant`.

## Open questions

- Structure loss is rare at the current tuning (1 building at 250 m start, 31
  at 500 m) — the front passes too quickly to accumulate ignition. Needs
  tuning if structure loss should be live pressure rather than a rare event.
- Ember reach saturates at its 2,500 m cap much of the time; that cap is doing
  more work than it should.
- At `max` speed the per-frame step cap (30 simulated seconds) will bind on a
  large fire, so achieved speed will fall below requested. Honest fix is
  displaying both, not raising the cap.
- Structure loss being rare means the building damage states (alight, charred)
  are almost never seen in play. Worth checking they look right before tuning
  the fire to produce more of them.
- 135 households intend to stay and defend and, at the current tuning, mostly
  never leave — the fire does not reach them. That is defensible, but it means
  the most interesting civilian behaviour in the model is currently inert.
- Congestion is a per-link occupancy count with a linear slowdown. It gives the
  right *shape* (a late mass departure is slower) but is not a traffic model;
  gridlock on the Aurelia does not emerge from it.
- The 6.4 ms routing refresh lands in a single frame, so at 512x there is a
  visible hitch once a simulated minute. Spreading it over frames or solving on
  a task pool is the fix if it starts to matter.
- Suppression units re-plan with an A* each, throttled to once a simulated
  minute. Eight units is nothing, but the throttle is the only thing keeping it
  nothing, and a larger roster would want the same multi-source treatment the
  civilians get.
- Hand crews are almost decorative at 120 m/h: 240 m of line in a two-hour
  incident, against a fire whose flanks spread 500 m each way. That is the real
  published rate and the honest answer, but it means the crews' role is holding a
  short piece of *existing* break rather than cutting new line. Worth measuring
  whether tasking them onto road-adjacent alignments (widening what is already
  there) makes them matter.
- Nothing models crew fatigue, shift length, or the water actually running out
  at the hydrant. Engines can shuttle indefinitely.
- The engine's four-cell work footprint is what makes its tank matter (see
  `reachable_targets`), but it is a tuning constant chosen to put one tank just
  past moisture of extinction. It has not been swept.
- The household action set is still the four the movement layer understood. A
  household graph cannot express "drive to a relative's house" or "shelter at
  the beach rather than in the house" — both real behaviours, both needing
  movement-layer work before a node for them would mean anything. "Go back for
  someone" is now expressible, but only for a person who is *already* away:
  a household cannot send one member back.
- **Reunification is unmeasured beyond "it changes the outcome".** The
  `family-first` profile ships at share 0 and the tests pin that it fires, that
  it respects its own limits, and that someone who gets home rejoins the
  household. What share of a real population does this, and what it costs in
  casualties at that share, is exactly the question the profile exists to let
  someone answer and nobody has.
- A person heading home plans one A* and never re-plans. If the fire cuts that
  path behind them they walk into it — which is arguably realistic and is
  certainly not modelled on purpose. The re-plan trigger the suppression layer
  learned to use (finding 18) is the shape of the fix.
- Separated people are ~400 of 1,577 and their behaviour runs per person per
  decision tick. That is half again the household load and has not been profiled
  at the top of the population range.
- Applying a behaviour restarts the incident. That is the right default and it
  is what makes the comparison controlled, but it also means a scientist cannot
  watch a threshold change take effect on the run in front of them. A "what
  would this household do under the other profile" readout in the inspector
  would give most of the value without the hot-swap.
- Subtype shares assign households by a hash of the id, so a profile is spread
  uniformly across the town. Real behavioural profiles correlate with
  neighbourhood, tenure and age; nothing in the composer can express that yet.
- The behaviour graph is evaluated per household per decision tick. At 750
  households and ~13 nodes it does not show up next to the routing refresh, but
  it has not been profiled at the top of the population range (2,000), and the
  evaluator allocates a `Vec` per input slot per node. The unit policy runs per
  unit per 4 s sub-step, which at eight units is nothing.
- **Composition is still one level deep.** A block is a shipped arrangement of
  primitives, so only a developer can decide what a block *is*. A scientist who
  wants "my own alarm rule, reused across four graphs" has to rebuild it out of
  primitives each time. Nesting — one graph usable as a node inside another —
  would fix that, and the cost is real: recursive validation, drill-in
  navigation, and override keys becoming paths, which is finding 24 one level
  deeper. Worth doing once the block vocabulary has settled from actual use.
- The two block sets were written from the hand-written models, which means they
  encode the assumptions that already existed. Whether they are the assumptions
  a scientist *wants* to vary is unmeasured, and the honest test is somebody
  other than us opening the editor.
- A unit policy cannot pick targets. "Attack the nearest suppressible fuel",
  "reposition to the flank", "go where the other engine is not" are all real
  behaviours and all need a `Target` port type plus a resolution layer in `abm`
  before a node for them would mean anything — and they would change what the
  commander's job is, which is a game-design decision rather than a modelling
  one.
- Nothing an authored policy does is visible on the map beyond the unit's note.
  A crew that pulled itself back because a graph said so looks identical to one
  that was ordered back, which is the wrong way round for a tool whose point is
  making assumptions legible. The Live tab answers it for the *selected* agent
  and only there; the map itself still says nothing.
- The live view explains one agent at a time. "Why did these forty households
  all leave at once" is the question a scientist actually has, and it needs an
  aggregate — which branch fired for how many, over time — not a slice.
- The active slice does not say which arm of an `Or` mattered, deliberately
  (finding 28). Per-node contribution semantics would let it, at the cost of
  every node author having to declare them, and it is not obvious the extra
  precision is worth a second thing every `behavior_node!` has to get right.
- Stepping is one decision interval, which is right for behaviour and wrong for
  the fire: at `STEP_S` = 6 s the front barely moves, so "step until something
  happens" is a lot of presses. A "step until this agent's decision changes"
  would be the useful one and needs the loop to know what it is watching.
- `block.unit_futile` ships but nothing in the default policy uses it: the
  shipped model's answer to a stranded unit is a note in the panel, and changing
  that would change the shipped measurements. It is there for an author to wire
  up, and until someone does, "what *should* a stranded engine do" is unanswered.
