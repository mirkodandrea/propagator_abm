# 03 — Fasi e verifiche

Le fasi vanno fatte **una alla volta**: ognuna finisce con un **checkpoint umano**, e la successiva non parte senza conferma dell'utente. Il lavoro dentro la fase si tiene in `docs/TODO.md`. Le prove si eseguono sempre con gli strumenti del repository e con seed salvati (1–3).

## Fasi (dal 2026-10-14, nuovo progetto)

| Fase | Cosa | Si chiude con | Checkpoint |
|---|---|---|---|
| **1. Prima passata** | Motore nuovo (`rocca`: pausa, esagoni, squadre, evacuazione semplificata, tutorial, stelle), gioco adattato, documenti riscritti. | Prove headless del tutorial e della partita, screenshot degli stati principali. | L'utente rivede e dice le modifiche. |
| **2. Bilanciamento** | Apertura della partita (oggi le squadre all'innesco al minuto 0 la risolvono), limite dell'evacuazione, raggio di protezione, soglie delle stelle. | Tabella: strategie semplici × semi 1–3 (vedi sotto). | L'utente sceglie i valori. |
| **3. UX e grafica** | Leggibilità di esagoni, percorsi e strade chiuse; icone delle squadre; trascinare la squadra sull'esagono; famiglie in strada visibili; Mario. | Screenshot `KIOSK_SNAP` di tutti gli stati, playtest con `scripts/playtest.py`. | L'utente decide le correzioni. |
| **4. Comunicazione** | Testi, personaggi, debriefing: da riscrivere a posteriori (richiesta dell'utente). | Proposta di testi. | L'utente approva. |
| **5. Pronto per la pubblicazione** | Macchina reale: FPS, stabilità (molte partite di fila), offline, web o nativo. | Lista di controllo firmata. | Ultimo checkpoint. |

**Consegna di ogni fase:** al massimo una pagina in italiano, in quattro sezioni:

1. **Realizzato:** file e moduli toccati.
2. **Evidenza:** comandi per riprodurre, più una tabella o un'immagine.
3. **Non risolto:** rischi e test falliti.
4. **Una sola proposta** per il passo successivo.

## Batteria di prove headless

Dalla radice del repository, in release (`cargo build --release -p rocca --examples`).

| Prova | Comando | Cosa guardare |
|---|---|---|
| Test del motore | `cargo test --release -p rocca` (e `-p game sim::` per l'invarianza dal passo) | Tutti verdi. |
| Tutorial | `TRACCIA=1 target/release/rocca tutorial` | Con il copione nessuna persona colta né casa colpita; senza ordini sì. |
| Partita con ordini | `target/release/rocca medium --squadra "20:0=5590;4400,20:1=5300;4600" --evacua "20:Le Terrazze"` | Tabella per borgo, ettari, stelle contro «senza ordini». `TRACCIA=1` stampa ogni 10 min fuoco, distanze e squadre. |
| Ricerca dell'innesco del tutorial | `target/release/examples/tutorial_cerca [VENTO_KMH MINUTI]` | Inneschi in cui il fuoco raggiunge il borgo senza ordini e si spegne con due squadre. |
| Laboratorio di spegnimento | `target/release/lab [--scenari ..] [--strategie "lead;planner"] [--squadre 1,2,4] [--semi 1-3] [--sweep chiave=v1,v2] [--senza-una] [--serie]` (`lab --lista`) | Scenari sintetici, strategie di coordinamento e telemetria completa: `docs/05-LABORATORIO.md`. |

## Verifiche che non si possono saltare

- **Nessuna strategia semplice domina:** né «tutte le squadre sull'innesco subito», né «evacua tutto appena possibile» devono bastare per avere 5 stelle ovunque.
- **Ordini solo in pausa**, un'evacuazione alla volta; nessuna ricarica della pausa dal 2026-10-16 (test `orders_only_in_pause_and_one_evacuation_at_a_time`).
- **Anteprima senza effetti:** `plan_move` e `plan_evacuation` non cambiano lo stato.
- **Stessi criteri ovunque:** ciò che il giocatore vede (persone colpite, case colpite) e ciò che le stelle contano vengono dalla stessa regola (`evac::Family::state`, `ExposureField::alight`).
- **Riproducibilità:** stessi seme, stato, ordini e tempi simulati danno lo stesso risultato. Gli effetti dipendono dal tempo simulato, non dalla velocità né dai frame (test `the_speed_does_not_change_the_game`).
- **Nessuna informazione dal futuro:** la previsione usa lo stato presente e il bollettino.
- **Strade:** percorsi su nodi raggiungibili (`nearest_reachable`), nessun A* a ogni passo (le evacuazioni ricontrollano ogni 60 s).
- **Grafica** (`KIOSK_SNAP`, guardando le immagini): segni sopra la vegetazione, nessuna mesh vuota (una mesh senza attributi manda in panico la pipeline di prepass), effetti puliti dopo il riavvio.
