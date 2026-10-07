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
| `Referee` (one set of books) + `turn() -> Turn`, `tokens() -> &[Token]`, `targets() -> Vec<Target>`, `preview(token, target) -> Option<Preview>`, `assign(token, target)`, `unassign(token)`, `end_turn()`, `report() -> &TurnReport`, `verdict() -> Verdict` | the turn engine; kiosk and twin call the same methods in the same order | 🔲 |
| `Turn { index, at_s, last: bool }`, `TURN_S` (8 min), `TURNS` (5), `MISSION_S` (60 min) | the clock | 🔲 |
| `Token { id, kind: TokenKind, state: TokenState, water: Option<f32>, at: Pos, order: Option<TargetId> }` | the tray and the units on the map | 🔲 |
| `TokenKind::{Pattuglia, ItAlert, Autobotte, Squadra, Canadair}` | §3 | 🔲 |
| `TokenState::{Libero, InViaggio{eta_s}, AlLavoro, Rifornimento{eta_s}, Ritirato, Perso, Usato, InArrivo{eta_s}, NonChiamato}` | badge on the token and the unit | 🔲 |
| `Target { id, kind: TargetKind, pos, label_pos }`, `TargetKind::{District(d), Head, Flank(Side), SpotFire(n), Sky}` | §4; `Sky` is the Canadair call | 🔲 |
| `Preview { eta_s, effect: Effect }`, `Effect::{Avvisa{families}, AvvisaTutti, Difende{homes}, Linea, Spegne, Ritirata, Chiamata{eta_s}, Inutile}` | what the target says before you commit (§4) | 🔲 |
| `TurnReport { lines: Vec<ReportLine> }` (≤ 3, ranked), `ReportLine { kind, district: Option<usize>, pos: Option<Pos>, n: Option<u32> }` | the between-turn report (§6) | 🔲 |
| `Verdict { districts: Vec<DistrictVerdict>, families_safe, families_caught, homes_hit, none: Counterfactual, notes: Vec<Note> }` | the end card (§6) | 🔲 |
| `DistrictVerdict { district, people: Stamp, homes_hit, homes_hit_none }`, `Stamp::{InTempo, Tardi, MaiAvvisati, GiustoNonAvvisare, Prudente, AllarmeInutile}` | one row per district | 🔲 |
| `Note::{TestaRitirata, CanadairInTempo, CanadairTardi, CanadairMaiChiamato, AutobotteASecco, FocolaioSpento, ItAlertSprecato, …}` | doctrine kept or broken, ≤ 3 on the card | 🔲 |
| `Forecast` + `Draw::forecast(issue)`, issue 2 at turn 2 | the forecast | ✅ |
| `District { name, households, centre, radius_m }`, `Report` | district facts | ✅ |
| `Event` (existing kinds) | raw material for report lines | ✅ |

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
| 🚓 **Pattuglia** (Polizia Locale) | 1 | warn district *d* | 🔲 new: the car drives from staging by road; `EvacuateDistrict(d)` is applied **when it arrives**; free again the turn after | one district at a time; the order you warn them in matters |
| 📢 **IT-alert** | 1 use | warn every district | `EvacuateAll`, immediate; households out of mast coverage are not reached (`abm::comms`) | Il Mulino gets a needless warning; spent for good |
| 🚒 **Autobotte** | 2, **+1 at turn 3** | defend district *d* · attack a spot fire · (head / flank) | `Defend` (option B, `defend_homes`), `Attack` | roads only, 60 m hose; 2,500 L = ~6 min pumping, then drives to a hydrant and back (`Refilling`); withdraws from lethal heat |
| 👷 **Squadra AIB** | 1 | cut line on a flank · line on a district's fire edge | `Attack` (crew: line) | 120 m/h; at the head it withdraws; ember shadow jumps a line 300 m ahead (finding 41) |
| ✈️ **Canadair** | 1 | call (turn *k*) · then one drop per turn on head / flank / spot fire / district edge | `request_air` then `Drop` | arrives `AIR_RESPONSE_S` (25 min ≈ 3 turns) after the call; a drop wets fuel, wears off (`DROP_DEFENCE_S`) |

Rules:
- **Reassign any time a turn opens.** A unit at work that is reassigned leaves
  its post and drives to the new one; travel time is the price, shown as the ETA.
- A token that is `Rifornimento`, `Ritirato` (one turn), `InArrivo` or `Perso`
  cannot be given an order that turn. `Usato` is the spent IT-alert.
- The reinforcement engine appears in the tray at turn 3 at staging.
- Units are placed by the model's own logic around the target (`post_facing`,
  nearest reachable road — finding 17); the player picks *what*, never *where to
  the metre*.

## 4. Targets

Fixed points the model publishes each turn; the UI lights the ones valid for the
selected token and shows `preview` on hover.

| Target | Where | Valid for | Preview effect |
|---|---|---|---|
| **District d** | the district's fire-facing edge (`District::post_facing`) | Pattuglia, Autobotte, Squadra, Canadair | *Avvisa N famiglie* / *Difende N case* / *Linea* / *Bagna il bordo* |
| **Testa** | `run::head_of` | Autobotte, Squadra, Canadair | *Si ritirerà* (ground units — lesson 4); drop: *Rallenta poco* |
| **Fianco sx / dx** | perpendicular to the spread direction, 1/3 back from the head | Autobotte (if a road reaches), Squadra, Canadair | *Linea* / *Bagna* / *Strada troppo lontana* |
| **Focolaio n** | each spot fire (`SpotFire` events, finding 37), until out or merged | Autobotte, Squadra, Canadair | *Spegne* |
| **Cielo** | the tray slot itself | Canadair (when `NonChiamato`) | *Arriva al turno k* |
| — | everywhere | IT-alert | *Avvisa tutti (anche chi non serve)* |

A target never valid for a token is not lit. `Effect::Inutile` is shown, not
refused: the player may still send an engine to Il Mulino and learn from it.

## 5. Lessons — each a test that must fire (`tests/lessons.rs`)

| # | Lesson | Assertion (Rocca Ventosa, drawn seeds) |
|---|---|---|
| 1 | The wind decides who | no orders: wind holds → Il Borgo caught ≫ Le Coste; shift → Le Coste caught > 0; Il Mulino 0 |
| 2 | Warn early | patrol to Il Borgo at turn 1 ≪ at turn 3 in families caught |
| 3 | Don't warn everyone | IT-alert at turn 1 = patrol policy ± s.e. on families, and always stamps Il Mulino *AllarmeInutile* |
| 4 | Never the head | engine or crew to `Head` ⇒ `Ritirato` within the turn, homes hit = no units |
| 5 | Defend where it goes | 2 engines on Il Borgo vs on Il Mulino: homes hit halved vs unchanged |
| 6 | Water runs out | engines posted at turn 1 vs turn 2: fewer working when the front reaches Il Borgo (if the sweep confirms, §7.5) |
| 7 | Call aircraft early | Canadair called turn 1 delivers ≥ 2 drops before T+40; called turn 3 ≤ 1; and a drop matters (§7.2) |
| 8 | People first | units alone (no warning) change homes hit, not families caught; warnings alone the reverse |

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
| `InTempo` | warned, and the warning landed ≥ `IN_TIME_MIN` before the fire reached the district |
| `Tardi` | warned, landed later than that |
| `MaiAvvisati` | reached, never warned |
| `GiustoNonAvvisare` | not warned, never threatened |
| `Prudente` | warned while the forecast in force pointed at it; never threatened. **Not a mistake.** |
| `AllarmeInutile` | warned, never threatened, and no forecast pointed at it |

**Counterfactual:** the same draw, no orders, from the twin.
**Notes:** at most three, from the order log and unit history (§1 `Note`), the
broken rule first.

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

## 8. Balance targets (`tests/balance.rs`)

Policies: `none`; `patrol-borgo-t1`; `it-alert-t1`; `patrol-borgo-t1 +
patrol-coste-t2`; `engines-borgo-t1`; `engines-head` (wrong); `all-in` (IT-alert
t1, engines Borgo t1, Canadair t1); `forecast-player` (patrol Borgo t1, Canadair
t1, engines Borgo t1→Coste on shift, patrol Coste t2 if the forecast says
likely); `wait-and-see` (everything on threat).

Targets: `forecast-player` is best or tied on families **and** homes;
`all-in` ties on families but always loses a stamp (*AllarmeInutile*);
`engines-head` = `none` on homes; `wait-and-see` loses on families; no single
policy beats `forecast-player` on both counts in more than a third of seeds.

## 9. Model rules (binding)

`Sim::advance` + `Referee` is the one stepping path (finding 5); same seed +
draw + orders ⇒ same verdict, live or twin (tested); every new mechanism —
patrol delay, reinforcement, targets — is **provably inert until used**
(finding 34); every new behaviour gets a test that asserts it fires; a number
published in `CLAUDE.md` is not changed without flagging; `cargo test --release`
stays green.
