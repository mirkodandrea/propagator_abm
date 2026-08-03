#!/usr/bin/env python3
"""Build the small, deterministic scenario labs used to develop the ABM.

These are not miniature towns assembled at random.  Each scenario isolates a
question: household decisions, warning policy, apparatus access, a road being
cut, congestion, fire severity, or simulation scale.  Spotorno is real source
data and is never touched by this script.
"""

from __future__ import annotations

import json
import math
import shutil
from dataclasses import dataclass
from pathlib import Path

import numpy as np


ROOT = Path(__file__).resolve().parent.parent
SCENARIOS_DIR = ROOT / "data" / "scenarios"
CREATED = "2026-07-31"


@dataclass(frozen=True)
class ScenarioSpec:
    id: str
    name: str
    focus: str
    people: int
    households: int
    world_m: int
    grid: int
    roads: str
    fire: str
    population: str = "mixed"
    seed: int = 42


SPECS = (
    ScenarioSpec(
        "abm_micro", "ABM Lab: One Street",
        "Inspect individual people, households, vehicle choice, preparation and departure.",
        8, 3, 1600, 64, "one_street", "moderate", "hand_authored", 11,
    ),
    ScenarioSpec(
        "policy_lab", "ABM Lab: Warning Policies",
        "Compare early/general/no-order runs across four visible behavioural cohorts.",
        48, 16, 2400, 96, "small_grid", "moderate", "policy_cohorts", 21,
    ),
    ScenarioSpec(
        "suppression_access", "ABM Lab: Firefighting Access",
        "Exercise engines, hand crews and aircraft against hydrant, road, track and open-water constraints.",
        60, 20, 3200, 128, "suppression", "mixed", "mixed", 31,
    ),
    ScenarioSpec(
        "road_cutoff", "ABM Lab: Cut Road and Rerouting",
        "Put one exit through the fire corridor, retain a long vehicle detour and a foot-only escape.",
        90, 30, 3200, 128, "cutoff", "severe", "mixed", 41,
    ),
    ScenarioSpec(
        "congestion_funnel", "ABM Lab: Single-Exit Congestion",
        "Send many household cars through one shared collector and expose queueing and mode choice.",
        # Sized so the exit actually saturates. The lab shipped with 80
        # households, which is 51 cars over an 85-minute departure spread and a
        # peak of 11 on the road at once -- no traffic model of any kind queues
        # with that, and for as long as it stood the lab could only ever report
        # that congestion did not happen. The exit is one residential street
        # (800 veh/h), so binding it needs demand above ~13 veh/min sustained.
        3000, 1000, 3200, 128, "bottleneck", "moderate", "car_heavy", 51,
    ),
    ScenarioSpec(
        "fire_mild", "ABM Lab: Mild Fire",
        "Controlled severity comparison: patchy low-susceptibility grass, gentle terrain.",
        120, 40, 3200, 128, "severity", "mild", "mixed", 61,
    ),
    ScenarioSpec(
        "fire_extreme", "ABM Lab: Extreme Fire",
        "Controlled severity comparison: the same settlement with continuous high-susceptibility fuel and steep terrain.",
        120, 40, 3200, 128, "severity", "extreme", "mixed", 61,
    ),
    ScenarioSpec(
        "town_scale", "ABM Scale: Small Town",
        "Exercise routing, warnings, household decisions and traffic with roughly one thousand people.",
        1200, 400, 5000, 160, "town", "mixed", "mixed", 71,
    ),
    ScenarioSpec(
        "mass_evacuation", "ABM Scale: Mass Evacuation",
        "Performance and aggregate-behaviour case with five thousand people and several constrained exits.",
        5000, 1667, 6400, 192, "mass", "severe", "mixed", 81,
    ),
)


def road(road_id: int, name: str, line: list[tuple[float, float]], *,
         drivable: bool = True, track: bool = False, road_class: str = "tertiary") -> dict:
    return {
        "id": road_id,
        "class": road_class,
        "drivable": drivable,
        "track": track,
        "name": name,
        "oneway": False,
        "line": [[float(round(x, 2)), float(round(y, 2))] for x, y in line],
    }


def network(kind: str, size: float) -> tuple[list[dict], list[tuple[float, float]]]:
    """Return intentionally connected roads and suitable household anchors."""
    r: list[dict] = []
    homes: list[tuple[float, float]] = []

    def add(name, points, **kwargs):
        r.append(road(len(r) + 1, name, points, **kwargs))

    if kind == "one_street":
        x = size * .50
        add("Evacuation Road", [(x, 0), (x, size * .38), (x, size)])
        add("Home Street", [(size * .25, size * .38), (x, size * .38), (size * .75, size * .38)])
        homes = [(size * f, size * .40) for f in (.35, .50, .65)]

    elif kind in {"small_grid", "severity"}:
        coords = [size * f for f in (.25, .50, .75)]
        for x in coords:
            add(f"North-south {x:.0f}", [(x, 0), *[(x, y) for y in coords], (x, size)])
        for y in coords:
            add(f"East-west {y:.0f}", [(0, y), *[(x, y) for x in coords], (size, y)])
        homes = [(x, y) for y in (size * .27, size * .40) for x in np.linspace(size * .27, size * .73, 20)]

    elif kind == "suppression":
        mid = size * .50
        cross_streets = (size * .22, size * .42, size * .62)
        add("Engine Spine", [(mid, 0), *[(mid, y) for y in cross_streets], (mid, size)])
        for y in cross_streets:
            add(f"Appliance Road {y:.0f}", [(size * .15, y), (mid, y), (size * .85, y)])
        add("Crew Track", [(mid, size * .62), (size * .68, size * .76), (size * .82, size * .88)],
            drivable=False, track=True, road_class="track")
        homes = [(x, y) for y in (size * .22, size * .42) for x in np.linspace(size * .22, size * .78, 10)]

    elif kind == "cutoff":
        # The lower bar is the short exit; the upper/side loop is the detour.
        x1, x2 = size * .32, size * .68
        y1, y2 = size * .24, size * .55
        add("Short South Exit", [(size * .50, 0), (size * .50, y1)])
        add("Settlement Bar", [(x1, y1), (size * .50, y1), (x2, y1)])
        add("West Detour", [(x1, y1), (x1, y2), (0, y2)])
        add("East Detour", [(x2, y1), (x2, y2), (size, y2)])
        add("Loop", [(x1, y2), (size * .50, y2), (x2, y2)])
        add("Foot Ridge Escape", [(size * .50, y1), (size * .55, size * .08), (size * .80, 0)],
            drivable=False, track=True, road_class="path")
        homes = [(x, y1 + 25) for x in np.linspace(x1, x2, 30)]

    elif kind == "bottleneck":
        neck_x, neck_y = size * .50, size * .18
        # One ordinary street, deliberately: the neck is the *class* of road as
        # much as the fact that there is only one of it.
        add("Only Exit", [(neck_x, 0), (neck_x, neck_y), (neck_x, size * .34)],
            road_class="residential")
        xs = [size * f for f in (.30, .40, .50, .60, .70)]
        ys = [size * f for f in (.34, .44, .54)]
        for y in ys:
            add(f"Residential {y:.0f}", [(xs[0], y), *[(x, y) for x in xs[1:]]])
        for x in xs:
            add(f"Collector {x:.0f}", [(x, ys[0]), (x, ys[1]), (x, ys[2])])
        homes = [(x, y + 18) for y in ys for x in np.linspace(xs[0], xs[-1], 60)]

    else:  # town and mass: connected arterials plus denser residential grids.
        n = 7 if kind == "town" else 10
        coords = np.linspace(size * .12, size * .88, n)
        for x in coords:
            add(f"Avenue {x:.0f}", [(x, 0), *[(x, y) for y in coords], (x, size)])
        for y in coords:
            # Only selected streets reach the boundary, creating shared exits.
            ends = (0, size) if round(y) % 2 == 0 else (coords[0], coords[-1])
            add(f"Street {y:.0f}", [(ends[0], y), *[(x, y) for x in coords], (ends[1], y)])
        homes = [(x + 15, y + 15) for y in coords[1:-1] for x in coords]

    return r, homes


def paint_segment(grid: np.ndarray, a: list[float], b: list[float], size: float, value: int, radius: int = 0):
    rows, cols = grid.shape
    length = math.dist(a, b)
    for t in np.linspace(0.0, 1.0, max(2, int(length / (size / cols)) * 2)):
        x, y = a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t
        col = int(np.clip(x / size * cols, 0, cols - 1))
        row = int(np.clip((size - y) / size * rows, 0, rows - 1))
        grid[max(0, row-radius):row+radius+1, max(0, col-radius):col+radius+1] = value


def terrain_and_fuel(spec: ScenarioSpec, roads: list[dict]) -> tuple[np.ndarray, np.ndarray]:
    rng = np.random.default_rng(spec.seed)
    n = spec.grid
    yy, xx = np.mgrid[0:n, 0:n]
    south = 1.0 - yy / max(1, n - 1)

    if spec.fire == "mild":
        dem = 80.0 + south * 12.0 + xx * .015
        fuel = np.ones((n, n), dtype=np.int32)
        fuel[((xx // 6 + yy // 6) % 3) == 0] = 0
    elif spec.fire == "extreme":
        dem = 60.0 + south * 280.0 + xx * .04
        fuel = np.full((n, n), 12, dtype=np.int32)
    elif spec.fire == "severe":
        dem = 70.0 + south * 180.0 + 8.0 * np.sin(xx / 9.0)
        fuel = np.full((n, n), 9, dtype=np.int32)
        fuel[(xx + 2 * yy) % 17 == 0] = 12
    elif spec.fire == "mixed":
        dem = 75.0 + south * 100.0 + 10.0 * np.sin(xx / 12.0)
        fuel = np.full((n, n), 5, dtype=np.int32)
        fuel[(xx // 10 + yy // 8) % 4 == 0] = 8
    else:
        dem = 80.0 + south * 55.0
        fuel = np.full((n, n), 2, dtype=np.int32)

    dem = dem + rng.normal(0.0, 1.2, (n, n))
    # Roads and footprints are non-burnable but only one cell wide, preserving
    # fuel continuity while making the built environment legible in the fire.
    for item in roads:
        if item["drivable"]:
            for a, b in zip(item["line"], item["line"][1:]):
                paint_segment(fuel, a, b, spec.world_m, 0)
    return dem.astype(np.float64), fuel


def traits(spec: ScenarioSpec, household_id: int, rng: np.random.Generator) -> dict:
    cohorts = (
        ("leave_early", "mobile_alert", .85, .85, 4.0, .35),
        ("wait_and_see", "siren", .55, .70, 14.0, .25),
        ("stay_defend", "self_observed", .70, .45, 24.0, .65),
        ("wait_and_see", "none", .25, .20, 35.0, .10),
    )
    if spec.population in {"policy_cohorts", "hand_authored"}:
        intent, channel, risk, trust, prep, defend = cohorts[household_id % len(cohorts)]
    else:
        intent = str(rng.choice(["leave_early", "wait_and_see", "stay_defend"], p=[.25, .55, .20]))
        channel = str(rng.choice(["mobile_alert", "neighbour", "siren", "self_observed", "none"], p=[.40, .15, .20, .20, .05]))
        risk, trust = float(rng.uniform(.25, .9)), float(rng.uniform(.25, .9))
        prep, defend = float(rng.uniform(4, 35)), float(rng.uniform(.1, .75))
    if spec.population == "car_heavy":
        vehicles = 0 if household_id % 10 == 0 else (2 if household_id % 4 == 0 else 1)
    else:
        vehicles = (0, 1, 1, 2)[household_id % 4]
    return {
        "vehicles": vehicles, "risk_perception": risk,
        "prior_fire_experience": household_id % 7 == 0,
        "warning_channel": channel, "trust_authority": trust, "intent": intent,
        "prep_time_min": prep, "defensible_space": defend,
        "has_pets_livestock": household_id % 6 == 0,
    }


def distribute_people(total: int, households: int) -> list[int]:
    sizes = [total // households] * households
    for i in range(total % households):
        sizes[i] += 1
    assert all(1 <= n <= 5 for n in sizes)
    return sizes


def create(spec: ScenarioSpec) -> dict:
    out = SCENARIOS_DIR / spec.id
    out.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(spec.seed)
    roads, anchors = network(spec.roads, float(spec.world_m))
    dem, fuel = terrain_and_fuel(spec, roads)
    cellsize = spec.world_m / spec.grid

    buildings, dwellings, households, people = [], [], [], []
    sizes = distribute_people(spec.people, spec.households)
    for hid, count in enumerate(sizes):
        ax, ay = anchors[hid % len(anchors)]
        ring_no = hid // len(anchors)
        x = float(np.clip(ax + (ring_no % 5 - 2) * 7 + rng.uniform(-4, 4), 12, spec.world_m - 32))
        y = float(np.clip(ay + (ring_no // 5) * 7 + rng.uniform(-4, 4), 12, spec.world_m - 28))
        bid = hid + 1
        ring = [[x - 7, y - 5], [x + 7, y - 5], [x + 7, y + 5], [x - 7, y + 5]]
        buildings.append({"id": bid, "kind": "residential", "name": f"House {bid}", "centroid": [x, y], "ring": ring})
        col = int(np.clip(x // cellsize, 0, spec.grid - 1))
        row = int(np.clip((spec.world_m - y) // cellsize, 0, spec.grid - 1))
        fuel[row, col] = 0
        dwellings.append({
            "osm_id": bid, "kind": "residential", "pos": [x, y], "area_m2": 140.0,
            "levels": 2, "units": 1, "cell": [row, col], "dist_to_fuel_m": cellsize,
            "fuel_at_site": 0,
        })
        member_ids = list(range(len(people), len(people) + count))
        household = {
            "id": hid, "building": bid, "pos": [x, y], "cell": [row, col], "size": count,
            "dist_to_fuel_m": cellsize, **traits(spec, hid, rng), "status": "normal", "members": member_ids,
        }
        households.append(household)
        for offset, pid in enumerate(member_ids):
            age = (8, 17, 34, 48, 72)[(hid + offset) % 5]
            needs_help = age >= 72 or (pid % 19 == 0)
            people.append({
                "id": pid, "household": hid, "age": age,
                "walk_speed": 0.85 if needs_help else 1.35 + (pid % 4) * .08,
                "needs_assistance": needs_help, "at_home": pid % 7 != 0,
            })

    water = [
        {"id": 1, "kind": "hydrant", "pos": [spec.world_m * .50, spec.world_m * .20]},
        {"id": 2, "kind": "hydrant", "pos": [spec.world_m * .50, spec.world_m * .55]},
        {"id": 3, "kind": "open_water", "pos": [spec.world_m * .08, spec.world_m * .08]},
    ]
    vectors = {
        "world_size_m": [float(spec.world_m), float(spec.world_m)],
        "fire_grid": {"rows": spec.grid, "cols": spec.grid, "cellsize": cellsize},
        "buildings": buildings, "roads": roads, "water": water,
    }
    population = {"synthetic": True, "seed": spec.seed, "dwellings": dwellings, "households": households, "people": people}

    # Render terrain at fire-grid resolution: synthetic labs favour fast load
    # and inspection; visual detail belongs in the real Spotorno assets.
    render = dem.astype(np.float32)
    dem.astype("<f8").tofile(out / "dem.f64")
    fuel.astype("<i4").tofile(out / "fuel.i32")
    render.astype("<f4").tofile(out / "render_terrain.f32")
    (out / "osm.json").write_text(json.dumps(vectors, indent=2) + "\n")
    (out / "population.json").write_text(json.dumps(population, indent=2) + "\n")
    (out / "render_terrain.json").write_text(json.dumps({
        "rows": spec.grid, "cols": spec.grid, "posting_m": cellsize,
        "world_size_m": [float(spec.world_m), float(spec.world_m)],
        "elev_min": float(dem.min()), "elev_max": float(dem.max()),
    }, indent=2) + "\n")

    metadata = {
        "id": spec.id, "name": spec.name, "description": spec.focus,
        "location": "Synthetic ABM laboratory", "country": "Synthetic",
        "coordinates": [0.0, 0.0], "utm_zone": 0,
        "world_size_m": [float(spec.world_m), float(spec.world_m)],
        "fire_grid_size": [spec.grid, spec.grid], "buildings_count": len(buildings),
        "households_count": len(households), "people_count": len(people),
        "scenario_type": "synthetic", "creation_date": CREATED, "version": "2.0.0",
        "tags": ["dev", "abm-lab", spec.roads, f"fire-{spec.fire}", spec.population],
        "is_dev": True,
        "vr_palette": {"void": [.02, .03, .08], "grid": [0.0, .85, 1.0], "accent": [.9, .95, 1.0]},
        "development": {
            "focus": spec.focus, "fire_profile": spec.fire, "road_profile": spec.roads,
            "population_profile": spec.population,
            "suggested_runs": ["no evacuation order", "early general order", "late/zoned order", "suppression before evacuation"],
        },
    }
    (out / "scenario.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(f"{spec.id:22} {len(people):5} people  {len(households):4} households  {len(roads):3} roads")
    return metadata


def remove_old_synthetic_scenarios():
    """Remove generated/dev scenario directories, explicitly never Spotorno."""
    for path in SCENARIOS_DIR.iterdir():
        if not path.is_dir() or path.name == "spotorno":
            continue
        metadata_path = path / "scenario.json"
        try:
            metadata = json.loads(metadata_path.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        if metadata.get("scenario_type") == "synthetic" or metadata.get("is_dev") is True:
            shutil.rmtree(path)


def main():
    remove_old_synthetic_scenarios()
    generated = [create(spec) for spec in SPECS]

    # Every *real* scenario on disk, not just Spotorno.  This rebuilt the
    # registry from `[spotorno, *generated]`, which was correct exactly as long
    # as Spotorno was the only real place -- and `mati`, `pedrogao` and
    # `rhodes` were baked later, so running this script silently dropped three
    # scenarios out of the selector while reporting that it had "preserved
    # Spotorno".  Nothing failed: `Scenario::load_by_id` reads the per-scenario
    # directory rather than the registry, so the model tests kept passing on
    # windows the game could no longer offer.  Deriving the list from the
    # directory is the same lesson as finding 33 -- a hand-maintained list of
    # what exists is a claim, and the only thing that checks it is the data.
    real = sorted(
        p.name
        for p in SCENARIOS_DIR.iterdir()
        if p.is_dir() and (p / "scenario.json").exists()
        and p.name not in {spec.id for spec in SPECS}
    )
    kept = [json.loads((SCENARIOS_DIR / name / "scenario.json").read_text()) for name in real]
    registry = {"default": "spotorno", "scenarios": [*kept, *generated]}
    (ROOT / "data" / "scenarios.json").write_text(json.dumps(registry, indent=2) + "\n")
    print(
        f"\nWrote {len(generated)} synthetic ABM labs; "
        f"preserved {len(kept)} real scenarios ({', '.join(real)})."
    )


if __name__ == "__main__":
    main()
