"""Headless fire sweeps on a candidate: only the ignition and the wind vary.

The propagation runs in Rust (`crates/fire/src/bin/fire_sweep.rs`, i.e.
`fire::FireSim`); this module only writes the job list, calls the binary and
reads the arrival-time rasters back.
"""

from __future__ import annotations

import csv
import json
import subprocess
from pathlib import Path

import numpy as np

from .export import DATA, OUT, ROOT
from .grid import CELL_M, CORE_OFF_M, N, to_xy

# The sweep, saved with every run. Fuel moisture is one low value for the whole
# territory and every experiment (02-SCENARIO-FACTORY §A); it is not a lever.
SWEEP = {
    "moisture_pct": 3.0,
    "wind_kmh": 40.0,
    "wind_from_deg": [0, 45, 90, 135, 180, 225, 270, 315],
    "seeds": [1, 2, 3],
    "ignitions": 7,
    "ignition_radius_m": 60.0,
    "core_margin_m": 800.0,  # ignitions only inside the core, this far in
    "minutes": 360,
    "step_s": 60,
    # A wind change at T+60: (from, to) bearings, both at wind_kmh.
    "shifts": [[270, 0], [180, 90]],
    "shift_at_min": 60,
}

BIN = ROOT / "target" / "release" / "fire_sweep"


def fires_dir(cid: str) -> Path:
    return OUT / "fires" / cid


def pick_ignitions(fuel: np.ndarray, n: int, margin_m: float, radius_m: float) -> list[dict]:
    """Farthest-point sampling over burnable cells of the core, away from its
    edge, so the starts cover it without being chosen by hand."""
    m = int((CORE_OFF_M + margin_m) / CELL_M)
    r = int(np.ceil(radius_m / CELL_M))
    burn = (fuel >= 1) & (fuel <= 12)
    ok = np.zeros_like(burn)
    for row in range(m, N - m):
        for col in range(m, N - m):
            # the whole patch must be mostly fuel, or the start may fizzle
            ok[row, col] = burn[row, col] and burn[row - r:row + r + 1, col - r:col + r + 1].mean() > 0.8
    cand = np.argwhere(ok)
    centre = np.array([N / 2, N / 2])
    picks = [cand[np.argmin(((cand - centre) ** 2).sum(1))]]
    d = ((cand - picks[0]) ** 2).sum(1)
    while len(picks) < n:
        picks.append(cand[np.argmax(d)])
        d = np.minimum(d, ((cand - picks[-1]) ** 2).sum(1))
    out = []
    for i, (row, col) in enumerate(picks):
        x, y = to_xy(int(row), int(col))
        out.append({"name": f"I{i + 1}", "row": int(row), "col": int(col), "x": x, "y": y,
                    "fuel": int(fuel[row, col])})
    return out


# Case names, kept from the three-locality layouts (Il Borgo, Il Piano, Le Coste)
# so that earlier cases and tests keep their names.
CASE_PREFIX = {"Castelvento": "Borgo", "Pian dei Grilli": "Piano", "Le Ghiande": "Coste"}


def pick_ignitions_around(fuel: np.ndarray, localities: dict, radius_m: float, dist_m: float = 1000.0,
                          per_locality: int = 3, clear_m: float = 400.0) -> list[dict]:
    """Starts that matter for a town: for each locality, `per_locality` starts
    about `dist_m` from its centre at evenly spaced bearings (rotated per
    locality so they do not align), each the nearest mostly-fuel patch to the
    target point that is at least `clear_m` from every household. Chosen from
    geometry and fuel only, never from how the fire then behaves."""
    r = int(np.ceil(radius_m / CELL_M))
    homes = np.array([p for pts in localities.values() for p in pts])
    burn = (fuel >= 1) & (fuel <= 12)
    rows, cols = np.mgrid[0:N, 0:N]
    X, Y = (cols + 0.5) * CELL_M, N * CELL_M - (rows + 0.5) * CELL_M
    out = []
    for li, (loc, pts) in enumerate(sorted(localities.items())):
        cx, cy = np.mean(pts, axis=0)
        for k in range(per_locality):
            b = np.radians(li * 40 + k * 360 / per_locality)
            tx, ty = cx + dist_m * np.sin(b), cy + dist_m * np.cos(b)
            order = np.argsort(((X - tx) ** 2 + (Y - ty) ** 2).ravel())
            for flat in order[:20000]:
                row, col = divmod(int(flat), N)
                if not burn[row, col] or row < r or col < r or row >= N - r or col >= N - r:
                    continue
                x, y = X[row, col], Y[row, col]
                d = np.hypot(homes[:, 0] - x, homes[:, 1] - y).min()
                if d < clear_m or burn[row - r:row + r + 1, col - r:col + r + 1].mean() <= 0.8:
                    continue
                out.append({"name": f"{CASE_PREFIX.get(loc, loc.split()[-1][:5])}{k + 1}", "row": row, "col": col, "x": float(x),
                            "y": float(y), "fuel": int(fuel[row, col]), "near": loc,
                            "bearing_deg": int(np.degrees(b)) % 360, "dist_to_homes_m": round(float(d))})
                break
    return out


def job_name(ign: str, wind: str, seed: int) -> str:
    return f"{ign}_w{wind}_s{seed}"


def reuse_ignitions(src: str, fuel: np.ndarray, radius_m: float) -> list[dict]:
    """The ignitions of an earlier sweep (the natural terrain), so the built
    world is compared on the same starts. One that now sits mostly on cleared
    ground (a village, a field) is dropped, not moved."""
    meta = json.loads((fires_dir(src) / "sweep.json").read_text())
    r = int(np.ceil(radius_m / CELL_M))
    keep = []
    for ig in meta["ignitions"]:
        patch = fuel[ig["row"] - r:ig["row"] + r + 1, ig["col"] - r:ig["col"] + r + 1]
        if ((patch >= 1) & (patch <= 12)).mean() > 0.8:
            keep.append({**ig, "fuel": int(fuel[ig["row"], ig["col"]])})
    return keep


def run(cid: str, fuel: np.ndarray, sweep: dict | None = None, ignitions_from: str | None = None,
        ignitions: list[dict] | None = None, out_name: str | None = None) -> Path:
    s = {**SWEEP, **(sweep or {})}
    # Always build: a no-op when current, and never a stale binary.
    subprocess.run(["cargo", "build", "--release", "-q", "-p", "fire", "--bin", "fire_sweep"],
                   cwd=ROOT, check=True)
    out = fires_dir(out_name or cid)
    out.mkdir(parents=True, exist_ok=True)
    igns = ignitions or (reuse_ignitions(ignitions_from, fuel, s["ignition_radius_m"]) if ignitions_from
            else pick_ignitions(fuel, s["ignitions"], s["core_margin_m"], s["ignition_radius_m"]))
    lines = []
    for ig in igns:
        for seed in s["seeds"]:
            base = f"{ig['name']} {ig['row']} {ig['col']} {s['ignition_radius_m']} {seed} {s['moisture_pct']}"
            for w in s["wind_from_deg"]:
                name = job_name(ig["name"], str(w), seed)
                lines.append(f"{name} {base.split(' ', 1)[1]} {w} {s['wind_kmh']}")
            for a, b in s["shifts"]:
                name = job_name(ig["name"], f"{a}to{b}", seed)
                lines.append(f"{name} {base.split(' ', 1)[1]} {a} {s['wind_kmh']} "
                             f"{s['shift_at_min']} {b} {s['wind_kmh']}")
    (out / "jobs.txt").write_text("\n".join(lines) + "\n")
    (out / "sweep.json").write_text(json.dumps({"sweep": s, "ignitions": igns}, indent=2) + "\n")
    res = subprocess.run([str(BIN), str(DATA), cid, str(out / "jobs.txt"), str(out),
                          str(s["minutes"]), str(s["step_s"])],
                         check=True, capture_output=True, text=True)
    (out / "results.csv").write_text(res.stdout)
    return out


def load(cid: str) -> tuple[dict, list[dict], dict[str, np.ndarray]]:
    out = fires_dir(cid)
    if not (out / "results.csv").exists():
        raise SystemExit(f"{cid}: mancano gli incendi -- eseguire prima `fires`")
    meta = json.loads((out / "sweep.json").read_text())
    rows = list(csv.DictReader((out / "results.csv").open()))
    arrivals = {r["name"]: np.fromfile(out / f"{r['name']}.i32", dtype="<i4").reshape(N, N) for r in rows}
    return meta, rows, arrivals
