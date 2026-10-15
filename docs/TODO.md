# Isola di Roccavento — lista di lavoro

Va aggiornata a ogni iterazione, nello stesso commit del lavoro (vedi `CLAUDE.md`). Le voci chiuse si cancellano: la storia è in git. Le fasi sono in `docs/03-VERIFICHE.md`.

## Fase corrente: 1. Prima passata (nuovo progetto, 2026-10-14) — fatta, commit `ab7d0bf`

**Prossimo checkpoint:** il bilanciamento (fase 2). Si parte dalla tabella qui sotto, poi decide l'utente. Le specifiche originali sono in `docs/00-SPECIFICHE.md`.

### Decisioni recenti

- 2026-10-15 (richiesta dell'utente): **strade secondarie applicate al territorio principale** (`tools/factory/strade.py`, quattro vie sulla piana est in `osm.json`, `fuel.i32` invariato, dettagli in `02-TERRITORIO.md`); la variante `rocca_strade` è stata tolta. Test e tutorial invariati (11 ha senza ordini anche senza le strade). Con la SP 12 aperta tempi di arrivo ed evacuazione non cambiano; `medium` seme 1/2/3, piano ragionato: persone colte 77/211/181 → 92/236/243 (a Pian dei Grilli, dove non ci sono strade nuove: il fuoco diverge, rumore). **Grafica non vista** (le strade compaiono nel diorama?), **tabella di bilanciamento da rifare**, prova con la SP 12 chiusa da fare.
- 2026-10-15: le auto restano sulle strade carrabili; le persone della squadra sono unità separate (`Unit::carrier`/`aboard` in `abm::suppression`, `Team::crew_units`), viaggiano a bordo, scendono alla fermata e camminano in linea retta (prima usavano anche sentieri e piste con il furgone, poi tagliavano il bosco). Possono separarsi: ventaglio di 60 m e linee proprie. Un nuovo ordine aspetta chi è ancora a piedi (`Game::dispatch`). Test: `the_engine_keeps_to_the_road_and_its_people_get_off_and_spread`. Compilato e provato headless, **grafica non vista** (nessuno screenshot). `medium` con 2 squadre, semi 1/2/3: ettari 221/497/385, persone colte 46/156/201 (prima 392/387/379 e 150/130/112): **tabella di bilanciamento da rifare** (a piedi si arriva più tardi: 1,1 m/s contro 3 m/s sulle piste).
- 2026-10-15: dove una squadra a piedi taglia la linea (`FireSim::cleared`) l'85 % delle piante sparisce dal diorama, a caso ma con bordo netto (`vegetation::burn`, vertici collassati, `CLEARED_SHARE`) e il suolo diventa terra nuda (strato di terra di `fire_view`, `FireField::cleared_fraction`). Solo grafica; compilato, **non ancora visto a schermo** (nessuno screenshot, su richiesta dell'utente).
- 2026-10-15: esagoni di **300 m** (`HEX_M`, prima 500) e raggio d'azione delle persone a piedi di **1 esagono** intorno = 300 m (2 esagoni = 600 m erano stati provati e scartati) (`CREW_RANGE_M`, prima 400 m), richiesta dell'utente. Con 300 m, `medium` (2 squadre a 5590;4400 e 5300;4600 + evacua Le Terrazze), semi 1/2/3: ettari 392/387/379 (prima 95/364/488), persone colte 150/130/112 (prima 55/158/172): il punto stradale dell'esagono cambia con la griglia. Tutorial: la prima mossa passa a 3792;3595 (con 3295 colpiva 5 persone); il test chiede ettari < senza ordini / 3 (prima / 4: 3,3 ha contro 12,2). **Da riverificare nella tabella di bilanciamento.** `PROTECT_M` (300 m) invariato.
- 2026-10-15: la simulazione parte alle **11:00** (prima 14:00), richiesta dell'utente: `rocca::words::START_H`; l'orologio e il sole del chiosco ne derivano (11:00–14:00). Il meteo dello scenario non dipende dall'ora.
- 2026-10-14: le strade si riaprono 8 min dopo l'accensione delle celle vicine (`FLAME_S` in `fire`): le celle che covano non chiudono più la strada (restano a 0,25 come le bruciate). Traccia dell'acqua e della linea tagliata più visibile (`units.rs`). **Da riverificare nella tabella di bilanciamento.**

### Da fare, in ordine

1. [ ] **Tabella di bilanciamento** (`target/release/rocca medium --seme N ...`), semi 1–3, con ritardo di 30 min. Strategie:
   - «nessun ordine»;
   - «tutte le squadre sull'innesco subito»;
   - «evacua tutto, un borgo dopo l'altro, appena possibile»;
   - «piano ragionato».

   Nessuna deve dominare. Oggi il piano ragionato prende Persone 5, Case 2, Ettari 2–3 (semi 1 e 2). Per ripetere le prove conviene un esempio `examples/strategie.rs` che le giochi tutte e stampi la tabella.
2. [ ] **Soglia delle persone colpite:** chi è a casa senza ordine viene contato «colpito» quando il pericolo alla porta arriva a `ALARMING`. È severa: 65 persone a Le Terrazze già alle 11:55. Proporre `IMPASSABLE` o la casa colpita, poi decide l'utente.
3. [ ] **Soglie delle stelle** (`score::{PERSONE, CASE, ETTARI}`), da ricavare dalla tabella del punto 1.
4. [ ] **Persone a piedi delle squadre:** vederle a schermo con `KIOSK_SNAP`, e misurare quanto conta il taglio rispetto all'acqua (partite con e senza `crews_follow_the_fire`).
5. [ ] **Tutorial** (ridisegnato il 2026-10-14: parte in pausa, 2 ordini, 30 min a ×120 con pausa automatica, evacuazione, fine): provare a mano il flusso nel chiosco; il tutorial finisce a evacuazione terminata (`Game::evacuation_finished`), caso di 60 min. **Velocità:** unica ×120 o ferma (`rocca::RUN_X`, 2026-10-14); tolto il selettore dal menu operatore. Le prove di bilanciamento non dipendono dalla velocità, ma la ricarica dell'esperto ora è 10 s reali = 20 min simulati (2026-10-15, richiesta dell'utente: ×120 e 10 s): il bilanciamento del giorno 2 va ripreso con questo ritmo.
6. [ ] **Grafica e UX:**
   - famiglie in strada invisibili;
   - coni 3D trascinabili (senza conferma), pulsante di pausa rotondo, piano modificabile in pausa, percorso e tempo dell'evacuazione al passaggio del mouse, pausa che ferma il mondo: fatti il 2026-10-14 ma provati solo con `KIOSK_SNAP`. Il trascinamento vero e la pausa d'apertura vanno provati a mano;
   - `scripts/playtest.py` cerca ancora «Conferma» (punto 8).
7. [ ] **Pulizia del codice:**
   - togliere dal crate `abm` il modello IBM delle persone (`Abm`, traffico, comportamenti delle famiglie), tenendo `network`, `drive`, `suppression` e `refuge`;
   - ripulire gli avvisi di compilazione nel chiosco (`icons.rs`, `overlays.rs`).
8. [ ] **Playtest:** aggiornare `scripts/playtest.py` al nuovo flusso, poi partite con persone che non conoscono il progetto.
9. [ ] **Versione web:** provare `scripts/build_web.sh`. Il confronto senza ordini si calcola alla fine, nel frame: potrebbe bloccare.
10. [ ] **Comunicazione** (fase 4): testi, personaggi e debriefing da riscrivere a posteriori.
11. [ ] **Mario:** ricollegarlo per ultimo (decisione dell'utente).

## Test rotti noti

- `abm/tests/incident_gaps.rs`: 5 test pensati per un territorio senza mare.
- `game models::tests::the_island_models_stand_on_the_ground` (punto più basso della rocca a −2,6 m).

## Decisioni in vigore (con data)

- 2026-10-14 (richiesta dell'utente): pannello del tempo. In pausa il mondo è fermo e ▶ lo fa ripartire; in gioco il pulsante conta i secondi alla prossima pausa, **30 s dalla ripresa** (`PAUSE_COOLDOWN_REAL_S`), con anello ambra e «N s». La regola era già così; cambiato solo l'aspetto della ricarica.

- 2026-10-14: dietro la schermata iniziale gira l'incendio della partita, senza ordini, con la camera dinamica del vecchio chiosco (inquadrature da 26 s: fronte, borgo più vicino, fronte da vicino; velocità legata all'inquadratura; ricomincia a fine caso). `kiosk::{Subject, demo_shot}`, `view::pick_shot`. Provato solo con `KIOSK_SNAP` (t=0); il movimento a mano no. Niente squadre scriptate (il vecchio piano scriptato non esiste più): l'inquadratura «squadra» scatta solo se una squadra è vicina al fuoco.

- 2026-10-14 (notte, richieste dell'utente): testo del tutorial sulla pausa («vedrai anche le previsioni di dove andrà il fuoco nelle prossime ore»); segni sulla mappa con un materiale piatto e acceso sopra la vegetazione (`game::flat`); evacuazione in verde, squadre in ciano; il riquadro dell'evacuazione mostra solo persone e minuti; il timer conta fino all'arrivo dell'ultima famiglia, con la stessa regola del tempo annunciato (prova: `cargo run --release -p rocca --example timer_evac`).

- 2026-10-14: schermata iniziale, tutorial, giorno 2 e finale ripresi dal vecchio chiosco (branch `settimana-protezione-civile`) nello stile e con i volti dei personaggi, adattati al nuovo gioco: `kiosk/screens.rs`, `kiosk/characters.rs` (§6 di `01-GIOCO.md`). Non ripresi: velo scuro con buchi sul tutorial (il nuovo tutorial è interattivo), livelli di difficoltà, sindaca, stelle Tempismo/Misura. Provate con `KIOSK_SNAP`; il flusso vero a mano no.
- 2026-10-14 (notte, richieste dell'utente): il cono semitrasparente che fluttua sostituisce la bandiera; la ricarica dell'esperto è di **30 s reali** (2 h 30 min simulati); pulsante di pausa rotondo con anelli per durata e ricarica; in pausa il piano si cambia (evacuazione annullabile o sostituibile); icone e anelli al posto del testo; aree di raccolta più accessibili per ogni borgo.
- 2026-10-14 (sera, richieste dell'utente): la **pausa ferma il mondo**, senza limite di tempo (tolto `PAUSE_REAL_S`; il pulsante, grande, sta nel pannello del tempo in alto a sinistra e conta i secondi alla prossima pausa); il giorno 2 **parte in pausa**, senza limite, e la pausa d'apertura non consuma l'esperto; le squadre hanno una **bandiera 3D** dove sono assegnate, da trascinare in pausa, e l'ordine parte al rilascio, **senza conferma** (tolta l'icona sul veicolo); percorso e tempo dell'evacuazione si vedono **passando sopra il borgo**; l'autobotte senz'acqua **resta sul posto** e torna piena dopo 10 min (`REFILL_S`), niente viaggi agli idranti; esagoni e isocrona delle 3 ore più visibili.
- 2026-10-14: il presidio dei volontari AIB passa da San Rocco a Pian dei Grilli (2450, 2598), dall'altra parte dell'isola rispetto alla caserma di Castelvento, su richiesta dell'utente (`game.json`, `stations[1]`). Da rivedere il bilanciamento di `medium`: il presidio sta nel borgo che brucia.
- 2026-10-14: gli ordini si danno sulla mappa: squadre trascinate sull'esagono, borghi cliccati con «Evacua» in un riquadro accanto. Timer circolare sul borgo in evacuazione.
- 2026-10-14: l'incendio della partita parte già sviluppato (segnalazione al minuto 30). Le squadre attaccano il fuoco come possono: le persone a piedi tagliano la vegetazione verso il fuoco, l'autobotte bagna il combustibile che raggiunge. Mario va ricollegato per ultimo.
- 2026-10-14: **nuovo progetto.** Si riscrive `rocca` dentro questo repository. Partita in due parti, tutorial scriptato (giorno 1) e partita (giorno 2). Tempo reale (allora ×300, poi ×60 il 2026-10-14), pausa (allora ×60) di 30 s reali con ricarica di 20 min simulati, ordini solo in pausa, previsione dell'esperto in pausa. Squadre = autobotte + 3 persone: 3 VVF nel borgo più grande, 2 volontari in un borgo minore. Esagoni di 500 m adiacenti alle strade. Evacuazione semplificata, un borgo alla volta, con limite di persone al minuto. Niente IBM delle persone. Stelle e classifica tenute: persone, poi case, poi ettari. Comunicazione da riscrivere dopo.
- 2026-10-13: portata dello spegnimento ×20 (`game::SUPPRESSION` = linea 3, acqua 30, portata 20).
- 2026-10-13: territorio dell'isola con valico e costa est rifatta (`02-TERRITORIO.md`).
- 2026-10-12: schermo del chiosco di almeno 1080p.
