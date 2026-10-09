"""Checkpoint plate for the fine resolution: the 5 m terrain against the 20 m
one, the pass road's profile, and whether the fire still does what it did at
checkpoint 2 on the same starts (phase-2 sweep kept as `t4_paese_fase2`)."""

from __future__ import annotations

import json

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib.colors import ListedColormap  # noqa: E402

from . import fine, fires, plate  # noqa: E402
from . import roads as R  # noqa: E402
from .export import DATA, ROOT, load as load_terrain  # noqa: E402
from .grid import CELL_M, WORLD_M  # noqa: E402

DOCS = ROOT / "docs" / "factory" / "fine"
COVER_COLOURS = ["#00000000", "#c0392b", "#8d6e63", "#222222", "#bdbdbd", "#7cb342", "#0277bd"]


def _hs(z, cell):
    gy, gx = np.gradient(z, cell)
    az, alt = np.radians(315), np.radians(45)
    sl = np.arctan(np.hypot(gx, gy))
    asp = np.arctan2(-gx, gy)
    return np.sin(alt) * np.cos(sl) + np.cos(alt) * np.sin(sl) * np.cos(az - asp)


def load_fine(cid):
    d = DATA / "scenarios" / cid
    meta = json.loads((d / "render_terrain.json").read_text())
    terr = np.fromfile(d / "render_terrain.f32", dtype="<f4").reshape(meta["rows"], meta["cols"])
    cm = json.loads((d / "cover.json").read_text())
    cov = np.fromfile(d / "cover.u8", dtype="u1").reshape(cm["rows"], cm["cols"])
    params = json.loads((d / "params.json").read_text())
    return terr, cov, params


def figure_terrain(cid, nature, views, path):
    osm, pop, check, dem, fuel = plate.load(cid)
    terr, cov, _ = load_fine(cid)
    dem_n, _, _ = load_terrain(nature)
    h20, h5 = _hs(dem_n, CELL_M), _hs(terr.astype(float), fine.FINE_M)
    half = fine.FINE_M / 2
    fig, axs = plt.subplots(2, len(views), figsize=(6 * len(views), 12))
    for k, (name, (cx, cy), size) in enumerate(views):
        x0, x1, y0, y1 = cx - size / 2, cx + size / 2, cy - size / 2, cy + size / 2
        ax = axs[0, k]
        ax.imshow(h20, cmap="gray", extent=(0, WORLD_M, 0, WORLD_M), interpolation="nearest")
        for r in osm["roads"]:
            xs, ys = zip(*r["line"])
            ax.plot(xs, ys, color="#c0392b", lw=0.8, alpha=0.8)
        ax.set_title(f"{name}: DEM del fuoco, 20 m (approvato in fase 1)", fontsize=9)
        ax = axs[1, k]
        ax.imshow(h5, cmap="gray", extent=(-half, WORLD_M + half, -half, WORLD_M + half), interpolation="bilinear")
        ax.imshow(np.ma.masked_where(cov == 0, cov), cmap=ListedColormap(COVER_COLOURS), vmin=-0.5, vmax=6.5,
                  alpha=0.55, extent=(0, WORLD_M, 0, WORLD_M), interpolation="nearest")
        ax.set_title(f"{name}: terreno a 5 m con sedi stradali, piazzole e copertura", fontsize=9)
        for a in axs[:, k]:
            a.set_xlim(x0, x1)
            a.set_ylim(y0, y1)
            a.tick_params(labelsize=6)
    handles = [plt.Rectangle((0, 0), 1, 1, color=c, label=n) for c, n in
               zip(COVER_COLOURS[1:], fine.COVER_NAMES[1:])]
    axs[1, 0].legend(handles=handles, fontsize=7, loc="lower left", framealpha=0.9)
    fig.tight_layout()
    fig.savefig(path, dpi=75)
    plt.close(fig)


def figure_profile(cid, nature, road_name, path):
    osm, *_ = plate.load(cid)
    terr, _, _ = load_fine(cid)
    dem_n, _, _ = load_terrain(nature)
    road = next(r for r in osm["roads"] if r["name"] == road_name)
    pts = np.array(R.resample([tuple(p) for p in road["line"]], fine.FINE["station_m"]))
    s = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(pts, axis=0).T))])
    z_road = fine.sample(terr.astype(float), pts)
    z_20 = np.array([R.elevation(dem_n, x, y) for x, y in pts])

    def grade(z, w=20.0):  # over 20 m stretches, as a driver feels it
        k = int(w / fine.FINE["station_m"])
        return 100 * np.abs(z[k:] - z[:-k]) / (s[k:] - s[:-k]), s[k // 2:-k + k // 2]

    fig, axs = plt.subplots(2, 1, figsize=(14, 7), sharex=True)
    axs[0].plot(s / 1000, z_20, color="#999", lw=1, label="terreno 20 m sotto il tracciato")
    axs[0].plot(s / 1000, z_road, color="#c0392b", lw=1.4, label="piano stradale sul terreno 5 m")
    axs[0].set_ylabel("quota (m)")
    axs[0].legend(fontsize=8)
    for z, col, lab in [(z_20, "#999", "terreno 20 m"), (z_road, "#c0392b", "strada 5 m")]:
        g, sg = grade(z)
        axs[1].plot(sg / 1000, g, color=col, lw=1, label=f"{lab}: p95 {np.percentile(g, 95):.0f} %")
    axs[1].axhline(14, color="k", ls=":", lw=0.8)
    axs[1].set_ylabel("pendenza su 20 m (%)")
    axs[1].set_xlabel("km da Castelvento")
    axs[1].legend(fontsize=8)
    fig.suptitle(f"{road_name}: profilo longitudinale", fontsize=10)
    fig.tight_layout()
    fig.savefig(path, dpi=80)
    plt.close(fig)


def compare_fires(old: str, new: str, path):
    """Same ignitions, winds and seeds: burnt area and burnt footprint, phase 2
    against now. The fire DEM is the same; only the 20 m fuel can differ, where
    the rerouted provincial roads are painted as fuel breaks."""
    _, rows_o, arr_o = fires.load(old)
    _, rows_n, arr_n = fires.load(new)
    ro = {r["name"]: r for r in rows_o}
    pairs, jac = [], []
    for r in rows_n:
        if r["name"] not in ro:
            continue
        pairs.append([float(ro[r["name"]]["ha_120min"]), float(r["ha_120min"]),
                      float(ro[r["name"]]["ha_360min"]), float(r["ha_360min"])])
        a, b = arr_o[r["name"]], arr_n[r["name"]]
        a6, b6 = (a >= 0) & (a <= 21600), (b >= 0) & (b <= 21600)
        u = (a6 | b6).sum()
        if u:
            jac.append((a6 & b6).sum() / u)
    p = np.array(pairs)
    fig, axs = plt.subplots(1, 3, figsize=(16, 5))
    for ax, (i, j, lab) in zip(axs[:2], [(0, 1, "2 h"), (2, 3, "6 h")]):
        ax.scatter(p[:, i], p[:, j], s=8, alpha=0.6)
        m = max(p[:, i].max(), p[:, j].max())
        ax.plot([0, m], [0, m], "k:", lw=0.8)
        ax.set_xlabel(f"ha a {lab}, fase 2 (terreno 20 m)")
        ax.set_ylabel(f"ha a {lab}, ora (strade ritracciate)")
        ax.set_title(f"area bruciata a {lab}, {len(p)} incendi", fontsize=9)
    axs[2].hist(jac, bins=20, color="#888")
    axs[2].set_xlabel("sovrapposizione dell'area bruciata a 6 h (Jaccard)")
    axs[2].set_title("stessa forma? (1 = identica)", fontsize=9)
    fig.tight_layout()
    fig.savefig(path, dpi=80)
    plt.close(fig)
    ratio = p[:, 3] / np.maximum(p[:, 2], 1)
    return {"runs": len(p),
            "ha_2h_p50": [float(np.median(p[:, 0])), float(np.median(p[:, 1]))],
            "ha_6h_p50": [float(np.median(p[:, 2])), float(np.median(p[:, 3]))],
            "ratio_6h_p10_p50_p90": [float(np.percentile(ratio, q)) for q in (10, 50, 90)],
            "jaccard_6h_p10_p50_p90": [float(np.percentile(jac, q)) for q in (10, 50, 90)]}


def threat_diff(old_json, new_json):
    o = json.loads(old_json.read_text())["threat"]
    n = json.loads(new_json.read_text())["threat"]
    lines = ["| località | " + " | ".join(plate.atlas.WIND_NAMES[int(w)].split()[0] for w in n[next(iter(n))])
             + " |", "|---|" + "---|" * len(n[next(iter(n))])]
    for loc in n:
        cells = []
        for w in n[loc]:
            a, b = o[loc][w]["by_2h"] * 100, n[loc][w]["by_2h"] * 100
            cells.append(f"{a:.0f} → {b:.0f}" if round(a) != round(b) else f"{b:.0f}")
        lines.append(f"| {loc} | " + " | ".join(cells) + " |")
    return "\n".join(lines)


def make(cid: str, nature: str, old: str):
    DOCS.mkdir(parents=True, exist_ok=True)
    _, pop, _, _, fuel = plate.load(cid)
    coste = np.array([h["pos"] for h in pop["households"] if h["locality"] == "Le Ghiande"]).mean(0)
    views = [("Castelvento", (5000, 5150), 900), ("tornanti del versante NE", (4000, 4750), 900),
             ("Le Ghiande", tuple(coste), 900)]
    figure_terrain(cid, nature, views, DOCS / "terreno_fine.png")
    figure_profile(cid, nature, "Strada del Passo", DOCS / "profilo_passo.png")
    fire_cmp = compare_fires(old, cid, DOCS / "confronto_incendi.png")
    tt, nat_cmp, table = plate.plate(cid, nature, docs=DOCS)
    _, cov, params = load_fine(cid)
    cvf = fine.cover_vs_fuel(cov, fuel)
    cvf.pop("built_share")
    diff = threat_diff(plate.DOCS / "metriche_paese.json", DOCS / "metriche_paese.json")
    (DOCS / "minaccia_confronto.md").write_text(diff + "\n")
    out = {"fires_phase2_vs_fine": fire_cmp, "nature_vs_built": nat_cmp, "cover_vs_fuel": cvf,
           "fine_vs_fire_dem": params["fine"]["fine_vs_fire_dem"], "roads": params["road_profiles"]}
    (DOCS / "metriche_fine.json").write_text(json.dumps(out, indent=1) + "\n")
    return out, diff
