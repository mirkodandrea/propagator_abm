#!/usr/bin/env bash
set -euo pipefail

# Produces a self-contained static site in dist/.  Scenario assets are embedded
# in game.wasm, so GitHub Pages only needs these three output files.
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

command -v wasm-bindgen >/dev/null || {
  echo "wasm-bindgen-cli is required: cargo install wasm-bindgen-cli --version 0.2.126 --locked" >&2
  exit 1
}

cargo build --package game --target wasm32-unknown-unknown --profile wasm-release
rm -rf dist
mkdir -p dist
wasm-bindgen \
  --target web \
  --out-dir dist \
  --out-name game \
  target/wasm32-unknown-unknown/wasm-release/game.wasm
cp web/index.html dist/index.html

# Optional but worthwhile when Binaryen is installed (the CI installs it).
if command -v wasm-opt >/dev/null; then
  wasm-opt -Oz --strip-debug -o dist/game.opt.wasm dist/game_bg.wasm
  mv dist/game.opt.wasm dist/game_bg.wasm
fi

echo "Built GitHub Pages site in $root/dist"
