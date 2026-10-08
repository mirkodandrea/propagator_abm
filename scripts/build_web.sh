#!/usr/bin/env bash
set -euo pipefail

# The browser build: a static site in target/web/ with the data compiled into the
# wasm (crates/datafs). Serve it with any static server, or GitHub Pages.
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

command -v wasm-bindgen >/dev/null || {
  echo "wasm-bindgen-cli is required (same version as the wasm-bindgen crate in Cargo.lock)" >&2
  exit 1
}

cargo build --package game --target wasm32-unknown-unknown --profile wasm-release
rm -rf target/web
mkdir -p target/web
wasm-bindgen --target web --no-typescript --out-dir target/web --out-name game \
  target/wasm32-unknown-unknown/wasm-release/game.wasm
# A version in the URLs, so a browser that has the previous build cached
# fetches the new one (python's server and GitHub Pages both let it cache).
v="$(shasum target/web/game_bg.wasm | cut -c1-12)"
sed -e "s#'./game.js'#'./game.js?v=$v'#" -e "s#init()#init({ module_or_path: './game_bg.wasm?v=$v' })#" web/index.html > target/web/index.html
if command -v wasm-opt >/dev/null; then
  wasm-opt -Os --strip-debug -o target/web/game.opt.wasm target/web/game_bg.wasm
  mv target/web/game.opt.wasm target/web/game_bg.wasm
fi
echo "Built $root/target/web ($(du -sh target/web/game_bg.wasm | cut -f1) wasm)"
