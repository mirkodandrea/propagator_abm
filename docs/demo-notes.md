# Demo — notes for the next pass

Written 2026-10-06, while the first kiosk loop was being built. Companion to
`demo-spec.md`; this file is what the spec does *not* yet say.

## 1. The big gap: it is buttons, not a game

The first loop (attract → briefing → play → outcome → compare) works, but play
is "press Evacuazione, press a few unit buttons, watch". Mirko's feedback: it
should feel like **real gameplay**, with **tradeoffs**, and be **more
interactive** than an action bar. To be improved later; do not polish the
current shell before this is decided, because the answer changes the screen.

### 1.1 Tradeoffs the model already has but the UI hides

An order should cost something, or pressing it at T+0 is always right. Candidates,
in order of how little new model they need:

| Choice | The cost that is *already* in the model | What is missing |
|---|---|---|
| Evacuate early vs late | Cars take the only exit: a queue forms (`abm::traffic`), and a road crossing the fire's path is a trap (`porto`). Evacuating everyone at T+0 on `demo_porto` puts the town on the road while the road is the thing that burns. | The player never sees the queue; no map overlay. |
| Evacuate everyone vs a zone | Orders are all-or-nothing in the demo. `order_evacuation_all` vs the 2 km radius order the workbench has. A zone order warns fewer people and spares the rest a needless flight. | Zone order tool (click + radius) in kiosk. |
| Crew vs engine vs aircraft | Crews cut permanent line but at 120 m/h; engines need a road and run dry in 6 min; aircraft take 25 min to arrive and a drop is lost to the ember shadow (finding 41). | Tradeoff is real but unreadable: nothing on screen says *why* a unit refused or withdrew. |
| Defend the line vs protect people | Units and civilians are separate resources; a crew sent at the head is a crew not holding the road the town leaves by. | Needs a **road-hold** task (keep this road passable) — currently units only suppress fuel. |
| Cry wolf | Finding 42 / open question: the model has no memory of an earlier order. A *needless* evacuation should lower trust in the next one. | New mechanism (must be provably inert, finding 34). |
| Order too early costs real things | Evacuees leave homes undefended; households that would `stay_defend` lose their garden defence. | Surface it: "famiglie che avrebbero difeso la casa". |

Proposal: introduce **one scarce, visible resource** so every action trades off —
e.g. *attention/radio time* (one order every N sim-minutes), or *credibility*
(each needless evacuation lowers `trust_authority` of the next order). Credibility
is the one that teaches the real lesson (warning fatigue) and is data-driven.

### 1.2 What "interactive" could mean

Ideas, not decisions. Each is a replacement for "press a button":

1. **Draw, don't press.** Drag a line on the map: a fire line for a crew, a road
   closure, an evacuation zone polygon. The preview ring/route already exists
   (`command::target_preview`).
2. **Tasks the player *watches and corrects***: units visibly drive, get stuck,
   withdraw; the player re-routes them. Needs unit status chips on the map and
   click-to-select units (the removed workbench deliberately had none: three tools
   contend for click).
3. **Things that ask for a decision**: a radio call ("Il Sindaco chiede: apro la
   strada per il paese vicino?") — the scripted decision pauses of spec §7.4, but
   generated from the model's real events (road cut, spot fire, mast burnt).
4. **Individuals**: tap a family to see what it knows and decide for *it*
   (knock on the door / call). The LLM bubbles (spec §7.9) belong here: the
   interaction is *talking to people*, and they answer from their own senses only
   (finding 30).
5. **Prediction**: before the wind shifts, let the player place a marker where
   they think the head will be in 20 min; show the real front afterwards. A cheap
   way to make "the fire goes where the wind blows" something they *do*.
6. **Time as a verb**: scrub/rewind the last minute after the outcome ("rimetti
   il tempo a 20 minuti fa e scegli di nuovo") — the headless twin already makes
   this free because a run is deterministic (`demo::Run`).

### 1.3 Smallest slice that would change the feel

- a zone evacuation order drawn on the map (tradeoff: who you warn);
- a road-hold task for crews (tradeoff: fire vs exit);
- credibility as the visible cost of a needless order;
- unit chips on the map with one-sentence status ("in attesa, strada tagliata").

## 2. State of the first loop (what exists)

- `crates/game/src/kiosk/`: state machine (Attract / Briefing / Play / Outcome /
  Compare), idle reset (60 s; 90 s + 30 s in play), operator corner (hold
  top-right 3 s), fixed 6 s steps shared with `demo::Run` so COMPARE is the same
  fire, all strings in `strings_it.rs`, camera limits in `view.rs`.
- Run with `cargo run --release -p game` (`KIOSK_WINDOWED=1` for a window,
  `KIOSK_TOWN=borgo|valle|porto`, `KIOSK_PLAY_S=<s>` to shorten a session,
  `KIOSK_SHOT=<dir>` to walk one session and photograph each screen).
- No keyboard shortcut is registered and `UiFocus::keyboard` is held true, so none
  can fire (finding 25 by construction).
- Visual language follows CIMA Foundation (cimafoundation.org): navy `#001E31`,
  blue `#004070`, orange `#DD7500`, pale grey `#D4DBDE`; the white mark is
  `assets/brand/cima_logo_white.png`, embedded.

## 3. Findings from this session

- **`homes_lost` was `alight`, and `alight` saturates.** Firebrands reach 2.5 km, so
  every house in a 4 km town ignites whatever the player does (borgo 250/250,
  porto 251/350). The card now counts households with burnt ground within 150 m
  (`demo::run::LOST_RADIUS_M`, measured in `crates/demo/tests/lost.rs`): 54 / 15 /
  47 on borgo / valle / porto, graded and responsive to suppression. Labelled
  "case colpite" (hit by fire), not "perse": it is not the same thing as lost.
  An order never changes it — evacuation saves people, only suppression saves
  houses, which is itself a lesson worth showing.
- Compare on `demo_borgo`, order at T+0 vs none: 232/250 vs 216/250 safe, caught
  at home 18 vs 33 — the outcome card works, the margin is modest.
- The play camera framed the fire with the town hidden behind the HUD; the home
  focus now leans toward the town.

## 4. Known rough edges (not yet done)

The prioritised, evidence-backed list is `demo-spec.md` §16 (playtest 2026-10-06);
the items below are the older, shorter list.


- Nothing on the map yet for: wind arrow *on the terrain*, spot-fire rings,
  evacuation chevrons, queues, closed roads, selection ring (spec §7.6).
- Town switching reloads the scenario (a visible hitch on the way back to
  attract); consider keeping all three loaded.
- Briefing card covers the town; the fly-in is the only thing showing it.
- Advisors, scripted decision pauses, NPC bubbles, audio (cut), blender assets,
  lighting pass, `scripts/run_demo.sh`, operator doc: not started.
- `KIOSK_SHOT` screenshots are taken from the render, UI included; the first
  frame of a phase can show the previous phase's UI (wait ≥1 s).
- Counterfactual runs once per session (town + seed) on a thread (~1 s); Compare shows a
  spinner until it lands.
- The towns' names are placeholders in Roman-region style ("Rocca Ventosa", "Due
  Casali", "Porto Pineta"); copy needs a native-speaker read.
