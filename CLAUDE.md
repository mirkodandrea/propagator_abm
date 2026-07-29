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
crates/fire/       PROPAGATOR integration, exposure, interventions, ignition
crates/game/       Bevy app
scripts/           offline asset baking (Python) — never runs at game time
data/              the baked scenario
```

The dependency direction is strict: `scenario` knows nothing about Bevy or the
fire core; `fire` depends on `scenario`; `game` depends on both. Keep it that
way — it is what makes the headless tests fast and the model testable without
a window.

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

---

## Current state

**Working:** terrain mesh (256 chunks, vertex-coloured by fuel), road ribbons
with the drivable/track split, 750 household entities with status beacons,
orbit camera, fire rendered as an age-coloured mesh rebuilt only on sim
generation change, egui "Incident" panel with a logarithmic 1x–512x time
slider, play/pause, live stats.

Interventions go through the core's own boundary conditions rather than being
bolted on: `Fireline` → `vegetation_changes`, `Water` → `additional_moisture`.

**The shipped scenario** (seed 42, tramontana 35 km/h from N, 6% moisture,
ignition at cell (153, 246), r=250 m):

```
  15 min   23.4 ha   front 586   FLI 80,908 kW/m   threatened  27
  90 min   38.7 ha   front 103   FLI 17,047        threatened  18
 105 min   42.8 ha   front 128   FLI 66,596        threatened 107
 120 min   49.0 ha   front 200   FLI 66,596        threatened 137
```

**Not built yet:** any agent behaviour — households are exposure-coloured
markers, not decision-makers. No crews or engines as entities. No
click-to-inspect. No debrief. No wasm.

### Commands

```bash
cargo run --release -p game              # play
SPOTORNO_AUTOPLAY=1 cargo run --release -p game   # start running immediately
cargo test -p fire --release             # fast headless model tests
cargo test -p fire --release -- --ignored --nocapture   # slow calibration sweeps
```

Controls: `space` play/pause · `[` `]` speed · drag orbit · right-drag pan ·
scroll zoom.

---

## Working agreements

- **Two agents are active.** One (this line of work) owns the model:
  `crates/scenario`, `crates/fire`, `scripts/`, `data/`. Another owns
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
- Nothing is committed to git yet (`git init` done, no commits).
