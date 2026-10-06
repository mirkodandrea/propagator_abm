# Demo — spec and remaining work

Interactive kiosk demo for students visiting the gazebo at the *Settimana della
Protezione Civile*, Rome. Spec written 2026-10-06 and updated the same day after
the first playable loop; event about a week out. Companion: `docs/demo-notes.md`
(gameplay critique and ideas). Read `CLAUDE.md` for the model's findings (they are
constraints), keeping in mind it still describes the workbench, composer and live
debugger, which are removed or about to be (§4).

**There is one mode.** The game *is* the kiosk: `cargo run --release -p game`, no
`DEMO` flag, no workbench. Env vars that remain are test/ops harnesses only
(`KIOSK_SHOT`, `KIOSK_TOWN`, `KIOSK_WINDOWED`, `KIOSK_PLAY_S`).

Legend: ✅ done · 🔲 to do · ✂ cut-list item (drop first if days slip).

## 1. Goal

A student walks up cold and, within 30 s, is making decisions as an incident
commander in a small stylised town, watching individual families respond to a
fire. A session is **3–5 min**, ends on a clear outcome, and invites a second
try. Success: a bystander says unprompted *"the fire goes where the wind blows,
people need warning early, but not too early, and my decisions changed who got
out."*

**The branch's purpose is a clean demo game.** Everything that is not the demo is
to be deleted (§4), not kept alive behind a flag.

## 2. Decisions and non-goals

Settled with Mirko:
- Hardware: capable desktop, mouse only, no touch, **no speakers** (captions
  only, no audio). Audience: students; tone clear, not childish; loss framing
  never dwells on harm.
- **Setting: Roman region.** Towns are fictional but Lazio-flavoured (placeholder
  names: Rocca Ventosa, Due Casali, Porto Pineta; native-speaker review needed).
- **Graphics target: the *Link's Awakening* (2019 Switch remake) diorama look** —
  a toy-like miniature on a plinth: glossy plastic-smooth chunky models, soft
  saturated pastel palette, rounded shapes, strong **tilt-shift** (sharp band in
  the middle, blur toward top and bottom edges), soft shadows, a table-top edge
  with the world visibly ending. Solid 30 fps on modest hardware: the look comes
  from art direction and one post pass, not from expensive effects.
- **Visual identity: CIMA Foundation** (cimafoundation.org): navy `#001E31`, blue
  `#004070`, orange `#DD7500`, pale grey `#D4DBDE`, and the white mark
  (`assets/brand/cima_logo_white.png`). ✅ in the UI; the same palette should
  guide the in-world look.
- Facts + counterfactual on the end card; **no numeric score**. (Money, §6.5, is
  a *cost shown*, not a score.)
- Air tankers (Canadair ×2) are exposed.
- **LLM bubbles: OpenRouter** (needs network at the gazebo; canned fallback is
  mandatory).
- **The model may be changed** (experimental branch, no back-compat). Every change
  keeps `cargo test --release` green.
- **No day/night cycle.** One still, interesting lighting (§8).

Non-goals: real places or historical fires (**no `spotorno`, `mati`, `pedrogao`,
`rhodes`**); zoning/budgets beyond the intervention cost in §6.5; web build;
debrief; control API in the kiosk; persistent leaderboard.

## 3. Done

- ✅ **Three towns** (`scripts/generate_demo_scenarios.py` →
  `data/scenarios/demo_{borgo,valle,porto}`, 250/300/350 households, 4 km, 20 m
  grid). Populations authored *complacent*.
- ✅ **`crates/demo`** headless lib: `mission::spec` (pinned opening conditions,
  `demo_valle` wind shift), `run::Run` (fire-then-agents at the 6 s step),
  `run::Tally` (shared outcome counting: `caught`, `homes_lost`, …).
- ✅ **Model:** `abm::SEE_RANGE_M` 2500 → 800 m; behaviour-graph distance
  observations keep their own 2500 m cap (`OBS_RANGE_M`).
- ✅ **Beat tests** (`crates/demo/tests/towns.rs`, five-seed means, caught at home):
  borgo 32 / 13 / 20 / 32 and porto 43 / 20 / 22 / 34 for none / T+0 / T+10 / T+20;
  valle hamlet B 13 → 4 with an early order; towns load with connected roads; an
  order moves people; runs are deterministic.
- ✅ **`homes_lost` no longer saturates.** It was "alight", which firebrands
  (2.5 km reach) set on every house whatever the player did (borgo 250/250). Now
  "households with burnt ground within 150 m" (`LOST_RADIUS_M`, measured in
  `tests/lost.rs`): 54 / 15 / 47 on borgo / valle / porto. UI label "case colpite".
  An order never changes it; only suppression does — a lesson, not a bug.
- ✅ **Kiosk shell** (`crates/game/src/kiosk/`): attract / briefing /
  play / outcome / compare; idle reset (60 s; 90 s + 30 s in play); hidden
  operator corner (hold top-right 3 s); fixed 6 s steps shared with `demo::Run`;
  counterfactual twin computed on a thread at load; clamped camera; all strings in
  `strings_it.rs`; **no shortcut can fire** (`UiFocus::keyboard` held true).
- ✅ **UI v1** in CIMA colours: painted pictograms, counters, live wind compass,
  action bar (Evacuazione · Squadra · Autobotte · Canadair · Pausa), outcome and
  compare cards, CIMA mark. Screenshot harness `KIOSK_SHOT=<dir>`,
  `KIOSK_PLAY_S`, `KIOSK_TOWN`, `KIOSK_WINDOWED`.
- ✅ **Removed** the composer, behaviour/debug tabs, `egui-snarl`, and the `DEMO`
  flag (kiosk mode is unconditional; `workbench_systems` is deleted, its modules are
  now unreferenced and go in §4).
- ✅ **§6.4 opening fire** at the 60 m floor, beats re-measured, borgo forestry road.
- ✅ **§6.1 v1**: seeded weather draw per session, calibrated two-issue forecast,
  forecast card in briefing and play (`demo::weather`, `tests/forecast.rs`).
- ✅ `demo::Run` takes unit orders (`Order::Attack`, `Order::Drop`) so the twin can
  price them.

## 4. Clean-up: delete everything that is not the demo 🔲 (in progress)

Status:
- ✅ real-data scripts (kept `generate_demo_scenarios.py`, `bake_fuels.py`,
  `build_models.py`), `tools/mcp`, `web/`, Pages workflow, docs for removed tools
  (`web`, `behavior-workspace`, `renderers`, `ux-playtest`, `il-gioco`); the `DEMO`
  flag and `workbench_systems`.
- 🔲 real/lab scenario data and the tests that load them (port to `demo_*` or
  delete; keep pins for findings 5, 17–19, 34, 39–42).
- 🔲 the now-unreferenced workbench modules in `crates/game` (list below).
- 🔲 `crates/telemetry`, wasm profile, far terrain/sea/sky clock, CLAUDE.md rewrite,
  crate layout.

The branch ends as a demo-only codebase. Do this **before** new features so
nothing new is built on code about to be removed. Do it in small commits, tests
green after each.

Delete:
- **Real scenarios and their pipeline**: `data/scenarios/{spotorno,mati,pedrogao,
  rhodes}`, `data/spotorno_*`, `data/osm_raw.json`, the synthetic ABM labs
  (`abm_micro`, `test_small`, `town_scale`, `mass_evacuation`,
  `congestion_funnel`, …), `scripts/{clip_cogs,fetch_osm,build_render_terrain,
  generate_population,bake_fire_rasters,write_scenario_json,bake_fuels,
  generate_synthetic_scenarios}.py` and `places.py` — keep
  `generate_demo_scenarios.py` and **`bake_fuels.py`'s output** (`data/fuels_eu12.json`
  is shared: keep the file, drop the script's real-data parts).
- **Workbench UI in `crates/game`**: `menu.rs`, `ui.rs` panels, `browser.rs`,
  `scenario_selector.rs`, `ignition_edit.rs`, `inspect.rs` panels (keep selection
  ring only if §7.6 needs it), `selftest.rs`, `api.rs` + `tiny_http`,
  `interview.rs` (the LLM *interview window*; keep `crates/chat` for bubbles),
  `map2d.rs`, `history.rs` if unused, `native_accessibility.rs`, `web_clipboard.rs`,
  `native_text_input.rs`, `capture.rs` scripted layers (keep the screenshot
  harness in `kiosk::shots`).
- **`crates/telemetry`** unless the event log is wanted for the outcome card;
  **web/wasm** targets and `wasm-release` profile; `docs/` for removed tools;
  `tools/mcp`.
- **Far terrain, 25 km sea, day/night sky** (`far_terrain.rs`, `sea.rs`, `sky.rs`
  clock) — replaced by a plinth and a fixed light (§8).
- **CLAUDE.md**: rewrite to describe only the demo; move the findings that still
  bind (2, 3, 5, 7, 11–14, 17–19, 21–23, 25, 34, 37, 39–41) and drop the rest.

Keep (the model): `scenario`, `fire`, `abm`, `behavior` (the graphs are the
decision layer — `abm` has no second implementation, so the library stays; the
*editor* is gone), `propagator-core`, `chat`.

Resulting layout: either rename `game` → `demo` and fold `crates/demo` (headless)
into it as `demo::headless`, or keep two crates (`demo` lib + `demo-app` bin). Pick
the simpler one; the headless tests must stay window-free.

## 5. Session flow ✅ (v1), 🔲 changes below

```
ATTRACT ──click──▶ BRIEFING ──▶ PLAY ──▶ OUTCOME ──▶ COMPARE ──▶ retry | ATTRACT
   ▲                                                                   │
   └────────────── 60 s idle on any screen except PLAY ────────────────┘
```

- ATTRACT: slow orbit, fire burning in accelerated time. BRIEFING (≤20 s): one
  card, fly-in. PLAY: ~3 min real time = the mission at a fixed speed (verify the
  30 s per-frame step cap never binds). OUTCOME then COMPARE (identical fire, no
  orders, computed at load). Idle: 60 s outside PLAY → ATTRACT; in PLAY 90 s →
  "Sei ancora lì?", reset after 30 s more. Restart goes through the reset path
  (finding 21).
- 🔲 The briefing now includes the **forecast** (§6.1).
- 🔲 COMPARE must still be *the same fire*: with random weather (§6.1) the twin
  gets the same drawn weather and seed, orders removed.

## 6. Next gameplay step

The first loop is "press buttons, watch" (see `demo-notes.md`). The next step gives
the player *uncertainty, consequences and cost*. All six items below are in scope;
they interact, so build them in the order given.

### 6.1 Varying weather and imperfect forecasts ✅ (v1: draw, forecast card), 🔲 (usefulness test needs §6.5 cost)
- Each session draws its weather from a **seeded distribution per town** (wind
  direction and speed, fuel moisture, and *when/whether* a shift happens), seed
  recorded so the COMPARE twin and tests reproduce it.
- The player sees a **forecast**, not the truth: wind bearing/speed with an
  uncertainty cone, a "probabilità di cambio vento" figure, a forecast hour that
  refreshes (and can contradict the earlier one). The compass shows the *observed*
  wind; the forecast is a separate, honestly-wrong card.
- Forecast error is drawn from the same seed: sometimes the shift arrives early,
  late or not at all. A cautious player pays (needless evacuation, §6.2 and §6.5);
  a trusting player is caught. **The lesson is acting under uncertainty**, so the
  forecast must be *useful on average and wrong sometimes*, not noise. Measure
  that (a test: following the forecast beats ignoring it over many seeds, but not
  on every seed).
- Replaces the single scripted `demo_valle` shift with a drawn one (valle keeps a
  high shift probability).
- **Done** (`demo::weather`, `tests/forecast.rs`, kiosk `forecast_card`): per-session seed
  (new visitor = new draw, Riprova = same one); wind ±15° / ±5 km/h / ±1.5 % moisture;
  shift chance drawn per session (borgo/porto 15–85 %, valle 60–95 %), rolled against.
  Issue 1 at the briefing reports that chance in 5 % steps; issue 2 at T+15 leans
  toward the truth but still contradicts issue 1 in some sessions. Measured
  calibration: mean forecast 0.50 vs 0.51 real; sessions rated ≥60 % shift 73 %,
  ≤40 % shift 28 %. In borgo/porto a shift turns the fire off the town (caught
  0 vs 29–37), which is what makes the forecast a real decision. The COMPARE twin
  uses the same draw and seed.
- **Not done:** "following the forecast beats ignoring it" needs a price on acting
  (§6.5) — with orders free, evacuating at T+0 always wins. (Measured, §16 item 13-14.)

### 6.2 Population anger / trust 🔲
- A **needless** evacuation order (the fire would not have reached the household,
  decided afterwards against the twin or by a forward threat estimate) lowers the
  trust of those households in the *next* order: `trust_authority` drops, and their
  `prep_time`/confirmation delay rises (finding 42's `order_confirmation` block is
  the existing hook). Households that evacuated needlessly and are then told to
  stay show *anger* in the HUD ("Fiducia: 71 % → 54 %").
- Visible consequence: a later, *needed* order is obeyed by fewer families. This is
  the cry-wolf effect, and it is the tradeoff that stops "evacuate at T+0" being
  the dominant strategy.
- Must be **provably inert** until an order is given (finding 34): nothing changes
  at T+0, and the shipped figures in `crates/fire/tests` stay unchanged. Pin both
  halves in a test (branch fires; shipped runs identical).
- Tuning is measured, not guessed: a sweep of (early-needless order, later-needed
  order) pairs across seeds.

### 6.3 Spotting on, multiple fires 🔲
- Shrub spotting is already enabled in the model (finding 41); the demo must make
  it *readable*: a spot fire appears as a new ring with a "!" and a one-line advisor
  call ("Nuovo focolaio a est del paese"), and the player can send units to it.
- Ensure every town produces **≥1 spot fire** in a typical run (assert it per town
  in `tests/towns.rs`; `demo_porto` is built around it).
- Spot fires are a reason to **split units**: the player must prioritise.

### 6.4 Start with a very small fire ✅ (fire), 🔲 (suppression)
- Opening fire becomes small (a few hectares, radius ~40–60 m) so that **early
  action visibly works**: a crew or an engine at T+3 min can stop it, a late one
  cannot. Today's radius is 120–150 m, already a going fire.
- This changes the pacing: the first minutes are about *whether* to commit
  resources, the later minutes about *where the weather takes it*. It also makes the
  "start small, grows with wind" story the briefing tells.
- Constraint from finding 3: single-cell ignitions fizzle ~20 % of seeds, and
  `MIN_IGNITION_RADIUS_M` is 60 m for exactly that reason. Either keep 60 m as the
  floor (then "very small" means ~1–2 ha) or **seed-filter** (reject draws that do
  not establish, deterministically). Measure which; do not ship a coin flip.
- Re-measure every beat test (§3) — they were taken on the larger fire.
- **Done:** all three towns open at the 60 m floor (~1 ha at T+3, 18–55 ha at the end;
  establishes on 24/24 seed-town runs, so no seed filter). Beats re-measured
  (`tests/small.rs`, five-seed means, caught at home): borgo 38 / 14 / 17 / 38 for
  none / T+0 / T+10 / **T+40**; porto 30 / 10 / 11 / 27; valle hamlet B 18 → 4. A small
  fire arrives later, so the "late order" is now T+40, not T+20. Valle's shift is at
  T+30, 45 km/h, ignition x=2200. Borgo gained a forestry road to its fire.
- **Finding, not done** (re-confirmed over 16 seeds, §16 item 17): a crew or engine at T+3 does *not* stop the fire in this model.
  Measured at 10 km/h with all six ground units: 3.8 ha against 4.0 untouched. The
  engine works 14 m from the front, but pumps 6 min then spends ~10 on a hydrant
  round trip; the front is dozens of 20 m cells; crews cut ~0 m (120 m/h, withdraw
  under heat). That follows from findings 15, 16 and 41, not from the demo. Making
  early action visible needs a deliberate change in `abm` (spec §10 forbids it for
  this step): a demo-only effectiveness knob, inert by default, is the candidate.
  `Run` now takes `Order::Attack`/`Order::Drop` so the twin can price them.

### 6.5 Extinction cost (money) 🔲
- Every intervention costs money; a small running total ("Spesa: 18.400 €") is
  shown, never as a score. Illustrative unit costs (to be set from published
  figures and written down in code with their source): Canadair drop per load,
  engine per hour, hand crew per hour, a general evacuation (lost working day,
  buses), a *needless* evacuation costing the same as a needed one.
- It is the other half of the tradeoff in §6.2: acting is not free, waiting is not
  free. The outcome card shows **money spent beside what it saved** (families safe,
  hectares) and, in COMPARE, "senza ordini: 0 €".
- Needs per-action accounting in `Suppression` (units already track `drops`,
  `water_used_l`, `line_cut_m`) and a `demo::cost` module so the headless twin and
  the live game price identically. Test: cost is a pure function of the action
  log.

### 6.6 Interactivity beyond buttons 🔲
Not in this step's scope but it shapes the UI of the above: drawing evacuation
zones and fire lines on the map, unit chips with one-sentence status, radio calls
that ask for a decision, tapping a family to hear it (LLM, §7.9). See
`demo-notes.md` §1.

## 7. UI

Full-screen borderless, cursor visible, **no single-key shortcuts** (finding 25),
hidden operator corner (✅) for restart, pin town, next town. egui at 1.5–2×, CIMA
navy/orange, large rounded targets (✅).

- ✅ **7.1 Action bar** and **7.2 HUD** v1 (above). 🔲 add: forecast card (§6.1),
  trust meter (§6.2), money counter (§6.5), a refusal/banner line (✅ banner exists).
- 🔲 **7.3 Advisors**: Capo squadra (fire), Polizia locale (roads), Sindaco
  (warnings / trust). Templated Italian from `Sim` events, one bubble at a time,
  ≥8 s apart, fired on a *change*. One `advisors.rs` table of
  `(trigger, speaker, text_it)`. Spot fires and forecast updates are triggers.
- 🔲 **7.4 Decision pauses**: scripted per scenario as data; dim map, card with 2–3
  big choices incl. "aspetta". Prefer events generated from real model changes
  (road cut, spot fire, forecast changed) to hand scripting.
- ✅ **7.5 Outcome/Compare** v1. 🔲 add money (§6.5) and trust at the end.
- 🔲 **7.6 Overlays** (≥+20 m above ground, finding 13; verify by screenshot): wind
  arrow *on the terrain*, spot-fire ring + "!", threat tint, household beacons
  (green / amber / red), evacuation chevrons, closed-road icons, selection ring.
- ✅ **7.7 Localisation** structure (one `strings_it.rs`); 🔲 native-speaker review.
- 🔲 **7.9 NPC speech bubbles (OpenRouter)** ✂: short in-character bubbles via
  `crates/chat` (finding 30: only the agent's own traits, senses and event log).
  Hard timeout, canned templated fallback, rate-limited, never pauses the
  incident. Angry households (§6.2) are the natural speakers.

## 8. Graphics — Link's Awakening diorama, one still light

Reference: the 2019 *Link's Awakening* remake. Priority: legibility from 2 m, then
the toy-diorama read, then polish. Fire is the only truly bright, saturated thing
against a calm pastel world. **Fixed lighting; no day/night cycle.**

- 🔲 **Tilt-shift is core, no longer cut-list.** One cheap full-screen post pass:
  blur ramps in with distance from a horizontal focus band (and a slight
  vignette/saturation lift), never applied to egui. Fallback: Bevy's built-in
  `DepthOfField`. Keep the focus band on the town so houses, people and cars stay
  crisp; the blurred edges hide the world's end.
- 🔲 **Lighting**: one warm, low-angle sun that never moves, soft shadows (few
  cascades, large softness), generous ambient so shadows stay pastel not black,
  TonyMcMapface/AgX, bloom threshold high so only fire/embers/beacons glow. Remove
  the simulated clock from `sky.rs` (static rig, no time-of-day UI). Pick the light
  by screenshot against the reference.
- 🔲 **The diorama**: terrain as a raised **plinth/tile block** with visible sides
  (layered earth strata), a wooden/stone base, slightly rounded corners; the sea,
  where there is one, is a glossy tile *inside* the block. Drop `far_terrain` and
  the 25 km sea; the horizon is the table, softly out of focus.
- 🔲 **Materials**: smooth, slightly glossy "plastic" look — flat or two-tone
  shading, gentle specular, vertex-colour palettes (CIMA-tinted), thin darker
  outlines optional (inverted-hull) on buildings and figures only if cheap.
- 🔲 **Buildings** (priority): chunky rounded low-poly houses with clearly different
  roofs, chimneys and little gardens; church, town hall, school, fire station.
  States intact / threatened / alight / charred read at a glance by colour, smoke
  and glow.
- 🔲 **Vegetation**: big round-topped trees and bushes as single chunky props (a few
  hundred, not 230 k plants), fuel class readable by tree *type* and ground tint;
  gentle wind sway by a vertex sine. Large frame-rate win.
- 🔲 **People and cars**: round, bright, slightly oversized figures (keep the 3×
  scale) with a walking bob, family groups; chunky toy cars with headlight blink
  and queues that read at the exit; engine, crew van and Canadair with distinct
  silhouettes. Shelters/refuges are drawn as *places* (piazza, car park, quay,
  with a sign), and sheltering groups are visible.
- 🔲 **Fire/smoke/embers**: stylised — layered flame billboards that bob and flicker,
  soft round smoke puffs bent by the wind, ember streaks with a flash on spot-fire
  landing, water-drop cloud and a darkened wet strip, fireline strip, hose stream.
- **Frame budget**: 30 fps floor, decided by measurement on day 1. Disable in order:
  shadow resolution, cascades, bloom quality, tilt-shift sample count, MSAA→FXAA.
  Never disable: fire glow, beacons, wind arrow, the tilt-shift read (reduce
  samples instead). Operator `quality` low/medium/high.

## 9. Assets 🔲

Reuse the existing pipeline: `assets/models/emergency_assets.blend` →
`scripts/build_models.py` → `meshes.json` (embedded, vertex colours; read
`assets/models/README.md`). New or restyled, low-poly (<2 k tris), a shared palette
tied to CIMA colours, baked vertex AO: house variants (+ charred), civic buildings,
pine / maquis / olive / grass props, car / fire engine / crew van / tanker
aircraft, simple figures, plinth, action-bar icons (2D, painted in egui ✅).
✂ custom assets (keep the existing set) if days slip.

## 10. Model rules

The model may change for the demo, but: `Sim::advance` is the one stepping path
(finding 5); demo speed is a fixed simulated-s per real-s; a restart clears
latched view state (finding 21) and is tested; same scenario + seed + weather draw +
orders ⇒ same outcome (COMPARE depends on it; tested); any new mechanism (§6.2
anger, §6.5 cost, forecast) is **provably inert until used** (finding 34); a tuned
constant is per-scenario (finding 38); every new behaviour gets a test that asserts
it *fires*, not that the model runs (findings 26, 35, 42); `cargo test --release`
stays green.

## 11. Testing

- ✅ Beat tests (§3). 🔲 Re-measure after §6.4 (small fire) and add: forecast
  usefulness (§6.1), anger fires and is inert at T+0 (§6.2), ≥1 spot fire per town
  (§6.3), early crew stops the small fire and a late one does not (§6.4), cost is a
  pure function of the action log (§6.5), good scripted commander beats "no orders"
  on families safe **and** money spent is plausible, idle commander still reaches
  OUTCOME (playability), refuge/haven/mast coverage per town (findings 9, 34).
- 🔲 A `KIOSK_SELFTEST=1` harness (not a mode) for the Bevy side: state machine, reset fan-out, order
  buttons, "no key does anything", idle-reset leaks nothing.
- 🔲 Screenshots at T+10 / T+60 / end, reviewed by eye (✅ harness exists).
- 🔲 Frame-rate measurement on the real machine: 30 fps floor.

## 12. Robustness and ops 🔲

`scripts/run_demo.sh` supervisor relaunches on exit; fully offline **except**
OpenRouter bubbles (fallback when the network is down); fullscreen borderless; OS
sleep/screensaver off. Ship `docs/demo-operator.md` (one page: launch, operator
corner, reset, frozen-screen procedure, power settings, the three towns in one
sentence each, staff talking points).

## 13. Milestones (remaining)

| Step | Deliverable | Gate |
|---|---|---|
| 1 | §4 clean-up: demo-only repo, CLAUDE.md rewritten, tests green (**partly done**, §4 status) | `cargo test --release` green, `cargo build` small |
| 2 | §6.4 small fire + re-measured beats ✅; §6.3 spot-fire visible 🔲 | ≥1 spot per town; early action visibly works 🔲 (needs a model decision, §6.4) |
| 3 | §6.1 draw + forecast card ✅; §6.2 trust/anger 🔲 | forecast useful-but-wrong test (needs §6.5); cry-wolf test |
| 4 | §6.5 cost accounting, outcome card with money | twin and live price identically |
| 5 | §8 diorama pass: fixed light, plinth, tilt-shift, buildings, people, cars | screenshot beside the reference; 30 fps |
| 6 | Advisors, overlays, Italian review, operator doc, supervisor script | **playtest with 3–5 new people**, no one stuck >60 s |
| 7 | Buffer, fixes only; LLM bubbles if time | |

Cut order if days slip: LLM bubbles, outline shader, `demo_porto`,
advisors (plain ticker instead), custom assets. **Never cut**: kiosk shell, idle
reset, outcome + counterfactual, the wind arrow, the forecast (it is the new
point of the game).

## 14. Acceptance

1. Cold start to PLAY ≤30 s with no staff explanation.
2. Full session 3–5 min, ends on the outcome card without an operator.
3. 60 s idle anywhere returns to ATTRACT with clean state.
4. All three towns: good play beats idle on families safe, by the margin the beat
   tests assert, **and** a needless evacuation costs trust and money.
5. A forecast is shown, is right on average and wrong sometimes.
6. No single-key shortcut does anything; the operator corner does.
7. No English visible; no audio required.
8. `cargo test --release` passes; the repo contains no non-demo scenarios, UI or
   scripts.

## 15. Open questions

- Money: real unit costs and their sources (Protezione Civile / regional tariffs);
  shown in € per session.
- How is a *needless* order judged — against the twin (hindsight, exact, but
  unknown to the household at the time) or a forward threat estimate (what the
  commander could have known)? The twin is simplest; the estimate is fairer. Decide
  from a measurement of how often they disagree.
- Clean-up end state: one crate or two (§4).
- Final town names and Italian copy, native-speaker review.
- GPU/resolution on the actual machine (sets the default quality; target is modest, Switch-class).

## 16. Playtest findings (2026-10-06)

Method: `KIOSK_SHOT` walked all three towns (windowed, 3200x2000, `KIOSK_PLAY_S=25`, evacuation
ordered at T+0 by the harness), every PNG read; code read in `crates/game/src/kiosk/*`;
headless play with `demo::Run` over seeds 1-16 x 3 towns x 9 strategies (none, evacuate at
T+0/10/20/40, follow-forecast, units-only at T+3, evacuate+units); determinism (same seed
twice, all three towns) holds; `cargo test --release` green. Throwaway test deleted.
"Blocks never-cut" = touches kiosk shell, idle reset, outcome + counterfactual, wind arrow, or the forecast (§13).

Headline numbers (16-seed means; safe = card's "al sicuro"; caught = caught at home):

| town | none safe/caught | T+0 safe/caught | T+40 safe/caught | forecast-follower safe/caught | units only | homes hit (any strategy) |
|---|---|---|---|---|---|---|
| borgo (250) | 147 / 13.8 | 219 / 4.8 | 182 / 13.4 | 167 / 9.6 | 144 / 13.3 | 12.8 (units: 13.9) |
| valle (300) | 61 / 8.5 | 236 / 2.2 | 211 / 5.5 | 61 / 8.5 | 57 / 10.3 | 14.3 (units: 15.6) |
| porto (350) | 140 / 12.0 | 289 / 5.2 | 221 / 12.1 | 189 / 9.1 | 138 / 15.3 | 21.4 (units: 21.8) |

### Bugs

1. **Refusal/forecast banner is half hidden behind the action bar** — *major* (blocks "never cut": the forecast-update banner is the only prompt to look at the forecast). `ui.rs` `play`: `banner_y = screen.bottom() - 220`, pill ~48 px tall, but the action bar starts at `bottom - btn.1 - 56 = bottom - 204`. Screenshots (all 3 towns, `3_play.png`): "Nuove previsioni del vento: guardale!" is navy text on orange, cut by the bar's top edge. Same slot carries every unit refusal, so refusals are unreadable too. Fix: `banner_y = bottom - 204 - 70`; derive both from one constant.
2. **Forecast card title collides with the caveat** — *major* (blocks "never cut": forecast legibility). When the card says "Previsioni aggiornate" (22 px, left) the caveat "Sono previsioni: possono sbagliare" (14 px, right-aligned) is drawn over it (`forecast_card`, all 3 towns in play). Fix: move the caveat to its own line at the bottom, or shorten the title to "Aggiornate"; also 14 px is unreadable at 2 m — make it >= 18.
3. **Play-time idle reset can fire while the player is just watching** — *major*. `PLAY_IDLE_WARN_S` 90 + grace 30 = reset at 120 s of no mouse movement, but a session is 180 s and the natural play is "order, then watch the fire". Result: the dim "Sei ancora lì?" overlay at 90 s and a hard reset to the next town at 120 s with the student's session lost, no outcome shown. Pause does not suspend the idle clock either. Fix: in PLAY, count idle only while no order has been given, or make idle = no input AND paused/no order; at minimum warn at 150 s. Blocks "never cut: idle reset" being safe.
4. **Porto: the town is behind the HUD and the attract button** — *major*. `home_focus` nudges toward the ignition (inland, centre), so Porto Pineta's houses sit at the very bottom edge: under "Clicca per iniziare" in attract (`porto/1_attract.png`) and under the action bar in play (`porto/3_play.png`), only orange slivers show. The town whose lesson is "the road out burns" never shows its people or road. Fix: per-town focus in `Spec`/view.rs (porto: look at the town-road-fire triple, PLAY_DIST bigger), and test by screenshot.
5. **"Al sicuro" counts evacuees only, so doing nothing in a safe session reads as a disaster** — *major* (design bug in the outcome card). `Tally::outcome.safe = agents.stats().safe` (reached a refuge). Valle with no wind-shift (5/16 seeds): caught = 0 with no orders, yet "none" shows 0-6/300 safe. Borgo with a shift (9/16 seeds): caught 0 with no orders, safe 2-239. The card then says (takeaway branch 2) "Grazie ai tuoi ordini N famiglie in più sono arrivate al sicuro" — praising an evacuation nobody needed. Fix: add "non toccate dal fuoco" (never in danger and still home) to safe, or headline `caught`/"in pericolo" instead of `safe`; at minimum drop the `you_safe > cf_safe + 2` branch.
6. **Outcome and compare cards show different stats** — *minor*. Outcome: safe / "ancora in pericolo" / case colpite / ettari. Compare: safe / "sorprese in casa" / case colpite; no ettari, no in-pericolo. Outcome lines also do not add up (borgo: 215 safe + 15 in danger of 250; the other 20 are at home/defending/sheltering and are never named). Fix: same four rows on both, plus an "a casa, al riparo" row or fold into safe (see 5).
7. **Outcome headline duplicated** — *minor*. "215/250 famiglie al sicuro" appears twice on the card (headline + first stat row).
8. **Unmapped refusals fall through to a generic sentence** — *minor*. `strings_it::refusal` handles the English strings from `command::target_preview` but not "unit is lost", "not on the incident: request air support first", "Target is not workable.", "Select an available unit and an order first." -> all become "Non si può fare lì." (wrong reason). Also the matcher is substring-on-English: any reworded model string silently degrades. Fix: add arms; add a test that every `&'static str` the model can return maps to something other than the fallback.
9. **CIMA logo is overdrawn by the Evacuazione button** — *minor*. Bottom-left in play: logo at `(24, bottom-92)` sits under the action bar's left edge (`3_play.png`). Move logo to top-left or shrink the bar.
10. **Clock is elapsed time with no label** — *minor*. "24:00" in the mission strip is elapsed mm:ss (`play`); the unused `TIME_LEFT` string suggests "time left" was intended. A student reads a rising clock as a countdown. Fix: show "Tempo rimasto 51:00" (mission duration minus elapsed) or label it "Ora dell'incidente".
11. **Unused strings** — *minor*: `SKIP`, `BACK`, `TIME_LEFT` are defined, never shown (the briefing says "skippable" but only Comincia exists; auto-start at 20 s).
12. **Operator "Ricomincia" sets `pinned = true` silently** (`ui.rs operator`) — *minor*: after one reset the kiosk stops rotating towns until someone un-ticks the box; the box is only visible in the hidden panel.

### Balance / design problems

13. **Evacuate at T+0 dominates on all three towns; nothing costs anything** — *blocker for acceptance 4* (and for the spec's whole lesson). Mean safe 219/236/289 vs none 147/61/140; it wins even in sessions where the wind shifts away and nobody is ever threatened. "Not too early" is not expressible until §6.2/§6.5 exist. Blocks nothing in §13's never-cut list but defeats the demo's stated lesson and acceptance 4.
14. **The forecast is not a decision aid yet; following it is worse than always evacuating** — *major* (blocks "never cut: the forecast is the point"). "Evacuate at T+0 only if forecast shift chance < 50%": safe 167 / 61 / 189 vs 219 / 236 / 289 for always-evacuating; in valle shift_p is always >= 60%, so the follower never evacuates and equals "none" exactly. Re-evacuating at T+15 when the forecast says shift (fcast2) recovers to ~216/236/281, i.e. about "evacuate always". A shifting wind is only *useful to know* once a wrong guess costs something (§6.5/6.2). Until then the card is decoration. Also: nothing on the map shows where the fire goes after the forecast shift (no cone, no arrow of the predicted wind).
15. **55% of borgo/porto sessions never threaten anyone** — *major*. With no orders, caught = 0 in 9/16 borgo and 9/16 porto seeds (shift drawn; shift_p range 0.15-0.85, mean 0.5, shift at T+15-40 min turns the fire off the town). In those sessions the right play is nothing, and the beat ("people need warning") does not show. Porto's "one road, it burns" is invisible in them. Fix: raise the floor of shift_p for borgo/porto to ~0.1-0.5 (or make the shift less decisive), and/or pin one seed per town for the first visitor of the day.
16. **Valle: no-shift sessions leave hamlet B untouched and hamlet A never caught** — *minor*. caught = 0 for all 5 no-shift seeds without any order (fire only burns the meadow, 12-26 ha); the "two hamlets" beat only exists in the 11/16 shift sessions. Same remedy as 15.
17. **Units are decorative on all towns; sending them early does not help and slightly hurts** — *major* (spec §6.4 already records the finding). Crew + engine + Canadair at T+3 at the ignition: homes hit 13.9 vs 12.8 (borgo), 15.6 vs 14.3 (valle), 21.8 vs 21.4 (porto); hectares -3 on borgo, 0 elsewhere; caught equal or worse. Three of five action buttons therefore do nothing measurable, and the outcome never mentions them. Needs the demo-only effectiveness knob (spec §6.4) before step 5 art.
18. **"Case colpite" is invariant to evacuation and to units** — *minor*. By design (spec §3) but the card shows the same red number on both columns in every compare screenshot (9/9, 33/33, 36/36), which a student reads as "my choices didn't matter for houses". Add one line on the card ("l'evacuazione salva le persone, non le case") or show it only with units used.
19. **Late order (T+40) is indistinguishable from none on borgo/porto** — *idea/info*: caught 13.4 vs 13.8 and 12.1 vs 12.0, i.e. the lesson "too late" works; but valle T+40 still gets 211 safe, so there "late" is almost free.

### UX / legibility (from screenshots)

20. **Town is tiny and dark in every screen; houses read as faint orange dashes** — *major* (legibility at 2 m; graphics pass §8 not done). Attract/play at 2700 m distance: houses ~3 px, no people, no cars, no unit markers visible. The compass is the only readable world cue. Spot fires, queues, closed roads, unit positions are not drawn at all (§7.6 not done). Units dispatched in the real game were not visible in my shots because the harness only orders an evacuation: **could not verify unit visibility**.
21. **Attract title overlaps the town and the tagline touches the button** — *minor*. Title and subtitle sit on the village in borgo/valle attract; "Rocca Ventosa · Un borgo sulla collina" is within 5 px of the button top. Raise the button or the caption by ~30 px.
22. **Briefing card hides the town; forecast card above it competes with it** — *minor* (known, notes §4). In `2_briefing.png` the card covers the lower 55% and the forecast card the village. For valle the briefing never shows the two hamlets clearly. The 8 s fly-in is invisible behind the cards. Fix: make the briefing a single card at the bottom third, keep the town centred.
23. **Wind compass is readable; the forecast shift is not visualised** — *minor*. "poi da E, tra 15 e 40 min" is text only. A faint second arrow on the compass for the post-shift wind would make the forecast legible at 2 m. Never-cut item (wind arrow) itself is fine: arrow, "Vento da S · 32 km/h", `to`-direction convention checked on borgo (from S, arrow points N), valle (from E, arrow W) and porto (from N, arrow S).
24. **Action bar: "Evacuazione" turns dark/disabled after use and is indistinguishable from "Off"** — *minor* (dim body + tick). A student who hits it by accident has no undo and no explanation of what it did ("ordine dato"); add "avvisate N famiglie" and the time given.
25. **Refusal is only a banner for 6 s** — *minor*; see 1. A unit-chip with persistent status was already listed (notes §1.3).

### Missing / confusing copy (Italian)

26. "Previsioni aggiornate" + the banner "guardale!" is informal and the banner says "guardale" for a card that is already on screen; suggest "Nuove previsioni del vento" only. — *minor*.
27. Compass abbreviations "SO", "NO", "O" (and "NE") are used in "Vento da O" / "poi da SO"; students may not know "O = ovest". Spell out ("ovest", "nord-est") on the card, or add the arrow. — *minor*.
28. "Probabilità che il vento cambi" with "75%" and "poi da E, tra 15 e 40 min" is shown even at 30%; reads as a certain change to a casual reader. Phrase as "Se cambia: da E, tra 15 e 40 min". — *minor*.
29. Compare title "Con i tuoi ordini e senza" is ungrammatical-sounding; suggest "Con i tuoi ordini / Senza ordini". "sorprese dal fuoco in casa" is a little melodramatic; prefer "raggiunte dal fuoco in casa". — *minor*.
30. Takeaway text: "Avvisare presto è ciò che conta di più" is hard-coded for every town/case and is shown even when the evacuation was needless (see 5). In borgo (shift) / porto it is the opposite of the §6.2 lesson. — *major* until cry-wolf exists.
31. Whole file still needs the native-speaker review (§7.7). Brand line "PROPAGATOR · modello di incendio" is the only English-like token; fine. No English strings seen on screen in any state I reached. — *info*.

### Performance / robustness

32. **Town change reloads the scenario via `AppState::SelectingScenario`** (known). `wants_reload` is `kiosk.reload || kiosk.cf_rx.is_none()`, i.e. also true if the twin was never started; if the twin thread errors, `cf_rx` stays `Some` and Compare shows the spinner forever (error only on stderr). Fix: send an `Err` variant over the channel and show "non disponibile" instead of an endless spinner; add a 10 s timeout. — *minor*.
33. **`Cmd::Begin` redraws with a new seed and spawns a new twin thread on every click** with no cancellation; fast repeated attract clicks / the operator "Ricomincia" pile up ~1 s runs. Harmless today (twin runs in ~1 s). — *minor*.
34. **Idle in OUTCOME/COMPARE goes to the *next town*, not the attract of the same one** (`Cmd::NextTown`), and pinned is ignored by nothing else; fine, but the next visitor's town order depends on the previous idle. — *info*.
35. **Headless is fast (a 90 min run ~0.1 s; 432 runs in 32 s on 4 threads)**, so tuning sweeps and a CI "good play beats idle" test are cheap; none exists yet (§11). I did not measure live fps (see "could not check").
36. **Repo still contains the non-demo world** — *major for acceptance 8*. `data/scenarios` still holds spotorno, mati, pedrogao, rhodes and eight labs; `crates/game/src` still has menu.rs, api.rs, interview.rs, map2d.rs, scenario_selector.rs, selftest.rs, etc. (§4).

### Ideas worth doing (small, concrete)

37. A "perché?" line under the outcome: name the single event that decided it ("il vento è girato a T+31", "ordine dato a T+0: 218 famiglie partite in 15 min"). The data exists (`ordered_at_s`, shift time, `first_caught_s`).
38. Show a ghost of the *no-orders* fire perimeter on the COMPARE screen (the twin already has the burnt-cell mask) — one picture says "same fire".
39. Draw the forecast shift as a ghost arrow on the compass and tick the compass when the wind actually changes (needs `sim.weather` vs spec change; flash the banner "Il vento è cambiato!" — currently the shift is silent except for a moving arrow).
40. First visitor of the day / `pinned` towns: add a "seed of the day" so the staff can show a session that exhibits the town's beat (see 15, 16).
41. Chip under each town counter naming who is in danger ("Rocca Ventosa: 12 famiglie in pericolo") to make "In pericolo" actionable.
42. Make the idle warning appear only after ~150 s and show a progress ring; add `KIOSK_IDLE_S` for testing the reset (currently untestable in under 2 minutes).

### Acceptance (§14), one by one

1. Cold start to PLAY <= 30 s: *could not verify by timing*; path is click anywhere -> "Comincia" (or auto after 20 s). Likely passes.
2. Session 3-5 min, ends on outcome without operator: *pass by code* (180 s play, Outcome waits for a click, 60 s idle returns to attract) — but see bug 3.
3. 60 s idle returns to ATTRACT with clean state: *pass by code* for OUTCOME/COMPARE/BRIEFING (goes to the next town's attract); no `KIOSK_SELFTEST`, so reset-fan-out leaks are *not verified*.
4. Good play beats idle by the asserted margin **and** needless evacuation costs trust and money: first half **pass** (see table), second half **fail** (not built, §6.2/6.5; finding 13).
5. Forecast shown, right on average, wrong sometimes: **pass** (calibration test green; shown in briefing and play) but it is not yet useful (finding 14).
6. No shortcut does anything; operator corner does: *no shortcut* pass by construction (`focus.keyboard = true`, nothing registered); operator corner (hold top-right 3 s) *could not verify* — needs a mouse.
7. No English visible; no audio: **pass** on all screens I could reach (see 31); unit-order refusals not seen on screen.
8. `cargo test --release` passes: **pass**. Repo has no non-demo scenarios/UI/scripts: **fail** (finding 36).

### Could not check

- Live frame rate (screenshots only, no profiling); 30 fps floor unverified.
- Mouse interaction: unit tasking on the map, bad-point refusals on screen, order-button double-click, cancel button, pause/resume, operator corner, camera clamp, scroll/orbit — no pointer available; read from code only (`Evacuazione` can be pressed only once; `action_button` returns false in the Done state, so no double order).
- The idle warn/reset timings in practice (60/90+30 s) and the briefing's 20 s auto-advance.
- Unit sprites/markers, spot-fire rings, smoke, evacuation traffic on the map (the harness orders only an evacuation and shoots at ~T+25 s of a compressed run).
- Whether the Bevy-side restart leaks latched view state (finding 21) after Riprova/NextTown; no `KIOSK_SELFTEST` exists.
- COMPARE twin equality with the live `Sim` run for the same orders (headless `Run` determinism is verified; live-vs-headless was not).
