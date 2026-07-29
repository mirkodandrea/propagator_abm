"""Bake the fire-model rasters to raw little-endian arrays.

The game needs fuel and DEM as plain grids to hand to propagator-core. Pulling
a TIFF decoder into the Rust build just to read two fixed-size arrays isn't
worth the dependency, so they're flattened here alongside the GeoTIFFs.
"""

from pathlib import Path

import numpy as np
import rasterio

DATA = Path(__file__).resolve().parents[1] / "data"

with rasterio.open(DATA / "spotorno_fuel.tif") as src:
    fuel = src.read(1).astype("<i4")
with rasterio.open(DATA / "spotorno_dem.tif") as src:
    dem = src.read(1).astype(np.float64)

dem = np.where(dem == -9999, 0.0, dem).astype("<f8")

fuel.tofile(DATA / "spotorno_fuel.i32")
dem.tofile(DATA / "spotorno_dem.f64")

print(f"fuel {fuel.shape} {fuel.dtype} -> spotorno_fuel.i32 ({fuel.nbytes / 1e6:.1f} MB)")
print(f"dem  {dem.shape} {dem.dtype} -> spotorno_dem.f64  ({dem.nbytes / 1e6:.1f} MB)")
print(f"burnable cells: {int(np.isin(fuel, range(1, 13)).sum())}")
