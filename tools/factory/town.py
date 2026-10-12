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
    # organic layouts: the built-up fabric inside the core (aligned rows) and
    # how it is built
    mix: list = field(default_factory=lambda: [("house", .55), ("terrace", .25), ("villa", .1), ("shop", .1)])
    urban: tuple | None = None  # (u0, u1, v0, v1) of the aligned rows, when wider than the core
    urban_gap: float = 1.5      # m between houses of a row: ~0 = attached, larger = detached
    block_m: float = 58.0       # distance between the parallel streets


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
    # (settlement, (u0, u1, v0, v1), fuel): vegetation painted back over the
    # painted-out roads, where a narrow road runs between untended plots and a
    # 20 m bare cell would be an artificial firebreak
    verges: list = field(default_factory=list)
    # (road, settlement, u0, u1, fuel): the same along a road's own line, for
    # the stretch whose points fall between u0 and u1 in the settlement's frame
    # (a box misses a road that bends)
    wooded_roads: list = field(default_factory=list)
    # Hand-drawn look (2026-10-12, user: the villages looked square): cores and
    # irrigated plots get a rounded, irregular outline instead of a box, and
    # houses vary in size, sit looser on the lane and keep more apart. The
    # outlines draw nothing from the layout's rng.
    organic: bool = False


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

# Layout 5 (2026-10-11, user decision, docs/fase5/uscite): evacuating everyone
# cost nothing, because every hamlet was 300-500 m from the refuge in
# Castelvento's non-burnable core and its road emptied before the fire came.
# San Rocco moves ~1 km further down the SP 12, at the foot of the wooded
# slope with the grass plain on the other side: its one road to the refuge runs
# through the fuel and then down Castelvento's main street, the way
# Castelvento's own families take to the assembly area.
SAN_ROCCO_FROM_CASTELVENTO_M = 1000.0

T4_L5 = dataclasses.replace(
    T4_L4,
    id="t4_paese5", seed=405,
    settlements=[
        *T4_L4.settlements[:3],
        # The plain is meadow (eu_fuel12 2), and a meadow fire's flames are too
        # short to make a 20 m road impassable (fire::threat): abandoned olive
        # terraces gone to macchia line the road from Castelvento, the way
        # layout 3 brought them to Castelvento's SW edge.
        dataclasses.replace(T4_L4.settlements[3],
                            centre=_centre_from(128.0, -200, _shift(_C, 275 + SAN_ROCCO_FROM_CASTELVENTO_M, -10)),
                            axis_deg=128.0,
                            wild=[(-1100, -230, -250, 250, 8)]),
        T4_L4.settlements[4],
    ],
    # The SP 12 is 6 m of tarmac between those terraces, not a 20 m firebreak:
    # painted out, no fire could ever reach a car on it (measured: the road
    # nodes never reach fire::threat::IMPASSABLE, whatever burns beside them).
    verges=[("San Rocco", (-1100, -230, -30, 30), 8)],
)

# Layout 6 (2026-10-11, user decision): no new refuges (assembly areas out in
# the fuel are not how civil protection works), but the town itself sprawls into
# the vegetation, as Ligurian villages do. Castelvento's newer houses string out
# along the SP 12 towards San Rocco, villas up short lanes into the macchia, and
# San Rocco loses its bare core: gardens and macchia run between its houses. The
# SP 12 keeps its verges along its whole line between the two (layout 5's box
# missed the bend, where the road swings 60 m off the axis).
_RIBBON = [("villa", .5), ("house", .5)]
_SR = T4_L5.settlements[3]

T4_L6 = dataclasses.replace(
    T4_L5,
    id="t4_paese6", seed=406,
    settlements=[
        dataclasses.replace(T4_L5.settlements[0], lanes=[
            *T4_L5.settlements[0].lanes,
            # along the SP 12 itself, following its bend (C frame, measured off
            # the built road: v = -62 at u = 900)
            Lane("main", 400, [(600, -40), (800, -59), (950, -62), (1100, -50), (1200, -27)], _RIBBON,
                 step=(30, 55), setback=20, road=False),
            Lane("main", 470, [(480, 70), (455, 150), (480, 220)], _RIBBON, step=(24, 44), setback=15),
            Lane("main", 640, [(650, -130), (680, -210), (650, -280)], _RIBBON, step=(24, 44), setback=15),
            Lane("main", 840, [(850, 20), (880, 100), (860, 180)], _RIBBON, step=(24, 44), setback=15),
            Lane("main", 1040, [(1040, -140), (1070, -220)], _RIBBON, step=(24, 44), setback=15),
        ]),
        *T4_L5.settlements[1:3],
        dataclasses.replace(_SR, core=(-25, 25, -15, 15),
                            wild=[*_SR.wild, (-230, 260, -160, 260, 8)]),
        T4_L5.settlements[4],
    ],
    verges=[],
    wooded_roads=[("SP 12 della Valle", "Castelvento", 320, 1700, 8)],
    households={**T4_L5.households, "Castelvento": 230},
)

# Layout 7 (2026-10-12, user request): layout 5 with the hand-drawn look
# (`Layout.organic`): irregular cores, houses of different sizes turned a little
# more, and kept apart by 21 m (the kiosk draws them 1.9x, `visual_scale`).
T4_L7 = dataclasses.replace(
    T4_L5, id="t4_paese7", organic=True,
    settlements=[
        dataclasses.replace(T4_L5.settlements[0], mix=[("terrace", .4), ("house", .3), ("shop", .2), ("apartments", .1)],
                            block_m=54.0),
        dataclasses.replace(T4_L5.settlements[1], mix=[("house", .5), ("terrace", .2), ("shop", .1), ("villa", .2)]),
        dataclasses.replace(T4_L5.settlements[2], mix=_OLD, urban=(-200, 200, -70, 110)),
        dataclasses.replace(T4_L5.settlements[3], mix=_NEW, urban=(-190, 190, -70, 110), urban_gap=7.0, block_m=62.0),
        T4_L5.settlements[4],
    ],
)

LAYOUTS = {("t4", 7): T4_L7, ("t4", 1): T4_L1, ("t4", 2): T4_L2, ("t4", 3): T4_L3, ("t4", 4): T4_L4, ("t4", 5): T4_L5, ("t4", 6): T4_L6}


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


def _vnoise(x, y, seed):
    """Smooth 2-D value noise in [-1, 1], deterministic, no rng."""
    def h(i, j):
        n = (i * 374761393 + j * 668265263 + seed * 1442695041) & 0xFFFFFFFF
        n = ((n ^ (n >> 13)) * 1274126177) & 0xFFFFFFFF
        return ((n ^ (n >> 16)) & 0xFFFF) / 32767.5 - 1.0
    i, j = math.floor(x), math.floor(y)
    fx, fy = x - i, y - j
    fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    return (h(i, j) * (1 - fx) + h(i + 1, j) * fx) * (1 - fy) + (h(i, j + 1) * (1 - fx) + h(i + 1, j + 1) * fx) * fy


def organic_box(w, u0, u1, v0, v1, seed, corner=70.0, wobble=26.0, step=22.0):
    """Outline of a rounded box whose edge wanders by up to `wobble` m: the
    footprint of a village that grew, not a plot that was surveyed. Returns
    world points of a (generally non-convex) polygon."""
    cu, cv = (u0 + u1) / 2, (v0 + v1) / 2
    hu, hv = (u1 - u0) / 2, (v1 - v0) / 2
    corner = min(corner, hu, hv)
    pts = []
    n = max(24, int(2 * math.pi * max(hu, hv) / step))
    for k in range(n):
        t = 2 * math.pi * k / n
        # superellipse: rounded corners with the same half-axes as the box
        c, s_ = math.cos(t), math.sin(t)
        e = 2.0 + 6.0 * (corner / max(hu, hv))
        r = (abs(c / hu) ** e + abs(s_ / hv) ** e) ** (-1.0 / e)
        u, v = r * c, r * s_
        nrm = math.hypot(u / hu ** 2, v / hv ** 2) or 1.0
        nu, nv = u / hu ** 2 / nrm, v / hv ** 2 / nrm
        d = wobble * (0.7 * _vnoise((cu + u) / 90.0, (cv + v) / 90.0, seed)
                      + 0.3 * _vnoise((cu + u) / 35.0, (cv + v) / 35.0, seed + 1)) - wobble * 0.2
        pts.append(w(cu + u + nu * d, cv + v + nv * d))
    return pts


def paint_poly_any(fuel, poly, value):
    """`paint_poly` for a polygon that need not be convex."""
    from matplotlib.path import Path as MplPath
    xs, ys = [p[0] for p in poly], [p[1] for p in poly]
    r0, c0 = to_cell(min(xs), max(ys))
    r1, c1 = to_cell(max(xs), min(ys))
    r0, r1, c0, c1 = max(0, r0 - 1), min(N, r1 + 2), max(0, c0 - 1), min(N, c1 + 2)
    rr, cc = np.mgrid[r0:r1, c0:c1]
    P = np.c_[((cc + 0.5) * CELL_M).ravel(), (WORLD_M - (rr + 0.5) * CELL_M).ravel()]
    m = MplPath(np.asarray(poly)).contains_points(P).reshape(rr.shape)
    fuel[r0:r1, c0:c1][m] = value


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
    for si, s in enumerate(layout.settlements):
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
        ring_core = None
        ring_urban = None
        if layout.organic and s.core[1] > s.core[0]:
            ring_core = organic_box(w, *s.core, seed=layout.seed + si)
            ring_urban = organic_box(w, *(s.urban or s.core), seed=layout.seed + si + 20, wobble=34.0) \
                if s.urban else ring_core
        if s.lanes:
            main_line = next((built[n]["line"] for n, _, wps in layout.trunk
                              if any(f"{s.name}:" in str(a) and f"{s.name}:" in str(b) for a, b in zip(wps[:-1], wps[1:]))),
                             next((r_["line"] for r_ in roads if r_["name"] == f"{s.name} - via 0"), None))
            _lanes(s, w, layout, roads, add, main_line, lane_lines, road_pts, homes, ring_urban)
            if ring_urban is not None:
                _urban(s, w, layout, ring_urban, add, main_line, road_pts, homes, _civic(layout), layout.seed + 99 + si)
        for (cu, v0, v1) in s.cross:
            add(f"{s.name} - vicolo {cu:+.0f}", R.resample([w(cu, v0), w(cu, v1)], 40.0), "residential")
        for *box, f in s.wild:
            paint_poly(fuel, rect(w, *box), f)  # vegetation, so not an area: the render reads it from the fuel
        for kind, boxes in (("core", [s.core]), ("irrigated", s.irrigated)):
            for bi, box in enumerate(boxes):
                if layout.organic and kind == "core" and box[1] - box[0] > 0:
                    ring = ring_core
                    paint_poly_any(fuel, ring, 0)
                elif layout.organic and kind == "irrigated":
                    ring = organic_box(w, *box, seed=layout.seed + 50 + bi, corner=40.0, wobble=18.0)
                    paint_poly_any(fuel, ring, 0)
                else:
                    ring = rect(w, *box)
                    paint_poly(fuel, ring, 0)
                areas.append({"kind": kind, "locality": s.name, "ring": ring})

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
    for sname, box, f in layout.verges:
        paint_poly(fuel, rect(frame(next(s for s in layout.settlements if s.name == sname)), *box), f)
    for rname, sname, u0, u1, f in layout.wooded_roads:
        st = next(s for s in layout.settlements if s.name == sname)
        a = math.radians(st.axis_deg)
        u = lambda p: (p[0] - st.centre[0]) * math.sin(a) + (p[1] - st.centre[1]) * math.cos(a)
        line = R.resample([tuple(p) for p in built[rname]["line"]], CELL_M / 2)
        paint_line(fuel, [p for p in line if u0 <= u(p) <= u1], f)

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
            if layout.organic:
                bw, bd = house_size(kind, x, y)
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


def _civic(layout):
    out = []
    for _, sn, lu, lv, wd, ht, _ in layout.landmarks:
        if sn:
            cs = next(c for c in layout.settlements if c.name == sn)
            out.append((*frame(cs)(lu, lv), max(wd, ht) / 2 + 12))
        else:
            out.append((lu, lv, max(wd, ht) / 2 + 12))
    return out


def _lanes(s, w, layout, roads, add, main_line, lane_lines, road_pts, homes, ring=None):
    """Lay out `s.lanes`: roads and the houses along them."""
    rng = np.random.default_rng(layout.seed + 7 + len(lane_lines))
    a = math.radians(s.axis_deg)
    ux, uy = math.sin(a), math.cos(a)
    civic = _civic(layout)
    zone = None
    if ring is not None:
        from matplotlib.path import Path as MplPath
        zone = MplPath(np.asarray(ring))

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
                off = ln.setback + (rng.uniform(-2.0, 14.0) if layout.organic else rng.uniform(-2.0, 5.0))
                x, y = dense[k][0] - ty * side * off, dense[k][1] + tx * side * off
                step = rng.uniform(*ln.step)
                lu, lv = local(x, y)
                if zone is not None:
                    # rural: outside the built-up core, loose, each turned its own way
                    h = rng.random()
                    acc, kind = 0.0, ln.mix[-1][0]
                    for kd, wt in ln.mix:
                        acc += wt
                        if h < acc:
                            kind = kd
                            break
                    bearing = math.degrees(math.atan2(tx, ty)) + rng.uniform(-22.0, 22.0)
                    if not zone.contains_point((x, y)) and rng.random() < ln.p \
                            and not any(math.hypot(lu - gu, lv - gv) < gr for gu, gv, gr in s.gaps) \
                            and house_fits(x, y, kind, bearing, homes, 9.0, civic, road_pts,
                                           skip_road=None if not ln.road else mine):
                        homes.append((ln.loc or s.name, x, y, kind, bearing))
                    d += step
                    continue
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


# --------------------------------------------------------------------------- house placement

RENDER_SCALE = 1.9      # the kiosk draws footprints this much larger (`visual_scale::BUILDING_FOOTPRINT`)
STREET_HALF_M = 5.0     # half the painted width of a street, as drawn


def house_size(kind, x, y):
    """Footprint (along the street, across it) in metres, varying from house to
    house, from the position alone so placement and the written ring agree."""
    bw, bd = FOOTPRINT[kind]
    h1 = (int(x * 7.3) * 2654435761 + int(y * 3.1) * 40503) % 1000 / 1000.0
    h2 = (int(y * 5.7) * 2246822519 + int(x * 1.9) * 3266489917) % 997 / 997.0
    return bw * (0.78 + 0.3 * h1), bd * (0.82 + 0.25 * h2)


def _rect_axes(x, y, kind, bearing):
    bw, bd = house_size(kind, x, y)
    a = math.radians(90 - bearing)
    return (x, y, bw * RENDER_SCALE / 2, bd * RENDER_SCALE / 2, math.cos(a), math.sin(a))


def rects_overlap(r1, r2, gap):
    """Separating-axis test of two rendered footprints kept `gap` m apart."""
    (x1, y1, a1, b1, c1, s1), (x2, y2, a2, b2, c2, s2) = r1, r2
    dx, dy = x2 - x1, y2 - y1
    if dx * dx + dy * dy > (math.hypot(a1, b1) + math.hypot(a2, b2) + gap) ** 2:
        return False
    for ux, uy in ((c1, s1), (-s1, c1), (c2, s2), (-s2, c2)):
        ext1 = a1 * abs(c1 * ux + s1 * uy) + b1 * abs(-s1 * ux + c1 * uy)
        ext2 = a2 * abs(c2 * ux + s2 * uy) + b2 * abs(-s2 * ux + c2 * uy)
        if abs(dx * ux + dy * uy) > ext1 + ext2 + gap:
            return False
    return True


def house_fits(x, y, kind, bearing, homes, gap, civic, road_pts, skip_road=None, free_of=None):
    """True when the rendered footprint overlaps no house, civic building or
    road other than `skip_road`, and no `free_of(x, y)` exclusion."""
    r = _rect_axes(x, y, kind, bearing)
    reach = math.hypot(r[2], r[3])
    for _, hx, hy, hk, hb in homes:
        if abs(hx - x) < 60 and abs(hy - y) < 60 and rects_overlap(r, _rect_axes(hx, hy, hk, hb), gap):
            return False
    for cx, cy, cr in civic:
        if math.hypot(x - cx, y - cy) < cr + reach * 0.6:
            return False
    near = STREET_HALF_M + min(r[2], r[3]) * 0.9
    for j, pts in enumerate(road_pts):
        if j == skip_road:
            continue
        if np.min(np.hypot(pts[:, 0] - x, pts[:, 1] - y)) < near:
            return False
    return not (free_of and free_of(x, y))


def _urban(s, w, layout, ring, add, main_line, road_pts, homes, civic, seed):
    """The built-up fabric inside the core: streets parallel to the main one,
    joined by cross streets, with houses in aligned rows facing them, attached
    or nearly so. Rural land outside the core keeps its scattered lanes."""
    from matplotlib.path import Path as MplPath
    zone = MplPath(np.asarray(ring))
    inside = lambda x, y: bool(zone.contains_point((x, y)))
    a = math.radians(s.axis_deg)
    ux, uy = math.sin(a), math.cos(a)
    cu0, cu1, cv0, cv1 = s.urban or s.core
    rng = np.random.default_rng(seed)

    def local(x, y):
        dx, dy = x - s.centre[0], y - s.centre[1]
        return dx * ux + dy * uy, -dx * uy + dy * ux

    streets = []        # (road_pts index or None, dense polyline)
    # 1. parallels, wavering a little, clipped to the core
    k_max = int(max(abs(cv0), abs(cv1)) // s.block_m)
    for k in range(-k_max, k_max + 1):
        if k == 0:
            continue
        v0 = k * s.block_m
        if not (cv0 + 12 < v0 < cv1 - 12):
            continue
        pts = []
        u = cu0
        while u <= cu1:
            v = v0 + 7.0 * _vnoise(u / 140.0, k * 3.7, seed)
            pts.append(w(u, v))
            u += 12.0
        run = []
        for p in pts + [None]:
            if p is not None and inside(*p):
                run.append(p)
                continue
            if len(run) >= 5:
                line = R.resample(R.chaikin(run[::2] + [run[-1]], 1), 25.0)
                add(f"{s.name} - via {k:+d}", line, "residential")
                dense = np.array(R.resample(line, 2.0))
                road_pts.append(dense)
                streets.append((len(road_pts) - 1, dense))
            run = []
    # 2. cross streets between the outermost parallels, at varied spacing
    vs = sorted({round(v) for v in [cv0 + 12, cv1 - 12]})
    u = cu0 + 55.0
    n = 0
    while u < cu1 - 30:
        uu = u + 12.0 * _vnoise(u / 60.0, 11.3, seed)
        va, vb = vs[0], vs[-1]
        line = [w(uu + 6 * _vnoise(v / 50.0, 5.0, seed), v) for v in np.linspace(va, vb, 9)]
        run = []
        for p in line + [None]:
            if p is not None and inside(*p):
                run.append(p)
                continue
            if len(run) >= 3:
                add(f"{s.name} - vicolo {n}", R.resample(run, 25.0), "residential")
                road_pts.append(np.array(R.resample(run, 2.0)))
                n += 1
            run = []
        u += 78.0 + 30.0 * (_vnoise(u / 30.0, 2.0, seed) + 1) / 2
    # 3. the main street itself
    if main_line is not None:
        dense = np.array(R.resample([tuple(q) for q in main_line], 2.0))
        streets.append((None, dense))
    # 4. houses along every street, both sides, in a row
    for road_idx, dense in streets:
        seg = np.hypot(*np.diff(dense, axis=0).T)
        cum = np.concatenate([[0.0], np.cumsum(seg)])
        for side in (-1, 1):
            d = rng.uniform(0, 8)       # the row's leading edge, along the street
            while d < cum[-1]:
                mh = rng.random()
                acc, kind = 0.0, s.mix[-1][0]
                for kd, wt in s.mix:
                    acc += wt
                    if mh < acc:
                        kind = kd
                        break
                kk = min(int(np.searchsorted(cum, d)), len(dense) - 1)
                bw, bd = house_size(kind, *dense[kk])
                dc = d + bw * RENDER_SCALE / 2
                k = min(int(np.searchsorted(cum, dc)), len(dense) - 1)
                k0, k1 = max(k - 4, 0), min(k + 4, len(dense) - 1)
                tx, ty = dense[k1] - dense[k0]
                nn = math.hypot(tx, ty) or 1.0
                tx, ty = tx / nn, ty / nn
                bearing = math.degrees(math.atan2(tx, ty)) + rng.uniform(-2.0, 2.0)
                setback = STREET_HALF_M + bd * RENDER_SCALE / 2 + rng.uniform(0.8, 2.5)
                x = dense[k][0] - ty * side * setback
                y = dense[k][1] + tx * side * setback
                lu, lv = local(x, y)
                free = inside(x, y) and not any(math.hypot(lu - gu, lv - gv) < gr for gu, gv, gr in s.gaps) \
                    and house_fits(x, y, kind, bearing, homes, s.urban_gap * 0.5, civic, road_pts, skip_road=road_idx)
                if free and rng.random() < 0.96:
                    homes.append((s.name, x, y, kind, bearing))
                    d += bw * RENDER_SCALE + s.urban_gap * rng.uniform(0.3, 1.2)
                else:
                    d += 3.0     # try again a little further on: an alley, a yard


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
