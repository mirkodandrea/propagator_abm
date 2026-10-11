"""Exit map: road nodes driven by the evacuation, coloured by how many
districts share them, over the fire's arrival minute."""
import csv, sys
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

src, out, box = sys.argv[1], sys.argv[2], [float(v) for v in sys.argv[3].split(",")]
cases = sys.argv[4:]
cols = 3
rows = (len(cases) + cols - 1) // cols
fig, axes = plt.subplots(rows, cols, figsize=(6 * cols, 6 * rows), squeeze=False)
for ax, case in zip(axes.flat, cases):
    fire = list(csv.DictReader(open(f"{src}/{case}_fire.csv")))
    sc = ax.scatter([float(r["x"]) for r in fire], [float(r["y"]) for r in fire], c=[float(r["min"]) for r in fire],
                    s=1.2, cmap="YlOrRd_r", vmin=0, vmax=180, marker="s", linewidths=0)
    nodes = list(csv.DictReader(open(f"{src}/{case}_nodes.csv")))
    names = [k for k in nodes[0] if k not in ("node", "x", "y", "first_min", "last_min", "cut_min")]
    share = {1: "#b8c6cc", 2: "#2a9d8f", 3: "#1d3557"}
    for k in (1, 2, 3):
        sel = [r for r in nodes if min(3, sum(int(r[n]) > 0 for n in names)) == k]
        tot = [sum(int(r[n]) for n in names) for r in sel]
        ax.scatter([float(r["x"]) for r in sel], [float(r["y"]) for r in sel], s=[(3 if k == 1 else 14) + t / 10 for t in tot], c=share[k],
                   label={1: "un borgo", 2: "due borghi", 3: "tre o più"}[k], zorder=3)
    cut = [r for r in nodes if r["cut_min"]]
    ax.scatter([float(r["x"]) for r in cut], [float(r["y"]) for r in cut], marker="x", c="k", s=18, zorder=4, label="tagliato dal fuoco")
    for d in csv.DictReader(open(f"{src}/{case}_districts.csv")):
        ax.annotate(f"{d['name']}\n{d['households']} fam.", (float(d["x"]), float(d["y"])), fontsize=8, ha="center",
                    bbox=dict(boxstyle="round,pad=0.2", fc="white", alpha=0.75), zorder=5)
    ax.set_title(case)
    ax.set_xlim(box[0], box[1]); ax.set_ylim(box[2], box[3])
    ax.set_aspect("equal")
    ax.set_xticks([]); ax.set_yticks([])
for ax in axes.flat[len(cases):]:
    ax.axis("off")
axes.flat[0].legend(loc="lower left", fontsize=8)
fig.colorbar(sc, ax=axes.ravel().tolist(), shrink=0.5, label="minuto di arrivo del fuoco")
fig.savefig(out, dpi=90, bbox_inches="tight")
