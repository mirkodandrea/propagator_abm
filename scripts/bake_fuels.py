"""Bake the eu_fuel12 table to JSON for the Rust core.

propagator-core ships only the 7-class legacy table; our rasters use the
12-class EU system, so the definitions travel as data rather than being
hard-coded in Rust.
"""

import json
from pathlib import Path

import yaml

DATA = Path(__file__).resolve().parents[1] / "data"
SRC = Path(
    "/Users/mirko/dev/fire/propagator/propagator_sim/example/pedrogao/fuels_eu12.yaml"
)

defs = yaml.safe_load(SRC.read_text())["fuels"]
out = []
for fid, f in defs.items():
    out.append(
        {
            "id": int(fid),
            "name": f["name"],
            "v0": float(f.get("v0", 0.0)),
            "d0": float(f.get("d0", 0.0)),
            "d1": float(f.get("d1", 0.0) or 0.0),
            "hhv": float(f.get("hhv", 0.0)),
            "humidity": (None if f.get("humidity") is None else float(f["humidity"])),
            "spotting": bool(f.get("spotting", False)),
            "prob_ign_by_embers": float(f.get("prob_ign_by_embers", 0.0)),
            "burn": bool(f.get("burn", True)),
            "spread_probability": [
                [int(k), float(v)] for k, v in (f.get("spread_probability") or {}).items()
            ],
        }
    )

path = DATA / "fuels_eu12.json"
path.write_text(json.dumps(out, indent=1))
print(f"{len(out)} fuel classes -> {path}")
for f in out:
    if f["d1"] and f["humidity"] is None:
        print(f"  WARNING: id {f['id']} has live load but no humidity (core will reject)")
