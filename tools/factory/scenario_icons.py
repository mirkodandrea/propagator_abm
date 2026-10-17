#!/usr/bin/env python3
"""Square icons of the playable scenarios for the kiosk's scenario screen.

Same terrain, fire arrivals and colours as `scenario_images.py`, no text: the
fire's arrival (no orders, seed 2), the towns it hits, the ignition and the wind.
Needs the arrivals of the headless `scenario_maps` example in
results/scenario-impacts/focused/images/data (the cases: `medium` from
game.json plus variety-cases.json). Writes assets/scenarios/<case>.png.

MPLCONFIGDIR=/tmp/rocca-mpl .venv/bin/python tools/factory/scenario_icons.py
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import numpy as np
import scenario_images as si
from matplotlib.colors import BoundaryNorm, ListedColormap
from matplotlib.collections import LineCollection
import matplotlib.pyplot as plt

ROOT = si.ROOT
DATA = si.OUT / 'data'
OUT = ROOT / 'assets/scenarios'
PX = 480


def cases():
    game = json.loads((si.SOURCE / 'game.json').read_text())['cases']
    variety = json.loads((si.OUT.parent / 'variety-cases.json').read_text())['cases']
    return {c['name']: c for c in game + variety}


def icon(case, rgb, roads):
    result = json.loads((DATA / f"{case['name']}.json").read_text())
    arrival = np.fromfile(DATA / f"{case['name']}.i32", dtype='<i4').reshape(400, 400)
    valid = arrival != np.iinfo(np.int32).min
    age = np.ma.array(arrival.astype(float) / 60 - case['start_min'], mask=~valid)
    fig = plt.figure(figsize=(4, 4), dpi=PX / 4, facecolor=si.BG)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb, extent=[0, 8, 0, 8], origin='upper', zorder=0)
    lines = [np.array(r['line']) / 1000 for r in roads if r.get('drivable')]
    ax.add_collection(LineCollection(lines, colors='#f4f2e9', linewidths=.8, alpha=.8, zorder=2))
    ax.imshow(age, extent=[0, 8, 0, 8], origin='upper', cmap=ListedColormap(si.FIRE_COLORS),
              norm=BoundaryNorm([-31, 0, 60, 120, 180, 1000], 5), interpolation='nearest', alpha=.9, zorder=3)
    for d in result['districts']:
        colour = si.RED if d['homes'] else si.PURPLE if d['caught'] else '#68796f'
        x, y = np.array(d['centre']) / 1000
        ax.scatter([x], [y], s=120, c=colour, edgecolors='white', linewidths=2, zorder=7)
    x, y = np.array(case['ignition']) / 1000
    ax.scatter([x], [y], marker='*', s=520, c=si.INK, edgecolors='white', linewidths=1.6, zorder=10)
    # wind: arrows point where it blows to; a second one for the shift
    shift = case.get('shift')
    for k, (w, colour) in enumerate([(case, si.BLUE)] + ([(shift, si.PURPLE)] if shift else [])):
        a = np.deg2rad(w['wind_from_deg'])
        cx, cy = 0.9 + 1.0 * k, 7.0
        ax.annotate('', xy=(cx - .5 * np.sin(a), cy - .5 * np.cos(a)), xytext=(cx + .5 * np.sin(a), cy + .5 * np.cos(a)),
                    arrowprops=dict(arrowstyle='-|>', lw=3.2, color=colour, mutation_scale=22,
                                    path_effects=[]), zorder=11)
    ax.set(xlim=(0, 8), ylim=(0, 8), aspect='equal')
    ax.axis('off')
    OUT.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT / f"{case['name']}.png", dpi=PX / 4, facecolor=si.BG)
    plt.close(fig)
    print(OUT / f"{case['name']}.png")


def main():
    rgb, _ = si.terrain()
    roads = json.loads((si.SOURCE / 'osm.json').read_text())['roads']
    by_name = cases()
    levels = json.loads((si.SOURCE / 'game.json').read_text())['levels']
    for level in levels:
        icon(by_name[level['case']], rgb, roads)


if __name__ == '__main__':
    main()
