"""Checkpoint-2 plate: the town over the main propagations, and which wind
threatens which locality how soon. Everything here is read back from the
scenario files and the Rust sweep; nothing is simulated in Python.

"Threatened" here is a *proxy* for the plate only: fire within 150 m of a
household, from the arrival-time raster (houses sit on non-burnable cells and
never ignite themselves, TECHNICAL-FINDINGS #2). It is not the structure
exposure model and must not be shown to players as damage.
"""

from __future__ import annotations

import json

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib.colors import ListedColormap  # noqa: E402
from matplotlib.lines import Line2D  # noqa: E402

from . import atlas, fires  # noqa: E402
from .export import DATA, ROOT  # noqa: E402
from .grid import CELL_M, N, WORLD_M, hillshade, to_cell  # noqa: E402

DOCS = ROOT / "docs" / "factory" / "fase2"
NEAR_M = 150.0
ROAD_STYLE = {"secondary": ("#c0392b", 2.2), "tertiary": ("#e67e22", 1.8), "unclassified": ("#7f6a4f", 0.9),
              "residential": ("#555555", 1.0), "track": ("#8d6e63", 0.8)}
LOC_COLOURS = {"Castelvento": "#1f4e9c", "Pian dei Grilli": "#6a1b9a", "Le Ghiande": "#00838f"}


def load(cid):
    d = DATA / "scenarios" / cid
    osm = json.loads((d / "osm.json").read_text())
    pop = json.loads((d / "population.json").read_text())
    check = json.loads((d / "check.json").read_text()) if (d / "check.json").exists() else {}
    dem = np.fromfile(d / "dem.f64", dtype="<f8").reshape(N, N)
    fuel = np.fromfile(d / "fuel.i32", dtype="<i4").reshape(N, N)
    return osm, pop, check, dem, fuel


def near_mask(points, radius_m):
    """Fire cells within `radius_m` of any of `points`."""
    m = np.zeros((N, N), dtype=bool)
    k = int(np.ceil(radius_m / CELL_M))
    yy, xx = np.mgrid[-k:k + 1, -k:k + 1]
    disc = (xx ** 2 + yy ** 2) * CELL_M ** 2 <= radius_m ** 2
    for x, y in points:
        r, c = to_cell(x, y)
        r0, c0 = r - k, c - k
        sl = (slice(max(0, r0), min(N, r0 + 2 * k + 1)), slice(max(0, c0), min(N, c0 + 2 * k + 1)))
        sub = disc[sl[0].start - r0:sl[0].stop - r0, sl[1].start - c0:sl[1].stop - c0]
        m[sl] |= sub
    return m


def threat_table(pop, meta, arrivals):
    """Per locality and wind: share of runs (ignitions x seeds) with fire within
    NEAR_M of the locality's households by 1, 2 and 6 h, and the median first time."""
    s = meta["sweep"]
    locs = sorted({h["locality"] for h in pop["households"]})
    masks = {loc: near_mask([h["pos"] for h in pop["households"] if h["locality"] == loc], NEAR_M) for loc in locs}
    out = {}
    for loc in locs:
        out[loc] = {}
        for w in s["wind_from_deg"]:
            firsts = []
            for ig in meta["ignitions"]:
                for seed in s["seeds"]:
                    a = arrivals[f"{ig['name']}_w{w}_s{seed}"][masks[loc]]
                    a = a[a >= 0]
                    firsts.append(a.min() / 60 if a.size else np.inf)
            f = np.array(firsts)
            out[loc][w] = {"by_1h": float(np.mean(f <= 60)), "by_2h": float(np.mean(f <= 120)),
                           "by_6h": float(np.mean(f <= 360)),
                           "median_min": float(np.median(f[np.isfinite(f)])) if np.isfinite(f).any() else None}
    return out


def draw_town(ax, osm, pop, check, labels=True):
    for r in osm["roads"]:
        col, lw = ROAD_STYLE.get(r["class"], ("#555", 1.0))
        xs, ys = zip(*r["line"])
        ax.plot(xs, ys, color=col, lw=lw, ls=":" if r.get("track") and not r["drivable"] else "-", zorder=5)
    for b in osm["buildings"]:
        xs, ys = zip(*b["ring"])
        loc = b.get("locality")
        civic = b["id"] >= 100000
        ax.fill(xs, ys, color="#333333" if civic else LOC_COLOURS.get(loc, "#444"), zorder=6, lw=0)
    for wtr in osm["water"]:
        ax.plot(*wtr["pos"], marker="o" if wtr["kind"] == "hydrant" else "s", ms=4,
                color="#0277bd", mec="white", mew=0.5, zorder=7)
    for ref in check.get("refuges", {}).get("list", []):
        if ref["kind"] == "clearing":
            ax.plot(ref["x"], ref["y"], marker="P", ms=11, color="#2e7d32", mec="white", zorder=8)
        else:
            ax.plot(ref["x"], ref["y"], marker=">", ms=9, color="#2e7d32", mec="white", zorder=8)
    if labels:
        for loc, colr in LOC_COLOURS.items():
            pts = np.array([h["pos"] for h in pop["households"] if h["locality"] == loc])
            if len(pts):
                cx, cy = pts.mean(0)
                n = len(pts)
                ax.text(cx + 120, cy + 160, f"{loc}\n{n} famiglie", color=colr, fontsize=8, weight="bold",
                        bbox=dict(boxstyle="round,pad=0.2", fc="white", ec=colr, alpha=0.85), zorder=9)


def legend(ax):
    h = [Line2D([], [], color=c, lw=lw, label=lab) for lab, (c, lw) in
         [("strada provinciale", ROAD_STYLE["secondary"]), ("strada del passo", ROAD_STYLE["tertiary"]),
          ("vie del paese", ROAD_STYLE["residential"]), ("accessi poderi", ROAD_STYLE["unclassified"])]]
    h += [Line2D([], [], color=ROAD_STYLE["track"][0], lw=1, ls=":", label="strada forestale (solo squadre)"),
          Line2D([], [], marker="P", ls="", color="#2e7d32", ms=9, label="area di attesa (rifugio del modello)"),
          Line2D([], [], marker=">", ls="", color="#2e7d32", ms=8, label="uscita dal territorio"),
          Line2D([], [], marker="o", ls="", color="#0277bd", ms=5, label="idrante"),
          Line2D([], [], marker="s", ls="", color="#0277bd", ms=5, label="vasca / laghetto")]
    ax.legend(handles=h, fontsize=7, loc="lower left", framealpha=0.9)


def plate(cid: str, nature: str, docs=DOCS):
    docs.mkdir(parents=True, exist_ok=True)
    osm, pop, check, dem, fuel = load(cid)
    meta, rows, arrivals = fires.load(cid)
    s = meta["sweep"]

    # 1. The town plate, full territory and the core.
    fig, axs = plt.subplots(1, 2, figsize=(18, 9.2))
    for ax, (lo, hi) in zip(axs, [(0, WORLD_M), (1800, 6200)]):
        atlas.fuel_image(ax, dem, fuel)
        ax.contour(dem[::-1], levels=np.arange(0, 1000, 50), colors="k", linewidths=0.25, alpha=0.6,
                   extent=atlas.EXT)
        draw_town(ax, osm, pop, check, labels=(lo > 0))
        for ig in meta["ignitions"]:
            ax.plot(ig["x"], ig["y"], marker="*", ms=10, color="k", mec="white", zorder=8)
            ax.text(ig["x"] + 60, ig["y"] + 60, ig["name"], fontsize=7, zorder=9)
        ax.set_xlim(lo, hi)
        ax.set_ylim(lo, hi)
        ax.tick_params(labelsize=6)
    legend(axs[1])
    atlas.fuel_legend(axs[0], fuel, loc="upper left")
    axs[0].set_title(f"{cid}: il territorio (8 km), combustibili dopo case, strade e orti", fontsize=10)
    axs[1].set_title("il nucleo (4 km): abitati, strade, aree di attesa, inneschi dello sweep", fontsize=10)
    fig.tight_layout()
    fig.savefig(docs / "tavola_paese.png", dpi=100)
    plt.close(fig)

    # 2. The town over the propagations, one panel per wind: how many
    #    ignitions reach each cell within 2 h.
    fig, axs = plt.subplots(2, 4, figsize=(18, 9.4))
    cmap = ListedColormap(["#fdd49e", "#fc8d59", "#d7301f", "#7f0000"])
    tt = threat_table(pop, meta, arrivals)
    for ax, w in zip(axs.ravel(), s["wind_from_deg"]):
        count = sum((sum(((arrivals[f"{ig['name']}_w{w}_s{sd}"] >= 0) & (arrivals[f"{ig['name']}_w{w}_s{sd}"] <= 7200))
                         .astype(int) for sd in s["seeds"]) * 2 > len(s["seeds"])).astype(int)
                    for ig in meta["ignitions"])
        atlas._base(ax, dem)
        ax.imshow(np.ma.masked_where(count == 0, np.minimum(count, 4)), cmap=cmap, vmin=0.5, vmax=4.5,
                  extent=atlas.EXT, alpha=0.75, interpolation="nearest")
        draw_town(ax, osm, pop, check, labels=False)
        atlas._wind_arrow(ax, w)
        ax.set_xlim(1800, 6200)
        ax.set_ylim(1800, 6200)
        ax.tick_params(labelsize=5)
        hit = ", ".join(f"{loc.split()[-1]} {tt[loc][w]['by_2h'] * 100:.0f}%" for loc in tt)
        ax.set_title(f"da {atlas.WIND_NAMES[w]}\nfuoco a <150 m entro 2 h: {hit}", fontsize=7)
    fig.suptitle(f"{cid}: da quanti inneschi (su {len(meta['ignitions'])}) e raggiunta ogni cella entro 2 h "
                 f"(chiaro 1 ... scuro 4+), con gli abitati", fontsize=10)
    fig.tight_layout()
    fig.savefig(docs / "paese_venti.png", dpi=95)
    plt.close(fig)

    # 3. Nature vs built: same ignitions, same winds, same seeds.
    nmeta, nrows, _ = fires.load(f"{nature}__per_{cid}")
    nat = {r["name"]: r for r in nrows}
    pairs = [(float(nat[r["name"]]["ha_360min"]), float(r["ha_360min"]),
              float(nat[r["name"]]["ha_120min"]), float(r["ha_120min"]))
             for r in rows if r["name"] in nat and "to" not in r["name"].split("_")[1]]
    p = np.array(pairs)
    compare = {"runs": len(p), "nature_ha_2h_p50": float(np.median(p[:, 2])), "built_ha_2h_p50": float(np.median(p[:, 3])),
               "nature_ha_6h_p50": float(np.median(p[:, 0])), "built_ha_6h_p50": float(np.median(p[:, 1])),
               "smaller_share_6h": float(np.mean(p[:, 1] < p[:, 0] - 1)),
               "ratio_6h_p50": float(np.median(p[:, 1] / np.maximum(p[:, 0], 1)))}
    (docs / "metriche_paese.json").write_text(json.dumps({"threat": tt, "nature_vs_built": compare}, indent=1) + "\n")

    lines = ["| località | " + " | ".join(atlas.WIND_NAMES[w].split()[0] for w in s["wind_from_deg"]) + " |",
             "|---|" + "---|" * len(s["wind_from_deg"])]
    for loc, row in tt.items():
        cells = []
        for w in s["wind_from_deg"]:
            v = row[w]
            med = f" ({v['median_min']:.0f}')" if v["median_min"] is not None and v["by_6h"] > 0 else ""
            cells.append(f"{v['by_2h'] * 100:.0f}% / {v['by_6h'] * 100:.0f}%{med}")
        lines.append(f"| {loc} | " + " | ".join(cells) + " |")
    (docs / "minaccia.md").write_text("\n".join(lines) + "\n")
    return tt, compare, "\n".join(lines)
