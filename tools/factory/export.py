"""Write a candidate in the existing `Scenario` format, so `Scenario::load_by_id`
reads it unchanged. At this stage there are no buildings, roads or people:
the vectors and population are empty, as the loader allows."""

from __future__ import annotations

import json
import shutil
from pathlib import Path

import numpy as np

from .grid import CELL_M, N, WORLD_M

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "out" / "factory"
DATA = OUT / "data"  # a data dir of its own: fuels_eu12.json + scenarios/<id>/


def scenario_dir(cid: str) -> Path:
    return DATA / "scenarios" / cid


def write(cid: str, name: str, description: str, dem: np.ndarray, fuel: np.ndarray, params: dict) -> Path:
    out = scenario_dir(cid)
    out.mkdir(parents=True, exist_ok=True)
    DATA.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / "data" / "fuels_eu12.json", DATA / "fuels_eu12.json")

    dem.astype("<f8").tofile(out / "dem.f64")
    fuel.astype("<i4").tofile(out / "fuel.i32")
    dem.astype("<f4").tofile(out / "render_terrain.f32")
    (out / "render_terrain.json").write_text(json.dumps({
        "rows": N, "cols": N, "posting_m": CELL_M, "world_size_m": [WORLD_M, WORLD_M],
        "elev_min": float(dem.min()), "elev_max": float(dem.max())}, indent=2) + "\n")
    (out / "osm.json").write_text(json.dumps({
        "world_size_m": [WORLD_M, WORLD_M], "fire_grid": {"rows": N, "cols": N, "cellsize": CELL_M},
        "buildings": [], "roads": [], "water": []}, indent=2) + "\n")
    (out / "population.json").write_text(json.dumps({
        "synthetic": True, "seed": params.get("seed", 0), "dwellings": [], "households": [], "people": []},
        indent=2) + "\n")
    (out / "scenario.json").write_text(json.dumps({
        "id": cid, "name": name, "description": description,
        "location": "Luogo immaginario", "country": "Italia", "nationality": "Italian",
        "region": "territorio sintetico (Scenario Factory, fase 1)", "localities": [],
        "coordinates": [0.0, 0.0], "utm_zone": 0,
        "world_size_m": [WORLD_M, WORLD_M], "fire_grid_size": [N, N],
        "buildings_count": 0, "households_count": 0, "people_count": 0,
        "scenario_type": "synthetic", "creation_date": "2026-10-08", "version": "0.1.0",
        "tags": ["factory", "nature"], "is_dev": True}, indent=2) + "\n")
    (out / "params.json").write_text(json.dumps(params, indent=2) + "\n")
    return out


def load(cid: str) -> tuple[np.ndarray, np.ndarray, dict]:
    d = scenario_dir(cid)
    if not d.exists():
        raise SystemExit(f"{cid}: manca {d} -- eseguire prima `nature`")
    dem = np.fromfile(d / "dem.f64", dtype="<f8").reshape(N, N)
    fuel = np.fromfile(d / "fuel.i32", dtype="<i4").reshape(N, N)
    return dem, fuel, json.loads((d / "params.json").read_text())
