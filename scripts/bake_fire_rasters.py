"""Bake the fire-model rasters to raw little-endian arrays.

The game needs fuel and DEM as plain grids to hand to propagator-core. Pulling
a TIFF decoder into the Rust build just to read two fixed-size arrays isn't
worth the dependency, so they're flattened here alongside the GeoTIFFs.

Reads `<scenario>/fuel.tif` / `dem.tif` (as written by `clip_cogs.py`) and
writes `fuel.i32` / `dem.f64` next to them.
"""

import argparse
from pathlib import Path

import numpy as np
import rasterio

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data"


def source_tif(scenario_dir: Path, scenario_id: str, name: str) -> Path:
    """`<scenario_dir>/<name>.tif`, falling back to Spotorno's original flat
    layout (`data/spotorno_<name>.tif`) so the shipped scenario re-bakes
    without needing a fresh COG clip."""
    local = scenario_dir / f"{name}.tif"
    if local.exists():
        return local
    legacy = DATA / f"{scenario_id}_{name}.tif"
    if scenario_id == "spotorno" and legacy.exists():
        return legacy
    raise SystemExit(f"missing {local} (run scripts/clip_cogs.py first)")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenario", default="spotorno", help="scenario id under data/scenarios/")
    args = ap.parse_args()

    scenario_dir = DATA / "scenarios" / args.scenario
    scenario_dir.mkdir(parents=True, exist_ok=True)

    with rasterio.open(source_tif(scenario_dir, args.scenario, "fuel")) as src:
        fuel = src.read(1).astype("<i4")
    with rasterio.open(source_tif(scenario_dir, args.scenario, "dem")) as src:
        dem = src.read(1).astype(np.float64)

    dem = np.where(dem == -9999, 0.0, dem).astype("<f8")

    fuel_out = scenario_dir / "fuel.i32"
    dem_out = scenario_dir / "dem.f64"
    fuel.tofile(fuel_out)
    dem.tofile(dem_out)

    print(f"fuel {fuel.shape} {fuel.dtype} -> {fuel_out} ({fuel.nbytes / 1e6:.1f} MB)")
    print(f"dem  {dem.shape} {dem.dtype} -> {dem_out} ({dem.nbytes / 1e6:.1f} MB)")
    print(f"burnable cells: {int(np.isin(fuel, range(1, 13)).sum())}")


if __name__ == "__main__":
    main()
