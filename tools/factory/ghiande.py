"""Le Ghiande in one hamlet (2026-10-21, user's request).

The district had three nuclei 0.8-2.4 km apart, so the click on its badge
looked like it covered one place. The two outer nuclei (the one at (2985, 2671)
and the one at (4025, 5048)) are laid out again beside the middle one, on the
cleared grass patch just north-west of it, along new streets that start from
the middle nucleus's northern end.

* the 21 houses of the outer nuclei move; the 11 of the middle one stay;
* their old "Podere" streets go, three new ones are drawn (ids >= 400);
* each moved house gets the same shift on its dwelling and households, and its
  20 m fuel cell is cleared (as `borghi.py` does);
* `cover.u8` loses the old built and street cells and gains the new ones.

Run `borghi.py` afterwards: it turns each house to its street and pushes it
clear of its neighbours. Deterministic; run on its own output it finds nothing
left to move.

    python3 -I tools/factory/ghiande.py [--check]
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
KEEP = np.array([3385.0, 3452.0])      # the nucleus that stays
KEEP_R = 300.0
SIDE = 21.0                            # street axis to house centre
STEP = 32.0                            # house spacing along a street
TARGET = np.array([3360.0, 3600.0])    # slots are filled nearest to this first

# (id, name, polyline): from the north end of Podere 341 (3408, 3507)
STREETS = [
    (400, "Via delle Ghiande Alte", [(3408, 3507), (3378, 3552), (3350, 3610), (3335, 3672)]),
    (401, "Via delle Querce", [(3378, 3552), (3300, 3556), (3225, 3562)]),
    (402, "Via del Poggio", [(3350, 3610), (3280, 3612), (3212, 3618)]),
    (403, "Via della Fonte", [(3335, 3672), (3265, 3674), (3205, 3678)]),
]


def rc(x, y):
    return int((WORLD_H - y) // CELL), int(x // CELL)


def rect_of(ring):
    r = np.array(ring[:4], float)
    e1 = r[1] - r[0]
    return r.mean(0), float(np.hypot(*e1)), float(np.hypot(*(r[2] - r[1]))), math.atan2(e1[1], e1[0])


def corners(c, w, h, th):
    u = np.array([math.cos(th), math.sin(th)])
    v = np.array([-u[1], u[0]])
    return [c + sx * u * w / 2 + sy * v * h / 2 for sx, sy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]


def slots():
    """House slots on both sides of the new streets, with the street angle."""
    out = []
    for _id, _name, pts in STREETS:
        p = np.array(pts, float)
        for a, b in zip(p, p[1:]):
            length = float(np.hypot(*(b - a)))
            u = (b - a) / length
            n = np.array([-u[1], u[0]])
            th = math.atan2(u[1], u[0])
            t = 22.0
            while t <= length - 12.0:
                for side in (1, -1):
                    out.append((a + u * t + n * side * SIDE, th))
                t += STEP
    # drop the slots that crowd each other where two streets meet
    kept = []
    for c, th in out:
        if all(np.hypot(*(c - k)) > 24.0 for k, _ in kept):
            kept.append((c, th))
    return sorted(kept, key=lambda s: float(np.hypot(*(s[0] - TARGET))))


def paint(cover, pts, half_m, cls, only=None):
    """Class `cls` on the 5 m cells within `half_m` of the polyline."""
    cs = 5.0
    p = np.array(pts, float)
    for a, b in zip(p, p[1:]):
        n = int(np.hypot(*(b - a)) / 2.5) + 1
        for t in np.linspace(0, 1, n + 1):
            x, y = a + (b - a) * t
            for dy in np.arange(-half_m, half_m + 0.1, cs / 2):
                for dx in np.arange(-half_m, half_m + 0.1, cs / 2):
                    if dx * dx + dy * dy <= half_m * half_m:
                        r, c = int((WORLD_H - (y + dy)) // cs), int((x + dx) // cs)
                        if 0 <= r < cover.shape[0] and 0 <= c < cover.shape[1] and (only is None or cover[r, c] in only):
                            cover[r, c] = cls


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    osm = json.loads((SRC / "osm.json").read_text())
    pop = json.loads((SRC / "population.json").read_text())
    fuel = np.fromfile(SRC / "fuel.i32", dtype="<i4").reshape(400, 400)
    cover = np.fromfile(SRC / "cover.u8", dtype="u1").reshape(1600, 1600).copy()

    mine = [b for b in osm["buildings"] if b.get("locality") == "Le Ghiande"]
    mov = [b for b in mine if np.hypot(*(np.array(b["centroid"]) - KEEP)) > KEEP_R]
    if not mov:
        print("nothing to move: Le Ghiande is already one hamlet")
        return
    print(f"houses to move: {len(mov)}, staying: {len(mine) - len(mov)}")
    sl = slots()
    if len(sl) < len(mov):
        raise SystemExit(f"only {len(sl)} slots for {len(mov)} houses")
    # bigger buildings first, into the slots nearest the target
    mov.sort(key=lambda b: -(rect_of(b["ring"])[1] * rect_of(b["ring"])[2]))
    dw = {d["osm_id"]: d for d in pop["dwellings"]}
    by_dw = {}
    for h in pop["households"]:
        by_dw.setdefault(h["building"], []).append(h)

    old_roads = [r for r in osm["roads"] if (r.get("name") or "").startswith("Podere Le Ghiande")
                 and np.hypot(*(np.array(r["line"][0]) - KEEP)) > KEEP_R]
    # the old cells: built (3), street (1) and yard (4) around the old houses and streets
    for b in mov:
        c, w, h, th = rect_of(b["ring"])
        paint(cover, [c], 18.0, 0, only={1, 2, 3, 4})
    for r in old_roads:
        paint(cover, r["line"], 7.0, 0, only={1, 2, 3, 4})

    moved = 0.0
    cleared = 0
    new_cells = []
    for b, (c_new, th) in zip(mov, sl):
        c, w, h, _ = rect_of(b["ring"])
        delta = c_new - c
        moved += float(np.hypot(*delta))
        ring = corners(c_new, w, h, th if w >= h else th + math.pi / 2)
        b["ring"] = [[float(x), float(y)] for x, y in ring]
        b["centroid"] = [float(c_new[0]), float(c_new[1])]
        new_cells.append((c_new, w, h, th))
        d = dw.get(b["id"])
        homes = ([d] if d is not None else []) + by_dw.get(b["id"], [])
        for o in homes:
            o["pos"] = [o["pos"][0] + float(delta[0]), o["pos"][1] + float(delta[1])]
            o["cell"] = [rc(*o["pos"])[0], rc(*o["pos"])[1]]
            r, cc = o["cell"]
            if fuel[r, cc] != 0:
                fuel[r, cc] = 0
                cleared += 1
            if "fuel_at_site" in o:
                o["fuel_at_site"] = 0
    print(f"mean move {moved / len(mov):.0f} m, fuel cells cleared {cleared}")

    # streets: the old ones out, the new ones in
    gone = {r["id"] for r in old_roads}
    osm["roads"] = [r for r in osm["roads"] if r["id"] not in gone and r["id"] not in {s[0] for s in STREETS}]
    for sid, name, pts in STREETS:
        osm["roads"].append({"id": sid, "class": "unclassified", "drivable": True, "track": False, "name": name,
                             "oneway": False, "line": [[float(x), float(y)] for x, y in pts]})
        paint(cover, pts, 3.5, 1, only={0, 4, 5})
    for c, w, h, th in new_cells:
        ring = corners(c, w * 1.9, h * 1.9, th)
        paint(cover, [ring[0], ring[1]], 0, 3)
        # filled footprint: rows across the width
        u = np.array([math.cos(th), math.sin(th)])
        v = np.array([-u[1], u[0]])
        for t in np.arange(-h * 0.95 / 2 * 1.9, h * 0.95 / 2 * 1.9 + 0.1, 2.5):
            paint(cover, [c + v * t - u * w * 0.95 / 2 * 1.9, c + v * t + u * w * 0.95 / 2 * 1.9], 0, 3)
    print(f"streets removed: {len(gone)}, added: {len(STREETS)}")
    if a.check:
        return
    fuel.astype("<i4").tofile(SRC / "fuel.i32")
    cover.tofile(SRC / "cover.u8")
    (SRC / "osm.json").write_text(json.dumps(osm, indent=2, ensure_ascii=False) + "\n")
    (SRC / "population.json").write_text(json.dumps(pop, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
