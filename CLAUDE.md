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
| Place | One handcrafted area: **Spotorno, Liguria** (44.2265 N, 8.4176 E), UTM 32N |
| Agent types | Civilians (individual humans), ground crews on foot, engines/vehicles. **No aircraft.** |
| Population | ~500–2,000 people, each individually inspectable |
| Scenario length | 1–3 h simulated (initial attack) |
| Civilian model | Wildfire-specific, **built from scratch** — not a port of igad's flood sentiment model |
| Scoring | **None.** But surface information on everything |
| Architecture | New cargo workspace here; `propagator-core` as a path dependency |
| Deferred | Web/wasm build, record/replay debrief |
| Engine | Bevy **0.14** + `bevy_egui` **0.28**, matching igad so its UI ports directly |

---

## Architecture

```
crates/scenario/   baked assets + coordinate frames   (no Bevy, no fire core)
crates/fire/       PROPAGATOR integration, exposure, threat, interventions
crates/abm/        civilians: perception, decision, road network, evacuation
crates/game/       Bevy app
scripts/           offline asset baking (Python) — never runs at game time
data/              the baked scenario
```

The dependency direction is strict: `scenario` knows nothing about Bevy or the
fire core; `fire` depends on `scenario`; `abm` depends on both; `game` depends
on all three. Keep it that way — it is what makes the headless tests fast and
the model testable without a window.

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

---

## Current state

**Working:** terrain mesh (256 chunks), procedural vegetation from the fuel
raster, road ribbons with the drivable/track split, orbit camera, fire
rendered as an age-coloured mesh rebuilt only on sim generation change, egui
"Incident" panel with a logarithmic 1x–512x time slider, play/pause, live
stats.

**Roads** (`crates/game/src/roads.rs`): 1,793 drivable ways and 1,676 tracks as
casing-plus-surface ribbons draped on the 5 m field, resampled to 5 m and
mitred at the joints, in 234 chunks / 656 k triangles. Asphalt dark, tracks
pale dirt — the split is also the drive/walk distinction the ABM routes on.

**Wildfire controls** (`crates/game/src/ui.rs` `wildfire_panel`,
`ignition_edit.rs`, `pick.rs`): the "Wildfire" panel. Wind direction (with a
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
People who were away from home start their own walk out. Rendered as one
capsule per person plus one vehicle per driving household, drawn at 3× life
size (`people::FIGURE_SCALE`) because at command altitude a person is
sub-pixel.

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

**Not built yet:** no debrief. No wasm. No reunification behaviour — people who
are out do not go home for family. No dozers (the only line-cutting resource is a
hand crew, which is why line production is the binding constraint). Units are
selected from the Resources panel, not by clicking them on the map — the
screen-space picker in `inspect` would do it, but three tools already contend for
left-click and a fourth needs a rule, not a patch.

### Commands

```bash
cargo run --release -p game              # play
SPOTORNO_AUTOPLAY=1 cargo run --release -p game   # start running immediately
SPOTORNO_ORDER_AT=600 cargo run --release -p game  # auto-order evacuation at T+10 min
SPOTORNO_ATTACK_AT=300 cargo run --release -p game # commit every unit to the head at T+5 min
cargo test --release                     # everything, ~4 s
cargo test -p abm --release -- --ignored --nocapture     # evacuation timeline + routing cost
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
```

Controls: `space` play/pause · `[` `]` speed · `1`–`4` fire layer · `e` general
evacuation order · `i` arm the ignition tool (then left-click the map) · `esc`
disarm · `r` restart · drag orbit · right-drag pan · scroll zoom.

Suppression: `tab` next unit · `a` attack here · `l` cut line (two clicks) ·
`d` drop here · `x` stand down · `c` request air support. Units are selected in
the **Resources** panel or with `tab`; the order is then placed by clicking the
ground.

**Three tools contend for left-click** — ignition placement, agent inspection,
and suppression orders — and the invariant is that at most one is armed. Arming
either tool disarms the other, `inspect::pick_click` stands down while either is
armed, and `esc` returns to plain inspect-and-orbit. While a tool is armed
left-drag no longer orbits; right-drag pan, scroll and WASD always work, so the
view is never stuck.

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
