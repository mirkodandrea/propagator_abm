"""Clip the fuel and DEM windows for a scenario straight out of the EU COGs.

This is the step that used to be done by hand for Spotorno -- there was no
script for it, just a one-off `rasterio`/`gdalwarp` session, which is why the
flat `data/spotorno_fuel.tif` / `spotorno_dem.tif` exist with no script that
produced them. Formalising it here is what makes a second *real* scenario
possible without repeating that by hand: give `places.py` a UTM zone and a
window, and this reads exactly that window out of the COG, cheaply, because
it is a windowed read over HTTP/S3 rather than a download of the whole tile.

Needs `AWS_PROFILE=return` (see `data/README.md` for how that profile is set
up) and network access to `s3://cima-propagator-return`. Both COGs share one
grid -- origin `(0, 7960000)`, 20 m pixels -- so a window defined in that grid
lands on identical row/col indexing in fuel and DEM.
"""

import argparse
from pathlib import Path

import rasterio
from rasterio.windows import from_bounds

import places

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data"

BUCKET = "cima-propagator-return"
DEM_COG = "s3://{bucket}/cogs/eu/eu_dem_utm_{zone}.tif"
FUEL_COG = "s3://{bucket}/cogs/eu/eu_fuel12_utm_{zone}.tif"


def clip(src_uri: str, left: float, bottom: float, right: float, top: float, out: Path) -> None:
    with rasterio.open(src_uri) as src:
        window = from_bounds(left, bottom, right, top, transform=src.transform)
        data = src.read(1, window=window)
        transform = src.window_transform(window)
        profile = src.profile.copy()
        profile.update(
            height=data.shape[0], width=data.shape[1], transform=transform,
            driver="GTiff", compress="deflate",
        )
    out.parent.mkdir(parents=True, exist_ok=True)
    with rasterio.open(out, "w", **profile) as dst:
        dst.write(data, 1)
    print(f"{src_uri} -> {out}  {data.shape}  ({out.stat().st_size / 1e6:.1f} MB)")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenario", required=True, help="place id from scripts/places.py")
    args = ap.parse_args()
    place = places.get(args.scenario)

    left, bottom = place.utm_corner
    width_m, height_m = place.world_size_m
    right, top = left + width_m, bottom + height_m

    scenario_dir = DATA / "scenarios" / place.id
    clip(
        DEM_COG.format(bucket=BUCKET, zone=place.utm_zone),
        left, bottom, right, top, scenario_dir / "dem.tif",
    )
    clip(
        FUEL_COG.format(bucket=BUCKET, zone=place.utm_zone),
        left, bottom, right, top, scenario_dir / "fuel.tif",
    )
    print(f"\nnext: python scripts/fetch_osm.py --scenario {place.id}")


if __name__ == "__main__":
    main()
