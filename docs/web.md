# Browser build and GitHub Pages

The browser build is self-contained: every entry in `data/scenarios.json` and
its scenario inputs are compiled into the WASM rather than fetched at run
time. The startup chooser therefore exposes the same scenarios as the desktop
app. Render terrains larger than 512 samples on an edge are reduced for the
browser (Spotorno goes from 5 m to 20 m posting), and vegetation is capped at
12% density.

Build it locally with:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
./scripts/build_web.sh
python3 -m http.server --directory dist 8080
```

Open `http://localhost:8080`; do not open `index.html` directly, since browsers
do not allow WASM modules to load from `file://` URLs.

The GitHub Actions workflow in `.github/workflows/pages.yml` publishes the
same `dist/` folder whenever `main` is updated. In the repository settings,
set **Pages → Source** to **GitHub Actions** once.
