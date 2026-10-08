"""Fine resolution: the 5 m terrain that roads, houses, agents and the
renderer sit on (decided 2026-10-08, docs/TODO.md).

**The fire never sees any of it.** The propagator keeps the approved 20 m DEM
and the 20 m fuel unchanged (user, 2026-10-08: the fine model is for graphics
and agents only). The fine surface is the 20 m one upsampled, with micro
relief, road benches cut to a drivable grade and flat pads under the houses,
plus a 5 m land-cover raster (road, building, paved, irrigated, water,
natural). Its cell average is kept only as a check that the two surfaces
still agree (`fine_vs_fire_dem`).

Fuel stays an explicit 20 m decision (town.build): a lane under the canopy is
a 5 m strip, not a firebreak, and only the declared breaks (provincial road
with verges, built cores, irrigated plots) are non-burnable cells. The cover
raster lets a check count how much of each 20 m cell is really built.

Frame: nodes at x = j * FINE_M, y = WORLD_M - i * FINE_M (row 0 north), which
is how `scenario::Terrain::height_at` reads `render_terrain.f32`. Cover cells
are the FINE_M squares between nodes, row 0 north.
"""

from __future__ import annotations

import math

import numpy as np
from matplotlib.path import Path as MplPath
from scipy.ndimage import map_coordinates
from scipy.spatial import cKDTree

from . import roads as R
from .grid import CELL_M, N, WORLD_M, smoothstep

FINE_M = 5.0
K = int(round(CELL_M / FINE_M))          # fine steps per fire cell
NF = int(round(WORLD_M / FINE_M)) + 1    # nodes per edge: 1601
NC = NF - 1                              # cover cells per edge: 1600

# Land cover at 5 m.
NATURAL, ROAD, TRACK, BUILDING, PAVED, IRRIGATED, WATER = range(7)
COVER_NAMES = ["naturale", "strada", "pista", "edificio", "piazzale", "orto irriguo", "acqua"]

# Per road class: carriageway half-width, verge (m) and the design grade limit.
ROAD_SPEC = {
    "secondary":    {"half_w": 3.5, "verge": 1.5, "grade": 0.10},
    "tertiary":     {"half_w": 2.75, "verge": 1.0, "grade": 0.14},
    "residential":  {"half_w": 3.0, "verge": 0.5, "grade": 0.12},
    "unclassified": {"half_w": 2.25, "verge": 0.5, "grade": 0.18},
    "track":        {"half_w": 1.75, "verge": 0.5, "grade": 0.22},
}
ORDER = ["secondary", "tertiary", "residential", "unclassified", "track"]
BATTER = 0.7      # max cut/fill face slope (rise/run) beside a bench
# Half-width of the level platform. The kiosk draws roads as map symbols
# (crates/game/src/roads.rs: 4 m half-width + 1.6 m casing, tracks 1.6 + 1.6),
# wider than the carriageway; a bench narrower than the ribbon lets the cut
# face poke through its edges and they render ragged.
BENCH_HALF_M = {"drivable": 6.0, "track": 3.5}
PAD_MARGIN_M = 3.0
OPEN_KINDS = {"plaza", "assembly", "pitch", "parking"}  # landmarks that are open ground, not roofs

FINE = {"micro_relief_m": 0.8, "micro_relief_slope_m": 1.6, "micro_seed": 4401, "profile_sigma_m": 30.0,
        "station_m": 2.5, "chunk_m": 60.0}


# --------------------------------------------------------------------------- grids

def node_xy():
    c = np.arange(NF) * FINE_M
    return np.meshgrid(c, WORLD_M - c)


def upsample(dem20: np.ndarray) -> np.ndarray:
    """Cubic spline through the 20 m cell centres, sampled at the fine nodes."""
    f = np.arange(NF) * FINE_M / CELL_M - 0.5
    rr, cc = np.meshgrid(f, f, indexing="ij")
    return map_coordinates(dem20, [rr, cc], order=3, mode="nearest")


def micro_relief(rng, shape, scale_min_m=8.0, scale_max_m=160.0) -> np.ndarray:
    """Band-limited noise (8-160 m), unit std: the hummocks and terracing a
    20 m DEM cannot carry. Zero mean, so block averages barely move."""
    m = shape[0] + 64
    ky = np.fft.fftfreq(m, FINE_M)[:, None]
    kx = np.fft.rfftfreq(m, FINE_M)[None, :]
    k = np.sqrt(kx ** 2 + ky ** 2)
    k[0, 0] = 1.0
    amp = k ** -1.4 * np.exp(-(k * scale_min_m) ** 2) * (1 - np.exp(-(k * scale_max_m) ** 2))
    amp[0, 0] = 0.0
    f = np.fft.irfft2(np.fft.rfft2(rng.normal(size=(m, m))) * amp, s=(m, m))[:shape[0], :shape[1]]
    return (f - f.mean()) / f.std()


def to_fire_dem(fine: np.ndarray) -> np.ndarray:
    """Cell mean of the fine surface: trapezoid weights over the K+1 nodes on
    each side of a 20 m cell, so the mean is centred on the cell centre."""
    w = np.ones(K + 1)
    w[[0, -1]] = 0.5
    w /= w.sum()
    out = np.zeros((N, N))
    for a in range(K + 1):
        for b in range(K + 1):
            out += w[a] * w[b] * fine[a:a + K * N:K, b:b + K * N:K]
    return out


def sample(fine: np.ndarray, pts) -> np.ndarray:
    p = np.asarray(pts, dtype=float)
    return map_coordinates(fine, [(WORLD_M - p[:, 1]) / FINE_M, p[:, 0] / FINE_M], order=1, mode="nearest")


def _gauss1d(z, sigma):
    if sigma <= 0 or len(z) < 3:
        return z
    k = int(3 * sigma)
    t = np.arange(-k, k + 1)
    g = np.exp(-0.5 * (t / sigma) ** 2)
    g /= g.sum()
    zp = np.pad(z, k, mode="edge")
    return np.convolve(zp, g, mode="valid")


# --------------------------------------------------------------------------- road profiles

def design_profile(z_ground, s, grade, pins):
    """A drivable longitudinal profile: the ground smoothed over ~profile_sigma,
    then held under `grade` by averaging a forward- and a backward-clamped
    profile (each feasible; the constraint set is convex, so their mean is
    too). Junction pins are blended in over a run long enough to keep the
    added grade under 3 %."""
    ds = np.diff(s)
    z = _gauss1d(z_ground, FINE["profile_sigma_m"] / FINE["station_m"])
    zf = z.copy()
    for i in range(1, len(z)):
        zf[i] = np.clip(zf[i], zf[i - 1] - grade * ds[i - 1], zf[i - 1] + grade * ds[i - 1])
    zb = z.copy()
    for i in range(len(z) - 2, -1, -1):
        zb[i] = np.clip(zb[i], zb[i + 1] - grade * ds[i], zb[i + 1] + grade * ds[i])
    zp = 0.5 * (zf + zb)
    for sp, zpin in pins:
        d = zpin - np.interp(sp, s, zp)
        run = abs(d) / 0.03 + 15.0
        zp = zp + d * np.clip(1 - np.abs(s - sp) / run, 0, 1)
    return zp


class Accum:
    """Weighted blend of several target surfaces into the base: each feature
    contributes (weight, target height); overlapping features average, which
    keeps hairpins and junctions continuous."""

    def __init__(self, base):
        self.base = base
        self.sw = np.zeros_like(base)
        self.swz = np.zeros_like(base)
        self.wmax = np.zeros_like(base)

    def add(self, r0, c0, w, z):
        sl = (slice(r0, r0 + w.shape[0]), slice(c0, c0 + w.shape[1]))
        self.sw[sl] += w
        self.swz[sl] += w * z
        self.wmax[sl] = np.maximum(self.wmax[sl], w)

    def apply(self):
        z = np.where(self.sw > 0, self.swz / np.maximum(self.sw, 1e-9), self.base)
        return self.base * (1 - self.wmax) + z * self.wmax


def _window(x0, y0, x1, y1, margin):
    c0 = max(0, int(math.floor((x0 - margin) / FINE_M)))
    c1 = min(NF - 1, int(math.ceil((x1 + margin) / FINE_M)))
    r0 = max(0, int(math.floor((WORLD_M - y1 - margin) / FINE_M)))
    r1 = min(NF - 1, int(math.ceil((WORLD_M - y0 + margin) / FINE_M)))
    cs = np.arange(c0, c1 + 1) * FINE_M
    rs = WORLD_M - np.arange(r0, r1 + 1) * FINE_M
    X, Y = np.meshgrid(cs, rs)
    return r0, c0, X, Y


def carve_roads(fine, roads):
    """Bench every road into the fine surface. Returns the new surface and, per
    road id, the stats of its profile (length, grade p95, cut/fill p95/max)."""
    acc = Accum(fine)
    pinned: dict = {}
    stats = {}
    by_class = sorted(roads, key=lambda r: ORDER.index(r["class"]) if r["class"] in ORDER else 99)
    for road in by_class:
        spec = ROAD_SPEC.get(road["class"], ROAD_SPEC["unclassified"])
        line = [tuple(p) for p in road["line"]]
        pts = np.array(R.resample(line, FINE["station_m"]))
        seg = np.hypot(*np.diff(pts, axis=0).T)
        s = np.concatenate([[0], np.cumsum(seg)])
        zg = sample(fine, pts)
        # pins: vertices already profiled by a higher-ranked road (junctions)
        pins = []
        for v in line:
            key = (round(v[0], 1), round(v[1], 1))
            if key in pinned:
                i = int(np.argmin(np.hypot(pts[:, 0] - v[0], pts[:, 1] - v[1])))
                pins.append((s[i], pinned[key]))
        zp = design_profile(zg, s, spec["grade"], pins)
        for v in line:
            key = (round(v[0], 1), round(v[1], 1))
            if key not in pinned:
                i = int(np.argmin(np.hypot(pts[:, 0] - v[0], pts[:, 1] - v[1])))
                pinned[key] = float(zp[i])
        cut = zp - zg
        g = np.abs(np.diff(zp)) / np.maximum(seg, 1e-6)
        stats[road["id"]] = {"name": road["name"], "class": road["class"], "length_m": round(float(s[-1])),
                             "grade_p95": round(float(np.percentile(g, 95)), 3) if len(g) else 0.0,
                             "ground_grade_p95": round(float(np.percentile(
                                 np.abs(np.diff(zg)) / np.maximum(seg, 1e-6), 95)), 3) if len(g) else 0.0,
                             "cutfill_p95_m": round(float(np.percentile(np.abs(cut), 95)), 2),
                             "cutfill_max_m": round(float(np.abs(cut).max()), 2)}
        flat = max(spec["half_w"] + spec["verge"],
                   BENCH_HALF_M["track" if road.get("track") and not road["drivable"] else "drivable"])
        # chunks of stations, each blended with its nearest-station target
        step = int(FINE["chunk_m"] / FINE["station_m"])
        for a in range(0, len(pts), step):
            b = min(len(pts), a + step + 1)
            P, Z = pts[a:b], zp[a:b]
            dmax = float(np.abs(cut[a:b]).max())
            reach = flat + 2.0 + dmax / BATTER
            r0, c0, X, Y = _window(P[:, 0].min(), P[:, 1].min(), P[:, 0].max(), P[:, 1].max(), reach + FINE_M)
            d, idx = cKDTree(P).query(np.c_[X.ravel(), Y.ravel()])
            d, idx = d.reshape(X.shape), idx.reshape(X.shape)
            zt = Z[idx]
            base = fine[r0:r0 + X.shape[0], c0:c0 + X.shape[1]]
            # the bench is flat to the verge, then a batter face to the ground
            face = flat + 1.0 + np.abs(zt - base) / BATTER
            w = 1.0 - smoothstep(flat, face, d)
            # chunks share an end station, so neighbours agree where they overlap
            acc.add(r0, c0, w, zt)
    return acc.apply(), stats


def pad_buildings(fine, buildings):
    """A level pad under each footprint (+margin), blended out over a short
    batter: houses on a slope sit on a terrace, not on a tilted plane."""
    acc = Accum(fine)
    for b in buildings:
        if b.get("kind") in OPEN_KINDS:
            continue
        ring = np.array(b["ring"])
        x0, y0 = ring.min(0)
        x1, y1 = ring.max(0)
        r0, c0, X, Y = _window(x0, y0, x1, y1, 15.0)
        P = np.c_[X.ravel(), Y.ravel()]
        inside = MplPath(ring).contains_points(P).reshape(X.shape)
        base = fine[r0:r0 + X.shape[0], c0:c0 + X.shape[1]]
        zpad = float(base[inside].mean()) if inside.any() else float(sample(fine, [b["centroid"]])[0])
        # distance outside the footprint, approximated by distance to its vertices/edges
        d = _dist_to_ring(ring, X, Y)
        d[inside] = 0.0
        face = PAD_MARGIN_M + 1.0 + np.abs(zpad - base) / BATTER
        w = 1.0 - smoothstep(PAD_MARGIN_M, face, d)
        acc.add(r0, c0, w, np.full_like(base, zpad))
    return acc.apply()


def _dist_to_ring(ring, X, Y):
    d = np.full(X.shape, np.inf)
    for (ax, ay), (bx, by) in zip(ring, np.roll(ring, -1, axis=0)):
        vx, vy = bx - ax, by - ay
        t = np.clip(((X - ax) * vx + (Y - ay) * vy) / max(vx * vx + vy * vy, 1e-9), 0, 1)
        d = np.minimum(d, np.hypot(X - ax - t * vx, Y - ay - t * vy))
    return d


# --------------------------------------------------------------------------- land cover

def cover_raster(out: dict) -> np.ndarray:
    """5 m land cover from the same vectors the scenario ships."""
    cov = np.zeros((NC, NC), dtype=np.uint8)
    cc = (np.arange(NC) + 0.5) * FINE_M
    CX, CY = np.meshgrid(cc, WORLD_M - cc)

    def poly(ring, value):
        ring = np.asarray(ring, dtype=float)
        (x0, y0), (x1, y1) = ring.min(0), ring.max(0)
        c0, c1 = max(0, int(x0 // FINE_M)), min(NC, int(x1 // FINE_M) + 1)
        r0, r1 = max(0, int((WORLD_M - y1) // FINE_M)), min(NC, int((WORLD_M - y0) // FINE_M) + 1)
        sub = (slice(r0, r1), slice(c0, c1))
        P = np.c_[CX[sub].ravel(), CY[sub].ravel()]
        m = MplPath(ring).contains_points(P).reshape(CX[sub].shape)
        cov[sub][m] = value

    for a in out.get("areas", []):
        poly(a["ring"], IRRIGATED if a["kind"] == "irrigated" else PAVED)
    for b in out["buildings"]:
        poly(b["ring"], PAVED if b.get("kind") in OPEN_KINDS else BUILDING)
    for road in sorted(out["roads"], key=lambda r: -ORDER.index(r["class"]) if r["class"] in ORDER else 0):
        spec = ROAD_SPEC.get(road["class"], ROAD_SPEC["unclassified"])
        pts = np.array(R.resample([tuple(p) for p in road["line"]], FINE_M / 2))
        x0, y0 = pts.min(0)
        x1, y1 = pts.max(0)
        hw = spec["half_w"]
        c0, c1 = max(0, int((x0 - hw) // FINE_M)), min(NC, int((x1 + hw) // FINE_M) + 1)
        r0, r1 = max(0, int((WORLD_M - y1 - hw) // FINE_M)), min(NC, int((WORLD_M - y0 + hw) // FINE_M) + 1)
        sub = (slice(r0, r1), slice(c0, c1))
        d, _ = cKDTree(pts).query(np.c_[CX[sub].ravel(), CY[sub].ravel()])
        m = d.reshape(CX[sub].shape) <= hw
        keep = cov[sub] == BUILDING
        cov[sub][m & ~keep] = TRACK if road.get("track") and not road["drivable"] else ROAD
    for w in out["water"]:
        if w["kind"] == "open_water":
            x, y = w["pos"]
            poly([(x + 12 * math.cos(t), y + 9 * math.sin(t)) for t in np.linspace(0, 2 * math.pi, 24)], WATER)
    return cov


def cover_vs_fuel(cov: np.ndarray, fuel: np.ndarray) -> dict:
    """How the 20 m fuel decisions compare with what the 5 m cover says is
    really built: the share of each fire cell that is not vegetation."""
    built = (cov != NATURAL).reshape(N, K, N, K).mean(axis=(1, 3))
    nb = fuel == 0
    return {
        "fire_cells_nonburnable": int(nb.sum()),
        "fine_built_ha": round(float((cov != NATURAL).sum() * FINE_M ** 2 / 1e4), 1),
        "fire_nonburnable_ha": round(float(nb.sum() * CELL_M ** 2 / 1e4), 1),
        "nonburnable_cells_mostly_vegetated": int((nb & (built < 0.5)).sum()),
        "burnable_cells_mostly_built": int((~nb & (built >= 0.5)).sum()),
        "cover_ha": {COVER_NAMES[k]: round(float((cov == k).sum() * FINE_M ** 2 / 1e4), 1) for k in range(len(COVER_NAMES))},
        "built_share": built,
    }


# --------------------------------------------------------------------------- the whole step

def build(dem20: np.ndarray, out: dict, seed: int) -> dict:
    """dem20 is the approved 20 m terrain the town was laid out on."""
    rng = np.random.default_rng(FINE["micro_seed"] + seed)
    base = upsample(dem20)
    gy, gx = np.gradient(base, FINE_M)
    slope = np.hypot(gx, gy)
    relief = micro_relief(rng, base.shape)
    natural = base + (FINE["micro_relief_m"] + FINE["micro_relief_slope_m"] * np.clip(slope / 0.5, 0, 1)) * relief
    benched, road_stats = carve_roads(natural, out["roads"])
    fine = pad_buildings(benched, out["buildings"])
    fine = np.maximum(fine, 40.0)
    avg = to_fire_dem(fine)
    cov = cover_raster(out)
    return {"fine": fine, "natural": natural, "cover": cov, "road_stats": road_stats,
            "params": {"fine_m": FINE_M, "nodes": NF, **FINE, "road_spec": ROAD_SPEC, "batter": BATTER, "bench_half_m": BENCH_HALF_M,
                       # the 5 m surface averaged per fire cell vs the fire DEM: a check, not an input
                       "fine_vs_fire_dem": {
                           "mean_abs_m": round(float(np.abs(avg - dem20).mean()), 3),
                           "p99_abs_m": round(float(np.percentile(np.abs(avg - dem20), 99)), 2),
                           "max_abs_m": round(float(np.abs(avg - dem20).max()), 2)}}}
