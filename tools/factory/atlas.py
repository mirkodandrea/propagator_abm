"""The propagation atlas: a few readable images and one table per candidate,
for checkpoint 1. Every figure is derived from the arrival-time rasters the
Rust sweep wrote; nothing here simulates fire."""

from __future__ import annotations

import itertools

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib.colors import ListedColormap  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

from .export import ROOT  # noqa: E402
from .grid import CELL_M, CORE_M, CORE_OFF_M, WORLD_M, hillshade  # noqa: E402
from .vegetation import FUEL_NAMES  # noqa: E402

DOCS = ROOT / "docs" / "factory" / "fase1"
EXT = [0, WORLD_M, 0, WORLD_M]
HA = CELL_M * CELL_M / 10_000
FUEL_COLOURS = {0: "#9a9a9a", 1: "#efe7b0", 2: "#dfe58a", 3: "#e9c862", 4: "#6fae63", 5: "#3f8a3f",
                7: "#c2bd78", 8: "#959338", 9: "#6e6a1f", 11: "#2e5e3e", 12: "#1b4430"}
WIND_NAMES = {0: "N (tramontana)", 45: "NE (grecale)", 90: "E (levante)", 135: "SE (scirocco)",
              180: "S (ostro)", 225: "SO (libeccio)", 270: "O (ponente)", 315: "NO (maestrale)"}


def _base(ax, dem, alpha=0.55):
    ax.imshow(hillshade(dem), cmap="gray", extent=EXT, vmin=-0.4, vmax=1.1, alpha=alpha)
    ax.contour(dem[::-1], levels=np.arange(0, 1000, 100), colors="#555555", linewidths=0.35, alpha=0.7, extent=EXT)
    ticks = np.arange(0, WORLD_M + 1, 2000)
    ax.set_xticks(ticks)
    ax.set_yticks(ticks)
    ax.tick_params(labelsize=6)
    _core(ax)


def _core(ax):
    """The designed core, where every ignition is; the rest is buffer."""
    ax.add_patch(matplotlib.patches.Rectangle((CORE_OFF_M, CORE_OFF_M), CORE_M, CORE_M, fill=False,
                                              ec="#333333", lw=0.7, ls=":", zorder=3))


def _wind_arrow(ax, from_deg, label=None):
    """An arrow pointing where the wind blows TO, in a white disc bottom-right."""
    to = np.radians(from_deg + 180)
    k = WORLD_M / 4000
    x, y, L = WORLD_M - 380 * k, 380 * k, 230 * k
    ax.add_patch(matplotlib.patches.Circle((x, y), 330 * k, fc="white", ec="#1060c0", lw=0.8, alpha=0.9, zorder=4))
    ax.annotate("", xy=(x + L * np.sin(to), y + L * np.cos(to)), xytext=(x - L * np.sin(to), y - L * np.cos(to)),
                arrowprops=dict(arrowstyle="-|>", color="#1060c0", lw=2.2), zorder=5)
    if label:
        ax.text(x, y - 480 * k, label, color="#1060c0", fontsize=7, ha="center", weight="bold")


def _ignitions(ax, igns, color="k"):
    for ig in igns:
        ax.plot(ig["x"], ig["y"], marker="*", ms=9, mec="white", mfc=color, mew=0.8)
        ax.text(ig["x"] + WORLD_M / 60, ig["y"] + WORLD_M / 60, ig["name"], fontsize=7, weight="bold", color=color,
                bbox=dict(boxstyle="round,pad=0.1", fc="white", ec="none", alpha=0.6))


def fuel_image(ax, dem, fuel):
    ids = sorted(FUEL_COLOURS)
    lut = np.zeros(13, dtype=int)
    for i, k in enumerate(ids):
        lut[k] = i
    ax.imshow(lut[np.clip(fuel, 0, 12)], cmap=ListedColormap([FUEL_COLOURS[k] for k in ids]),
              vmin=0, vmax=len(ids) - 1, extent=EXT, interpolation="nearest")
    ax.imshow(hillshade(dem), cmap="gray", extent=EXT, alpha=0.3)
    _core(ax)


def dem_image(ax, dem):
    _base(ax, dem)
    im = ax.imshow(dem, cmap="terrain", alpha=0.45, extent=EXT, vmin=0, vmax=900)
    ax.contour(dem[::-1], levels=np.arange(0, 1000, 50), colors="k", linewidths=0.3, extent=EXT)
    return im


def fuel_legend(ax, fuel, **kw):
    present = [k for k in sorted(FUEL_COLOURS) if (fuel == k).any()]
    ax.legend(handles=[Patch(fc=FUEL_COLOURS[k], label=f"{k} {FUEL_NAMES.get(k, '')} {(fuel == k).mean() * 100:.0f}%")
                       for k in present], fontsize=6, **kw)


def burnt(a):
    return a >= 0


def metrics(meta, rows, arrivals) -> dict:
    s = meta["sweep"]
    igns = [i["name"] for i in meta["ignitions"]]
    winds = s["wind_from_deg"]
    by = {r["name"]: r for r in rows}
    plain = [r for r in rows if "to" not in r["name"].split("_")[1]]
    a60 = np.array([float(r["ha_60min"]) for r in plain])
    a180 = np.array([float(r["ha_end"]) for r in plain])
    marks = {k: float(np.median([float(r[k]) for r in plain])) for k in rows[0] if k.startswith("ha_") and k.endswith("min")}
    est = (a60 >= 5.0).mean()
    def edge_s(a):
        b = np.concatenate([a[0], a[-1], a[:, 0], a[:, -1]])
        b = b[b >= 0]
        return b.min() if b.size else np.inf
    edges = np.array([edge_s(arrivals[r["name"]]) for r in plain])
    edge = np.mean(edges <= s["minutes"] * 60)
    edge60 = np.mean(edges <= 3600)
    edge180 = np.mean(edges <= 3 * 3600)
    # How much the wind changes WHERE it burns: mean Jaccard of burnt masks
    # between wind directions, same ignition and seed (low = the wind matters).
    jac = []
    for ig in igns:
        for seed in s["seeds"]:
            ms = [burnt(arrivals[f"{ig}_w{w}_s{seed}"]) for w in winds]
            for a, b in itertools.combinations(ms, 2):
                u = (a | b).sum()
                if u:
                    jac.append((a & b).sum() / u)
    ever = np.zeros_like(next(iter(arrivals.values())), dtype=bool)
    for r in plain:
        ever |= burnt(arrivals[r["name"]])
    # Seed spread: same ignition and wind, different spotting draws.
    spread = []
    for ig in igns:
        for w in winds:
            v = [float(by[f"{ig}_w{w}_s{seed}"]["ha_end"]) for seed in s["seeds"]]
            if np.mean(v) > 0:
                spread.append(np.std(v) / np.mean(v))
    shift = []
    for ig in igns:
        for a, b in s["shifts"]:
            for seed in s["seeds"]:
                m0 = burnt(arrivals[f"{ig}_w{a}_s{seed}"])
                m1 = burnt(arrivals[f"{ig}_w{a}to{b}_s{seed}"])
                u = (m0 | m1).sum()
                shift.append((m0 ^ m1).sum() / u if u else 0.0)
    return {
        "runs": len(rows),
        "established_60": est,
        "ha60_p50": np.median(a60), "ha180_p10": np.percentile(a180, 10),
        "ha180_p50": np.median(a180), "ha180_p90": np.percentile(a180, 90),
        "edge_share": edge, "edge_share_60": edge60, "edge_share_180": edge180, "median_ha": marks,
        "wind_jaccard": float(np.mean(jac)),
        "ever_burnt_share": ever.mean(),
        "seed_cv": float(np.median(spread)),
        "shift_changed_share": float(np.mean(shift)),
    }


def figure_terrain(cid, name, dem, fuel, igns, path):
    fig, ax = plt.subplots(1, 2, figsize=(11, 5.4))
    im = dem_image(ax[0], dem)
    ax[0].set_title(f"{cid} {name} - quota (curve 50 m)", fontsize=9)
    fig.colorbar(im, ax=ax[0], shrink=0.7, label="m s.l.m.")
    fuel_image(ax[1], dem, fuel)
    _ignitions(ax[1], igns)
    ax[1].set_title("combustibili (eu_fuel12) e inneschi dello sweep", fontsize=9)
    fuel_legend(ax[1], fuel, loc="upper left", bbox_to_anchor=(1.01, 1.0))
    for a in ax:
        a.tick_params(labelsize=6)
    fig.tight_layout()
    fig.savefig(path, dpi=110)
    plt.close(fig)


def figure_winds(cid, dem, meta, arrivals, path):
    s = meta["sweep"]
    fig, axs = plt.subplots(2, 4, figsize=(14, 7.4))
    cmap = ListedColormap(["#fdd49e", "#fc8d59", "#d7301f", "#7f0000"])
    for ax, w in zip(axs.ravel(), s["wind_from_deg"]):
        # how many ignitions reach the cell, in a majority of the seeds
        count = sum((sum(burnt(arrivals[f"{ig['name']}_w{w}_s{seed}"]).astype(int) for seed in s["seeds"])
                     * 2 > len(s["seeds"])).astype(int) for ig in meta["ignitions"])
        _base(ax, dem)
        ax.imshow(np.ma.masked_where(count == 0, np.minimum(count, 4)), cmap=cmap, vmin=0.5, vmax=4.5,
                  extent=EXT, alpha=0.8, interpolation="nearest")
        _wind_arrow(ax, w)
        _ignitions(ax, meta["ignitions"])
        ax.set_title(f"vento da {WIND_NAMES[w]}, {s['wind_kmh']:.0f} km/h", fontsize=8)
    fig.suptitle(f"{cid}: da quanti inneschi (su {len(meta['ignitions'])}) e raggiunta ogni cella entro "
                 f"{s['minutes']} min - chiaro 1, arancio 2, rosso 3, scuro 4+ (maggioranza dei "
                 f"{len(s['seeds'])} seed)", fontsize=10)
    fig.tight_layout()
    fig.savefig(path, dpi=100)
    plt.close(fig)


def _isochrones(ax, dem, arr, ig, w, title):
    _base(ax, dem)
    mins = np.where(arr >= 0, arr / 60.0, np.nan)
    ax.contourf(mins[::-1], levels=[0, 30, 60, 120, 240, 360],
                colors=["#b30000", "#e34a33", "#fc8d59", "#fdcc8a", "#fef0d9"], extent=EXT, alpha=0.85)
    ax.contour(mins[::-1], levels=[30, 60, 120, 240], colors="k", linewidths=0.4, extent=EXT)
    _wind_arrow(ax, w)
    _ignitions(ax, [ig])
    ax.set_title(title, fontsize=8)


def figure_arrivals(cid, dem, meta, arrivals, path, picks):
    winds = [0, 90, 180, 270]
    fig, axs = plt.subplots(len(picks), 4, figsize=(14, 3.6 * len(picks)))
    for row, ig in zip(np.atleast_2d(axs), picks):
        for ax, w in zip(row, winds):
            a = arrivals[f"{ig['name']}_w{w}_s1"]
            _isochrones(ax, dem, a, ig, w, f"{ig['name']}, vento da {WIND_NAMES[w]}: {burnt(a).sum() * HA:.0f} ha")
    fig.suptitle(f"{cid}: tempi di arrivo (rosso scuro <30 min, rosso <1 h, arancio <2 h, giallo <4 h, chiaro <6 h), seed 1",
                 fontsize=10)
    fig.tight_layout()
    fig.savefig(path, dpi=100)
    plt.close(fig)


def figure_shift(cid, dem, meta, arrivals, path, picks):
    s = meta["sweep"]
    fig, axs = plt.subplots(len(picks), 2 * len(s["shifts"]), figsize=(14, 3.6 * len(picks)))
    for row, ig in zip(np.atleast_2d(axs), picks):
        for k, (a, b) in enumerate(s["shifts"]):
            m0 = arrivals[f"{ig['name']}_w{a}_s1"]
            m1 = arrivals[f"{ig['name']}_w{a}to{b}_s1"]
            _isochrones(row[2 * k], dem, m0, ig, a, f"{ig['name']}: sempre da {WIND_NAMES[a]}")
            _isochrones(row[2 * k + 1], dem, m1, ig, b,
                        f"{ig['name']}: da {WIND_NAMES[a]}, poi {WIND_NAMES[b]} a T+{s['shift_at_min']}")
            row[2 * k + 1].contour(burnt(m0)[::-1].astype(float), levels=[0.5], colors="#1060c0",
                                   linewidths=0.8, linestyles="--", extent=EXT)
    fig.suptitle(f"{cid}: cambio di vento a T+{s['shift_at_min']} min (tratteggio blu = area senza cambio), seed 1",
                 fontsize=10)
    fig.tight_layout()
    fig.savefig(path, dpi=100)
    plt.close(fig)


def most_wind_sensitive(meta, arrivals, k=2):
    """The ignitions whose burnt area moves most with the wind: the examples."""
    s = meta["sweep"]
    score = []
    for ig in meta["ignitions"]:
        ms = [burnt(arrivals[f"{ig['name']}_w{w}_s1"]) for w in s["wind_from_deg"]]
        size = np.mean([m.sum() for m in ms])
        j = np.mean([(a & b).sum() / max((a | b).sum(), 1) for a, b in itertools.combinations(ms, 2)])
        score.append(((1 - j) * np.sqrt(size), ig))
    return [ig for _, ig in sorted(score, key=lambda t: -t[0])[:k]]


def overview(cands, path):
    fig, axs = plt.subplots(2, len(cands), figsize=(4.2 * len(cands), 8.6), squeeze=False)
    for col, (c, dem, fuel) in enumerate(cands):
        dem_image(axs[0, col], dem)
        axs[0, col].set_title(f"{c.id} {c.name}\n{dem.min():.0f}-{dem.max():.0f} m", fontsize=9)
        fuel_image(axs[1, col], dem, fuel)
    fuel_legend(axs[1, -1], np.concatenate([f.ravel() for _, _, f in cands]), loc="upper left",
                bbox_to_anchor=(1.01, 1.0))
    fig.tight_layout()
    fig.savefig(path, dpi=90)
    plt.close(fig)
