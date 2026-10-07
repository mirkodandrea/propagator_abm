<!-- Fresh Codex blind playtest; commit c68bd76; binary sha256 a09189ced8643eb5432819f14022f073ef2242f2b84750e2ffaeb27cb083f2d6; sandbox /tmp/rv-codex-playtest-eslii534 -->

Commit `c68bd76` · seeds played: 656166, 261793 · three sessions

# Blind player evaluation

Played only from brief.md and player-visible commands. No source, specifications, saved state, binary inspection, `stato`, other reports, web or agents used. Persona during play: curious, impatient 16-year-old, no firefighting knowledge. Three complete sessions, including one retry and a newly generated fire. Scores and conclusions below are designer judgments after playing, not operational firefighting advice.

## Scores

| Dimension | Score / 5 | Reason |
|---|---:|---|
| Usability | 3 | Command syntax and choosing resources are easy to learn; persistent orders help. Every choice reprints a tall, visually dense ASCII screen, while several important reasons are absent. Distance/status can suggest danger where the ending says no danger existed. |
| Fun | 3 | Quick turns, visible losses and replay comparisons provide stakes. A real wind shift made me adapt. Most choices become dispatch-and-wait; calling the plane immediately has no visible downside, and all fire-edge targets were advertised as ineffective. |
| Interest | 4 | The distinction between saving families and saving houses is compelling. Retry produced fewer damaged houses but more trapped families, which made me want to understand priority and timing. Opaque predictions and debrief wording weaken that interest. |

Exact confusing screen quotes:

- Initial Mulino: `38 famiglie · fuoco a 800 m · attenzione · non avvisato`; ending: `[!] Allarme inutile — avvisati, ma niente indicava che il fuoco andasse lì`. “Attenzione” and proximity initially looked like an indication.
- After warning Borgo: `avvisato: 0 in viaggio, 0 al sicuro`. No reason or departure delay appears.
- At T+32 in the first two sessions: `il vento potrebbe girare e soffiare da Est (possibile, 30 %), tra T+12 e T+26`. The forecast window is already past.
- Session 3, truck preview: `si ferma sulla strada: bersaglio fuori portata`; next screen: `al lavoro: Testa del fuoco · acqua 46 %`. I could not tell whether it was doing anything.
- Session 3 at T+32: Borgo `MINACCIATO`, but plane preview `[1] Il Borgo — arriva in 2′ · inutile adesso`. There is no explanation of why distance/status and usefulness disagree.
- Session 3 ending: `Canadair chiamato tardi: non ha fatto in tempo a lanciare.` Earlier the plane was due at turn 5, one minute away, and an order promised arrival in two minutes.

## Sessions: exact command order and per-turn outcomes

All listed commands were prefixed exactly with `./play`. Each turn ends with one `./play avanti`; no omitted orders. Numeric targets: 1 Borgo, 2 Coste, 3 Mulino, 4 fire head. Distances below are Borgo/Coste/Mulino, in metres. Safe/travelling counts are families. Initial map and settlement populations remain 148/64/38 in all three games.

### Session 1: first encounter, 37 km/h south wind, initial 55% east-wind forecast

Opening command sequence: `./play nuova`, `./play aiuto`, `./play scegli E1`, `./play scegli P`, `./play scegli K`, `./play scegli S`.

| Turn | Exact further command order | Outcome after advancing |
|---|---|---|
| 1 | `./play ordina P 1`; `./play ordina E1 1`; `./play ordina E2 2`; `./play ordina S 1`; `./play ordina K`; `./play avanti` | T+08: distances 750/550/750. Borgo warned, no families moving/safe. E1 at Borgo 100% water, E2 at Coste 56%, S seven minutes from Borgo, K 17 minutes away. Forecast downgraded to 30%. |
| 2 | `./play ordina P 2`; `./play avanti` | T+16: distances 550/500/700. Borgo 14 travelling/23 safe, Coste warned 0/0. S working at Borgo. E3 appears free without a reinforcement report. E2 water 79%. |
| 3 | `./play ordina P 3`; `./play ordina E3 3`; `./play avanti` | E3 order said `inutile adesso`. T+24: distances 250/450/650; Borgo MINACCIATO, 16 travelling/39 safe; Coste 5/7; Mulino warned 0/0. E3 working at Mulino 16% water. One new spot fire. K one minute away. |
| 4 | `./play scegli K`; `./play ordina K 1`; `./play avanti` | Plane preview protects 12 Borgo houses. T+32: distances 100/350/600; Borgo MINACCIATO, 35 travelling/74 safe; Coste 6/9; Mulino 1/1. K at Borgo. E2/E3 refilling; E1 remains 100%. Two new spot fires. |
| 5 | `./play avanti` | No new orders; automatic continuation to T+60. 10 families blocked vs 18 without orders; 36 houses hit vs 76. Borgo alert In tempo, Coste In tempo, Mulino Allarme inutile. Canadair called in time: five drops. All house losses and blocked families were Borgo. |

### Session 2: same fire using retry, more house protection, different warning priority

Opening: `./play riprova`. Same initial weather/map as session 1.

| Turn | Exact command order | Outcome after advancing |
|---|---|---|
| 1 | `./play ordina P 2`; `./play ordina E1 1`; `./play ordina E2 1`; `./play ordina S 1`; `./play ordina K`; `./play avanti` | T+08 distances 750/550/750; Coste warned 0 travelling/0 safe, Borgo unwarned. E1/E2 working Borgo with 100%/26% water. S four minutes away; K 17 minutes away. Orders previewed 32, 14 and 36 houses defended respectively. |
| 2 | `./play ordina P 1`; `./play avanti` | T+16 distances 550/500/700. Borgo warned, 14 travelling/23 safe; Coste 11/1. E3 appears. S working Borgo, E2 water 55%. |
| 3 | `./play ordina E3 1`; `./play avanti` | Preview 18 houses defended. T+24 distances 250/450/650; Borgo MINACCIATO, 14 travelling/39 safe, four ground resources assigned; Coste 5/9. E3 water 23%, E2 83%. K one minute away. |
| 4 | `./play ordina K 1`; `./play avanti` | T+32 distances 100/350/600. Borgo 18 travelling/57 safe; Coste 16/11. E2 refilling, E3 working 49%. Forecast still predicts T+12–26. |
| 5 | `./play avanti` | T+60: 18 families blocked vs 18 baseline; 18 houses hit vs 76. Borgo Tardi, Coste In tempo, Mulino Giusto non avvisare. Five Canadair drops; debrief says trucks at Borgo prevented 58 house hits. More protected houses did not compensate for warning Borgo later. |

### Session 3: new fire, initial south 36 km/h and 40% forecast, actual east wind shift

Opening: `./play nuova`. Same place/map, distinct weather: 36 rather than 37 km/h, initial forecast 40% rather than 55%. Later spot fires, wind shift and baseline outcomes differ, confirming a different incident through visible information.

| Turn | Exact command order | Outcome after advancing |
|---|---|---|
| 1 | `./play ordina P 1`; `./play ordina E1 4`; `./play ordina E2 1`; `./play ordina S 2`; `./play avanti` | E1 fire-head preview explicitly outside range; tried anyway out of curiosity. T+08 distances 750/550/750; Borgo warned 0/0. E1 `al lavoro: Testa del fuoco`, 46% water; E2 Borgo 100%; S 14 minutes from Coste. Updated forecast: east shift 80%, T+10–22. |
| 2 | `./play ordina P 2`; `./play ordina E1 2`; `./play ordina K`; `./play avanti` | E1 arrival five minutes, 23 houses defended; K arrival turn 5. T+16 distances 450/400/650. Borgo 11 travelling/26 safe; Coste warned 0/0. S five minutes away, E3 appears, K 17 minutes away. Two spot fires. |
| 3 | `./play ordina E3 1`; `./play avanti` | Preview 14 houses defended. T+24 wind now east 45 km/h, fire pushed west. Distances 350/200/650. Coste MINACCIATO, 11 travelling/7 safe; Borgo 17/43. S working Coste. K nine minutes away; fire flanks renamed south/north. |
| 4 | `./play scegli E3`; `./play ordina E3 2`; `./play avanti` | E3 Borgo preview `inutile adesso`, Coste 12 houses, so relocated. T+32 distances 250/100/650: Borgo MINACCIATO, Coste RAGGIUNTO. Borgo 38 travelling/74 safe; Coste 13/19. E3 Coste 88% water; E1 46%, E2 100%. K one minute away. Three spot fires. |
| 5 | `./play scegli K`; `./play ordina K 2`; `./play avanti` | K preview: Borgo useless, Coste eight houses protected, arrival two minutes. T+60 ending: 14 families blocked vs 17; 39 houses hit vs 78. Borgo 12 houses vs 14 and no trapped families; Coste 27 houses vs 64, 14 trapped vs 17. Borgo In tempo, Coste Tardi, Mulino Giusto non avvisare. Debrief says plane had no time to drop, despite above timeline. |

## What I learned, unprompted after play

Avvisare le famiglie presto può contare più di mandare tanti camion. Salvare case e mettere al sicuro le persone sono due risultati diversi. Il vento dice dove il fuoco potrebbe andare, e una previsione può cambiare prima che cambi davvero il vento. I camion lavorano dalle strade e la squadra a piedi ci mette più tempo ad arrivare. Non conviene allarmare tutti soltanto perché c’è un incendio vicino. L’aereo va chiamato subito e sembra servire vicino alle case, non sulla testa del fuoco.

## Decisions: real, obvious, fake

- Real: limited single patrol and whether to warn Borgo or nearer Coste first; a retry exposed consequences for families. Reallocating E3 when wind actually shifted west; trading concentration at Borgo against preparing Coste. Slow team travel makes early deployment consequential.
- Obvious: plane call immediately, because no cost, scarce slot, risk or downside was shown. Choose a neighborhood with a positive houses-defended preview over `inutile adesso`. Once warnings and dispatches were done, final turn was frequently just `avanti`.
- Fake or decorative: fire head/flanks appear as prominent targets but every resource preview I saw made them useless, out of range or unsafe. I tried the head anyway and it consumed water without a clear visible action. Map can be used for direction, but precise menus usually solve targeting more directly than studying it.
- Uncertain: warning Mulino sounded prudent to a novice because it was always labelled attention and nearby. Ending judged it unnecessary but showed no tangible harm or tradeoff from that alert. Was it a mistake, or merely a lesson the debrief wanted to give?

## Ranked suggestions (maximum seven)

1. **Large impact:** Align the warning-risk display with the final alert judgment. Say `fuori dalla direzione prevista`, `rischio attuale`, `rischio se il vento gira` instead of calling every neighborhood attention. Explain why a forecast justifies warning one place and not another.
2. **Large impact:** Make every debrief claim agree with the visible timeline and outcomes. “Tardi” should identify actual warning/fire/departure times; plane feedback should show arrival and actual drops. In session 3 Coste had fewer blocked families than baseline yet received wording that includes `senza riduzione`.
3. **Medium impact:** Show compact per-turn changes and queue summary. Preserve the full map on request, but avoid printing the entire tall screen after every dispatch; announce E3 as a reinforcement. The interesting part should not require scrolling through repeated scenery.
4. **Medium impact:** Explain effectiveness previews briefly: houses currently exposed, protection already assigned, reason `inutile adesso`, time horizon, and whether the number is additional protection. With this I could reason rather than just select the biggest menu number.
5. **Medium impact:** Give meaningful intermediate reports: warning departures, waiting reasons, houses protected/hit, truck water consumption/refill and why a resource is idle. `al lavoro` is misleading for an out-of-range fire-head assignment.
6. **Medium impact:** Make the plane call a meaningful choice or explicitly present it as a mandatory early checklist action. Current play provides no reason to delay, so learning comes from obeying a timing rule rather than weighing a tradeoff.
7. **Small impact:** Expire old forecasts visibly and resolve them with “window passed, no shift observed”; retain direction arrows/text. Consider reducing prominence of universally ineffective targets, or tell players why these choices exist.

## Reproducible bugs / visible inconsistencies

These are player-visible issues; implementation causes are unknown. Newly generated incidents cannot be guaranteed to reproduce the same weather using the allowed commands; `riprova` should reproduce the current incident.

1. **Stale future forecast (confirmed twice on same incident):** On the 37 km/h, 55%-initial incident, advance to T+32 via session 1 or 2 command sequences. Screen still says wind `potrebbe girare` `tra T+12 e T+26`. Expected: forecast expires or states the shift did not occur in its window.
2. **Plane timeline/debrief conflict (observed, needs targeted repeat):** On the new 36 km/h / east-shift incident, call K in turn 2, choose/order K 2 in turn 5. It is one minute from arrival at T+32 and menu says arrival in two minutes; simulation runs through T+60. Ending says `non ha fatto in tempo a lanciare`. Cannot establish from player output whether no drops actually happened or just the feedback is wrong.
3. **Alert feedback wording inconsistent with numerical improvement (observed):** Session 3 Coste ending says `Tardi — avviso tardivo o senza riduzione delle famiglie bloccate` while trapped families fall from 17 to 14. It might truly be late by an unstated criterion, but neither timings nor criterion are shown. Combined alternatives obscure why it failed.
4. **Non-effective order labelled working (confirmed session 3):** Order E1 4 at opening, advance. Preview says outside range, status says `al lavoro: Testa del fuoco`, water falls to 46%, report says `Autobotte 1 è al lavoro.` Expected player feedback: outside range/ineffective, with reason for water use if intentional.

Not asserted as bugs: changing estimated team arrival/protection after other orders; different truck water levels; hidden reinforcement scheduling. They are usability uncertainties because visible screens do not explain them. No crashes or rejected valid command were encountered.

## Short think-aloud log, recorded each turn

### First session

1. “Ok, trucks save houses and patrol warns. Borgo has most families, Coste is nearer the fire. Send most help Borgo, one truck Coste, call plane now because 25 minutes is huge.” Confusion: initially expected trucks to extinguish the fire; selection explains house defense.
2. “Warned but zero safe—do they wait until the fire is closer? Warn Coste and keep others working.” Confusion: no evacuation delay explanation.
3. “A third truck appeared without announcement. Send it and patrol Mulino.” Confusion: cannot see whether fire is being fought, only approaching.
4. “Borgo is threatened: plane there. Why has useless Mulino truck used nearly all its water?” Confusion: preview versus ongoing work.
5. “Everyone has a job, leave it. Forecast still says T+12–26 at time 32—is it expired?” End: improved result feels worthwhile, Mulino scolding was unexpected.

### Retry

1. “I know Borgo gets hit. Send two trucks and team there, warn nearer Coste first, call plane immediately.” Confusion: no visible cost to plane call.
2. “Second truck lost three quarters of its water, first none—are these different? Warn Borgo now.” Confusion: resource differences and consumption not explained.
3. “Don't waste alert on Mulino. Third truck Borgo; three should be better, right?” Intention: improve house result.
4. “Plane Borgo again; no other sensible choice.” Confusion: little remaining agency.
5. “Wait for ending. Fewer safe at Borgo than before—warning later may be wrong despite more trucks.” End: learning that protecting houses does not substitute for timely warning; headline defeat underplays 58 saved houses.

### New incident

1. “New fire but identical map, slightly different numbers. Try stopping the head directly; warn Borgo first.” Confusion: why offer prominently a target all previews dislike?
2. “Outside range but working and water halved? Move truck Coste. East wind forecast is now 80%, feels urgent. Call plane late to see if still useful.” Confusion: status doesn't confirm ineffective order.
3. “No Mulino alert. Third truck Borgo; cover both places. Wait for wind.” Confusion: actual total protection remains unclear.
4. “Wind really turned! Coste threatened and Borgo truck now useless: move it. This feels like a real decision.” Positive interest: forecast changes lead to meaningful response.
5. “Borgo threatened but plane useless? Pick Coste because menu says eight protected. Want to know why, not just trust menu.” End: plane apparently never dropped despite expected arrival, and Coste feedback says late despite some family improvement.


## Lead triage

The tester received only the player brief and compiled game, with no repository
context or earlier reports. Scores remain usability 3/5, fun 3/5, interest 4/5;
the playtest acceptance gate is still unmet. This run changes no gameplay or
presentation code.

Priorities for the next implementation pass:

1. Verify and expire forecasts when their stated time window has passed.
2. Reproduce the aircraft case using the recorded third-session seed and
   orders. Distinguish late arrival, unsafe flight, and failure to reach a drop
   target before writing the debrief; the report establishes a visible
   contradiction, not its internal cause.
3. Show the actual reason for each warning stamp and distinguish proximity
   from forecast-supported risk, so a nearby upwind district does not look
   like a justified warning that is later punished.
4. Explain ineffective work and water consumption, and announce reinforcement
   arrivals. These should come from model facts shared by both front ends.
5. Review compact change reports, passive late turns and the cost-free early
   plane call as gameplay/presentation decisions before changing rules.

The user requested a Codex agent playtest; no Claude process was used. The
Claude builder worktree was the source of previously recovered commits, not
this tester's working directory. This test used a fresh temporary directory
outside the repository, with the main-workspace binary from `c68bd76`.
