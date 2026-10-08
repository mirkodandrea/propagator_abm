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

from factory import atlas, export, fires, terrain, town, vegetation
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


def cmd_fires(ids, ignitions_from=None):
    for cid in ids:
        _, fuel, _ = export.load(cid)
        t = time.time()
        out = fires.run(cid, fuel, ignitions_from=ignitions_from)
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
    if len(cands) == len(terrain.CANDIDATES):
        atlas.overview(cands, atlas.DOCS / "candidati.png")
    # merge, so re-running one candidate keeps the others' rows
    path = atlas.DOCS / "metriche.json"
    table = {**(json.loads(path.read_text()) if path.exists() else {}), **json.loads(json.dumps(table, default=float))}
    table = dict(sorted(table.items()))
    path.write_text(json.dumps(table, indent=2) + "\n")
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


def cmd_build_town(terrain_id, layout_n, fine_res=True):
    lay = town.LAYOUTS[(terrain_id, layout_n)]
    dem, fuel, params = export.load(terrain_id)
    t = time.time()
    out = town.build(lay, dem, fuel)
    extra = {}
    if fine_res:
        from factory import fine
        # graphics and agents only: the fire keeps the approved 20 m DEM below
        f = fine.build(dem, out, lay.seed)
        extra = {"fine": f["params"], "road_profiles": f["road_stats"]}
        print(json.dumps(f["params"]["fine_vs_fire_dem"]))
    town.write(lay, dem, out, {**params, "layout": layout_n, "town_id": lay.id, "town_seed": lay.seed,
                                "town_notes": out["notes"], **extra}, fine=f if fine_res else None)
    print(f"{lay.id}: {len(out['buildings'])} edifici, {len(out['households'])} famiglie, "
          f"{len(out['people'])} persone, {len(out['roads'])} strade in {time.time() - t:.1f} s")
    print(json.dumps(out["notes"], indent=1))


def cmd_verify(cid):
    """The model's own checks (crates/abm/src/bin/scenario_check.rs)."""
    import subprocess
    subprocess.run(["cargo", "build", "--release", "-q", "-p", "abm", "--bin", "scenario_check"],
                   cwd=export.ROOT, check=True)
    res = subprocess.run([str(export.ROOT / "target" / "release" / "scenario_check"), str(export.DATA), cid],
                         capture_output=True, text=True)
    path = export.DATA / "scenarios" / cid / "check.json"
    path.write_text(res.stdout)
    rep = json.loads(res.stdout)
    for c in rep.get("hard_checks", []):
        print(f"{'OK ' if c['ok'] else 'NO '} {c['name']}: {c.get('detail', '')}")
    print(f"-> {path} (exit {res.returncode})")
    return res.returncode


def cmd_town_fires(cid, nature, ignitions_from=None):
    """The built world and its natural terrain, on the same town-centred starts
    (or on the starts of an earlier sweep, `ignitions_from`, to compare two
    versions of the same town)."""
    built = town.load_built(cid)
    locs = {}
    for h in built["pop"]["households"]:
        locs.setdefault(h["locality"], []).append(tuple(h["pos"]))
    _, fuel_b, _ = export.load(cid)
    _, fuel_n, _ = export.load(nature)
    if ignitions_from:
        igns = fires.reuse_ignitions(ignitions_from, fuel_b, fires.SWEEP["ignition_radius_m"])
    else:
        igns = fires.pick_ignitions_around(fuel_b, locs, fires.SWEEP["ignition_radius_m"])
    # keep only starts that are fuel in both worlds, so the pair is comparable
    igns = [ig for ig in igns if 1 <= fuel_n[ig["row"], ig["col"]] <= 12]
    for name, f, tag in [(cid, fuel_b, cid), (nature, fuel_n, f"{nature}__per_{cid}")]:
        t = time.time()
        fires.run(name, f, ignitions=igns, out_name=tag)
        print(f"{tag}: {len(igns)} inneschi, {time.time() - t:.1f} s")


def cmd_plate(cid, nature):
    from factory import plate
    tt, compare, table = plate.plate(cid, nature)
    print(table)
    print(json.dumps(compare, indent=1))


def cmd_fine_plate(cid, nature):
    from factory import fineplate
    out, diff = fineplate.make(cid, nature, f"{cid}_fase2")
    print(json.dumps({k: v for k, v in out.items() if k != "roads"}, indent=1))
    print(diff)


def cmd_game_cases(cid):
    from factory import cases
    g = cases.write(cid)
    print(f"{cid}/game.json: {len(g['cases'])} casi, {len(g['roster'])} mezzi, stazioni {[x['name'] for x in g['stations']]}")


def cmd_publish(cid):
    from factory import cases
    dst = cases.publish(cid)
    print(f"{cid} -> {dst}")


def main(argv):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("command", choices=["nature", "fires", "atlas", "all", "build-town", "verify", "town-fires", "plate", "fine-plate", "game-cases", "publish"])
    p.add_argument("--terrain", default="t4")
    p.add_argument("--layout", type=int, default=1)
    p.add_argument("--scenario", default="t4_paese")
    p.add_argument("--ignitions-from", default=None, help="riusa gli inneschi dello sweep di un altro scenario")
    p.add_argument("--coarse", action="store_true", help="build-town senza terreno fine (render a 20 m, come in fase 2)")
    p.add_argument("--candidates", default=",".join(c.id for c in terrain.CANDIDATES))
    a = p.parse_args(argv)
    ids = [s.strip() for s in a.candidates.split(",") if s.strip()]
    if a.command == "build-town":
        return cmd_build_town(a.terrain, a.layout, not a.coarse)
    if a.command == "town-fires":
        return cmd_town_fires(a.scenario, a.terrain, a.ignitions_from)
    if a.command == "fine-plate":
        return cmd_fine_plate(a.scenario, a.terrain)
    if a.command == "publish":
        return cmd_publish(a.scenario)
    if a.command == "game-cases":
        return cmd_game_cases(a.scenario)
    if a.command == "plate":
        return cmd_plate(a.scenario, a.terrain)
    if a.command == "verify":
        return sys.exit(cmd_verify(a.scenario))
    if a.command in ("nature", "all"):
        cmd_nature(ids)
    if a.command in ("fires", "all"):
        cmd_fires(ids, a.ignitions_from)
    if a.command in ("atlas", "all"):
        cmd_atlas(ids)


if __name__ == "__main__":
    main(sys.argv[1:])
