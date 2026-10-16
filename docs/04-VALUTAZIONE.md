# Valutazione e finale didattico — 2026-10-16

Richiesta: valutazione meno punitiva, spiegazione dei risultati e messaggi finali di protezione civile. Su richiesta del 2026-10-16 i testi aggiunti restano brevi e colloquiali: frasi complete per i risultati, un consiglio e tre messaggi di protezione civile con un minimo di contesto.

## Già attivo nei sorgenti

Il finale desktop e mobile spiega, per Persone, Case e Bosco, il danno evitato rispetto allo stesso incendio senza ordini, in numeri e percentuale. Distingue un miglioramento da danni uguali o maggiori; non attribuisce cause agli ordini senza una traccia che le dimostri. Una voce senza danni nel riferimento non assegna stelle, ma segnala comunque eventuali danni nella partita del giocatore.

Il suggerimento per riprovare dà priorità alle persone, segnala chi è ancora in viaggio, richiama la sicurezza dei soccorritori e poi la difesa dei borghi. Non confonde “nessuno raggiunto dal fuoco” con “tutti arrivati al sicuro”. La colonna desktop scorre; il telefono conserva la pagina scorrevole.

Tre messaggi per i cittadini: conoscere il Piano comunale; segnalare fumo e fiamme al 115 o al 112 dove attivo, localizzando l'incendio; non ostacolare i soccorsi e seguire le autorità. Testi disponibili offline, con collegamento alla [fonte ufficiale Io non rischio](https://iononrischio.protezionecivile.gov.it/it/preparati/incendi-boschivi/cosa-fare/). Il gioco è un territorio immaginario e non insegna procedure operative di spegnimento.

## Scala candidata, da approvare

La guida `CLAUDE.md` richiede prove headless e scelta dell'utente prima di cambiare il bilanciamento. `score::proposed_stars` è verificabile, ma **il gioco e la classifica usano ancora `score::stars`**. Nessun flag o attivazione implicita.

La quota misura il danno evitato sul danno del riferimento, non la popolazione totale dell'isola. Ogni categoria parte da una stella; le soglie aggiungono una stella ciascuna. Nessun miglioramento o peggioramento resta a una stella. Denominatore zero: nessuna valutazione.

| Categoria | Soglie attuali per 2/3/4/5 stelle | Proposta per 2/3/4/5 stelle |
|---|---|---|
| Persone | 20 / 45 / 70 / 90% | 10 / 25 / 50 / 75% |
| Case | 10 / 30 / 50 / 75% | 5 / 15 / 30 / 50% |
| Ettari | 10 / 25 / 45 / 65% | 5 / 10 / 20 / 35% |

La proposta valorizza progressi parziali: evitare metà del danno alle persone passa da 3 a 4 stelle; evitare metà del danno alle case da 4 a 5. I numeri restano sempre visibili, anche con 5 stelle e danni residui.

## Prova sul caso pubblicato

Riproduzione: `cargo run --offline --release -p rocca --example score_review`.

Motore e dati correnti, caso indicato da `Territory::game`, semi 1–3. Il riferimento è simulato di nuovo per ogni seme. Politiche: nessun ordine; tutte le squadre all'innesco alla prima pausa; evacuazioni successive in ordine di minaccia; combinata con squadre ridistribuite sui borghi e stesse evacuazioni. Ordini alle pause consentite. Sono strategie di confronto, non una soluzione ottimizzata.

| Seme | Strategia | Persone colpite | Case colpite | ha | Stelle attuali P/C/E | Proposta P/C/E |
|---|---|---:|---:|---:|---|---|
| 1 | nessuno | 93 | 28 | 359 | 1 / 1 / 1 | 1 / 1 / 1 |
| 1 | innesco | 60 | 17 | 229 | 2 / 3 / 3 | 3 / 4 / 5 |
| 1 | evacuazione | 0 | 28 | 359 | 5 / 1 / 1 | 5 / 1 / 1 |
| 1 | combinata | 0 | 17 | 170 | 5 / 3 / 4 | 5 / 4 / 5 |
| 2 | nessuno | 263 | 47 | 493 | 1 / 1 / 1 | 1 / 1 / 1 |
| 2 | innesco | 426 | 140 | 477 | 1 / 1 / 1 | 1 / 1 / 1 |
| 2 | evacuazione | 0 | 47 | 493 | 5 / 1 / 1 | 5 / 1 / 1 |
| 2 | combinata | 0 | 99 | 481 | 5 / 1 / 1 | 5 / 1 / 1 |
| 3 | nessuno | 216 | 34 | 420 | 1 / 1 / 1 | 1 / 1 / 1 |
| 3 | innesco | 426 | 124 | 486 | 1 / 1 / 1 | 1 / 1 / 1 |
| 3 | evacuazione | 23 | 34 | 420 | 4 / 1 / 1 | 5 / 1 / 1 |
| 3 | combinata | 26 | 117 | 373 | 4 / 1 / 2 | 5 / 1 / 3 |

Nessuna delle strategie semplici ottiene 5/5/5. Le sole evacuazioni migliorano le persone senza proteggere case e bosco. Nei semi 2–3, l'attacco all'innesco peggiora le persone e le case: abbassare le soglie non può trasformarlo in un successo. Anche la strategia combinata ha limiti: il bilanciamento della simulazione resta da esaminare separatamente. Tre semi e queste strategie non bastano a escludere ogni possibile strategia dominante.

Se si approva la scala, il passo successivo è usarla in `stars` e distinguere la nuova classifica da quella precedente, evitando confronti fra voti con soglie diverse.

## Verifica dell'implementazione

- `cargo test --offline --release -p rocca -j 1`: 8 test di libreria e 3 di integrazione superati. Fallisce il test del tutorial già documentato: il riferimento colpisce 43 persone ma nessuna casa, mentre il test richiede anche case colpite. Non sono stati modificati fuoco, territorio o tutorial.
- `cargo build --offline --release -p game -j 1`: riuscita.
- Screenshot del finale controllati a 1600×1000 e 390×844, in `out/score-review/desktop/finale.png` e `out/score-review/mobile/finale.png`. Sul desktop corretta l'altezza inizialmente insufficiente del pannello scorrevole. Per riprodurre, usare `KIOSK_SNAP_SPEC='name=prepara,phase=tutorial;name=finale,phase=fine,t=300'`: il passaggio dal tutorial avvia anche il confronto senza ordini.
- `git diff --check`: superato.
