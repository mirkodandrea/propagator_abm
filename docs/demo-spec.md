# Demo rewrite — spec

Interactive kiosk demo for students visiting the gazebo at the *Settimana della
Protezione Civile*, Rome. Written 2026-10-06 for a fresh session; the event is
about a week out. Read `CLAUDE.md` first — its findings are constraints here, not
background.

## 1. Goal

A student walks up cold and, within 30 seconds, is making decisions as an
incident commander in a small stylised town, watching individual families
respond to a fire. A session is **3–5 minutes**, ends on a clear outcome, and
invites a second try.

Success is a bystander who can say, unprompted: *"the fire goes where the wind
blows, people need warning early, and my decisions changed who got out."*

## 2. Non-goals

- Real places, real historical fires, or anything modelled on a fatal incident.
  **No `spotorno`, `mati`, `pedrogao`, `rhodes` in the demo.**
- Economy, zoning, budgets, building mechanics. The only SimCity idea we keep is
  *limited resources*, which the model already has.
- Changing any number in the model crates. See §9.
- The composer, interview/LLM chat, debrief, control API, web build. (Web is a
  possible later target — `docs/web.md` — but not for this event.)
- A persistent leaderboard or numeric score. See §6.5 for what the end card
  shows instead.

## 3. Architecture decision

**New binary crate `crates/demo`**, depending on `scenario`, `fire`, `abm` (and
`behavior` only because `abm` needs it). It does **not** depend on `game`,
`chat`, `telemetry`'s chat table, or anything under `game/src/composer/`.

Why a new crate rather than a mode inside `game`: `game` has ~40 modules of
researcher UI, shortcut handling (finding 25) and egui panels that a kiosk must
not expose, and a demo that is "game minus things" keeps leaking them.

Reuse by **copying then trimming**, not by refactoring `game` this week:
`frame.rs` (the only place the north→−Z flip lives — keep using `to_bevy` /
`to_world`), `terrain_mesh.rs`, `sea.rs`, `sky.rs`, `models.rs`, `roads.rs`,
`buildings.rs`, `people.rs`, `units.rs`, `fire_view.rs`, `fire_shader.rs`,
`sim.rs` (`Sim::advance`, `Sim::new`, `opening_conditions`). Preserve the
findings that live in them: 11 (winding), 12 (resample drapes), 13 (overlays
under canopy), 21 (restart clears latched view state), 22–23 (fog, sea). If
copying proves heavier than expected, the fallback is to make `game` a lib and
add a `demo` feature — decide in the first two hours, do not agonise.

Dependency direction stays as in `CLAUDE.md`; `demo` is a new leaf at the top.

## 4. Session flow

```
ATTRACT ──touch──▶ BRIEFING ──▶ PLAY (with 2–3 decision pauses) ──▶ OUTCOME ──▶ COMPARE ──▶ (retry | ATTRACT)
   ▲                                                                                              │
   └───────────────────────── 60 s idle on any screen except PLAY ───────────────────────────────┘
```

- **ATTRACT**: slow camera orbit over the town, the fire already burning in
  accelerated time, large "Tocca per iniziare". Loops forever. Pre-seeded run,
  no player.
- **BRIEFING** (≤20 s, skippable): one card. Who you are, what's at stake, what
  the buttons do. Camera fly-in (≈8 s) to the town.
- **PLAY**: fixed mission length of **~3 minutes real time** (≈ 60–90 simulated
  minutes at a fixed demo speed; tune in §8). Real-time with scripted pauses.
- **OUTCOME**: four facts (§6.5), no points.
- **COMPARE**: *"Stesso incendio, nessun ordine"* — rerun the identical fire
  with no orders, show the same four facts side by side. Restart is already a
  controlled comparison (`Sim::ignitions` carry `at_s`); the "no orders" run
  can be computed headlessly at load time so it is instant.
- **Idle reset**: 60 s without input outside PLAY → ATTRACT. During PLAY: 90 s
  without input → auto-pause with "Sei ancora lì?", then reset after another
  30 s. A restart must go through the existing reset path so latched view state
  clears (finding 21).

## 5. Synthetic scenarios

Three authored fictional towns, one per difficulty step. They are **designed
for a teaching beat**, not generated at random, and each is verified by a
headless test (§9) that the beat actually happens.

Authoring: extend `scripts/generate_synthetic_scenarios.py` (it already emits
`scenario.json`, roads, population and fuel/DEM rasters for the labs) with a
new `demo` family of specs, or add `scripts/generate_demo_scenarios.py` reusing
its helpers. They register in `data/scenarios.json` like any other. Finding 39a
applies: do not hand-write the registry.

| id | Name (placeholder) | Beat | Mechanism it teaches |
|---|---|---|---|
| `demo_borgo` | *Borgo San Fiorenzo* | **Warn early.** Hillside village, one clear downwind slope, two exits. Wind constant. | An early order saves families; waiting costs them. |
| `demo_valle` | *Valle dei Pini* | **The wind changes.** Same village shape, a scripted wind shift at ~T+25 min turns the fire toward a second hamlet that was safe. | Conditions change; re-assess. Re-plan ignition/wind via the existing mid-run wind control. |
| `demo_porto` | *Porto Rosso* | **One road out.** Coastal town, single narrow exit and a beach/haven; spot fires ignite behind the front. | Traffic queues, spotting, and the front being unpredictable. Uses the traffic queue (finding 39) and spot fires (finding 41). |

Design requirements for every demo scenario:

- **World size 3–5 km, 150–400 households.** Small enough to read at a glance
  and to hold a high frame rate; large enough that individual families are
  visible (the `town_scale` lab is 5 km / 400 households — use it as the
  reference for performance).
- **Fire must threaten people within the first ~minute of play.** Finding 4: a
  fire travels only 500–800 m in two hours, so ignition placement and radius are
  the scenario. Size per scenario empirically in the style of
  `crates/fire/tests/sizing.rs` and `real_scenario_ignitions.rs`, using
  `ignite_patch` (finding 3) and **five-seed means** (finding 41). Pin the
  chosen `radius_m` / wind / moisture in `sim.rs::opening_conditions`, keyed on
  id (finding 38).
- **Houses never burn in the CA** (finding 2) and threat at a house peaks
  around 0.3 (finding 35). Do not design a scenario whose drama depends on
  `StructureExposure` reaching "alight" without measuring that it does. Spotting
  (finding 41) is what makes buildings alight; confirm with a run.
- **Refuges and havens must be real** (findings 9, 34): check the derived
  refuge/haven/mast coverage on each new window, and make it an assertion.
- Each scenario carries Italian `name`, `description`, and **invented**
  `localities` (fictional place names; no `addr:city` derivation applies).
- Terrain is authored, not clipped from data: gentle ridge, valley, a
  coastline for `demo_porto`. Keep DEM smooth; fuel is a hand-painted mosaic of
  `eu_fuel12` classes (maquis, pine, grass, non-vegetated village core) so the
  fire's path is legible from the air.

**Behaviour for the demo.** Use the shipped behaviour library plus the
`takes-some-convincing` profile at a nonzero share *for demo scenarios only* if
playtesting shows an order that nearly everyone obeys instantly feels hollow
(finding 42). That is a demo-only data choice; it must not touch the shipped
`data/behaviours/` for the other scenarios. Measure before deciding.

## 6. UI

Full-screen, no window chrome, cursor hidden outside play. **No single-key
shortcuts at all** in the demo build (finding 25 — strangers at the keys). A
hidden operator chord (e.g. hold top-right corner 3 s, or `Ctrl+Shift+F10`)
opens a small operator panel: scenario select, restart, toggle sound, quit.

Use egui, `set_pixels_per_point` ≈ 1.5–2.0, dark high-contrast theme, large
rounded widgets. Design for reading at 2–3 m. Min button height ≈ 96 px, body
text ≥ 24 pt.

### 6.1 Action bar (bottom-centre)
Five icon buttons, Italian labels, each with a state (ready / cooldown /
unavailable-with-reason):

| Button | Does | Existing machinery |
|---|---|---|
| **Evacuazione** | general order | `Sim` order path (`e`) |
| **Squadra** | arm hand-crew order, then tap map | `command.rs` crews |
| **Autobotte** | arm engine order, then tap map | `command.rs` engines |
| **Canadair** | request air support / drop here | `abm::suppression` air tankers (`Canadair 1-2`): requested, 25 min to arrive, briefable while inbound |
| **Pausa** | pause / resume | clock |

Tap button → tap map. A big orange banner states the pending order
(*"Tocca la mappa per mandare la squadra"*) with a visible ✕. One tool armed at
a time, as now. Refusals are one Italian sentence, shown on screen, never silent
(the model already generates them).

### 6.2 HUD
- **Top-left counters**, four, each with icon, colour and an animated tick on
  change: *Al sicuro · In fuga · In pericolo · Case perse*. Sourced from
  aggregate `Sim` state — the player is the commander, so the god view is
  correct *here* (finding 30 constrains agent *speech*, not the HUD).
- **Top-centre**: clock + mission line (*"T+00:42 · Proteggi Borgo San Fiorenzo"*).
- **Wind arrow** on the map and in the HUD, always visible. It is the single best
  teaching visual.

### 6.3 Advisors
Three portraits with speech bubbles, templated Italian text driven by `Sim`
events and thresholds — **no LLM**:

- **Capo squadra** — fire behaviour ("Il fuoco sta salendo verso il crinale").
- **Polizia locale** — roads/traffic ("La strada per Noli è intasata").
- **Sindaco** — population/warnings ("Metà delle famiglie non ha ricevuto l'allarme").

Rules: at most one bubble at a time, ≥8 s apart, newest replaces oldest, each
fires on a *change* not a state. A single `advisors.rs` with a table of
`(trigger, speaker, text_it)` so non-programmers can edit the wording.

### 6.4 Decision pauses
Scripted per scenario as data (`demo/missions/<id>.ron` or JSON): at sim time
*t* or on a condition, pause, dim the map, push the camera to the relevant area,
show a card with 2–3 big choices. Choices map to the same orders as §6.1 plus
"aspetta". Each scenario has 2–3.

### 6.5 Outcome card
Facts, no points:

```
famiglie al sicuro   212 / 250
ancora in pericolo    11
case perse             4
ettari bruciati       38
```

then the COMPARE screen with the same four numbers for "nessun ordine" beside
yours, and a one-line Italian takeaway chosen from the difference (e.g. "Il tuo
ordine ha salvato 61 famiglie in più"). `CLAUDE.md` records *Scoring: none* as a
settled decision; this is deliberately **facts and a counterfactual, not a
score**. Flag to Mirko if that line is judged too close.

## 7. Graphics

Hardware is a **capable desktop with a mouse** (§16), so the budget is a modern
GPU at 1080p-1440p, not an integrated one. Spend it in this order: **legibility
from 2 m first, then a cohesive stylised look, then polish effects.** Do not
restyle terrain/road materials from scratch; spend effort on overlays, lighting
and post-processing, which are cheap in Bevy 0.14 and change the picture most.

Art direction in one line: **a lit, warm, slightly oversaturated miniature model
of a Mediterranean town**, with fire as the only truly bright thing on screen.
Everything else is mid-value so the fire, the beacons and the overlays always
win the eye.

### 7.1 Diorama camera and framing
- Fixed pitch ~45-55 degrees, yaw limited to a +/-30 degree arc, zoom/pan clamped
  to the town. Narrow-FOV perspective first; true orthographic only if clearly
  better (fog, sea and chunk culling were tuned for perspective).
- Crop to the town with a visible **table edge / plinth** (see §8). Retire
  `far_terrain` and the 25 km sea bands in the demo: they hide the window edge
  and are pure cost here (findings 22-23 stop applying; do not carry the code).
- Gentle automatic camera assists, all cancellable by any mouse input: ease to
  a new spot fire or decision area; slow drift in ATTRACT; fly-in on BRIEFING.

### 7.2 Lighting, shadows and colour (biggest look-per-hour)
Use Bevy 0.14 built-ins; verify each against the actual `bevy = "0.14"` feature
flags and the project's custom `Material`s (water and fire shaders are custom,
so anything that needs `StandardMaterial` prepasses must be tested on them).
- **Directional sun with cascaded shadow maps**, tight cascade bounds around the
  town. Soft long shadows at low sun angle sell the miniature. Time of day
  advances through the mission toward golden hour then dusk (existing sky/time
  controls); dusk makes the fire glow, so it doubles as pacing.
- **Tonemapping** (`TonyMcMapface` or `AgX`) with **HDR + Bloom**. Bloom is
  what makes the fire front, embers, beacons and headlights feel luminous.
  Keep the threshold high so only emissive things bloom, never the sea or roofs.
- **Colour grading**: lift saturation ~10-15%, warm highlights, slightly cool
  shadows. One preset per time-of-day stop, interpolated.
- **Ambient occlusion** (SSAO) for contact shadows under eaves, between houses
  and at tree bases, if it composes with the chosen AA mode; otherwise bake a
  cheap vertex-colour AO into the Blender assets (§8) and skip SSAO.
- **Anti-aliasing**: MSAA 4x or TAA, whichever coexists with the effects above.
  Thin ribbons (roads) and rooflines alias badly without it.
- **Fog/haze**: a distance-based warm haze tinted by smoke density near the
  front. Because a custom `Material` gets no fog (finding 22), the water and
  fire shaders must apply it themselves; reuse `apply_scene_fog`.
- **Auto exposure** is optional; a hand-tuned exposure per time-of-day stop is
  more predictable on a kiosk.

### 7.3 Tilt-shift and depth of field
- Bevy 0.14 has a built-in `DepthOfField` (gaussian/bokeh). Try it first with a
  focal distance on the town centre for a free miniature look.
- A true tilt-shift (blur by *screen-space vertical distance*, not depth) is a
  small custom post-process pass: sharp horizontal band, blur ramping to the top
  and bottom edges, strength ~1-2 percent of screen height at the extremes. Budget
  half a day. Cut it, not the legibility work, if it costs frames.
- Never blur the HUD or any overlay; apply before egui and before screen-space
  markers.

### 7.4 Fire, smoke and embers (the hero effect)
- **Front**: emissive orange to yellow along the active edge by age/intensity
  (`get_fireline_int`), dark red-black char behind it, a faint ash-grey at
  burn-out. Animated flicker via a noise-scrolled emissive in `fire_shader.rs`,
  not by re-meshing.
- **Flame billboards/particles** along the head only, scaled by fireline
  intensity (flame length `L = 0.0775 * I^0.46`, finding 6) so a crowning
  conifer run visibly towers over a grass creep. This is a teaching visual, not
  decoration: bigger flames = more danger.
- **Smoke**: soft, lit, wind-bent columns that lean *downwind* and darken with
  intensity; ground haze drifting with the wind. Layered billboard particles
  with depth-fade are enough; volumetric fog is an option only if frame rate
  allows.
- **Embers**: bright streaks flung downwind from the front. When one lands and
  starts a spot fire, a visible arc and a flash make the mechanism readable
  before the advisor even says it.
- **Dusk glow**: fire casts a warm point or area light onto nearby houses and
  trees (cap the count; fake with emissive vertex tint on the houses in
  `buildings.rs` if real lights are too costly).
- **Aircraft drops**: a visible tanker flying a straight run, a spreading
  water cloud and a darkened wet strip on the ground that fades with the
  moisture decay (finding 15).
- **Fireline**: a pale cut strip drawn on the ground where a crew has worked.
- **Hose and water**: a thin animated stream from an engine to its target.

### 7.5 Terrain, vegetation and water
- Stylised terrain shading: fuel-class colour blending with a gentle height
  gradient, subtle contour lines for relief readability at the diorama angle.
  Keep the existing 5 m posting; the demo windows are small so the vertex
  budget is generous.
- Vegetation: Blender instanced props (§8) with a little wind sway
  (vertex-shader sine, driven by the real wind speed and direction) so the wind
  is visible in the scenery, not only in the HUD arrow.
- Water: keep the custom shader; add a soft shoreline foam fade and a subtle
  specular glint. Boats at anchor for `demo_porto`.
- Roads: keep casing-plus-surface ribbons, but lighter asphalt and clearer
  junctions so queues read; add small lane markings on the main roads if cheap.
  Re-check winding on any new draped strip (finding 11).

### 7.6 Make the invisible visible
Per `CLAUDE.md`, "nothing new is drawn on the map". All overlays sit at **+20 m
or higher** (finding 13) and are verified by actually looking at a screenshot:
- Spot fire: pulsing ring plus "!" billboard for ~20 s.
- Threat tint on terrain around the front (from `ThreatField`), a soft red
  gradient, not a hard polygon.
- Household beacons: green safe / amber preparing / red threatened, with a gentle
  pulse for "threatened and not yet leaving".
- Evacuation flow: car and walker trails or chevrons toward refuges; refuges and
  havens glow.
- Closed road: barrier icon. Burnt mast: broken-mast icon, with the lost
  coverage shown on the Allarmi layer.
- Layers as icon toggles replacing keys 1-4: **Fuoco**, **Evacuazione**,
  **Traffico**, **Allarmi**, each with a legend.
- Selection: a clear ring and a small floating info card (traits an agent could
  know, in the spirit of finding 30), not the researcher inspector.

### 7.7 Buildings, people and vehicles
- **Building states** read at a glance: intact, threatened (warm rim light /
  heat shimmer), alight (flames and glow), charred (dark roof, missing section).
  **Take screenshots first**: the open question in `CLAUDE.md` is that nobody has
  looked at alight and charred buildings since shrub spotting made them
  reachable.
- Keep the 3x figure scale. Colour people by state, a small bob when walking,
  a headlight blink on cars, a short trail behind moving vehicles. A puff when a
  house ignites; a chime-and-sparkle when a family reaches safety.

### 7.8 UI polish
- A consistent icon set (SVG/PNG, drawn once) for buttons and layers; soft
  glass-style panels; rounded corners; drop shadows under floating cards.
- Motion: counters tick with a short ease, cards slide in, the decision pause
  dims and slightly desaturates the world behind the card. Keep every
  transition under ~250 ms.
- Typography: one rounded, highly legible font, large sizes (§6), tabular
  figures for counters so they do not jitter.

### 7.9 Frame budget and fallbacks
Decide on day 1 by measurement. Order to disable if frames drop, cheapest loss
first: tilt-shift, SSAO, volumetric haze, point lights from fire, cascade count,
shadow resolution, MSAA to FXAA. Never disable: bloom on fire, the threat tint,
beacons, the wind arrow. Provide a single `quality` setting (low/medium/high)
in the operator panel so a slower machine on the day can be fixed without code.

## 8. Assets (Blender)

A Blender pipeline already exists and **must be reused, not replaced**:
`assets/models/emergency_assets.blend` → `scripts/build_models.py` →
`assets/models/meshes.json`, embedded via `include_str!` in
`crates/game/src/models.rs` (one mesh + material per symbol; vertex colours
and a `wood` flag per vertex). Read `assets/models/README.md` before touching
it. Keep the embed so native and any later web build load identically.

New or restyled assets (toy/miniature look, flat vertex colours, low poly,
< 2 k tris each, shared palette):

- Houses: 4–5 Mediterranean variants, plus a **burning** and a **charred** variant of each (or a shader tint, if cheaper).
- Village landmarks: church with campanile, town hall, school, fire station, a harbour crane/boat for `demo_porto`.
- Vegetation: pine, maquis clump, olive, grass tuft, as instanced props.
- Vehicles: car, fire engine, crew van, tanker aircraft; people as simple figures.
- Icons for the action bar and layers (can be 2D SVG/PNG in egui rather than Blender).
- A **table base / plinth** mesh for the diorama edge.
- Bake cheap vertex-colour **ambient occlusion** into every asset (darker at creases and contact points): free at run time, and it carries the miniature look even with SSAO off.
- Lower-poly variants for far instances if the town needs them; check the triangle total in a screenshot pass.

Pipeline rule: assets are baked offline to `meshes.json`; nothing in the game
reads `.blend` at run time (matches the project's "scripts never run at game
time" rule).

## 9. Model invariants (do not break)

The demo is a presentation layer over a measured model. Hard rules:

1. **No change to `crates/scenario`, `crates/fire`, `crates/abm`, `crates/behavior`
   behaviour or numbers.** New scenarios are *data*; new mission logic is in
   `crates/demo`. If a model change is unavoidable, stop and ask — every figure
   in `CLAUDE.md` was measured against the current model.
2. Anything accumulated per update is a bug (finding 5); keep using
   `Sim::advance` so single-step and play take the same path.
3. Step size: demo speed is a fixed simulated-seconds-per-real-second; verify the
   per-frame cap (30 simulated s) never binds at the chosen speed.
4. A restart clears latched view state (finding 21). Test it: run, restart, and
   assert the new run opens clean.
5. Determinism: same scenario + seed + orders ⇒ same outcome. The COMPARE
   screen depends on it.

## 10. Testing

Headless, under `crates/demo/tests/` (or `crates/abm/tests/` for model-side
facts), in the style of the existing `*_report` / assertion split:

- **Beat tests** — one per scenario, asserting the mechanism *fires*, not that
  the run completes (findings 26, 35, 39): `demo_borgo` — ordering at T+0 saves
  strictly more households than ordering at T+20 min, on a five-seed mean;
  `demo_valle` — the wind shift puts the second hamlet in the threat field;
  `demo_porto` — a queue forms on the exit and at least one spot fire occurs.
- **Counterfactual test** — the "no orders" run is worse than a competent run on
  each scenario, with margin, or the COMPARE screen has nothing to say.
- **Playability budget** — a scripted "good" commander and a scripted "idle"
  commander both reach OUTCOME inside the mission length.
- **Idle reset** — simulated idle drives ATTRACT with no leaked state.
- A `DEMO_SELFTEST=1` mode mirroring `SPOTORNO_SELFTEST=1` for the Bevy-side
  parts (state machine, reset fan-out, order buttons) that only exist as
  resources and events.
- **Screenshots at T+10 / T+60 / end** via the existing `SPOTORNO_SHOT_*`-style
  capture, reviewed by eye. "Can I see it" is the only check for graphics.

## 11. Performance and robustness

- Target **60 fps** on the actual demo machine; **30 fps floor**. Measure on
  that hardware on day 1, before building more on top.
- If it does not hold: reduce render-terrain factor, cut tilt-shift, then cap
  instance counts. The demo worlds are 3–5 km so the 4 M-vertex budget of the
  real windows does not apply — choose the render posting for the smaller
  window (a 5 m posting over 4 km is ~0.64 M verts).
- **Crash safety:** a supervisor script (`scripts/run_demo.sh`) relaunches the
  binary on exit. Ship a looping pre-recorded attract video as a fallback
  screen.
- Offline: no network access anywhere in the demo. No LLM, no telemetry upload.
- Fullscreen, borderless, cursor hidden outside the play area, OS sleep and
  screensaver disabled (operator checklist, §13).

## 12. Localisation

Italian only for the demo UI. Every user-visible string lives in one
`strings_it.rs` (or `.toml`), looked up by key — no literals in widgets — so a
second language is a table, not a hunt. Advisor text, briefing, decision cards,
refusals and the outcome takeaway are all in that table. Have a native speaker
read it before the event.

## 13. Operator notes (ship as `docs/demo-operator.md`)

One page: how to launch, the operator chord, how to reset, what to do if frozen,
power/sleep settings, the three scenarios in one sentence each, and the
talking points a staff member can use ("why did the wind change the result?").

## 14. Milestones

| Days | Deliverable | Gate |
|---|---|---|
| 1 | `crates/demo` skeleton on a copied terrain + one synthetic town; kiosk shell, state machine, idle reset; **frame rate measured on target hardware**; screenshots of damage states | fps ≥ 30 on the demo machine |
| 2 | Action bar, HUD counters, orders working end to end on `demo_borgo` | a stranger can place an order unaided |
| 3 | Outcome card + counterfactual; briefing; scripted pauses | full loop ATTRACT→OUTCOME→ATTRACT |
| 4 | Overlays/layers, advisors, spot-fire and wind visuals; `demo_valle` | all §7.2 visible in a screenshot |
| 5 | `demo_porto`; Blender assets in; diorama look; audio | all three scenarios pass beat tests |
| 6 | Italian review, operator doc, supervisor script, **playtest on real hardware with 3–5 people who have not seen it** | no one gets stuck in under 60 s |
| 7 | Buffer, fixes only. Freeze at the end of day 6 if possible. | |

If days slip: cut, in order — tilt-shift, audio, `demo_porto`, advisors (fall
back to a plain ticker), custom assets (keep the existing Blender set). Never
cut: the kiosk shell, idle reset, outcome + counterfactual, the wind arrow.

## 15. Acceptance

1. Cold start to PLAY in ≤30 s with no explanation from staff.
2. A full session is 3–5 min and ends on the outcome card without an operator.
3. Idle for 60 s anywhere returns to ATTRACT with a clean state.
4. All three scenarios: good play beats idle on families safe, by the margin the
   beat tests assert.
5. No single-key shortcut does anything; the operator chord does.
6. No English strings visible to the player.
7. `cargo test --release` still passes at the repository root with the model
   crates untouched (`git diff` shows no changes under `crates/{scenario,fire,abm,behavior}`).

## 16. Decisions and remaining questions

Settled with Mirko on 2026-10-06:
- **Hardware**: a capable desktop with a mouse. No touch. The §7 graphics budget
  assumes a modern discrete GPU; still measure on the day-1 gate. Buttons stay
  large (read from a distance) but hover states and a visible cursor are fine.
- **Outcome card**: facts plus the counterfactual comparison are acceptable; no
  numeric score.
- **Aircraft**: yes, expose the air tankers. `CLAUDE.md`'s decisions table has
  been updated to match.
- **Audience**: students. Tone is clear and direct, not childish; the loss
  framing ("case perse") stays but never dwells on harm to people.

Remaining:
- **Names**: are the three fictional town names fine, or should they nod to a
  Roman-region setting?
- **GPU model and resolution** on the actual machine (sets the default quality
  preset in §7.9).
- **Sound**: will the gazebo have speakers, or is it a noisy open space where
  audio should be optional and captions carry everything?
