# Demo — shared spec

Interactive kiosk demo for students visiting the gazebo at the *Settimana della
Protezione Civile*, Rome. Spec written 2026-10-06; event about a week out.
Read `CLAUDE.md` for the model's findings (they are constraints).

| Spec | Works on | Verified by |
|---|---|---|
| [`demo-spec-gameplay.md`](demo-spec-gameplay.md) | what the player decides and what it costs: towns, districts, units, cost, forecast, balance | headless `crates/demo` runs and A/B sweeps — **no GUI** |
| [`demo-spec-presentation.md`](demo-spec-presentation.md) | how it looks and reads: graphics, town kit, kiosk UI, overlays, ops | screenshots, frame rate, a real pointer |

Companion: `docs/demo-notes.md` (the first-loop critique that led to v2) and
`docs/demo-operator.md` (operator one-pager).

**There is one mode.** The game *is* the kiosk: `cargo run --release -p game`.
Env vars are test/ops harnesses only (`KIOSK_SHOT`, `KIOSK_TOWN`,
`KIOSK_WINDOWED`, `KIOSK_PLAY_S`, `KIOSK_SHOT_ZOOM`, `KIOSK_SHOT_FOCUS`,
`KIOSK_IDLE_S`, `KIOSK_FPS`).

## 0. The game in one paragraph (v2, 2026-10-06)

A fire starts near a small Lazio town made of **three districts** at three
bearings from it. The wind drives it at one district; the forecast gives odds
that the wind turns onto a second; the third is upwind and safe. The briefing
**stops the clock**: the student reads the wind arrow and the forecast card and
gives the first orders from **chips pinned over each district** — *Avvisa*
(warn this district) and *Difendi* (send a fire engine to protect its homes) —
then presses **Via!**. The fire runs (~2½ min), advisors comment on what the
model does (spot fires, the wind turning, a district threatened, a mast
down), a forecast update arrives at T+8 when acting on it still helps. The end
panel, beside the burnt town, tells **each district's story** ("Avvisato a
T+0 · fuoco a T+24: 24 min di anticipo" / "Mai avvisato" / "falso allarme"),
compares families caught with the **same fire and no orders**, and awards up to
three medals: *In tempo*, *Nessun falso allarme*, *Case difese*.

The lessons, each pinned by a headless test (gameplay §2): **the fire goes
where the wind blows** (a different district is caught when the wind turns);
**warn early** (T+0 ≈ ¼ of families caught vs no orders, T+20 ≈ no orders);
**waiting to see the fire saves nobody**; **warn who is at risk, not everyone**
(same families saved, one fewer false alarm); **engines protect homes, not
people, and only where the fire is going** (right district: homes hit halved;
upwind district: nothing).

## 1. Goal

A student walks up cold and, within 30 s, is making decisions as an incident
commander in a small stylised town, watching individual families respond to a
fire. A session is **3–4 min**, ends on a clear outcome, and invites a second
try. Success: a bystander says unprompted *"the fire goes where the wind blows,
people need warning early, but not everyone, and my decisions changed who got
out."*

## 2. Decisions and non-goals

Settled with Mirko:
- Hardware: capable desktop, mouse only, no touch, **no speakers** (captions only).
  Audience: students; tone clear, not childish; loss framing never dwells on harm.
- **Setting: Roman region.** Fictional Lazio-flavoured towns (Rocca Ventosa, Due
  Casali, Porto Pineta; native-speaker review needed).
- **Graphics target: toy diorama / SimCity-like town** — chunky pastel buildings
  with recognisable civic buildings and utilities, plinth, tilt-shift, soft
  shadows, 30 fps on modest hardware (presentation §4).
- **Visual identity: CIMA Foundation** — navy `#001E31`, blue `#004070`, orange
  `#DD7500`, pale grey `#D4DBDE`, white mark `assets/brand/cima_logo_white.png`.
- Facts + counterfactual on the end card; **no numeric score**. Medals are facts
  about the session (in time / no false alarm / homes defended), not points.
  Money is a *cost shown*, not a score.
- **LLM bubbles: OpenRouter** (canned fallback mandatory) — cut-list item.
- **The model may be changed** (experimental branch), except that changing a number
  published in `CLAUDE.md` is flagged to Mirko. `cargo test --release` stays green.
- **No day/night cycle.**

Taken in this pass (2026-10-06, user mandate "improve in every aspect"; reversible):
- **W1 → option B.** Units *protect homes*: the kiosk plays with
  `Variant { defend_homes: true }`; *Difendi* posts an engine on a district's
  fire-facing edge. Crews are no longer offered in the kiosk (they withdraw from
  the head and cannot change an outcome, gameplay §4).
- **Cry-wolf off in the kiosk.** Measured: no in-session judgement time is both
  fair to a correct early warning and early enough to bite (gameplay 5.2). False
  alarms are judged in hindsight on the end card.
- **Districts are data.** A household's `locality` *is* its district; the towns
  were regenerated (gameplay §2). The old one-village layouts are gone.

Still waiting on Mirko: **W2** delete `mati`/`pedrogao`/`rhodes` (keep `spotorno`
as a test fixture) — not needed by the demo, untouched; **W3** sourced tariffs
(the bill is shown but uses placeholder prices).

Non-goals: real places or historical fires; zoning/budgets beyond intervention
cost; web build; debrief; control API in the kiosk; persistent leaderboard.

## 3. The contract

The only interface between model and presentation is the `demo::` API (table in
`demo-spec-gameplay.md` §1). **One set of books:** both the headless twin
(`demo::Run`) and the live kiosk (`game::Sim`) step a `demo::Referee` around the
model, with the same `Variant`; the test
`game::sim::tests::the_live_session_and_its_headless_twin_agree` pins outcome,
district reports and ledger equal on every town. The UI never computes game
logic (badges, district levels, reached/needless are all `demo::district`), and
the model never formats Italian (`strings_it.rs` maps every `EventKind`).

## 4. Milestones (remaining)

| Step | Deliverable | Gate | State |
|---|---|---|---|
| 1 | Clean-up to a demo-only repo | `cargo test --release` green, acceptance 8 | game ✅, model side 🔲 (W2) |
| 2 | District game in the model, pinned | `tests/districts.rs`, `units.rs`, `trust.rs` green | ✅ |
| 3 | District game in the kiosk, one set of books | twin == live test | ✅ |
| 4 | Town kit (Blender) + SimCity look | screenshot of every town; 30 fps | 🔶 kit in, polish §presentation 12 |
| 5 | **Playtest with 3–5 new people**, Italian review, operator doc, `KIOSK_SELFTEST`, supervisor | nobody stuck > 60 s | 🔲 |
| 6 | Buffer, fixes only; LLM bubbles if time | | 🔲 |

Cut order if days slip: LLM bubbles, Canadair button, `demo_porto`, custom
assets polish. **Never cut**: kiosk shell, idle reset, the district chips, the
end card with the no-orders comparison, the wind arrow, the forecast.

## 5. Work needed next (ordered)

1. **Both — Playtest** (milestone 5) on a real display with a real mouse: does a
   newcomer find *Avvisa* within 30 s; is the frozen briefing understood; are the
   chips too busy when all three show; fps on the target machine.
2. **G — Balance.** Valle's best play still leaves ~8 families caught (borgo ~4);
   Porto's "warn the at-risk" lags "warn all" (Il Faro is caught via the road in
   shift sessions). Decide whether Porto's district 2 should be bait at all.
3. **P — Look.** Fire stylisation pass (flames/smoke read as a smudge from the
   play camera); house roofs and street trees; district ground tint by level;
   selection feedback when *Difendi* is pressed (a ring on the post).
4. **P — Ops.** `KIOSK_SELFTEST`, `scripts/run_demo.sh`, operator quit, native
   Italian review of `strings_it.rs`.
5. **G — W2/W3** when Mirko decides.

## 6. Acceptance

1. Cold start to first order ≤ 30 s with no staff explanation. — not timed.
2. Session 3–4 min, ends on the outcome panel without an operator. ✅ (pace: 3 s per
   simulated minute to T+22, 1.6 after; fast-forward ×3).
3. 60 s idle anywhere returns to ATTRACT with clean state — leaks unverified until
   `KIOSK_SELFTEST`. 🔲
4. Every town: the right early warning halves families caught vs no orders ✅;
   waiting to see the fire saves nobody ✅; warning everyone raises false alarms ✅
   (`tests/districts.rs`).
5. A forecast is shown, right on average, wrong sometimes ✅, and *useful*: acting on
   the T+8 update still saves about half of a shifted district ✅ (`warning_decay`).
6. No single-key shortcut does anything; the operator corner does. ✅ / 🔲 (pointer).
7. No English visible; no audio required. ✅
8. `cargo test --release` passes ✅; the repo contains no non-demo scenarios ❌ (W2).
9. No fixed policy dominates on *everything*: "warn all" ties on families but loses
   on false alarms and money; "warn downwind only" loses on shift sessions;
   "react to the shift" and "wait to see" lose on families ✅ (`district_sweep`).
10. Engines on the right district beat engines on the wrong one and none ✅
    (`units::defending_the_right_district_saves_homes`).
11. The town is legible at 2 m: districts, civic buildings, people, cars, fire
    labelled, in a screenshot of every town ✅ (dev Mac); 30 fps on target 🔲.

## 7. History

v1 (one *Evacuazione* button, five-button action bar, COMPARE screen) and its
playtest (§16, 37 findings) are in git at `07cc021`/`1eb263c`. v2 (districts,
Referee, town kit) is `bb649ed` + `523c14a`.
