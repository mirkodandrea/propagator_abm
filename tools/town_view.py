#!/usr/bin/env python3
"""Rendering semplificato dei paesi dello scenario pubblicato, per giudicarne
il disegno a colpo d'occhio: strade, case (rettangoli orientati), edifici
pubblici e, sullo sfondo, le celle non combustibili (grigio) e la vegetazione.

    python3 tools/town_view.py                  # data/scenarios/rocca_ventosa -> out/town_view/
    python3 tools/town_view.py -s t4_paese7 --data out/factory/data -o out/town_view --half 450 --no-fuel
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib.patches import Polygon  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
SCALE = 1.9  # come li disegna il chiosco (visual_scale::BUILDING_FOOTPRINT); --scale 1 per i dati
KIND = {"house": "#e8a87c", "terrace": "#d1785a", "villa": "#f2d16b", "shop": "#7b6fd0", "apartments": "#b5473a",
        "farm": "#a1887f"}
CIVIC = {"church": "#ffffff", "townhall": "#4caf50", "school": "#4caf50", "fire_station": "#e53935",
         "assembly": "#66bb6a", "pitch": "#9ccc65", "parking": "#90a4ae", "plaza": "#eeeeee", "cemetery": "#78909c",
         "fuel": "#ff9800", "mill": "#8d6e63", "chapel": "#ffffff"}
ROAD = {"secondary": (3.0, "#c0392b"), "tertiary": (2.4, "#e67e22"), "residential": (1.8, "#444444"),
        "unclassified": (1.0, "#666666"), "track": (0.8, "#999999")}


def load(cid, data="data"):
    d = ROOT / data / "scenarios" / cid
    osm = json.loads((d / "osm.json").read_text())
    fg = osm["fire_grid"]
    fuel = np.fromfile(d / "fuel.i32", dtype="<i4")
    fuel = fuel.reshape(fg["rows"], fg["cols"]) if fuel.size == fg["rows"] * fg["cols"] else None
    return osm, fuel, fg["cellsize"]


def draw(ax, osm, fuel, cell, cx, cy, half, title, show_fuel=True):
    x0, x1, y0, y1 = cx - half, cx + half, cy - half, cy + half
    if fuel is not None and show_fuel:
        n = fuel.shape[0]
        ax.imshow(np.where(fuel == 0, 0.0, 1.0), cmap="Greys_r", vmin=-0.4, vmax=1.6, alpha=0.25,
                  extent=(0, n * cell, 0, n * cell), interpolation="nearest", zorder=0)
        veg = np.ma.masked_where(fuel == 0, fuel)
        ax.imshow(veg, cmap="YlGn_r", alpha=0.35, extent=(0, n * cell, 0, n * cell), interpolation="nearest", zorder=0)
    for r in osm["roads"]:
        w, c = ROAD.get(r["class"], (1.0, "#666"))
        xs, ys = zip(*r["line"])
        ax.plot(xs, ys, color=c, lw=w, solid_capstyle="round", zorder=2, ls=":" if not r["drivable"] else "-")
    for b in osm["buildings"]:
        col = KIND.get(b["kind"]) or CIVIC.get(b["kind"], "#cccccc")
        civic = b["kind"] in CIVIC
        ring = b["ring"] if civic or SCALE == 1 else [[b["centroid"][0] + (x - b["centroid"][0]) * SCALE, b["centroid"][1] + (y - b["centroid"][1]) * SCALE] for x, y in b["ring"]]
        ax.add_patch(Polygon(ring, closed=True, fc=col, ec="k", lw=0.5 if not civic else 0.8,
                             alpha=0.55 if b["kind"] in ("assembly", "pitch", "parking", "plaza") else 1.0,
                             zorder=1 if b["kind"] in ("assembly", "pitch", "parking", "plaza") else 3))
    for w_ in osm.get("water", []):
        ax.plot(*w_["pos"], "o", ms=4, color="#1e88e5", zorder=4)
    ax.set_xlim(x0, x1)
    ax.set_ylim(y0, y1)
    ax.set_aspect("equal")
    ax.set_title(title, fontsize=10)
    ax.tick_params(labelsize=7)


def main():
    global SCALE
    ap = argparse.ArgumentParser()
    ap.add_argument("-s", "--scenario", default="rocca_ventosa")
    ap.add_argument("-o", "--out", default="out/town_view")
    ap.add_argument("--half", type=float, default=0, help="mezzo lato della vista in m (0: automatico)")
    ap.add_argument("--no-fuel", action="store_true")
    ap.add_argument("--data", default="data", help="cartella dati (es. out/factory/data per uno scenario appena costruito)")
    ap.add_argument("--scale", type=float, default=SCALE)
    a = ap.parse_args()
    SCALE = a.scale
    osm, fuel, cell = load(a.scenario, a.data)
    out = ROOT / a.out
    out.mkdir(parents=True, exist_ok=True)
    towns = {}
    for b in osm["buildings"]:
        if b["kind"] in KIND and b.get("locality"):
            towns.setdefault(b["locality"], []).append(b["centroid"])
    paths = []
    for name, pts in sorted(towns.items()):
        p = np.array(pts)
        c = np.median(p, axis=0)
        half = a.half or max(250.0, float(np.percentile(np.hypot(*(p - c).T), 95)) + 120)
        fig, ax = plt.subplots(figsize=(9, 9))
        draw(ax, osm, fuel, cell, c[0], c[1], half, f"{name}: {len(pts)} edifici", not a.no_fuel)
        f = out / f"{name.replace(' ', '_')}.png"
        fig.savefig(f, dpi=110, bbox_inches="tight")
        plt.close(fig)
        paths.append(f)
    ncol = 3
    nrow = math.ceil(len(towns) / ncol)
    fig, axs = plt.subplots(nrow, ncol, figsize=(7 * ncol, 7 * nrow))
    for ax, (name, pts) in zip(axs.flat, sorted(towns.items())):
        p = np.array(pts)
        c = np.median(p, axis=0)
        half = a.half or max(250.0, float(np.percentile(np.hypot(*(p - c).T), 95)) + 120)
        draw(ax, osm, fuel, cell, c[0], c[1], half, f"{name}: {len(pts)} edifici", not a.no_fuel)
    for ax in list(axs.flat)[len(towns):]:
        ax.axis("off")
    fig.tight_layout()
    fig.savefig(out / "tutti.png", dpi=90)
    print("\n".join(str(x) for x in paths + [out / "tutti.png"]))


if __name__ == "__main__":
    main()
