# Demo — gameplay spec (headless, A/B-tested)

Read `docs/demo-spec.md` first (the game in one paragraph, decisions, the
contract) and `CLAUDE.md` (the model's findings are constraints). The
presentation side is `docs/demo-spec-presentation.md`.

Legend: ✅ done · 🔶 partly · 🔲 to do · ✂ cut-list item.

## 0. How this side works

**Backend only. No window, no Bevy, no screenshots.** Everything here is built and
judged through `crates/demo` and the model crates under it. If a change cannot be
measured headlessly it is not finished.

**Owns:** `crates/{demo,abm,fire,scenario,behavior,chat}`, `data/`, `scripts/`
(including `generate_demo_scenarios.py`). Anything the player must *see or read*
is delivered as a field or enum on a `demo::` type plus a row in §1.

**A/B method.** Every design question is settled by a measured comparison:
- a *policy* is a scripted commander (`demo::policy`): `none`, `evac T+k`,
  `at-risk T+k`, `downwind T+0`, `react to shift`, `district forecast`,
  `warn when threatened`, `DefendDistrict(d, n)`, …;
- a *variant* is a model option inert by default (finding 34), in `demo::Variant`;
- a sweep = policies × variants × towns × drawn seeds (`demo::sweep::run_grid`,
  all cores). Report mean ± s.e.; pin the conclusion with an assertion; record the
  table here.
- **Target:** no fixed policy wins on every dimension (families caught, false
  alarms, money), and every lesson in `demo-spec.md` §0 is a test that *fires*.

## 1. Contract with the presentation side

| Delivered by `demo::` | Meaning | Status |
|---|---|---|
| `Referee` (`new(spec, scn, agents, variant)`, `before_step(fire, crews) -> shifted`, `after_step(Parts)`, `order(Order, Parts)`, `tally`, `log`, `events`, `districts`, `reports`, `posted(d)`, `ledger(now)`, `trust(agents)`, `outcome(&Parts)`, `facts()`) and `Parts { scn, fire, agents, crews }` | **the one set of books**: the kiosk steps it around `Sim::advance`, `Run` around its own model | ✅ |
| `Run { scn, fire, agents, crews, spec, referee }`, `Run::with_variant`, `play`, `step`, `order`, `head()`, `district_toward(bearing)` | headless twin and sweeps | ✅ |
| `district::{District { name, households, centre, radius_m }, Report, Level, Badges, badges(), IN_TIME_MIN, THREATENED_M, WATCH_M}` | district chips and the end card. `Report { households, warned_at_s, threatened_at_s, reached_at_s, defended_at_s, closest_m, fire_now_m, caught, safe, moving }`, `level()`, `needless()`, `lead_min()` | ✅ |
| `Order::{EvacuateAll, EvacuateZone, EvacuateDistrict(d), Defend { kind, district }, Attack, Drop}` | everything the player can do | ✅ |
| `Outcome { households, safe, moving, in_danger, caught, homes_lost, hectares }` + `secure()` | headline numbers | ✅ |
| `Forecast` + `Draw::forecast(issue)`, `ISSUE_2_AT_S` (= 8 min) | forecast card, the T+8 update | ✅ |
| `Event { at_s, kind, pos }`, kinds `SpotFire`, `WindShifted`, `UnitWithdrew`, `UnitLost`, `MastDown`, `FireNearTown`, `DistrictThreatened{district}`, `DistrictReached{district}`, `FalseAlarm` | advisor lines (`strings_it::advisor`) | ✅ |
| `cost::{Log, Ledger}` via `Referee::ledger` | the bill | ✅ (tariffs placeholder, W3) |
| `run::{head_of, best_unit, staging}` | the fire label; unit choice shared by twin and kiosk | ✅ |
| `Variant { unit_effect, defend_homes, shift_p, cry_wolf }` | sweep switches; **kiosk plays `defend_homes: true` only** | ✅ |
| `why::Why` | one-line explanation (superseded on the card by per-district stories; kept for tests) | ✅ |
| `refusal::Refusal` | typed refusals (only the Canadair map click can be refused now) | ✅ |

## 2. The district game (v2) ✅

### 2.1 Why

v1 had one decision — *when* to press Evacuazione — and pressing it at T+0 was
always right. Measured on the v1 towns (`district_probe`, 12 drawn sessions): the
fire arrived **everywhere in a town at once** (borgo T+49–60 in every 200 m
bucket) and only reached the town in about half the sessions, so *who* to warn
had no answer and half the sessions taught nothing. Fires were also slow
(17–60 ha/h, ~1 km/h head run from a 60 m start), so the first two minutes on
screen were a smudge 1 km from the houses.

### 2.2 The towns (`scripts/generate_demo_scenarios.py`)

Each town is three named districts (a household's `locality`; `District` order =
the scenario's `localities`), placed so that **district 0 is downwind of the
opening wind, district 1 is downwind of the forecast shift, district 2 is upwind
in every forecast** (a false alarm if warned). Houses stand along streets, one
building per street slot shared by its households, with kinds (villa, house,
terrace, shop, apartments, hotel) and landmarks (presentation §5). Each town has
an *area di attesa* that `abm::refuge::choose` finds by itself (measured:
without it the only refuges were map-edge exits, and Porto's single refuge was
the end of the road through the fire).

| town | districts (households) | fire | wind / shift |
|---|---|---|---|
| Rocca Ventosa (`demo_borgo`) | Il Borgo 148 · Le Coste 64 · Il Mulino 38 | pines S of the village, r=100 m | S 35 km/h; 20–70 % → E 45 km/h at T+12–26 |
| Due Casali (`demo_valle`) | Casale Ovest 102 · Casale Est 102 · Fondovalle 96 | car fire on the road between the hamlets, r=100 m | E 30; 25–75 % → W 45 |
| Porto Pineta (`demo_porto`) | La Pineta 102 · Centro 180 · Il Faro 68 | pines N of town, r=150 m | N 40; 15–60 % → NE 40 |

Mission length 60 simulated minutes; preparation 6–18 min (12–35 left a third of
the town caught however early the warning).

### 2.3 Measured (pinned in `tests/districts.rs`, tables `tests/district_sweep.rs`)

Families caught at home, drawn sessions (12–16 per town):

| town | none | at-risk T+0 | at-risk T+20 | warn when threatened | warn everyone T+0 | false alarms at-risk / everyone |
|---|---|---|---|---|---|---|
| borgo | 13.2 | **4.2** | 12.9 | 12.8 | 4.2 | 0.17 / 1.17 |
| valle | 27.2 | **7.6** | 23.8 | 26.0 | 7.6 | 0.25 / 1.25 |
| porto | 20.3 | 9.8 | 20.0 | 20.3 | **6.9** | 0.00 / 0.75 |

- **The wind decides who** (no orders): borgo, wind holds → Il Borgo 13.9 / Le
  Coste 0; wind shifts → 0 / 11.6; upwind 0.0. Valle 15.7 / 0 vs 0 / 27.2.
- **Warning decay** (`warning_decay`, the district at risk warned at T+k): borgo
  hold 4.7 / 9.1 / 12.3 / 13.6 at T+0/5/10/15; valle shift 4.2 / 6.9 / 10.6 / 15.8.
  A warning at T+10 still saves about half — hence the forecast update at T+8.
- **Reacting to the shift when it happens** saves nothing (borgo 17.8 = downwind-only;
  valle 16.8 → 10.2 only because valle's shift is early). Act on the forecast.
- Earlier sweep with the 12–35 min preparation for reference: best play left
  10–16 caught; with 6–18 it is 4–10.

### 2.4 Engines (option B, adopted) — `tests/units.rs`

An engine posted to a district defends the homes within `DEFEND_REACH_M` (120 m)
of its post **from first arrival until it is re-tasked or lost**, not only while
pumping (the model rightly pulls crews out of lethal heat when the front passes,
which is when houses need them; tied to `Working`, three engines changed nothing).
Posts spread along the fire-facing edge (`District::post_facing`). Homes hit
(burnt ground within 150 m, or 25 m when defended), sessions where the wind holds:

| town | none | 3 engines on district 0 | on the upwind district |
|---|---|---|---|
| borgo | 68.4 | **32.9** | 68.4 |
| valle | 64.7 | **16.7** | 64.7 |

Timing matters less than place (T+3 vs T+15 within noise on borgo; the engines
arrive ~T+8 either way). Option A (multiplier) is not used; its pin was removed
because god-mode units now save ~8 homes on borgo — irrelevant while B ships.

### 2.5 Cry-wolf — off in the kiosk

Mechanism unchanged (`trust::CryWolf`, variant only). Measured
(`district_probe::threatened_times`): districts the fire really reaches are first
threatened at T+21–29 median (p10 11–14, max 46), so a ten-minute judgement (tried)
branded most *correct* early warnings false alarms and the trust meter punished
the right play. At 30 minutes it is fair but can no longer cost a later order that
matters. **Decision:** judge false alarms in hindsight on the end card
(`Report::needless`, medal *Nessun falso allarme*); `JUDGE_AFTER_S` back to 30 min;
`tests/trust.rs` pins only that the mechanism fires and is inert.

## 3. Clean-up (model side) 🔲 — blocked on W2

Unchanged: deleting `data/scenarios/{mati,pedrogao,rhodes}` needs Mirko's approval;
~40 abm/fire tests pin findings on Spotorno's geometry. Proposal stands: keep
`spotorno` as an unregistered test fixture, delete the other three, port the
pins one at a time.

## 4. Work needed (priority order)

1. **Porto balance.** "At-risk T+0" (La Pineta + Centro) leaves 9.8 caught against
   6.9 for "everyone": Il Faro's families are caught on the road in shift sessions.
   Either move Il Faro so it is truly upwind of both winds, or give it its own exit
   along the coast; re-run `district_sweep`.
2. **Valle residual.** Best play leaves ~8 caught (borgo ~4). Check who (the
   `who_is_caught_after_a_t0_warning` probe) before tuning.
3. **Cost (W3).** Replace placeholder tariffs with sourced ones; decide whether the
   bill should separate needless evacuations.
4. **Spot fires.** Check each town still produces ≥1 spot fire in a typical run
   (advisor line exists; no pin on the new towns).
5. **Seed of the day / operator switch** (open question).
6. **W2 clean-up** (§3).

## 5. Earlier steps (status)

- 5.1 Cost ✅ (`demo::cost`, placeholder tariffs; pure function of the log, pinned).
  The v1 forecast gate ("follow-forecast beats always/never") belonged to the
  one-order game and was retired; its district equivalent is §2.3.
- 5.2 Cry-wolf ✅ mechanism, off in the kiosk (§2.5).
- 5.3 Forecast ✅: calibrated draw, issue 2 at T+8 (was T+15, after the decision
  could still be taken).
- 5.4 Events ✅, plus district transitions and `FalseAlarm`.
- 5.5 Zone evacuation → superseded by districts ✅. `HoldRoad` ✂.
- 5.6 Decision prompts → advisors (presentation) ✅; road-cut event 🔲.
- 5.7 Typed refusals ✅.

## 6. Model rules (binding)

`Sim::advance` + `Referee` is the one stepping path (finding 5); same scenario +
seed + draw + orders ⇒ same outcome, live or twin (tested); every new mechanism is
**provably inert until used** (finding 34); tuned constants are per-scenario
(finding 38); every new behaviour gets a test that asserts it *fires*;
`cargo test --release` stays green. Demo constants changed in v2 (none published
in `CLAUDE.md`): `NEEDLESS_RADIUS_M` 800→300, `DEFEND_REACH_M` 80→120,
`ISSUE_2_AT_S` 15→8 min, defence follows the posting.

## 7. Open questions

- Should Porto keep a bait district, given its geometry (4.1)?
- Money: real unit costs and sources (W3).
- Session seeding: random per visitor (now), seed of the day, or operator switch.
- Should *Allerta generale* stay? It ties on families and teaches false alarms by
  contrast — keep, unless playtests show it becomes the only button pressed.
