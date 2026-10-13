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
# The cases the kiosk plays, in its order: the main dilemma, the non-obvious
# one, the introductory one. Since 2026-10-13 they are the only ones in
# game.json, renamed after their level (LEVEL_IDS); the others go to
# prove.json, for the headless trials and the tests only. Re-picked on layout 5 with the messengers
# (2026-10-12, docs/fase5/livelli_l5/README.md): Forna3_gira takes hold in one
# seed out of three there, and Coste2_gira scores the same with every plan.
FEATURED = ["Borgo2_gira", "Terra1_gira", "Rocco3"]
# What the visitor chooses at the start (user, 2026-10-10: «facile, medio,
# esperto»): one featured case per level, from the role each was chosen for.
LEVELS = [
    {"name": "Facile", "case": "Rocco3", "about": "Un paese minacciato, il vento non cambia."},
    {"name": "Medio", "case": "Terra1_gira", "about": "Più paesi in pericolo e il vento che gira."},
    {"name": "Esperto", "case": "Borgo2_gira", "about": "Il fuoco punta due paesi e le squadre non bastano per tutti."},
]
# Who carries the orders to the door (`rocca::messengers`): variant A of
# docs/fase5/messaggeri/README.md, chosen by the user on 2026-10-12. Four
# teams, one household a minute each, mobile alerts still reach their
# households, the teams shared out evenly (by priority on a tie).
MESSENGERS = {"teams": 4, "per_min": 1.0, "broadcast": True, "by_priority": False}
# Evacuation needs the mayor's authorisation (`rocca::mayor`, user
# 2026-10-13, docs/fase6/sindaca/README.md): she gives it when the front could
# reach a district within 20 minutes.
MAYOR = {"lead_s": 1200}
# The game's cases are named after their level (user, 2026-10-13); the old
# name stays as `alias`, so the earlier reports still replay.
LEVEL_IDS = {"Rocco3": "easy", "Terra1_gira": "medium", "Borgo2_gira": "expert"}


def split(g: dict) -> tuple[dict, dict]:
    """`game.json` with only the levels' cases, renamed, in the kiosk's
    order; and `prove.json` with the rest."""
    game = [{**c, "name": LEVEL_IDS[c["name"]], "alias": c["name"]}
            for n in FEATURED for c in g["cases"] if c["name"] == n and n in LEVEL_IDS]
    trials = [c for c in g["cases"] if c["name"] not in LEVEL_IDS]
    g = {**g, "cases": game,
         "featured": [LEVEL_IDS[n] for n in g["featured"] if n in LEVEL_IDS],
         "levels": [{**lv, "case": LEVEL_IDS[lv["case"]]} for lv in g["levels"] if lv["case"] in LEVEL_IDS]}
    return g, {"about": "Fires for the headless trials and tests only (user, 2026-10-13): "
                        "the game plays the three levels in game.json.", "cases": trials}


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
        "levels": [lv for lv in LEVELS if any(c["name"] == lv["case"] for c in cases)],
        "messengers": MESSENGERS,
        "mayor": MAYOR,
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
    game, trials = split(g)
    (DATA / "scenarios" / cid / "game.json").write_text(json.dumps(game, indent=1) + "\n")
    (DATA / "scenarios" / cid / "prove.json").write_text(json.dumps(trials, indent=1) + "\n")
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
    game, trials = split(g)
    (dst / "game.json").write_text(json.dumps(game, indent=1) + "\n")
    (dst / "prove.json").write_text(json.dumps(trials, indent=1) + "\n")
