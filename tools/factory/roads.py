"""Roads that follow the orography: a least-cost path on the DEM with a grade
penalty, so a road climbs to a pass in bends instead of straight up the
fall line (02-SCENARIO-FACTORY §C: "le strade seguono l'orografia").

Also the junction helpers the routing graph needs: `RoadNetwork::build` welds
coincident vertices only, so a crossing without a shared vertex is no junction.
"""

from __future__ import annotations

import heapq
import math

import numpy as np

from .grid import CELL_M, N, smooth, to_cell, to_xy

# 16-neighbour moves: the knight moves let a road hold a gentle grade across a
# slope without a staircase of 45-degree zigzags.
_MOVES = [(dr, dc) for dr in range(-2, 3) for dc in range(-2, 3)
          if (dr, dc) != (0, 0) and math.gcd(abs(dr), abs(dc)) == 1]


def least_cost_path(dem: np.ndarray, a: tuple[float, float], b: tuple[float, float],
                    grade_ok: float = 0.09, grade_max: float = 0.14, avoid: np.ndarray | None = None,
                    margin_m: float = 900.0, coarse: int = 2, turn_m: float = 8.0,
                    hairpin_m: float = 60.0) -> list[tuple[float, float]]:
    """Dijkstra between two world points on a `coarse` x 20 m grid, over
    (cell, heading) states so that turning costs something. Cost per metre
    grows with the square of the grade above `grade_ok`; above `grade_max` a
    move is forbidden. A bend costs `turn_m` metres per radian squared and a
    reversal (> 90 degrees) a further `hairpin_m`: without the heading the
    cheapest way up a slope is a saw of 40-60 m zigzags, which a 20 m DEM
    hides and a 5 m one shows (docs/TODO.md, risoluzione fine). With it the
    road holds long legs across the slope and turns in a few real hairpins.
    `avoid` (bool raster on the fire grid) adds a strong penalty."""
    m = coarse
    n = N // m
    z = smooth(dem, 1.0 * m)[:n * m, :n * m].reshape(n, m, n, m).mean(axis=(1, 3))
    av = None if avoid is None else avoid[:n * m, :n * m].reshape(n, m, n, m).any(axis=(1, 3))
    step = CELL_M * m
    (ra, ca), (rb, cb) = [(min(n - 1, r // m), min(n - 1, c // m)) for r, c in (to_cell(*a), to_cell(*b))]
    pad = int(margin_m / step)
    r0, r1 = max(0, min(ra, rb) - pad), min(n - 1, max(ra, rb) + pad)
    c0, c1 = max(0, min(ca, cb) - pad), min(n - 1, max(ca, cb) + pad)
    nm = len(_MOVES)
    ang = [math.atan2(dr, dc) for dr, dc in _MOVES]
    tcost = [[0.0] * nm for _ in range(nm)]
    for i in range(nm):
        for j in range(nm):
            t = abs((ang[j] - ang[i] + math.pi) % (2 * math.pi) - math.pi)
            tcost[i][j] = turn_m * t * t + (hairpin_m if t > math.pi / 2 + 1e-6 else 0.0)
    # edge costs depend on the cell pair only: cache them
    dist: dict = {}
    prev: dict = {}
    pq = []
    for k in range(nm):
        dist[(ra, ca, k)] = 0.0
        pq.append((0.0, ra, ca, k))
    heapq.heapify(pq)
    goal = None
    while pq:
        d, r, c, k = heapq.heappop(pq)
        if (r, c) == (rb, cb):
            goal = (r, c, k)
            break
        if d > dist.get((r, c, k), math.inf):
            continue
        tk = tcost[k]
        for j, (dr, dc) in enumerate(_MOVES):
            nr, nc = r + dr, c + dc
            if not (r0 <= nr <= r1 and c0 <= nc <= c1):
                continue
            L = step * math.hypot(dr, dc)
            g = abs(z[nr, nc] - z[r, c]) / L
            if g > grade_max:
                continue
            cost = L * (1.0 + 25.0 * (max(0.0, g - grade_ok) / grade_ok) ** 2) + tk[j]
            if av is not None and av[nr, nc]:
                cost += 10 * L
            nd = d + cost
            key = (nr, nc, j)
            if nd < dist.get(key, math.inf):
                dist[key] = nd
                prev[key] = (r, c, k)
                heapq.heappush(pq, (nd, nr, nc, j))
    if goal is None:
        raise RuntimeError(f"no road within grade {grade_max} from {a} to {b}")
    cells = [goal]
    while cells[-1] in prev:
        cells.append(prev[cells[-1]])
    half = (m - 1) / 2
    pts = [to_xy(r * m + half, c * m + half) for r, c, _ in reversed(cells)]
    pts[0], pts[-1] = a, b
    return chaikin(simplify(pts, 6.0), 3)


def simplify(pts, tol):
    """Douglas-Peucker."""
    if len(pts) < 3:
        return list(pts)
    (ax, ay), (bx, by) = pts[0], pts[-1]
    L = math.hypot(bx - ax, by - ay) or 1e-9
    dmax, imax = 0.0, 0
    for i, (px, py) in enumerate(pts[1:-1], 1):
        d = abs((bx - ax) * (ay - py) - (ax - px) * (by - ay)) / L
        if d > dmax:
            dmax, imax = d, i
    if dmax <= tol:
        return [pts[0], pts[-1]]
    return simplify(pts[:imax + 1], tol)[:-1] + simplify(pts[imax:], tol)


def chaikin(pts, n):
    """Corner cutting, keeping both ends: bends instead of grid corners."""
    for _ in range(n):
        out = [pts[0]]
        for (ax, ay), (bx, by) in zip(pts[:-1], pts[1:]):
            out += [(0.75 * ax + 0.25 * bx, 0.75 * ay + 0.25 * by), (0.25 * ax + 0.75 * bx, 0.25 * ay + 0.75 * by)]
        out.append(pts[-1])
        pts = out
    return pts


def resample(pts, step):
    """Vertices at most `step` apart, so draping and link lengths stay sane."""
    out = [pts[0]]
    for (ax, ay), (bx, by) in zip(pts[:-1], pts[1:]):
        L = math.hypot(bx - ax, by - ay)
        n = max(1, int(math.ceil(L / step)))
        out += [(ax + (bx - ax) * k / n, ay + (by - ay) * k / n) for k in range(1, n + 1)]
    return out


def nearest_vertex(line, p):
    return min(line, key=lambda v: (v[0] - p[0]) ** 2 + (v[1] - p[1]) ** 2)


def elevation(dem: np.ndarray, x: float, y: float) -> float:
    """Bilinear DEM sample at a world point (cell centres on the 20 m grid)."""
    fc = x / CELL_M - 0.5
    fr = (N * CELL_M - y) / CELL_M - 0.5
    c0, r0 = int(np.clip(math.floor(fc), 0, N - 2)), int(np.clip(math.floor(fr), 0, N - 2))
    tc, tr = np.clip(fc - c0, 0, 1), np.clip(fr - r0, 0, 1)
    top = dem[r0, c0] * (1 - tc) + dem[r0, c0 + 1] * tc
    bot = dem[r0 + 1, c0] * (1 - tc) + dem[r0 + 1, c0 + 1] * tc
    return float(top * (1 - tr) + bot * tr)


def grade_stats(dem, line, step: float = 30.0):
    """95th-percentile grade along the road over `step` stretches, and length."""
    pts = resample(line, step)
    z = [elevation(dem, x, y) for x, y in pts]
    g = [abs(z[k + 1] - z[k]) / max(1e-6, math.hypot(pts[k + 1][0] - pts[k][0], pts[k + 1][1] - pts[k][1]))
         for k in range(len(pts) - 1)]
    length = sum(math.hypot(b[0] - a[0], b[1] - a[1]) for a, b in zip(line[:-1], line[1:]))
    return (float(np.percentile(g, 95)) if g else 0.0), float(length)


# --- junctions (from scripts/generate_demo_scenarios.py, same contract) ------

def crossings(roads: list[dict]) -> None:
    """Add a vertex on both roads wherever two segments properly cross."""
    def seg_x(p, q, r, s):
        d = (q[0] - p[0]) * (s[1] - r[1]) - (q[1] - p[1]) * (s[0] - r[0])
        if abs(d) < 1e-9:
            return None
        t = ((r[0] - p[0]) * (s[1] - r[1]) - (r[1] - p[1]) * (s[0] - r[0])) / d
        u = ((r[0] - p[0]) * (q[1] - p[1]) - (r[1] - p[1]) * (q[0] - p[0])) / d
        return (t, u) if 0 < t < 1 and 0 < u < 1 else None
    new = {id(r): [] for r in roads}
    for i, A in enumerate(roads):
        for B in roads[i + 1:]:
            if not (A["drivable"] and B["drivable"]):
                continue
            for ia, (p, q) in enumerate(zip(A["line"], A["line"][1:])):
                for ib, (r_, s_) in enumerate(zip(B["line"], B["line"][1:])):
                    h = seg_x(p, q, r_, s_)
                    if h:
                        pt = [round(p[0] + h[0] * (q[0] - p[0]), 2), round(p[1] + h[0] * (q[1] - p[1]), 2)]
                        new[id(A)].append((ia, h[0], pt))
                        new[id(B)].append((ib, h[1], pt))
    for r in roads:
        for idx, _, pt in sorted(new[id(r)], key=lambda x: (x[0], x[1]), reverse=True):
            r["line"].insert(idx + 1, pt)


def connect_junctions(roads: list[dict]) -> None:
    """Insert every other road's vertices that lie on a segment, so a T-junction
    is a node in the routing graph."""
    verts = [tuple(v) for r in roads for v in r["line"]]
    for r in roads:
        out = [r["line"][0]]
        for a, b in zip(r["line"], r["line"][1:]):
            L2 = (b[0] - a[0]) ** 2 + (b[1] - a[1]) ** 2
            hits = []
            for vx, vy in verts:
                if L2 == 0:
                    continue
                t = ((vx - a[0]) * (b[0] - a[0]) + (vy - a[1]) * (b[1] - a[1])) / L2
                if 0.001 < t < 0.999:
                    px, py = a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])
                    if (px - vx) ** 2 + (py - vy) ** 2 < 4.0:
                        hits.append((t, [float(vx), float(vy)]))
            out += [h for _, h in sorted(hits)]
            out.append(b)
        r["line"] = out
