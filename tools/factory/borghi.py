"""Village layout: houses aligned to the streets, clear of them and of each other (2026-10-16).

The houses of the six villages were scattered at random angles. The kiosk draws
every footprint at `visual_scale::BUILDING_FOOTPRINT` (1.9x), so a house on a
ring of 12 x 9 m stands 23 x 17 m on screen: houses overlapped each other and
the streets. This script re-lays them out on the rendered size:

* each house is turned so that its long side runs along the nearest street;
* it is pushed out of the street (surface + casing, plus a margin);
* it is pushed away from its neighbours and from the public buildings;
* it moves as little as it can (at most `MAX_MOVE` m from where it was).

A house must stand on ground with no fuel (`abm::bin::scenario_check`). Most
villages stand on a small cleared patch, so a house that leaves it gets its
cell cleared in `fuel.i32` (fuel 0), and nothing else in the grid changes.

It rewrites, consistently, the ring and centroid in `osm.json`, position and
cell of dwellings and households in `population.json`, and those cells in
`fuel.i32`. Roads, people and stations stay as they are. Deterministic (no random numbers);
run again on its own output it changes nothing.

Stand-alone (numpy only):

    python3 -I tools/factory/borghi.py [--check]

`--check` writes nothing and prints the violations that remain.
"""
import argparse
import json
import math
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "data/scenarios/rocca_ventosa"
CELL = 20.0
WORLD_H = 8000.0

TOY = 1.9                      # visual_scale::BUILDING_FOOTPRINT
ROAD_HALF = 4.0 + 1.6          # drivable half-width + casing (roads.rs)
TRACK_HALF = 1.6 + 1.6
STREET_MARGIN = 2.0            # clear ground between a wall and the casing
HOUSE_GAP = 2.5                # between two walls
MAX_MOVE = 60.0
SNAP_RANGE = 70.0              # a house farther than this from any street keeps its angle
RESIDENTIAL = {"house", "villa", "terrace", "apartments", "farm", "shop"}
FLAT = {"plaza", "assembly", "pitch", "parking", "cemetery"}   # ground, not walls
VILLAGES = ["Castelvento", "Fornaci", "Le Terrazze", "San Rocco", "Pian dei Grilli", "Le Ghiande"]


def rect_of(ring):
    r = np.array(ring[:4], float)
    e1, e2 = r[1] - r[0], r[2] - r[1]
    return r.mean(0), float(np.hypot(*e1)), float(np.hypot(*e2)), math.atan2(e1[1], e1[0])


def corners(c, w, h, th):
    u = np.array([math.cos(th), math.sin(th)])
    v = np.array([-u[1], u[0]])
    return np.array([c - u * w / 2 - v * h / 2, c + u * w / 2 - v * h / 2,
                     c + u * w / 2 + v * h / 2, c - u * w / 2 + v * h / 2])


def sdf_box(p, c, w, h, th):
    """Signed distance from points p (n,2) to a rotated box; negative inside."""
    u = np.array([math.cos(th), math.sin(th)])
    v = np.array([-u[1], u[0]])
    d = p - c
    q = np.stack([np.abs(d @ u) - w / 2, np.abs(d @ v) - h / 2], 1)
    return np.hypot(*np.maximum(q, 0).T) + np.minimum(np.maximum(q[:, 0], q[:, 1]), 0)


def mtv(a, b):
    """Minimum translation to push convex quad a out of quad b (None if apart)."""
    best = None
    for P in (a, b):
        for i in range(4):
            e = P[(i + 1) % 4] - P[i]
            n = np.array([-e[1], e[0]])
            n /= np.hypot(*n)
            pa, pb = a @ n, b @ n
            o = min(pa.max() - pb.min(), pb.max() - pa.min())
            if o <= 0:
                return None
            if best is None or o < best[0]:
                s = 1.0 if pa.mean() >= pb.mean() else -1.0
                best = (o, n * s)
    return best[1] * best[0]


def sample_roads(roads):
    pts, tan, half = [], [], []
    for r in roads:
        L = np.array(r["line"], float)
        hw = ROAD_HALF if r["drivable"] else TRACK_HALF
        for i in range(len(L) - 1):
            a, b = L[i], L[i + 1]
            n = max(1, int(np.hypot(*(b - a)) // 2.0))
            t = (b - a) / max(np.hypot(*(b - a)), 1e-9)
            for k in range(n + 1):
                pts.append(a + (b - a) * k / n)
                tan.append(math.atan2(t[1], t[0]))
                half.append(hw)
    return np.array(pts), np.array(tan), np.array(half)


STREET_NAMES = (" - via ", "Podere ", "Strada dei Campi", "SP 12", "SP 9", "Strada del Monte")
MAX_TURN = 18.0      # degrees at a vertex; the hairpins of the mountain roads are left alone
WELD_M = 12.0        # a free street end this near another road is a near miss: joined to it


def _key(p):
    return (round(float(p[0]), 1), round(float(p[1]), 1))


def _turn(a, b, c):
    u, v = b - a, c - b
    return abs(math.degrees(math.atan2(u[0] * v[1] - u[1] * v[0], u @ v)))


def unstar(roads, min_spokes=4):
    """Hamlet access ways drawn as a star (one way per house from one point).

    The ways of such a hub become a tree: the house nearest the hub is joined
    to it, then each other house to the nearest point of the tree built so far
    (a vertex is inserted there, so the network joins). Lanes are shared and
    the houses hang off them as short spurs. Returns the hubs rebuilt."""
    hubs = {}
    for r in roads:
        if "Podere" in r["name"] and r["drivable"]:
            hubs.setdefault(_key(r["line"][0]), []).append((r, 0))
            hubs.setdefault(_key(r["line"][-1]), []).append((r, -1))
    done = 0
    for hub, spokes in hubs.items():
        if len(spokes) < min_spokes:
            continue
        H = np.array(hub, float)
        todo = []
        for r, end in spokes:
            far = np.array(r["line"][-1 if end == 0 else 0], float)
            todo.append((r, far))
        lines = []          # the tree: one polyline per road, as lists of points
        placed = []
        todo.sort(key=lambda t: float(np.hypot(*(t[1] - H))))
        r0, e0 = todo.pop(0)
        r0["line"] = [[*H], [*map(float, e0)]]
        lines.append(r0["line"])
        placed.append(r0)
        while todo:
            best = None
            for k, (r, e) in enumerate(todo):
                for L in lines:
                    for i in range(len(L) - 1):
                        a, b = np.array(L[i], float), np.array(L[i + 1], float)
                        ab = b - a
                        t = float(np.clip((e - a) @ ab / max(ab @ ab, 1e-9), 0, 1))
                        q = a + t * ab
                        d = float(np.hypot(*(e - q)))
                        if best is None or d < best[0]:
                            best = (d, k, L, i, t, q)
            _, k, L, i, t, q = best
            r, e = todo.pop(k)
            a, b = np.array(L[i], float), np.array(L[i + 1], float)
            if np.hypot(*(q - a)) < 8:
                q = a
            elif np.hypot(*(q - b)) < 8:
                q = b
            else:
                L.insert(i + 1, [float(q[0]), float(q[1])])
            r["line"] = [[float(q[0]), float(q[1])], [float(e[0]), float(e[1])]]
            lines.append(r["line"])
        done += 1
    return done


def unfan(roads, spacing=32.0, min_ways=4):
    """Village streets fanning out from one point (a star junction).

    The longest of them is the trunk: it is prolonged past the hub along the
    mean direction of the others, and each of them leaves the prolongation
    at its own point instead of at the hub. Returns the hubs rebuilt."""
    ends = {}
    for r in roads:
        if r["drivable"] and " - via " in r["name"]:
            for end in (0, -1):
                ends.setdefault(_key(r["line"][end]), []).append((r, end))
    done = 0
    for hub, ways in ends.items():
        if len(ways) < min_ways:
            continue
        H = np.array(hub, float)

        def length(r):
            L = np.array(r["line"], float)
            return float(np.hypot(*(L[1:] - L[:-1]).T).sum())

        ways.sort(key=lambda w: -length(w[0]))
        trunk, tend = ways[0]
        branches = ways[1:]
        dirs = []
        for r, end in branches:
            L = np.array(r["line"] if end == 0 else r["line"][::-1], float)
            v = L[min(2, len(L) - 1)] - H
            dirs.append(v / max(np.hypot(*v), 1e-9))
        d = np.mean(dirs, 0)
        d /= max(np.hypot(*d), 1e-9)
        nrm = np.array([-d[1], d[0]])
        order = sorted(range(len(branches)), key=lambda k: float(dirs[k] @ nrm))
        spine = [H + d * spacing * (i + 1) for i in range(len(branches))]
        pts = [[float(p[0]), float(p[1])] for p in spine]
        if tend == -1:
            trunk["line"] = trunk["line"] + pts
        else:
            trunk["line"] = pts[::-1] + trunk["line"]
        for rank, k in enumerate(order):
            r, end = branches[k]
            L = r["line"] if end == 0 else r["line"][::-1]
            reach = spacing * (rank + 1) + 6.0
            while len(L) > 2 and float((np.array(L[1]) - H) @ d) < reach:
                L.pop(1)
            L[0] = pts[rank]
            r["line"] = L if end == 0 else L[::-1]
        done += 1
    return done


def fix_roads(roads):
    """Weld near-miss ends, then take the kinks out of village streets.

    A vertex shared by two roads (a junction) never moves, so the network stays
    joined exactly where it was, plus the welds. Idempotent."""
    stars = unstar(roads) + unfan(roads)
    streets = [r for r in roads if r["drivable"] and any(s in r["name"] for s in STREET_NAMES)]
    count = {}
    for r in roads:
        for p in r["line"]:
            count[_key(p)] = count.get(_key(p), 0) + 1
    welded = 0
    for r in streets:
        if "via" not in r["name"] and "Podere" not in r["name"]:
            continue
        for end in (0, -1):
            e = np.array(r["line"][end], float)
            if count[_key(e)] > 1:
                continue
            best = None
            for s in roads:
                if s is r or not s["drivable"]:
                    continue
                L = np.array(s["line"], float)
                for i in range(len(L) - 1):
                    ab = L[i + 1] - L[i]
                    t = float(np.clip((e - L[i]) @ ab / max(ab @ ab, 1e-9), 0, 1))
                    q = L[i] + t * ab
                    d = float(np.hypot(*(e - q)))
                    if best is None or d < best[0]:
                        best = (d, s, i, t, q)
            if best is None or best[0] > WELD_M or best[0] < 0.05:
                continue
            _, s, i, t, q = best
            if t < 0.2 and count[_key(s["line"][i])] >= 1:
                q = np.array(s["line"][i], float)
            elif t > 0.8:
                q = np.array(s["line"][i + 1], float)
            else:
                s["line"].insert(i + 1, [float(q[0]), float(q[1])])
            count[_key(q)] = count.get(_key(q), 0) + 1
            count[_key(e)] -= 1
            r["line"][end] = [float(q[0]), float(q[1])]
            count[_key(q)] += 1
            welded += 1
    smoothed = 0
    for r in streets:
        L = [np.array(p, float) for p in r["line"]]
        pinned = [count[_key(p)] > 1 for p in L]
        pinned[0] = pinned[-1] = True
        for _ in range(60):
            worst = 0.0
            for i in range(1, len(L) - 1):
                tn = _turn(L[i - 1], L[i], L[i + 1])
                worst = max(worst, tn)
                if tn > MAX_TURN and not pinned[i]:
                    L[i] = (L[i - 1] + 2 * L[i] + L[i + 1]) / 4
                    smoothed += 1
            if worst <= MAX_TURN:
                break
        r["line"] = [[float(p[0]), float(p[1])] for p in L]
    return {"stars": stars, "welded": welded, "smoothed": smoothed}


class World:
    def __init__(self):
        self.osm = json.loads((SRC / "osm.json").read_text())
        self.pop = json.loads((SRC / "population.json").read_text())
        self.fuel = np.fromfile(SRC / "fuel.i32", dtype="<i4").reshape(400, 400)
        self.road_report = fix_roads(self.osm["roads"])
        self.rp, self.rt, self.rh = sample_roads(self.osm["roads"])

    def nearest_road(self, p):
        d = np.hypot(*(self.rp - p).T)
        i = int(d.argmin())
        return float(d[i]), float(self.rt[i])


def layout(wd, loc):
    bs = [b for b in wd.osm["buildings"] if b.get("locality") == loc]
    mov = [b for b in bs if b["kind"] in RESIDENTIAL]
    # (centre, w, h, th, kind) of the fixed things: walls at the rendered size, ground as is.
    fixed = []
    for b in bs:
        if b["kind"] in RESIDENTIAL:
            continue
        c, w, h, th = rect_of(b["ring"])
        k = 1.0 if b["kind"] in FLAT else TOY
        fixed.append((c, w * k, h * k, th, b["kind"]))
    S = []
    for b in mov:
        c, w, h, th = rect_of(b["ring"])
        d, rt = wd.nearest_road(c)
        if d < SNAP_RANGE:
            th = rt if w >= h else rt + math.pi / 2
        S.append({"b": b, "c0": c.copy(), "c": c.copy(), "w": w * TOY, "h": h * TOY, "th": th, "w0": w, "h0": h})
    # Streets near this village only: keeps the loops cheap.
    allc = np.array([s["c"] for s in S])
    lo, hi = allc.min(0) - 120, allc.max(0) + 120
    m = (wd.rp[:, 0] > lo[0]) & (wd.rp[:, 0] < hi[0]) & (wd.rp[:, 1] > lo[1]) & (wd.rp[:, 1] < hi[1])
    rp, rh = wd.rp[m], wd.rh[m]
    for it in range(400):
        moved = 0.0
        for i, s in enumerate(S):
            push = np.zeros(2)
            # streets
            sd = sdf_box(rp, s["c"], s["w"], s["h"], s["th"]) - (rh + STREET_MARGIN)
            bad = np.where(sd < 0)[0]
            if len(bad):
                j = bad[sd[bad].argmin()]
                away = s["c"] - rp[j]
                # a street through the middle of the box: leave along the box normal toward the near side
                n = np.hypot(*away)
                away = away / n if n > 1e-6 else np.array([1.0, 0.0])
                push += away * min(-sd[j] + 0.3, 6.0)
            # neighbours
            for k, t in enumerate(S):
                if k == i or np.hypot(*(s["c"] - t["c"])) > 60:
                    continue
                a = corners(s["c"], s["w"] + HOUSE_GAP, s["h"] + HOUSE_GAP, s["th"])
                bq = corners(t["c"], t["w"], t["h"], t["th"])
                v = mtv(a, bq)
                if v is not None:
                    push += v * 0.5
            for (c, w, h, th, _kind) in fixed:
                if np.hypot(*(s["c"] - c)) > 120:
                    continue
                v = mtv(corners(s["c"], s["w"] + HOUSE_GAP, s["h"] + HOUSE_GAP, s["th"]), corners(c, w, h, th))
                if v is not None:
                    push += v
            if not push.any():
                continue
            new = s["c"] + push
            off = new - s["c0"]
            if np.hypot(*off) > MAX_MOVE:
                new = s["c0"] + off / np.hypot(*off) * MAX_MOVE
            moved += float(np.hypot(*(new - s["c"])))
            s["c"] = new
        if moved < 0.05:
            break
    return S, fixed, (rp, rh)


def violations(S, fixed, roads):
    rp, rh = roads
    out = {"street": 0, "house": 0, "public": 0}
    for i, s in enumerate(S):
        if (sdf_box(rp, s["c"], s["w"], s["h"], s["th"]) - rh < 0).any():
            out["street"] += 1
        a = corners(s["c"], s["w"], s["h"], s["th"])
        for t in S[i + 1:]:
            if np.hypot(*(s["c"] - t["c"])) < 60 and mtv(a, corners(t["c"], t["w"], t["h"], t["th"])) is not None:
                out["house"] += 1
        for (c, w, h, th, _k) in fixed:
            if np.hypot(*(s["c"] - c)) < 120 and mtv(a, corners(c, w, h, th)) is not None:
                out["public"] += 1
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    wd = World()
    dw = {d["osm_id"]: d for d in wd.pop["dwellings"]}
    by_dw = {}
    for h in wd.pop["households"]:
        by_dw.setdefault(h["building"], []).append(h)
    print("roads:", wd.road_report)
    cleared = 0
    for loc in VILLAGES:
        S, fixed, roads = layout(wd, loc)
        v = violations(S, fixed, roads)
        mv = [float(np.hypot(*(s["c"] - s["c0"]))) for s in S]
        print(f"{loc:16s} houses {len(S):3d}  moved mean {np.mean(mv):4.1f} max {np.max(mv):4.1f} m  remaining {v}")
        if a.check:
            continue
        for s in S:
            b = s["b"]
            delta = s["c"] - s["c0"]
            th = s["th"]
            ring = corners(s["c"], s["w0"], s["h0"], th)
            b["ring"] = [[float(x), float(y)] for x, y in ring]
            b["centroid"] = [float(s["c"][0]), float(s["c"][1])]
            d = dw.get(b["id"])
            if d is not None:
                d["pos"] = [d["pos"][0] + float(delta[0]), d["pos"][1] + float(delta[1])]
                d["cell"] = [int((WORLD_H - d["pos"][1]) // CELL), int(d["pos"][0] // CELL)]
            for h in by_dw.get(b["id"], []):
                h["pos"] = [h["pos"][0] + float(delta[0]), h["pos"][1] + float(delta[1])]
                h["cell"] = [int((WORLD_H - h["pos"][1]) // CELL), int(h["pos"][0] // CELL)]
                r, c = h["cell"]
                if wd.fuel[r, c] != 0:
                    wd.fuel[r, c] = 0
                    cleared += 1
    if not a.check:
        wd.fuel.astype("<i4").tofile(SRC / "fuel.i32")
        print(f"cells cleared in fuel.i32: {cleared}")
        (SRC / "osm.json").write_text(json.dumps(wd.osm, indent=2, ensure_ascii=False) + "\n")
        (SRC / "population.json").write_text(json.dumps(wd.pop, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
