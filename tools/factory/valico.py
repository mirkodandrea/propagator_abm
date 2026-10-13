"""The flammable pass of Rocca Ventosa (user, 2026-10-13).

A band of tall macchia (eu_fuel12 class 9, spotting) along the Strada del
Passo, from Le Terrazze over the saddle down to the macchia above Pian dei
Grilli, so a fire on the east side can reach the west villages: before it the
pass was low-susceptibility broadleaf (class 4), and no fire reached both
sides of the massif. Only vegetated cells change: roads, clearings and the
rocca's ring stay as they are. The kiosk draws the vegetation from the same
`fuel.i32`.

Already applied to `data/scenarios/rocca_ventosa/fuel.i32`; running it again
changes nothing. Trial and evidence: `docs/factory/valico/`, `docs/TODO.md`.

    .venv/bin/python tools/factory/valico.py
"""

from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
FUEL = ROOT / "data/scenarios/rocca_ventosa/fuel.i32"
N, CELL, SIZE = 400, 20.0, 8000.0

# the pass, in metres (x east, y north)
PATH = [(5400, 4650), (4500, 4650), (3850, 4250), (3600, 3600), (3150, 3050)]
WIDTH_M = 320.0
CLASS = 9


def band(path, width_m):
    x = (np.arange(N) + 0.5) * CELL
    y = SIZE - (np.arange(N) + 0.5) * CELL  # row 0 = north
    xx, yy = np.meshgrid(x, y)
    d = np.full((N, N), np.inf)
    for (ax, ay), (bx, by) in zip(path, path[1:]):
        vx, vy = bx - ax, by - ay
        t = np.clip(((xx - ax) * vx + (yy - ay) * vy) / (vx * vx + vy * vy), 0, 1)
        d = np.minimum(d, np.hypot(xx - ax - t * vx, yy - ay - t * vy))
    return d <= width_m / 2


def main():
    fuel = np.fromfile(FUEL, dtype=np.int32).reshape(N, N)
    m = band(PATH, WIDTH_M) & (fuel > 0)
    changed = int((fuel[m] != CLASS).sum())
    fuel[m] = CLASS
    fuel.tofile(FUEL)
    print(f"valico: {m.sum()} celle ({m.sum() * CELL * CELL / 1e4:.0f} ha) in classe {CLASS}, {changed} cambiate")


if __name__ == "__main__":
    main()
