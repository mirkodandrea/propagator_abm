<!-- Codex blind playtest: commit d8c2b79; local test-only edits; frozen binary in /tmp/rv-resumed-playtest-round1 -->

Commit `d8c2b79` · seeds played: 762648, 998559 · three sessions

# Blind playtest, round 1

I read only brief.md, then played through player-visible ./play commands. No source, binary, saved state, parent directory, or stato inspection. First session persona: curious, impatient 16-year-old with no firefighting knowledge. Later sessions use only lessons learned on screen. All sessions completed through T+60.

## Sessions and think-aloud log

Every listed turn ends with `./play avanti`; orders below use `./play ordina`. Selections inspect the visible command menu without advancing time.

### Session 1 — nuova, South wind 38 km/h, initial forecast 65%

1. Selected K; ordered K (call), E1 4, P 1. “Big plane sounds strongest; call it now. Truck should go straight to the fire. Warn the biggest town too.” Preview for E1: “arriva in 5′ · strada troppo lontana: non ci arriva”. I left the order queued because I wanted to see what happens, but wondered how arrival and not arriving coexist. Result: truck withdrawn for heat, Borgo warned, nobody moving yet. Verdict: clear danger, unclear failure reason.
2. Selected P, E2, S; ordered P 2, E2 2, S 1. “Okay, trucks protect houses instead. Coste is nearest, send the truck there; walkers protect the biggest place.” Exact helpful/pointless labels made choices easy. Result: Borgo 26 safe, Coste warned, S still travelling. Verdict: satisfying evidence of evacuation, slow squad now understandable.
3. Ordered P 3, E1 1, E3 2. “Warn everyone! Extra truck just appeared, use it on the close town.” Result: wind turns East, Coste down to 300 m. Why is S listed under “qui” while still travelling? Verdict: mounting threat is engaging, reinforcements lack introduction.
4. Selected K; ordered K 2. “Finally the plane. Menu says only Coste helps, so obviously Coste.” Result: Coste threatened at 150 m; trucks continue and refill without my orders. Verdict: useful but action is almost dictated by preview.
5. Ordered K 2 again. “Does plane keep dropping if I do nothing? It says one launch a turn, so repeat.” Result: 7 families blocked versus 9 without orders; 37 houses hit versus 68. “Hai fatto la differenza” feels good. Mulino warning judged useless; feedback says dangerous districts waited, although I already warned both. Borgo has 6 houses hit versus 4 without orders despite protection: unexplained. Verdict: modest win with confusing attribution.

### Session 2 — riprova, same fire

1. Ordered P 2, S 2, E1 1, E2 2, K. “Now I know Coste is the problem. Start slow walkers early, trucks protect both towns.” S preview changes from 16 protected houses to 35 immediately after adding E2, without explaining cooperation. Verdict: learning produces a stronger opening but numbers feel mysterious.
2. Ordered P 1. “Warn Borgo next; no need to alarm Mulino again.” Result: people move, S still travelling. Verdict: straightforward cleanup turn.
3. Ordered E3 2. “Reinforce Coste, keep everything else doing its job.” Result: wind turns as expected; troops arrive. Verdict: uncertainty reduced by replay knowledge.
4. Ordered K 2. “Use the plane where it says useful.” Result: Coste threatened, S and both trucks working; automatic refill visible. Verdict: no difficult choice left.
5. No orders. “Let's test whether the plane carries on by itself.” Result: 4 families blocked versus 9; 18 houses hit versus 68; all warning verdicts OK. Plane reports 5 launches anyway. Borgo again 6 houses hit versus 4 without orders. Verdict: meaningful improvement, but empty final turn and automation blur my contribution.

### Session 3 — nuova, different fire, South wind 32 km/h, initial forecast 55%

1. Read aiuto; ordered P 1, S 2, E1 1, E2 2, K. “Similar map but different weather. Biggest population first, pre-position protection in Coste.” Aiuto explains cancellation and eight-minute turns. Verdict: familiar opening; new fire feels only slightly different.
2. Ordered P 2. “Coste gets warning now.” Result: East wind already by turn 3, much earlier than previous fire. Verdict: variation starts to matter.
3. Ordered E3 2; selected P. Menu calls Borgo and Coste “inutile adesso” and offers warning Mulino. “I learned not to warn Mulino, so patrol can stay still.” Result: no new patrol work, protection arrives. Verdict: deciding not to act is sensible but weakly explained.
4. Ordered K 2. “Plane still only useful at Coste.” Result: Coste 250 m, everyone else unchanged. Verdict: same optimal-looking action as previous sessions.
5. No orders. “All assigned; wait.” Result: 6 families blocked versus 6; 7 houses hit versus 47. Headline “Il fuoco è stato più veloce”. Coste is “Tardi — avvisati troppo tardi: non ha cambiato nulla per loro” despite visibly reaching safe counts of 18 by turn 5, and despite 40 fewer houses hit there. Verdict: saving 40 houses still feels branded a loss; unclear whether warning had any value.

## Designer review

| Criterion | Score | Evidence |
|---|---:|---|
| Usability | 3/5 | Short commands, queued orders, arrival times and resource descriptions work. Repeated large maps bury important changes. Exact confusing text: “arriva in 5′ · strada troppo lontana: non ci arriva”; “qui: S” alongside “in viaggio”; “un lancio d'acqua a turno” alongside final “5 lanci”. |
| Fun | 3/5 | Watching threat approach and improving replay results is rewarding. Final two turns repeatedly collapse to plane-to-Coste then wait. Exact labels “inutile adesso” and “protegge 8 case” make many decisions mechanical. |
| Interest | 4/5 | Wind forecast updates, travel time, warning order, spot fires and water offer a compelling emergency story. Confusing feedback “non ha cambiato nulla per loro” hides what happened to warned families. |

The first-session truck mistake is a good educational opportunity. The game supports learning by retrying and its without-orders comparison shows consequences. It needs more trustworthy explanations of those consequences. It gives exact protection numbers without explaining why they change, while the genuinely uncertain forecast is more interesting. The late game lacks enough meaningful decisions for five full turns.

## What the game taught me

I cannot just send a fire truck at the front of a fast fire and expect it to stop. Protecting houses near roads and warning people early are different jobs. The walking team needs a long lead time, and a plane must be called before it is needed. Wind can move the threat to a different neighborhood, so I should watch forecasts rather than warn the nearest place automatically. Fire can also jump ahead through sparks. Saving houses does not guarantee that every family gets out in time.

## Decisions that felt real, obvious, or fake

- Real: biggest population versus closest fire for the first patrol visit; committing slow S before the forecast resolves; protecting two districts with limited trucks; avoiding unnecessary alarm at Mulino after learning from the first finish.
- Obvious: call K immediately because no cost or alternative is shown; K to the sole target labelled useful; E3 to Coste; do nothing once all resources are working.
- Fake or under-explained: fire-head and flank targets remain prominently offered but are all described as ineffective; exactly known house counts feel like a puzzle hint; patrol feedback claims districts waited after both were already warned; protecting Borgo yields worse house damage than its baseline without explanation.

## Suggested changes, ranked

1. **Large impact:** make end feedback explain cause and metric precisely. Say whether “Tardi” means the number blocked did not improve, and distinguish this from houses saved and families evacuated. Explain increases versus baseline or avoid misleading comparisons.
2. **Large impact:** explicitly state persistent orders: trucks work/refill/return automatically; squad continues; plane keeps launching; warnings start evacuation after a delay. Show ongoing effects so a novice knows what needs another order.
3. **Medium impact:** align order previews with actual failure reasons. A road-inaccessible destination should not promise arrival and then attribute failure solely to heat.
4. **Medium impact:** separate useful action information from certainty. Explain protection as estimated/conditional, show cooperation when queued orders change numbers, and describe “inutile adesso” in plain causal terms.
5. **Medium impact:** improve late-turn choices with meaningful reassignment tradeoffs or shorten a session once only waiting remains.
6. **Medium impact:** make arriving reinforcements, refills, spot-fire locations and wind changes stand out in a concise change report above the map; reserve full repeated maps for material changes.
7. **Small impact:** say “in arrivo verso” rather than “qui” for travelling units; expand AIB on first mention; put the eight-minute-turn and cancellation instructions on the first screen.

## Reproducible bugs / player-visible inconsistencies

These are observable text issues; I make no claim about internal cause.

1. **Preview/result mismatch:** session 1, turn 1 `ordina E1 4`, then `avanti`. Preview: “arriva in 5′ · strada troppo lontana: non ci arriva”. Result: “si è ritirata: lì il calore era troppo forte.” Debrief: “col vento la testa non si ferma.” Three reasons for one failed order without reconciliation.
2. **Location/status mismatch:** session 2, `ordina S 2` on turn 1 then `avanti`. Turn 2 district says “qui: E2 S” while S says “in viaggio, arriva in 13′ · verso le Coste”. Session 1 likewise lists S at Borgo while still five minutes away.
3. **Unexplained adverse baseline:** first fire, session 2 order sequence above. Protected Borgo ends with 6 hit houses versus 4 without orders, also observed in session 1. This may have a valid explanation but the screen supplies none.
4. **Plane frequency ambiguity:** session 2 call K turn 1, target K 2 turn 4, no order turn 5. Visible description says “un lancio d'acqua a turno”, final says “5 lanci”. Continued activity through T+60 might explain it; the clock and ongoing rule need to be stated.
5. **Debrief attribution mismatch:** session 1 warnings P 1 turn 1, P 2 turn 2, P 3 turn 3. End says “La pattuglia si è fermata al Mulino, dove il fuoco non andava: intanto i quartieri in pericolo aspettavano.” Both endangered neighborhoods were already warned, and menus labelled repeat visits useless.

No crashes, command failures, or save failures observed. New fire visibly changed wind speed, forecast and outcome; riprova preserved the original fire. This report logs every played turn and all submitted orders; raw terminal transcripts are in the tool conversation, not inspected from files.


## Triage

Lead review after the first blind round; material game-design proposals remain
for Mirko to review. Six of seven intended lessons appear unprompted (the
cost of an early unnecessary patrol stop is not stated). Scores miss the
milestone-2 gate: usability 3/5, fun 3/5, interest 4/5.

| Suggestion | Decision | Reason / next step |
|---|---|---|
| Precise verdict causes and metrics | Accept factual text fixes; measure headline changes first | A late stamp cannot claim no effect in every case. Show caught families per district. Rewarding homes-only improvement changes verdict policy and remains open. |
| Explain persistent orders | Accept → playtest spec §1.3 | Orders continue between turns and during the finale. State that at the opening and in the finale. |
| Align arrival and failure previews | Accept wording fix; measure risk previews first | ETA is to the nearest reachable road; a head target can remain beyond the hose and the unit can later withdraw. Do not promise that all failures share one cause. |
| Replace certainty with estimates / cooperation | Measure first | Protection counts come from a concrete post; queued orders change how posts are allocated. Do not relabel exact coverage as an estimate without a defined uncertainty rule. |
| Add late-turn tradeoffs or shorten sessions | Measure first; Mirko decision | This changes the five-turn contract and balance. Preserve the existing loop for the next wording round. |
| Prioritise change reports over repeated maps | Measure first | Current report is ranked and capped at three lines; missed reinforcements may require a separate notification area. |
| Fix travelling labels and explain AIB / controls | Accept → playtest spec §1.3 | District lists show assignments, not physical positions. Add cancellation and turn length to the opening; expand AIB. |

The +2 homes hit at Borgo against no orders is a reported per-seed outcome.
Intervention can change the stochastic fire path; the cause of this specific
increase has not yet been isolated. Keep the paired
comparison; investigate sensitivity before claiming every individual district
must improve. The corrected 40-seed crew sweep and overall balance are recorded
in `demo-spec-gameplay.md`.
