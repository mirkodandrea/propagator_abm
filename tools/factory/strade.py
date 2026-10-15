"""Secondary roads: alternative routes for the island (2026-10-15).

Adds drivable ways (ids >= 200) to `osm.json` for the eastern plain, where the
fires are: a north bypass of the SP 12, a road across the plain south of the
ignition (access to the flank), a link from Fornaci to Le Terrazze and a road
from San Rocco to the Costiera.  Links over the massif (Le Ghiande, Grilli)
were tried and do not exist: no way over the ridge under a 20 % grade. Each way is routed on the DEM at least cost
(slope squared, no sea, a grade ceiling), then its ends are welded to the
nearest existing vertex, because `abm::network` joins ways only where vertices
coincide.

The roads are NOT fuel breaks: `fuel.i32` is not touched, so fire crosses them
like any other macchia.  The danger on them comes from `fire::threat`.

Run again it gives the same file (ways with id >= 200 are rebuilt from scratch).
Stand-alone (numpy only):

    python3 -I tools/factory/strade.py [--out <dir>] [--preview map.json]

By default it rewrites `osm.json` of the main territory in place (applied
2026-10-15). With `--out <dir>` the file goes to another directory, whose other
files become symbolic links to `rocca_ventosa` (a trial: `ROCCA_SCENARIO=<id>`).
"""
import argparse
import heapq
import json
import os
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "data/scenarios/rocca_ventosa"
W = 8000.0
STEP = 40.0                      # routing grid, m
MAX_GRADE = 0.13                 # a lorry road
FIRST_ID = 200

# name, class, start (x, y), via points, end (x, y), grade ceiling.  Ends snap to existing vertices.
WAYS = [
    ("Variante Nord di Castelvento", "tertiary", (5000, 5330), [(5800, 5500)], (6150, 4330), MAX_GRADE),
    ("Strada dei Campi", "unclassified", (4640, 5420), [], (4800, 4900), MAX_GRADE),
    # Ends where it first meets the Costiera: aimed further west it ran 1.5 km beside it.
    ("Strada di San Rocco al Mare", "tertiary", (6250, 4200), [], (7043, 2552), MAX_GRADE),
    ("Strada del Piano", "unclassified", (4800, 4850), [(5600, 4080)], (6200, 4150), 0.18),
]


def load_dem(n_hint=None):
    dem = np.fromfile(SRC / "dem.f64", dtype="<f8")
    n = int(round(len(dem) ** 0.5))
    return dem.reshape(n, n)


def route(dem, a, b, via, max_grade=MAX_GRADE):
    n = dem.shape[0]
    cell = W / n
    g = int(W / STEP)

    def elev(i, j):  # i = row from the north, j = column
        r = min(n - 1, int(i * STEP / cell))
        c = min(n - 1, int(j * STEP / cell))
        return dem[r, c]

    def node(p):
        return (g - 1 - min(g - 1, int(p[1] / STEP)), min(g - 1, int(p[0] / STEP)))

    def leg(s, t):
        dist = {s: 0.0}
        prev = {}
        pq = [(0.0, s)]
        while pq:
            d, u = heapq.heappop(pq)
            if u == t:
                break
            if d > dist[u]:
                continue
            for di in (-1, 0, 1):
                for dj in (-1, 0, 1):
                    if not (di or dj):
                        continue
                    v = (u[0] + di, u[1] + dj)
                    if not (0 <= v[0] < g and 0 <= v[1] < g):
                        continue
                    ev = elev(*v)
                    if ev < 4.0:
                        continue
                    ln = STEP * (1.414 if di and dj else 1.0)
                    grade = abs(ev - elev(*u)) / ln
                    if grade > max_grade:
                        continue
                    nd = d + ln * (1 + (grade / 0.06) ** 2)
                    if nd < dist.get(v, 1e18):
                        dist[v] = nd
                        prev[v] = u
                        heapq.heappush(pq, (nd, v))
        if t not in prev and s != t:
            raise RuntimeError(f"no road between {s} and {t} under the {max_grade} grade ceiling")
        out = [t]
        while out[-1] != s:
            out.append(prev[out[-1]])
        return out[::-1]

    stops = [node(a)] + [node(v) for v in via] + [node(b)]
    cells = []
    for s, t in zip(stops, stops[1:]):
        cells += leg(s, t)[(1 if cells else 0):]
    return [((j + 0.5) * STEP, (g - 1 - i + 0.5) * STEP) for i, j in cells]


def smooth(pts, passes=4):
    p = np.array(pts, dtype=float)
    for _ in range(passes):
        q = p.copy()
        q[1:-1] = 0.25 * p[:-2] + 0.5 * p[1:-1] + 0.25 * p[2:]
        p = q
    return p


def resample(p, every=25.0):
    seg = np.hypot(*np.diff(p, axis=0).T)
    s = np.concatenate([[0], np.cumsum(seg)])
    t = np.arange(0, s[-1], every)
    t = np.append(t, s[-1])
    return np.stack([np.interp(t, s, p[:, 0]), np.interp(t, s, p[:, 1])], axis=1)


def crossing(a, b, c, d):
    """Where segment ab crosses segment cd, as (point, t on ab, u on cd), or None.
    A crossing at the very ends of ab is not one (the way is welded there);
    one at an end of cd is, so a junction already in the old way is reused."""
    den = (b[0] - a[0]) * (d[1] - c[1]) - (b[1] - a[1]) * (d[0] - c[0])
    if abs(den) < 1e-9:
        return None
    t = ((c[0] - a[0]) * (d[1] - c[1]) - (c[1] - a[1]) * (d[0] - c[0])) / den
    u = ((c[0] - a[0]) * (b[1] - a[1]) - (c[1] - a[1]) * (b[0] - a[0])) / den
    if not (1e-6 < t < 1 - 1e-6 and -1e-3 <= u <= 1 + 1e-3):
        return None
    p = [round(a[0] + t * (b[0] - a[0]), 2), round(a[1] + t * (b[1] - a[1]), 2)]
    # Within half a metre of an old vertex: that vertex, so a second run adds nothing.
    for end, ue in ((c, 0.0), (d, 1.0)):
        if np.hypot(p[0] - end[0], p[1] - end[1]) < 0.5:
            return list(end), t, ue
    if not 0 < u < 1:
        return None
    return p, t, u


def stop_at_first_meeting(pts, end, roads):
    """Cut the way where it first crosses the road it ends on: the routed line
    can cross it and come back, and that last stretch only doubles the road."""
    target = [r for r in roads if r["drivable"] and any(v[0] == end[0] and v[1] == end[1] for v in r["line"])]
    for i in range(len(pts) - 1):
        for r in target:
            ol = r["line"]
            for k in range(len(ol) - 1):
                hit = crossing(pts[i], pts[i + 1], ol[k], ol[k + 1])
                if hit:
                    if 0 < hit[2] < 1:
                        ol.insert(k + 1, list(hit[0]))
                    return np.vstack([pts[: i + 1], [hit[0]]])
    return pts


def weld_crossings(way, others):
    """Put a shared vertex wherever `way` crosses a drivable way of `others`:
    `abm::network` joins ways only where vertices coincide, so a crossing
    without one is a bridge, and an engine cannot turn there."""
    welded = 0
    changed = True
    while changed:
        changed = False
        line = way["line"]
        for i in range(len(line) - 1):
            for o in others:
                if not o["drivable"] or o is way:
                    continue
                ol = o["line"]
                for k in range(len(ol) - 1):
                    hit = crossing(line[i], line[i + 1], ol[k], ol[k + 1])
                    if not hit:
                        continue
                    p, _, u = hit
                    line.insert(i + 1, p)
                    if 0 < u < 1:
                        ol.insert(k + 1, list(p))
                    print(f"  incrocio con {o.get('name')} a ({p[0]:.0f}, {p[1]:.0f})")
                    welded += 1
                    changed = True
                    break
                if changed:
                    break
            if changed:
                break
    return welded


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(SRC))
    ap.add_argument("--preview", help="also write the new ways as JSON for a map")
    a = ap.parse_args()

    osm = json.loads((SRC / "osm.json").read_text())
    roads = [r for r in osm["roads"] if r["id"] < FIRST_ID]
    dem = load_dem()

    verts = np.array([v for r in roads if r["drivable"] for v in r["line"]])

    def snap(p):
        d = np.hypot(verts[:, 0] - p[0], verts[:, 1] - p[1])
        return verts[int(d.argmin())]

    new = []
    for name, cls, s, via, e, grade in WAYS:
        s, e = snap(s), snap(e)
        try:
            pts = smooth(route(dem, tuple(s), tuple(e), via, grade))
        except RuntimeError as err:
            print(f"{name}: SALTATA ({err})")
            continue
        pts = resample(pts)
        pts[0], pts[-1] = s, e                       # weld to the existing vertex exactly
        pts = stop_at_first_meeting(pts, e, roads)
        e = pts[-1]
        length = float(np.hypot(*np.diff(pts, axis=0).T).sum())
        print(f"{name}: {length:5.0f} m, da ({s[0]:.0f}, {s[1]:.0f}) a ({e[0]:.0f}, {e[1]:.0f})")
        new.append({
            "id": FIRST_ID + len(new), "class": cls, "drivable": True, "track": False,
            "name": name, "oneway": False,
            "line": [[round(float(x), 2), round(float(y), 2)] for x, y in pts],
        })
        weld_crossings(new[-1], roads + new[:-1])

    osm["roads"] = roads + new
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    for f in SRC.iterdir():
        if f.name != "osm.json" and not (out / f.name).exists():
            os.symlink(os.path.relpath(f, out), out / f.name)
    (out / "osm.json").write_text(json.dumps(osm, indent=2, ensure_ascii=False) + "\n")
    if a.preview:
        Path(a.preview).write_text(json.dumps(new))


if __name__ == "__main__":
    main()
