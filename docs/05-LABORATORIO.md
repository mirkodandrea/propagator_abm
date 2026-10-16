# Laboratorio di spegnimento

Analisi headless degli algoritmi con cui le squadre (una o più, coordinate) spengono il fuoco, su territori sintetici semplificati. Passa tutto da `rocca::Game` (nessuna fisica o regola a parte); il laboratorio legge e dà gli stessi ordini del giocatore. Codice: `crates/rocca/src/lab/`, programma `lab` (`crates/rocca/src/bin/lab.rs`), test `crates/rocca/tests/lab.rs`. Nessuna finestra, nessun chiosco.

```
cargo build --release --offline -p rocca
target/release/lab --lista
target/release/lab --out out/suppress-lab --serie                  # tutto: 9 scenari × 6 strategie × 1/2/4 squadre × semi 1–3 (~2 min)
target/release/lab --scenari pianura,pendio --strategie "lead;planner" --squadre 2,4 --semi 1-5
target/release/lab --sweep engine_range_m=60,150,250 --sweep refill_s=300,1200 --senza-una
```

## Che cosa c'è

- **Scenari** (`lab/synth.rs`, scritti in `out/.../data/<nome>/` nello stesso formato di `data/scenarios`): mondo di 4 km a celle di 20 m; rilievo piano, pendio o crinale; combustibile uniforme, a chiazze o con fascia non combustibile; reticolo di strade, croce o strada unica; uno o due borghi; vento costante o che gira. Nove: `pianura pendio crinale macchia fascia due-borghi strada-unica raffica cambio-vento`.
- **Strategie** (`lab/strategy.rs`): al posto del giocatore, all'apertura e a ogni revisione (pausa) dicono dove vogliono le squadre; il laboratorio le aggancia all'esagono raggiungibile più vicino. `none`, `head` (tutti sulla testa), `lead` (davanti al fronte), `flanks`, `village`, `split`, `planner` (esagoni a punteggio: case protette, combustibile minacciato, blocco della testa, tempo di arrivo, margine di sicurezza; assegnazione golosa senza sovrapposizioni). Parametri: `lead:lead=600,spread=300,rev=20`.
- **Tarature** (`Game::tuning`, `rocca::Tuning`): le costanti degli algoritmi in un solo punto (portata e ricerca dell'autobotte, margini di parcheggio, ricarica, distanza e portata della linea a mano, raggio di protezione, moltiplicatori di linea/acqua/sangue freddo). I valori predefiniti sono le costanti di sempre: il gioco non cambia.
- **Misure** (`lab/telemetry.rs`), per corsa in `runs.csv` e a confronto con lo stesso fuoco senza ordini (stesso seme): ettari, case colpite, squadre perse, ettari risparmiati (e celle salvate/in più), curva dell'area a 15–240 min e avanzata della testa, litri e metri di linea (anche per ettaro risparmiato), tempo di primo contatto e di prima acqua, quota di tempo per stato (in viaggio, al lavoro, in ricarica, ferma, ritirata) per autobotte e per persone a piedi, perché un'unità non lavora (le note), acqua e linee **tenute** o fallite dal fuoco, dove si lavora rispetto al fuoco (davanti/fianco/dietro), distanza dalla fiamma all'arrivo e minima, calore, ritirate, ricollocamenti dell'autobotte, ordini e riordini. Con `--serie`: `series.csv` (fuoco ogni minuto) e `units.csv` (ogni unità ogni 2 min).
- **Rapporto** (`report.md`): tabella per scenario, migliore per scenario, risparmio contro numero di squadre, effetto di ogni taratura (`--sweep`), valore di ogni squadra (`--senza-una`), segnali sugli algoritmi.

## Letture finora (semi 1–3, `out/suppress-lab`)

Risparmio medio in ettari su 9 scenari, 1/2/4 squadre: `head` 17/20/22, `flanks` 15/26/36, `lead` 18/40/66, `planner` 44/71/85, `village` 4/5/11. Tutte le squadre sulla testa saturano subito (ritorno marginale ≈ 2 ha); stare davanti al fronte rende molto di più; il `planner` col peso del blocco a 12, case 0,2, revisione ogni 10 min e margine 120 m (da 7 ettari con 2 squadre a 71) è il riferimento da battere. Tarature (una alla volta): la leva più forte è `engine_range_m` (60 m: −16 punti percentuali; 150 m: +9; 250 m: +22 ma più squadre perse); lasciare il parcheggio a 150 m dalla fiamma costa 6 punti; ricarica lenta (1200 s) −8; i parametri della linea a mano sono già vicini all'ottimo; `protect_m` non ha effetto finché nessuna strategia tiene le squadre vicino alle case. Con 4 squadre il valore marginale di una squadra spesso è nullo o negativo (si sovrappongono). Il bilanciamento resta una decisione dell'utente: nessuna regola del gioco è cambiata.

## Algoritmi automatici delle squadre (2026-10-16)

Che cosa fanno le squadre **dopo** l'ordine del giocatore (non la scelta dell'esagono). Ogni meccanismo ha un interruttore in `Tuning` (`--tune`/`--sweep chiave=valore`, 0/1 per gli interruttori); spenti tutti, il gioco è identico a prima (675 corse su 675 uguali). Nuove misure in `runs.csv`: `engine_behind_share`/`crew_behind_share` (lavoro dietro il fuoco per tipo) e `engine_crowded_share`/`crew_crowded_share` (tempo al lavoro con un'unità di un'altra squadra vicina: autobotti entro 200 m, persone entro 60 m).

**Accesi (predefiniti):**

- `soaked_pts=40` (`SOAKED_PTS`, `Suppression::soaked_pts`, `reachable_targets` e `Game::engines_follow_the_fire`): l'autobotte non bagna il combustibile che ha già 40 punti di umidità aggiunta (estinzione 30). Un serbatoio ne mette già ~70 su tutto ciò che è a portata: il secondo era sprecato e l'autobotte passava il 44 % del tempo in ricarica. Ora, quando tutto è fradicio, si sposta sul combustibile asciutto.
- `reengage=1` (`Game::teams_return_to_work`): dopo una ritirata l'autobotte torna all'esagono quando la strada è di nuovo aperta e dove sta non c'è pericolo; prima restava in caserma «in attesa di ordini».
- `share_fuel=1` (`engines_follow_the_fire`): nello scegliere dove parcheggiare, il combustibile già a portata dell'autobotte di un'altra squadra non conta; senza lavoro a portata, si tiene lontana dalle altre autobotti.

**Provati e spenti** (guadagno nel rumore, sopra i tre accesi): `park_ahead` (minaccia pesata davanti al fronte: +1,3 ± 0,5 ha), `relocate_x=2` (spostarsi anche con lavoro a portata se altrove c'è il doppio: +1,0 ± 0,6), `crew_relaunch` (le persone tracciano la linea anche durante la ricarica e chi è risalito dopo una ritirata conta come libero: persone al lavoro dal 30 % al 62 %, ma −0,5 ± 0,5 ha e più ritirate), `crew_line_ahead` (linea a valle della fiamma più avanzata: −0,7 ± 0,5), `prewet_m=150` (−2,1 ± 0,6), `soaked_pts` 30 e 60 (+0,6 e −2,2). Tolto il ripiego vicino dopo la ritirata (squadre perse 0,23 → 0,35).

**Evidenza** (9 scenari × `head lead flanks split village` × 1/2/4 squadre × semi 1–5 = 675 corse, stessi semi; `out/auto-off` = tutto spento, `out/auto-final` = predefinito):

```
target/release/lab --out out/auto-final --strategie "head;lead;flanks;split;village" --squadre 1,2,4 --semi 1-5
target/release/lab --out out/auto-off   --strategie "head;lead;flanks;split;village" --squadre 1,2,4 --semi 1-5 --tune soaked_pts=0,reengage=0,share_fuel=0
target/release/lab --out out/auto-s3 ... --tune soaked_pts=40,reengage=1,share_fuel=1 --sweep park_ahead=1 --sweep relocate_x=2 ...
```

| misura (media su 675 corse) | prima | dopo |
|---|---:|---:|
| ettari risparmiati | 22,7 | 48,4 (+25,7 ± 1,3) |
| case colpite | 13,4 | 11,8 |
| squadre perse a corsa | 0,23 | 0,06 |
| lavoro dietro il fuoco | 23 % | 25 % |
| acqua mai messa alla prova | 55 % | 71 % |
| acqua fallita | 20 % | 17 % |
| autobotte in ricarica / al lavoro | 44 % / 44 % | 14 % / 71 % |
| autobotti sovrapposte | 34 % | 7 % |
| litri usati a corsa | 58 900 | 18 600 |

Ritorno marginale da 2 a 4 squadre (ha): `head` 0,4 → 5,5, `lead` 15 → 12, `flanks` 13 → 36, `split` 15 → 23. Risparmio a 4 squadre: `lead` 102, `split` 86, `flanks` 73, `head` 41, `village` 16: la testa non domina.

**Aperti:** il lavoro dietro il fuoco non scende (le autobotti seguono la minaccia, che è simmetrica; la mira «davanti» non ha dato un guadagno chiaro); la quota di acqua «mai messa alla prova» sale perché ora si bagna una volta sola anche dove il fuoco poi non arriva; le persone a piedi restano ferme ~60 % (il rilancio delle linee le fa lavorare ma non salva ettari: le linee a 100 m dalla fiamma contano poco con queste tarature). Il bilanciamento del gioco cambia: va rifatta la tabella.
