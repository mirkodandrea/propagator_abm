# Demo — presentation spec (graphics, GUI, kiosk shell)

Owner: the **presentation agent**. Read `docs/demo-spec.md` first (goal, decisions,
the contract with the gameplay agent) and `CLAUDE.md` (findings 11–13, 21–23, 25 bind
rendering and input). The model side is `docs/demo-spec-gameplay.md`.

Legend: ✅ done · 🔲 to do · ✂ cut-list item (drop first if days slip).

## 0. How this agent works

**Owns:** `crates/game` (kiosk shell, rendering, UI, camera), `assets/`,
`shaders/`, `scripts/build_models.py`, `strings_it.rs`, the screenshot harness.
**Does not touch:** `crates/{demo,abm,fire,scenario,behavior}` model behaviour, `data/`
scenarios. Needs a number, event or refusal the model doesn't expose? Add a row to the
contract table in `docs/demo-spec-gameplay.md` §1 and build against the *current*
type meanwhile (render "—" or hide the element); never compute game logic in the UI.

**Method:** screenshot, read every PNG, fix, repeat. `KIOSK_SHOT=<dir>` walks a
session (`KIOSK_TOWN`, `KIOSK_PLAY_S`, `KIOSK_WINDOWED`); check **all three towns**
and read the actual image — a correct overlay nobody can see looks like a broken one
(finding 13), and a back-facing mesh draws nothing while the logs say it exists
(finding 11). Frame rate is measured, not assumed.

**Do not polish a screen the gameplay agent is about to change.** Money, trust,
decision cards and unit chips (contract rows) change the HUD; build the layout slots
now, fill them when the field lands.

## 1. Done

- ✅ Kiosk shell (`crates/game/src/kiosk/`): attract / briefing / play / outcome /
  compare, operator corner, fixed 6 s steps shared with `demo::Run`, counterfactual
  on a thread, clamped camera, `strings_it.rs`, no shortcut can fire.
- ✅ UI v1 in CIMA colours: pictograms, counters, compass, action bar, forecast card,
  outcome and compare cards, logo.
- ✅ Removed: composer, behaviour/debug tabs, `egui-snarl`, `DEMO` flag.
- ✅ **Playtest fixes (2026-10-06, `07cc021`):** banner above the action bar
  (`ACTION_BAR_TOP`), forecast card layout, play idle only before the first order
  (warn 150 s, pause suspends), outcome/compare cards on `Outcome::secure()` with the
  same rows, one headline, takeaway no longer praises needless evacuation, clock
  shows "Tempo rimasto", bearings spelled out, camera frames the households'
  centroid (Porto), logo moved off the action bar.

## 2. Clean-up (game side) 🔲 — do before new features

Delete what is not the demo from `crates/game`: `menu.rs`, `ui.rs` panels (keep
`UiFocus`), `browser.rs`, `scenario_selector.rs`, `ignition_edit.rs`, `inspect.rs`
panels (keep a selection ring only if §6 needs it), `selftest.rs`, `api.rs` +
`tiny_http`, `interview.rs`, `map2d.rs`, `history.rs` if unused,
`native_accessibility.rs`, `web_clipboard.rs`, `native_text_input.rs`, `capture.rs`
scripted layers (keep the harness in `kiosk::shots`); web/wasm targets and the
`wasm-release` profile; far terrain, 25 km sea and the sky clock (`far_terrain.rs`,
`sea.rs`, `sky.rs`) — replaced by the plinth and fixed light (§4). Then rewrite
`CLAUDE.md` for the demo only, keeping findings 2, 3, 5, 7, 11–14, 17–19, 21–23, 25,
34, 37, 39–41. Resulting layout: one crate or two (`demo` lib + `demo-app` bin) —
pick the simpler; headless tests must stay window-free. Small commits, tests green
after each. (Model-side data/test deletion is the gameplay agent's, spec §3.)

## 3. Kiosk shell and UI

Full-screen borderless, cursor visible, **no single-key shortcuts** (finding 25),
hidden operator corner, egui at 1.5–2×, CIMA navy/orange, large rounded targets.

Session flow: ATTRACT → BRIEFING → PLAY → OUTCOME → COMPARE → retry | ATTRACT; idle
60 s outside PLAY returns to ATTRACT. Restart goes through the reset path
(finding 21).

Remaining UI work:
- 🔲 **Slots for gameplay fields** (contract rows): money counter, trust meter,
  forecast ghost arrow, "perché?" line, unit chips with one-sentence status, closed
  road / spot fire markers. Lay out now; wire when the field exists.
- 🔲 **Advisors** (§7.3 old): Capo squadra (fire), Polizia locale (roads), Sindaco
  (warnings/trust); one bubble at a time, ≥ 8 s apart, fired on an `Event` from the
  contract. One `advisors.rs` table `(trigger, speaker, text_it)`.
- 🔲 **Decision pauses** (§7.4 old): dim map, card with 2–3 big choices incl.
  "aspetta"; driven by model `Event`s, not scripts.
- 🔲 **Overlays** ≥ +20 m above ground (finding 13), verified by screenshot: wind
  arrow on the terrain, spot-fire ring + "!", threat tint, household beacons
  (green/amber/red), evacuation chevrons, closed-road icons, selection ring, ghost of
  the no-orders perimeter on COMPARE (§16 #38).
- 🔲 **Briefing**: single card in the bottom third, town centred; the fly-in must be
  visible (§16 #22).
- 🔲 **Attract**: tagline and button spacing, title off the village (§16 #21 rest).
- 🔲 **Action bar**: "Evacuazione" after use says "avvisate N famiglie · T+mm" (§16
  #24); refusals as persistent unit chips, not a 6 s banner (§16 #25).
- 🔲 **Compass**: faint second arrow for the forecast shift; flash "Il vento è
  cambiato!" when it does (§16 #23, #39).
- 🔲 **Outcome**: line "l'evacuazione salva le persone, non le case" (§16 #18);
  town chip "N famiglie in pericolo" (§16 #41).
- 🔲 **Robustness of the twin spinner**: `Err` variant over the channel, "non
  disponibile" after a 10 s timeout; cancel a stale twin on `Cmd::Begin` (§16 #32–33).
- 🔲 **Operator**: "Ricomincia" must not silently pin the town (§16 #12); delete unused
  strings `SKIP`, `BACK` or use them (§16 #11); `KIOSK_IDLE_S` to test the reset in
  seconds; progress ring on the idle warning (§16 #42).
- 🔲 **Localisation**: native-speaker review of `strings_it.rs`; refusal mapping
  (when the gameplay agent delivers `Refusal`, a test that every variant maps).
- 🔲 **LLM bubbles (OpenRouter)** ✂: short in-character bubbles via `crates/chat`
  (finding 30), hard timeout, canned fallback, rate-limited, never pauses; angry
  households (trust) are the natural speakers. Needs network; fallback mandatory.

## 4. Graphics — *Link's Awakening* diorama, one still light

Reference: the 2019 remake. Priority: legibility from 2 m, then the toy-diorama read,
then polish. Fire is the only truly bright thing against a calm pastel world. **Fixed
lighting; no day/night.** Today the town is tiny and dark and houses read as faint
orange dashes (§16 #20) — this is the largest remaining quality gap.

- 🔲 **Tilt-shift (core):** one cheap full-screen post pass, blur ramping with
  distance from a horizontal focus band, slight vignette/saturation lift, never on
  egui; fallback Bevy `DepthOfField`. Keep the band on the town.
- 🔲 **Lighting:** one warm low-angle sun that never moves, soft shadows (few cascades),
  generous ambient, TonyMcMapface/AgX, bloom threshold high so only fire/embers/
  beacons glow. Remove the clock from `sky.rs`. Pick by screenshot against the
  reference.
- 🔲 **Diorama:** terrain as a raised plinth/tile block with visible strata sides and a
  base; sea as a glossy tile inside the block; drop `far_terrain` and the 25 km sea;
  horizon is the table, softly out of focus.
- 🔲 **Materials:** smooth slightly glossy "plastic", flat or two-tone shading,
  vertex-colour palettes (CIMA-tinted); inverted-hull outlines only if cheap.
- 🔲 **Buildings (priority):** chunky rounded low-poly houses, distinct roofs/chimneys/
  gardens; church, town hall, school, fire station; states intact / threatened /
  alight / charred read at a glance.
- 🔲 **Vegetation:** big round-topped trees and bushes as chunky props (hundreds, not
  230 k plants), fuel class readable by tree type and ground tint, vertex-sine sway.
  Large frame-rate win.
- 🔲 **People and cars:** round, bright, oversized figures (keep 3× scale), walking
  bob, family groups; chunky toy cars with headlight blink, queues that read at the
  exit; engine, crew van, Canadair with distinct silhouettes; refuges drawn as
  *places* (piazza, car park, quay, sign) with visible sheltering groups.
- 🔲 **Fire/smoke/embers:** layered flame billboards that flicker, soft round smoke
  bent by the wind, ember streaks with a flash on spot-fire landing, water-drop cloud
  and wet strip, fireline strip, hose stream.
- **Frame budget:** 30 fps floor, decided by measurement on day 1 on the real
  machine. Disable in order: shadow resolution, cascades, bloom quality, tilt-shift
  samples, MSAA→FXAA. Never disable: fire glow, beacons, wind arrow, the tilt-shift
  read. Operator `quality` low/medium/high.

## 5. Assets 🔲

Reuse `assets/models/emergency_assets.blend` → `scripts/build_models.py` →
`meshes.json` (embedded, vertex colours; read `assets/models/README.md`). Low-poly
(< 2 k tris), shared CIMA-tinted palette, baked vertex AO: house variants (+ charred),
civic buildings, pine/maquis/olive/grass props, car / fire engine / crew van / tanker
aircraft, figures, plinth. ✂ custom assets (keep the existing set) if days slip.

## 6. Robustness and ops 🔲

`scripts/run_demo.sh` supervisor relaunches on exit; fully offline except OpenRouter
bubbles; fullscreen borderless; OS sleep/screensaver off. Ship
`docs/demo-operator.md` (one page: launch, operator corner, reset, frozen-screen
procedure, power settings, the three towns in one sentence each, staff talking
points).

## 7. Testing (presentation)

- 🔲 `KIOSK_SELFTEST=1` harness (not a mode) for the Bevy side: state machine, reset
  fan-out (finding 21), order buttons, "no key does anything", idle reset leaks
  nothing, COMPARE twin equals the live `Sim` for the same orders.
- 🔲 Screenshots at T+10 / T+60 / end for all three towns, reviewed by eye.
- 🔲 Frame-rate measurement on the real machine; 30 fps floor.
- 🔲 Test that every refusal string/variant maps to something other than the fallback.
- 🔲 Operator corner, pause/resume, cancel, camera clamp, scroll/orbit: verify with a
  real pointer (not possible headless).

## 8. Open questions (presentation)

- Clean-up end state: one crate or two (§2).
- Final town names and Italian copy; native-speaker review.
- GPU/resolution on the actual machine (sets default quality; Switch-class target).
- Whether to keep all three towns loaded to avoid the reload hitch on town change
  (§16 #32 old notes).

## 9. Playtest findings owned here (2026-10-06)

Bugs #1–#4, #6, #7, #9, #10, #12 fixed (see §1); remaining from the playtest:
#8 refusal mapping, #11 unused strings, #20 legibility (§4), #21 attract spacing,
#22 briefing card, #23 forecast arrow, #24 action-bar feedback, #25 persistent
refusal, #26–#29 copy (done: #26, #27, #28, #29 title), #31 native review, #32–#34
twin/restart robustness, #37–#39 "perché?", COMPARE ghost, wind-changed flash,
#41 danger chip, #42 idle ring. Full text in git history at `da0549b`.

Could not check (needs a pointer/profiler): live fps, unit tasking on the map,
bad-point refusals on screen, order-button double click, operator corner, idle
timings in practice, unit sprites/spot rings/smoke/traffic on the map, restart leaks.

## 10. Status after the first presentation pass

Shipped: §2 clean-up (workbench, api, interview, far terrain, sea, sky, wasm,
`egui-snarl`; `crates/game` is kiosk-only, `CLAUDE.md` rewritten, `docs/demo-operator.md`
written); §4 plinth with strata walls and table (`plinth.rs`), one fixed warm sun,
depth-of-field as the tilt-shift read, toy-scale houses (1.7x), per-town camera framing
(`kiosk::view::play_dist`); `KIOSK_IDLE_S`, `KIOSK_FPS`; Ricomincia no longer pins the
town; unused strings removed. Measured ~32-38 fps in the screenshot harness on the dev
Mac (windowed 1600x1000 at 2x), so the 30 fps floor holds with little margin.

Not done: overlays (§3), briefing/attract fixes, action-bar feedback, twin robustness,
advisors, `KIOSK_SELFTEST`, supervisor script, vegetation props (still 230 k plants),
people/cars/units art, fire/smoke polish, native Italian review, remaining dead-code
warnings (`FireLayer` variants in `fire_view.rs`).

Requests to gameplay: none new. Slots for money/trust/events are not yet laid out.

## 11. Status after the second presentation pass

Shipped: vegetation is ~4 k chunky toy props (pine stacks, cloud-lobe broadleaves,
macchia domes; `PROP_SCALE` 2.6) over a pastel land-cover ground tint (bilinear,
domain-warped; `terrain_mesh::cover_tint`) instead of 230 k plants; `toy.rs` builds
person, crew squad, car, fire engine, crew van (swaps with the squad when the crew
works on foot), air tanker and refuge sign in code (no Blender needed; the
`meshes.json` pipeline is now unused by the game for these, still used for
`models::mesh` trees nowhere); toy houses (footprint 1.5x, height 1.7x, chimneys, six
roof colours, warm cast for "threatened", loud alight/charred); figures 8x with walk
bob, pastel cars; `overlays.rs`: wind arrow beside the fire, household beacons
(amber/red/blue), spot-fire ring + "!", closure ring + barrier; fire flames larger,
smoke lighter; briefing as bottom card with forecast top-left; attract spacing;
outcome/compare anchored low; camera framing = town/fire midpoint, `KIOSK_SHOT_ZOOM`
and `KIOSK_SHOT_FOCUS` for close-ups, `3b_play_late` screenshot.
Frame rate (dev Mac, windowed 1600x1000 @2x): before 20-30 fps in play; now 57 fps in
play (uncapped with `KIOSK_FPS`, so CPU-bound by the model step, not the renderer) and
120 fps on attract. Vegetation triangles 10 M -> 0.36 M.

Not done: vertex-sine sway (needs a custom vertex shader on the retro material),
evacuation chevrons, selection ring, inverted-hull outlines, tilt-shift is still Bevy
DepthOfField, advisors, decision pauses, twin robustness, `KIOSK_SELFTEST`, supervisor
script. The outcome/compare cards still cover most of the map. Requests to gameplay:
none; HUD slots for money/trust not laid out.
