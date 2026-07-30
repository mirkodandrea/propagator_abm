# Browser build and GitHub Pages

The browser build is self-contained: scenario inputs are compiled into the
WASM rather than fetched at run time. Its terrain is sampled at 20 m instead
of the desktop build's 5 m, and vegetation is capped at 12% density.

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
