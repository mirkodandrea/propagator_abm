# Scenario data

The project now supports **multiple scenarios** for testing and validating the ABM.

## Scenario structure

Each scenario is stored in its own directory under `scenarios/`:

```
data/
├── scenarios.json                 # Registry of all available scenarios
├── fuels_eu12.json               # Shared fuel table (used by all scenarios)
├── scenarios/
│   ├── spotorno/
│   │   ├── scenario.json         # Scenario metadata
│   │   ├── render_terrain.f32    # 2048² elevation @ 5 m
│   │   ├── render_terrain.json
│   │   ├── dem.f64               # 512² elevation @ 20 m (fire grid)
│   │   ├── fuel.i32              # 512² fuel classes @ 20 m
│   │   ├── osm.json              # Buildings, roads, water
│   │   └── population.json       # Dwellings, households, people
│   ├── [other scenarios]/
│   │   └── (same structure)
└── behaviours/                    # authored agent behaviour, scenario-independent
    ├── graphs/*.json              # node graphs from the Agent Behaviour Composer
    └── subtypes/*.json            # named behavioural profiles over those graphs
```

`behaviours/` sits beside `scenarios/` rather than inside one because a
behaviour is a hypothesis about people, not a property of a place: the whole
point of running one against `abm_micro` and then against `spotorno` is that
it is the same behaviour. It is loaded from
`$SPOTORNO_DATA/behaviours`, and a missing directory is not an error — the game
falls back to the library built into `crates/behavior/src/defaults.rs`, which is
also what regenerates these files:

```bash
cargo test -p behavior --release -- --ignored write_shipped_library
```

One file per graph and per profile, so a changed threshold is a three-line diff
and two people editing different profiles do not conflict.

### Loading scenarios

**Desktop build:**
- Default: shows **in-game scenario selector panel** at startup (lists all available scenarios with metadata)
- Skip selector: `SPOTORNO_SCENARIO=scenario_id cargo run --release -p game`

**Web build:**
- Shows the same in-game selector and scenarios as the desktop build; all
  registered assets and the shipped behavior library are embedded in WASM.

### Scenario selector UI

When you run the game without setting `SPOTORNO_SCENARIO`, you'll see the scenario selector window on startup:
- Lists all scenarios from `scenarios.json`
- Separates real incidents from development laboratories
- Shows metadata: buildings count, households, people, location
- Shows grid dimensions
- Dev scenarios marked with 🔧 badge
- Select a scenario and click "Launch" to load it and start the game
- Close without selecting to use the default scenario
- Window title updates to show loaded scenario name

The selector supports:
- **Interactive selection** (default): pick scenario from list, click Launch
- **Fast path via env var**: `SPOTORNO_SCENARIO=scenario_id` skips UI, loads directly
- **Headless/CI**: env var auto-selects scenario, game loads without UI interaction

### Synthetic ABM labs

Regenerate the complete development catalog with:

```bash
python3 scripts/generate_synthetic_scenarios.py
```

The script removes generated synthetic directories, never touches `spotorno`,
and writes deterministic populations, terrain, fuel and connected road graphs.
The labs are deliberately focused rather than random miniature towns:

| Scenario | People | Development purpose |
|---|---:|---|
| `abm_micro` | 8 | inspect individual people, households, cars and departures |
| `policy_lab` | 48 | compare warning/evacuation policy timing across four cohorts |
| `suppression_access` | 60 | engine roads, crew-only track, hydrants and open water |
| `road_cutoff` | 90 | fire-cut short exit, vehicle detour and foot escape |
| `congestion_funnel` | 240 | car-heavy evacuation through one collector |
| `fire_mild` / `fire_extreme` | 120 each | controlled pair: identical population and roads, different fuel and slope |
| `town_scale` | 1,200 | whole-ABM development at small-town scale |
| `mass_evacuation` | 5,000 | performance and aggregate behaviour at multi-thousand scale |

Every synthetic `scenario.json` also contains a `development` brief describing
its focus and suggested no-order, early-order, late/zoned-order and suppression
runs. Unknown metadata is intentionally safe for older loaders to ignore.

---

## Spotorno scenario data

Window: 512 x 512 cells @ 20 m = **10.24 x 10.24 km**, EPSG:32632 (WGS84 UTM 32N),
SW corner at UTM `(448360, 4892080)`. Covers Spotorno, Bergeggi, Noli and the
hinterland ridges behind them.

All vector and population assets use a **local metric world frame**: origin at
the window's SW corner, +x east, +y north, metres. The 20 m raster spacing is a
constraint of the fire model alone — nothing else is quantised to it.

## Provenance

| Layer | Source | Status |
|---|---|---|
| Fuel (12-class `eu_fuel12`) | `s3://cima-propagator-return/cogs/eu/eu_fuel12_utm_32.tif` | **Real** |
| DEM, fire grid | `s3://cima-propagator-return/cogs/eu/eu_dem_utm_32.tif`, native 20 m | **Real** |
| DEM, render mesh | above, cubic-resampled to 5 m + smoothed | **Real**, interpolated |
| Building footprints | OpenStreetMap (Overpass) | **Real** |
| Road network | OpenStreetMap, classified drivable vs track/path | **Real** |
| Hydrants & open water | OpenStreetMap | **Real** |
| Dwelling selection & unit counts | Derived from OSM footprint area, levels, kind | Derived |
| Household/person attributes | Sampled from Liguria ISTAT anchors | **Synthetic** |
| Behavioural priors | Wildfire evacuation literature (PADM; McLennan et al.) | **Synthetic**, uncalibrated |
| Weather (wind, moisture) | Scenario-authored | **Synthetic** |

Both COGs share one grid (origin `(0, 7960000)`, 20 m), so DEM and fuel windows
use identical row/col indexing. Access needs `AWS_PROFILE=return`.

## Files (Spotorno scenario)

| File | Contents | Location |
|---|---|---|
| `scenario.json` | Scenario metadata (name, description, grid size, counts) | `scenarios/spotorno/` |
| `fuel.i32` | 512² int fuel classes, 20 m — **read by the game** | `scenarios/spotorno/` |
| `dem.f64` | 512² float elevation, 20 m — **read by the game** | `scenarios/spotorno/` |
| `fuel.tif` | same fuel grid as GeoTIFF — source, for inspection | `scenarios/spotorno/` |
| `dem.tif` | same DEM as GeoTIFF — source, for inspection | `scenarios/spotorno/` |
| `render_terrain.f32` | 2048² float32 heightfield @ 5 m — **read by the game** | `scenarios/spotorno/` |
| `render_terrain.json` | Metadata for render terrain (rows, cols, posting, bounds) | `scenarios/spotorno/` |
| `render_terrain.tif` | same, as GeoTIFF for inspection | `scenarios/spotorno/` |
| `osm.json` | buildings, roads, water, in world metres — each building carries `address`/`locality` where the OSM bake could tell (`scripts/fetch_osm.py::assign_addresses`) | `scenarios/spotorno/` |
| `population.json` | dwellings, households, people — each household carries the `address`/`locality` of the building it is in | `scenarios/spotorno/` |
| `fuels_eu12.json` | the 12-class fuel table — **read by all scenarios** | `data/` (top level) |
| `osm_raw.json` | raw Overpass response (cache; delete to refetch) | `data/` (top level) |

## Source COGs

These URLs are **not** in the propagator_sim repo — its docs redact them to
`<comma-separated DEM COG URLs>` / the `EU_DEM_COGS` / `EU_FUEL_COGS` env
vars, so they are recorded here.

```
s3://cima-propagator-return/cogs/eu/eu_dem_utm_{26..39}.tif
s3://cima-propagator-return/cogs/eu/eu_fuel12_utm_{26..37}.tif
```

Note fuel covers fewer UTM zones than DEM. Liguria is **zone 32**. Both share
one grid — origin `(0, 7960000)`, 20 m pixels — so DEM and fuel windows use
identical row/col indexing.

Access needs `AWS_PROFILE=return`, which lives in `~/.aws/credentials` (not
`~/.aws/config`, so grepping config alone finds nothing). The machine's
`[default]` profile is a different account and gets 403 on this bucket.

## Regenerating a real scenario, or adding a new one

Every real-data script takes `--scenario <id>` (default `spotorno`) and reads
the place's identity — name, UTM zone, window corner, narrative text — from
one place, `scripts/places.py`. Adding a **new** real scenario is: add an
entry there, then run the same five steps against its id.

```bash
export AWS_PROFILE=return
PY=/Users/mirko/dev/fire/propagator/propagator_sim/.venv/bin/python
ID=spotorno   # or a new id registered in scripts/places.py

# 1. clip the fuel/DEM window straight out of the EU COGs (windowed read, not
#    a download of the whole tile) -- this is the step that used to be a
#    one-off gdal_translate session with no script behind it
$PY scripts/clip_cogs.py --scenario $ID

# 2. everything downstream
$PY scripts/fetch_osm.py --scenario $ID              # cached per scenario; delete osm_raw.json to refetch
$PY scripts/build_render_terrain.py --scenario $ID --factor 4 --smooth 1.0
$PY scripts/generate_population.py --scenario $ID --people 1500 --seed 42
$PY scripts/bake_fire_rasters.py --scenario $ID       # GeoTIFF -> raw arrays the Rust game reads
$PY scripts/write_scenario_json.py --scenario $ID     # scenario.json + data/scenarios.json, from the actual bake
$PY scripts/bake_fuels.py                             # eu12 fuel table -> fuels_eu12.json (shared, run once)
```

`bake_fire_rasters.py` and `bake_fuels.py` are **required** — the game loads
`fuel.i32`, `dem.f64` and `fuels_eu12.json`, not the GeoTIFFs. Pulling a TIFF
decoder into the Rust build just to read two fixed-size grids is not worth
the dependency.

For Spotorno specifically, steps 2–5 re-run from the already-committed
`data/spotorno_fuel.tif` / `spotorno_dem.tif` and the cached
`data/osm_raw.json` with **no network call** — every script falls back to
that flat legacy layout when a scenario has no `fuel.tif`/`dem.tif`/
`osm_raw.json` of its own yet. `write_scenario_json.py` derives
`localities` (named places the OSM bake actually found, most-populated
first) and `buildings_count`/`households_count`/`people_count` from the bake
itself rather than having them hand-typed, so they cannot drift from what a
regeneration actually produced.

## Current population (seed 42)

750 households, 1,577 people, mean household size 2.10. 61% of households sit
within 100 m of burnable fuel; 28% have no vehicle; 22% of people need
assistance to move; 497 are aged 65+.

## Shipped behaviour library

One graph, `default-evacuation` (36 nodes, 40 wires), transcribing the
hand-written decision layer in `abm::decide`, and four profiles over it —
`prepared-resident` (25%), `wait-and-see` (45%), `committed-defender` (20%),
`needs-assistance` (10%). They share the graph and differ only in parameter
overrides and starting traits, which is the pattern the composer exists for.

Nothing runs it unless the composer applies it: `Sim::behaviour` is `None` by
default and the shipped hand-written model is what a fresh run uses.

## Known caveats

- The source EU DEM has visible rectangular tile seams in hillshade. Present in
  the raw data, not introduced by resampling.
- Elevation is orthometric as supplied; no vertical datum shift applied.
- OSM building `kind` is `yes` for 6,578 of 7,629 footprints, so residential
  classification leans on footprint area and levels rather than tags.
- A genuine 5–10 m DTM (Tinitaly, Regione Liguria geoportale) can replace the
  interpolated render terrain by changing only `load_dem` in
  `scripts/build_render_terrain.py`.
