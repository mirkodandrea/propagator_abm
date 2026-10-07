# Demo — shared spec

Interactive kiosk for students visiting the gazebo at the *Settimana della
Protezione Civile*, Rome. Written 2026-10-07; this is the only version of the
game — earlier designs are scrapped, not carried. Read `CLAUDE.md` for the
model's findings (they are constraints).

| Spec | Works on | Verified by |
|---|---|---|
| [`demo-spec-gameplay.md`](demo-spec-gameplay.md) | the scenario, turns, resources, targets, lessons, verdict, balance | headless `crates/demo` runs and A/B sweeps — **no GUI** |
| [`demo-spec-playtest.md`](demo-spec-playtest.md) | the headless `play` binary (ASCII screen, text commands) and the blind playtest loop | a playtest agent that has never seen this repo |
| [`demo-spec-presentation.md`](demo-spec-presentation.md) | the kiosk: screens, token tray, map signals, verdict card, look, ops | screenshots, frame rate, a real pointer |

`docs/demo-operator.md` is rewritten when the kiosk ships (milestone 4).

Legend: ✅ done · 🔶 partly · 🔲 to do · ✂ cut-list item.

## 0. The game in one paragraph

A fire starts in the pines south of **Rocca Ventosa**, a small Lazio town of
three districts. The student is the incident commander and plays **five turns
of eight simulated minutes**. Each turn the clock stops: they read the wind and
the forecast, then give orders by **picking a resource and then a target** — a
district, the fire's head or flanks, a spot fire. The resources are few and
each has a catch: **one police patrol** that warns a district only when it gets
there, **one IT-alert** that warns everyone (including the district that did not
need it), **two fire engines** (+1 at turn 3) that save homes but run dry in six
minutes, **one hand crew** that cuts line slower than the fire runs, and **one
Canadair** that comes only if called, 25 minutes later. *Avanti* plays the
turn; a three-line report says what happened. After turn 5 the fire runs to
T+60 and each district is stamped. The verdict counts **families safe** and
**homes saved**, each against the same fire with no orders, and names the
firefighting rules the student kept or broke.

## 1. Goal and lessons

A student walks up cold and within 30 s gives a first order. A session is
**3–4 min**, ends on a clear verdict, and invites a second try. Success: asked
afterwards what they learnt, a player says something like *"you can't stop the
head of a fire in the wind — you warn the people in its way early, put the
engines where it's going, and call the plane before you need it."*

The lessons (each a headless test that must *fire*, gameplay §5):

1. **The wind decides who is at risk.** The forecast says where it may go next.
2. **Warn early.** Families need time to leave; a late warning saves few.
3. **Don't warn everyone.** A needless warning is a cost, not a safe default.
4. **Never attack the head.** Units sent there pull back and the turn is lost.
5. **Defend where the fire is going.** Engines save homes only on its path.
6. **Water runs out.** An engine committed too early is refilling when the
   front arrives.
7. **Call aircraft early.** A 25-minute response is a decision about the future.
8. **People first.** Warnings save people; units save homes. Counted apart.

The lessons are **never shown to a playtester** (playtest spec §3): whether a
blind player states them unprompted is the measure.

## 2. Decisions

- **One scenario: Rocca Ventosa** (`demo_borgo`). The specs describe nothing
  else; other scenario files in `data/` are left as they are.
- **Turn-based.** The clock never runs while the player decides.
- **Scarce resources with a catch each** are the game; alerting alone is an
  on/off decision.
- **One interaction grammar:** resource → target. The only other action is
  *Avanti*.
- **Judged on what the player knew.** A warning the forecast justified is never
  a false alarm, even if the wind held.
- **No numeric score.** Facts against a counterfactual (no orders, same fire).
  No money on screen.
- Hardware: capable desktop, mouse only, no touch, **no speakers**. Audience:
  students; tone clear, not childish; loss framing never dwells on harm.
- Visual identity: CIMA Foundation — navy `#001E31`, blue `#004070`, orange
  `#DD7500`, pale grey `#D4DBDE`, white mark `assets/brand/cima_logo_white.png`.
- Italian only, on the kiosk and in `play`. No day/night cycle.
- **The model may be changed**; changing a number published in `CLAUDE.md`, or
  making units more effective than the published model, is flagged to Mirko
  first. `cargo test --release` stays green.

## 3. The contract

The only interface between model and front ends is the `demo::` API (gameplay
§1). **One set of books:** the headless twin (`demo::Run`), the `play` binary
and the kiosk (`game::Sim`) all drive the same `demo::Referee` with the same
orders at the same turn boundaries. Front ends never compute game logic —
target validity, ETAs, previews, report lines, stamps and notes come from
`demo::`. All player-facing Italian lives in one Bevy-free crate
(`crates/text`) that both `play` and the kiosk use, so a playtest reads the
exact words the kiosk will show.

## 4. How it is built: a builder and a blind playtester

- The **builder** agent implements the specs, gameplay and `play` first.
- The **playtester** agent knows nothing about the project: it gets a sandbox
  with the `play` binary and a one-paragraph brief, plays several sessions,
  and writes a report on usability, fun and interest with suggested changes
  (playtest spec).
- Reports land in `docs/playtests/`. Mirko (with the lead agent) triages them;
  accepted changes go back into these specs, then to the builder. The builder
  never edits a spec to match what it built.

## 5. Milestones

| # | Deliverable | Gate | State |
|---|---|---|---|
| 0 | **Sweeps** on Rocca Ventosa (gameplay §7) | each resource has a right and a wrong use that differ measurably; resources that fail are cut | 🔲 |
| 1 | Turn engine in `demo`; `crates/text`; `play` binary; `scripts/playtest.sh` | gameplay §5 lessons pinned; `play` walks a full session in < 1 min of commands | 🔲 |
| 2 | **Blind playtest rounds** (≥ 2, changes between them) | playtester scores clarity and fun ≥ 4/5; states ≥ 4 lessons unprompted | 🔲 |
| 3 | Kiosk: tray, targets, turn loop, report, finale, verdict | screenshot of every screen; twin test green | 🔲 |
| 4 | Human playtest (3–5 people), operator doc, `KIOSK_SELFTEST`, Italian review | first order ≤ 30 s; nobody stuck > 60 s | 🔲 |
| 5 | Buffer, fixes only | | 🔲 |

Cut order if days slip: the hand crew, the reinforcement, the spot-fire target,
the rule notes on the verdict, visual polish. **Never cut:** turns, patrol +
IT-alert, engines, the Canadair call, wind and forecast on the map, the verdict
against no orders, idle reset.

## 6. Acceptance

1. Cold start to first order ≤ 30 s with no staff explanation.
2. Session 3–4 min, ends on the verdict without an operator.
3. 60 s idle anywhere returns to the attract screen with clean state.
4. Every lesson in §1 is a headless test that fires (gameplay §5).
5. No fixed policy dominates on both families and homes (gameplay §8).
6. The last blind playtest meets milestone 2's gate.
7. No single-key shortcut does anything; the operator corner does.
8. No English visible; no audio required.
9. `cargo test --release` passes.
10. Town, fire head and flanks, every unit and its state legible from 2 m;
    30 fps on the target machine.

## 7. Open (Mirko)

- If a sweep shows a resource cannot change anything on Rocca Ventosa: cut it,
  or play with a `unit_effect` variant (flag: more effective than the published
  model).
- Seed: random per visitor (default) or a fixed seed of the day.
