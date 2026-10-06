#!/usr/bin/env python3
"""Author the three fictional towns of the kiosk demo (docs/demo-spec.md §5).

Unlike the ABM labs these are *designed for a teaching beat*, not to isolate a
model question, and they are real scenario directories the game loads like any
other.  Nothing here is clipped from data and nothing models a real place.

  demo_borgo   warn early, and only who needs it   a village and two hamlets, three bearings
  demo_valle   the wind changes                    two hamlets either side of the fire, one crosswind
  demo_porto   one road out                        a seaside town, the exit through the pines, a beach

Every town is three *districts* (a household's `locality`), at different
bearings from the fire, so which one the wind threatens is the decision.

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
    landmarks: list = field(default_factory=list)


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


# --------------------------------------------------------------------------- helpers
def along(line, spacing, offset, skip_ends=25.0):
    """House anchors on both sides of a street, every `spacing` metres."""
    pts = []
    for (ax, ay), (bx, by) in zip(line, line[1:]):
        L = math.dist((ax, ay), (bx, by))
        ux, uy = (bx - ax) / L, (by - ay) / L
        nx, ny = -uy, ux
        t = skip_ends
        while t <= L - skip_ends:
            for side in (-1, 1):
                pts.append((ax + ux * t + nx * offset * side, ay + uy * t + ny * offset * side))
            t += spacing
    return pts


def district(name, streets, kinds=None, spacing=34.0, offset=17.0):
    """A named district: its streets (residential roads) and the homes along
    them. `kinds[k]` is the building mix on street k, as (kind, weight) pairs:
    a main street of shops with flats above, a lane of villas with gardens."""
    homes = []
    for k, line in enumerate(streets):
        mix = (kinds or {}).get(k, [("house", 1.0)])
        for i, (x, y) in enumerate(along(line, spacing, offset)):
            # deterministic, so the same street always gets the same frontage
            h = (i * 2654435761 + k * 40503 + len(name) * 97) % 1000 / 1000.0
            acc, kind = 0.0, mix[-1][0]
            for kd, w in mix:
                acc += w
                if h < acc:
                    kind = kd
                    break
            homes.append((x, y, kind))
    return name, streets, homes


def ring(X, Y, cx, cy, rx, ry, w):
    return blob(X, Y, cx, cy, rx + w, ry + w) & ~blob(X, Y, cx, cy, rx, ry)


# --------------------------------------------------------------------------- borgo
# Three districts at three bearings from a fire lit in the pines below the
# village. A south wind drives it at Il Borgo; if the wind backs to the east the
# fire turns onto Le Coste; Il Mulino is upwind either way, so warning it is a
# false alarm (cry-wolf bait).
MAIN = {1: [("shop", .55), ("apartments", .45)]}
VILLAS = {0: [("house", .7), ("villa", .3)], 1: [("villa", .6), ("house", .4)]}
BORGO = [
    district("Il Borgo", [[(1760, 2150), (2240, 2150)], [(1760, 2280), (2240, 2280)], [(1800, 2400), (2200, 2400)]],
             {0: [("terrace", .6), ("house", .4)], 1: [("shop", .5), ("apartments", .5)], 2: [("villa", .5), ("house", .5)]}),
    district("Le Coste", [[(1050, 1650), (1450, 1650)], [(1100, 1770), (1400, 1770)]], VILLAS),
    district("Il Mulino", [[(2850, 1500), (3250, 1500)], [(2900, 1620), (3200, 1620)]], {0: [("house", .8), ("terrace", .2)], 1: [("house", 1.0)]}),
]
# Things a town has besides homes: (kind, x, y, width, depth, name). Placed in
# the gaps between the house rows; homes that would overlap one are dropped.
BORGO_LANDMARKS = [
    ("plaza", 2000, 2215, 70, 44, "Piazza"), ("church", 2075, 2215, 30, 18, "Chiesa"), ("townhall", 1915, 2215, 26, 18, "Municipio"),
    ("school", 2140, 2340, 40, 22, "Scuola"),
    ("fire_station", 1860, 2340, 28, 18, "Vigili del fuoco"), ("fuel", 2560, 2490, 22, 14, "Distributore"),
    ("water_tower", 2300, 2640, 10, 10, "Serbatoio"), ("substation", 1560, 2600, 26, 18, "Cabina elettrica"),
    ("cemetery", 1550, 2380, 50, 36, "Cimitero"), ("parking", 2120, 2470, 40, 24, "Parcheggio"),
    ("assembly", 2000, 2900, 160, 110, "Area di attesa"), ("pitch", 1880, 2930, 70, 44, "Stadio"), ("parking", 2120, 2850, 60, 40, "Parcheggio"),
    ("chapel", 1150, 1710, 14, 10, "Cappella"), ("farm", 1520, 1860, 24, 14, "Cascina"), ("barn", 1560, 1880, 18, 12, "Fienile"),
    ("mill", 3130, 1560, 22, 16, "Mulino"), ("industrial", 2950, 1560, 34, 20, "Officina"), ("silo", 3260, 1560, 8, 8, "Silo"),
]


def borgo_dem(rng):
    X, Y = grid_xy()
    # a hill that rises to the north, a shoulder under the village
    z = 120 + 0.055 * Y + 25 * np.exp(-((X - 2000) / 1500) ** 2) + 12 * np.sin(X / 700)
    return smooth(z + rng.normal(0, .6, z.shape), 4)


def borgo_fuel(rng, roads):
    X, Y = grid_xy()
    f = np.full((GRID, GRID), 9, dtype=np.int32)             # maquis everywhere
    f[(Y < 2050)] = 11                                       # pine below the village
    f[blob(X, Y, 1900, 1450, 650, 520)] = 12                 # dense pine stand: the fire's fuel
    f[blob(X, Y, 1450, 1500, 450, 380)] = 12                 # and toward Le Coste
    f[blob(X, Y, 2750, 1900, 380, 300)] = 3                  # grass patch east
    f[(Y > 3200)] = 8                                        # maquis ridge behind
    f[blob(X, Y, 2000, 2900, 330, 330)] = 0                  # area di attesa: sports ground and car parks
    for cx, cy, rx, ry in ((2000, 2275, 300, 175), (1250, 1710, 230, 110), (3050, 1560, 230, 110)):
        f[ring(X, Y, cx, cy, rx, ry, 140) & (rng.random(f.shape) < .45)] = 1   # gardens, patchy
        f[blob(X, Y, cx, cy, rx, ry)] = 0                    # the built-up core
    return f


def borgo_roads():
    r = []
    def add(name, pts, **k): r.append(road(len(r) + 1, name, pts, **k))
    add("Via Aurelia", [(0, 2520), (1000, 2520), (2000, 2520), (3000, 2520), (4000, 2520)], road_class="secondary")
    add("Via del Borgo", [(2000, 2050), (2000, 2150), (2000, 2280), (2000, 2400), (2000, 2520)], road_class="tertiary")
    add("Via Vecchia", [(1760, 2150), (1760, 2280), (1800, 2400), (1800, 2520)], road_class="residential")
    add("Via Nuova", [(2240, 2150), (2240, 2280), (2200, 2400), (2200, 2520)], road_class="residential")
    add("Strada delle Coste", [(1250, 1650), (1250, 1770), (1250, 2100), (1250, 2520)], road_class="tertiary")
    add("Strada del Mulino", [(3050, 1500), (3050, 1620), (3050, 2100), (3050, 2520)], road_class="tertiary")
    for name, streets, _ in BORGO:
        for k, line in enumerate(streets):
            add(f"{name} {k}", line, road_class="residential")
    # A forestry road down through the pines to where the fire starts.
    add("Strada Forestale", [(2000, 2050), (2000, 1600), (2000, 1100)], road_class="unclassified")
    add("Via del Campo", [(2000, 2520), (2000, 2900)], road_class="tertiary")
    add("Sentiero del Crinale", [(2000, 2900), (2100, 3300), (2300, 3800)], drivable=False, track=True, road_class="path")
    return r


# --------------------------------------------------------------------------- valle
# A fire on the valley road between two hamlets. The east wind drives it at
# Casale Ovest; if the wind swings round to the west it turns on Casale Est,
# which had no reason to worry. Fondovalle is crosswind in both.
VALLE = [
    district("Casale Ovest", [[(1100, 1950), (1450, 1950)], [(1130, 2080), (1420, 2080)]], {0: [("shop", .3), ("terrace", .7)], 1: [("house", .6), ("villa", .4)]}),
    district("Casale Est", [[(2580, 1900), (2930, 1900)], [(2610, 2030), (2900, 2030)]], {0: [("shop", .3), ("terrace", .7)], 1: [("house", .6), ("villa", .4)]}),
    district("Fondovalle", [[(1800, 950), (2200, 950)], [(1850, 1070), (2150, 1070)]], {0: [("shop", .4), ("apartments", .6)], 1: [("house", 1.0)]}),
]
VALLE_LANDMARKS = [
    ("chapel", 1210, 2015, 16, 11, "Chiesetta"), ("plaza", 1340, 2015, 40, 30, "Piazzetta"),
    ("chapel", 2690, 1965, 16, 11, "Chiesetta"), ("plaza", 2820, 1965, 40, 30, "Piazzetta"),
    ("church", 2080, 1010, 30, 18, "Chiesa"), ("townhall", 1920, 1010, 26, 16, "Municipio"), ("school", 2200, 1010, 36, 20, "Scuola"),
    ("fire_station", 2060, 1160, 28, 18, "Vigili del fuoco"), ("fuel", 2050, 1500, 22, 14, "Distributore"),
    ("water_tower", 1650, 1250, 10, 10, "Serbatoio"), ("substation", 2350, 1250, 26, 18, "Cabina elettrica"),
    ("farm", 1600, 2350, 26, 14, "Agriturismo"), ("barn", 1640, 2380, 18, 12, "Fienile"),
    ("farm", 2450, 2350, 26, 14, "Cascina"), ("barn", 2410, 2380, 18, 12, "Fienile"), ("cemetery", 1700, 900, 44, 30, "Cimitero"),
    ("pitch", 2350, 900, 60, 36, "Campo sportivo"),
    ("assembly", 2000, 3300, 160, 110, "Area di attesa"), ("parking", 2120, 3250, 60, 40, "Parcheggio"),
]


def valle_dem(rng):
    X, Y = grid_xy()
    # a shallow valley running N-S, hamlets on the flanks
    z = 150 + 0.025 * Y + 40 * (1 - np.exp(-((X - 2000) / 800) ** 2)) + 6 * np.sin(Y / 500)
    return smooth(z + rng.normal(0, .6, z.shape), 4)


def valle_fuel(rng, roads):
    X, Y = grid_xy()
    f = np.full((GRID, GRID), 9, dtype=np.int32)              # maquis on the flanks
    f[blob(X, Y, 2000, 2000, 450, 1400)] = 3                  # grassy valley floor
    f[blob(X, Y, 1550, 2050, 380, 420)] = 12                  # pines toward Casale Ovest
    f[blob(X, Y, 2450, 1980, 380, 420)] = 12                  # and toward Casale Est
    f[blob(X, Y, 2000, 1450, 300, 250)] = 2                   # meadow above Fondovalle
    f[blob(X, Y, 2000, 3300, 330, 330)] = 0                   # area di attesa up the valley
    for cx, cy, rx, ry in ((1275, 2015, 210, 110), (2755, 1965, 210, 110), (2000, 1010, 230, 110)):
        f[ring(X, Y, cx, cy, rx, ry, 140) & (rng.random(f.shape) < .4)] = 1
        f[blob(X, Y, cx, cy, rx, ry)] = 0
    return f


def valle_roads():
    r = []
    def add(name, pts, **k): r.append(road(len(r) + 1, name, pts, **k))
    add("Strada Provinciale", [(0, 2000), (1100, 2000), (1450, 2010), (2000, 2000), (2580, 1960), (2930, 1960), (4000, 1960)], road_class="secondary")
    add("Strada di Fondovalle", [(2000, 0), (2000, 950), (2000, 1070), (2000, 2000), (2000, 3300), (2000, 4000)], road_class="tertiary")
    add("Via Ovest", [(1275, 1950), (1275, 2080)], road_class="residential")
    add("Via Est", [(2755, 1900), (2755, 2030)], road_class="residential")
    for name, streets, _ in VALLE:
        for k, line in enumerate(streets):
            add(f"{name} {k}", line, road_class="residential")
    return r


# --------------------------------------------------------------------------- porto
# A seaside town with one road out, north through the pines. The fire starts in
# the pines and a north wind drives it at La Pineta, the houses built among the
# trees beside that road, then at the centre. Il Faro, out on the headland, is
# upwind of all of it. The beach does not burn.
PORTO = [
    district("La Pineta", [[(2500, 1600), (2850, 1600)], [(2530, 1730), (2820, 1730)]], {0: [("villa", .7), ("house", .3)], 1: [("villa", 1.0)]}),
    district("Centro", [[(1650, 1000), (2350, 1000)], [(1700, 1130), (2300, 1130)]], {0: [("shop", .45), ("apartments", .35), ("hotel", .2)], 1: [("terrace", .5), ("apartments", .5)]}),
    district("Il Faro", [[(800, 1080), (1150, 1080)], [(830, 1200), (1120, 1200)]], {0: [("house", .6), ("terrace", .4)], 1: [("house", .5), ("villa", .5)]}),
]
PORTO_LANDMARKS = [
    ("plaza", 2000, 1065, 60, 40, "Piazza"), ("church", 2090, 1065, 30, 18, "Chiesa"), ("townhall", 1900, 1065, 26, 16, "Municipio"),
    ("school", 1700, 1250, 40, 22, "Scuola"), ("fire_station", 2200, 1250, 28, 18, "Vigili del fuoco"),
    ("fuel", 2380, 1350, 22, 14, "Distributore"), ("water_tower", 1850, 1330, 10, 10, "Serbatoio"),
    ("substation", 2600, 1300, 26, 18, "Cabina elettrica"), ("harbour", 2000, 760, 120, 26, "Porto"),
    ("lido", 1500, 820, 90, 30, "Lido"), ("lido", 2550, 780, 90, 30, "Lido"), ("lighthouse", 640, 1180, 10, 10, "Faro"),
    ("campsite", 3000, 1900, 140, 90, "Campeggio"), ("parking", 1600, 930, 40, 24, "Parcheggio"),
    ("assembly", 2200, 900, 120, 40, "Area di attesa"),
    ("chapel", 975, 1140, 14, 10, "Cappella"), ("cemetery", 1350, 1300, 44, 30, "Cimitero"),
]


def porto_dem(rng):
    X, Y = grid_xy()
    # land rises from the sea (south) to hills in the north; a headland to the west
    shore = 700 + 120 * np.sin(X / 600)
    z = np.where(Y < shore, -3.0, 4 + 0.06 * (Y - shore))
    z = z + 22 * np.exp(-((X - 900) / 450) ** 2 - ((Y - 1200) / 400) ** 2)
    return smooth(z + rng.normal(0, .5, z.shape), 3)


def porto_fuel(rng, roads):
    X, Y = grid_xy()
    shore = 700 + 120 * np.sin(X / 600)
    f = np.full((GRID, GRID), 8, dtype=np.int32)
    f[Y > 1300] = 11                                         # pines behind the town
    f[blob(X, Y, 2400, 2300, 700, 800)] = 12                 # dense pine where it starts
    f[blob(X, Y, 3300, 1300, 500, 400)] = 9                  # maquis east
    for cx, cy, rx, ry in ((2675, 1665, 210, 110), (2000, 1065, 380, 120), (975, 1140, 210, 110)):
        f[ring(X, Y, cx, cy, rx, ry, 140) & (rng.random(f.shape) < .5)] = 1
        f[blob(X, Y, cx, cy, rx, ry)] = 0
    f[(Y >= shore) & (Y < shore + 110)] = 0                  # beach / promenade
    f[blob(X, Y, 2000, 800, 520, 300)] = 0                   # area di attesa: seafront, car parks, harbour
    f[Y < shore] = 0                                         # sea
    return f


def porto_roads():
    r = []
    def add(name, pts, **k): r.append(road(len(r) + 1, name, pts, **k))
    # One way out of town: north through the pines, past La Pineta.
    add("Via dell'Unica Uscita", [(2350, 1000), (2350, 1130), (2420, 1600), (2420, 1730), (2450, 2600), (2500, 4000)], road_class="residential")
    add("Lungomare", [(700, 900), (975, 890), (1200, 880), (1650, 880), (2000, 880), (2350, 880), (2900, 880)], road_class="tertiary")
    add("Via del Porto", [(2000, 880), (2000, 1000), (2000, 1130)], road_class="residential")
    add("Via del Faro", [(975, 890), (975, 1080), (975, 1200)], road_class="residential")
    add("Viale dei Pini", [(2420, 1600), (2500, 1600)], road_class="residential")
    add("Viale dei Pini Alto", [(2420, 1730), (2530, 1730)], road_class="residential")
    for name, streets, _ in PORTO:
        for k, line in enumerate(streets):
            add(f"{name} {k}", line, road_class="residential")
    add("Sentiero del Faro", [(800, 1200), (500, 1400), (300, 1700)], drivable=False, track=True, road_class="path")
    return r


def homes_of(districts):
    return [(x, y, name, kind) for name, _, homes in districts for (x, y, kind) in homes]


DEMOS = (
    Demo("demo_borgo", "Rocca Ventosa",
         "Un borgo sulla collina e due frazioni. Il vento decide chi e in pericolo: "
         "avvisare in tempo, e solo chi serve.",
         [d[0] for d in BORGO], 250, 101,
         borgo_dem, borgo_fuel, borgo_roads, lambda: homes_of(BORGO),
         [{"id": 1, "kind": "hydrant", "pos": [2000, 2280]}, {"id": 2, "kind": "hydrant", "pos": [1250, 1710]},
          {"id": 3, "kind": "hydrant", "pos": [3050, 1560]}], BORGO_LANDMARKS),
    Demo("demo_valle", "Due Casali",
         "Due casali e una frazione in una valle. Il secondo casale e al sicuro finche il vento non cambia: "
         "bisogna rivalutare la situazione.",
         [d[0] for d in VALLE], 300, 102,
         valle_dem, valle_fuel, valle_roads, lambda: homes_of(VALLE),
         [{"id": 1, "kind": "hydrant", "pos": [1275, 2015]}, {"id": 2, "kind": "hydrant", "pos": [2755, 1965]},
          {"id": 3, "kind": "hydrant", "pos": [2000, 1010]}], VALLE_LANDMARKS),
    Demo("demo_porto", "Porto Pineta",
         "Un paese di mare con una sola strada d'uscita, che passa nella pineta. Con il fuoco alle spalle, "
         "la strada si puo chiudere: la spiaggia non brucia.",
         [d[0] for d in PORTO], 350, 103,
         porto_dem, porto_fuel, porto_roads, lambda: homes_of(PORTO),
         [{"id": 1, "kind": "hydrant", "pos": [2000, 1065]}, {"id": 2, "kind": "hydrant", "pos": [2675, 1665]},
          {"id": 3, "kind": "open_water", "pos": [2000, 600]}], PORTO_LANDMARKS),
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


# Minutes a household takes to get going once it has decided. 12-35 left a
# third of the town caught however early the warning came (district_sweep).
PREP_MIN = (6, 18)


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
        "prep_time_min": float(rng.uniform(PREP_MIN[0], PREP_MIN[1])), "defensible_space": float(rng.uniform(.1, .6)),
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

    def clear_of_landmarks(x, y):
        return all(abs(x - lx) > lw / 2 + 12 or abs(y - ly) > lh / 2 + 10 for _, lx, ly, lw, lh, _ in d.landmarks)
    anchors = [a for a in d.homes() if clear_of_landmarks(a[0], a[1])]
    spec = type("S", (), {"population": "mixed"})  # `traits` only reads .population
    people_total = int(d.households * 3.0)
    sizes = distribute_people(people_total, d.households)
    buildings, dwellings, households, people = [], [], [], []
    for hid, count in enumerate(sizes):
        ax, ay, locality, bkind = anchors[hid % len(anchors)]
        ring_no = hid // len(anchors)
        x = float(np.clip(ax + rng.uniform(-3, 3) + (ring_no % 2) * 15 * (1 if hid % 2 else -1), 12, WORLD - 32))
        y = float(np.clip(ay + rng.uniform(-3, 3) + (ring_no // 2) * 9, 12, WORLD - 28))
        bid = hid + 1
        ring = [[x - 7, y - 5], [x + 7, y - 5], [x + 7, y + 5], [x - 7, y + 5]]
        buildings.append({"id": bid, "kind": bkind, "name": f"Casa {bid}", "centroid": [x, y], "ring": ring, "locality": locality})
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

    # Landmarks: buildings nobody lives in, and the open spaces a town has.
    for k, (kind, lx, ly, lw, lh, name) in enumerate(d.landmarks):
        bid = 100000 + k
        ring = [[lx - lw / 2, ly - lh / 2], [lx + lw / 2, ly - lh / 2], [lx + lw / 2, ly + lh / 2], [lx - lw / 2, ly + lh / 2]]
        buildings.append({"id": bid, "kind": kind, "name": name, "centroid": [float(lx), float(ly)], "ring": ring})
        if kind not in ("harbour", "lido"):
            for gx in np.arange(lx - lw / 2, lx + lw / 2 + 1, cs / 2):
                for gy in np.arange(ly - lh / 2, ly + lh / 2 + 1, cs / 2):
                    fuel[int(np.clip((WORLD - gy) // cs, 0, GRID - 1)), int(np.clip(gx // cs, 0, GRID - 1))] = 0
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
