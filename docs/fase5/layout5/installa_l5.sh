#!/bin/sh
# A data dir for headless trials of layout 5 without publishing it: everything
# in data/ is linked, except scenarios/rocca_ventosa, which is the built
# t4_paese5 with its game.json. Run the examples from $DEST/l5 (they read ./data).
#   DEST=/tmp/prova sh docs/fase5/layout5/installa_l5.sh
set -e
REPO=$(cd "$(dirname "$0")/../../.." && pwd)
DEST=${DEST:?imposta DEST}
mkdir -p "$DEST/l5/data/scenarios"
for f in "$REPO"/data/*; do [ "$(basename "$f")" != scenarios ] && ln -sf "$f" "$DEST/l5/data/"; done
rm -rf "$DEST/l5/data/scenarios/rocca_ventosa"
cd "$REPO/tools" && ../.venv/bin/python - "$DEST/l5/data/scenarios/rocca_ventosa" <<'PY'
import json, shutil, sys
from pathlib import Path
from factory import cases
from factory.export import DATA
src, dst = DATA / "scenarios" / "t4_paese5", Path(sys.argv[1])
shutil.copytree(src, dst, ignore=shutil.ignore_patterns("check.json"))
meta = json.loads((dst / "scenario.json").read_text())
meta.update({"id": "rocca_ventosa", "is_dev": False})
meta.pop("vr_palette", None)
(dst / "scenario.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False) + "\n")
cases.write_game(dst, "t4_paese5")
PY
