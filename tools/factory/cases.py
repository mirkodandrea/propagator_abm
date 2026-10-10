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
# The "_gira" variant of each case: at this minute the wind turns to drive the
# fire at the nearest other locality (01-SPEC-GIOCO §3: a wind change that
# overturns the exposure is one of the crises the game is about).
SHIFT_AT_MIN = 45
# The cases the kiosk plays, in its order (checkpoint 4, 2026-10-08): the main
# dilemma, the non-obvious one, the introductory one. The others stay in
# game.json for the headless runner and the operator.
FEATURED = ["Forna3_gira", "Coste2_gira", "Rocco3"]


def build(cid: str) -> dict:
    d = DATA / "scenarios" / cid
    osm = json.loads((d / "osm.json").read_text())
    sweep, _, _ = fires.load(cid)
    station = next(b for b in osm["buildings"] if b["kind"] == "fire_station")
    road = next(r for r in osm["roads"] if r.get("name") == AIB_ENTRY_ROAD)
    # whichever end of the road is on the east edge
    entry = max([road["line"][0], road["line"][-1]], key=lambda p: p[0])
    wind = sweep["sweep"]["wind_kmh"]
    pop = json.loads((d / "population.json").read_text())
    centres = {}
    for h in pop["households"]:
        centres.setdefault(h["locality"], []).append(h["pos"])
    centres = {k: (sum(p[0] for p in v) / len(v), sum(p[1] for p in v) / len(v)) for k, v in centres.items()}
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
    cases += [shifted(c, centres) for c in cases]
    return {
        "scenario": cid,
        "moisture_pct": sweep["sweep"]["moisture_pct"],
        "stations": [
            {"name": f"{station['name']} - {station['locality']}", "pos": station["centroid"]},
            {"name": "Squadra AIB (da fuori, SP 12)", "pos": entry},
        ],
        "roster": ROSTER,
        "featured": [n for n in FEATURED if any(c["name"] == n for c in cases)],
        "cases": cases,
    }


def shifted(case: dict, centres: dict) -> dict:
    """The same fire, with the wind turning at SHIFT_AT_MIN to blow from the
    start toward the nearest locality other than the one it threatened."""
    import math
    x, y = case["ignition"]
    other = min((k for k in centres if k != case["near"]),
                key=lambda k: math.hypot(centres[k][0] - x, centres[k][1] - y))
    tx, ty = centres[other]
    # bearing the wind blows TOWARD, then FROM = +180
    toward = math.degrees(math.atan2(tx - x, ty - y)) % 360
    return {**case, "name": case["name"] + "_gira",
            "shift": {"at_min": SHIFT_AT_MIN, "wind_from_deg": round((toward + 180) % 360, 1),
                      "wind_kmh": case["wind_kmh"], "toward": other}}


def write(cid: str) -> dict:
    g = build(cid)
    (DATA / "scenarios" / cid / "game.json").write_text(json.dumps(g, indent=1) + "\n")
    return g


# The game has one territory, published under one id in the repository's data/.
GAME_ID = "rocca_ventosa"


def publish(cid: str):
    """Copy the built town into `data/scenarios/rocca_ventosa`, the one
    scenario the game loads, with its id and `game.json` rewritten."""
    import shutil
    from .export import ROOT

    src = DATA / "scenarios" / cid
    dst = ROOT / "data" / "scenarios" / GAME_ID
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(src, dst, ignore=shutil.ignore_patterns("check.json"))
    meta = json.loads((dst / "scenario.json").read_text())
    meta.update({"id": GAME_ID, "is_dev": False})
    meta.pop("vr_palette", None)
    (dst / "scenario.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False) + "\n")
    write_game(dst, cid)
    return dst


def write_game(dst, cid: str) -> None:
    g = build(cid)
    g["scenario"] = GAME_ID
    (dst / "game.json").write_text(json.dumps(g, indent=1) + "\n")
