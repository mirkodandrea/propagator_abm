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
class Lane:
    """A free-form lane: a smoothed polyline from a point on a parent road, with
    houses strewn along it at irregular gaps and setbacks, turned to follow its
    bends. The way an Italian hill village grows, not a street grid."""
    parent: object              # "main", an index into the settlement's lanes, or (settlement, index)
    t: float                    # u on the main street, or fraction along the parent lane
    pts: list                   # control points (u, v) after the start
    mix: list = field(default_factory=lambda: [("house", .7), ("villa", .3)])
    step: tuple = (15, 30)      # gap between houses along one side, m
    setback: float = 12.0
    road: bool = True           # False: houses only, along a road that already exists
    houses_until: float = 1e9   # no houses beyond this arclength from the start, m
    sides: tuple = (-1, 1)
    p: float = 0.92             # chance a slot is built
    loc: str | None = None      # locality, when it differs from the settlement's
    cls: str = "unclassified"   # road class; only "residential"/"secondary" are painted out as a fuel break


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
    wild: list = field(default_factory=list)  # (u0, u1, v0, v1, fuel) untended land, painted under the rest
    lanes: list = field(default_factory=list)  # Lane, in order: a lane's parent comes before it


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

# Layout 3 (2026-10-09, user decision): Castelvento was never reached, because
# 300-400 m of grass separate its SW edge from the wood and a grass front's
# embers do not carry to the houses. Abandoned terraces gone to macchia, then
# the oak wood, now come up to the gardens on the SW side, the one the
# prevailing wind blows across from the slope.
T4_L3 = dataclasses.replace(
    T4_L2,
    id="t4_paese3",
    settlements=[
        dataclasses.replace(T4_L2.settlements[0], wild=[(-320, 320, -260, -130, 8), (-320, 320, -450, -260, 5)]),
        T4_L2.settlements[1],
    ],
)

def frame(s: Settlement):
    """Local (u along the main street, v to its left) -> world."""
    a = math.radians(s.axis_deg)
    ux, uy = math.sin(a), math.cos(a)
    vx, vy = -uy, ux

    def w(u, v):
        return (s.centre[0] + u * ux + v * vx, s.centre[1] + u * uy + v * vy)
    return w


def _shift(base: Settlement, du: float, dv: float):
    """World position of the point (du, dv) in `base`'s frame."""
    return frame(base)(du, dv)


_C = T4_L3.settlements[0]
_P = T4_L3.settlements[1]

# Layout 4 (2026-10-09, user request): bigger settlements with several
# districts, laid out as Italian hill villages: winding lanes off the provincial
# road, houses at irregular gaps turned to follow the bends. Castelvento keeps
# its piazza and civic buildings; Fornaci (west) and San Rocco (east) are on the
# same provincial road; Le Terrazze are villas on the macchia slope south-west
# of the core, the exposed side. Each is its own locality (a district the
# player prioritises).
def _centre_from(axis_deg, u, target):
    """Centre of a settlement whose main street, at `u`, passes through `target`."""
    a = math.radians(axis_deg)
    return (target[0] - u * math.sin(a), target[1] - u * math.cos(a))


_OLD = [("terrace", .5), ("house", .4), ("shop", .1)]
_MID = [("house", .6), ("terrace", .25), ("villa", .15)]
_NEW = [("house", .45), ("villa", .4), ("terrace", .15)]
_V = [("villa", .6), ("house", .4)]

T4_L4 = dataclasses.replace(
    T4_L3,
    id="t4_paese4", seed=404,
    settlements=[
        dataclasses.replace(
            _C, streets=[], cross=[],
            gaps=[(0, 42, 55)], core=(-280, 280, -135, 225),
            lanes=[
                Lane("main", -260, [(260, 0)], [("shop", .4), ("terrace", .6)], step=(13, 21), setback=11, road=False),
                Lane("main", -170, [(-190, 50), (-150, 110), (-185, 170)], [("terrace", .5), ("house", .5)], step=(13, 24), setback=11, houses_until=260),
                Lane("main", -110, [(-115, 60), (-130, 120), (-95, 190), (-60, 270), (-30, 350), (0, 420)], _MID, step=(13, 24), setback=11, houses_until=250),
                Lane("main", 70, [(95, 55), (140, 100), (110, 155), (150, 205)], [("terrace", .4), ("house", .4), ("apartments", .2)], step=(13, 24), setback=11),
                Lane("main", 190, [(215, 50), (255, 95), (240, 150)], [("house", .7), ("villa", .3)], step=(14, 26)),
                Lane("main", -210, [(-215, -50), (-180, -85), (-205, -108)], _MID, step=(14, 26)),
                Lane("main", -60, [(-45, -45), (-70, -80), (-45, -108)], [("terrace", .4), ("house", .6)], step=(13, 24)),
                Lane("main", 60, [(70, -50), (105, -80), (85, -108)], [("house", .6), ("villa", .4)], step=(14, 26)),
                Lane("main", 200, [(190, -55), (225, -85)], [("villa", .5), ("house", .5)], step=(15, 28)),
                Lane(6, 1.0, [(-40, -190), (-20, -265), (0, -340)], houses_until=0, cls="unclassified"),
            ]),
        dataclasses.replace(
            _P, cross=[], gaps=[(20, 30, 35)], core=(-240, 240, -100, 190),
            lanes=[
                Lane("main", -220, [(220, 0)], [("house", .6), ("shop", .2), ("terrace", .2)], step=(13, 22), setback=11, road=False),
                Lane("main", -130, [(-135, 55), (-100, 110), (-130, 165)], _MID, step=(14, 26), houses_until=220),
                Lane("main", 60, [(75, 60), (110, 115), (80, 170)], _MID, step=(14, 26)),
                Lane("main", 160, [(165, 50), (195, 100)], _MID, step=(15, 28)),
                Lane("main", -150, [(-155, -80), (-150, -170), (-150, -250), (-150, -330)], _MID, step=(14, 26), houses_until=130),
                Lane("main", 20, [(15, -55), (50, -105), (20, -155)], _MID, step=(14, 26)),
                Lane("main", 110, [(120, -55), (150, -100)], _MID, step=(15, 28)),
            ]),
        Settlement("Fornaci", _centre_from(118.0, 200, _shift(_C, -275, 10)), 118.0, streets=[], gaps=[(-90, 25, 28)],
                   core=(-90, 90, -35, 60), irrigated=[], spacing=28.0,
                   lanes=[
                       Lane("main", -200, [(200, 0)], _OLD, step=(13, 22), setback=11, road=False),
                       Lane("main", -170, [(-175, 55), (-135, 105), (-165, 160)], _OLD, step=(13, 24), houses_until=200),
                       Lane("main", -100, [(-105, 60), (-100, 140), (-125, 210), (-120, 330)], _MID, step=(13, 24), houses_until=200),
                       Lane("main", -30, [(-15, 60), (30, 115), (0, 170)], _OLD, step=(13, 24)),
                       Lane("main", 90, [(100, 55), (140, 100), (115, 155)], _MID, step=(14, 26)),
                       Lane("main", -120, [(-125, -45), (-95, -80), (-120, -108)], _MID, step=(14, 26)),
                       Lane("main", 60, [(65, -50), (105, -80), (85, -108)], _MID, step=(14, 26)),
                   ]),
        Settlement("San Rocco", _centre_from(138.0, -200, _shift(_C, 275, -10)), 138.0, streets=[], gaps=[],
                   core=(-90, 90, -35, 60), irrigated=[], spacing=28.0,
                   lanes=[
                       Lane("main", -200, [(200, 0)], _NEW, step=(18, 32), setback=13, road=False),
                       Lane("main", -150, [(-140, 60), (-110, 120), (-135, 185)], _NEW, step=(18, 34)),
                       Lane("main", -10, [(0, 70), (40, 125), (15, 190)], _NEW, step=(18, 34)),
                       Lane("main", 60, [(75, 70), (95, 150), (110, 230), (130, 330)], _NEW, step=(18, 34), houses_until=170),
                       Lane("main", 140, [(145, 60), (185, 110), (165, 165)], _NEW, step=(18, 34)),
                       Lane("main", -100, [(-95, -50), (-65, -85), (-95, -112)], _NEW, step=(18, 34)),
                       Lane("main", 90, [(100, -50), (135, -85), (115, -112)], _NEW, step=(18, 34)),
                   ]),
        Settlement("Le Terrazze", _shift(_C, 0, -340), 128.0, streets=[], gaps=[], core=(-60, 60, -25, 25),
                   spacing=34.0,
                   lanes=[
                       Lane(("Castelvento", 9), 1.0, [(70, 12), (140, -8), (210, 10)], _V, step=(24, 42), setback=15, cls="unclassified"),
                       Lane(("Castelvento", 9), 1.0, [(-70, 12), (-140, -8), (-205, 10)], _V, step=(24, 42), setback=15, cls="unclassified"),
                       Lane(("Castelvento", 9), 1.0, [(10, -70), (-30, -125)], _V, step=(24, 42), setback=15, cls="unclassified"),
                   ]),
    ],
    trunk=[
        ("SP 12 della Valle", "secondary",
         [(1500, 7990), "Fornaci:-200", "Fornaci:200", "Castelvento:-260", "Castelvento:260",
          "San Rocco:-200", "San Rocco:200", (7990, 3300)]),
        *T4_L3.trunk[1:],
    ],
    scattered={"Le Ghiande": ("Strada del Passo", (170, 400), 32, 25.0, 5)},
    landmarks=T4_L3.landmarks + [
        ("church", "Fornaci", -90, 25, 28, 16, "Chiesa di San Nicola"),
        ("assembly", "Fornaci", -120, 330, 140, 100, "Area di attesa"),
        ("pitch", "Fornaci", 30, 330, 60, 36, "Campo sportivo"),
        ("church", "San Rocco", 40, 40, 28, 16, "Chiesa di San Rocco"),
        ("assembly", "San Rocco", 130, 330, 140, 100, "Area di attesa"),
        ("parking", "San Rocco", -30, 330, 60, 40, "Parcheggio"),
        ("shop", "Le Terrazze", 100, 45, 20, 14, "Agriturismo"),
    ],
    water=T4_L3.water + [("hydrant", "Fornaci", -120, 0), ("hydrant", "San Rocco", 120, 0),
                         ("hydrant", "Le Terrazze", 0, 0)],
    households={"Castelvento": 190, "Pian dei Grilli": 90, "Fornaci": 90, "San Rocco": 100,
                "Le Terrazze": 30, "Le Ghiande": 38},
    links=[],
)

LAYOUTS = {("t4", 1): T4_L1, ("t4", 2): T4_L2, ("t4", 3): T4_L3, ("t4", 4): T4_L4}


# --------------------------------------------------------------------------- geometry

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
            if math.hypot(b[0] - a[0], b[1] - a[1]) < 1.0:
                continue  # the same point named from two frames
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
    lane_lines, road_pts = {}, []
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
        if s.lanes:
            main_line = next((built[n]["line"] for n, _, wps in layout.trunk
                              if any(f"{s.name}:" in str(a) and f"{s.name}:" in str(b) for a, b in zip(wps[:-1], wps[1:]))),
                             next((r_["line"] for r_ in roads if r_["name"] == f"{s.name} - via 0"), None))
            _lanes(s, w, layout, roads, add, main_line, lane_lines, road_pts, homes)
        for (cu, v0, v1) in s.cross:
            add(f"{s.name} - vicolo {cu:+.0f}", R.resample([w(cu, v0), w(cu, v1)], 40.0), "residential")
        for *box, f in s.wild:
            paint_poly(fuel, rect(w, *box), f)  # vegetation, so not an area: the render reads it from the fuel
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


def _lanes(s, w, layout, roads, add, main_line, lane_lines, road_pts, homes):
    """Lay out `s.lanes`: roads and the houses along them."""
    rng = np.random.default_rng(layout.seed + 7 + len(lane_lines))
    a = math.radians(s.axis_deg)
    ux, uy = math.sin(a), math.cos(a)
    civic = []
    for _, sn, lu, lv, wd, ht, _ in layout.landmarks:
        if sn:
            cs = next(c for c in layout.settlements if c.name == sn)
            civic.append((*frame(cs)(lu, lv), max(wd, ht) / 2 + 12))
        else:
            civic.append((lu, lv, max(wd, ht) / 2 + 12))

    def local(x, y):
        dx, dy = x - s.centre[0], y - s.centre[1]
        return dx * ux + dy * uy, -dx * uy + dy * ux

    if main_line is not None:
        road_pts.append(np.array(R.resample([tuple(q) for q in main_line], 2.0)))
    for i, ln in enumerate(s.lanes):
        if ln.parent == "main":
            if ln.road:
                start = R.nearest_vertex(main_line, w(ln.t, 0.0))
            else:
                start = w(ln.t, 0.0)
        else:
            key = ln.parent if isinstance(ln.parent, tuple) else (s.name, ln.parent)
            pl = lane_lines[key]
            start = pl[int(round(ln.t * (len(pl) - 1)))]
        ctrl = [tuple(start)] + [w(*q) for q in ln.pts]
        if ln.road:
            line = R.resample(R.chaikin(ctrl, 2), 25.0)
            add(f"{s.name} - via {len(roads)}", line, ln.cls)
            roads[-1]["line"] = [[round(float(x), 2), round(float(y), 2)] for x, y in line]
            line = [tuple(p_) for p_ in roads[-1]["line"]]
        else:
            line = ctrl
        lane_lines[(s.name, i)] = line
        dense = np.array(R.resample(line, 2.0))
        seg = np.hypot(*np.diff(dense, axis=0).T)
        cum = np.concatenate([[0.0], np.cumsum(seg)])
        mine = len(road_pts)
        if ln.road:
            road_pts.append(dense)
        for side in ln.sides:
            d = rng.uniform(0, ln.step[1])
            while d < min(cum[-1], ln.houses_until):
                k = min(int(np.searchsorted(cum, d)), len(dense) - 1)
                k0, k1 = max(k - 3, 0), min(k + 3, len(dense) - 1)
                tx, ty = dense[k1] - dense[k0]
                n_ = math.hypot(tx, ty) or 1.0
                tx, ty = tx / n_, ty / n_
                off = ln.setback + rng.uniform(-2.0, 5.0)
                x, y = dense[k][0] - ty * side * off, dense[k][1] + tx * side * off
                step = rng.uniform(*ln.step)
                lu, lv = local(x, y)
                free = not any(math.hypot(lu - gu, lv - gv) < gr for gu, gv, gr in s.gaps) \
                    and all(math.hypot(x - hx, y - hy) >= 15.0 for _, hx, hy, _, _ in homes) \
                    and all(math.hypot(x - cx, y - cy) >= cr for cx, cy, cr in civic) \
                    and all(np.min(np.hypot(pts[:, 0] - x, pts[:, 1] - y)) >= 9.0
                            for j, pts in enumerate(road_pts) if j != mine or not ln.road)
                if free and rng.random() < ln.p:
                    h = rng.random()
                    acc, kind = 0.0, ln.mix[-1][0]
                    for kd, wt in ln.mix:
                        acc += wt
                        if h < acc:
                            kind = kd
                            break
                    bearing = math.degrees(math.atan2(tx, ty)) + rng.uniform(-9.0, 9.0)
                    homes.append((ln.loc or s.name, x, y, kind, bearing))
                d += step


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
