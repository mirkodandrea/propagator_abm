"""Generate a synthetic population placed on the real OSM building stock.

The *placement* is real (actual Spotorno footprints, actual road access, actual
distance to fuel); the *people* are synthetic, sampled from Liguria-level
demographic anchors. Nothing here claims to be a real census microdata set.

Per-person attributes are chosen for a wildfire evacuation model rather than
borrowed from a flood one: what drives behaviour in a fire is warning receipt,
cue-seeking, milling time, whether the household has a vehicle and a defensible
house, and whether anyone present needs assistance to move.

Demographic anchors (Liguria is Italy's oldest region):
  - mean household size 2.0 (ISTAT; Liguria lowest in Italy)
  - 28.9% of residents aged 65+ (ISTAT)
  - ~60 cars per 100 residents (ISTAT, Liguria below national average)
Behavioural priors are from the wildfire evacuation literature (PADM;
McLennan et al. on "prepare, stay and defend or leave early"); they are
plausible, not calibrated to Spotorno.
"""

import argparse
import json
import math
from pathlib import Path

import numpy as np
import rasterio

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data"

# OSM building kinds that plausibly house residents.
RESIDENTIAL = {
    "yes", "house", "apartments", "residential", "detached",
    "semidetached_house", "terrace", "cabin", "bungalow", "villa",
}
# Kinds that are definitely not dwellings.
NON_RESIDENTIAL = {
    "industrial", "church", "chapel", "school", "retail", "commercial",
    "roof", "ruins", "construction", "garage", "garages", "shed", "hut",
    "greenhouse", "warehouse", "hotel", "civic", "public", "hospital",
    "train_station", "cemetery", "toilets", "carport", "service",
}

BURNABLE = set(range(1, 13))  # eu_fuel12: 0 / -1 are non-vegetated


def polygon_area(ring: list) -> float:
    p = np.asarray(ring, dtype=np.float64)
    x, y = p[:, 0], p[:, 1]
    return abs(np.dot(x, np.roll(y, 1)) - np.dot(y, np.roll(x, 1))) / 2.0


def source_fuel_tif(scenario_dir: Path, scenario_id: str) -> Path:
    """`<scenario_dir>/fuel.tif`, falling back to Spotorno's original flat
    layout (`data/spotorno_fuel.tif`) so the shipped scenario re-bakes
    without needing a fresh COG clip."""
    local = scenario_dir / "fuel.tif"
    if local.exists():
        return local
    legacy = DATA / f"{scenario_id}_fuel.tif"
    if scenario_id == "spotorno" and legacy.exists():
        return legacy
    raise SystemExit(f"missing {local} (run scripts/clip_cogs.py first)")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenario", default="spotorno", help="scenario id under data/scenarios/")
    ap.add_argument("--people", type=int, default=1500, help="target population")
    ap.add_argument("--seed", type=int, default=42)
    args = ap.parse_args()
    rng = np.random.default_rng(args.seed)

    scenario_dir = DATA / "scenarios" / args.scenario
    osm = json.loads((scenario_dir / "osm.json").read_text())
    with rasterio.open(source_fuel_tif(scenario_dir, args.scenario)) as src:
        fuel = src.read(1)
    cell = osm["fire_grid"]["cellsize"]
    rows, cols = fuel.shape
    ox, oy = 0.0, 0.0  # world frame already has origin at SW corner

    # Distance from every cell to the nearest burnable cell, in metres. This is
    # the WUI exposure that decides which houses are actually at risk.
    from scipy.ndimage import distance_transform_edt

    burnable = np.isin(fuel, list(BURNABLE))
    dist_to_fuel = distance_transform_edt(~burnable) * cell

    def world_to_cell(x: float, y: float) -> tuple[int, int]:
        """World metres (origin SW, +y north) -> fire-grid (row, col)."""
        col = int(np.clip(x / cell, 0, cols - 1))
        row = int(np.clip((rows * cell - y) / cell, 0, rows - 1))
        return row, col

    # --- select dwellings ------------------------------------------------
    dwellings = []
    for b in osm["buildings"]:
        kind = b["kind"]
        if kind in NON_RESIDENTIAL or kind not in RESIDENTIAL:
            continue
        area = polygon_area(b["ring"])
        if area < 30 or area > 3000:  # sheds and big sheds
            continue
        levels = b.get("levels")
        try:
            levels = max(1, min(8, int(float(levels))))
        except (TypeError, ValueError):
            levels = 3 if kind == "apartments" else 2
        x, y = b["centroid"]
        r, c = world_to_cell(x, y)
        # dwelling units scale with floor area; ~110 m2 per unit
        units = max(1, int(round(area * levels / 110.0)))
        if kind == "house":
            units = min(units, 2)
        dwellings.append(
            {
                "osm_id": b["id"],
                "kind": kind,
                "pos": [x, y],
                "area_m2": round(area, 1),
                "levels": levels,
                "units": units,
                "cell": [r, c],
                "dist_to_fuel_m": round(float(dist_to_fuel[r, c]), 1),
                "fuel_at_site": int(fuel[r, c]),
                # carried straight from the OSM bake -- see fetch_osm.py's
                # `assign_addresses`. Not randomised, so this cannot shift the
                # RNG stream that drives everything below.
                "address": b.get("address"),
                "locality": b.get("locality"),
            }
        )

    total_units = sum(d["units"] for d in dwellings)
    print(f"dwelling buildings : {len(dwellings)}  ({total_units} units)")

    # --- occupy a subset to hit the target population --------------------
    mean_hh = 2.0
    n_households = max(1, int(round(args.people / mean_hh)))
    n_households = min(n_households, total_units)

    # Weight occupancy toward the WUI edge so the scenario has people at risk,
    # but keep it a preference rather than a rule.
    weights = np.array(
        [1.0 + 1.5 * math.exp(-d["dist_to_fuel_m"] / 300.0) for d in dwellings]
    )
    weights = np.repeat(
        weights / weights.sum(), [d["units"] for d in dwellings]
    )
    owner = np.repeat(np.arange(len(dwellings)), [d["units"] for d in dwellings])
    weights = weights / weights.sum()
    chosen = rng.choice(len(owner), size=n_households, replace=False, p=weights)

    households, people = [], []
    pid = 0
    for hid, unit in enumerate(sorted(chosen.tolist())):
        d = dwellings[owner[unit]]
        # household size: zero-truncated, mean ~2.0, long right tail
        size = int(np.clip(rng.geometric(0.5), 1, 7))

        has_vehicle = bool(rng.random() < (0.62 if size == 1 else 0.88))
        vehicles = 0 if not has_vehicle else int(1 + (rng.random() < 0.35))

        # Defensible space: a real function of how far the fuel is, plus effort.
        cleared = float(np.clip(
            rng.beta(2, 3) + 0.3 * (d["dist_to_fuel_m"] > 100), 0, 1
        ))

        members = []
        for _ in range(size):
            # age: 28.9% over 65, bimodal working/retired
            u = rng.random()
            if u < 0.289:
                age = int(rng.integers(65, 95))
            elif u < 0.45:
                age = int(rng.integers(0, 25))
            else:
                age = int(rng.integers(25, 65))
            impaired = bool(
                age >= 80 or (age >= 65 and rng.random() < 0.25)
                or rng.random() < 0.03
            )
            members.append(
                {
                    "id": pid,
                    "household": hid,
                    "age": age,
                    # m/s on foot; the old and the very young are slower
                    "walk_speed": round(
                        float(np.clip(rng.normal(1.3, 0.15), 0.4, 1.9))
                        * (0.55 if impaired else 1.0)
                        * (0.7 if age < 8 else 1.0),
                        2,
                    ),
                    "needs_assistance": impaired,
                    "at_home": bool(rng.random() < 0.75),
                }
            )
            pid += 1

        households.append(
            {
                "id": hid,
                "building": d["osm_id"],
                "pos": d["pos"],
                "cell": d["cell"],
                "size": size,
                "vehicles": vehicles,
                "dist_to_fuel_m": d["dist_to_fuel_m"],
                "address": d["address"],
                "locality": d["locality"],
                # --- wildfire-specific behavioural state ---
                # baseline risk perception, raised by cues (smoke, embers)
                "risk_perception": round(float(rng.beta(2, 4)), 3),
                "prior_fire_experience": bool(rng.random() < 0.30),
                # how the household first hears about it
                "warning_channel": str(
                    rng.choice(
                        ["mobile_alert", "neighbour", "siren", "self_observed", "none"],
                        p=[0.45, 0.20, 0.10, 0.20, 0.05],
                    )
                ),
                # trust that official advice is worth acting on
                "trust_authority": round(float(rng.beta(4, 2)), 3),
                # pre-formed intent; revised during the event
                "intent": str(
                    rng.choice(
                        ["leave_early", "wait_and_see", "stay_defend"],
                        p=[0.35, 0.50, 0.15],
                    )
                ),
                # minutes of milling/preparation before actually moving
                "prep_time_min": round(float(np.clip(rng.gamma(3, 4), 1, 60)), 1),
                "defensible_space": round(cleared, 3),
                "has_pets_livestock": bool(rng.random() < 0.28),
                "status": "normal",
                "members": [m["id"] for m in members],
            }
        )
        people.extend(members)

    at_risk = sum(1 for h in households if h["dist_to_fuel_m"] < 100)
    no_car = sum(1 for h in households if h["vehicles"] == 0)
    assist = sum(1 for p in people if p["needs_assistance"])

    out = {
        "synthetic": True,
        "note": (
            "Placement on real OSM footprints; people sampled from Liguria "
            "demographic anchors. Behavioural priors are plausible, not calibrated."
        ),
        "seed": args.seed,
        "units": "metres, origin at SW corner, +x east +y north",
        "world_size_m": osm["world_size_m"],
        "fire_grid": osm["fire_grid"],
        "counts": {
            "dwellings": len(dwellings),
            "households": len(households),
            "people": len(people),
        },
        "dwellings": dwellings,
        "households": households,
        "people": people,
    }
    path = scenario_dir / "population.json"
    path.write_text(json.dumps(out))

    print(f"households         : {len(households)}")
    print(f"people             : {len(people)}  (mean hh {len(people) / len(households):.2f})")
    print(f"  within 100 m of burnable fuel : {at_risk} households ({100 * at_risk / len(households):.0f}%)")
    print(f"  no vehicle                    : {no_car} households ({100 * no_car / len(households):.0f}%)")
    print(f"  need assistance to move       : {assist} people ({100 * assist / len(people):.0f}%)")
    print(f"  aged 65+                      : {sum(1 for p in people if p['age'] >= 65)} people")
    print(f"\nwrote {path} ({path.stat().st_size / 1e6:.1f} MB)")


if __name__ == "__main__":
    main()
