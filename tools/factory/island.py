"""The island: the territory as it is, with a coast of its own.

The scenario's square window is mountainous right up to its edges (91-684 m),
so the world was a clipped piece of land. This pass keeps every house, road,
ignition and the whole interior as they are and gives the land a *shape*: the
coast is an iso-line of a field built from the settled ground (distance to the
houses and ignitions, warped and varied by fractal noise), not a margin cut off
the window. It then shapes the ground along it:

* a beach side (east, round the port): a long sandy shelf, dunes behind it;
* a cliff side (everywhere else): the land drops steeply to the sea over a
  hundred metres, rough with ridged noise, with a few offshore stacks;
* pocket beaches where the coast is low.

The original data are not touched: the result is written to `out/island_data/`
in the same format, so `ROCCA_DATA=out/island_data` plays it and `data/` stays
the A of the A/B.

What changes in the model (and only there):
* the sea is cells at or below 0.5 m (render) / 2 m (fire DEM), non-burnable --
  the rule `abm::haven::is_water` and `terrain_mesh` already read;
* the shore strip (sand, bare rock) is non-burnable;
* roads are cut where they meet the sea; the SP 12's east end becomes a port
  with a pier to the window edge (`refuge::EDGE_M`: a node within 120 m of the
  edge is the exit), and a coastal road joins it to the SP 9;
* the outside team arrives at the quay.

Fractal geometry: coast and relief come from spectral fractional Brownian
fields (power ~ k^-(2H+2)) at several scales, with domain warping, so the
outline is irregular at every scale from kilometres to a few metres.
"""

from __future__ import annotations

import json
import math
import shutil
from pathlib import Path

import numpy as np
from scipy.ndimage import distance_transform_edt, gaussian_filter, label, map_coordinates
from scipy.spatial import cKDTree

from . import fine as F
from . import roads as R
from .grid import CELL_M, N, WORLD_M, smoothstep

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "data" / "scenarios" / "rocca_ventosa"
OUT = ROOT / "out" / "island_data"
DOCS = ROOT / "docs" / "factory" / "isola"

FM = F.FINE_M
NF = F.NF
SEA_RENDER_M = 0.5          # terrain_mesh: at or below is sea
LAND_MIN_M = 0.62           # the lowest land node
PORT_XY = (7520.0, 3300.0)  # where the beach and the port are meant to be
EDGE_CLEAR_M = 300.0        # the coast stays this far from the window edge
HOUSE_CLEAR_M = 170.0       # and houses this far from the water

# The rocca on the summit and the stone tower on a headland (render props:
# `crates/game/src/landmarks.rs`). Radii in metres: the level platform and
# where it blends back into the ground; the props are drawn at `scale`.
LANDMARKS = {
    "rocca": {"flat_m": 104.0, "blend_m": 200.0, "scale": 1.9, "wall_half_m": 31.0, "fuel_free_m": 130.0},
    "torre": {"flat_m": 22.0, "blend_m": 60.0, "scale": 2.2, "fuel_free_m": 32.0},
}

PARAMS = {
    "seed": 7,
    "reach_m": [760.0, 1280.0],
    "warp_m": 260.0,
    "mid_detail_m": 90.0,
    "fine_detail_m": 15.0,
    "hurst": {"warp": 0.75, "reach": 0.7, "mid": 0.65, "fine": 0.6},
    "beach_width_m": 800.0,
    "cliff_width_m": 110.0,
    "cliff_cut": 0.78,
    "peninsulas": [[(2300, 3000), (1050, 1450)], [(4800, 6300), (5250, 7250)], [(4500, 2450), (4850, 1250)]],
    "east_lobe": {"centre": (6750.0, 3500.0), "semi_axes_m": (800.0, 1400.0), "strength_m": 700.0},
}


# --------------------------------------------------------------------------- fractal fields

def fractal(rng, hurst, scale_min_m, scale_max_m):
    """Unit-variance fractional Brownian field on the 5 m node grid: power
    spectrum k^-(2H+2), band-limited to [scale_min, scale_max] metres."""
    m = NF + 64
    ky = np.fft.fftfreq(m, FM)[:, None]
    kx = np.fft.rfftfreq(m, FM)[None, :]
    k = np.sqrt(kx ** 2 + ky ** 2)
    k[0, 0] = 1.0
    amp = k ** -(hurst + 1.0) * np.exp(-(k * scale_min_m) ** 2) * (1 - np.exp(-(k * scale_max_m) ** 2))
    amp[0, 0] = 0.0
    f = np.fft.irfft2(np.fft.rfft2(rng.normal(size=(m, m))) * amp, s=(m, m))[:NF, :NF]
    return (f - f.mean()) / f.std()


def seg_dist(X, Y, a, b):
    ax, ay = a
    bx, by = b
    vx, vy = bx - ax, by - ay
    t = np.clip(((X - ax) * vx + (Y - ay) * vy) / (vx * vx + vy * vy), 0, 1)
    return np.hypot(X - ax - t * vx, Y - ay - t * vy)


def smooth01(x):
    x = np.clip(x, 0.0, 1.0)
    return x * x * (3 - 2 * x)


# --------------------------------------------------------------------------- the shape of the land

def build_shape(osm, game, rng):
    """Signed distance to the coast on the node grid (metres, positive inland),
    the beach weight, and the stacks. Returns a dict."""
    X, Y = F.node_xy()
    rr, cc = np.mgrid[0:NF, 0:NF].astype(float)

    pts = np.array([b["centroid"] for b in osm["buildings"]])
    ign = np.array([c["ignition"] for c in game["cases"]])
    sta = np.array([s["pos"] for s in game["stations"] if s["pos"][0] < WORLD_M - 100])

    def dist_to(points):
        m = np.zeros((NF, NF), bool)
        c = np.clip(np.round(points[:, 0] / FM).astype(int), 0, NF - 1)
        r = np.clip(np.round((WORLD_M - points[:, 1]) / FM).astype(int), 0, NF - 1)
        m[r, c] = True
        return distance_transform_edt(~m, sampling=FM)

    d_b = dist_to(pts)
    d_i = dist_to(np.vstack([ign, sta]))
    # the levels' fires start where they were tuned to: they keep a wide
    # margin of land, so a new bay does not change how the fire runs
    named = {c["name"]: c["ignition"] for c in game["cases"]}
    feat = np.array([named[n] for n in game.get("featured", []) if n in named])
    d_f = dist_to(feat) if len(feat) else np.full((NF, NF), 1e9)
    d_eff = np.minimum(d_b, d_i + 150.0)

    H = PARAMS["hurst"]
    wx = PARAMS["warp_m"] * fractal(rng, H["warp"], 110.0, 3000.0)
    wy = PARAMS["warp_m"] * fractal(rng, H["warp"], 110.0, 3000.0)
    d_w = map_coordinates(d_eff, [rr + wy / FM, cc + wx / FM], order=1, mode="nearest")

    lo, hi = PARAMS["reach_m"]
    reach = lo + (hi - lo) * (0.5 + 0.5 * np.tanh(1.4 * fractal(rng, H["reach"], 500.0, 4500.0)))
    for a, b in PARAMS["peninsulas"]:
        reach = reach + 650.0 * np.exp(-(seg_dist(X, Y, a, b) / 330.0) ** 2)
    f_main = reach - d_w

    lobe = PARAMS["east_lobe"]
    (ex, ey), (sx, sy) = lobe["centre"], lobe["semi_axes_m"]
    f_lobe = lobe["strength_m"] * (1.0 - np.sqrt(((X - ex) / sx) ** 2 + ((Y - ey) / sy) ** 2))
    field = np.maximum(f_main, f_lobe)

    # the coast itself is fractal at every scale: mid-sized bays and headlands,
    # then the rocks and inlets
    field = field + PARAMS["mid_detail_m"] * fractal(rng, H["mid"], 150.0, 900.0)
    field = field + PARAMS["fine_detail_m"] * fractal(rng, H["fine"], 20.0, 160.0)
    # the settled ground and every ignition keep dry land round them whatever
    # the noise does: a hard floor on the field
    field = np.maximum(field, np.maximum(np.maximum(480.0 - d_i, 400.0 - d_b), 1500.0 - d_f))
    wsdf = np.minimum(np.minimum(X, Y), np.minimum(WORLD_M - X, WORLD_M - Y))
    field = np.minimum(field, wsdf - EDGE_CLEAR_M)
    field = gaussian_filter(field, 1.2)

    land = field > 0
    # keep the piece with the houses; other pieces are dropped (the stacks are
    # added on purpose, below)
    lab, n = label(land)
    keep = set(np.unique(lab[np.clip(np.round((WORLD_M - pts[:, 1]) / FM).astype(int), 0, NF - 1),
                             np.clip(np.round(pts[:, 0] / FM).astype(int), 0, NF - 1)]))
    keep.discard(0)
    land = np.isin(lab, list(keep))

    # beach side: a broad weight round the port, plus pocket beaches where the
    # coast is mid-scale concave and low (decided later, once heights are known)
    px, py = PORT_XY
    b_large = np.exp(-((X - px) ** 2 + (Y - py) ** 2) / (2 * 1700.0 ** 2))
    pocket = smooth01((fractal(rng, 0.6, 200.0, 900.0) - 0.9) / 0.7)
    return {"X": X, "Y": Y, "land": land, "field": field, "b_large": b_large, "pocket": pocket,
            "d_b": d_b, "d_i": d_i}


def add_stacks(shape, rng, hn):
    """A few offshore rock stacks on the cliff side."""
    X, Y, land = shape["X"], shape["Y"], shape["land"]
    L = distance_transform_edt(~land, sampling=FM)     # distance out to the land
    cand = np.argwhere((L > 150) & (L < 420) & (shape["b_large"] < 0.15)
                       & (X > 600) & (X < WORLD_M - 600) & (Y > 600) & (Y < WORLD_M - 600))
    rng.shuffle(cand)
    chosen = []
    for r, c in cand:
        p = (c * FM, WORLD_M - r * FM)
        if all(math.hypot(p[0] - q[0], p[1] - q[1]) > 650 for q in chosen):
            chosen.append(p)
        if len(chosen) >= 6:
            break
    stack = np.zeros_like(land)
    meta = []
    detail = fractal(rng, 0.6, 8.0, 80.0)
    for p in chosen:
        rad = float(rng.uniform(38, 105))
        height = float(rng.uniform(16, 44))
        d = np.hypot(X - p[0], Y - p[1])
        m = (rad + 18.0 * detail - d) > 0
        stack |= m
        meta.append({"pos": [round(p[0], 1), round(p[1], 1)], "radius_m": round(rad, 1), "height_m": round(height, 1)})
    return stack, meta


# --------------------------------------------------------------------------- the ground

def island_ground(base, shape, stack, stack_meta, rng):
    """The height field: the original ground shaped by the coast. Returns
    heights (sea = 0), the signed distance to the coast, and the beach weight."""
    X, Y, land = shape["X"], shape["Y"], shape["land"]
    full = land | stack
    L = distance_transform_edt(full, sampling=FM) - distance_transform_edt(~full, sampling=FM)
    Lm = distance_transform_edt(land, sampling=FM) - distance_transform_edt(~land, sampling=FM)

    hn = np.maximum(base, 0.6)
    # beach weight: broad round the port, pocket beaches where the ground is low
    low = smooth01((260.0 - base) / 160.0)
    B = np.clip(1.25 * shape["b_large"] + 0.9 * shape["pocket"] * low * (1 - shape["b_large"]), 0, 1)
    B = gaussian_filter(B, 6.0)

    Lp = np.maximum(Lm, 0.0)
    beach_w, cliff_w = PARAMS["beach_width_m"], PARAMS["cliff_width_m"]

    # beach: gentle sand, then dunes, then the ground
    dune_n = 0.6 + 0.4 * fractal(rng, 0.7, 10.0, 120.0)
    dunes = 2.6 * np.exp(-((Lp - 70.0) / 38.0) ** 2) * dune_n + 1.5 * np.exp(-((Lp - 135.0) / 30.0) ** 2) * dune_n
    z_beach = 0.6 + 0.035 * np.minimum(Lp, 35.0) + (hn - 0.6) * smooth01(Lp / beach_w) ** 1.6 + dunes

    # cliff: near-vertical rise, rough with ridged noise, lower at the edge
    cl = np.clip(Lp / cliff_w, 0.0, 1.0) ** 0.4
    shrink = (1 - PARAMS["cliff_cut"]) + PARAMS["cliff_cut"] * smooth01(Lp / 1200.0)
    ridged = 1.0 - np.abs(fractal(rng, 0.8, 9.0, 220.0))
    rough = ridged - ridged.mean()
    rough_zone = smooth01(Lp / 40.0) * (1 - smooth01((Lp - 120.0) / 380.0))
    z_cliff = 0.6 + (hn - 0.6) * shrink * cl + 0.075 * (hn - 0.6) * rough * rough_zone

    z = B * z_beach + (1 - B) * z_cliff
    z = np.where(Lm > 0, np.maximum(z, LAND_MIN_M), 0.0)

    # stacks: their own small profile
    if stack.any():
        zs = np.zeros_like(z)
        for s in stack_meta:
            d = np.hypot(X - s["pos"][0], Y - s["pos"][1])
            prof = np.clip(1.0 - (d / (s["radius_m"] + 8.0)) ** 2, 0, 1) ** 0.5
            zs = np.maximum(zs, s["height_m"] * prof)
        rs = 0.55 + 0.45 * (1.0 - np.abs(fractal(rng, 0.8, 6.0, 60.0)))
        zs = np.where(stack & (L > 0), np.maximum(zs * rs, LAND_MIN_M), 0.0)
        z = np.maximum(z, zs)
    return z, L, B


# --------------------------------------------------------------------------- roads

def sample_grid(a, pts):
    p = np.asarray(pts, float)
    return map_coordinates(a, [(WORLD_M - p[:, 1]) / FM, p[:, 0] / FM], order=1, mode="nearest")


def trim_roads(roads, L, min_L=14.0, port_min_L=14.0):
    """Cut every road where it reaches the water. A road that leaves the land
    and comes back is split in two; runs shorter than 40 m are dropped."""
    out = []
    next_id = max(r["id"] for r in roads) + 1
    for road in roads:
        line = [tuple(p) for p in road["line"]]
        z = sample_grid(L, line)
        runs, cur = [], []
        for i, p in enumerate(line):
            if z[i] >= min_L:
                if not cur and i > 0 and z[i - 1] < min_L:
                    t = (min_L - z[i - 1]) / (z[i] - z[i - 1])
                    a, b = line[i - 1], p
                    cur.append((a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])))
                cur.append(p)
            else:
                if cur:
                    t = (min_L - z[i - 1]) / (z[i] - z[i - 1])
                    a, b = line[i - 1], p
                    cur.append((a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])))
                    runs.append(cur)
                    cur = []
        if cur:
            runs.append(cur)
        first = True
        for run in runs:
            length = sum(math.hypot(b[0] - a[0], b[1] - a[1]) for a, b in zip(run[:-1], run[1:]))
            if length < 40.0:
                continue
            r = dict(road)
            r["line"] = [[round(x, 2), round(y, 2)] for x, y in run]
            if first:
                first = False
            else:
                r["id"] = next_id
                next_id += 1
            out.append(r)
    return out


def keep_connected(roads):
    """Only the largest connected piece of the road network: a stub left alone
    by the sea is not a road anyone can use."""
    parent = {}

    def find(a):
        while parent.setdefault(a, a) != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a
    key = lambda p: (round(p[0], 1), round(p[1], 1))
    for r in roads:
        k0 = key(r["line"][0])
        for p in r["line"][1:]:
            parent[find(key(p))] = find(k0)
    length = {}
    for r in roads:
        root = find(key(r["line"][0]))
        length[root] = length.get(root, 0.0) + sum(math.hypot(b[0] - a[0], b[1] - a[1]) for a, b in zip(r["line"][:-1], r["line"][1:]))
    best = max(length, key=length.get)
    return [r for r in roads if find(key(r["line"][0])) == best]


# --------------------------------------------------------------------------- port and coastal road

def make_port(roads, L, game, rng):
    """The SP 12's east end becomes a port: a pier out to the window edge, a
    breakwater round a small basin, a quay at the end."""
    sp12 = max((r for r in roads if r["name"] == "SP 12 della Valle"),
               key=lambda r: max(p[0] for p in r["line"]))
    line = sp12["line"]
    i_e = int(np.argmax([p[0] for p in line]))
    root = tuple(line[i_e])
    # the pier runs east from the road's end, straight to 100 m from the edge
    tip = (WORLD_M - 100.0, root[1])
    n_pier = max(2, int(math.hypot(tip[0] - root[0], tip[1] - root[1]) // 40))
    pier = [(root[0] + (tip[0] - root[0]) * k / n_pier, root[1] + (tip[1] - root[1]) * k / n_pier) for k in range(n_pier + 1)]
    zz = sample_grid(L, pier)
    shore_i = int(np.argmax(zz < 0)) if (zz < 0).any() else 0
    shore = pier[shore_i]
    next_id = max(r["id"] for r in roads) + 1
    pier_road = {"id": next_id, "class": "secondary", "drivable": True, "track": False, "name": "Molo del Porto",
                 "oneway": False, "pier": True, "line": [[round(x, 2), round(y, 2)] for x, y in pier]}
    # the breakwater arms: out from either side of the shore, then in toward the pier
    u, n = np.array([1.0, 0.0]), np.array([0.0, 1.0])
    s = np.array(shore)
    arms = []
    for sg in (+1, -1):
        a = s + sg * n * 150.0 - u * 25.0
        b = a + u * 260.0
        c = b - sg * n * 95.0 + u * 45.0
        arms.append([tuple(a), tuple(b), tuple(c)])
    moorings = []
    for k in range(9):
        t = 0.15 + 0.7 * k / 8
        side = 1 if k % 2 == 0 else -1
        p = np.array(pier[0]) * (1 - t) + np.array(pier[-1]) * t
        p = p + side * n * 24.0
        moorings.append([round(float(p[0]), 1), round(float(p[1]), 1)])
    return {"road": pier_road, "root": root, "tip": tip, "shore": tuple(shore), "arms": arms,
            "moorings": moorings, "root_index": i_e}


def coastal_road(roads, L, port, next_id):
    """A road from the port's land end to the SP 9's west end, along the south
    side of the coast: the 110 m inland iso-line of the signed distance,
    smoothed so it follows the headlands and cuts across the small coves, with
    a stub at each end to the roads it joins."""
    import contourpy
    from scipy.ndimage import gaussian_filter1d

    sp9 = next(r for r in roads if r["name"] == "SP 9 di Fondovalle")
    end_a = np.array(port["root"])
    end_b = np.array(min(sp9["line"], key=lambda p: p[0]))
    ys = np.arange(NF) * FM
    xs = np.arange(NF) * FM
    cg = contourpy.contour_generator(xs, ys, gaussian_filter(L, 3.0)[::-1, :])
    ring = np.asarray(max(cg.lines(110.0), key=len))
    n = len(ring)
    ia = int(np.argmin(np.hypot(ring[:, 0] - end_a[0], ring[:, 1] - end_a[1])))
    ib = int(np.argmin(np.hypot(ring[:, 0] - end_b[0], ring[:, 1] - end_b[1])))
    fwd = ring[[(ia + k) % n for k in range((ib - ia) % n + 1)]]
    bwd = ring[[(ia - k) % n for k in range((ia - ib) % n + 1)]]
    arc = fwd if fwd[:, 1].mean() < bwd[:, 1].mean() else bwd
    # even spacing, then a long smoothing along the arc
    seg = np.hypot(*np.diff(arc, axis=0).T)
    s_ = np.concatenate([[0], np.cumsum(seg)])
    t = np.arange(0, s_[-1], 10.0)
    arc = np.c_[np.interp(t, s_, arc[:, 0]), np.interp(t, s_, arc[:, 1])]
    arc = np.c_[gaussian_filter1d(arc[:, 0], 9, mode="nearest"), gaussian_filter1d(arc[:, 1], 9, mode="nearest")]
    # smoothing cuts the coves: push anything near the water back inland
    gy, gx = np.gradient(gaussian_filter(L, 4.0), FM)
    for _ in range(40):
        z = sample_grid(L, arc)
        bad = z < 60.0
        if not bad.any():
            break
        rows = (WORLD_M - arc[:, 1]) / FM
        cols = arc[:, 0] / FM
        gxs = map_coordinates(gx, [rows, cols], order=1, mode="nearest")
        gys = map_coordinates(gy, [rows, cols], order=1, mode="nearest")
        step = np.c_[gxs, -gys]   # rows grow south: a rise along +row is a fall along +y
        nrm = np.maximum(np.hypot(step[:, 0], step[:, 1]), 1e-6)[:, None]
        arc[bad] += (step / nrm)[bad] * 8.0
        arc = np.c_[gaussian_filter1d(arc[:, 0], 3, mode="nearest"), gaussian_filter1d(arc[:, 1], 3, mode="nearest")]
    pts = [tuple(end_a)] + [tuple(p) for p in arc[3:-3]] + [tuple(end_b)]
    pts = R.simplify(R.resample(pts, 20.0), 2.5)
    return {"id": next_id, "class": "secondary", "drivable": True, "track": False,
            "name": "Strada Costiera", "oneway": False,
            "line": [[round(x, 2), round(y, 2)] for x, y in pts]}


# --------------------------------------------------------------------------- the whole pass


def flatten(z, centre, r_flat, r_blend, pct=None):
    """A level platform under `r_flat`, blended back into the ground out to
    `r_blend`. The level is the mean ground height, or its `pct` percentile:
    a high one fills a slope instead of cutting it, so a tower on a cliff edge
    stands on a ledge and does not sink into a notch. Returns the field and
    the level."""
    X, Y = F.node_xy()
    d = np.hypot(X - centre[0], Y - centre[1])
    under = z[d <= r_flat]
    level = float(under.mean() if pct is None else np.percentile(under, pct))
    w = 1.0 - smooth01((d - r_flat) / (r_blend - r_flat))
    return z * (1 - w) + level * w, level


def pick_summit(z, L):
    """The island's highest ground, well inland."""
    inland = np.where(L > 350.0, z, -1.0)
    r, c = np.unravel_index(np.argmax(inland), inland.shape)
    return (float(c * FM), float(WORLD_M - r * FM))


def summit_track(roads, summit, dem20, next_id):
    """A track from the nearest road up to the rocca's gate. The gate faces the
    side the track comes from, snapped to the walls' four sides."""
    best, bd = None, 1e18
    for r in roads:
        if not r["drivable"] or r["name"] == "Strada Costiera":
            continue
        for p in r["line"]:
            d = math.hypot(p[0] - summit[0], p[1] - summit[1])
            if d < bd:
                best, bd = tuple(p), d
    ang = math.atan2(best[1] - summit[1], best[0] - summit[0])
    side = round(ang / (math.pi / 2)) * (math.pi / 2)
    half = LANDMARKS["rocca"]["wall_half_m"] * LANDMARKS["rocca"]["scale"]
    gate = (summit[0] + math.cos(side) * half, summit[1] + math.sin(side) * half)
    last = None
    for gmax in (0.16, 0.20, 0.25):
        try:
            path = R.least_cost_path(dem20, best, gate, grade_ok=0.10, grade_max=gmax, margin_m=2500.0, coarse=2,
                                     turn_m=12.0, hairpin_m=40.0)
            break
        except RuntimeError as e:
            last = e
    else:
        raise last
    track = {"id": next_id, "class": "track", "drivable": False, "track": True, "name": "Sentiero della Rocca",
             "oneway": False, "line": [[round(x, 2), round(y, 2)] for x, y in R.resample(path, 25.0)]}
    return track, gate, math.degrees(side)


def pick_tower(coast_road, L, summit):
    """A headland on the coast road: where the sea is on most sides, a little
    inland of the road, away from the port."""
    from scipy.ndimage import uniform_filter
    sea = (L <= 0).astype(float)
    sea_frac = uniform_filter(sea, size=int(900 / FM))
    pts = np.array(R.resample([tuple(p) for p in coast_road["line"]], 20.0))
    score = sample_grid(sea_frac, pts)
    score[pts[:, 0] > 5200.0] = -1.0                      # the east is the port's
    score[np.hypot(pts[:, 0] - summit[0], pts[:, 1] - summit[1]) < 900.0] = -1.0
    i = int(np.argmax(score))
    p = pts[i]
    gy, gx = np.gradient(gaussian_filter(L, 3.0), FM)
    row, col = (WORLD_M - p[1]) / FM, p[0] / FM
    g = np.array([map_coordinates(gx, [[row], [col]], order=1)[0], -map_coordinates(gy, [[row], [col]], order=1)[0]])
    g = g / max(np.hypot(*g), 1e-6)
    return (float(p[0] + g[0] * 40.0), float(p[1] + g[1] * 40.0))   # inland of the road, on the cliff top


def stats_fractal_dimension(land):
    """Box-counting dimension of the coastline."""
    edge = land ^ np.roll(land, 1, 0) | land ^ np.roll(land, 1, 1)
    sizes = [1, 2, 4, 8, 16, 32, 64]
    cnt = []
    for s in sizes:
        m = (NF // s) * s
        b = edge[:m, :m].reshape(m // s, s, m // s, s).any(axis=(1, 3))
        cnt.append(b.sum())
    sl = np.polyfit(np.log(sizes), np.log(cnt), 1)[0]
    return float(-sl)


def build(out_dir: Path = OUT):
    rng = np.random.default_rng(PARAMS["seed"])
    osm = json.loads((SRC / "osm.json").read_text())
    game = json.loads((SRC / "game.json").read_text())
    params = json.loads((SRC / "params.json").read_text())
    pop = json.loads((SRC / "population.json").read_text())
    dem20 = np.fromfile(SRC / "dem.f64", "<f8").reshape(N, N)
    fuel = np.fromfile(SRC / "fuel.i32", "<i4").reshape(N, N)
    cover = np.fromfile(SRC / "cover.u8", "u1").reshape(F.NC, F.NC)
    old_fine = np.fromfile(SRC / "render_terrain.f32", "<f4").reshape(NF, NF)

    shape = build_shape(osm, game, rng)
    base = F.upsample(dem20)
    stack, stack_meta = add_stacks(shape, rng, base)
    z, L, B = island_ground(base, shape, stack, stack_meta, rng)
    land_all = shape["land"] | stack

    # checks the shape has to pass
    pts = np.array([b["centroid"] for b in osm["buildings"]])
    Lb = sample_grid(L, pts)
    ign = np.array([c["ignition"] for c in game["cases"]])
    Li = sample_grid(L, ign)
    assert Lb.min() > HOUSE_CLEAR_M, f"a house is {Lb.min():.0f} m from the sea"
    assert Li.min() > 120.0, f"an ignition is {Li.min():.0f} m from the sea"

    # roads: cut at the sea, then the port and the coastal road
    roads = trim_roads([dict(r) for r in osm["roads"]], L)
    port = make_port(roads, L, game, rng)
    next_id = max(r["id"] for r in roads) + 2
    coast_road = coastal_road(roads, L, port, next_id)
    summit = pick_summit(z, L)
    track, gate, gate_deg = summit_track(roads + [coast_road], summit, F.to_fire_dem(z), next_id + 1)
    tower = pick_tower(coast_road, L, summit)
    carve_roads = roads + [coast_road, track]
    all_roads = carve_roads + [port["road"]]
    R.crossings(all_roads)
    R.connect_junctions(all_roads)
    all_roads = keep_connected(all_roads)
    carve_roads = [r for r in all_roads if not r.get("pier")]

    # the ground the way `fine.build` makes it: relief, benches, pads
    rngm = np.random.default_rng(F.FINE["micro_seed"] + params["seed"])
    gy, gx = np.gradient(z, FM)
    slope = np.hypot(gx, gy)
    relief = F.micro_relief(rngm, z.shape)
    natural = z + (F.FINE["micro_relief_m"] + F.FINE["micro_relief_slope_m"] * np.clip(slope / 0.5, 0, 1)) * relief
    sea = L <= 0
    natural = np.where(sea, 0.0, np.maximum(natural, LAND_MIN_M))
    lm = LANDMARKS
    natural, z_rocca = flatten(natural, summit, lm["rocca"]["flat_m"], lm["rocca"]["blend_m"])
    natural, z_torre = flatten(natural, tower, lm["torre"]["flat_m"], lm["torre"]["blend_m"], pct=80)
    landmarks = {
        "rocca": {"pos": [round(summit[0], 1), round(summit[1], 1)], "ground_m": round(z_rocca, 2),
                  "yaw_deg": round(gate_deg, 1), "scale": lm["rocca"]["scale"], "wall_half_m": lm["rocca"]["wall_half_m"]},
        "torre": {"pos": [round(tower[0], 1), round(tower[1], 1)], "ground_m": round(z_torre, 2), "scale": lm["torre"]["scale"]},
    }
    benched, road_stats = F.carve_roads(natural, carve_roads)
    fine = F.pad_buildings(benched, osm["buildings"])
    fine = np.where(sea, 0.0, np.maximum(fine, LAND_MIN_M))
    fine = shape_port(fine, L, port)

    return {"landmarks": landmarks, "fine": fine, "L": L, "B": B, "land": land_all, "roads": all_roads, "port": port, "stack": stack_meta,
            "osm": osm, "game": game, "params": params, "pop": pop, "dem20": dem20, "fuel": fuel, "cover": cover,
            "old_fine": old_fine, "road_stats": road_stats, "shape": shape}


def dem20_from(z):
    """The fire grid's view of a fine field (cell means)."""
    return F.to_fire_dem(np.asarray(z, float))


def shape_port(fine, L, port):
    """Raise the pier, quay and breakwater out of the sea."""
    X, Y = F.node_xy()
    out = fine.copy()
    def raise_strip(poly, half_w, z):
        d = np.full(X.shape, np.inf)
        for a, b in zip(poly[:-1], poly[1:]):
            d = np.minimum(d, seg_dist(X, Y, a, b))
        w = 1.0 - smooth01((d - half_w) / 5.0)
        nonlocal out
        out = np.maximum(out, z * w)
        return d <= half_w + 2.5
    pier = [tuple(p) for p in port["road"]["line"]]
    raise_strip(pier, 8.0, 2.5)
    for arm in port["arms"]:
        raise_strip(arm, 7.0, 3.3)
    # the quay: a level square at the pier's end
    tx, ty = port["tip"]
    q = (np.abs(X - (tx - 10.0)) <= 28.0) & (np.abs(Y - ty) <= 20.0)
    out = np.where(q, np.maximum(out, 2.5), out)
    return out


# --------------------------------------------------------------------------- writing

def write(res, out_dir: Path = OUT):
    fine, L, B = res["fine"], res["L"], res["B"]
    sdir = out_dir / "scenarios" / "rocca_ventosa"
    sdir.mkdir(parents=True, exist_ok=True)
    data = ROOT / "data"
    # everything outside `scenarios/` is shared with the original data dir
    for item in data.iterdir():
        if item.name in ("scenarios", "osm_raw.json", "spotorno_render_terrain.tif"):
            continue
        dst = out_dir / item.name
        if not dst.exists():
            dst.symlink_to(item)
    port = res["port"]

    # --- terrain
    sea = fine <= SEA_RENDER_M
    fine.astype("<f4").tofile(sdir / "render_terrain.f32")
    (sdir / "render_terrain.json").write_text(json.dumps({
        "rows": NF, "cols": NF, "posting_m": FM, "world_size_m": [WORLD_M, WORLD_M],
        "elev_min": float(fine.min()), "elev_max": float(fine.max())}, indent=2) + "\n")

    # --- fire grid: the new surface where the coast changed it, the old elsewhere
    avg = F.to_fire_dem(fine)
    L20 = F.to_fire_dem(L)
    dem20 = np.where(L20 < 750.0, avg, res["dem20"])
    sea20 = F.to_fire_dem(sea.astype(float)) > 0.5
    shore = (L > 0) & (L < 45.0)
    beach = (L > 0) & (L < 85.0) & (B > 0.5)
    port_nodes = (L <= 0) & (fine > SEA_RENDER_M)
    nb = (sea20
          | (F.to_fire_dem((shore | beach).astype(float)) > 0.4)
          | (F.to_fire_dem(port_nodes.astype(float)) > 0.15))
    fuel = res["fuel"].copy()
    fuel[nb] = 0
    # the rocca and the tower stand in a ring of non-burnable ground (bare rock
    # and paving): nothing there burns, and no trees stand in the walls. The ring
    # is wider than the platform.
    cy20, cx20 = np.mgrid[0:N, 0:N]
    X20, Y20 = (cx20 + 0.5) * CELL_M, WORLD_M - (cy20 + 0.5) * CELL_M
    for name in ("rocca", "torre"):
        px, py = res["landmarks"][name]["pos"]
        fuel[np.hypot(X20 - px, Y20 - py) <= LANDMARKS[name]["fuel_free_m"]] = 0
    # the fire keeps the approved DEM round the summit: the levelled platform is
    # for the picture (the fine surface), not for the model
    rx, ry = res["landmarks"]["rocca"]["pos"]
    near_rocca = np.hypot(X20 - rx, Y20 - ry) <= LANDMARKS["rocca"]["blend_m"] + 40.0
    dem20 = np.where(near_rocca, res["dem20"], dem20)
    dem20.astype("<f8").tofile(sdir / "dem.f64")
    fuel.astype("<i4").tofile(sdir / "fuel.i32")

    # --- 5 m cover: water, the port's concrete, the new roads
    cov = res["cover"].copy()
    corners = np.stack([sea[:-1, :-1], sea[1:, :-1], sea[:-1, 1:], sea[1:, 1:]])
    cov[corners.all(axis=0)] = F.WATER
    pn = np.stack([port_nodes[:-1, :-1], port_nodes[1:, :-1], port_nodes[:-1, 1:], port_nodes[1:, 1:]]).any(axis=0)
    cov[pn] = F.PAVED
    cc = (np.arange(F.NC) + 0.5) * FM
    CX, CY = np.meshgrid(cc, WORLD_M - cc)
    for name, r_m in (("rocca", LANDMARKS["rocca"]["fuel_free_m"] - 4.0), ("torre", 26.0)):
        px, py = res["landmarks"][name]["pos"]
        cov[(np.hypot(CX - px, CY - py) <= r_m) & (cov == F.NATURAL)] = F.PAVED
    new_roads = [r for r in res["roads"] if r["name"] in ("Strada Costiera", "Molo del Porto", "Sentiero della Rocca")]
    mini = F.cover_raster({"areas": [], "buildings": [], "roads": new_roads, "water": []})
    keep = cov == F.BUILDING
    cov[(mini == F.ROAD) & ~keep] = F.ROAD
    cov.astype("u1").tofile(sdir / "cover.u8")
    shutil.copyfile(SRC / "cover.json", sdir / "cover.json")

    # --- vectors, population, game
    osm = dict(res["osm"])
    osm["roads"] = [{k: v for k, v in r.items() if k != "pier"} for r in res["roads"]]
    (sdir / "osm.json").write_text(json.dumps(osm, indent=2) + "\n")
    shutil.copyfile(SRC / "population.json", sdir / "population.json")
    game = json.loads((SRC / "game.json").read_text())
    tx, ty = port["tip"]
    for st in game["stations"]:
        if st["pos"][0] > WORLD_M - 100:
            st["pos"] = [round(tx - 10.0, 1), round(ty, 1)]
            st["name"] = "Squadra AIB (dal traghetto, al porto)"
    (sdir / "game.json").write_text(json.dumps(game, indent=2) + "\n")
    (sdir / "island.json").write_text(json.dumps({
        "sea_level_m": 0.0,
        "port": {"root": list(port["root"]), "tip": list(port["tip"]), "shore": list(port["shore"]),
                 "arms": [[list(p) for p in a] for a in port["arms"]], "moorings": port["moorings"]},
        "stacks": res["stack"],
        "landmarks": res["landmarks"],
    }, indent=2) + "\n")

    # --- scenario and params
    scn = json.loads((SRC / "scenario.json").read_text())
    scn["description"] = ("Un'isola con un borgo in piano, un nucleo di fondovalle e case sparse sul versante, "
                          "separati da un crinale con un valico, una rocca sulla cima e una torre di pietra sulla costa. "
                          "Spiaggia e porto a est, scogliera sul resto della costa. Territorio sintetico.")
    scn["name"] = "Isola di Roccavento"
    scn["region"] = "territorio sintetico (Scenario Factory, isola)"
    scn["tags"] = sorted(set(scn.get("tags", [])) | {"island"})
    (sdir / "scenario.json").write_text(json.dumps(scn, indent=2) + "\n")
    land = res["land"]
    params = dict(res["params"])
    params["elev_m"] = [round(float(fine.min()), 1), round(float(fine.max()), 1)]
    edge = (land ^ np.roll(land, 1, 0)) | (land ^ np.roll(land, 1, 1))
    params["island"] = {
        "script": "tools/factory/island.py", "land_km2": round(float(land.sum() * FM * FM / 1e6), 2),
        "coast_length_km": round(float(edge.sum() * FM / 1e3), 1),
        "coast_box_dimension": round(stats_fractal_dimension(land), 3),
        "fire_cells_water": int(sea20.sum()), "fire_cells_shore_nonburnable": int(nb.sum() - sea20.sum()),
        "params": json.loads(json.dumps(PARAMS)),
        "coast_road": res["road_stats"].get(next((r["id"] for r in res["roads"] if r["name"] == "Strada Costiera"), -1)),
    }
    (sdir / "params.json").write_text(json.dumps(params, indent=2) + "\n")
    return sdir


# --------------------------------------------------------------------------- a picture to judge it by

def figure(res, path: Path):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.colors import LightSource

    fine, L = res["fine"], res["L"]
    fig, axes = plt.subplots(1, 2, figsize=(17, 8.4))
    ext = (0, WORLD_M, 0, WORLD_M)
    for ax, (z, title) in zip(axes, [(res["old_fine"], "prima"), (fine, "isola")]):
        ls = LightSource(azdeg=300, altdeg=35)
        z2 = np.where(z <= SEA_RENDER_M, np.nan, z)
        rgb = ls.shade(np.nan_to_num(z, nan=0.0), cmap=plt.cm.terrain, vert_exag=2.0, blend_mode="overlay", vmin=-80, vmax=700)
        rgb[(z <= SEA_RENDER_M)] = (0.10, 0.28, 0.42, 1.0)
        ax.imshow(rgb, extent=ext)
        ax.set_title(title)
        ax.set_xlim(0, WORLD_M)
        ax.set_ylim(0, WORLD_M)
    ax = axes[1]
    for r in res["roads"]:
        xs, ys = zip(*r["line"])
        lw, col = (2.2, "#ffd34d") if r["name"] in ("Strada Costiera", "Molo del Porto") else (0.9, "white")
        ax.plot(xs, ys, color=col, lw=lw, alpha=0.9)
    b = np.array([q["centroid"] for q in res["osm"]["buildings"]])
    ax.plot(b[:, 0], b[:, 1], ".", color="#ff6a3d", ms=1.6)
    ign = np.array([c["ignition"] for c in res["game"]["cases"]])
    ax.plot(ign[:, 0], ign[:, 1], "x", color="red", ms=4)
    for s in res["stack"]:
        ax.plot(*s["pos"], "o", color="#cccccc", ms=3)
    path.parent.mkdir(parents=True, exist_ok=True)
    fig.tight_layout()
    fig.savefig(path, dpi=100)
    plt.close(fig)


def main():
    res = build()
    sdir = write(res)
    figure(res, DOCS / "isola_mappa.png")
    pi = json.loads((sdir / "params.json").read_text())["island"]
    print(f"isola: {pi['land_km2']} km2, costa {pi['coast_length_km']} km (dimensione box {pi['coast_box_dimension']}), "
          f"celle d'acqua {pi['fire_cells_water']} -> {sdir}")
    cr = pi.get("coast_road")
    if cr:
        print(f"strada costiera: {cr['length_m']} m, pendenza p95 {cr['grade_p95']}, sterro/riporto p95 {cr['cutfill_p95_m']} m (max {cr['cutfill_max_m']} m)")
    return res
