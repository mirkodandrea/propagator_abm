"""Settlements, roads, safe areas and population on a chosen terrain
(02-SCENARIO-FACTORY §C-D), written in the existing `Scenario` format.

A layout is plain data: where each settlement sits and which way its streets
run, the waypoints the main roads must pass, and the open spaces a town has.
Road geometry between waypoints comes from `roads.least_cost_path`, so it
follows the orography. Fuel is then rewritten where people have built: houses,
paved roads, the dense core and irrigated plots are non-vegetated (eu_fuel12 0),
which is what makes the in-town assembly areas pass the model's own refuge
test (`abm::refuge`: <=12% burnable within 300 m).
"""

from __future__ import annotations

import dataclasses

import json
import math
from dataclasses import dataclass, field

import numpy as np

from . import roads as R
from .export import DATA, ROOT
from .grid import CELL_M, N, WORLD_M, slope_aspect, to_cell

# --------------------------------------------------------------------------- layout data


@dataclass
class Settlement:
    name: str
    centre: tuple[float, float]
    axis_deg: float            # bearing of the main street, clockwise from north
    streets: list              # (v_offset, u_from, u_to, kind mix) in the local frame
    cross: list = field(default_factory=list)  # (u, v_from, v_to)
    spacing: float = 26.0
    setback: float = 14.0
    gaps: list = field(default_factory=list)   # (u, v, radius) left unbuilt: piazza etc.
    core: tuple = (0, 0, 0, 0)                 # (u0, u1, v0, v1) built-up, non-vegetated
    irrigated: list = field(default_factory=list)  # (u0, u1, v0, v1) irrigated plots, non-vegetated


@dataclass
class Layout:
    terrain: str
    id: str
    seed: int
    settlements: list
    trunk: list                 # (name, class, [waypoints]) routed with least_cost_path
    scattered: dict             # name -> (along road name, z range, count, min spacing)
    landmarks: list             # (kind, settlement or None, u, v | x, y, w, h, name)
    water: list
    tracks: list                # (name, [waypoints]) forest tracks, not drivable
    households: dict            # name -> target households
    links: list = field(default_factory=list)  # (settlement, (u0, v0), (u1, v1), name) access lanes


T4_L1 = Layout(
    terrain="t4", id="t4_paese", seed=401,
    settlements=[
        # NE plain, at the foot of the wooded slope: compact, vulnerable at the SW margin.
        Settlement("Castelvento", (5000, 5150), 128.0,
                   streets=[(0, -260, 260, [("shop", .45), ("terrace", .55)]),
                            (-85, -200, 200, [("terrace", .6), ("house", .4)]),
                            (85, -230, 230, [("terrace", .5), ("apartments", .3), ("house", .2)]),
                            (170, -170, 170, [("house", .7), ("villa", .3)])],
                   cross=[(-150, -85, 170), (0, -85, 0), (140, -85, 170)],
                   gaps=[(0, 42, 55)], core=(-280, 280, -110, 200),
                   irrigated=[(-420, 420, 200, 720)]),
        # SW basin floor: a small valley hamlet on the road out.
        Settlement("Pian dei Grilli", (2450, 2600), 135.0,
                   streets=[(0, -220, 220, [("house", .6), ("shop", .2), ("terrace", .2)]),
                            (-80, -150, 150, [("house", .8), ("villa", .2)])],
                   cross=[(-60, -80, 0), (90, -80, 0)],
                   spacing=30.0, gaps=[(20, 30, 35)], core=(-240, 240, -100, 25),
                   irrigated=[(-450, 300, -560, -100)]),
    ],
    trunk=[
        ("SP 12 della Valle", "secondary", [(1500, 7990), "Castelvento:-260", "Castelvento:260", (7990, 3300)]),
        ("Strada del Passo", "tertiary", ["Castelvento:0", (3900, 4100), (3350, 2950), "Pian dei Grilli:220"]),
        ("SP 9 di Fondovalle", "secondary", ["Pian dei Grilli:-220", (10, 5000)]),
        ("Strada del Monte", "unclassified", ["Pian dei Grilli:0", (2000, 10)]),
    ],
    # Houses strung along the pass road on the sunny SW slope, in the macchia.
    scattered={"Le Ghiande": ("Strada del Passo", (170, 400), 24, 70.0)},
    landmarks=[
        ("plaza", "Castelvento", 0, 42, 70, 44, "Piazza"), ("church", "Castelvento", 60, 42, 30, 18, "Chiesa"),
        ("townhall", "Castelvento", -60, 42, 26, 18, "Municipio"),
        ("school", "Castelvento", -230, 130, 40, 22, "Scuola"),
        ("fire_station", "Castelvento", 230, -50, 30, 20, "Vigili del fuoco"),
        ("assembly", "Castelvento", 0, 420, 160, 110, "Area di attesa"),
        ("pitch", "Castelvento", -150, 430, 70, 44, "Campo sportivo"),
        ("parking", "Castelvento", 150, 420, 60, 40, "Parcheggio"),
        ("cemetery", "Castelvento", 360, 120, 50, 36, "Cimitero"),
        ("fuel", "Castelvento", -330, 20, 22, 14, "Distributore"),
        ("church", "Pian dei Grilli", 20, 30, 24, 14, "Chiesa"), ("mill", "Pian dei Grilli", -120, -160, 22, 16, "Mulino"),
        ("assembly", "Pian dei Grilli", -150, -330, 140, 100, "Area di attesa"),
        ("pitch", "Pian dei Grilli", 40, -330, 60, 36, "Campo sportivo"),
        ("chapel", None, 3300, 3330, 14, 10, "Cappella delle Ghiande"),
    ],
    water=[("hydrant", "Castelvento", -120, 0), ("hydrant", "Castelvento", 140, 85), ("hydrant", "Pian dei Grilli", 0, 0),
           ("open_water", None, 3880, 4140), ("open_water", "Pian dei Grilli", -260, -260)],
    tracks=[("Strada forestale del Crinale", [(3900, 4100), (2769, 5231)]),
            ("Strada forestale del Monte", [(3900, 4100), (4300, 3800)])],
    households={"Castelvento": 160, "Pian dei Grilli": 55, "Le Ghiande": 30},
    # The assembly areas must be on the road graph: refuges are road nodes.
    links=[("Castelvento", (0, 170), (0, 420), "Via del Campo"),
           ("Pian dei Grilli", (-150, -80), (-150, -330), "Via del Campo")],
)

# Hamlets of Le Ghiande: centres at least this far apart, houses within this of
# their centre (one engine's post covers ~120 m).
HAMLET_SPACING_M = 600.0
HAMLET_RADIUS_M = 110.0

# Layout 2 (2026-10-08, after phase 4's first measurements): Castelvento with
# the wood up to its north edge (the irrigated belt shrinks to the block round
# the assembly area, which must stay a refuge), and Le Ghiande as four hamlets
# a unit can defend, instead of 24 farms no plan can hold.
T4_L2 = dataclasses.replace(
    T4_L1,
    id="t4_paese2",
    settlements=[
        dataclasses.replace(T4_L1.settlements[0], irrigated=[(-320, 320, 230, 640)]),
        T4_L1.settlements[1],
    ],
    scattered={"Le Ghiande": ("Strada del Passo", (170, 400), 24, 25.0, 4)},
)

LAYOUTS = {("t4", 1): T4_L1, ("t4", 2): T4_L2}


# --------------------------------------------------------------------------- geometry

def frame(s: Settlement):
    """Local (u along the main street, v to its left) -> world."""
    a = math.radians(s.axis_deg)
    ux, uy = math.sin(a), math.cos(a)
    vx, vy = -uy, ux

    def w(u, v):
        return (s.centre[0] + u * ux + v * vx, s.centre[1] + u * uy + v * vy)
    return w


def anchor(layout: Layout, ref):
    if isinstance(ref, tuple):
        return ref
    name, u = ref.split(":")
    s = next(s for s in layout.settlements if s.name == name)
    return frame(s)(float(u), 0.0)


def rect(w, u0, u1, v0, v1):
    return [w(u0, v0), w(u1, v0), w(u1, v1), w(u0, v1)]


def paint_poly(fuel, poly, value):
    """Set every fire cell whose centre is inside `poly` (convex) to `value`."""
    xs, ys = [p[0] for p in poly], [p[1] for p in poly]
    r0, c0 = to_cell(min(xs), max(ys))
    r1, c1 = to_cell(max(xs), min(ys))
    for r in range(max(0, r0 - 1), min(N, r1 + 2)):
        for c in range(max(0, c0 - 1), min(N, c1 + 2)):
            x, y = (c + 0.5) * CELL_M, WORLD_M - (r + 0.5) * CELL_M
            inside = all((b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]) >= 0
                         for a, b in zip(poly, poly[1:] + poly[:1]))
            if inside:
                fuel[r, c] = value


def paint_line(fuel, line, value, half_width_m=0.0):
    for x, y in R.resample(line, CELL_M / 2):
        r, c = to_cell(x, y)
        k = int(half_width_m // CELL_M)
        fuel[max(0, r - k):r + k + 1, max(0, c - k):c + k + 1] = value


def road(rid, name, line, cls, drivable=True, track=False):
    return {"id": rid, "class": cls, "drivable": drivable, "track": track, "name": name, "oneway": False,
            "line": [[round(float(x), 2), round(float(y), 2)] for x, y in line]}


# --------------------------------------------------------------------------- population

PREP_MIN = (6, 18)


def vehicles(hid: int, scattered: bool) -> int:
    """Cars per household. In the countryside everyone drives: an isolated
    house without a car is not a plausible household, and walking out through
    the woods is not a plausible evacuation. In the villages about one
    household in twelve has none (the elderly), and the assembly areas are a
    walk away. Deterministic in `hid`, so it draws nothing from `rng`."""
    if scattered:
        return (1, 1, 2, 2)[hid % 4]
    return 0 if hid % 12 == 0 else (1, 1, 2)[hid % 3]


def traits(hid: int, rng: np.random.Generator) -> dict:
    """The complacent population of `scripts/generate_demo_scenarios.py::demo_traits`
    (wait-and-see, long preparation), kept identical so the evacuation numbers
    stay comparable with the old Rocca Ventosa; phase 3 revisits it together
    with the explicit pre-alert."""
    intent = str(rng.choice(["wait_and_see", "leave_early", "stay_defend"], p=[.92, .05, .03]))
    channel = str(rng.choice(["mobile_alert", "neighbour", "siren", "self_observed", "none"],
                             p=[.45, .15, .25, .10, .05]))
    return {
        "vehicles": vehicles(hid, False), "risk_perception": float(rng.uniform(.08, .30)),
        "prior_fire_experience": False, "warning_channel": channel,
        "trust_authority": float(rng.uniform(.55, .95)), "intent": intent,
        "prep_time_min": float(rng.uniform(*PREP_MIN)), "defensible_space": float(rng.uniform(.1, .6)),
        "has_pets_livestock": hid % 6 == 0,
    }


PAINTED_ROADS = {"secondary", "residential"}

FOOTPRINT = {"villa": (12, 10), "house": (13, 10), "terrace": (17, 10), "shop": (16, 11),
             "apartments": (18, 13), "farm": (16, 11)}
UNITS = {"villa": 1, "house": 1, "terrace": 2, "shop": 1, "apartments": 3, "farm": 1}


# --------------------------------------------------------------------------- build

def build(layout: Layout, dem: np.ndarray, fuel_nature: np.ndarray) -> dict:
    rng = np.random.default_rng(layout.seed)
    fuel = fuel_nature.copy()
    slope, _ = slope_aspect(dem)
    roads, notes, areas = [], {}, []

    def add(name, line, cls, **k):
        roads.append(road(len(roads) + 1, name, line, cls, **k))
        return roads[-1]

    # 1. Trunk roads, routed on the terrain between their waypoints.
    built = {}
    for name, cls, wps in layout.trunk:
        pts = [anchor(layout, w) for w in wps]
        line = [pts[0]]
        for a, b in zip(pts[:-1], pts[1:]):
            if any(isinstance(w, str) for w in wps) and _same_settlement(layout, a, b):
                line.append(b)  # through the village: the main street itself, straight
            else:
                line += R.least_cost_path(dem, a, b)[1:]
        line = R.resample(line, 40.0)
        built[name] = add(name, line, cls)
        g95, L = R.grade_stats(dem, line)
        notes[name] = {"length_m": round(L), "grade_p95": round(g95, 3)}

    # 2. Village streets, in each settlement's frame. Streets meet the trunk at
    #    shared vertices (connect_junctions) because they are laid across it.
    homes = []
    for s in layout.settlements:
        w = frame(s)
        in_trunk = any(f"{s.name}:" in str(a) and f"{s.name}:" in str(b)
                       for _, _, wps in layout.trunk for a, b in zip(wps[:-1], wps[1:]))
        for k, (v, u0, u1, mix) in enumerate(s.streets):
            if v != 0 or not in_trunk:  # the main street, unless a trunk road already runs it
                add(f"{s.name} - via {k}", R.resample([w(u0, v), w(u1, v)], 40.0), "residential")
            u = u0 + s.spacing / 2
            i = 0
            while u <= u1 - s.spacing / 2:
                for side in (-1, 1):
                    hu, hv = u, v + side * s.setback
                    if any(math.hypot(hu - gu, hv - gv) < gr for gu, gv, gr in s.gaps):
                        continue
                    h = (i * 2654435761 + k * 40503) % 1000 / 1000.0
                    acc, kind = 0.0, mix[-1][0]
                    for kd, wt in mix:
                        acc += wt
                        if h < acc:
                            kind = kd
                            break
                    homes.append((s.name, *w(hu, hv), kind, s.axis_deg))
                    i += 1
                u += s.spacing
        for sname, (u0, v0), (u1, v1), lname in layout.links:
            if sname == s.name:
                add(f"{s.name} - {lname}", R.resample([w(u0, v0), w(u1, v1)], 40.0), "residential")
        for (cu, v0, v1) in s.cross:
            add(f"{s.name} - vicolo {cu:+.0f}", R.resample([w(cu, v0), w(cu, v1)], 40.0), "residential")
        paint_poly(fuel, rect(w, *s.core), 0)
        areas.append({"kind": "core", "locality": s.name, "ring": rect(w, *s.core)})
        for box in s.irrigated:
            paint_poly(fuel, rect(w, *box), 0)
            areas.append({"kind": "irrigated", "locality": s.name, "ring": rect(w, *box)})

    # 3. Scattered houses along a road, on gentle enough ground, each with a lane.
    for name, spec in layout.scattered.items():
        road_name, (z0, z1), count, min_d = spec[:4]
        # optional: group the houses in this many hamlets (farm clusters)
        clusters = spec[4] if len(spec) > 4 else 0
        line = [tuple(p) for p in built[road_name]["line"]]
        cands = []
        for (ax, ay), (bx, by) in zip(line[:-1], line[1:]):
            L = math.hypot(bx - ax, by - ay)
            nx, ny = -(by - ay) / L, (bx - ax) / L
            for off in (-45.0, -30.0, 30.0, 45.0):
                x, y = (ax + bx) / 2 + nx * off, (ay + by) / 2 + ny * off
                r, c = to_cell(x, y)
                if z0 <= dem[r, c] <= z1 and slope[r, c] < 20:
                    cands.append((x, y, (ax + bx) / 2, (ay + by) / 2, math.degrees(math.atan2(bx - ax, by - ay))))
        rng.shuffle(cands)
        chosen = []
        if clusters:
            # hamlet centres far apart along the road, then houses close to them
            centres = []
            for cnd in cands:
                if all(math.hypot(cnd[0] - o[0], cnd[1] - o[1]) >= HAMLET_SPACING_M for o in centres):
                    centres.append(cnd)
                if len(centres) == clusters:
                    break
            per = -(-count // max(len(centres), 1))
            for cx, cy, jx, jy, bearing in centres:
                group = [(cx, cy, jx, jy, bearing, True)]
                for k in range(1, per):
                    for _ in range(40):
                        a = rng.uniform(0, 2 * math.pi)
                        d = rng.uniform(min_d, HAMLET_RADIUS_M)
                        x, y = cx + d * math.cos(a), cy + d * math.sin(a)
                        r, c = to_cell(x, y)
                        if z0 - 40 <= dem[r, c] <= z1 + 40 and slope[r, c] < 25 and \
                                all(math.hypot(x - o[0], y - o[1]) >= min_d for o in group):
                            # the lane joins the hamlet's first house, not the road
                            group.append((x, y, cx, cy, bearing, False))
                            break
                chosen += group[:per]
            chosen = chosen[:count]
        else:
            for cnd in cands:
                if all(math.hypot(cnd[0] - o[0], cnd[1] - o[1]) >= min_d for o in chosen):
                    chosen.append((*cnd, True))
                if len(chosen) == count:
                    break
        for x, y, jx, jy, bearing, from_road in chosen:
            # a lane from the road, or (within a hamlet) from its first house
            j = R.nearest_vertex(line, (jx, jy)) if from_road else (jx, jy)
            add(f"Podere {name} {len(homes)}", [j, (x, y)], "unclassified")
            homes.append((name, x, y, "villa" if rng.random() < .3 else ("farm" if rng.random() < .25 else "house"),
                          bearing))
        notes[name] = {"houses_requested": count, "houses_placed": len(chosen)}

    # 4. Forest tracks: crews on foot and 4x4, not cars.
    for name, wps in layout.tracks:
        line = [wps[0]]
        for a, b in zip(wps[:-1], wps[1:]):
            line += R.least_cost_path(dem, a, b, grade_ok=0.10, grade_max=0.20)[1:]
        add(name, R.resample(line, 40.0), "track", drivable=False, track=True)

    R.crossings(roads)
    R.connect_junctions(roads)
    # Only roads wide enough to be a fuel break at 20 m are painted out: the
    # provincial road with its verges, and village streets (already inside the
    # built core). A 5 m lane under the canopy painted as a 20 m bare strip
    # would be an artificial firebreak -- measured: the pass road's hairpins
    # alone cut the 2 h burnt area by about a third.
    for r_ in roads:
        if r_["drivable"] and r_["class"] in PAINTED_ROADS:
            paint_line(fuel, r_["line"], 0)

    # 5. Landmarks and water.
    buildings = []
    lm_id = 100000
    for kind, sname, a, b, wd, ht, label in layout.landmarks:
        if sname:
            s = next(s for s in layout.settlements if s.name == sname)
            w = frame(s)
            cx, cy = w(a, b)
            ring = rect(w, a - wd / 2, a + wd / 2, b - ht / 2, b + ht / 2)
        else:
            cx, cy = a, b
            ring = [(a - wd / 2, b - ht / 2), (a + wd / 2, b - ht / 2), (a + wd / 2, b + ht / 2), (a - wd / 2, b + ht / 2)]
        buildings.append({"id": lm_id, "kind": kind, "name": label, "centroid": [cx, cy],
                          "ring": [[float(x), float(y)] for x, y in ring], "locality": sname})
        lm_id += 1
        paint_poly(fuel, [tuple(p) for p in ring], 0)
        fuel[to_cell(cx, cy)] = 0  # smaller than a cell: no cell centre falls inside
    water = []
    for k, (kind, sname, a, b) in enumerate(layout.water):
        x, y = frame(next(s for s in layout.settlements if s.name == sname))(a, b) if sname else (a, b)
        water.append({"id": k + 1, "kind": kind, "pos": [float(x), float(y)]})
        r, c = to_cell(x, y)
        fuel[r, c] = 0

    # 6. Homes and households. Each locality gets its target households spread
    #    over its buildings by capacity; every home and household cell is
    #    non-vegetated, or the exposure layer would read a house as a bush.
    dwellings, households, people = [], [], []
    by_loc: dict = {}
    for loc, x, y, kind, bearing in homes:
        by_loc.setdefault(loc, []).append((x, y, kind, bearing))
    for loc, target in layout.households.items():
        slots = by_loc.get(loc, [])
        alloc = allocate(target, [UNITS[k] for _, _, k, _ in slots])
        for (x, y, kind, bearing), n_hh in zip(slots, alloc):
            if n_hh == 0:
                continue
            bid = len(buildings) + 1
            bw, bd = FOOTPRINT[kind]
            a = math.radians(90 - bearing)
            ca, sa = math.cos(a), math.sin(a)
            ring = [[x + u * ca - v * sa, y + u * sa + v * ca]
                    for u, v in [(-bw / 2, -bd / 2), (bw / 2, -bd / 2), (bw / 2, bd / 2), (-bw / 2, bd / 2)]]
            buildings.append({"id": bid, "kind": kind, "name": f"Casa {bid}", "centroid": [x, y],
                              "ring": ring, "locality": loc})
            r, c = to_cell(x, y)
            fuel[r, c] = 0
            dwellings.append({"osm_id": bid, "kind": "residential", "pos": [x, y], "area_m2": float(bw * bd),
                              "levels": {"apartments": 3, "terrace": 2, "shop": 2}.get(kind, 2), "units": n_hh,
                              "cell": [r, c], "dist_to_fuel_m": CELL_M, "fuel_at_site": 0})
            for k in range(n_hh):
                hid = len(households)
                # a few metres apart, inside the same non-vegetated cell
                hx, hy = x + (k - (n_hh - 1) / 2) * 3.0, y
                hr, hc = to_cell(hx, hy)
                fuel[hr, hc] = 0
                size = int(rng.choice([1, 2, 2, 3, 3, 4, 5]))
                members = list(range(len(people), len(people) + size))
                households.append({"id": hid, "building": bid, "pos": [hx, hy], "cell": [hr, hc], "size": size,
                                   "dist_to_fuel_m": CELL_M, **traits(hid, rng),
                                   "vehicles": vehicles(hid, loc in layout.scattered), "status": "normal",
                                   "members": members, "locality": loc})
                for off, pid in enumerate(members):
                    age = (8, 17, 34, 48, 72)[(hid + off) % 5]
                    needs = age >= 72 or pid % 19 == 0
                    people.append({"id": pid, "household": hid, "age": age,
                                   "walk_speed": 0.85 if needs else 1.35 + (pid % 4) * .08,
                                   "needs_assistance": needs, "at_home": pid % 7 != 0})
    notes["households"] = {loc: sum(1 for h in households if h["locality"] == loc) for loc in layout.households}
    return {"fuel": fuel, "roads": roads, "buildings": buildings, "water": water, "areas": areas, "dwellings": dwellings,
            "households": households, "people": people, "notes": notes}


def allocate(target: int, caps: list[int]) -> list[int]:
    """Households per building: one each first (evenly spread if there are more
    buildings than households), then up to each building's units, then evenly."""
    n = len(caps)
    alloc = [0] * n
    if n == 0:
        return alloc
    order = list(range(n)) if target >= n else [round(k * (n - 1) / max(1, target - 1)) for k in range(target)]
    for i in order[:target]:
        alloc[i] += 1
    left = target - sum(alloc)
    while left > 0:
        room = [i for i in range(n) if alloc[i] < caps[i]] or list(range(n))
        for i in room:
            if left == 0:
                break
            alloc[i] += 1
            left -= 1
    return alloc


def _same_settlement(layout, a, b):
    for s in layout.settlements:
        if all(math.hypot(p[0] - s.centre[0], p[1] - s.centre[1]) < 400 for p in (a, b)):
            return True
    return False


def write(layout: Layout, dem: np.ndarray, out: dict, params: dict, fine: dict | None = None) -> None:
    """`dem` is the 20 m fire DEM. With `fine` (factory.fine.build), the render
    terrain is the 5 m surface and a 5 m land cover is written beside it;
    without it the render terrain falls back to the fire DEM."""
    d = DATA / "scenarios" / layout.id
    d.mkdir(parents=True, exist_ok=True)
    dem.astype("<f8").tofile(d / "dem.f64")
    out["fuel"].astype("<i4").tofile(d / "fuel.i32")
    if fine is None:
        terr, posting = dem, CELL_M
    else:
        from .fine import COVER_NAMES, FINE_M, NC
        terr, posting = fine["fine"], FINE_M
        fine["cover"].astype("u1").tofile(d / "cover.u8")
        (d / "cover.json").write_text(json.dumps({
            "rows": NC, "cols": NC, "cell_m": FINE_M, "world_size_m": [WORLD_M, WORLD_M],
            "classes": dict(enumerate(COVER_NAMES))}, indent=2) + "\n")
    terr.astype("<f4").tofile(d / "render_terrain.f32")
    (d / "render_terrain.json").write_text(json.dumps({
        "rows": terr.shape[0], "cols": terr.shape[1], "posting_m": posting, "world_size_m": [WORLD_M, WORLD_M],
        "elev_min": float(terr.min()), "elev_max": float(terr.max())}, indent=2) + "\n")
    (d / "osm.json").write_text(json.dumps({
        "world_size_m": [WORLD_M, WORLD_M], "fire_grid": {"rows": N, "cols": N, "cellsize": CELL_M},
        "buildings": out["buildings"], "roads": out["roads"], "water": out["water"]}, indent=1) + "\n")
    (d / "population.json").write_text(json.dumps({
        "synthetic": True, "seed": layout.seed, "dwellings": out["dwellings"],
        "households": out["households"], "people": out["people"]}, indent=1) + "\n")
    (d / "scenario.json").write_text(json.dumps({
        "id": layout.id, "name": "Rocca Ventosa",
        "description": "Un borgo in piano, un nucleo di fondovalle e case sparse sul versante, "
                       "separati da un crinale con un valico. Territorio sintetico.",
        "location": "Luogo immaginario", "country": "Italia", "nationality": "Italian",
        "region": "territorio sintetico (Scenario Factory)",
        "localities": [s.name for s in layout.settlements] + list(layout.scattered),
        "coordinates": [0.0, 0.0], "utm_zone": 0,
        "world_size_m": [WORLD_M, WORLD_M], "fire_grid_size": [N, N],
        "buildings_count": len(out["buildings"]), "households_count": len(out["households"]),
        "people_count": len(out["people"]), "scenario_type": "synthetic", "creation_date": "2026-10-08",
        "version": "0.1.0", "tags": ["factory", "town"], "is_dev": True}, indent=2) + "\n")
    (d / "params.json").write_text(json.dumps(params, indent=2) + "\n")


def load_built(cid: str) -> dict:
    d = DATA / "scenarios" / cid
    return {"osm": json.loads((d / "osm.json").read_text()), "pop": json.loads((d / "population.json").read_text())}


__all__ = ["LAYOUTS", "build", "write", "load_built", "ROOT"]
