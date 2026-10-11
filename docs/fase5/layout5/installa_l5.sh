#!/bin/sh
# A data dir for headless trials of layout 5 without publishing it: everything
# in data/ is linked, except scenarios/rocca_ventosa, which is the built
# layout (t4_paese5, or SCEN) with its game.json. Run the examples from $DEST/l5
# (or $DEST/$SUB; they read ./data).
#   DEST=/tmp/prova sh docs/fase5/layout5/installa_l5.sh
#   DEST=/tmp/prova SCEN=t4_paese6 SUB=l6 sh docs/fase5/layout5/installa_l5.sh
set -e
REPO=$(cd "$(dirname "$0")/../../.." && pwd)
DEST=${DEST:?imposta DEST}
SCEN=${SCEN:-t4_paese5}
SUB=${SUB:-l5}
mkdir -p "$DEST/$SUB/data/scenarios"
for f in "$REPO"/data/*; do [ "$(basename "$f")" != scenarios ] && ln -sf "$f" "$DEST/$SUB/data/"; done
rm -rf "$DEST/$SUB/data/scenarios/rocca_ventosa"
cd "$REPO/tools" && ../.venv/bin/python - "$DEST/$SUB/data/scenarios/rocca_ventosa" "$SCEN" <<'PY'
import json, shutil, sys
from pathlib import Path
from factory import cases
from factory.export import DATA
src, dst = DATA / "scenarios" / sys.argv[2], Path(sys.argv[1])
shutil.copytree(src, dst, ignore=shutil.ignore_patterns("check.json"))
meta = json.loads((dst / "scenario.json").read_text())
meta.update({"id": "rocca_ventosa", "is_dev": False})
meta.pop("vr_palette", None)
(dst / "scenario.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False) + "\n")
cases.write_game(dst, sys.argv[2])
PY
