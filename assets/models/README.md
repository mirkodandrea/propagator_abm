# Emergency simulation models

Presentation proportions are centralized in `crates/game/src/visual_scale.rs`:
building footprints ×1.9 and heights ×2.5, cars ×3, people ×4.5, emergency
units ×7, trees ×1.6 and shrubs ×1.25. Vegetation multipliers apply to the
existing species sizes. These affect rendering only; scenario coordinates,
terrain and fire resolution, routes, plant counts and mesh detail are unchanged.

Original, editable Blender assets generated for this project; no external art or textures.

- `emergency_assets.blend`: ten named collections (pedestrian, firefighter, car, fire_engine, pine, oak, chestnut, bush, olive, cypress). Each collection is authored at the origin; isolate a collection to edit it.
- `meshes.json`: baked positions, normals, vertex colors, triangle indices and bark masks, embedded in the native and web game.
- `preview.png`: Blender studio render. The fire engine is shown with red paint; game symbols use their live operational status tint.

Regenerate from the repository root:

```sh
/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/build_models.py
cargo test -p game models::tests
```

The generator is the source of truth: regeneration rebuilds the `.blend` and mesh bake. Make persistent design changes in `scripts/build_models.py`.

Blender coordinates are metres, Z up, front -Y; baked meshes use Y up and front +Z. People and vehicles have their origins at ground level. Trees include maritime pines (tall bare leaning trunk, flat umbrella pads), chestnuts (short trunk, big lobed round crown), broad oaks, gnarled olives, and narrow cypresses, and are approximately one metre tall before the vegetation system applies species-specific sizes. Bushes are normalized clumps. Existing map-symbol scale factors still apply.

People and suppression models bake neutral vertex tones so the existing status materials stay legible. Cars retain colored details. Vegetation retains species palettes and authored crown shading; bark masks distinguish stems from leaves. Plant meshes are welded, limited to 120 vertices / 160 triangles per archetype, and merged into existing spatial chunks so burning and culling keep working. Grass remains procedural.

Triangle budgets: pedestrian 356, firefighter 388, car 712, fire engine 1,688, pine 112, oak 152, chestnut 156, bush 100, olive 144, cypress 52. The landscape uses pine (fuel 10-12), chestnut (4-6) and bush (7-9) only, one model per group, so the fuel reads from the silhouette; olives remain as lone trees in grassland. More detailed vegetation increases triangle counts versus the former procedural models; draw-call batching and the existing browser density reduction are preserved.

Rocca Ventosa's hills, field parcels, curving village streets and rural lanes are
baked by `scripts/generate_demo_scenarios.py demo_borgo`. Isolated homes are real
populated buildings connected to the evacuation road network. Class 4 draws
open olive groves; other broadleaf stands mix olives and oaks, while conifer
stands mix umbrella pines with cypresses. Limestone outcrops are static geometry
and remain outside the vegetation burn ranges.
