<!-- Codex blind playtest: d8c2b79 + round-1 wording fixes; binary sha256 70de108d8098c603ddd455ed77ea9a4940612141e927815233364354058c4a00 -->

Seeds played: 547990, 139945 · three sessions · fresh tester with no earlier report

# Blind playtest, round 2

I read only brief.md and player-visible ./play output. I played three complete sessions (new, retry of that fire, different new fire). No stato, source, save, binary, web, or other agents were consulted. Perspective during play: curious, impatient 16-year-old with no firefighting knowledge. The think-aloud entries below describe what I understood at each decision. Designer review follows the sessions. All claims concern this game's displayed behavior, not real firefighting advice.

## Sessions

Commands below are arguments to `./play`; every turn ended with `avanti`. Inspection commands do not advance time.

### Session 1: nuova, initial wind South 36 km/h

| Turn | Orders and inspections | Think-aloud and turn verdict |
|---|---|---|
| 1, T+00 | `scegli K`; `ordina K`; `ordina E1 4`; `ordina E2 5`; `ordina P 1`; `ordina S 6` | “An airplane sounds strong; call that immediately. Fire is down there: send the trucks and people to it, warn the biggest town. What is a testa? I can infer front from wind. The trucks say out of range, but maybe getting closer still helps.” Poor opening: both trucks retreat. The order previews warned me, but their prominence did not overcome my intuitive impulse to attack the stars on the map. |
| 2, T+08 | `scegli S`; `aiuto`; `ordina P 2` | “Oops, the trucks ran away with full water. The people are still walking. Help says choose shows what a unit does; now I see S is for houses. I will warn the other town while waiting.” I inspect S but impatiently leave its ineffective trip running. Wind probability increases to 85%; Borgo becomes threatened next turn. |
| 3, T+16 | `ordina E1 1`; `ordina E2 2`; `ordina E3 1`; `ordina S 1`; `ordina P 3` | “Now Borgo is only 150 metres away! Put nearly everyone there, keep a truck at Coste. E3 suddenly exists; nice, but where did it come from? Warn Mulino too, why risk leaving people uninformed?” Recovery succeeds for houses; S can be redirected in transit. Wind changes to East, shifting threat west. |
| 4, T+24 | `scegli K`; `ordina K 2` | “It still says arriving at turn 4 although I'm already in turn 4. Just one minute left, fine. Why is an airplane useless at the threatened Borgo? The only helpful preview is Coste, so choose that.” Clear mechanical choice, unclear reason for difference between towns. |
| 5, T+32 | no new orders | “Everybody's working, water refills itself, let them finish. Is this really the end with another twenty minutes after?” Fire runs to T+60. Final judgement is informative but the Borgo verdict is puzzling. |

Final: **Hai fatto la differenza**. 12 families blocked (26 without orders), 65 houses hit (107). Borgo: **Tardi**, 2 blocked vs 3, 13 houses vs 43; Coste: **In tempo**, 10 vs 23, 52 houses vs 64; Mulino: **Allarme inutile**, zero damage. I feel I recovered, and immediately want a retry. I don't understand why the earliest possible Borgo warning is called late despite reducing blocked families.

### Session 2: riprova, same fire

| Turn | Orders and inspections | Think-aloud and turn verdict |
|---|---|---|
| 1 | `scegli E1`; `scegli P`; `ordina K`; `ordina P 2`; `ordina E1 1`; `ordina E2 2`; `ordina S 1` | “Okay, trucks defend homes. Coste lost the most houses, and the wind will go west, so warn Coste first. Get the slow walking team to Borgo now.” Good planning: no retreats. E1's preview explicitly offers 32 defended houses; S's travel is 13 minutes. |
| 2 | `ordina P 1` | “Warn Borgo next. The other orders keep going. It says warned Coste but zero travelling and zero safe—do they need time to pack?” Borgo still becomes threatened at T+16; Coste now has a head start on warning. |
| 3 | `ordina E3 2` | “A new truck again. Coste has only one, Borgo has the walking team; send the extra west.” Allocation feels consequential. E3 promises 23 defended houses at Coste. |
| 4 | `ordina K 2` | “Airplane repeats what worked. Don't disturb the others.” A fairly automatic turn. |
| 5 | no new orders | “Nothing else useful to do. Compare the numbers now.” A passive finish, but clear continuation of prior orders. |

Final: **Hai fatto la differenza**. 8 families blocked (26), 39 houses hit (107). Borgo: **Tardi**, 3 blocked vs 3, 12 houses vs 43; Coste: **In tempo**, 5 vs 23, 27 houses vs 64; Mulino: **Giusto non avvisare**. Improvement over session 1 is satisfying; tradeoff is clear from outcomes, although warning verdicts do not explain the path from warning to safety.

### Session 3: nuova, different fire, South 37 km/h

| Turn | Orders and inspections | Think-aloud and turn verdict |
|---|---|---|
| 1 | `scegli E1`; `ordina K`; `ordina P 1`; `ordina E1 1`; `ordina E2 2`; `ordina S 2` | “This looks almost the same; only one extra kilometre per hour? Try the walking team at Coste and warn big Borgo first.” The new fire later behaves differently, but the initial screen doesn't signal much variety. S takes 22 minutes to Coste. |
| 2 | `ordina P 2` | “The revised wind change is earlier, T+09 to T+21. Coste will probably matter most. Warn it now.” Wind turns East by T+16, sooner than the first fire; Borgo remains farther away. |
| 3 | `scegli E3`; `ordina E3 2`; `annulla E3`; `ordina E3 1` | “Preview says Borgo is useless now, but it still has the most people and the fire could spread. Maybe prepare there while the other team reaches Coste. Undo works; good.” Deliberate experiment against the recommendation. E3 arrives at Borgo, consumes water to 18%, and is labelled working; no explanation resolves what ‘inutile adesso’ means. |
| 4 | `scegli K`; `ordina K 2`; `ordina E3 2` | “Coste is threatened, Borgo is still 500 metres away. Move the extra truck and use airplane there.” Reallocation feels real; the order says 4 minutes and 12 defended houses despite only 18% water. |
| 5 | no new orders | “Coste says reached, so there's damage even with all those units. I warned both towns; why so many families still there?” Final outcome reveals warning timing matters, but not why so few families escaped. |

Final: **Hai fatto la differenza**. 18 families blocked (19), 25 houses hit (62). Borgo: **In tempo**, 0 blocked and 0 houses hit, both baseline zero; Coste: **Tardi**, 18 vs 19 blocked, 25 vs 62 houses; Mulino: **Giusto non avvisare**. Strong property protection, weak evacuation. The different fire changed the meaningful target, so replay isn't completely rote.

## Usability / Fun / Interest

- **Usability: 3/5.** Commands work and `scegli` gives useful previews, but huge repeated ASCII maps bury the changing details. Exact confusing text: `Dove la mandi? Bersagli possibili:` followed by `./play ordina K → Chiama il Canadair: arriva al turno 4`, then generic `./play ordina K <bersaglio>` even though calling needs no target. `K Canadair in arrivo al turno 4 (tra 1′)` is shown during turn 4. `avvisato: 0 in viaggio, 0 al sicuro` lacks a visible explanation for the delay. `ritirata: salta un turno` is understandable, but the preview `si ferma sulla strada: bersaglio fuori portata` doesn't explicitly warn that retreat and a lost turn will follow.
- **Fun: 3/5.** I wanted to retry because comparisons against “senza ordini” made improvement visible. The first mistake has consequences. Exact limiting text: `inutile adesso` and `non salva case: lì il fuoco non si ferma` leave many apparent choices feeling like traps or menu fillers. Turns 4–5 usually reduce to the sole positive airplane preview, then waiting. `Nessun ordine in questo turno` can sound like a missed action even when existing orders are deliberately continuing.
- **Interest: 4/5.** Wind forecasts, delayed arrival, a walking crew, limited patrol sequencing, new fires from sparks, water refill, and the difference between houses and families are intriguing. Exact confusing feedback: `[!] Tardi — avviso tardivo o senza riduzione delle famiglie bloccate`. This collapses two different explanations into one; in session 1 Borgo was warned first and had fewer blocked families. Third-session Borgo says `[OK] In tempo — avvisati prima che arrivasse il fuoco` even though its damage baseline was zero and the fire never reached houses. That clashes with the previous Mulino `Allarme inutile` explanation.

## What the game taught me

Wind tells me where danger is moving, and a forecast can make a place important before its warning label changes. Sending everything directly to the flames is a poor plan in this game: trucks stay on roads and people can have to retreat. Getting resources to houses early can save property even if the fire continues to spread. Warning a neighbourhood is separate from its families actually reaching safety, and one patrol has to choose an order for visiting places. The airplane has to be requested well ahead of time, while the walking team can take most of the playable time just to arrive. I still cannot explain how the game's warning-to-escape timing works, or why some warnings count as unnecessary and others as timely when neither neighbourhood burns.

## Decisions that felt real / obvious / fake

- **Real:** who the patrol visits first; starting the slow crew early; dividing the extra truck between Borgo and Coste; moving E3 after wind and distances change; accepting that property and evacuation outcomes can differ. Deliberately letting orders continue is a valid decision.
- **Obvious:** call K immediately because I never saw a cost or competing reason to delay. At turn 4 Coste was the only positive Canadair option. Mulino never offered a positive firefighting preview across these sessions.
- **Fake or weakly supported:** three fire targets are advertised but repeatedly say they save no houses; the attractive visual stars lure a novice into an order the game already knows is ineffective. ‘Defends N houses’ without a reason seems like a displayed answer to maximize. ‘Inutile adesso’ tells me what to pick, but not whether pre-positioning is legitimate. The only patrol is an interesting limit, but the fiction doesn't explain why all warning must be done in person.

## Suggested changes, ranked (expected player impact)

1. **Large:** separate final warning judgements into observable reasons: arrival time, when danger became visible, when routes closed, and families saved. Explicitly resolve earliest-possible warnings labelled late and no-damage warnings labelled timely.
2. **Large:** show one compact changed-state screen per decision, with optional map expansion or animation. Highlight arrivals, wind changes, retreat, and refill; repeated giant maps make a short stand game feel slower than its five turns.
3. **Medium:** put resource jobs into the initial screen: patrol warns, trucks defend houses from roads, crew walks slowly, airplane requires an early call. A novice currently has to discover those explanations through `scegli` after making mistakes.
4. **Medium:** replace `inutile adesso` with the actual reason and scope, e.g. no fire expected there under current forecast, already enough defence, or cannot reach the fire. Distinguish ‘no benefit this instant’ from ‘no predicted benefit before T+60’.
5. **Medium:** preview risky orders as risk plus consequence: ‘outside hose reach; heat may force withdrawal; lose next turn.’ Keep those choices available, but make a wrong order an understood gamble rather than a misleading journey.
6. **Medium:** make late turns require a meaningful response or shorten the finish when players are only maintaining orders. A new arrival or wind shift should visibly demand a decision; otherwise reinforce that existing orders are still working when no new order is issued.
7. **Small:** fix the Canadair prompt to show the no-target call syntax until requested and label ETA in minutes consistently; announce E3's arrival and explain ‘warned but not yet travelling’ with a short household preparation message.

## Reproducible bugs / player-visible inconsistencies

No crash or failed accepted command occurred. `annulla E3` removed the queued order as promised. These are text/feedback issues; I cannot establish hidden simulation bugs.

1. **Canadair call syntax contradicts footer.** At initial turn use `scegli K`. Its listed valid action is `ordina K`, while the footer is `ordina K <bersaglio>`. Actual no-target `ordina K` works. This is reproducible in both new initial screens inspected, and retry retained the same interface.
2. **Canadair turn-label ambiguity.** Call K at T+00; advance three turns. Turn 4 at T+24 shows `in arrivo al turno 4 (tra 1′)`. Internally plausible timing, but ‘arriva al turno 4’ reads as already available at the turn's start. The unit can receive a target while in transit, so this does not block play.
3. **Warning verdict lacks a matching stated condition in session 1.** Exact session-1 orders above: P warns Borgo on the first possible turn with displayed three-minute arrival. Final Borgo reports 2 blocked vs 3 without orders, yet `Tardi — avviso tardivo o senza riduzione delle famiglie bloccate`. At least the ‘without reduction’ alternative is contradicted by visible counts; no text establishes how the first-turn warning was late. Needs either a more precise criterion or corrected wording.
4. **Warning verdict consistency question across neighbourhoods.** Session 1 Mulino was warned and has 0/0 baseline damage, producing `Allarme inutile`. Session 3 Borgo was warned and also has 0/0 damage baseline, producing `In tempo — avvisati prima che arrivasse il fuoco`; it never reached Borgo houses at the visible sampled turns. The forecast may justify the difference, but the final wording says the fire arrived when the visible result doesn't establish that. Reproduce using session 3 orders and compare session 1 summary. Treat as explanatory inconsistency, not proof of identical danger.

## Log

Complete decision-level sequence, including non-order commands:

1. Read brief. `nuova` → `scegli K` → `ordina K` → `ordina E1 4` → `ordina E2 5` → `ordina P 1` → `ordina S 6` → `avanti`.
2. `scegli S` → `aiuto` → `ordina P 2` → `avanti`.
3. `ordina E1 1` → `ordina E2 2` → `ordina E3 1` → `ordina S 1` → `ordina P 3` → `avanti`.
4. `scegli K` → `ordina K 2` → `avanti`.
5. `avanti` → final 12/65 versus 26/107.
6. `riprova` → `scegli E1` → `scegli P` → `ordina K` → `ordina P 2` → `ordina E1 1` → `ordina E2 2` → `ordina S 1` → `avanti`.
7. `ordina P 1` → `avanti`.
8. `ordina E3 2` → `avanti`.
9. `ordina K 2` → `avanti`.
10. `avanti` → final 8/39 versus 26/107.
11. `nuova` → `scegli E1` → `ordina K` → `ordina P 1` → `ordina E1 1` → `ordina E2 2` → `ordina S 2` → `avanti`.
12. `ordina P 2` → `avanti`.
13. `scegli E3` → `ordina E3 2` → `annulla E3` → `ordina E3 1` → `avanti`.
14. `scegli K` → `ordina K 2` → `ordina E3 2` → `avanti`.
15. `avanti` → final 18/25 versus 19/62.

The turn-by-turn think-aloud is in the Sessions tables. Review deliberately judges only visible information. I did not attempt to reverse engineer why previews or evacuation counts change.


## Triage

Usability 3/5, fun 3/5, interest 4/5: the milestone-2 gate remains **not met**.
The tester independently states six core lessons; the cost of an unnecessary
patrol stop remains weak. This is a fresh tester on different random seeds,
so unchanged scores are not a controlled estimate of the wording fixes' effect.

| Suggestion | Decision | Reason / next step |
|---|---|---|
| Explain each warning stamp with the actual reason | Accept as next design work; define typed reasons in the spec before implementation | The two-clause late-warning explanation still fails. Show warning arrival, first threat, required lead, and caught-family comparison; distinguish justified precaution from fire reaching houses. |
| Compact changed-state screen, optional full map | Measure first; Mirko review | High potential for clarity, but alters the presentation contract. Keep map optional only after defining how players inspect geography. |
| Resource jobs on the opening | Accept as proposed next spec change | The player still attacks flames before reading selection descriptions. A concise job line may help without adding interactions. |
| Specific reasons for useless orders | Accept as proposed next contract change | A single `Inutile` reason cannot explain already-warned versus outside the forecast path. Add typed reasons rather than infer logic in the front end. |
| Risk / withdrawal previews | Measure first | Withdrawal was not guaranteed by the head sweeps. Explain possible heat risk without promising it will always happen. |
| Meaningful late turns or shorter finish | Measure first; Mirko decision | Both blind rounds report passive final turns. This requires a gameplay decision and new balance sweeps. |
| Canadair call footer, timing, reinforcements and preparation | Footer fixed after this round; other explanations proposed | Footer now omits `<bersaglio>` for an uncalled plane, with a CLI regression check. Arrival during turn 4 is valid; clearer minutes and household-preparation feedback remain next work. |

The footer fix was made **after** the frozen binary tested in this report;
these scores do not evaluate that fix. No firefighting mechanism, turn count,
verdict policy, or balance number was changed in this wording iteration.
