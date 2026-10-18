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

- `units.json`, `units_assets.blend`, `units_preview.png`: the player's teams, from `scripts/build_unit_models.py` (firefighter on foot, engine, air tanker). Real colours baked as vertex colours (red engine with blue beacons, orange AIB jackets and white helmets, white aircraft with red markings); the game no longer tints them by team or status (only a lost unit goes dark and a withdrawing one reddish). Animated parts are separate entries `<unit>.<part>` with a `rig` (pivot, axis, kind): engine wheels and beacons, firefighter legs and arms (the tool arm chops while working), tanker propellers; `units::animate_units` drives them on real time. Regenerate with `/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/build_unit_models.py`.

- `vegetation.json`, `vegetation_assets.blend`, `vegetation_preview/` (open `index.html`): the plants of the landscape, from `scripts/build_vegetation_models.py`. Each species has several **varieties** of one silhouette family (pine 4, chestnut 4, macchia 4, olive 3: the player reads the fuel from the silhouette), picked from the plant's yaw (`models::pick_variety`). A variety has a **live** model (`wood` trunk and boughs, `leaf` crown pads, same space and colours as `meshes.json`) and **what fire adds** (`remains`: the twig skeleton inside the crown; debris: ash and fallen wood; plus a 12-triangle `low` for chunks beyond the LOD distance). The two never overlap: the trunk and boughs of the live model are charred in place by the shader (`retro.wgsl`, `burn_plant`), the leaves are eaten with holes, the skeleton shows where the front has passed (`burn_front`) and the debris last. Budget (asserted by `models::the_vegetation_varieties_are_within_budget`): live ≤ 120 triangles / 80 vertices, remains ≤ 120 / 160 (they are drawn for every plant of a chunk the fire has touched). `meshes.json` still serves the oak and cypress and the proxies' source models. Regenerate with `/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/build_vegetation_models.py` (about 4 minutes, almost all of it the preview renders).

Regenerate from the repository root:

```sh
/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/build_models.py
cargo test -p game models::tests
```

The generator is the source of truth: regeneration rebuilds the `.blend` and mesh bake. Make persistent design changes in `scripts/build_models.py`.

Blender coordinates are metres, Z up, front -Y; baked meshes use Y up and front +Z. People and vehicles have their origins at ground level. Trees include maritime pines (tall bare leaning trunk, flat umbrella pads), chestnuts (short trunk, big lobed round crown), broad oaks, gnarled olives, and narrow cypresses, and are approximately one metre tall before the vegetation system applies species-specific sizes. Bushes are normalized clumps. Existing map-symbol scale factors still apply.

People and suppression models bake neutral vertex tones so the existing status materials stay legible. Cars retain colored details. Vegetation retains species palettes and authored crown shading; bark masks distinguish stems from leaves. Plant meshes are welded, about 40–50 vertices / 72 triangles per archetype (lumpy 12-vertex crowns; the foliage pixel shader adds the texture), and merged into existing spatial chunks so burning and culling keep working. Grass remains procedural.

Triangle budgets: pedestrian 356, firefighter 388, car 712, fire engine 1,688, pine 112, oak 152, chestnut 156, bush 100, olive 144, cypress 52. The landscape uses pine (fuel 10-12), chestnut (4-6) and bush (7-9) only, one model per group, so the fuel reads from the silhouette; olives remain as lone trees in grassland. More detailed vegetation increases triangle counts versus the former procedural models; draw-call batching and the existing browser density reduction are preserved.

Rocca Ventosa's hills, field parcels, curving village streets and rural lanes are
baked by `scripts/generate_demo_scenarios.py demo_borgo`. Isolated homes are real
populated buildings connected to the evacuation road network. Class 4 draws
open olive groves; other broadleaf stands mix olives and oaks, while conifer
stands mix umbrella pines with cypresses. Limestone outcrops are static geometry
and remain outside the vegetation burn ranges.
