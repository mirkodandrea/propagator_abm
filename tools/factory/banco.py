"""Banco di prova `rocca_banco`: i paesi di Rocca Ventosa su terreno neutro.

Per tarare i parametri di spegnimento senza il rumore del territorio vero:
stessi edifici, strade, famiglie, caserme e mezzi (collegamenti simbolici a
`rocca_ventosa`), ma orografia piatta, un solo combustibile su tutte le celle
vegetate (le strade, i piazzali e il mare restano come sono), vento costante e
inneschi a distanza nota **sopravvento** a un paese. Ogni caso è un punto del
banco: paese x distanza x vento.

    .venv/bin/python tools/factory/banco.py [--fuel 9] [--dist 800,1500,2500] [--wind 20,40]
    ROCCA_SCENARIO=rocca_banco cargo run --release -p rocca --example spegnimento -- ...
"""

import argparse
import json
import math
import os
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "data/scenarios/rocca_ventosa"
N, CELL = 400, 20.0

# paese -> (centro in metri, direzione da cui soffia il vento)
TOWNS = {
    "Castelvento": ((5033.0, 5188.0), 119.0),
    "SanRocco": ((6204.0, 4263.0), 80.0),
    "PianDeiGrilli": ((2443.0, 2590.0), 119.0),
}
FLAT_M = 100.0


def cell_of(x, y):
    return int((8000.0 - y) // CELL), int(x // CELL)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--id", default="rocca_banco")
    ap.add_argument("--fuel", type=int, default=9)
    ap.add_argument("--dist", default="800,1500,2500")
    ap.add_argument("--wind", default="20,40")
    ap.add_argument("--minutes", type=int, default=180)
    a = ap.parse_args()
    dst = ROOT / "data/scenarios" / a.id
    dst.mkdir(exist_ok=True)
    own = ("fuel.i32", "dem.f64", "game.json")
    for f in SRC.iterdir():
        link = dst / f.name
        if f.name not in own and not link.exists():
            os.symlink(os.path.relpath(f, dst), link)
    fuel = np.fromfile(SRC / "fuel.i32", dtype=np.int32).reshape(N, N)
    dem = np.fromfile(SRC / "dem.f64", dtype="<f8").reshape(N, N)
    veg = fuel > 0
    uniform = fuel.copy()
    uniform[veg] = a.fuel
    uniform.tofile(dst / "fuel.i32")
    np.where(dem > 0, FLAT_M, 0.0).astype("<f8").tofile(dst / "dem.f64")

    game = json.loads((SRC / "game.json").read_text())
    cases = []
    for town, ((cx, cy), from_deg) in TOWNS.items():
        for d in (float(v) for v in a.dist.split(",")):
            for w in (float(v) for v in a.wind.split(",")):
                b = math.radians(from_deg)
                x, y = cx + d * math.sin(b), cy + d * math.cos(b)
                r, c = cell_of(x, y)
                if not (0 <= r < N and 0 <= c < N) or not uniform[r, c] > 0:
                    print(f"salto {town} {d:.0f} m: innesco su cella non bruciabile ({x:.0f},{y:.0f})")
                    continue
                cases.append({"name": f"{town}_{d:.0f}_{w:.0f}", "near": {"SanRocco": "San Rocco", "PianDeiGrilli": "Pian dei Grilli"}.get(town, town),
                              "ignition": [round(x, 1), round(y, 1)], "radius_m": 60.0,
                              "wind_from_deg": from_deg, "wind_kmh": w, "minutes": a.minutes})
    game["cases"] = cases
    game["featured"] = [c["name"] for c in cases[:3]]
    game["levels"] = [{"name": "Banco", "case": cases[0]["name"], "about": "Banco di prova, non si gioca."}]
    (dst / "game.json").write_text(json.dumps(game, indent=1, ensure_ascii=False))
    print(f"{a.id}: {len(cases)} casi, combustibile {a.fuel} su {int(veg.sum())} celle, orografia piatta")


if __name__ == "__main__":
    main()
