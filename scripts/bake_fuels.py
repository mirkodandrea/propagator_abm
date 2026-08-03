"""Bake the eu_fuel12 table to JSON for the Rust core.

propagator-core ships only the 7-class legacy table; our rasters use the
12-class EU system, so the definitions travel as data rather than being
hard-coded in Rust.

One deliberate divergence from upstream: see SPOTTING_OVERRIDES below. Do not
"fix" it by re-copying the yaml.
"""

import json
from pathlib import Path

import yaml

DATA = Path(__file__).resolve().parents[1] / "data"
SRC = Path(
    "/Users/mirko/dev/fire/propagator/propagator_sim/example/pedrogao/fuels_eu12.yaml"
)

# Shrubs throw firebrands, and upstream says they do not.
#
# Both eu12 tables in propagator_sim (pedrogao, alexandroupolis) flag `spotting`
# on conifers alone, so in the core's kernel only a conifer cell can *generate*
# an ember -- while any burnable cell can receive one, since the landing test is
# `P_C0 * (1 + prob_ign_by_embers)` and a zero there is the base probability
# rather than a veto. That asymmetry is wrong for Mediterranean maquis, which
# is the fuel that actually carries these fires: shrub is 706 of the 1,226 cells
# that burn on Spotorno and 712 of 971 on `mati`, against 146 and 3 conifer.
# So spotting was switched on at the engine level (`FireSim::new`), ran, and
# could not fire -- `mati` and `pedrogao` produced *zero* spot fires over two
# hours, which is the same always-negative shape as houses never burning.
#
# Nothing tunes the ember *range* per fuel: the kernel's landing distance is
# wind and fireline intensity only (`d_median ~ U * I^(1/3)`), so a shrub run's
# shorter throw already falls out of its lower intensity, and the flag is the
# whole decision. Receiving matches conifers at 0.4 because fine dead shrub
# litter is at least as receptive as needle cast.
#
# This is a fork of CIMA's table, not a bake bug. It moves every fire figure
# measured before it -- see the sweep in crates/fire/tests/spotting.rs.
SPOTTING_OVERRIDES = {
    7: {"spotting": True, "prob_ign_by_embers": 0.4},
    8: {"spotting": True, "prob_ign_by_embers": 0.4},
    9: {"spotting": True, "prob_ign_by_embers": 0.4},
}

defs = yaml.safe_load(SRC.read_text())["fuels"]
out = []
for fid, f in defs.items():
    f = {**f, **SPOTTING_OVERRIDES.get(int(fid), {})}
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
