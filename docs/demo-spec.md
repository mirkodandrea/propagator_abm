# Demo — shared spec

Interactive kiosk demo for students visiting the gazebo at the *Settimana della
Protezione Civile*, Rome. Spec written 2026-10-06; event about a week out.
Read `CLAUDE.md` for the model's findings (they are constraints); it still
describes the workbench, composer and live debugger, which are removed or about to
be.

**The work is split between two agents, each with its own spec:**

| Spec | Agent | Works on | Verified by |
|---|---|---|---|
| [`demo-spec-gameplay.md`](demo-spec-gameplay.md) | **gameplay** | what the player decides and what it costs: units, cost, trust, forecast, spot fires, balance | headless `crates/demo` runs and A/B sweeps — **no GUI** |
| [`demo-spec-presentation.md`](demo-spec-presentation.md) | **presentation** | how it looks and reads: graphics, kiosk UI, overlays, assets, ops | screenshots, frame rate, a real pointer |

Companion: `docs/demo-notes.md` (gameplay critique and ideas; mostly gameplay's).

**There is one mode.** The game *is* the kiosk: `cargo run --release -p game`, no
`DEMO` flag, no workbench. Env vars that remain are test/ops harnesses only
(`KIOSK_SHOT`, `KIOSK_TOWN`, `KIOSK_WINDOWED`, `KIOSK_PLAY_S`).

**State (2026-10-06, branch `settimana-protezione-civile`; gameplay and presentation
agent branches merged, full `cargo test --release` green).**
- *Presentation:* game-side clean-up done (kiosk-only `crates/game`, `CLAUDE.md`
  rewritten, `docs/demo-operator.md` written); diorama pass (plinth, fixed sun,
  depth of field); ~4k toy vegetation props instead of ~230k plants (57 fps vs
  20-30 on the dev Mac); toy people/cars/units; overlays (wind arrow, spot ring,
  beacons, closures); briefing/attract/end-screen layout. Still open: civic
  buildings, sway, family groups, ember/water effects, chevrons, selection ring,
  money/trust/event HUD slots, advisors, `KIOSK_SELFTEST`, supervisor script.
- *Gameplay:* A/B harness (`demo::{policy,sweep,Variant}`), cost, cry-wolf trust,
  events, typed refusals, `why`, zone evacuation, valle shift range. Unit
  effectiveness measured: multiplier (A) and slower fire (C) fail; protecting homes
  (B) works on valle only. Model-side clean-up blocked on deleting scenarios.
- **Not yet wired:** the new `demo::` fields (ledger, trust, events, refusals, why)
  are not called by `crates/game`; that integration is the next cross-agent step.

**Decisions waiting on Mirko:** (1) unit effectiveness: adopt B (kiosk calls
`Tally::enable_defence()`) or cut the unit buttons; (2) delete `mati`, `pedrogao`,
`rhodes` and keep `spotorno` on disk as a test fixture, then port the finding pins;
(3) agree the cost scale (tariffs are placeholders; follower beats always/never by
only 4-8 % because a general evacuation is priced high).

## 1. Goal

A student walks up cold and, within 30 s, is making decisions as an incident
commander in a small stylised town, watching individual families respond to a fire.
A session is **3–5 min**, ends on a clear outcome, and invites a second try.
Success: a bystander says unprompted *"the fire goes where the wind blows, people
need warning early, but not too early, and my decisions changed who got out."*

**The branch's purpose is a clean demo game.** Everything that is not the demo is
deleted, not kept alive behind a flag (model-side deletion: gameplay §3; game-side:
presentation §2).

## 2. Decisions and non-goals

Settled with Mirko:
- Hardware: capable desktop, mouse only, no touch, **no speakers** (captions only).
  Audience: students; tone clear, not childish; loss framing never dwells on harm.
- **Setting: Roman region.** Fictional Lazio-flavoured towns (placeholders: Rocca
  Ventosa, Due Casali, Porto Pineta; native-speaker review needed).
- **Graphics target: the *Link's Awakening* (2019) diorama look** — toy miniature on
  a plinth, soft pastel palette, strong tilt-shift, soft shadows, 30 fps on modest
  hardware (presentation §4).
- **Visual identity: CIMA Foundation** — navy `#001E31`, blue `#004070`, orange
  `#DD7500`, pale grey `#D4DBDE`, white mark `assets/brand/cima_logo_white.png`.
- Facts + counterfactual on the end card; **no numeric score**. Money is a *cost
  shown*, not a score.
- Air tankers (Canadair ×2) are exposed.
- **LLM bubbles: OpenRouter** (network at the gazebo; canned fallback mandatory).
- **The model may be changed** (experimental branch, no back-compat), except that
  changing a number published in `CLAUDE.md` is flagged to Mirko. `cargo test
  --release` stays green.
- **No day/night cycle.**

Non-goals: real places or historical fires (**no `spotorno`, `mati`, `pedrogao`,
`rhodes`**); zoning/budgets beyond intervention cost; web build; debrief; control API
in the kiosk; persistent leaderboard.

## 3. Ownership and the contract

**Files.** Gameplay: `crates/{demo,abm,fire,scenario,behavior,chat}`, `data/`,
`scripts/`. Presentation: `crates/game`, `assets/`, shaders, `strings_it.rs`.
Neither edits the other's tree. A change needed across the line is a request in the
other spec, not a drive-by edit.

**Contract.** The only interface is the `demo::` types listed in the table in
`demo-spec-gameplay.md` §1 (`Outcome`, `Forecast`, `Order`, `Event`, `cost::Ledger`,
`trust`, `Refusal`, …). Gameplay delivers a field and a row in that table;
presentation renders it. The UI never computes game logic, and the model never
formats Italian. `Sim::advance` and `demo::Run` stay the one stepping path, so the
live game and the headless twin cannot drift.

**Working in parallel.** Use one git worktree/branch per agent
(`isolation: "worktree"`), rebase on the shared branch often; `docs/` and
`strings_it.rs` are the likeliest conflicts — edit only your own spec file. Gameplay
announces an interface change by editing the §1 table first.

## 4. Milestones (remaining)

| Step | Deliverable | Gate | Owner |
|---|---|---|---|
| 1 | Clean-up to a demo-only repo, CLAUDE.md rewritten | `cargo test --release` green, acceptance 8 | game side ✅, model side 🔲 (blocked on decision 2) |
| 2 ✅ | Small fire + draw + forecast card; playtest UI fixes | done | both |
| 2b 🔶 | **Decide and build unit effectiveness** (measured, awaiting decision 1); spot-fire events ✅; valle shift range ✅; borgo/porto shift floors 🔲 | units at T+3 beat T+30 by an asserted margin on every town | gameplay |
| 3 🔶 | Cost accounting ✅ + trust/anger ✅ in `demo`; wiring into the HUD and outcome card 🔲 | twin and live price identically; "evacuate always" and "never" both lose to the forecast-follower on average; cry-wolf test | gameplay → presentation slots |
| 4 🔶 | Diorama pass (mostly done; buildings, effects, overlays partial): light, plinth, tilt-shift, buildings, people, cars; overlays incl. forecast ghost | screenshot beside the reference; 30 fps measured; town legible at 2 m | presentation |
| 5 | Advisors, Italian review, operator doc, supervisor script, `KIOSK_SELFTEST` | **playtest with 3–5 new people**, nobody stuck > 60 s | both |
| 6 | Buffer, fixes only; LLM bubbles if time | | both |

Steps 2b/3 (gameplay) and 4 (presentation) run **in parallel**; only the HUD slots
for money/trust/events sync them.

Cut order if days slip: LLM bubbles, outline shader, `demo_porto`, advisors (plain
ticker instead), custom assets. **Never cut**: kiosk shell, idle reset, outcome +
counterfactual, the wind arrow, the forecast.

## 5. Acceptance

1. Cold start to PLAY ≤ 30 s with no staff explanation. — not timed; likely passes.
2. Full session 3–5 min, ends on the outcome card without an operator. ✅ (idle fix)
3. 60 s idle anywhere returns to ATTRACT with clean state — leaks unverified until
   `KIOSK_SELFTEST` (presentation §7). 🔲
4. All three towns: good play beats idle on families secure by the asserted margin ✅,
   **and** a needless evacuation costs trust and money ❌ (gameplay 5.1, 5.2).
5. A forecast is shown, right on average, wrong sometimes ✅ — and *useful* ❌
   (gameplay 5.3).
6. No single-key shortcut does anything; the operator corner does — shortcuts ✅ by
   construction; operator corner needs a mouse. 🔲
7. No English visible; no audio required. ✅
8. `cargo test --release` passes ✅; the repo contains no non-demo scenarios, UI or
   scripts ❌ (clean-up, both specs).
9. *(new, gameplay)* No fixed policy dominates: "always evacuate", "never" and "follow
   the forecast blindly" each lose to a better policy on average and win on some
   seeds (regret test, gameplay §0).
10. *(new, gameplay)* Units used early beat units used late and beat not using them.
11. *(new, presentation)* The town is legible at 2 m: houses, people, cars, unit
    markers, spot fires visible in a screenshot of every town; 30 fps measured.

## 6. Cross-cutting open questions

- Final town names and Italian copy; native-speaker review (presentation).
- Money costs and sources; unit-effectiveness option (gameplay, **Mirko's call**).
- Session seeding: random per visitor, seed of the day, or an operator switch —
  gameplay measures, presentation exposes the switch in the operator panel.
- Crate layout end state (presentation §2).

## 7. History

The pre-split single spec, including the full playtest write-up (§16, 37 findings,
the headline table), is in git at `07cc021` (`docs/demo-spec.md`). Findings were
distributed to the two specs above, each in its own "playtest findings owned here"
section.
