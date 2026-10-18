"""Service areas: pitches, car parks, assembly areas, cemetery (2026-10-18).

`borghi.py` left the service areas of each village as copies of one rotated
rectangle: same angle, a 70 x 44 m "football pitch", no way in. This script
lays them out again, in `osm.json` only (nothing in the simulation reads them
except the fuel cells they stand on):

* every area is turned to the nearest street plus its own offset (a hash of its
  id), so a village does not show one angle;
* a pitch is a real one (100 x 62 m, 90 x 56 m in the smaller villages);
* each area is pushed clear of the others, of the houses and of the streets;
* an area more than a few metres from a street gets a service track (non
  drivable, so routes and balance do not change) from its edge to the street;
* the yard cells the old boxes left in `cover.u8` are cleared;
* the fuel grid is not touched: nothing here is a house, and nothing counts it.

Deterministic; run on its own output it nudges areas that had to dodge each
other, so run it once from the committed `osm.json`.

    python3 -I tools/factory/servizi.py [--check]
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
TOY = 1.9                  # visual_scale::BUILDING_FOOTPRINT
ROAD_CLEAR = 9.0           # half-width + casing + margin
KINDS = {"pitch", "parking", "assembly", "cemetery"}
SIZES = {"pitch": (100.0, 62.0), "parking": None, "assembly": None, "cemetery": None}
SMALL = {"Pian dei Grilli", "Fornaci"}


def hash01(i, salt):
    h = (i * 0x9E3779B97F4A7C15 ^ salt * 0xC2B2AE3D27D4EB4F) & 0xFFFFFFFFFFFFFFFF
    h ^= h >> 29
    h = (h * 0xBF58476D1CE4E5B9) & 0xFFFFFFFFFFFFFFFF
    h ^= h >> 32
    return (h >> 40) / float(1 << 24)


def rect(c, lu, lv, ang):
    u = (math.cos(ang), math.sin(ang))
    v = (-u[1], u[0])
    return [
        [c[0] + a * lu / 2 * u[0] + b * lv / 2 * v[0], c[1] + a * lu / 2 * u[1] + b * lv / 2 * v[1]]
        for a, b in ((-1, -1), (1, -1), (1, 1), (-1, 1))
    ]


def sat(a, b):
    for P in (a, b):
        for i in range(len(P)):
            ex, ey = P[(i + 1) % len(P)][0] - P[i][0], P[(i + 1) % len(P)][1] - P[i][1]
            nx, ny = -ey, ex
            pa = [x * nx + y * ny for x, y in a]
            pb = [x * nx + y * ny for x, y in b]
            if max(pa) < min(pb) or max(pb) < min(pa):
                return False
    return True


def grown(ring, m):
    c = np.mean(ring, axis=0)
    out = []
    for p in ring:
        d = np.array(p) - c
        out.append((c + d * (1 + m / max(np.linalg.norm(d), 1e-6))).tolist())
    return out


def scaled(ring, k):
    c = np.mean(ring, axis=0)
    return [(c + (np.array(p) - c) * k).tolist() for p in ring]


def seg_hits(ring, line, m):
    """Whether the polyline passes within `m` of the convex ring."""
    big = grown(ring, m)
    for a, b in zip(line, line[1:]):
        n = max(1, int(math.dist(a, b) // 4))
        for k in range(n + 1):
            q = (a[0] + (b[0] - a[0]) * k / n, a[1] + (b[1] - a[1]) * k / n)
            if inside(big, q):
                return True
    return False


def inside(ring, q):
    s = [(ring[(i + 1) % len(ring)][0] - ring[i][0]) * (q[1] - ring[i][1]) - (ring[(i + 1) % len(ring)][1] - ring[i][1]) * (q[0] - ring[i][0]) for i in range(len(ring))]
    return all(v >= 0 for v in s) or all(v <= 0 for v in s)


def nearest(roads, p):
    best = (1e9, None, 0.0)
    for r in roads:
        L = r["line"]
        for a, b in zip(L, L[1:]):
            dx, dy = b[0] - a[0], b[1] - a[1]
            d2 = dx * dx + dy * dy or 1.0
            t = max(0.0, min(1.0, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / d2))
            q = (a[0] + t * dx, a[1] + t * dy)
            d = math.dist(p, q)
            if d < best[0]:
                best = (d, q, math.atan2(dy, dx))
    return best


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()
    osm = json.loads((SRC / "osm.json").read_text())
    B, R = osm["buildings"], osm["roads"]
    drive = [r for r in R if r["drivable"]]
    areas = [b for b in B if b["kind"] in KINDS]
    others = [b for b in B if b["kind"] not in KINDS and b["kind"] != "plaza"]
    plaza = [b for b in B if b["kind"] == "plaza"]
    placed = []
    moved = 0
    for b in sorted(areas, key=lambda b: ("assembly pitch parking cemetery".split().index(b["kind"]), b["id"])):
        old = b["ring"]
        c0 = [sum(p[0] for p in old) / len(old), sum(p[1] for p in old) / len(old)]
        e = (old[1][0] - old[0][0], old[1][1] - old[0][1])
        lu0, lv0 = math.hypot(*e), math.hypot(old[2][0] - old[1][0], old[2][1] - old[1][1])
        if lu0 < lv0:
            lu0, lv0 = lv0, lu0
        lu, lv = lu0, lv0
        if b["kind"] == "pitch":
            lu, lv = (90.0, 56.0) if b["locality"] in SMALL else SIZES["pitch"]
        k = TOY if b["kind"] == "parking" else 1.0
        d, q, road_ang = nearest(drive, c0)
        off = math.radians((hash01(b["id"], 1) - 0.5) * 44.0)
        ang = road_ang + off + (math.pi / 2 if hash01(b["id"], 2) < 0.35 and b["kind"] != "assembly" else 0.0)
        # Spiral out from where it was until nothing is in the way.
        found = None
        for ring_i in range(0, 14):
            for j in range(0 if ring_i == 0 else 8):
                t = 2 * math.pi * (j / 8 + hash01(b["id"], 3 + ring_i))
                c = [c0[0] + math.cos(t) * ring_i * 12.0, c0[1] + math.sin(t) * ring_i * 12.0]
                cand = rect(c, lu * k, lv * k, ang)
                if any(sat(grown(cand, 6.0), scaled(o["ring"], TOY)) for o in others):
                    continue
                if any(sat(cand, scaled(p["ring"], TOY if p["kind"] == "parking" else 1.0)) for p in placed + plaza):
                    continue
                if b["kind"] != "assembly" and any(seg_hits(cand, r["line"], ROAD_CLEAR) for r in drive):
                    continue
                found = (c, cand)
                break
            if found:
                break
        if not found:
            print("no room for", b["kind"], b["id"], b["locality"])
            found = (c0, rect(c0, lu * k, lv * k, ang))
        c, cand = found
        # `ring` is the plain footprint (parking is magnified when drawn).
        ring = rect(c, lu, lv, ang) if k == 1.0 else rect(c, lu, lv, ang)
        placed.append({**b, "ring": ring, "kind": b["kind"]})
        if not args.check:
            b["ring"] = [[round(x, 2), round(y, 2)] for x, y in ring]
            b["centroid"] = [round(c[0], 2), round(c[1], 2)]
        moved += 1
    # Service tracks: from each area to the street, unless it already touches one.
    tracks = [r for r in R if r.get("name") == "Pista di servizio"]
    if not args.check:
        R[:] = [r for r in R if r.get("name") != "Pista di servizio"]
    nid = max(r["id"] for r in R) + 1
    for b in placed:
        c = b["centroid"] if "centroid" in b else None
        c = [sum(p[0] for p in b["ring"]) / 4, sum(p[1] for p in b["ring"]) / 4]
        kk = TOY if b["kind"] == "parking" else 1.0
        full = scaled(b["ring"], kk)
        # Nearest point on the ring edge to the street.
        best = None
        for i in range(4):
            a, bb = full[i], full[(i + 1) % 4]
            for t in np.linspace(0.2, 0.8, 7):
                p = (a[0] + (bb[0] - a[0]) * t, a[1] + (bb[1] - a[1]) * t)
                d, q, _ = nearest(drive, p)
                if best is None or d < best[0]:
                    best = (d, p, q)
        d, p, q = best
        if d < 14.0:
            continue
        mid = ((p[0] + q[0]) / 2 + (hash01(b["id"], 9) - 0.5) * d * 0.25, (p[1] + q[1]) / 2 + (hash01(b["id"], 10) - 0.5) * d * 0.25)
        line = [p, ((p[0] + mid[0]) / 2, (p[1] + mid[1]) / 2), mid, ((mid[0] + q[0]) / 2, (mid[1] + q[1]) / 2), q]
        print(f"track {b['kind']} {b['id']} {b['locality']}: {d:.0f} m")
        if not args.check:
            R.append({"id": nid, "class": "track", "drivable": False, "track": True, "name": "Pista di servizio", "oneway": False, "line": [[round(x, 2), round(y, 2)] for x, y in line]})
            nid += 1
    print(f"{moved} areas")
    # `cover.u8` still has the yard (class 4) the old, axis-aligned boxes left on
    # the ground: grey patches that belong to no house. Clear every yard cell
    # with no building cell within 40 m, to the class around it.
    cover = np.fromfile(SRC / "cover.u8", dtype="u1").reshape(1600, 1600).copy()
    yard = cover == 4
    built = cover == 3
    near = np.zeros_like(built)
    R8 = 8
    for dr in range(-R8, R8 + 1):
        for dc in range(-R8, R8 + 1):
            near |= np.roll(np.roll(built, dr, 0), dc, 1)
    stray = yard & ~near
    ys, xs = np.nonzero(stray)
    for r, c in zip(ys, xs):
        win = cover[max(r - 6, 0) : r + 7, max(c - 6, 0) : c + 7]
        vals = win[(win != 4) & (win != 1) & (win != 2) & (win != 3)]
        cover[r, c] = np.bincount(vals).argmax() if len(vals) else 0
    print(f"{int(stray.sum())} stray yard cells cleared")
    if not args.check:
        (SRC / "osm.json").write_text(json.dumps(osm, indent=2, ensure_ascii=False) + "\n")
        cover.tofile(SRC / "cover.u8")


if __name__ == "__main__":
    main()
