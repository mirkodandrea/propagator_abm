"""The playable cases of a built town (`game.json`): where each fire starts,
the wind that drives it at its town, the stations and the roster.

The fire starts are the town sweep's own (`town-fires`), so every case has an
atlas entry behind it. The wind of a case blows from the start toward the
locality it was picked for; the rest of the sweep's winds stay in the atlas.
The roster follows the spec's MVP (01-SPEC-GIOCO §1): two engines and one AIB
hand crew. The engines are the local fire station's; the AIB crew comes in by
road from outside the map, so its ETA is a real drive."""

from __future__ import annotations

import json

from . import fires
from .export import DATA

ROSTER = [
    {"kind": "engine", "station": 0},
    {"kind": "engine", "station": 0},
    {"kind": "hand_crew", "station": 1},
]
# The AIB crew's way in: the SP 12 della Valle at the east edge of the map.
AIB_ENTRY_ROAD = "SP 12 della Valle"
CASE_MINUTES = 180


def build(cid: str) -> dict:
    d = DATA / "scenarios" / cid
    osm = json.loads((d / "osm.json").read_text())
    sweep, _, _ = fires.load(cid)
    station = next(b for b in osm["buildings"] if b["kind"] == "fire_station")
    road = next(r for r in osm["roads"] if r.get("name") == AIB_ENTRY_ROAD)
    # whichever end of the road is on the east edge
    entry = max([road["line"][0], road["line"][-1]], key=lambda p: p[0])
    wind = sweep["sweep"]["wind_kmh"]
    cases = [{
        "name": ig["name"],
        "near": ig["near"],
        "ignition": [ig["x"], ig["y"]],
        "radius_m": sweep["sweep"]["ignition_radius_m"],
        # `bearing_deg` is town -> start, so this is the wind that blows the
        # fire at the town (w_dir is where the wind comes FROM)
        "wind_from_deg": float(ig["bearing_deg"]),
        "wind_kmh": wind,
        "minutes": CASE_MINUTES,
    } for ig in sweep["ignitions"]]
    return {
        "scenario": cid,
        "moisture_pct": sweep["sweep"]["moisture_pct"],
        "stations": [
            {"name": f"{station['name']} - {station['locality']}", "pos": station["centroid"]},
            {"name": "Squadra AIB (da fuori, SP 12)", "pos": entry},
        ],
        "roster": ROSTER,
        "cases": cases,
    }


def write(cid: str) -> dict:
    g = build(cid)
    (DATA / "scenarios" / cid / "game.json").write_text(json.dumps(g, indent=1) + "\n")
    return g
