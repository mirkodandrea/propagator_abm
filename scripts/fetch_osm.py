"""Fetch the real OSM layers for the Spotorno scenario window and bake them
into a single JSON asset in a metric local world frame (UTM 32N derived).

Layers: building footprints, the road network (classified by whether an engine
can drive it), and water sources engines can refill from.

Vector geometry is kept in metres at full precision -- the 20 m fuel/DEM
spacing constrains the fire model only, not rendering or agent movement.
"""

import json
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

import rasterio
from pyproj import Transformer

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data"
CACHE = DATA / "osm_raw.json"
OUT = DATA / "spotorno_osm.json"

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


def fetch(query: str) -> dict:
    if CACHE.exists():
        print(f"using cached {CACHE}")
        return json.loads(CACHE.read_text())
    data = urllib.parse.urlencode({"data": query}).encode()
    for attempt in range(3):
        try:
            print(f"querying Overpass (attempt {attempt + 1})...")
            req = urllib.request.Request(
                OVERPASS, data=data, headers={"User-Agent": "propagator-abm/0.1"}
            )
            with urllib.request.urlopen(req, timeout=240, context=SSL_CTX) as resp:
                raw = json.loads(resp.read())
            CACHE.write_text(json.dumps(raw))
            print(f"cached {len(raw.get('elements', []))} elements -> {CACHE}")
            return raw
        except (urllib.error.URLError, TimeoutError) as exc:
            print(f"  failed: {exc}")
            if attempt < 2:
                time.sleep(15)
    raise SystemExit("Overpass unreachable after 3 attempts")


def main() -> None:
    with rasterio.open(DATA / "spotorno_fuel.tif") as src:
        bounds, transform, crs = src.bounds, src.transform, src.crs
        rows, cols = src.shape
    width_m = bounds.right - bounds.left
    height_m = bounds.top - bounds.bottom

    to_wgs = Transformer.from_crs(crs, "EPSG:4326", always_xy=True)
    to_utm = Transformer.from_crs("EPSG:4326", crs, always_xy=True)
    west, south = to_wgs.transform(bounds.left, bounds.bottom)
    east, north = to_wgs.transform(bounds.right, bounds.top)

    raw = fetch(overpass_query(south, west, north, east))

    def to_grid(lon: float, lat: float) -> tuple[float, float]:
        """lon/lat -> local world metres, origin at the window's SW corner,
        +x east / +y north.

        Deliberately *not* in fire-grid cell units: the 20 m raster spacing is
        a constraint of the fire model alone, and vector geometry carries far
        more precision than that. Converting to sim cells is a divide by
        `grid.cellsize`, done only where the fire core is actually addressed.
        """
        x, y = to_utm.transform(lon, lat)
        return round(x - bounds.left, 2), round(y - bounds.bottom, 2)

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
            buildings.append(
                {
                    "id": el["id"],
                    "kind": tags.get("building"),
                    "levels": tags.get("building:levels"),
                    "name": tags.get("name"),
                    "centroid": [round(cx, 2), round(cy, 2)],
                    "ring": ring,
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

    out = {
        "crs": str(crs),
        "units": "metres, origin at SW corner of the window, +x east +y north",
        "world_size_m": [width_m, height_m],
        "utm_origin": [bounds.left, bounds.bottom],
        "fire_grid": {"rows": rows, "cols": cols, "cellsize": 20.0},
        "transform": list(transform)[:6],
        "bbox_wgs84": {"south": south, "west": west, "north": north, "east": east},
        "buildings": buildings,
        "roads": roads,
        "water": water,
    }
    OUT.write_text(json.dumps(out))

    drivable = sum(1 for r in roads if r["drivable"])
    print(f"\nbuildings : {len(buildings)}")
    print(f"roads     : {len(roads)}  ({drivable} drivable, {len(roads) - drivable} track/path)")
    print(f"water     : {len(water)}  " + str({k: sum(1 for w in water if w['kind'] == k) for k in {w['kind'] for w in water}}))
    print(f"\nwrote {OUT} ({OUT.stat().st_size / 1e6:.1f} MB)")


if __name__ == "__main__":
    main()
