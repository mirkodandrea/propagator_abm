#!/usr/bin/env python3
"""Author the three fictional towns of the kiosk demo (docs/demo-spec.md §5).

Unlike the ABM labs these are *designed for a teaching beat*, not to isolate a
model question, and they are real scenario directories the game loads like any
other.  Nothing here is clipped from data and nothing models a real place.

  demo_borgo   warn early       hillside village, uphill fire, two exits
  demo_valle   the wind changes two hamlets, the second is safe until the shift
  demo_porto   one road out     coastal town, single exit, a beach, spotting

Frame: x east, y north, metres, SW corner origin; raster row 0 is the north edge.
Fuel classes are `eu_fuel12` ids: 0 non-vegetated, 1-2 grass, 5-6 conifer,
7-9 shrub/maquis (the spotters, finding 41).

The registry is *derived from the directory* (finding 39a): this script only
writes its own three directories and then rebuilds data/scenarios.json from
every scenario.json on disk, so it can run in any order with the other
generators.
"""

from __future__ import annotations

import json
import math
import sys
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
SCENARIOS_DIR = ROOT / "data" / "scenarios"


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


def paint_segment(grid: np.ndarray, a: list[float], b: list[float], size: float, value: int, radius: int = 0):
    rows, cols = grid.shape
    length = math.dist(a, b)
    for t in np.linspace(0.0, 1.0, max(2, int(length / (size / cols)) * 2)):
        x, y = a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t
        col = int(np.clip(x / size * cols, 0, cols - 1))
        row = int(np.clip((size - y) / size * rows, 0, rows - 1))
        grid[max(0, row-radius):row+radius+1, max(0, col-radius):col+radius+1] = value


def distribute_people(total: int, households: int) -> list[int]:
    sizes = [total // households] * households
    for i in range(total % households):
        sizes[i] += 1
    assert all(1 <= n <= 5 for n in sizes)
    return sizes


CREATED = "2026-10-06"
WORLD = 4000
GRID = 200  # 20 m, fixed by the fire model


@dataclass
class Demo:
    id: str
    name: str
    description: str
    localities: list[str]
    households: int
    seed: int
    dem: callable
    fuel: callable
    roads: callable
    homes: callable
    water: list[dict] = field(default_factory=list)


def grid_xy():
    """Cell-centre world coordinates, shaped like the raster (row 0 = north)."""
    cs = WORLD / GRID
    col = (np.arange(GRID) + .5) * cs
    row = WORLD - (np.arange(GRID) + .5) * cs
    return np.meshgrid(col, row)  # X[row, col], Y[row, col]


def smooth(a, n=3):
    for _ in range(n):
        p = np.pad(a, 1, mode="edge")
        a = (p[:-2, 1:-1] + p[2:, 1:-1] + p[1:-1, :-2] + p[1:-1, 2:] + 4 * a) / 8
    return a


def blob(X, Y, cx, cy, rx, ry):
    return ((X - cx) / rx) ** 2 + ((Y - cy) / ry) ** 2 < 1.0


# --------------------------------------------------------------------------- borgo
def borgo_dem(rng):
    X, Y = grid_xy()
    # a gentle hill that rises to the north, a shoulder under the village
    z = 120 + 0.055 * Y + 25 * np.exp(-((X - 2000) / 1500) ** 2) + 12 * np.sin(X / 700)
    return smooth(z + rng.normal(0, .6, z.shape), 4)


def borgo_fuel(rng, roads):
    X, Y = grid_xy()
    f = np.full((GRID, GRID), 9, dtype=np.int32)             # maquis everywhere
    f[(Y < 2100)] = 11                                       # pine below the village
    f[blob(X, Y, 1500, 1500, 700, 450)] = 12                 # dense pine stand: the fire's fuel
    f[blob(X, Y, 2800, 1700, 500, 400)] = 3                  # grass patch east
    f[(Y > 3200)] = 8                                        # maquis ridge behind
    f[blob(X, Y, 2050, 2650, 560, 300)] = 0                  # the village core
    for g in (blob(X, Y, 2050, 2650, 760, 440) & ~blob(X, Y, 2050, 2650, 560, 300),):
        f[g & (rng.random(f.shape) < .45)] = 1               # gardens, patchy
    return f


def borgo_roads():
    r = []
    def add(name, pts, **k): r.append(road(len(r) + 1, name, pts, **k))
    add("Via Aurelia Ovest", [(0, 2650), (900, 2650), (1500, 2650), (2000, 2650)], road_class="secondary")
    add("Via Aurelia Est", [(2000, 2650), (2600, 2650), (3200, 2650), (4000, 2650)], road_class="secondary")
    for x in (1700, 2000, 2300):
        add(f"Via {x}", [(x, 2400), (x, 2650), (x, 2900)], road_class="residential")
    add("Via Alta", [(1550, 2850), (2000, 2850), (2450, 2850)], road_class="residential")
    add("Via Bassa", [(1550, 2450), (2000, 2450), (2450, 2450)], road_class="residential")
    add("Via Bassa O", [(1550, 2450), (1550, 2650), (1550, 2850)], road_class="residential")
    add("Via Bassa E", [(2450, 2450), (2450, 2650), (2450, 2850)], road_class="residential")
    add("Sentiero del Crinale", [(2000, 2900), (2100, 3300), (2300, 3800)],
        drivable=False, track=True, road_class="path")
    return r


def borgo_homes():
    pts = []
    for y in (2480, 2530, 2780, 2820, 2880):
        pts += [(x, y) for x in np.linspace(1580, 2430, 18)]
    pts += [(x, 2700) for x in np.linspace(1580, 2430, 18)]
    return pts


# --------------------------------------------------------------------------- valle
def valle_dem(rng):
    X, Y = grid_xy()
    # a shallow valley running N-S between two flanks, hamlets on the flanks
    z = 150 + 0.025 * Y + 40 * (1 - np.exp(-((X - 2000) / 800) ** 2)) + 6 * np.sin(Y / 500)
    return smooth(z + rng.normal(0, .6, z.shape), 4)


def valle_fuel(rng, roads):
    X, Y = grid_xy()
    f = np.full((GRID, GRID), 9, dtype=np.int32)
    f[blob(X, Y, 1200, 1500, 650, 600)] = 12                 # pines where the fire starts
    f[blob(X, Y, 2000, 2000, 700, 900)] = 3                  # grassy valley floor: fast carrier
    f[blob(X, Y, 2900, 2800, 750, 650)] = 8                  # maquis around the far hamlet
    f[blob(X, Y, 1050, 2550, 380, 300)] = 0                  # hamlet A (near the fire)
    f[blob(X, Y, 3050, 2350, 380, 300)] = 0                  # hamlet B (safe, until the shift)
    for cx, cy in ((1050, 2550), (3050, 2350)):
        ring = blob(X, Y, cx, cy, 540, 440) & ~blob(X, Y, cx, cy, 380, 300)
        f[ring & (rng.random(f.shape) < .4)] = 1
    return f


def valle_roads():
    r = []
    def add(name, pts, **k): r.append(road(len(r) + 1, name, pts, **k))
    add("Strada Provinciale", [(0, 2550), (1050, 2550), (2000, 2450), (3050, 2350), (4000, 2350)], road_class="secondary")
    add("Via del Fondovalle", [(1050, 2550), (1050, 3300), (1050, 4000)], road_class="tertiary")
    add("Via del Colle", [(3050, 2350), (3050, 1500), (3050, 0)], road_class="tertiary")
    for cx, cy in ((1050, 2550), (3050, 2350)):
        add(f"Via Centro {cx}", [(cx - 300, cy + 130), (cx, cy + 130), (cx + 300, cy + 130)], road_class="residential")
        add(f"Via Sotto {cx}", [(cx - 300, cy - 130), (cx, cy - 130), (cx + 300, cy - 130)], road_class="residential")
        add(f"Via Nord {cx}", [(cx - 300, cy - 130), (cx - 300, cy), (cx - 300, cy + 130)], road_class="residential")
        add(f"Via Sud {cx}", [(cx + 300, cy - 130), (cx + 300, cy), (cx + 300, cy + 130)], road_class="residential")
    return r


def valle_homes():
    pts = []
    for cx, cy in ((1050, 2550), (3050, 2350)):
        for dy in (-160, -100, 100, 160):
            pts += [(x, cy + dy) for x in np.linspace(cx - 290, cx + 290, 10)]
    return pts


# --------------------------------------------------------------------------- porto
def porto_dem(rng):
    X, Y = grid_xy()
    # land rises from the sea (south) to hills in the north; a headland to the west
    shore = 700 + 120 * np.sin(X / 600)
    z = np.where(Y < shore, -3.0, 4 + 0.06 * (Y - shore))
    z = z + 22 * np.exp(-((X - 500) / 450) ** 2 - ((Y - 1200) / 500) ** 2)
    return smooth(z + rng.normal(0, .5, z.shape), 3)


def porto_fuel(rng, roads):
    X, Y = grid_xy()
    shore = 700 + 120 * np.sin(X / 600)
    f = np.full((GRID, GRID), 8, dtype=np.int32)
    f[Y > 2600] = 11
    f[blob(X, Y, 1100, 1900, 800, 650)] = 12                 # pine behind the town
    f[blob(X, Y, 3100, 2000, 600, 550)] = 9                  # dense maquis east
    f[Y < shore] = 0                                         # sea
    f[(Y >= shore) & (Y < shore + 90)] = 0                   # beach / promenade
    f[blob(X, Y, 2000, 1050, 800, 230)] = 0                  # the town
    for g in (blob(X, Y, 2000, 1050, 1000, 330) & ~blob(X, Y, 2000, 1050, 800, 230),):
        f[g & (rng.random(f.shape) < .5)] = 1
    f[(Y < shore) ] = 0
    return f


def porto_roads():
    r = []
    def add(name, pts, **k): r.append(road(len(r) + 1, name, pts, **k))
    # One way out of town: a single residential-class road north through the pines.
    add("Via dell'Unica Uscita", [(2000, 1000), (2000, 1700), (2000, 2600), (2000, 4000)], road_class="residential")
    add("Lungomare", [(1250, 880), (1650, 880), (2000, 880), (2350, 880), (2750, 880)], road_class="tertiary")
    add("Via Mercato", [(1250, 1000), (1650, 1000), (2000, 1000), (2350, 1000), (2750, 1000)], road_class="residential")
    add("Via Chiesa", [(1250, 1130), (1650, 1130), (2000, 1130), (2350, 1130), (2750, 1130)], road_class="residential")
    for x in (1250, 1650, 2350, 2750):
        add(f"Vicolo {x}", [(x, 880), (x, 1000), (x, 1130)], road_class="residential")
    add("Sentiero del Faro", [(1250, 900), (700, 1100), (400, 1400)],
        drivable=False, track=True, road_class="path")
    return r


def porto_homes():
    pts = []
    for y in (930, 1060, 1180):
        pts += [(x, y) for x in np.linspace(1270, 2730, 24)]
    return pts


DEMOS = (
    Demo("demo_borgo", "Borgo San Fiorenzo",
         "Un borgo sulla collina con due vie d'uscita. Il vento spinge il fuoco in salita verso le case: "
         "dare l'allarme in tempo cambia chi si salva.",
         ["Borgo San Fiorenzo", "Colle Alto", "Case Bassa"], 250, 101,
         borgo_dem, borgo_fuel, borgo_roads, borgo_homes,
         [{"id": 1, "kind": "hydrant", "pos": [1980, 2640]}, {"id": 2, "kind": "hydrant", "pos": [2320, 2640]}]),
    Demo("demo_valle", "Valle dei Pini",
         "Due borgate in una valle. La seconda e al sicuro finche il vento non cambia: "
         "bisogna rivalutare la situazione.",
         ["Valle dei Pini", "Borgata Alta", "Borgata Bassa"], 300, 102,
         valle_dem, valle_fuel, valle_roads, valle_homes,
         [{"id": 1, "kind": "hydrant", "pos": [1040, 2540]}, {"id": 2, "kind": "hydrant", "pos": [3040, 2340]}]),
    Demo("demo_porto", "Porto Rosso",
         "Un paese di mare con una sola strada d'uscita. Con il fuoco alle spalle, "
         "le code e le scintille contano quanto le fiamme.",
         ["Porto Rosso", "Lungomare", "Punta del Faro"], 350, 103,
         porto_dem, porto_fuel, porto_roads, porto_homes,
         [{"id": 1, "kind": "hydrant", "pos": [1990, 1010]}, {"id": 2, "kind": "hydrant", "pos": [2010, 1140]},
          {"id": 3, "kind": "open_water", "pos": [2000, 400]}]),
)


def connect_junctions(roads: list[dict]) -> None:
    """Insert every other road's vertices that lie on a segment, so a crossing
    is a node in the routing graph.  Without this two roads can cross on the
    map and be unreachable from each other -- nobody is ever evacuated, and
    nothing errors (the `cutoff` column is the only tell)."""
    verts = [tuple(v) for r in roads for v in r["line"]]
    for r in roads:
        out = [r["line"][0]]
        for a, b in zip(r["line"], r["line"][1:]):
            ax, ay, bx, by = a[0], a[1], b[0], b[1]
            L2 = (bx - ax) ** 2 + (by - ay) ** 2
            hits = []
            for vx, vy in verts:
                if L2 == 0:
                    continue
                t = ((vx - ax) * (bx - ax) + (vy - ay) * (by - ay)) / L2
                if 0.001 < t < 0.999:
                    px, py = ax + t * (bx - ax), ay + t * (by - ay)
                    if (px - vx) ** 2 + (py - vy) ** 2 < 4.0:
                        hits.append((t, [float(vx), float(vy)]))
            out += [h for _, h in sorted(hits)]
            out.append(b)
        r["line"] = out


def crossings(roads: list[dict]) -> None:
    """Add a vertex on both roads wherever two segments properly cross."""
    def seg_x(p, q, r, s):
        d = (q[0]-p[0])*(s[1]-r[1]) - (q[1]-p[1])*(s[0]-r[0])
        if abs(d) < 1e-9:
            return None
        t = ((r[0]-p[0])*(s[1]-r[1]) - (r[1]-p[1])*(s[0]-r[0])) / d
        u = ((r[0]-p[0])*(q[1]-p[1]) - (r[1]-p[1])*(q[0]-p[0])) / d
        return (t, u) if 0 < t < 1 and 0 < u < 1 else None
    new = {id(r): [] for r in roads}
    for i, A in enumerate(roads):
        for B in roads[i + 1:]:
            for ia, (p, q) in enumerate(zip(A["line"], A["line"][1:])):
                for ib, (r_, s_) in enumerate(zip(B["line"], B["line"][1:])):
                    h = seg_x(p, q, r_, s_)
                    if h:
                        pt = [p[0] + h[0]*(q[0]-p[0]), p[1] + h[0]*(q[1]-p[1])]
                        new[id(A)].append((ia, h[0], pt))
                        new[id(B)].append((ib, h[1], pt))
    for r in roads:
        ins = sorted(new[id(r)], key=lambda x: (x[0], x[1]), reverse=True)
        for idx, _, pt in ins:
            r["line"].insert(idx + 1, [round(pt[0], 2), round(pt[1], 2)])


def demo_traits(hid: int, rng: np.random.Generator) -> dict:
    """A village that underrates the fire, and trusts the people who tell it otherwise.

    A 4 km window sits entirely inside `SEE_RANGE_M` (2.5 km), so with
    Spotorno-style traits everyone sees the fire from minute zero, decides for
    themselves, and an order changes nothing -- the beat this demo exists to
    teach would not be there to teach.  Measured: `none` and `T+0` differ by two
    households.  So the villagers here are *complacent*: low risk perception,
    wait-and-see, long preparation.  That is a authored population, the same
    way a real one would be a bake; no model number moves.
    """
    intent = str(rng.choice(["wait_and_see", "leave_early", "stay_defend"], p=[.92, .05, .03]))
    channel = str(rng.choice(["mobile_alert", "neighbour", "siren", "self_observed", "none"],
                             p=[.45, .15, .25, .10, .05]))
    vehicles = (0, 1, 1, 2)[hid % 4]
    return {
        "vehicles": vehicles, "risk_perception": float(rng.uniform(.08, .30)),
        "prior_fire_experience": False, "warning_channel": channel,
        "trust_authority": float(rng.uniform(.55, .95)), "intent": intent,
        "prep_time_min": float(rng.uniform(12, 35)), "defensible_space": float(rng.uniform(.1, .6)),
        "has_pets_livestock": hid % 6 == 0,
    }


def create(d: Demo) -> dict:
    out = SCENARIOS_DIR / d.id
    out.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(d.seed)
    dem = d.dem(rng).astype(np.float64)
    roads = d.roads()
    crossings(roads)
    connect_junctions(roads)
    fuel = d.fuel(rng, roads)
    cs = WORLD / GRID
    for item in roads:
        if item["drivable"]:
            for a, b in zip(item["line"], item["line"][1:]):
                paint_segment(fuel, a, b, WORLD, 0)

    anchors = d.homes()
    spec = type("S", (), {"population": "mixed"})  # `traits` only reads .population
    people_total = int(d.households * 3.0)
    sizes = distribute_people(people_total, d.households)
    buildings, dwellings, households, people = [], [], [], []
    for hid, count in enumerate(sizes):
        ax, ay = anchors[hid % len(anchors)]
        ring_no = hid // len(anchors)
        x = float(np.clip(ax + rng.uniform(-6, 6) + ring_no * 13, 12, WORLD - 32))
        y = float(np.clip(ay + rng.uniform(-3, 3), 12, WORLD - 28))
        bid = hid + 1
        ring = [[x - 7, y - 5], [x + 7, y - 5], [x + 7, y + 5], [x - 7, y + 5]]
        locality = d.localities[hid % len(d.localities)]
        buildings.append({"id": bid, "kind": "residential", "name": f"Casa {bid}", "centroid": [x, y], "ring": ring})
        col = int(np.clip(x // cs, 0, GRID - 1))
        row = int(np.clip((WORLD - y) // cs, 0, GRID - 1))
        fuel[row, col] = 0
        dwellings.append({"osm_id": bid, "kind": "residential", "pos": [x, y], "area_m2": 140.0,
                          "levels": 2, "units": 1, "cell": [row, col], "dist_to_fuel_m": cs, "fuel_at_site": 0})
        member_ids = list(range(len(people), len(people) + count))
        households.append({"id": hid, "building": bid, "pos": [x, y], "cell": [row, col], "size": count,
                           "dist_to_fuel_m": cs, **demo_traits(hid, rng), "status": "normal",
                           "members": member_ids, "locality": locality})
        for offset, pid in enumerate(member_ids):
            age = (8, 17, 34, 48, 72)[(hid + offset) % 5]
            needs_help = age >= 72 or (pid % 19 == 0)
            people.append({"id": pid, "household": hid, "age": age,
                           "walk_speed": 0.85 if needs_help else 1.35 + (pid % 4) * .08,
                           "needs_assistance": needs_help, "at_home": pid % 7 != 0})

    vectors = {"world_size_m": [float(WORLD)] * 2,
               "fire_grid": {"rows": GRID, "cols": GRID, "cellsize": cs},
               "buildings": buildings, "roads": roads, "water": d.water}
    population = {"synthetic": True, "seed": d.seed, "dwellings": dwellings,
                  "households": households, "people": people}
    dem.astype("<f8").tofile(out / "dem.f64")
    fuel.astype("<i4").tofile(out / "fuel.i32")
    dem.astype("<f4").tofile(out / "render_terrain.f32")
    (out / "osm.json").write_text(json.dumps(vectors, indent=2) + "\n")
    (out / "population.json").write_text(json.dumps(population, indent=2) + "\n")
    (out / "render_terrain.json").write_text(json.dumps({
        "rows": GRID, "cols": GRID, "posting_m": cs, "world_size_m": [float(WORLD)] * 2,
        "elev_min": float(dem.min()), "elev_max": float(dem.max())}, indent=2) + "\n")
    meta = {
        "id": d.id, "name": d.name, "description": d.description,
        "location": "Luogo immaginario", "country": "Italia", "nationality": "Italian",
        "region": "an invented town in central Italy", "localities": d.localities,
        "coordinates": [0.0, 0.0], "utm_zone": 0,
        "world_size_m": [float(WORLD)] * 2, "fire_grid_size": [GRID, GRID],
        "buildings_count": len(buildings), "households_count": len(households),
        "people_count": len(people), "scenario_type": "synthetic",
        "creation_date": CREATED, "version": "1.0.0",
        "tags": ["demo", "kiosk"], "is_dev": False,
    }
    (out / "scenario.json").write_text(json.dumps(meta, indent=2) + "\n")
    print(f"{d.id:12} {len(people):5} people {len(households):4} households {len(roads):3} roads")
    return meta


def rebuild_registry():
    """Every scenario.json on disk, keeping the existing default and order."""
    path = ROOT / "data" / "scenarios.json"
    reg = json.loads(path.read_text())
    ids = {s["id"] for s in reg["scenarios"]}
    for d in DEMOS:
        meta = json.loads((SCENARIOS_DIR / d.id / "scenario.json").read_text())
        if d.id in ids:
            reg["scenarios"] = [meta if s["id"] == d.id else s for s in reg["scenarios"]]
        else:
            reg["scenarios"].append(meta)
    path.write_text(json.dumps(reg, indent=2) + "\n")


if __name__ == "__main__":
    for d in DEMOS:
        create(d)
    rebuild_registry()
