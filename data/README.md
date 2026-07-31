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
│   └── [other scenarios]/
│       └── (same structure)
```

### Loading scenarios

**Desktop build:**
- Default: shows **in-game scenario selector panel** at startup (lists all available scenarios with metadata)
- Skip selector: `SPOTORNO_SCENARIO=scenario_id cargo run --release -p game`

**Web build:**
- Default: loads spotorno (no selector in web version)
- Override: `SPOTORNO_WEB_SCENARIO=scenario_id cargo build --release --target wasm32-unknown-unknown -p game`

### Scenario selector UI

When you run the game without setting `SPOTORNO_SCENARIO`, you'll see the scenario selector window on startup:
- Lists all scenarios from `scenarios.json`
- Shows metadata: buildings count, households, people, location
- Shows grid dimensions
- Select a scenario and click "Launch" to start the game
- Close without selecting to use the default scenario

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
| `osm.json` | buildings, roads, water, in world metres | `scenarios/spotorno/` |
| `population.json` | dwellings, households, people | `scenarios/spotorno/` |
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

## Regenerating Spotorno

To regenerate Spotorno scenario assets in `scenarios/spotorno/`:

```bash
export AWS_PROFILE=return
PY=/Users/mirko/dev/fire/propagator/propagator_sim/.venv/bin/python

# 1. rasters: 512x512 window centred on Spotorno (grid col 22674, row 153140)
gdal_translate -srcwin 22418 152884 512 512 \
  /vsis3/cima-propagator-return/cogs/eu/eu_fuel12_utm_32.tif data/scenarios/spotorno/fuel.tif
gdal_translate -srcwin 22418 152884 512 512 \
  /vsis3/cima-propagator-return/cogs/eu/eu_dem_utm_32.tif  data/scenarios/spotorno/dem.tif

# 2. everything downstream
$PY scripts/fetch_osm.py              # cached; delete osm_raw.json to refetch
$PY scripts/build_render_terrain.py --factor 4 --smooth 1.0
$PY scripts/generate_population.py --people 1500 --seed 42
$PY scripts/bake_fire_rasters.py      # GeoTIFF -> raw arrays the Rust game reads
$PY scripts/bake_fuels.py             # eu12 fuel table -> fuels_eu12.json
$PY scripts/run_spotorno.py           # optional: Python-side fire model check
```

`bake_fire_rasters.py` and `bake_fuels.py` are **required** — the game loads
`fuel.i32`, `dem.f64` and `fuels_eu12.json`, not the GeoTIFFs. Pulling a TIFF
decoder into the Rust build just to read two fixed-size grids is not worth
the dependency.

Scripts are designed to output to `data/scenarios/spotorno/` by default (note the
new path). Adjust them if creating a different scenario.

## Current population (seed 42)

750 households, 1,577 people, mean household size 2.10. 61% of households sit
within 100 m of burnable fuel; 28% have no vehicle; 22% of people need
assistance to move; 497 are aged 65+.

## Known caveats

- The source EU DEM has visible rectangular tile seams in hillshade. Present in
  the raw data, not introduced by resampling.
- Elevation is orthometric as supplied; no vertical datum shift applied.
- OSM building `kind` is `yes` for 6,578 of 7,629 footprints, so residential
  classification leans on footprint area and levels rather than tags.
- A genuine 5–10 m DTM (Tinitaly, Regione Liguria geoportale) can replace the
  interpolated render terrain by changing only `load_dem` in
  `scripts/build_render_terrain.py`.
