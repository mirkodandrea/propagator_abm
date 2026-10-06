# Demo — presentation spec (graphics, GUI, kiosk shell)

Read `docs/demo-spec.md` first and `CLAUDE.md` (findings 11–13, 21–23, 25 bind
rendering and input). The model side is `docs/demo-spec-gameplay.md`.

Legend: ✅ done · 🔶 partly · 🔲 to do · ✂ cut-list item.

## 0. How this side works

**Owns:** `crates/game`, `assets/`, shaders, `scripts/build_models.py`,
`scripts/build_town_models.py`, `strings_it.rs`, the screenshot harness. Never
computes game logic: levels, reached/needless, medals, costs come from `demo::`.

**Method:** screenshot, read every PNG, fix, repeat. `KIOSK_SHOT=<dir>` walks a
session as a sensible commander (district 0 warned and defended at the
briefing), shooting attract, briefing (before/after orders), play (early/late)
and the outcome panel. `KIOSK_SHOT_ZOOM=<k>` and `KIOSK_SHOT_FOCUS=x,y` for
close-ups. Check **all three towns** and read the image — a correct overlay
nobody can see looks broken (finding 13), a back-facing mesh draws nothing
(finding 11).

## 1. Screens (v2) ✅

**Attract.** Title, tagline ("Un incendio, tre quartieri, il vento che decide.
Tocca a te."), the town's name, the fire playing at speed behind. Click anywhere.

**Briefing = planning, clock stopped.** Short fly-in (4 s) onto the whole town.
Bottom-left card: town, one-paragraph situation, how to play, **Via!**. Right
column: compass (with a dashed ghost arrow where the wind may turn) and forecast
card. Bottom-right strip: the lesson in one line ("avvisare presto… falso
allarme…"). The district chips are live: orders given here happen at T+0.
Starts by itself after 75 s.

**Play.** Top-left: town, `T+mm`, time left, progress; the bill under it. Top
strip: four counters (al sicuro / in viaggio / in pericolo / case colpite).
Right column: compass + wind in words + forecast card (pulses amber when issue 2
lands). **On the map:** one chip per district — name, households, status in
colour (green calm / amber watch / orange threatened, pulsing / red reached) with
"Fuoco a 650 m", and two buttons: **Avvisa** (turns into "✓ Avvisati 34/148 via"
with a progress fill) and **Difendi** (shows "2 autobotti"; disabled when none
is free). An **Incendio** label rides the fire's head. Bottom-left: advisor
bubble (Capo squadra VVF / Sindaco / Meteo, portrait, one line, 7 s, queued).
Bottom: action bar — *Allerta generale*, *Autobotti* (count; pressing it says
"usa «Difendi» sui quartieri"), *Canadair* (arm, click the map), *Pausa*,
*Veloce* (×3). Chips keep out of the right column and the advisor band.

**Outcome.** Camera slides the burnt town into the left half; a panel on the
right: headline ("Tutti al sicuro!" / "Ottimo lavoro!" / "Hai fatto la
differenza" / "Il fuoco è stato più veloce"), families safe with bar, families
caught vs **senza ordini** (from the twin; spinner, then "non disponibile" after
20 s), "Hai salvato N famiglie rispetto a nessun ordine", **one row per
district** (its story, caught vs without orders), **three medals** (earned in
gold, unearned dim with a one-line hint for next time), the lesson, the bill.
*Riprova* replays the same draw from the briefing; *Un altro paese*.

## 2. Town kit and look ✅ / 🔶

- **Blender kit** (`scripts/build_town_models.py` → `assets/models/town.json`,
  contact sheet `town_preview.png`, source `town_assets.blend`): church with
  campanile, chapel, town hall with clock and tricolore, school, fire station
  with red bay doors and hose tower, petrol station, water tower, substation
  with pylon, farmhouse, barn, silo, mill with water wheel, workshop with sawtooth
  roof, lighthouse; props (cypress, street tree, fountain, bench, umbrellas,
  loungers, beach hut, boat, sailboat, pier, Protezione Civile tent and *area di
  attesa* sign, camping tents, caravan, goal, streetlight); cars (hatchback,
  saloon, SUV, van, Ape, bus, camper); figures (man, woman, child, elder with a
  stick). Paint and clothes are pure white in the bake so the game's tint (car
  paint, status colour) shows through. Test: every face's winding agrees with
  its normal (`models::tests`).
- **Landmarks** (`town_kit.rs`, a child of `buildings`): a building whose kind
  is modelled is drawn with its kit model scaled to the footprint on a paved lot;
  open spaces (piazza, pitch, car park, *area di attesa*, lido, campsite,
  harbour, cemetery) are draped on the terrain and dressed with props. All in
  the building chunks' merged mesh.
- **Houses**: one per street slot, kind-sized (villa 12×10 … hotel 21×13), pastel
  Italian palette (ochre, apricot, cream, rose, pale yellow, Ligurian blue, mint,
  terracotta), shop awnings and sign boards, garden trees and cypresses behind
  about half the houses. Ground: lawn green in gardens, pale paving in the cores.
- **House states follow the books**: a home the referee counts as hit burns
  (8 simulated minutes) then chars; a threatened district takes a warm cast. Not
  the exposure layer (its 2.5 km ember reach tinted every house in these towns).
- **People and cars**: figure by age from the population; car body by household
  (mostly hatchbacks and saloons, the odd Ape and camper), eight paints.
- **Beacons** only over families the fire is on (trapped, or alarming threat at
  a home still occupied). Spot-fire rings and closures unchanged.
- Camera frames the districts and the fire (bounding box), not the households'
  centroid; zoom and pan stay free within limits.

## 3. Work needed (priority order)

1. 🔶 **Fire read.** Done: toy-scale flames (×2.8 Byram, 18–90 m), an 18-minute
   glowing band behind the front (was 7 min cubed — sub-pixel at the play
   camera), pale thinner smoke lofted higher, the *Incendio* label at the centre
   of the burning area. Open: ember sparks leaning with the wind are faint; a
   flame-front line shader would read better than billboards at 3 km.
2. ✅ **Order feedback on the map.** A sky-blue ring round each warned district;
   a green ring of `DEFEND_REACH_M` at each engine post (`Referee::posts`).
   Open: a one-shot pulse when the order is given.
3. **Chip density.** Three chips with two buttons each is the whole UI; check in
   the playtest whether a collapsed chip (name + status) that expands on hover
   reads better at 2 m.
4. **Polish.** Vertex sway on trees; headlight blink in queues; water drop
   splash and wet strip under a Canadair run; roofs (hip/gable mix by kind);
   district ground tint by level.
5. **Ops.** `KIOSK_SELFTEST=1` (state machine, reset fan-out per finding 21, no
   key does anything, idle reset leaks nothing); ✅ `scripts/run_demo.sh`
   supervisor; ✅ operator quit button; native-speaker review of `strings_it.rs`;
   dead-code warnings (`FireLayer`, `models::model`, `OrderKind::{Attack,Line}`).
6. **Target machine:** fps (was 57 on the dev Mac before the kit; the kit adds
   ~10 k triangles per town, measure), pointer, idle timings.

## 4. Frame budget

30 fps floor, measured on the real machine. Disable in order: shadow
resolution, cascades, bloom quality, depth-of-field samples, MSAA→FXAA. Never
disable: fire glow, the district chips, the wind arrow.

## 5. Assets pipeline

```sh
/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/build_town_models.py
/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/build_models.py   # trees, old figures
python3 scripts/generate_demo_scenarios.py   # towns: districts, houses, landmarks
cargo test -p game models::tests
```

Landmark footprints in the generator (`*_LANDMARKS`) and the kit's authored
footprints (`BUILDERS` in the Blender script) should match; the game scales one
to the other.

## 6. Testing (presentation)

- ✅ `sim::tests::the_live_session_and_its_headless_twin_agree` (COMPARE honesty).
- ✅ `models::tests` (bakes valid, kit winding).
- 🔲 `KIOSK_SELFTEST` (above). 🔲 Every `EventKind` has an advisor line or a
  deliberate `None` (exhaustive match today; a test would pin it).
- 🔲 Real pointer: operator corner, pause/fast, Canadair arming, chip clicks.

## 7. History

v1 status, playtest findings (#1–#42) and the first two presentation passes are in
git at `1eb263c` (this file). v2: districts on the map, merged outcome panel,
town kit — `523c14a`.
