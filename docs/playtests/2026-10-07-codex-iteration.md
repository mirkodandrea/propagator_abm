<!-- Fresh blind Codex iteration test; commit 031b19e; binary sha256 2506674d05278f47a235ea17624dfa9a0f091a9f219cd2560fc3f080e2bd000b; sandbox /var/folders/l9/pb1d2bf13tl1tc0_swnk_44r0000gn/T/rv-codex-iteration-blind-p9w4152m -->

Commit `031b19e` · seeds: 94004, 94004, 345187 · three sessions

# Blind player test — three complete sessions

## Scope and perspective

Read only brief.md. Played only through ./play player commands, never ./play stato. No repository, source, binary, saved state, specifications, history, web, or delegation inspected. Initial player perspective: curious, impatient 16-year-old at a Protezione Civile stand, no firefighting knowledge. The brief supplied only nuova and aiuto. The comments below reproduce my brief think-aloud reactions during play; design judgments come after the three sessions. All sessions completed to the visible T+60 result.

## Session 1 — first nuova, Rocca Ventosa, South wind 30 km/h, initial East-change chance 30%

Exact commands in order, grouped by the displayed turn:

```text
Turn 1
./play nuova
./play scegli K
./play ordina K
./play ordina P 1
./play ordina E1 1
./play ordina E2 2
./play ordina S 1
./play avanti
Turn 2
./play ordina P 2
./play ordina E2 1
./play avanti
Turn 3
./play ordina P 3
./play ordina E3 1
./play avanti
Turn 4
./play ordina K 1
./play avanti
Turn 5
./play avanti
```

Think aloud by turn:

1. “Il Borgo ha più famiglie ed è nella direzione del fuoco. Mando lì tutti, e chiamo subito l’aereo. Ma il comando per chiamarlo non lo vedo.” I selected K to discover the call command; kept one engine at Le Coste as insurance.
2. “Adesso dice zero per cento: sposto l’altra autobotte al Borgo. La pattuglia può avvisare anche gli altri, tanto qui ha finito.”
3. “Il fuoco si è quasi fermato, bene. Anche il rinforzo al Borgo; voglio vedere arrivare il Canadair.”
4. “L’aereo lo metto sul Borgo. Però le Coste sono a quattrocento metri: ‘fuori direzione’ non significa proprio tranquillo?”
5. “Non cambio nulla, sono già tutti dove c’è il rischio. Voglio il risultato.”

Final visible verdict: **“Ottimo lavoro”**. Families blocked: **3 (without orders 15)**. Houses hit: **12 (without orders 40)**. Borgo: “[OK] In tempo — anticipo sufficiente”, 12 houses hit, 3 families blocked; warning T+02:54, first threat T+41:36, houses reached T+50:36. Le Coste: “[OK] In tempo — anticipo sufficiente”, zero houses/families; warning T+10:48, first threat T+40:54, houses never reached. Mulino: “[!] Allarme inutile — nessuna direzione prevista giustificava l'avviso”, zero houses/families; warning T+21:00. Canadair: **5 launches**, called T+00:00, arrival T+25:00, first target T+24:00, first launch T+25:30. Feedback credited early aircraft and “28 case in meno colpite”, criticized the Mulino warning.

Player verdict: satisfying visible improvement, but “Ottimo” felt a bit generous with three blocked families and twelve houses hit. I had thought extra warnings were harmless.

## Session 2 — riprova, same fire

```text
Turn 1
./play riprova
./play ordina K
./play ordina P 1
./play ordina E1 1
./play ordina E2 1
./play ordina S 1
./play avanti
Turn 2
./play ordina P 2
./play avanti
Turn 3
./play ordina E3 1
./play avanti
Turn 4
./play ordina K 4
./play avanti
Turn 5
./play ordina K 1
./play avanti
```

Think aloud by turn:

1. “Riprovo: ‘Ottimo’ ma dodici case colpite? Stavolta difendo subito tutto il Borgo e non disturbo il Mulino. E voglio provare cosa fa l’aereo sul fuoco vero.”
2. “Le Coste l’ultima volta avevano bisogno dell’avviso anche senza case bruciate, quindi le avviso ancora. Gli altri ordini li lascio.”
3. “Rinforzo al Borgo ancora. Le coperture stavolta sono diverse: non so come sceglie il posto dentro il paese.”
4. “Dice che sul fuoco non salva case, ma allora a cosa serve il bersaglio ‘Testa del fuoco’? Lo provo un turno.”
5. “Non pare aver cambiato niente. Lo rimetto sul paese e finisco.”

Final visible verdict: **“Ottimo lavoro”**. Families blocked: **3 (without orders 15)**. Houses hit: **8 (without orders 40)**. Borgo: “[OK] In tempo — anticipo sufficiente”, 8 houses, 3 families; warning T+02:54, threat T+43:48, houses reached T+51:36. Le Coste: same zero losses, warning T+10:48 and threat T+40:54. Mulino: “[OK] Giusto non avvisare — mai minacciato, nessun avviso necessario”. Canadair: **3 launches**, call/arrival/first target/first launch still T+00:00/T+25:00/T+24:00/T+25:30. Feedback: early Canadair, “32 case in meno colpite”.

Player verdict: improved houses and avoided unnecessary warning. I cannot attribute that improvement to the aircraft experiment: E2 and S deployment also differed. The aircraft fire-head turn looked like busy work; the screen said it had no house defence, and the nearest-fire distances were the same as session 1 at T+32. Curiosity became suspicion that those numbered fire targets are traps.

## Session 3 — nuova, different fire at same location

Visible difference: South wind **31 km/h**, initial East-change chance **40%**, later **20%** instead of zero. Distances and final outcome also differed. Same settlement/map labels.

```text
Turn 1
./play nuova
./play ordina K
./play ordina P 1
./play ordina E1 1
./play ordina E2 1
./play ordina S 2
./play avanti
Turn 2
./play ordina P 2
./play avanti
Turn 3
./play ordina E3 1
./play avanti
Turn 4
./play ordina K 1
./play avanti
Turn 5
./play ordina E2 2
./play avanti
```

Think aloud by turn:

1. “Nuovo incendio, stesso paese, quaranta per cento invece di trenta. Stavolta mando la squadra alle Coste come assicurazione, due autobotti al Borgo.”
2. “Venti per cento non è zero, perché dice fuori direzione? Lascio la squadra lì, avviso le Coste e aspetto.”
3. “Il rinforzo al Borgo, come prima. Non ho molto altro da decidere adesso.”
4. “Aereo al Borgo. Alle Coste il fuoco si avvicina più che al Borgo: io la squadra non la tolgo.”
5. “Ecco, le Coste sono minacciate davvero. Sposto E2 che tanto sta viaggiando, gli altri restano al Borgo.”

Final visible verdict: **“Hai fatto la differenza”**. Families blocked: **6 (without orders 8)**. Houses hit: **24 (without orders 48)**. Borgo: “[OK] In tempo — anticipo sufficiente”, 24 houses, 6 families; warning T+02:54, first threat T+29:36, houses reached T+32:48. Le Coste: “[OK] In tempo — anticipo sufficiente”, zero houses/families; warning T+10:48, threat T+28:30, houses never reached. Mulino: “[OK] Giusto non avvisare — mai minacciato, nessun avviso necessario”. Canadair: **5 launches**, same call/arrival/first target/first launch times as session 1. Feedback credited aircraft timing and “24 case in meno colpite”.

Player verdict: this fire felt more urgent and the last E2 decision felt real, but the final report did not tell me whether keeping S at Le Coste or moving E2 helped. Borgo was hit just after the last decision screen; seeing it jump from 100 m to the final result reduced agency at the climax.

## Design review: scores

| Dimension | Score | Visible basis |
| --- | --- | --- |
| Usability | **3/5** | Can finish without aiuto once K is selected. Clear resource roles, persistent orders, warning arrival, revised weather, and final comparisons. Too much repeated text on every order; the useful choice details are far below unchanged status. K calling syntax initially hidden. Coverage allocation and risk labels remain unclear. |
| Fun | **2/5** | First deployment and late threat were engaging. Many later turns were “send reinforcement to Borgo, wait for K, avanti”; no dramatic visual payoff or new action at the climax. Invalid-looking fire targets seem to exist mainly to punish curiosity. |
| Interest | **4/5** | Wanted to replay, test direct aerial attack, and compare a new fire. Weather uncertainty, travel delay, delayed family departure, and baseline loss comparison prompt useful questions. Curiosity weakens once the visible advice makes most choices obvious and the debrief cannot explain individual resource effects. |

## Exact confusing screen quotes

- “Sei il comandante: 5 turni da 8 minuti. Scegli una risorsa, poi un punto sulla mappa.” There is no coordinate/click mechanism described; I actually selected a numbered target through commands. Also five eight-minute turns sounds like T+40, while play finishes at T+60.
- “K va chiamato presto: arriva dopo 25 minuti; assegna il bersaglio quando è vicino.” The crucial call syntax is only exposed after selecting K. Once found, it works.
- “Copertura: case al posto assegnato, può sovrapporsi; non sono case salvate garantite.” Understand the warning, but what chooses the actual posto? S at Borgo was 22 houses in session 1 and 40 in the replay with different earlier deployments.
- “orizzonte: direzione attuale / finestra meteo; fuori dalla direzione prevista” Abstract vocabulary. In session 3 a 20% possible East wind existed while Le Coste was marked outside the predicted direction.
- “K prosegue su fronte del fuoco: fuori direzione vento prevista, valuta riassegnazione” For K assigned to the fire head, calling the FIRE FRONT outside the wind direction feels like the wrong explanation.
- “[4] Testa del fuoco — arriva in 2′ · non salva case: lì il fuoco non si ferma” Why offer the attack at all? I was hoping to stop the fire; the visible rule says that cannot happen there.
- “Canadair — aereo: va chiamato, arriva dopo 25 minuti; poi un lancio d'acqua a turno” versus “Il Canadair ripete i lanci sul bersaglio fino a T+60.” Five launches happened after assigning at turn 4 even though only two player turns remained. The continuation explains some of that, but the time unit of a launch cycle remains unclear.

## Lessons learned without prompting

Avvisare le famiglie non le mette subito al sicuro: alcune si preparano e altre devono ancora viaggiare. Conviene chiamare presto il Canadair perché può arrivare quando il paese è già vicino al fuoco. Il vento aiuta a decidere dove mandare i mezzi, ma ho visto un quartiere fuori dalla direzione prevista diventare minacciato lo stesso. Proteggere le case e attaccare direttamente le fiamme sono scelte diverse in questo gioco, e il secondo può non salvare nessuna casa. Non serve avvisare tutti automaticamente: il gioco mi ha segnalato l’allarme al Mulino come inutile.

These are lessons from visible behavior, not verified real-world firefighting claims.

## Real, obvious, and fake-feeling choices

**Real:** splitting S toward a possible wind change versus all Borgo; deciding whether to follow reassignment advice after a bulletin; moving E2 when Le Coste was visibly threatened; deciding whether an extra warning was justified after the first debrief. Travel delay matters and different placements produce visibly different coverage.

**Obvious:** call K at T+00 (explicit repeated instruction and no apparent competing cost), P to largest downwind Borgo first, available E3 to Borgo, aircraft to Borgo once assignment becomes available. Automatic refill means I never chose water logistics despite repeatedly seeing water percentages.

**Fake-feeling:** three prominently numbered fire targets all openly described as outside engine reach or unable to save houses. K at the fire head says “al lavoro” but offers no positive outcome. Repeating a warning is openly pointless. These can teach limits, but do not feel like interesting strategic alternatives. Initial freedom narrows quickly into following the listed warning labels.

## Ranked improvements (at most seven)

1. **Large impact — make visible actions consequential and explain them.** Either give fire-head/flank work an understandable conditional use, or frame it explicitly as an educational mistake with a clear reason and observed result. Replace generic “al lavoro” with what changed or failed to change.
2. **Large impact — prioritize the current decision.** After an order, show a short acknowledgement and queue; after avanti, show changed threats/weather/arrivals first. Put repeated full status and coverage caveats behind mostra or scelta details. At a stand I would otherwise skip the paragraphs.
3. **Large impact — show a readable moving map at the important moments.** Current ASCII starts with a wall of quotation marks and is not automatically shown after advancing. A compact map with movement/fire overlays would make resource effects and near threats tangible.
4. **Medium impact — reconcile risk language.** Say “fuori dal vento attuale; rischio meteo basso (20%)” when that is what the screen means. Put observed MINACCIATO before directional advice. A novice should not infer safety from “fuori direzione”.
5. **Medium impact — debrief the player's actual tradeoffs.** Explain S at Le Coste, E2 transfer, coverage overlap, and aircraft fire-head experiment, including when an action contributed nothing. Keep the good without-orders comparison, but connect the result to concrete decisions.
6. **Medium impact — expose the start and timeline in one short example.** Include “./play ordina K” immediately, “orders run only when avanti”, and “you command until T+40, then observe T+60”. Explain aerial repetition in minutes or distinguish controlled turns from autonomous continuation.
7. **Small impact — explain automatic position assignment and coverage.** One sentence saying how the game chooses a post and whether order sequence matters would make 22 versus 40 covered houses and 0 added Canadair houses easier to interpret.

## Reproducible apparent bugs / inconsistencies

No crash, rejected valid command, or proven simulation bug observed. The following are visible anomalies; I cannot establish internal causes.

1. **Launch reporting after a fire-head assignment appears inconsistent.** Reproduce session 2 exactly: K called turn 1; K 4 turn 4; avanti; K 1 turn 5; avanti. At T+32 K is “al lavoro: Testa del fuoco” with “lavoro sul fuoco: nessuna difesa di case”, no launch count shown. Final says “Canadair: lanci completati · 3 lanci” and “primo lancio: T+25:30”, though K moved to Borgo only at T+32. Could be different launch accounting rather than a malfunction; screen does not explain whether launches on fire count.
2. **Direction advice can contradict observed threat emphasis.** Reproduce session 3: at turn 5 Le Coste is “MINACCIATO”, S advice correctly says “fuori direzione vento; pericolo osservato, valuta copertura”, but ordering E2 2 summarizes only “arriva in 3′ · fuori direzione prevista”. A deeper detail adds the observed-threat caveat. The headline understates the actionable danger and makes equivalent advice appear inconsistent.
3. **Threshold/rounding wording ambiguity.** In session 3 turn 5 the report says “Il fuoco è a meno di 300 m dalle Coste.” The neighborhood line says “fuoco a 300 m · MINACCIATO”. Could be normal rounding, but no approximation marker indicates it.
4. **Formatting defect.** In resource choice text for Le Coste, the line beginning “orizzonte: direzione attuale / finestra meteo; rischio se il vento gira nella finestra prevista” starts flush left while adjacent explanation lines are indented. Reproduced on initial E1/S selections. Minor, but increases the wall-of-text feel.


## Lead triage and implementation checkpoint

This is a new Codex agent with no history or repository context. It played a
frozen binary containing the reviewed iteration from `031b19e`. The builder
worked in `/tmp/propagator-abm-playtest-iteration`, separate from the main
checkout; its commit `4006b91` was cherry-picked into the main branch.

The iteration delivered elapsed/observed forecasts, warning reasons with exact
arrival/threat/reach times, unsafe-flight history, wind direction distinct from
proximity, continuing work and coverage explanations, family preparation,
persistent E3 arrival notices, and compact screens without repeated full maps.
The aircraft regression from seed 261793 is a safety abort: called T+08,
arrived T+33, interrupted T+33:36. No flight scheduling defect was reproduced.

Validation: final `cargo test --release` passed 294 tests, with 60 existing
ignored measurement/external tests. The seven balance lessons and preview
consistency checks passed. An isolated full CLI replay with its counterfactual
took 513 ms. Independent CLI replays preserve all overall and per-district
family/home totals and drops in the three prior fixed-order sessions:

| Recorded trajectory | Families blocked, versus no orders | Homes hit, versus no orders | Drops |
|---|---|---|---|
| 656166 first | 10 / 18 | 36 / 76 | 5 |
| 656166 retry | 18 / 18 | 18 / 76 | 5 |
| 261793 shifted wind | 14 / 17 | 39 / 78 | 0 |

Fresh scores are usability **3/5**, fun **2/5**, interest **4/5**. Milestone 2
remains open. This is a different pair of fires and a different tester; the
lower fun score is not a controlled before/after estimate. It nevertheless
provides no evidence that the presentation iteration met the fun gate.
The tester states at least four intended lessons unprompted: warning/departure
are separate, aircraft needs early calling, wind guides deployment, protecting
homes differs from attacking flames, and warnings should not be automatic.
A further caution is the implication that outside-direction means safe: the
report correctly noticed observed threats outside the predicted path.

Suggested changes, in report order:

1. **Measure first — consequential fire targets.** A conditional suppression
   benefit would change the calibrated model and cannot be invented from this
   report. The current measurements show no home benefit for direct head/flank
   work. Educational framing and exact ongoing/drop facts are a bounded next
   iteration; the gameplay checkpoint records them separately from rebalancing.
2. **Accept as next presentation iteration — shorter order acknowledgement.**
   Removing maps did not sufficiently reduce repeated status/details. Specify
   acknowledgement plus pending queue after orders, with fuller inspection
   available through `mostra`/`scegli`. No stepping changes.
3. **Measure first — compact moving map.** The full map remains available via
   `mostra`; removing automatic map frames was a deliberate accepted choice.
   Evaluate a small turn-change map against summary text before restoring large
   frames. The kiosk migration and human stand timing are still untested.
4. **Accept as next factual iteration — low forecast probabilities and actual
   threat first.** Publish actual percentage/window even below the advice
   threshold; outside the advised path must not imply zero meteorological risk.
   Lead with observed threat in order headlines, not only deeper explanations.
5. **Measure first — resource-specific causal debrief.** A baseline with no
   orders proves aggregate improvement, not individual resource attribution.
   Do not present causal contribution without controlled resource ablations.
   Exact jobs, movement, drops and overlap can be explained as observed facts.
6. **Accept as next wording iteration — call command and timeline.** Show
   `./play ordina K` before selection, separate five controlled turns through
   T+40 from autonomous continuation to T+60, and describe repeated drops in
   their actual cycle unit rather than player turns.
7. **Accept as next factual wording iteration — automatic posts.** Describe the
   deterministic placement/allocation rule and order-sequence effects, without
   claiming coverage is guaranteed saved homes.

Apparent inconsistencies:

- Aircraft fire-head launches: the report shows a first drop before the later
  Borgo assignment. This alone is not a simulation bug: non-district fire
  targets can receive drops. Reproduce and expose drop count/standing job for
  these targets rather than leave the player guessing.
- Fire-front reassignment advice saying outside the wind direction is a
  presentation reason mismatch; use a job-specific reason for non-district
  targets. Actual district threat should also lead the short order preview.
- 300 m versus less than 300 m needs explicit approximate display or consistent
  rounding; it is not evidence of simulation disagreement.
- Wrapped choice indentation needs a small formatting fix.

These are recorded next-iteration requirements in gameplay/playtest specs.
They have not been implemented in the frozen build evaluated here. Five-turn
and safety/suppression calibration remain unchanged. No kiosk or human
playtest acceptance is claimed.
