"""Fetch the real OSM layers for a scenario window and bake them into a
single JSON asset in a metric local world frame (UTM zone from `places.py`).

Layers: building footprints, the road network (classified by whether an engine
can drive it), and water sources engines can refill from. Buildings also carry
an address and a locality (nearest named place) where the data supports it --
see `assign_addresses` below.

Vector geometry is kept in metres at full precision -- the 20 m fuel/DEM
spacing constrains the fire model only, not rendering or agent movement.
"""

import argparse
import json
import math
import ssl
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

try:  # this interpreter's default trust store does not resolve Let's Encrypt
    import certifi

    SSL_CTX = ssl.create_default_context(cafile=certifi.where())
except ImportError:
    SSL_CTX = ssl.create_default_context()

from pyproj import Transformer

import places

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data"

OVERPASS = "https://overpass-api.de/api/interpreter"

# Roads an engine can actually drive, in descending capacity.
DRIVABLE = {
    "motorway", "trunk", "primary", "secondary", "tertiary",
    "unclassified", "residential", "living_street", "service",
    "motorway_link", "trunk_link", "primary_link", "secondary_link",
    "tertiary_link",
}
# Passable on foot / by small 4x4 -- crew access, and escape routes.
TRACKS = {"track", "path", "footway", "steps", "bridleway", "cycleway"}


def overpass_query(south: float, west: float, north: float, east: float) -> str:
    bbox = f"{south},{west},{north},{east}"
    return f"""
[out:json][timeout:180];
(
  way["building"]({bbox});
  relation["building"]({bbox});
  way["highway"]({bbox});
  node["emergency"="fire_hydrant"]({bbox});
  way["natural"="water"]({bbox});
  way["landuse"="reservoir"]({bbox});
  node["man_made"="water_tower"]({bbox});
);
out geom;
"""


def fetch(query: str, cache: Path) -> dict:
    if cache.exists():
        print(f"using cached {cache}")
        return json.loads(cache.read_text())
    data = urllib.parse.urlencode({"data": query}).encode()
    for attempt in range(3):
        try:
            print(f"querying Overpass (attempt {attempt + 1})...")
            req = urllib.request.Request(
                OVERPASS, data=data, headers={"User-Agent": "propagator-abm/0.1"}
            )
            with urllib.request.urlopen(req, timeout=240, context=SSL_CTX) as resp:
                raw = json.loads(resp.read())
            cache.parent.mkdir(parents=True, exist_ok=True)
            cache.write_text(json.dumps(raw))
            print(f"cached {len(raw.get('elements', []))} elements -> {cache}")
            return raw
        except (urllib.error.URLError, TimeoutError) as exc:
            print(f"  failed: {exc}")
            if attempt < 2:
                time.sleep(15)
    raise SystemExit("Overpass unreachable after 3 attempts")


def assign_addresses(buildings: list[dict]) -> dict[str, int]:
    """Fill in `address` and `locality` on every building, in place.

    `address` is exact where OSM carries `addr:street`/`addr:housenumber` --
    that is real but sparse (well under 1% of buildings in the shipped
    window). `locality` is filled two ways: exactly, from `addr:city` where
    present; otherwise by nearest-seed, where a seed is the centroid of every
    building tagged with a given `addr:city` in *this same fetch* -- real
    positions, not geocoded guesses, so a window with no address tags at all
    simply leaves every `locality` unset rather than inventing one.
    """
    seeds: dict[str, list[tuple[float, float]]] = {}
    for b in buildings:
        city = b.pop("_addr_city", None)
        if city:
            seeds.setdefault(city, []).append(tuple(b["centroid"]))
    centres = {city: (
        sum(p[0] for p in pts) / len(pts), sum(p[1] for p in pts) / len(pts)
    ) for city, pts in seeds.items()}

    counts: dict[str, int] = {}
    for b in buildings:
        if b.get("locality") is None and centres:
            cx, cy = b["centroid"]
            nearest = min(
                centres, key=lambda name: math.hypot(cx - centres[name][0], cy - centres[name][1])
            )
            b["locality"] = nearest
        if b.get("locality"):
            counts[b["locality"]] = counts.get(b["locality"], 0) + 1
    return counts


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenario", default="spotorno", help="place id from scripts/places.py")
    args = ap.parse_args()
    place = places.get(args.scenario)

    scenario_dir = DATA / "scenarios" / place.id
    scenario_dir.mkdir(parents=True, exist_ok=True)
    out_path = scenario_dir / "osm.json"

    cell = place.fire_cellsize_m
    width_m, height_m = place.world_size_m
    rows, cols = round(height_m / cell), round(width_m / cell)
    left, bottom = place.utm_corner
    right, top = left + width_m, bottom + height_m
    crs = f"EPSG:326{place.utm_zone:02d}"  # WGS84 / UTM zone N, northern hemisphere
    transform = [cell, 0.0, left, 0.0, -cell, top]  # affine, north-up, matches rasterio's convention

    to_wgs = Transformer.from_crs(crs, "EPSG:4326", always_xy=True)
    to_utm = Transformer.from_crs("EPSG:4326", crs, always_xy=True)
    west, south = to_wgs.transform(left, bottom)
    east, north = to_wgs.transform(right, top)

    # Cache: a scenario-specific file going forward; Spotorno's original fetch
    # cached to the flat top-level path, and reusing it here means this
    # regenerates from the exact same Overpass snapshot with no network call.
    cache = scenario_dir / "osm_raw.json"
    if not cache.exists() and place.id == "spotorno":
        legacy = DATA / "osm_raw.json"
        if legacy.exists():
            cache = legacy
    raw = fetch(overpass_query(south, west, north, east), cache)

    def to_grid(lon: float, lat: float) -> tuple[float, float]:
        """lon/lat -> local world metres, origin at the window's SW corner,
        +x east / +y north.

        Deliberately *not* in fire-grid cell units: the 20 m raster spacing is
        a constraint of the fire model alone, and vector geometry carries far
        more precision than that. Converting to sim cells is a divide by
        `grid.cellsize`, done only where the fire core is actually addressed.
        """
        x, y = to_utm.transform(lon, lat)
        return round(x - left, 2), round(y - bottom, 2)

    buildings, roads, water = [], [], []
    for el in raw.get("elements", []):
        tags = el.get("tags", {})
        geom = el.get("geometry")

        if "building" in tags:
            if not geom:
                continue
            ring = [to_grid(p["lon"], p["lat"]) for p in geom]
            cx = sum(p[0] for p in ring) / len(ring)
            cy = sum(p[1] for p in ring) / len(ring)
            if not (0 <= cx < width_m and 0 <= cy < height_m):
                continue
            street = tags.get("addr:street")
            housenumber = tags.get("addr:housenumber")
            address = f"{street} {housenumber}" if street and housenumber else street
            city = tags.get("addr:city")
            buildings.append(
                {
                    "id": el["id"],
                    "kind": tags.get("building"),
                    "levels": tags.get("building:levels"),
                    "name": tags.get("name"),
                    "centroid": [round(cx, 2), round(cy, 2)],
                    "ring": ring,
                    "address": address,
                    "locality": city,
                    "_addr_city": city,
                }
            )

        elif "highway" in tags:
            if not geom:
                continue
            hw = tags["highway"]
            line = [to_grid(p["lon"], p["lat"]) for p in geom]
            if not any(0 <= c < width_m and 0 <= r < height_m for c, r in line):
                continue
            roads.append(
                {
                    "id": el["id"],
                    "class": hw,
                    "drivable": hw in DRIVABLE,
                    "track": hw in TRACKS,
                    "name": tags.get("name"),
                    "oneway": tags.get("oneway") == "yes",
                    "line": line,
                }
            )

        elif (
            tags.get("emergency") == "fire_hydrant"
            or tags.get("natural") == "water"
            or tags.get("landuse") == "reservoir"
            or tags.get("man_made") == "water_tower"
        ):
            if geom:
                pts = [to_grid(p["lon"], p["lat"]) for p in geom]
                cx = sum(p[0] for p in pts) / len(pts)
                cy = sum(p[1] for p in pts) / len(pts)
            elif "lon" in el:
                cx, cy = to_grid(el["lon"], el["lat"])
            else:
                continue
            if not (0 <= cx < width_m and 0 <= cy < height_m):
                continue
            kind = (
                "hydrant"
                if tags.get("emergency") == "fire_hydrant"
                else "water_tower"
                if tags.get("man_made") == "water_tower"
                else "open_water"
            )
            water.append({"id": el["id"], "kind": kind, "pos": [round(cx, 2), round(cy, 2)]})

    locality_counts = assign_addresses(buildings)

    out = {
        "crs": crs,
        "units": "metres, origin at SW corner of the window, +x east +y north",
        "world_size_m": [width_m, height_m],
        "utm_origin": [left, bottom],
        "fire_grid": {"rows": rows, "cols": cols, "cellsize": cell},
        "transform": transform,
        "bbox_wgs84": {"south": south, "west": west, "north": north, "east": east},
        "buildings": buildings,
        "roads": roads,
        "water": water,
    }
    out_path.write_text(json.dumps(out))

    drivable = sum(1 for r in roads if r["drivable"])
    addressed = sum(1 for b in buildings if b["address"])
    localised = sum(1 for b in buildings if b["locality"])
    print(f"\nbuildings : {len(buildings)}")
    print(f"roads     : {len(roads)}  ({drivable} drivable, {len(roads) - drivable} track/path)")
    print(f"water     : {len(water)}  " + str({k: sum(1 for w in water if w['kind'] == k) for k in {w['kind'] for w in water}}))
    print(f"addresses : {addressed} buildings with a street address")
    print(f"localities: {localised} buildings assigned a place name -- {locality_counts}")
    print(f"\nwrote {out_path} ({out_path.stat().st_size / 1e6:.1f} MB)")


if __name__ == "__main__":
    main()
