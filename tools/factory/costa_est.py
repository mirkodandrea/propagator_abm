"""The east coast north of the port (user, 2026-10-13: «la spiaggia è diritta»).

`island.py` kept the coast 300 m from the window edge, so from the port's north
arm to the north-east corner the shore was a straight line at x = 7695 m, with
a square notch at the corner. This pass cuts it back to a drawn line (a sandy
bay, a small rocky headland, a cove, then diagonally round the corner into the
north coast), roughened with 2-D fractal noise, and reshapes the ground behind
it: a gentle beach shelf in the bay, a steeper face on the headland, both
keeping the fine relief of the old ground and fading back into it inland. It
only turns land into sea: the port, roads, houses and ignitions are inland of
the new line and untouched.

Edits the published files in place (`render_terrain.f32`, `dem.f64`,
`fuel.i32`, `cover.u8`) once; run again it finds nothing to cut and stops.

    .venv/bin/python tools/factory/costa_est.py
"""

from pathlib import Path

import numpy as np
from matplotlib.path import Path as Poly
from scipy.ndimage import distance_transform_edt, gaussian_filter

ROOT = Path(__file__).resolve().parents[2]
SCN = ROOT / "data" / "scenarios" / "rocca_ventosa"
WORLD_M, N = 8000.0, 400       # fire grid: 20 m cells, row 0 = north
FM = 5.0                       # render surface: nodes every 5 m
NF = int(WORLD_M / FM) + 1     # 1601 nodes per edge
NC = NF - 1                    # 1600 cover cells per edge
K = NF // N                    # 4 fine steps per fire cell
WATER = 6                      # cover class (cover.json)
SEA_RENDER_M = 0.5

# the new shore, (x, y) in metres, from the port's north arm round to the north coast
SHORE = [
    (7700, 3570), (7640, 3700), (7525, 3950), (7425, 4300), (7385, 4650),
    (7440, 4950), (7565, 5150), (7650, 5300), (7600, 5450), (7470, 5650),
    (7330, 5850), (7150, 6020), (7020, 6170), (6930, 6330), (6880, 6450),
]
# the sea side of it, closing the polygon far out
SEA_SIDE = [(7800, 6800), (8100, 6800), (8100, 3570)]
# and the short straight south of the port's south arm (the coastal road stays 75 m inland)
SHORE_S = [(7700, 3262), (7665, 3200), (7645, 3120), (7600, 3040), (7520, 2975), (7450, 2900)]
SEA_SIDE_S = [(7450, 2700), (8100, 2700), (8100, 3262)]
HEADLAND = (7600, 5320)            # rock round here, sand elsewhere
ROCK_R_M = (180.0, 380.0)          # full rock within, sand beyond
BEACH_SLOPE, ROCK_SLOPE = 0.035, 0.20
REACH_M = (250.0, 400.0)           # the shelf fades back into the old ground
NOISE_M = (12.0, 30.0)             # outline roughness: sand, rock
SEED = 13


def fbm2(n, rng, hurst=0.7, kmin=4):
    """A 2-D fractal field of n x n samples, unit std."""
    kx = np.fft.fftfreq(n)[None, :]
    ky = np.fft.fftfreq(n)[:, None]
    k = np.hypot(kx, ky)
    amp = np.where(k * n >= kmin, np.maximum(k, 1e-9) ** -(hurst + 1.0), 0.0)
    f = np.real(np.fft.ifft2(amp * np.exp(2j * np.pi * rng.random((n, n)))))
    return f / f.std()


def to_fire_dem(fine):
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


def main():
    z = np.fromfile(SCN / "render_terrain.f32", dtype="<f4").reshape(NF, NF).astype(np.float64)
    Y, X = np.meshgrid(WORLD_M - np.arange(NF) * FM, np.arange(NF) * FM, indexing="ij")
    pts = np.column_stack([X.ravel(), Y.ravel()])
    inside = np.zeros(NF * NF, bool)
    for line, side in ((SHORE, SEA_SIDE), (SHORE_S, SEA_SIDE_S)):
        inside |= Poly(line + side + [line[0]]).contains_points(pts)
    inside = inside.reshape(NF, NF)
    # signed distance to the drawn line: + towards the sea
    d_in = distance_transform_edt(inside) * FM
    d_out = distance_transform_edt(~inside) * FM
    sd = np.where(inside, d_in, -d_out)
    rng = np.random.default_rng(SEED)
    r = np.hypot(X - HEADLAND[0], Y - HEADLAND[1]) + 90.0 * fbm2(NF, rng, kmin=12)  # no round edge
    rock = np.clip((ROCK_R_M[1] - r) / (ROCK_R_M[1] - ROCK_R_M[0]), 0, 1)
    rock = rock * rock * (3 - 2 * rock)
    amp = NOISE_M[0] + (NOISE_M[1] - NOISE_M[0]) * rock
    cut = sd + fbm2(NF, rng) * amp > 0

    sea_old = z <= SEA_RENDER_M
    new_sea = cut & ~sea_old
    if not new_sea.any():
        print("costa est: già applicata, nessun cambiamento")
        return
    sea = sea_old | cut
    L = distance_transform_edt(~sea) * FM            # metres to the new sea
    Lcut = distance_transform_edt(~new_sea) * FM     # metres to where land was cut
    slope = BEACH_SLOPE + (ROCK_SLOPE - BEACH_SLOPE) * rock
    detail = np.clip(z - gaussian_filter(z, 6.0), -3.0, 3.0) * np.clip(L / 40.0, 0, 1)
    shelf = np.minimum(z, 0.62 + slope * L + 0.7 * detail)
    w = np.clip((np.maximum(L, Lcut) - REACH_M[0]) / (REACH_M[1] - REACH_M[0]), 0, 1)
    w = w * w * (3 - 2 * w)
    z_new = np.where(sea, np.minimum(z, 0.0), (1 - w) * shelf + w * z)
    z_new = np.where(sea | (z_new > SEA_RENDER_M), z_new, SEA_RENDER_M + 0.12)
    moved = np.abs(z_new - z) > 1e-6
    z_new.astype("<f4").tofile(SCN / "render_terrain.f32")

    # fire grid: the new surface where it changed; the sea and the beach strip don't burn
    dem = np.fromfile(SCN / "dem.f64", dtype="<f8").reshape(N, N)
    touched = to_fire_dem(moved.astype(float)) > 0
    dem = np.where(touched, to_fire_dem(z_new), dem)
    dem.tofile(SCN / "dem.f64")
    fuel = np.fromfile(SCN / "fuel.i32", dtype="<i4").reshape(N, N)
    near = Lcut < REACH_M[1]
    sand = (L > 0) & (L < 85.0) & (rock < 0.5) & near
    shore = (L > 0) & (L < 45.0) & near
    nb = (to_fire_dem(sea.astype(float)) > 0.5) | (to_fire_dem((sand | shore).astype(float)) > 0.4)
    fuel_new = np.where(nb, 0, fuel)
    fuel_new.astype("<i4").tofile(SCN / "fuel.i32")

    # 5 m cover: water where all four corners are sea
    cov = np.fromfile(SCN / "cover.u8", dtype=np.uint8).reshape(NC, NC)
    corners = np.stack([sea[:-1, :-1], sea[1:, :-1], sea[:-1, 1:], sea[1:, 1:]]).all(axis=0)
    cov[corners] = WATER
    cov.tofile(SCN / "cover.u8")
    print(f"costa est: {int(new_sea.sum())} nodi da terra a mare, {int(moved.sum())} quote cambiate, "
          f"{int((fuel_new != fuel).sum())} celle di combustibile a 0")


if __name__ == "__main__":
    main()
