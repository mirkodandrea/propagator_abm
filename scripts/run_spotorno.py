"""Spotorno initial-attack scenario: a tramontana-driven fire on the ridge
behind town, running downslope toward the coastal WUI.

Reads the 512x512 @20m window extracted from the EU COGs (UTM 32N) and runs
the PROPAGATOR core over a 2-hour initial-attack window.
"""

import json
from pathlib import Path

import numpy as np
import rasterio

from propagator.cli.main import fuels_from_yaml
from propagator.core import BoundaryConditions, Propagator

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data"
OUT = ROOT / "results" / "spotorno"
FUELS_YAML = Path(
    "/Users/mirko/dev/fire/propagator/propagator_sim/example/pedrogao/fuels_eu12.yaml"
)

# `w_dir` is the meteorological convention: the bearing the wind blows *from*
# (verified empirically on flat terrain -- w_dir=0 drives the fire south).
# Tramontana = north wind = 0, pushing the fire downslope onto the coast.
WIND_DIR = 0.0
WIND_SPEED = 35.0  # km/h
MOISTURE = 6.0  # percent
REALIZATIONS = 20
TIME_LIMIT = 2 * 3600  # initial attack window


def pick_ignition(fuel: np.ndarray, dem: np.ndarray) -> tuple[int, int]:
    """Burnable fuel at the WUI edge, just upwind of the built-up strip.

    A ridge-top ignition is the wrong scenario for an initial-attack window:
    under tramontana the wind pushes the fire downslope while the slope resists,
    so it crawls and never threatens anyone. Instead this sits the ignition a
    few hundred metres directly upwind (north) of a dense cluster of houses, so
    the fire is in the WUI from the start.
    """
    from scipy.ndimage import distance_transform_edt, uniform_filter

    rows, cols = fuel.shape
    pop = json.loads((DATA / "spotorno_population.json").read_text())
    cell = pop["fire_grid"]["cellsize"]

    houses = np.zeros((rows, cols), dtype=np.float32)
    for h in pop["households"]:
        r, c = h["cell"]
        houses[r, c] += 1.0
    # how many houses sit in the 600 m downwind (south) of each cell
    exposure = uniform_filter(houses, size=31, mode="constant")
    exposure = np.roll(exposure, shift=int(round(300.0 / cell)), axis=0)

    burnable = np.isin(fuel, range(1, 13))
    dist_to_houses = distance_transform_edt(houses == 0) * cell

    band = burnable & (dist_to_houses > 150) & (dist_to_houses < 500)
    if not band.any():
        band = burnable & (dist_to_houses < 800)

    score = np.where(band, exposure, -1.0)
    best = np.unravel_index(int(np.argmax(score)), score.shape)
    return int(best[0]), int(best[1])


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)

    with rasterio.open(DATA / "spotorno_fuel.tif") as src:
        fuel = src.read(1).astype(np.int32)
        transform, crs = src.transform, src.crs
        profile = src.profile
    with rasterio.open(DATA / "spotorno_dem.tif") as src:
        dem = src.read(1).astype(np.float32)
    dem = np.where(dem == -9999, 0.0, dem)

    row, col = pick_ignition(fuel, dem)
    x, y = transform * (col + 0.5, row + 0.5)
    print(f"ignition cell (row={row}, col={col})  UTM32N ({x:.0f}, {y:.0f})")
    print(f"  elevation {dem[row, col]:.0f} m, fuel class {fuel[row, col]}")

    sim = Propagator(
        dem=dem.astype(np.float32),
        veg=fuel,
        realizations=REALIZATIONS,
        fuels=fuels_from_yaml(FUELS_YAML),
        do_spotting=True,
        cellsize=20.0,
        seed=42,
        out_of_bounds_mode="ignore",
    )
    sim.set_boundary_conditions(
        BoundaryConditions(
            time=0,
            ignitions=[(row, col)],
            wind_speed=np.full(dem.shape, WIND_SPEED, dtype=np.float32),
            wind_dir=np.full(dem.shape, WIND_DIR, dtype=np.float32),
            moisture=np.full(dem.shape, MOISTURE, dtype=np.float32),
        )
    )

    history = []
    next_report = 600
    while (nt := sim.next_time()) is not None and nt <= TIME_LIMIT:
        sim.step()
        if sim.time >= next_report:
            prob = sim.compute_fire_probability()
            burned_ha = float((prob > 0.5).sum()) * 400.0 / 10_000.0
            reached_ha = float((prob > 0.0).sum()) * 400.0 / 10_000.0
            history.append(
                {
                    "time_s": int(sim.time),
                    "burned_ha_p50": round(burned_ha, 1),
                    "reached_ha_any": round(reached_ha, 1),
                }
            )
            print(
                f"  t={sim.time // 60:>3} min   "
                f"p>0.5: {burned_ha:7.1f} ha   any: {reached_ha:7.1f} ha"
            )
            out = profile.copy()
            out.update(dtype="float32", count=1, nodata=None, compress="deflate")
            with rasterio.open(
                OUT / f"fire_probability_{sim.time}.tif", "w", **out
            ) as dst:
                dst.write(prob.astype(np.float32), 1)
            next_report += 600

    (OUT / "history.json").write_text(
        json.dumps(
            {
                "scenario": "Spotorno initial attack, tramontana",
                "wind_dir_from": WIND_DIR,
                "wind_speed_kmh": WIND_SPEED,
                "moisture_pct": MOISTURE,
                "realizations": REALIZATIONS,
                "ignition": {"row": row, "col": col, "utm32n": [x, y]},
                "crs": str(crs),
                "history": history,
            },
            indent=2,
        )
    )
    print(f"\nwrote {OUT}")


if __name__ == "__main__":
    main()
