# Demo — presentation spec (kiosk: screens, tray, map signals, shell)

Read `docs/demo-spec.md` first and `CLAUDE.md` (findings 11–13, 21–23, 25 bind
rendering and input). The model side is `docs/demo-spec-gameplay.md`.

Legend: ✅ done · 🔶 partly · 🔲 to do · ✂ cut-list item.

## 0. How this side works

**Owns:** `crates/game`, `assets/`, shaders, `scripts/build_town_models.py`,
`scripts/build_models.py`, `crates/text` (all player-facing Italian, shared with
`play`), the screenshot harness. **Never
computes game logic:** target validity, ETAs, previews, report lines, stamps
and notes come from `demo::` (gameplay §1).

**Method:** screenshot, read every PNG, fix, repeat. A correct overlay nobody
can see looks broken (finding 13); a back-facing mesh draws nothing (finding 11).

## 1. Screens

```
ATTRACT ─► TURN 1 ─► PLAY ─► REPORT ─► TURN 2 ─► … ─► TURN 5 ─► PLAY ─► FINALE ─► VERDICT
```

| Screen | Shows | Player does | Ends |
|---|---|---|---|
| **Attract** | the town, a fire playing at speed, title *"Rocca Ventosa brucia. Tocca a te."* | click anywhere | click |
| **Turn 1** (= briefing) | 4 s fly-in onto the town; a three-line card that fades on first interaction: *"Sei il comandante. Ogni turno: scegli una risorsa, poi un punto sulla mappa. Poi Avanti."* | token → target, *Avanti* | *Avanti* |
| **Turn 2–5** | the same layout, the report lines still pinned on the map | token → target, *Avanti* | *Avanti* |
| **Play** | the fire runs 8 simulated min in ~10 s; units drive, cars leave, nothing clickable | — | automatic |
| **Report** | ≤ 3 lines (gameplay §6), each with a pulse at its place on the map; portrait (Capo squadra VVF / Sindaco / Meteo) | — | 4 s, then the next turn opens |
| **Finale** | after turn 5 the fire runs T+40→T+60 at ~1 s per simulated min; each district gets its **stamp** as the verdict for it becomes final | — | automatic |
| **Verdict** | the card (§5) beside the burnt town | *Riprova* (same fire) | 60 s idle → Attract |

Timers: a turn auto-advances after **45 s with no input** (reset by any
interaction), so an abandoned kiosk keeps moving. 60 s idle on Verdict →
Attract. Session target 3–4 min.

## 2. Turn layout

```
┌──────────────────────────────────────────────────────────────────┐
│ ROCCA VENTOSA       Turno 2 di 5 · T+08       📻 Meteo: il vento  │
│                                                potrebbe girare    │
│        ┌──────────┐  ░░░ forecast cone          da Est (60 %)     │
│        │ LE COSTE │ ░░░░                                          │
│        │ 64 fam.  │                    ┌────────────┐             │
│        │ 1,4 km   │   ◎ Focolaio       │ IL BORGO   │             │
│        └──────────┘                    │ 148 fam.   │             │
│              ◯ fianco  🔥━━━▶ Testa ✕  │ 700 m  ✓🚓 │             │
│   ┌──────────┐            ◯ fianco     │ 🚒 al lavoro│            │
│   │IL MULINO │                         └────────────┘             │
│   │ 38 fam.  │       ⟹⟹  wind arrow on the ground                │
│   └──────────┘                                                    │
│                                                                   │
│ ┌────────────────────────────────────────────────────┐ ┌────────┐ │
│ │ 🚓 libera │ 📢 1 │ 🚒 al lavoro │ 🚒 libera │ 👷 │ ✈️ │ │AVANTI ▸│ │
│ └────────────────────────────────────────────────────┘ └────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

- **Top bar:** town, *Turno n di 5*, `T+mm`, the forecast in one line (pulses
  amber the turn issue 2 lands).
- **District chips** are **information only** — no buttons. Name, families,
  distance to the fire, status colour (calm / watch / threatened, pulsing /
  reached), and badges for what is there: ✓🚓 warned (with a fill of families
  left), 🚒/👷 units posted.
- **Token tray** (bottom): one card per token — icon, name, state badge
  (*libera*, *in viaggio 3′*, *al lavoro*, *rifornimento*, *ritirata*, *in
  arrivo turno 4*, *non chiamato*, *usato*), water gauge on engines. The
  reinforcement card slides in at turn 3.
- **Avanti** (bottom-right), always enabled. Giving no orders is a legal turn.

## 3. The interaction: token → target

1. Click a token card → it lifts; the map dims slightly; **only valid targets
   light up**, each with its preview on a tag: ETA and effect in words
   (*"arriva in 4′ · difende 60 case"*, *"Testa: si ritirerà"*, *"Avvisa 148
   famiglie"*, *"Arriva al turno 4"*).
2. Click a target → a dashed route line from the unit to the target, the card
   shows the pending order. Click the card again to cancel before *Avanti*.
3. IT-alert has no target: clicking it shows a confirm tag over the whole town
   (*"Avvisa tutti, anche chi non serve"*) — click again to send.
4. Canadair: first use is the call (*Cielo*); once on station it targets like
   the others.

Hover on a target shows its preview; there is nothing to drag, type or
scroll. Camera: frames the whole town and fire at every turn; pan/zoom stay
available within the `view.rs` limits but are never needed.

## 4. Signals on the map

| Signal | Form | Why |
|---|---|---|
| Wind | a **large arrow on the terrain** through the fire, length ∝ speed | lesson 1 at a glance; the compass column is gone |
| Forecast | a translucent **cone** from the fire toward the shift bearing, opacity ∝ probability; sharpens or fades at issue 2 | the odds as a place, not a percentage |
| Head / flanks | markers on the fire at `Head` / `Flank`; the head carries a red ✕ hint once any unit has withdrawn from it | targets and lesson 4 |
| Spot fires | ring + *Focolaio* label at each (`SpotFire` target) | |
| Units | kit vehicles on the roads (engines, crew van, police car), plane on station; state badge above each | the tray, on the map |
| Warned district | sky-blue ring; cars leaving toward the *area di attesa* | |
| Engine post | green ring of `DEFEND_REACH_M` | |
| Drop | a wet strip on the ground for `DROP_DEFENCE_S` | |
| Report line | a pulse at its `pos` / district for the report's 4 s | links words to places |

All markers obey the canopy rule (finding 13: ≥ +20 m) and the winding rule
(finding 11).

## 5. Verdict card

The camera slides the burnt town into the left half; the card on the right:

1. **Headline** from families caught: *"Tutti al sicuro!"* / *"Ottimo lavoro"* /
   *"Hai fatto la differenza"* / *"Il fuoco è stato più veloce"*.
2. Two facts, each against **senza ordini** (spinner, then *non disponibile*
   after 20 s): **famiglie in salvo** and **case colpite**.
3. **One row per district:** name, its stamp (✅ *In tempo* · ⚠️ *Tardi* · ❌ *Mai
   avvisati* · ✅ *Giusto non avvisare* · ✅ *Prudente* · ⚠️ *Allarme inutile*),
   homes hit vs without orders.
4. **≤ 3 notes** on the firefighting rules (gameplay §1 `Note`), broken first:
   *"Un'autobotte mandata sulla testa si è ritirata"*, *"Canadair chiamato in
   tempo: 3 lanci"*.
5. *Riprova* (same draw from turn 1).

No medals, no money, no score.

## 6. Town kit and look

Blender kit (`scripts/build_town_models.py` → `assets/models/town.json`),
landmarks, pastel houses, people and cars, house states from the books (a home
the referee counts as hit burns then chars; finding 43), vegetation, plinth,
tilt-shift: already built and reused. Still needed from the kit: a **police car**, a
**crew van**, a **Canadair** model and a hydrant marker (for *rifornimento*).

## 7. Existing kiosk UI to delete

The action bar (*Allerta generale*, *Autobotti*, *Canadair*, *Pausa*,
*Veloce*), the *Avvisa*/*Difendi* buttons on chips, the four counters, the
bill, medals, the compass column and forecast card, the advisor queue, the
town rotation and town selector (the kiosk loads Rocca Ventosa only). Delete
them; do not hide them.

## 8. Ops

- Idle: 45 s per turn (auto-*Avanti*), 60 s on Attract/Verdict.
- Operator corner (hold top-right 3 s): restart, pause, quit. No key does
  anything (finding 25).
- `KIOSK_SHOT=<dir>` walks a session as `forecast-player` (gameplay §8) and
  shoots: attract, turn 1 before/with a token selected/after orders, a play
  frame, a report, turn 3 with the reinforcement, finale, verdict.
- `KIOSK_SELFTEST=1`: state machine round trip, reset fan-out (finding 21), no
  key acts, idle reset leaks nothing.
- `scripts/run_demo.sh` supervisor ✅; operator one-pager to rewrite.

## 9. Frame budget

30 fps floor on the target machine. Disable in order: shadow resolution,
cascades, bloom quality, depth-of-field samples, MSAA→FXAA. Never disable: fire
glow, the wind arrow, the forecast cone, target markers, the tray.

## 10. Tests

- `the_live_session_and_its_headless_twin_agree` re-pinned on turn orders.
- Every `ReportLine` kind, `Stamp`, `Note`, `Effect` and `TokenState` has an
  Italian string (exhaustive match + a test).
- `models::tests` (kit winding) for the new vehicles.
- Real pointer: token select/cancel, every target kind, IT-alert confirm,
  auto-*Avanti*, operator corner.
