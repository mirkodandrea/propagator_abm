libsm64 source: https://github.com/libsm64/libsm64
Revision: fd11813208272b4271d92bd92feb8f3fdbe61be5
License: LICENSE.md (upstream CC0).

The generated decomp/mario/{geo,model}.inc.{c,h} files were produced by
upstream import-mario-geo.py using n64decomp/sm64 revision
06ec56df7f951f88da05f468cdcacecba496145a. No ROM or extracted textures or
animations are included. Geometry and animation assets are initialized from
the user's local US ROM at runtime. The source is built offline by cc only
for native targets; the browser game has no dependency on this crate.
