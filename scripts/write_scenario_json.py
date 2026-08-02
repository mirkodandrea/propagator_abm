"""Assemble `scenario.json` for a real scenario, and rebuild the registry.

Run last, after `fetch_osm.py` / `generate_population.py` have written the
scenario's `osm.json` and `population.json`: this reads the actual counts and
the localities the OSM bake discovered (see `fetch_osm.py::assign_addresses`)
out of them, rather than repeating numbers by hand, and writes both
`data/scenarios/<id>/scenario.json` and the top-level `data/scenarios.json`
registry that `scenario::ScenarioRegistry::discover` reads.

The registry is rebuilt from every `scenario.json` under `data/scenarios/`,
not just this one, so running this for a new real scenario does not disturb
the synthetic ABM-lab entries `generate_synthetic_scenarios.py` wrote.
"""

import argparse
import json
from datetime import date
from pathlib import Path

import places

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data"
SCENARIOS_DIR = DATA / "scenarios"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenario", required=True, help="place id from scripts/places.py")
    ap.add_argument("--version", default="1.0.0")
    args = ap.parse_args()
    place = places.get(args.scenario)

    scenario_dir = SCENARIOS_DIR / place.id
    osm = json.loads((scenario_dir / "osm.json").read_text())
    population = json.loads((scenario_dir / "population.json").read_text())

    # Localities as the data actually found them (fetch_osm.py's
    # `assign_addresses`), most-populated first -- not hand-maintained, so a
    # re-fetch with a different window can never leave this stale.
    counts: dict[str, int] = {}
    for b in osm["buildings"]:
        if b.get("locality"):
            counts[b["locality"]] = counts.get(b["locality"], 0) + 1
    localities = sorted(counts, key=lambda name: -counts[name])

    existing_provenance = {}
    scenario_json_path = scenario_dir / "scenario.json"
    if scenario_json_path.exists():
        existing_provenance = json.loads(scenario_json_path.read_text()).get("data_provenance", {})

    metadata = {
        "id": place.id,
        "name": place.name,
        "description": place.description,
        "location": place.location,
        "country": place.country,
        "nationality": place.nationality,
        "region": place.region,
        "localities": localities,
        "coordinates": list(place.coordinates),
        "utm_zone": place.utm_zone,
        "utm_corner": list(place.utm_corner),
        "world_size_m": list(place.world_size_m),
        "fire_grid_size": [osm["fire_grid"]["rows"], osm["fire_grid"]["cols"]],
        "fire_cellsize_m": place.fire_cellsize_m,
        "buildings_count": len(osm["buildings"]),
        "households_count": len(population["households"]),
        "people_count": len(population["people"]),
        "scenario_type": "real",
        "data_provenance": existing_provenance or {
            "fuel": f"s3://cima-propagator-return/cogs/eu/eu_fuel12_utm_{place.utm_zone}.tif",
            "dem": f"s3://cima-propagator-return/cogs/eu/eu_dem_utm_{place.utm_zone}.tif",
            "buildings": "OpenStreetMap (Overpass)",
            "roads": "OpenStreetMap",
            "hydrants": "OpenStreetMap",
            "population": "Synthetic, placed on real footprints",
        },
        "creation_date": date.today().isoformat(),
        "version": args.version,
        "tags": place.tags,
        "authors": place.authors,
        "license": place.license,
    }
    scenario_json_path.write_text(json.dumps(metadata, indent=2) + "\n")
    print(f"wrote {scenario_json_path}")
    print(f"localities: {counts}")

    # Rebuild the registry from every scenario directory, so this never
    # clobbers scenarios another script (or another run of this one) wrote.
    registry_path = DATA / "scenarios.json"
    default = "spotorno"
    if registry_path.exists():
        default = json.loads(registry_path.read_text()).get("default", default)

    all_scenarios = []
    for d in sorted(SCENARIOS_DIR.iterdir()):
        sj = d / "scenario.json"
        if sj.exists():
            all_scenarios.append(json.loads(sj.read_text()))
    registry_path.write_text(
        json.dumps({"default": default, "scenarios": all_scenarios}, indent=2) + "\n"
    )
    print(f"wrote {registry_path} ({len(all_scenarios)} scenarios)")


if __name__ == "__main__":
    main()
