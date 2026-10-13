"""The flammable pass of Rocca Ventosa (user, 2026-10-13).

A band of tall macchia (eu_fuel12 class 9, spotting) along the Strada del
Passo, from Le Terrazze over the saddle down to the macchia above Pian dei
Grilli, so a fire on the east side can reach the west villages: before it the
pass was low-susceptibility broadleaf (class 4), and no fire reached both
sides of the massif. Only vegetated cells change: roads, clearings and the
rocca's ring stay as they are.

The kiosk draws the vegetation from `fuel.i32`, so the band must look grown,
not painted: its width wanders (fractal noise on the distance to the path),
the edge is a ragged fringe of medium macchia (class 8) thinning into the old
wood, and a few islands of the old wood stand inside it. The core along the
path is always class 9, so the fire's way over the saddle stays continuous.

Rebuilds the band from the fuel before it (`BASE_REV`; outside the band the
current file is kept) and writes `data/scenarios/rocca_ventosa/fuel.i32`;
running it again gives the same file. Trial and evidence: `docs/factory/valico/`.

    .venv/bin/python tools/factory/valico.py
"""

import subprocess
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
FUEL = ROOT / "data/scenarios/rocca_ventosa/fuel.i32"
BASE_REV = "1544614"   # the island before the pass
N, CELL, SIZE = 400, 20.0, 8000.0

# the pass, in metres (x east, y north)
PATH = [(5400, 4650), (4500, 4650), (3850, 4250), (3600, 3600), (3150, 3050)]
HALF_M = 160.0          # mean half-width
WANDER_M = 75.0         # how much the edge wanders
CORE_M = 70.0           # always macchia within this of the path
FRINGE_M = 70.0         # ragged fringe beyond the edge
CLASS, FRINGE_CLASS = 9, 8
SEED = 21


def fbm2(n, rng, hurst=0.6, kmin=6):
    kx = np.fft.fftfreq(n)[None, :]
    ky = np.fft.fftfreq(n)[:, None]
    k = np.hypot(kx, ky)
    amp = np.where(k * n >= kmin, np.maximum(k, 1e-9) ** -(hurst + 1.0), 0.0)
    f = np.real(np.fft.ifft2(amp * np.exp(2j * np.pi * rng.random((n, n)))))
    return f / f.std()


def dist_to_path(path):
    x = (np.arange(N) + 0.5) * CELL
    y = SIZE - (np.arange(N) + 0.5) * CELL  # row 0 = north
    xx, yy = np.meshgrid(x, y)
    d = np.full((N, N), np.inf)
    for (ax, ay), (bx, by) in zip(path, path[1:]):
        vx, vy = bx - ax, by - ay
        t = np.clip(((xx - ax) * vx + (yy - ay) * vy) / (vx * vx + vy * vy), 0, 1)
        d = np.minimum(d, np.hypot(xx - ax - t * vx, yy - ay - t * vy))
    return d


def main():
    rel = FUEL.relative_to(ROOT).as_posix()
    base = np.frombuffer(subprocess.run(["git", "show", f"{BASE_REV}:{rel}"], cwd=ROOT, check=True,
                                        capture_output=True).stdout, dtype="<i4").reshape(N, N)
    fuel = np.fromfile(FUEL, dtype="<i4").reshape(N, N)
    rng = np.random.default_rng(SEED)
    d = dist_to_path(PATH)
    edge = HALF_M + WANDER_M * fbm2(N, rng)                 # the band's own wandering edge
    islands = fbm2(N, rng, kmin=20) > 1.6                    # patches of the old wood
    speckle = rng.random((N, N))
    band = (d <= np.maximum(edge, CORE_M)) & ~(islands & (d > CORE_M))
    fringe = ~band & (d <= edge + FRINGE_M) & (speckle < 1 - (d - edge) / FRINGE_M)
    reach = d <= HALF_M + 3 * WANDER_M + FRINGE_M            # everything the pass may touch
    out = np.where(reach, base, fuel)                        # the band area starts from before the pass
    veg = out > 0
    out[band & veg] = CLASS
    out[fringe & veg] = FRINGE_CLASS
    out[(fuel == 0) & reach] = 0                             # what later edits made non-burnable stays so
    out.astype("<i4").tofile(FUEL)
    print(f"valico: {int((band & veg).sum())} celle in classe {CLASS} "
          f"({(band & veg).sum() * CELL * CELL / 1e4:.0f} ha), {int((fringe & veg).sum())} di frangia in classe {FRINGE_CLASS}")


if __name__ == "__main__":
    main()
