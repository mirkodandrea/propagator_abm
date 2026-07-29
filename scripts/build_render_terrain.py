"""Build the *rendering* heightfield, decoupled from the fire-sim grid.

The fire core is locked to the 20 m fuel/DEM raster. Rendering is not: this
resamples the DEM onto a finer posting with a cubic spline and lightly smooths
it, so the terrain mesh reads as smooth relief instead of 20 m stair-steps.

The source DEM is genuinely native 20 m (verified: zero identical neighbours
on land, ~4 m median relief per cell), so interpolation here is filling in
between real samples rather than inventing detail. Swapping in a true 5-10 m
DTM (Tinitaly, Regione Liguria) later means changing only `load_dem`.

Outputs a raw float32 heightfield + JSON sidecar for the game to mmap, and a
GeoTIFF for inspection.
"""

import argparse
import json
from pathlib import Path

import numpy as np
import rasterio
from scipy.ndimage import gaussian_filter, map_coordinates

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data"

SEA_LEVEL = 0.5  # m; below this the DEM is sea, kept dead flat


def load_dem() -> tuple[np.ndarray, rasterio.Affine, rasterio.crs.CRS, object]:
    with rasterio.open(DATA / "spotorno_dem.tif") as src:
        dem = src.read(1).astype(np.float64)
        return dem, src.transform, src.crs, src.bounds


def build(dem: np.ndarray, factor: int, smooth: float) -> np.ndarray:
    """Cubic-resample by `factor`, then smooth, preserving the coastline.

    Cubic interpolation across a coastline rings badly -- it undershoots below
    sea level just inland and overshoots just offshore. So land is interpolated
    on its own, with sea cells filled from the nearest land value first, and
    the sea is re-flattened afterwards from an independently upsampled mask.
    """
    rows, cols = dem.shape
    land = dem > SEA_LEVEL

    # Fill sea with nearest-land elevation so the spline has no cliff to ring on.
    from scipy.ndimage import distance_transform_edt

    idx = distance_transform_edt(~land, return_distances=False, return_indices=True)
    filled = dem[tuple(idx)]

    # Sample the fine grid at cell centres of the coarse grid.
    out_r, out_c = rows * factor, cols * factor
    rr = (np.arange(out_r) + 0.5) / factor - 0.5
    cc = (np.arange(out_c) + 0.5) / factor - 0.5
    grid_r, grid_c = np.meshgrid(rr, cc, indexing="ij")

    fine = map_coordinates(
        filled, [grid_r, grid_c], order=3, mode="nearest"
    ).astype(np.float32)

    if smooth > 0:
        fine = gaussian_filter(fine, sigma=smooth * factor / 4.0)

    # Re-impose the sea: upsample the land mask with linear interpolation and
    # blend, so the shoreline is a soft edge rather than a 20 m staircase.
    mask = map_coordinates(
        land.astype(np.float32), [grid_r, grid_c], order=1, mode="nearest"
    )
    fine = fine * mask
    return np.maximum(fine, 0.0).astype(np.float32)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--factor", type=int, default=2,
        help="upsample factor over the 20 m sim grid (2 -> 10 m posting)",
    )
    ap.add_argument(
        "--smooth", type=float, default=1.0,
        help="gaussian sigma in source cells; 0 disables",
    )
    args = ap.parse_args()

    dem, transform, crs, bounds = load_dem()
    dem = np.where(dem == -9999, 0.0, dem)
    fine = build(dem, args.factor, args.smooth)

    posting = 20.0 / args.factor
    rows, cols = fine.shape

    raw = DATA / "spotorno_render_terrain.f32"
    fine.tofile(raw)

    meta = {
        "description": "rendering heightfield, independent of the 20 m fire grid",
        "rows": rows,
        "cols": cols,
        "posting_m": posting,
        "dtype": "float32",
        "layout": "row-major, row 0 = north edge",
        "world_size_m": [cols * posting, rows * posting],
        "utm_origin": [bounds.left, bounds.bottom],
        "crs": str(crs),
        "source": "eu_dem_utm_32 @20m, cubic resample + gaussian smooth",
        "smooth_sigma_source_cells": args.smooth,
        "elev_min": float(fine.min()),
        "elev_max": float(fine.max()),
    }
    (DATA / "spotorno_render_terrain.json").write_text(json.dumps(meta, indent=2))

    prof = {
        "driver": "GTiff", "height": rows, "width": cols, "count": 1,
        "dtype": "float32", "crs": crs, "compress": "deflate",
        "transform": rasterio.Affine(
            posting, 0, bounds.left, 0, -posting, bounds.top
        ),
    }
    with rasterio.open(DATA / "spotorno_render_terrain.tif", "w", **prof) as dst:
        dst.write(fine, 1)

    # How much of the stair-stepping did we actually remove? Compare the
    # second derivative (what shading normals expose) before and after.
    def roughness(a: np.ndarray) -> float:
        m = a > SEA_LEVEL
        lap = np.abs(np.gradient(np.gradient(a, axis=0), axis=0))
        return float(lap[m].mean())

    print(f"render terrain: {rows} x {cols} @ {posting:g} m")
    print(f"  {raw} ({raw.stat().st_size / 1e6:.1f} MB, {rows * cols / 1e6:.2f} M vertices)")
    print(f"  elevation {fine.min():.1f} - {fine.max():.1f} m")
    print(f"  curvature per-sample: source {roughness(dem.astype(np.float32)):.4f}"
          f" -> render {roughness(fine):.4f}")


if __name__ == "__main__":
    main()
