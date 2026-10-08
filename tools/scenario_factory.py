#!/usr/bin/env python3
"""Scenario Factory for Rocca Ventosa (docs/02-SCENARIO-FACTORY.md).

Phase 1 commands, in order:

  python tools/scenario_factory.py nature [--candidates t1,t2]   orography + vegetation
  python tools/scenario_factory.py fires  [--candidates ...]     headless sweeps (Rust fire_sweep)
  python tools/scenario_factory.py atlas  [--candidates ...]     PNGs + metrics in docs/factory/fase1
  python tools/scenario_factory.py all                           the three above

Raw output goes to out/factory/ (gitignored, regenerable from the saved seeds and
parameters); the atlas images go to docs/factory/fase1/. Set up once with
`python3 -m venv .venv && .venv/bin/pip install -r tools/requirements.txt`.
"""

from __future__ import annotations

import argparse
import json
import sys
import time

import numpy as np

from factory import atlas, export, fires, terrain, vegetation
from factory.grid import slope_aspect


def cmd_nature(ids):
    for cid in ids:
        c = terrain.by_id(cid)
        dem = terrain.generate(c)
        fuel, stats = vegetation.generate(dem, c.seed)
        slope, _ = slope_aspect(dem)
        params = {"id": c.id, "name": c.name, "notes": c.notes, "seed": c.seed, **c.params,
                  "elev_m": [round(float(dem.min()), 1), round(float(dem.max()), 1)],
                  "slope_deg_p50_p95": [round(float(np.percentile(slope, q)), 1) for q in (50, 95)],
                  **stats}
        out = export.write(c.id, c.name, c.notes, dem, fuel, params)
        print(f"{cid}: {c.name}, quota {params['elev_m']}, pendenza p50/p95 {params['slope_deg_p50_p95']} -> {out}")


def cmd_fires(ids):
    for cid in ids:
        _, fuel, _ = export.load(cid)
        t = time.time()
        out = fires.run(cid, fuel)
        n = sum(1 for _ in (out / "results.csv").open()) - 1
        print(f"{cid}: {n} incendi in {time.time() - t:.1f} s -> {out}")


def cmd_atlas(ids):
    atlas.DOCS.mkdir(parents=True, exist_ok=True)
    cands, table = [], {}
    for cid in ids:
        c = terrain.by_id(cid)
        dem, fuel, params = export.load(cid)
        meta, rows, arrivals = fires.load(cid)
        cands.append((c, dem, fuel))
        atlas.figure_terrain(cid, c.name, dem, fuel, meta["ignitions"], atlas.DOCS / f"{cid}_terreno.png")
        atlas.figure_winds(cid, dem, meta, arrivals, atlas.DOCS / f"{cid}_venti.png")
        picks = atlas.most_wind_sensitive(meta, arrivals)
        atlas.figure_arrivals(cid, dem, meta, arrivals, atlas.DOCS / f"{cid}_arrivi.png", picks)
        atlas.figure_shift(cid, dem, meta, arrivals, atlas.DOCS / f"{cid}_cambio_vento.png", picks)
        table[cid] = {"name": c.name, "params": params, **atlas.metrics(meta, rows, arrivals)}
        print(f"{cid}: atlante scritto")
    atlas.overview(cands, atlas.DOCS / "candidati.png")
    (atlas.DOCS / "metriche.json").write_text(json.dumps(table, indent=2, default=float) + "\n")
    first = next(iter(table.values()))
    marks = list(first["median_ha"])
    hdr = ("| | inneschi attecchiti (>=5 ha a 1 h) | " + " | ".join(f"ha a {k[3:-3]}' (mediana)" for k in marks)
           + " | ha a fine run p10 / p90 | sovrapposizione fra venti (Jaccard) | territorio bruciato almeno una volta"
           " | area cambiata dal cambio di vento | al bordo entro 1 h / 3 h / fine |")
    lines = [hdr, "|" + "---|" * (len(marks) + 7)]
    for cid, m in table.items():
        lines.append(f"| **{cid}** {m['name']} | {m['established_60'] * 100:.0f}% | "
                     + " | ".join(f"{m['median_ha'][k]:.0f}" for k in marks)
                     + f" | {m['ha180_p10']:.0f} / {m['ha180_p90']:.0f} | {m['wind_jaccard']:.2f} | "
                     f"{m['ever_burnt_share'] * 100:.0f}% | {m['shift_changed_share'] * 100:.0f}% | "
                     f"{m['edge_share_60'] * 100:.0f}% / {m['edge_share_180'] * 100:.0f}% / {m['edge_share'] * 100:.0f}% |")
    (atlas.DOCS / "metriche.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


def main(argv):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("command", choices=["nature", "fires", "atlas", "all"])
    p.add_argument("--candidates", default=",".join(c.id for c in terrain.CANDIDATES))
    a = p.parse_args(argv)
    ids = [s.strip() for s in a.candidates.split(",") if s.strip()]
    if a.command in ("nature", "all"):
        cmd_nature(ids)
    if a.command in ("fires", "all"):
        cmd_fires(ids)
    if a.command in ("atlas", "all"):
        cmd_atlas(ids)


if __name__ == "__main__":
    main(sys.argv[1:])
