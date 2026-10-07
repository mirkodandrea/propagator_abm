# Demo — gameplay spec (headless, A/B-tested)

Read `docs/demo-spec.md` first (the game in one paragraph, lessons, decisions,
milestones) and `CLAUDE.md` (findings are constraints). Presentation:
`docs/demo-spec-presentation.md`.

Legend: ✅ done · 🔶 partly · 🔲 to do · ✂ cut-list item.

## 0. How this side works

**Backend only. No window, no Bevy, no screenshots.** Everything here is built
and judged through `crates/demo` and the model crates under it. If a change
cannot be measured headlessly it is not finished.

**Owns:** `crates/{demo,abm,fire,scenario,behavior,play}`, `data/`, `scripts/`
(including `scripts/playtest.sh`).
Anything the player must *see or read* is delivered as a typed field on a
`demo::` type plus a row in §1.

**A/B method.** A *policy* is a scripted commander (`demo::policy`) expressed as
token → target assignments per turn; a *variant* is a model option inert by
default (finding 34). A sweep = policies × variants × drawn seeds on Rocca
Ventosa (`demo::sweep::run_grid`, all cores). Report mean ± s.e., pin the
conclusion with an assertion, record the table here. **Every token must have a
measured right use and wrong use, or it is cut** — a resource that never
matters is a fake choice.

## 1. Contract with the presentation side

Names are proposals; the shape is binding.

| Delivered by `demo::` | Meaning | Status |
|---|---|---|
| `Session` on `Run` / `Referee` (one set of books) + `turn() -> Turn`, `tokens() -> Vec<Token>`, `targets() -> Vec<Target>`, `preview(token, target) -> Option<Preview>`, `assign(token, target)`, `unassign(token)`, `end_turn()`, `report() -> &TurnReport`, `verdict() -> Verdict` | the turn engine; kiosk and twin call the same methods in the same order | ✅ |
| `Turn { index, at_s, last: bool }`, `TURN_S` (8 min), `TURNS` (5), `MISSION_S` (60 min) | the clock | ✅ |
| `Token { id, kind: TokenKind, state: TokenState, water: Option<f32>, at: Pos, order: Option<TargetId> }` | the tray and the units on the map | ✅ |
| `TokenKind::{Pattuglia, Autobotte, Squadra, Canadair}` | §3 | ✅ |
| `TokenState::{Libero, InViaggio{eta_s}, AlLavoro, Rifornimento{eta_s}, Ritirato, Perso, InArrivo{eta_s}, NonChiamato}` | badge on the token and the unit | ✅ |
| `Target { id, kind: TargetKind, pos, label_pos }`, `TargetKind::{District(d), Head, Flank(Side), Sky}` | §4; `Sky` is the Canadair call; spot fires are map features, not targets | ✅ |
| `Preview { eta_s, effect: Effect }`, `Effect::{Avvisa{families}, Difende{homes}, Fascia{homes}, Bagna, NonSalvaCase, Lontano, Ritirata, Chiamata{eta_s}, Inutile}` | what the target says before you commit (§4) | ✅ |
| `TurnReport { lines: Vec<ReportLine> }` (≤ 3, ranked), `ReportLine { kind, district: Option<usize>, pos: Option<Pos>, n: Option<u32> }` | the between-turn report (§6) | ✅ |
| `Verdict { districts: Vec<DistrictVerdict>, families_caught, homes_hit, none: Counterfactual, notes: Vec<Note> }` | the end card (§6); families are shown **caught**, never "safe out of 250" | ✅ |
| `DistrictVerdict { district, people: Stamp, homes_hit, homes_hit_none }`, `Stamp::{InTempo, Tardi, MaiAvvisati, GiustoNonAvvisare, Prudente, AllarmeInutile}` | one row per district | ✅ |
| `Note::{TestaInutile, CanadairInTempo, CanadairTardi, CanadairMaiChiamato, PattugliaSprecata, …}` | doctrine kept or broken, ≤ 3 on the card | ✅ |
| `Forecast` + `Draw::forecast(issue)`, issue 2 at turn 2 | the forecast | ✅ |
| `District { name, households, centre, radius_m }`, `Report` | district facts | ✅ |
| `Event` (existing kinds) | raw material for report lines | ✅ |

The headless contract is implemented and used by `play`. Migrating the
kiosk to this turn API is still presentation milestone 3; a compiling kiosk
is not evidence that the new turn UI has shipped.

## 2. The scenario: Rocca Ventosa (`demo_borgo`)

The only scenario the kiosk plays. Built by `scripts/generate_demo_scenarios.py`.

| | |
|---|---|
| Ignition | pines S of the village, patch r = 100 m (finding 3) |
| Wind | from S, 35 km/h; forecast: 20–70 % it turns from E, 45 km/h, between T+12 and T+26 |
| Il Borgo (district 0) | 148 households, downwind of the opening wind |
| Le Coste (district 1) | 64 households, downwind if the wind turns |
| Il Mulino (district 2) | 38 households, upwind in every forecast |
| Area di attesa | cleared ground that `abm::refuge::choose` finds (finding 45) |
| Preparation to leave | 6–18 min per household |
| Fire station / staging | where tokens start; drive times from it are the ETAs |

Measured on this town with instant orders (the sweeps in §7 re-measure them as
turn policies): families caught, no orders 13.2, Il Borgo warned at T+0 4.2, at T+20
12.9. Warning decay on Il Borgo when the wind holds: 4.7 / 9.1 / 12.3 / 13.6 at
T+0/5/10/15. Homes hit when the wind holds: none 68.4, three engines on Il Borgo
32.9, on Il Mulino 68.4. Reacting to the shift when it happens saves nothing.

## 3. Resources (tokens)

| Token | Qty | Order | Model | Catch |
|---|---|---|---|---|
| 🚓 **Pattuglia** (Polizia Locale) | 1 | warn district *d* | the car drives from staging by road; `EvacuateDistrict(d)` is applied **when it arrives**; free again the turn after | one district at a time; the order you warn them in matters |
| 🚒 **Autobotte** | 2, **+1 at turn 3** | defend district *d* · (head / flank) | `Defend` (option B, `defend_homes`), `Attack` | roads only, 60 m hose; refills at a hydrant (shown, costs no homes — §7.5) |
| 👷 **Squadra AIB** (boosted) | 1 | defend homes at district *d*'s fire-facing side · (head / flank) | off-road home-defence fallback (`crews_defend`), with a line starting at the post — **more effective than the published model, approved by Mirko 2026-10-07** | no water or road needed, but slow: ~14′ to Il Borgo; useless off the fire's path (§7.7) |
| ✈️ **Canadair** | 1 | call (turn *k*) · then one drop per turn on head / flank / district edge | `request_air` then `Drop` | arrives `AIR_RESPONSE_S` (25 min ≈ 3 turns) after the call; only a drop on the threatened district's edge saves homes (§7.2) |

The IT-alert was cut (demo-spec §2): it warned every district at once at no
cost and beat every careful plan on families (§8). Warning is the patrol's
job alone, so a stop at a district nobody threatens is a turn of the only car
lost to the districts that are — the cost lesson 3 needs.

Rules:
- **Reassign any time a turn opens.** A unit at work that is reassigned leaves
  its post and drives to the new one; travel time is the price, shown as the ETA.
- A token that is `Rifornimento`, `Ritirato` (one turn) or `Perso` cannot be
  given an order that turn. The Canadair `InArrivo` may be briefed in the turn
  it arrives (a standing order: one drop per turn on its target).
- The reinforcement engine appears in the tray at turn 3 at staging.
- Units are placed by the model's own logic around the target (`post_facing`,
  nearest reachable road — finding 17); the player picks *what*, never *where to
  the metre*.

## 4. Targets

Fixed points the model publishes each turn; the UI lights the ones valid for the
selected token and shows `preview` on hover.

| Target | Where | Valid for | Preview effect |
|---|---|---|---|
| **District d** | the district's fire-facing edge (`District::post_facing`) | Pattuglia, Autobotte, Squadra, Canadair | *Avvisa N famiglie* / *Difende N case* / *Difende N case senza acqua* / *Bagna il bordo* |
| **Testa** | `run::head_of` | Autobotte, Squadra, Canadair | *Non salva case: col vento la testa non si ferma* (lesson 4) |
| **Fianco sx / dx** | perpendicular to the spread direction, 1/3 back from the head | Autobotte, Squadra, Canadair | what the model does there (measured: saves no home) / *Strada troppo lontana* |
| **Cielo** | the tray slot itself | Canadair (when `NonChiamato`) | *Arriva al turno k* |

Spot fires (`SpotFire` events, finding 37) are drawn on the map and named in
the report; they are not targets.

**Previews must be true.** A preview states what the model will do with that
order: `Lontano` ⇔ the unit never starts work there; `Difende N` / `Fascia N`
match what the verdict credits; `Ritirata` only where withdrawal follows.
Pinned by a test over every token × target × turn on seeds 1–10.

A target never valid for a token is not lit. `Effect::Inutile` is shown, not
refused: the player may still send an engine to Il Mulino and learn from it.

## 5. Lessons — each a test that must fire (`tests/lessons.rs`)

| # | Lesson | Assertion (Rocca Ventosa, drawn seeds) |
|---|---|---|
| 1 | The wind decides who | no orders: wind holds → Il Borgo caught ≫ Le Coste; shift → Le Coste caught > 0; Il Mulino 0 |
| 2 | Warn early | patrol to Il Borgo at turn 1 ≪ at turn 3 in families caught |
| 3 | Don't warn everyone | patrol to Il Mulino at turn 1, then the forecast plan one turn late, catches more families than the forecast plan; Il Mulino stamps *AllarmeInutile* |
| 4 | Never the head | engine, crew or drop on `Head`: homes hit = no units (± s.e.) |
| 5 | Defend where it goes | 2 engines on Il Borgo vs on Il Mulino: homes hit halved vs unchanged; boosted crew on Il Borgo's fire side saves ≥ 5 homes, on Il Mulino none |
| 6 | Call aircraft early | Canadair called turn 1 delivers ≥ 2 drops before T+40; called turn 3 ≤ 1; and a drop matters (§7.2) |
| 7 | People first | units alone (no warning) change homes hit, not families caught; warnings alone the reverse |

### Measured after triage (2026-10-07, seeds 1–40)

Reproduce: `cargo test --release -p demo --test lessons -- --ignored
--nocapture`. All seven lessons retain active assertions.

| # | Result | Numbers |
|---|---|---|
| 1 | Wind changes risk | no orders, holds: Borgo 12.1 ± 1.5 caught, Coste 0.0; shifts: Coste 13.8 ± 0.8; Mulino 0.0 |
| 2 | Early warnings help | holds: patrol Borgo t1 7.4 ± 1.4 caught, t3 12.1 ± 1.5 |
| 3 | An early needless stop costs time | forecast patrol 6.8 ± 0.6 caught, Mulino-first 11.3 ± 0.6; Mulino false-alarm stamp 39/40 |
| 4 | Head attacks save no homes | paired Δ vs none: engine +2.2 ± 2.3, crew +0.4 ± 0.5, Canadair −1.2 ± 1.4 |
| 5 | Units help on the fire's path | holds: two engines Borgo 23.5 homes hit vs 61.4 none; crew Borgo 34.6 vs 60.0 in that district; either on Mulino changes nothing |
| 6 | Call aircraft early | t1 mean 2.0 ± 0.1 drops by T+40, homes −6.5 ± 1.2; t3 no drops by T+40 and no mean home saving |
| 7 | Count people and homes apart | engines alone: homes −17.5 ± 2.7, families unchanged; patrol Borgo + Coste: families 7.8 vs 13.2, homes unchanged |

## 6. Turn, report, verdict

**Turn loop.** `end_turn()` applies the assignments, steps the model `TURN_S` in
`STEP_S` chunks (the one stepping path, finding 5), and builds the report.
After turn 5 the presentation calls `end_turn()` until `MISSION_S`; the verdict
is final then. Forecast issue 2 lands at the start of turn 2 (T+8); the
reinforcement at turn 3; the Canadair at the call turn + 3.

**Report** (≤ 3 lines, most important first): district threatened / reached;
families in motion from a warned district ("Il Borgo: 90 famiglie in viaggio");
unit states that need a decision (dry, withdrew, lost, arrived); new spot fire;
wind turned; mast down; Canadair on station. Ranking and wording are typed here,
formatted in `crates/text`.

**Stamps** (people, per district):

| Stamp | Rule |
|---|---|
| `InTempo` | warned ≥ `IN_TIME_MIN` before the first **threat or reach**, whichever is earlier; on the final card, caught families must also be fewer than with no orders (unless none would be caught anyway) |
| `Tardi` | warned later than that, or the final caught-family count did not improve against no orders |
| `MaiAvvisati` | threatened or reached, never warned; the explanation distinguishes threat near homes from fire actually reaching them |
| `GiustoNonAvvisare` | not warned, never threatened |
| `Prudente` | warned while the forecast in force pointed at it; never threatened. **Not a mistake.** |
| `AllarmeInutile` | warned, never threatened, and no forecast pointed at it |

**Counterfactual:** the same draw, no orders, from the twin. Families are
reported as **caught** ("Famiglie bloccate dal fuoco: 5 (senza ordini: 10)"),
never as safe out of the whole town, where a halving reads as 245 vs 240.
**Notes:** at most three, from the order log and unit history (§1 `Note`), the
broken rule first.

### Accepted iteration from the Codex follow-up playtest (2026-10-07)

Mirko requested implementation of the playtest findings. This iteration
improves factual feedback and the amount of repeated output; it does not add
costs to aircraft, alter suppression effectiveness, or shorten simulated time.

- Publish forecast status at the current clock: pending, shift observed, or
  forecast window elapsed without an observed shift. Preserve the actual
  observation; do not reveal a future drawn shift to the player. Historical
  warning justification uses the forecast that existed when the order was
  given, with its time window, rather than a later forecast.
- District proximity and predicted risk are distinct facts. Show whether a
  district is on the current fire path, in the possible shift path, or outside
  the predicted direction. A nearby upwind district must not be presented as
  an advised warning merely because the proximity level is *Watch*.
- Deliver a typed warning explanation for each verdict row: warning arrival,
  first threat/reach, the required ten-minute lead when applicable, and the
  caught-family comparison. Distinguish insufficient lead from no improvement
  rather than formatting both as alternatives. Threatened but never reached
  must not be described as fire arriving at houses. Keep existing stamp rules
  unless a reproducible contradiction requires a documented correction.
- Reproduce seed 261793 with the third-session orders in the follow-up report.
  Canadair feedback distinguishes call/arrival timing, no target, unsafe
  break-off, and completed drops. Zero drops alone never proves a late call.
  Record actual timing and history in the shared model contract. Fix a genuine
  scheduling defect if reproduced; do not make flight safe to obtain a drop.
- Publish the actual ongoing job and its effectiveness: a road-side head
  attack may consume water without saving homes. Show that distinction rather
  than suggesting that *al lavoro* means useful home defence. Explain coverage
  as homes covered at the assigned post, not an exact guarantee of homes saved;
  distinguish existing coverage from additional coverage where available.
- Publish preparation after warning and notable arrivals/refills. Reinforcement
  at turn 3 is a persistent opening notification, so a three-line incident
  report cannot silently hide it.
- Provide a short review of continuing orders and upcoming changes in later
  turns. Where no new order is needed, *Avanti* explicitly maintains the orders.
  Aircraft calling is an early preparation step, not a fabricated costly
  tradeoff. Preserve reassignment opportunities and five playable turns.

All these facts and reasons are computed in `demo::`, formatted in `text`,
and read by front ends. Pin reproduced defects with focused tests, and keep
release checks and preview consistency green. If forecast interpretation
changes policy outcomes, rerun the affected lesson/balance checks and report
any change; never weaken a failing assertion to hide it.

Contract clarification during this iteration: the existing engine classifies
an unwarned, threatened-but-unreached district as `MaiAvvisati`. The earlier
table omitted that case. Preserve this classification and state *threatened*
in its explanation, rather than claiming that fire arrived at homes.

## 7. Milestone 0 — sweeps that decide which tokens ship

Run before any UI work. Each has a pass criterion; a fail cuts or changes the
token (demo-spec §6 for the `unit_effect` question).

1. **Hand crew near a district.** Squadra on Il Borgo's edge at turn 1 vs none:
   homes hit. Pass: ≥ 5 homes fewer. Fail → cut the crew.
2. **Canadair drops.** Called turn 1, drops on Il Borgo's edge / a flank / a
   spot fire vs never called: homes hit, spot fires surviving. Pass: one use
   beats none by ≥ 5 homes or puts out a spot fire most of the time.
3. **Engine on a spot fire.** Engine to the first spot fire within one turn of
   it appearing vs never: does it go out, does it matter downwind. Also check
   Rocca Ventosa produces ≥ 1 spot fire in most sessions (if not, the
   `SpotFire` target and note are cut).
4. **Patrol delay.** Patrol to Il Borgo at turn 1 (arrives ~T+3–5) vs instant
   warning at T+0: cost in families caught. Pass: patrol still ≤ half of no
   orders. Measure Il Borgo → Le Coste sequencing in shift sessions.
5. **Tank.** Engines posted turn 1 vs turn 2–3 on Il Borgo: state when the front
   arrives, homes hit. Pass: a measurable difference either way (it decides
   whether lesson 6 is taught or dropped).
6. **Head attack.** Every ground unit sent to `Head` withdraws within one turn in
   every drawn session (lesson 4 depends on it never "working").

### Measured (milestone 0, 2026-10-07)

The tables in §§7.1–7.6 preserve the **pre-triage** measurements, including
resources and flags subsequently removed. Current lessons and balance are in
§§5 and 8; the isolated crew comparison and shipped fallback are in §7.7.

Rocca Ventosa, drawn seeds 1–40 (the wind turns in 26, holds in 14), every
policy played through `demo::Session` as a player would; mean ± s.e., Δ is the
paired difference per seed. The game's model options: home defence (option B)
and one fire station, the east map-edge exit (`demo::run::station`): patrol
2.9′ to Il Borgo, 4.6′ to Le Coste; engines 3.3′ / 5.3′; crew 13.9′ / 21.8′.
Reproduce: `cargo test --release -p demo --test sweeps -- --ignored --nocapture
--test-threads 1`.

**7.1 Hand crew — FAIL.**

| Policy | Homes hit | Line cut (m) | Δ homes vs none |
|---|---|---|---|
| none | 66.2 ± 2.7 | 0 | — |
| crew on Il Borgo's edge, turn 1 | 66.2 ± 2.7 | 91.3 ± 0.9 | 0.0 ± 0.0 |
| crew on left / right flank, turn 1 | 66.2 ± 2.7 | 0.1 / 0.0 | 0.0 ± 0.0 |
| *variant* `line_x` = 4 / 10, Il Borgo's edge | — | 180 / 180 | 0.0 ± 0.0 |

The district post is a home: the line it cuts lies across the village's
non-burnable ground. On a flank the crew walks ~20 minutes and pulls back on
arrival. No line production changes a single home. Left in the engine behind
`Session::crew` (on).

**7.2 Canadair — PASS (district edge only).**

| Called turn 1, then one drop a turn on… | Homes hit | Δ vs none | Δ wind holds | Δ shifts | Drops ≤ T+40 |
|---|---|---|---|---|---|
| never called | 66.2 ± 2.7 | — | — | — | 0 |
| Il Borgo's edge | 61.2 ± 2.7 | −5.1 ± 1.0 | −10.4 ± 2.0 | −2.2 ± 0.8 | 2.0 ± 0.0 |
| the district the wind drives the fire at | 59.7 ± 2.9 | −6.5 ± 1.2 | −10.4 ± 2.0 | −4.4 ± 1.3 | 2.0 ± 0.1 |
| left flank / right flank | 66.5 / 66.3 | +0.3 / +0.1 | | | 0.8 / 0.9 |
| the first spot fire | 65.9 ± 2.6 | −0.3 ± 1.5 | | | 1.2 ± 0.1 |
| the head | 65.0 ± 3.2 | −1.2 ± 1.4 | | | 0.6 ± 0.1 |

Spot fires put out: 0.2–0.3 per session whatever the plane does. The flight is
straight from the station; where it crosses lethal heat the aircraft's own
policy breaks off (counted as a withdrawal, `Ritirato` next turn).

**7.3 Engine on a spot fire — FAIL (spot fires do appear).** Every session
shows a spot-fire target at some turn opening (40/40; 2.7 ± 0.1 of the five
openings; 12.2 ± 1.0 spot fires per session). An engine sent to the first one
the turn it appears: it went out 2/40, merged with the main fire 34/40, still
burning 4/40; homes Δ −0.4 ± 2.3. The crew: out 3/40, Δ 0.0. Spot-fire targets
left behind `Session::spot_targets` (on).

**7.4 Patrol — FAIL (marginal).** Families caught:

| Policy | All | Wind holds (Il Borgo) | Shifts (Le Coste) | Warning lands Il Borgo / Le Coste |
|---|---|---|---|---|
| none | 13.2 ± 0.7 | 12.1 ± 1.5 | 13.8 ± 0.8 | — |
| patrol Il Borgo t1 | 11.6 ± 0.9 | 7.4 ± 1.4 | 13.8 ± 0.8 | T+2.9 / — |
| instant warning Il Borgo T+0 | 11.0 ± 0.9 | 5.7 ± 1.0 | 13.8 ± 0.8 | T+0 / — |
| patrol Il Borgo t3 | 13.2 ± 0.7 | 12.1 ± 1.5 | 13.8 ± 0.8 | T+19 / — |
| patrol Il Borgo t1 + Le Coste t2 | 7.8 ± 0.5 | 7.4 ± 1.4 | 8.1 ± 0.4 | T+2.9 / T+10.8 |
| patrol Le Coste t1 + Il Borgo t2 | 6.9 ± 0.7 | 10.9 ± 1.4 | 4.8 ± 0.3 | T+10.8 / T+4.6 |
| IT-alert t1 | 4.6 ± 0.4 | 5.7 ± 1.0 | 3.9 ± 0.2 | T+0 / T+0 |
| instant Il Borgo + Le Coste T+0 | 4.6 ± 0.4 | 5.7 ± 1.0 | 3.9 ± 0.2 | T+0 / T+0 |

Patrol 7.4 against a pass line of 6.05 (half of 12.1). The three-minute drive
costs 1.7 families against an instant warning. Warning Il Mulino changes
nothing (the IT-alert equals instant Il Borgo + Le Coste exactly).

**7.5 Tank — no early-commitment penalty; lesson 6 dropped.** Wind-holds
sessions (14):

| Engines on Il Borgo | Homes hit | Il Borgo homes | Δ vs posted t1 | At the threat: working with water / refilling |
|---|---|---|---|---|
| none | 61.4 ± 4.3 | 60.0 ± 3.8 | +39.4 ± 1.7 | — |
| posted turn 1 | 22.1 ± 3.5 | 20.6 ± 2.9 | — | 2.2 ± 0.1 / 0.4 ± 0.1 |
| posted turn 2 | 25.8 ± 3.4 | 24.2 ± 3.1 | +3.6 ± 2.1 | 2.1 ± 0.3 / 0.4 ± 0.1 |
| posted turn 3 | 25.2 ± 3.9 | 23.8 ± 3.6 | +3.1 ± 1.9 | 2.0 ± 0.4 / 0.0 |

Earlier is (weakly) better. Home defence counts an engine from the moment it
starts work at its post until it is re-tasked, refills included
(`demo::run::Tally`), so running dry costs no homes.

**7.6 Head attack — FAIL.**

| Sent to the head | Withdrew within the turn | Within two turns | Minutes to withdraw | Work done first |
|---|---|---|---|---|
| engine, turn 1 / 2 / 3 | 3 / 0 / 15 of 40 | 6 / 15 / 23 | 10.1 / 14.5 / 7.8 | 2,313 / 2,758 / 1,860 L |
| crew, turn 1 / 2 / 3 | 0 / 0 / 0 of 40 | 5 / 16 / 28 | 15.6 / 15.1 / 14.9 | 0 m |
| *`HeadOrder::Track`* engine, t1 / t2 / t3 | 3 / 2 / 21 of 40 | 6 / 21 / 23 | 7.8 / 11.4 / 5.6 | 2,298 / 2,512 / 1,517 L |

An engine stops at the road nearest the head and works the roadside out of
lethal heat; the crew is still driving at the end of the turn. Homes hit are
nonetheless unchanged (`engines-head` 66.4 ± 3.1 vs `none` 66.2 ± 2.7).
`Session::head_order` (`Fixed` by default; `Track` re-tasks to the moving head
every minute) is left for the lead.

## 8. Balance targets (`tests/balance.rs`)

Policies: `none`; `patrol-borgo-t1`; `patrol-borgo-t1 + patrol-coste-t2`;
`patrol-coste-t1 + patrol-borgo-t2`; `patrol-everyone` (Borgo, Coste, Mulino in
turns 1–3); `patrol-mulino-first`; `engines-borgo-t1`; `engines-head` (wrong);
`crew-borgo-t1`; `crew-mulino-t1` (wrong); `forecast-player` (patrol to the
district the forecast makes most likely to be hit first, then the other; Canadair
t1 on that district; engines and crew on Il Borgo t1, engines → Le Coste on the
shift); `wait-and-see` (everything on threat).

Targets: `forecast-player` is best or tied on families **and** homes;
`patrol-everyone` and `patrol-mulino-first` lose on families and a stamp;
`engines-head` = `none` on homes; `crew-borgo-t1` beats `crew-mulino-t1` by ≥ 5
homes; `wait-and-see` loses on families; no single policy beats
`forecast-player` on both counts in more than a third of seeds. Whether Borgo
or Coste first is right should **depend on the forecast** — report the split.

### Measured after triage (2026-10-07, seeds 1–40)

Reproduce: `cargo test --release -p demo --test balance -- --ignored
--nocapture`. IT-alert and spot-fire targets are removed; crew home defence
is on. Mean ± s.e.; differences pair the same seed.

| Policy | Families caught | Homes hit | Δ caught vs forecast-player | Δ homes | Il Mulino *Allarme inutile* | Beats forecast-player on both |
|---|---|---|---|---|---|---|
| none | 13.2 ± 0.7 | 66.2 ± 2.7 | +6.5 ± 0.7 | +36.3 ± 1.5 | 0/40 | 0/40 |
| patrol-borgo-t1 | 11.6 ± 0.9 | 66.2 ± 2.7 | +4.8 ± 0.9 | +36.3 ± 1.5 | 0/40 | 0/40 |
| patrol-borgo-t1 + patrol-coste-t2 | 7.8 ± 0.5 | 66.2 ± 2.7 | +1.1 ± 0.4 | +36.3 ± 1.5 | 0/40 | 0/40 |
| patrol-coste-t1 + patrol-borgo-t2 | 6.9 ± 0.7 | 66.2 ± 2.7 | +0.2 ± 0.5 | +36.3 ± 1.5 | 0/40 | 0/40 |
| patrol-everyone | 7.8 ± 0.5 | 66.2 ± 2.7 | +1.1 ± 0.4 | +36.3 ± 1.5 | 39/40 | 0/40 |
| patrol-mulino-first | 11.3 ± 0.6 | 66.2 ± 2.7 | +4.6 ± 0.5 | +36.3 ± 1.5 | 39/40 | 0/40 |
| engines-borgo-t1 | 13.2 ± 0.7 | 48.0 ± 3.4 | +6.5 ± 0.7 | +18.0 ± 2.4 | 0/40 | 0/40 |
| engines-head | 13.6 ± 0.7 | 67.2 ± 3.1 | +6.9 ± 0.7 | +37.3 ± 2.2 | 0/40 | 0/40 |
| crew-borgo-t1 | 13.2 ± 0.7 | 53.8 ± 2.7 | +6.5 ± 0.7 | +23.8 ± 1.7 | 0/40 | 0/40 |
| crew-mulino-t1 | 13.2 ± 0.7 | 66.2 ± 2.7 | +6.5 ± 0.7 | +36.3 ± 1.5 | 0/40 | 0/40 |
| forecast-player | 6.7 ± 0.5 | 29.9 ± 2.9 | — | — | 0/40 | — |
| wait-and-see | 13.2 ± 0.7 | 45.9 ± 3.3 | +6.5 ± 0.7 | +15.9 ± 2.1 | 0/40 | 0/40 |

All seven balance assertions pass. Forecast-player is best or statistically
tied on both counts; no tested policy beats it on both in any of the 40 seeds.
The crew on Il Borgo saves 12.4 homes on average; on Il Mulino it saves none.

Patrol order, split by the opening forecast (Borgo-first minus Coste-first;
negative means Borgo-first is better):

| Shift probability | Seeds | Wind turned | Δ families caught |
|---|---|---|---|
| <30 % | 3 | 1 | −1.3 ± 0.9 |
| 30–49 % | 19 | 11 | +0.0 ± 1.0 |
| 50–59 % | 5 | 4 | +2.6 ± 1.6 |
| ≥60 % | 13 | 10 | +2.2 ± 0.8 |

The low-probability group is too small for a strong conclusion; high shift
probability favours Le Coste first in this sample. The cost of a needless
stop is demonstrated by **Il Mulino first**, which delays the useful warnings.
Visiting Il Mulino *after* Borgo and Coste leaves families caught unchanged
(7.8 in both policies), and costs only the stamp. Do not teach that a third,
late stop necessarily traps extra families.

### 7.7 Boosted crew — measured (2026-10-07)

The approved fallback ships: **off-road home defence, no water, slow arrival**.
The published suppression numbers stay unchanged. `Variant::crews_defend`
is false by default outside the kiosk variant and inert without a crew order.

The corrected sweep isolates the mechanisms: home defence is **off** for all
perimeter-firebreak runs, and the fallback uses the actual game order
(`CrewMode::Difesa`). Reproduce: `cargo test --release -p demo --test sweeps
s7_boosted_crew -- --ignored --nocapture`.

Crew sent at turn 1, mean ± s.e.; paired change in homes hit in the target
district against no orders. Negative means homes saved.

| Crew | Il Borgo, wind holds | Le Coste, wind shifts | Il Mulino, all | Line cut (m, Borgo) | Starts work (min, Borgo) |
|---|---|---|---|---|---|
| perimeter firebreak, line ×1 | −0.1 ± 0.1 | +0.2 ± 0.3 | 0.0 | 85.6 ± 1.8 | 15.0 |
| perimeter firebreak, line ×4 | −0.1 ± 2.8 | +0.5 ± 0.9 | 0.0 | 336.4 ± 9.9 | 15.0 |
| perimeter firebreak, line ×10 | +0.2 ± 2.7 | −0.6 ± 1.1 | 0.0 | 484.6 ± 8.1 | 15.0 |
| perimeter firebreak, line ×20 | −0.3 ± 2.7 | +0.2 ± 1.4 | 0.0 | 494.2 ± 6.2 | 15.0 |
| perimeter firebreak, line ×40 | −0.1 ± 2.5 | +0.4 ± 1.4 | 0.0 | 502.9 ± 1.3 | 15.0 |
| home defence (game order) | **−25.4 ± 1.4** | **−13.8 ± 0.3** | **0.0** | 92.4 ± 0.7 | 13.4 ± 0.1 |

A firebreak alone still fails the five-home gate at the largest boost tried
(×40); the fallback passes on both threatened districts and stays inert on
Il Mulino. The game crew reaches its post on foot and covers homes within
`DEFEND_REACH_M`, using the same tally as engine defence. Its cut line alone
is not the source of the measured saving.

## 9. Model rules (binding)

`Sim::advance` + `Referee` is the one stepping path (finding 5); same seed +
draw + orders ⇒ same verdict, live or twin (tested); every new mechanism —
patrol delay, reinforcement, targets — is **provably inert until used**
(finding 34); every new behaviour gets a test that asserts it fires; a number
published in `CLAUDE.md` is not changed without flagging; `cargo test --release`
stays green.
