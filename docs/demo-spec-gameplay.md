# Demo — gameplay spec (headless, A/B-tested)

Owner: the **gameplay agent**. Read `docs/demo-spec.md` first (goal, decisions, the
contract with the presentation agent) and `CLAUDE.md` (the model's findings are
constraints). Companion: `docs/demo-notes.md` §1 (gameplay ideas).
The presentation side is `docs/demo-spec-presentation.md`.

Legend: ✅ done · 🔲 to do · ✂ cut-list item.

## 0. How this agent works

**Backend only. No window, no Bevy, no screenshots.** Everything here is built and
judged through `crates/demo` (`demo::Run`, `demo::Tally`, `demo::weather`,
`demo::mission`) and the model crates under it. If a change cannot be measured
headlessly it is not finished.

**Owns:** `crates/{demo,abm,fire,scenario,behavior,chat}`, `data/`, `scripts/`,
`propagator-core` integration. **Does not touch:** `crates/game`, `assets/`,
`strings_it.rs`, shaders. Anything the player must *see or read* is delivered as a
field or enum on a `demo::` type plus a line in the contract table (§1) — the
presentation agent renders it.

**A/B method (the point of this agent).** Every design question below is settled by
a measured comparison, not by argument:
- A *policy* is a scripted commander: a list of `(time_s, Order)` plus a name
  (`none`, `evacuate T+0/10/20/40`, `follow-forecast`, `units-only T+3`,
  `evacuate+units`, `zone`, `re-task aircraft`, …). Put them in
  `crates/demo/src/policy.rs` (new) so tests and sweeps share one definition.
- A *variant* is a model option behind a switch inert by default (finding 34): e.g.
  `unit_effect = {off, ×2, ×4}`, `trust_decay = {0, 0.1, 0.2}`. Variants live in a
  `demo::Variant` struct passed to `Run::new`, never in globals.
- A sweep = policies × variants × towns × seeds (≥16 seeds; a 90 min run ≈ 0.1 s,
  432 runs ≈ 32 s on 4 threads). Report mean and spread of: families safe/secure,
  caught at home, homes hit, hectares, money (§6.5), and **regret** (see below).
- Results go in `crates/demo/tests/` as `#[ignore]` reports **and** as asserted
  pins for the winner. Record the table in this file under the section it settles.
- **The target lesson to optimise for:** no single policy dominates across seeds.
  "Evacuate at T+0 always", "never", and "follow the forecast blindly" must each
  lose to a better policy on average, and each must win on *some* seeds. Define
  *regret* = (best policy's outcome on this seed) − (this policy's outcome); a good
  design has all fixed policies with non-trivial mean regret.

## 1. Contract with the presentation agent

Gameplay delivers, presentation renders. Add a row when a new field is needed; the
presentation agent reads this table rather than the model.

| Delivered by `demo::` | Meaning | Status |
|---|---|---|
| `Outcome { households, safe, moving, in_danger, caught, homes_lost, hectares }` + `secure()` | end-card numbers; `secure()` = households − in_danger | ✅ |
| `Forecast { issue, wind_from_deg, wind_kmh, cone_deg, shift_p, shift_to_deg, shift_eta_min }` | forecast card | ✅ |
| `Order::{EvacuateAll, Attack, Drop, …}` + `Run::order` | everything the player can do | ✅ partial |
| `cost::Log` (feed it: `push(Entry::Evacuation/AirLoad)`, `task(t, unit, kind)`) → `Log::price(now_s) -> cost::Ledger` (`total_eur()`, `by_action(Action::{Evacuation,Engines,Crews,Air})`, `items`); `Run::log`, `Run::ledger()` | money counter, end card. **The live game builds its own `Log` from the orders it issues and the drops it sees; same code prices both** | ✅ 5.1 (tariffs are placeholders, see `cost.rs` sources) |
| `Run::trust() -> trust::Trust { mean, angry_households }`; `trust::CryWolf::{new(agents, delta), note_order(t, ids), step(&mut agents, fire, world), trust(agents)}` for the live game | trust meter | ✅ 5.2, **off unless `Variant::cry_wolf = Some(delta)`** |
| `Event { at_s, kind: EventKind, pos }`, kinds `SpotFire`, `WindShifted{from,to}`, `UnitWithdrew{unit,kind}`, `UnitLost{..}`, `MastDown`, `FireNearTown`; `Run::events` (append-only), `Run::latest_spot()` | advisors, decision pauses, map markers. No text. "Forecast changed" is `ISSUE_2_AT_S` (a `Draw` fact, not a run event) | ✅ 5.4/5.6 (road cut: not yet) |
| `why::why(&Outcome, Facts) -> Why { kind: WhyKind, facts, lead_min }`, `Run::facts()`, `Run::why()`, `Outcome::why(facts)` | "perché?" line (§16 #37). Kinds: `FireNeverCame, CloseButNobodyCaught, OrderInTime, NoOrder, OrderTooLate, SomeSlowToLeave, WindShiftReachedTown` | ✅ |
| `refusal::Refusal` (11 variants, `Refusal::ALL`), `Refusal::from_assign(&str)`, `refusal::check(crews, agents, fire, scn, unit, Intent, pos)` | typed refusals; replaces `target_preview`'s strings. Presentation owns the Italian + the "every variant mapped" test | ✅ 5.7 (game still uses its own strings) |
| `Order::EvacuateZone { centre, radius_m }` | zone evacuation | ✅ order only; no map tool, no HoldRoad (5.5) |
| `Tally::enable_defence()` + `note_defence(crews, agents, fire, world, dropped_at)`; `Variant::defend_homes` | option B: engines on station / drops over homes defend houses (see §4.1). **Inert until the kiosk calls `enable_defence`** | ✅ implemented, **awaiting Mirko's decision** |
| `Variant { unit_effect, defend_homes, shift_p, cry_wolf }`, `Run::with_variant` | sweep switches, all inert at `Default` (pinned) | ✅ |
| `policy::Policy` (scripted commanders), `sweep::{run_grid, summarise, paired, regret_by}` | A/B harness (headless only) | ✅ |

## 2. Done

- ✅ Three towns (`scripts/generate_demo_scenarios.py` → `data/scenarios/demo_*`).
- ✅ `crates/demo`: `mission::spec`, `run::Run`, `run::Tally`; `Order::Attack/Drop`.
- ✅ `abm::SEE_RANGE_M` 800 m; `homes_lost` = burnt ground within 150 m.
- ✅ §6.1 seeded weather draw + calibrated two-issue forecast (`demo::weather`).
- ✅ §6.4 fire part: 60 m opening fires, beats re-measured (`tests/small.rs`).
- ✅ `Outcome::secure()`.
- ✅ Beat tests (`tests/towns.rs`, `small.rs`), determinism (same seed ⇒ same outcome).

## 3. Clean-up (model side) 🔲 — **blocked, see below**

**Status:** attempted and stopped. Deleting `data/scenarios/{mati,pedrogao,rhodes}` was
denied by the sandbox (irreversible local destruction), so none of the deletion was done.
What was learned: ~40 abm/fire tests (the pins for findings 5, 17-19, 34, 39-42 among
them) load `Scenario::load(data_dir)` = the registry default `spotorno`, and are
calibrated to Spotorno's geometry (hard-coded ignition cell, 512x512 grid, 750
households); on the demo towns (200x200 grid, 250-350 households) 17 of them fail on
calibration, not on a bug. Cheap, safe route proposed: keep `spotorno` on disk as a
**test fixture only** (point those tests at `Scenario::load_by_id(.., "spotorno")`,
unregister it from `data/scenarios.json` so no selector shows it), delete `mati`,
`pedrogao`, `rhodes` (91 MB) and their 3 test uses (incident_gaps mast/shore tests can
run on the demo towns; `traffic.rs` pedrogao -> demo_porto; `real_scenario_ignitions.rs`
goes), then port the finding pins to demo towns one at a time. Needs a human to approve
the deletions.

Delete the non-demo data and the tests that load it, **keeping the pins for findings
5, 17–19, 34, 39–42** (port them to `demo_*`):
- `data/scenarios/{spotorno,mati,pedrogao,rhodes}` + labs, `data/spotorno_*`,
  `data/osm_raw.json`, `scripts/` real-data pipeline (keep
  `generate_demo_scenarios.py`, `bake_fuels.py` output, `build_models.py`).
- `crates/telemetry` unless §6.7's event stream wants it; wasm profile.
- Do this first, in small commits, tests green after each. The `crates/game` side of
  the clean-up belongs to the presentation agent.

## 4. The open design question: do units matter? 🔲 (blocks the demo)

Finding (§16 #17, 16 seeds × 3 towns): crew + engine + Canadair at T+3 change homes
hit by ≈ 0 (13.9 vs 12.8 borgo; 15.6 vs 14.3 valle; 21.8 vs 21.4 porto). Three of
five buttons do nothing measurable. Causes: engine pumps 6 min then spends ~10 on a
hydrant trip; crews cut ~0 m at 120 m/h and withdraw under heat; a front is dozens
of 20 m cells (findings 15, 16, 41).

**A/B these options against the same sweep (policy `units-only T+3`,
`evacuate+units`), pick by the criterion below, and record the table here:**

| Option | What changes | Cost |
|---|---|---|
| A. effectiveness multiplier | demo-only `Variant::unit_effect` scaling engine water / crew line rate (×1, ×2, ×4, ×8), inert at ×1, pinned by a test | changes a published `abm` number — **Mirko's call; ask before merging, run the sweep first** |
| B. different job | units *protect* rather than stop: engine **sprinklers a house cluster** (that cluster's `homes_lost` falls), crew **holds a road** open (the exit stays passable), aircraft **wets a corridor** ahead of the front | fits the model as it is; needs a protect task + exposure discount; the closest to `demo-notes.md` §1.1 |
| C. smaller fire / earlier window | opening fire ~1 ha is already the floor (finding 3); try a slower-growing opening so T+3 action is on a *front of a few cells* | little; may fall short |
| D. cut the buttons | units are scenery | loses half the game |

**Acceptance for whichever wins:** units at T+3 reduce homes hit **and** hectares by a
margin an asserted test pins, on every town's mean; units at T+30 reduce them by
clearly less (so timing matters); no unit order is worth more than its §6.5 cost on
the seeds where the fire never threatens anyone. Report the sweep, not a single seed.

### 4.1 Sweep results (2026-10-06, 16 drawn seeds x 3 towns, paired on the same draw)

Harness: `demo::policy` (scripted commanders, units sent to `Run::head()` as it is
*when ordered*), `demo::sweep` (parallel grid, mean/sd, paired difference +- s.e.,
regret), `abm::suppression::UnitEffect` and `demo::Variant` (all inert at default,
pinned by `tests/units.rs`). Reports: `tests/units_sweep.rs`, `tests/units_diag.rs`
(`cargo test -p demo --release --test units_sweep -- --ignored <name> --nocapture`).

**Why units did nothing (units_diag):** crews cut 0 m — they withdraw because the head
is over `WORK_LIMIT` (the ember shadow, finding 41); aircraft "broke off: not
survivable" on borgo/porto and drop 37 loads on valle to no effect; engines
saturate their 4 cells at x1 already (6 L/m2 >> extinction), so a bigger dose is moot.
The limit is *area and survivability*, not intensity.

**Option A (multiplier) — fails.** Paired change vs no units, homes hit (mean +- s.e.),
`units T+3 @+300 m`: x1 / x8 / x32 = borgo +0.2/-2.5/-1.1 (+-2-3), valle +1.2/+1.2/+1.0
(+-1), porto -1.2/+0.8/-0.3 (+-3). Even "god mode" (line x8, water x8, hose reach x4,
steadier nerves x0.25 or 0): borgo -0.4/-1.7, valle +3.8/+2.4, porto +3.7/+4.9, all
within noise; hectares -2 to -6 of ~20-55 (best single cell: borgo -6.1 +- 1.6).
Pinned: `scaling_what_a_unit_does_does_not_save_homes`. **No change to a published
abm number was made or is proposed.**

**Option C (slower opening fire) — fails.** Wind x0.6 and +3 % moisture: homes hit
fall to 0 on borgo/porto (3.6 on valle) *before* units matter; units save 1-2 ha of
12-18 (10 %) on a fire that threatens nobody. A slower fire removes the lesson.

**Option B (protect homes) — works where the fire reaches the town in time.** Demo-side
rule (`Tally::enable_defence`, `Variant::defend_homes`): an engine working with water
within 80 m defends the homes there for a tank's worth of sprinkling (25 min at
~100 L/min, restarted by a refill); a drop over homes defends them 15 min; a defended
home is lost only to flame contact (25 m), an undefended one when burnt ground is
within 150 m. Loss is latched at first contact, so *when* the engine is on station is
the mechanic. Paired change in homes hit vs no units (both with defence on):

| town | protect T+3 | protect T+10 | protect T+30 | protect when head <600 m | <300 m |
|---|---|---|---|---|---|
| valle | -4.9 +- 0.8 | -4.6 +- 0.7 | -2.9 +- 1.2 | -4.5 +- 0.7 | -1.2 +- 0.5 |
| borgo | +1.3 +- 2.1 | +1.1 +- 1.8 | +1.1 +- 1.9 | +1.5 +- 1.7 | -1.7 +- 0.9 |
| porto | +3.5 +- 2.9 | 0.0 +- 1.4 | 0.0 +- 0.0 | -1.7 +- 1.9 | -0.1 +- 0.1 |

Hectares are unchanged by B (it protects homes, it does not stop the fire). Pinned:
`protecting_homes_saves_homes_and_earlier_saves_more` on valle's scripted beat (wind
shift at T+30, 12 seeds): protect T+3 **-7.1 +- 0.9** homes, protect T+30 **+0.3** — so
timing matters strongly where the fire reaches the town on schedule — and
`home_defence_changes_nothing_until_a_unit_is_posted`. (The table above was taken on
the drawn climate before valle's shift odds were lowered in 5.3; on the new drawn
climate valle protect T+3 is -2.3 +- 0.8, T+30 -2.2: fewer sessions have the shift.)

**Spot split (5.4), 16 seeds, paired vs none, defence off:** all units at the head
`@+300m` T+3: homes +0.2/+0.9/-1.2 (borgo/valle/porto, +-2-3); one engine + one crew at
the head and one of each at every spot: -0.4/+0.9/+2.2. Nothing separates them: the
spot-fire split is not a lever while units cannot stop fire (same finding as A).

**Honest status: no option meets the §4 acceptance on every town.** B is the only
one with a real, timed effect, on valle. Borgo/porto engines arrive ~T+13, spend their
25-minute tank before the fire reaches the town (head 1000 m from the nearest home
at T+25) and the effect vanishes; in those towns the fire often threatens no one
(finding §16 #15/#16, see 5.3). **Decision needed (Mirko):** adopt B as the demo
rule (kiosk calls `Tally::enable_defence`; `protect` becomes the unit action), and
raise `shift_p`/threat floors (5.3) so borgo/porto reach the town; or D (cut the
unit buttons). Option A should not be pursued.

## 5. Next gameplay steps (build in this order; they interact)

**Status 2026-10-06 (gameplay agent):** 5.1 ✅ (gate pinned, tables below), 5.2 ✅
(mechanism + pin; off by default), 5.3 ✅ partly (valle odds changed; borgo/porto
threat share not raised, see below), 5.4 ✅ events + pins, 5.7 ✅, `Outcome::why` ✅,
5.5 zone order only, 5.6 partly (events exist; road-cut event and decision prompts
not). §3 clean-up **not done** (see §3).

### 5.1 Cost (money) ✅ — §6.5

Tariffs in `demo::cost` (placeholders from memory, **to verify**): air load EUR 3,000;
engine EUR 250/h; crew EUR 60/h; **evacuation EUR 250 per household** (EUR 62.5 k borgo,
75 k valle, 87.5 k porto). The evacuation scale was chosen by sweep (`tests/cost_sweep.rs`):
at EUR 40/household the order cost ~EUR 10 k and T+0 won everywhere. Mean loss in EUR k
(spent + 20 k x families caught at home + 20 k x homes reached; analysis weights only,
never shown), 24 drawn seeds:

| town | never | evac T+0 | T+10 | T+20 | T+40 | follow-forecast |
|---|---|---|---|---|---|---|
| borgo | 354 | 297 | 310 | 342 | 411 | **285** |
| valle | 412 | 361 | 361 | 362 | 432 | **333** |
| porto | 462 | 454 | 462 | 498 | 547 | **426** |

Pinned (`tests/cost.rs`): follower < both always and never on every town; each of "never"
and "T+0" is the best policy on some seeds with non-trivial regret; cost is a pure
function of the log. Caveats: the follower's edge is 4-8 %, and it exists only because
the evacuation bill is large relative to a family; at EUR 5 k/family T+0 ties or wins on
borgo and loses on nothing. The ratio (evacuation bill : harm) is the design lever, and it
is a number to agree with the presentation side's end card, not a fact.

(Original text:) Every intervention costs money
Every intervention costs money; a running total is shown, never as a score. Unit costs
set from published tariffs, source written in code (`demo::cost`): Canadair per load,
engine/crew per hour, a general evacuation (lost working day, buses), a *needless*
evacuation costing the same as a needed one. `Suppression` already tracks `drops`,
`water_used_l`, `line_cut_m`. **Cost is a pure function of the action log** (test), so
the headless twin and the live game price identically. End card: money beside what it
saved; COMPARE: "senza ordini: 0 €".
*A/B:* with cost on, "evacuate at T+0 always" and "never" must both lose to the
forecast-follower on average (finding §16 #13/#14). This is the gate for the forecast
being a real decision (§6.1 "Not done").

### 5.2 Trust / anger (cry-wolf) ✅ — §6.2

Built as `demo::trust::CryWolf`, off unless `Variant::cry_wolf = Some(delta)`. An order
is judged 30 min later by what the fire did (hindsight, from the fire state, not a
twin): if more than half the households it moved never had fire within 800 m, it was
needless, and every household not yet ordered loses `delta x needless_share` of
`trust_authority`; the shipped compliance gate (trust > 0.35) does the rest. Pinned
(`tests/trust.rs`): nothing moves with no order; on valle, a needless zone order at T+0
then a general order at T+31: mean trust -0.24, ~229 households lose >= 0.10, families
caught at home 5.9 -> 7.6 (+1.7, 8 seeds, delta 0.35). Modest because the second order
is late anyway; the mechanism needs a *sequence* of orders to matter, which one general
order per session cannot give: it pays off with zone orders (5.5) or a repeat order. Open: twin-hindsight vs
forward estimate of "needless" was not compared (spec 8).

(Original text:)
A **needless** order lowers `trust_authority` of those households for the *next*
order and raises their confirmation delay (finding 42's `block.order_confirmation`
is the hook). Needless = decided against the twin (hindsight, exact) or a forward
threat estimate (fairer): **measure how often they disagree, then choose** (§7 open
question). Provably inert until an order is given (finding 34): pin both halves —
branch fires; shipped runs unchanged. Tune by sweeping (early-needless, later-needed)
pairs. Deliver `trust` and `angry_households` (§1 contract).

### 5.3 Forecast usefulness + shift probabilities ✅ partly — §6.1 remainder, §16 #14–16

Done: valle `shift_p` 0.6-0.95 -> **0.10-0.70** so a no-shift valle session exists and the
forecast follower beats evacuate-always there too (table in 5.1); `Climate::shift_threatens`
says which way the shift cuts; `weather::draw_with` + `Variant::shift_p` sweep odds
(`cost_sweep::climate_sweep`). Measured finding: **raising threat share and keeping the
forecast a decision pull in opposite directions**. At borgo/porto shipped odds (0.15-0.85)
about 25-50 % of sessions threaten (caught >= 3 under no orders) and the follower wins; at
(0.05-0.45) 62 % threaten but the follower collapses to "evacuate at T+0" (tie, 617 vs 617).
Shipped odds kept for borgo/porto. Open: seed-of-day (not built).
Also found: valle's no-shift fire never throws an ember at 25-35 km/h / 4-6 % (0/10
sessions); spotting there comes only with the shift's west wind (10/10).

(Original text:)
- Test: following the forecast beats ignoring it over many seeds but not on every
  seed — needs §5.1 to bite.
- Raise the floor of `shift_p` for borgo/porto (≈0.1–0.5) and/or make the shift less
  decisive so ≥ ~75 % of sessions threaten the town (today 55 % of borgo/porto and
  all valle no-shift sessions threaten nobody). Alternative: a **seed of the day**
  (pinned beat seed per town) — decide by sweep; see open question.
- COMPARE twin keeps the same drawn weather and seed, orders removed (✅, keep tested).

### 5.4 Spot fires as a first-class event 🔲 — §6.3
Shrub spotting is on (finding 41). Assert ≥1 spot fire per town in a typical run
(`tests/towns.rs`; porto is built around it); emit a spot-fire `Event` with position
and age; a unit can be sent to it. Spot fires are the reason to split units — A/B
"all units at the head" vs "one at the spot".

### 5.5 Zone evacuation and a road-hold task 🔲 — `demo-notes.md` §1.3
`Order::EvacuateZone { centre, radius }` (warns fewer, spares needless flight) and
`Order::HoldRoad` for crews (keep an exit passable). A/B vs all-or-nothing on porto,
whose lesson is "the road out burns".

### 5.6 Decision events from real model changes 🔲 — §7.4 data side
The model, not a script, says when something needs a decision: road cut, spot fire,
mast burnt, forecast changed, wind shifted, unit stranded. Emit them as `Event`s
(§1) with enough context for a one-sentence prompt and 2–3 choices incl. "aspetta".

### 5.7 Typed refusals 🔲 — §16 #8
`command::target_preview` returns English `&'static str`s which the UI substring-
matches. Replace with a `Refusal` enum (in `abm`/`demo`); test that every variant has
an Italian mapping (the test lives in presentation, the enum here).

### 5.8 Beyond buttons (not in scope now, shapes the API) 🔲 — §6.6
Drawn zones/fire lines, unit chips, radio calls, tapping a family (LLM), prediction
marker, rewind-and-choose (free because `Run` is deterministic). Keep `Order` and
`Event` expressive enough that none of these needs a model change later.

## 6. Model rules (binding)

`Sim::advance` is the one stepping path (finding 5); demo speed is a fixed simulated-
s per real-s; same scenario + seed + weather draw + orders ⇒ same outcome (COMPARE
depends on it; tested); every new mechanism (anger, cost, forecast, unit effect) is
**provably inert until used** (finding 34); tuned constants are per-scenario
(finding 38); every new behaviour gets a test that asserts it *fires*, not that the
model runs (findings 26, 35, 42); `cargo test --release` stays green. A change to a
number published in `CLAUDE.md` is flagged in the commit message and to Mirko.

## 7. Testing (gameplay)

- ✅ Beat tests. 🔲 Add: forecast usefulness (5.3); anger fires and is inert at T+0
  (5.2); ≥1 spot fire per town (5.4); unit effectiveness margin and timing (§4);
  cost is a pure function of the action log (5.1); a scripted good commander beats
  "no orders" on `secure` **and** money spent is plausible; idle commander still
  reaches the end (playability); refuge/haven/mast coverage per town (findings 9,
  34); outcome rows add up to the household count (§16 #6); every fixed policy has
  non-trivial regret (§0).
- Sweeps are cheap: run them in tests, print the table with `--ignored --nocapture`,
  pin the conclusion with an assertion.

## 8. Open questions (gameplay)

- Unit effectiveness: A / B / C / D (§4). **Ask Mirko before changing a published
  `abm` number**; bring the sweep.
- Money: real unit costs and their sources (Protezione Civile / regional tariffs).
- How a *needless* order is judged: twin vs forward estimate (5.2).
- Session seeding: random per visitor (now), seed of the day, or an operator switch.
- Outcome semantics: `secure()` shipped; revisit if playtests find it misleading.
- Whether `demo_porto` stays (cut-list item) once measured.

## 9. Playtest findings owned here (2026-10-06)

Evidence for the above, from 16 seeds × 3 towns × 9 policies (full text in git
history at `da0549b`, `docs/demo-spec.md` §16):

| town | none safe/caught | T+0 | T+40 | forecast-follower | units only | homes hit (any) |
|---|---|---|---|---|---|---|
| borgo (250) | 147 / 13.8 | 219 / 4.8 | 182 / 13.4 | 167 / 9.6 | 144 / 13.3 | 12.8 (units: 13.9) |
| valle (300) | 61 / 8.5 | 236 / 2.2 | 211 / 5.5 | 61 / 8.5 | 57 / 10.3 | 14.3 (units: 15.6) |
| porto (350) | 140 / 12.0 | 289 / 5.2 | 221 / 12.1 | 189 / 9.1 | 138 / 15.3 | 21.4 (units: 21.8) |

(Safe here is the old evacuees-only count; `secure()` is the card's number now.)

- #13 evacuate-at-T+0 dominates; nothing costs anything → 5.1, 5.2.
- #14 following the forecast is worse than always evacuating → 5.1, 5.3.
- #15, #16 55 % of borgo/porto and valle no-shift sessions threaten nobody → 5.3.
- #17 units decorative → §4.
- #18 "case colpite" invariant to evacuation → by design; presentation adds a line.
- #19 late order (T+40) ≈ none on borgo/porto, but valle T+40 is nearly free → check
  after 5.1.
- #35 headless is fast → the whole method in §0.
