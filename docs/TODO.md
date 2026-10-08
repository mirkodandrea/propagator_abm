# Rocca Ventosa — todo list

Aggiornare a ogni iterazione (vedi `CLAUDE.md`). Fase corrente in cima.

## Stato

- **Fase corrente: fase 5 consegnata (nuova UX), in attesa del checkpoint 5** (checkpoint 4 approvato il 2026-10-08: un solo motore `rocca::Game`, un solo scenario `data/scenarios/rocca_ventosa`, una sola modalità).
- **Prossimo checkpoint umano: checkpoint 5** (`docs/fase5/README.md`): una partita al chiosco per caso (Coste2_gira, Piano2, Borgo2), prima azione e debrief. Poi la fase 6 (playtest).
- **Per riprendere:**
  - **Comandi:**
    - `cargo test --release --workspace` (35 target verdi);
    - `target/release/rocca <caso> --priorita A,B --b-priorita B,A`;
    - esempi `rocca`: `ab_sweep`, `crisi`, `civili`, `porta`;
    - kiosk: `KIOSK_CASE=Coste2_gira KIOSK_SPEED=300 KIOSK_WINDOWED=1 KIOSK_SHOT=<dir> target/release/game` (partita scriptata fino a «Altro incendio»);
    - kiosk: F2 apre la barra operatore; `KIOSK_FPS=1` scrive gli FPS; `KIOSK_VEG_DENSITY` regola la vegetazione (0,5 di serie); `KIOSK_SCALE=1` forza 1 pixel per punto.
  - **Territorio:** si rigenera con `scenario_factory.py build-town --layout 2`, `town-fires --scenario t4_paese2 --ignitions-from t4_paese` e `publish --scenario t4_paese2`.

## Decisioni dell'utente

- 2026-10-08: **checkpoint 4 approvato** («Continua» dopo la proposta): layout 2 e i tre casi del kiosk, cioè Coste2_gira (principale), Piano2 e Borgo2 (introduttivo).
- 2026-10-08: **prestazioni: non ottimizzare ora**, solo ridurre la densità degli alberi (fatto: metà densità, da 24 a circa 40 FPS su M4 Pro).
- 2026-10-08: la difesa delle case deve **ridurre l'esposizione simulata**, non restare un proxy contabile (`Tally::note_defence`).
- 2026-10-08: incendi più vivaci e più lunghi. Vento di prova 40 km/h, 6 h simulate, umidità **3 %** fissa e uniforme. Evitare i bordi: mondo di 8 km con nucleo progettato di 4 km.
- 2026-10-08: terreno scelto **t4 (Crinale e sella)**.
- 2026-10-08: **checkpoint 2 approvato**, layout 1 di `t4_paese` così com'è. Affrontare la risoluzione fine **prima** della fase 3.
- 2026-10-08: **DTM, strade, case e rendering andranno a risoluzione più fine dei 20 m** del propagatore (che resta a 20 m). Avrà effetto sia sulla grafica sia sui sistemi di gioco. Da pianificare nelle prossime iterazioni.
- 2026-10-08: **il terreno fine serve solo a grafica e agenti, non all'incendio.** Il fuoco usa il DEM a 20 m approvato, invariato (non la media del terreno fine).
- 2026-10-08: punto 2 del checkpoint 3: "correggiamo il territorio". Dopo la misura sui casi veri (`docs/fase4/porta.md`), il blocco «fuoco alla porta» scatta (soglia 0,35). Il territorio è stato corretto per il punto 1: layout 2.
- 2026-10-08: **checkpoint 3 approvato.** La squadra AIB viaggia su mezzo. Le Coste indifendibile nei casi rapidi: accettato.
- 2026-10-08: **un solo motore, un solo scenario, una sola modalità.** Non fare riferimento a versioni vecchie del codice: rimosse (`demo`, `play`, `text`, `chat`, `telemetry`, scenari reali e dev, build web, palette VR, vecchio flusso del kiosk).
- 2026-10-08: fase 3 avviata senza aspettare gli screenshot di `cover.u8` ("segui il piano").
- 2026-10-08: **niente altro lavoro grafico ora**: seguire il piano, proseguire con sistemi e gameplay (fase 3).
- 2026-10-08: **risoluzione fine approvata** ("commit and push … continua"): terreno a 5 m e passo ritracciato a 8,9 km nello stesso corridoio. Avanti con `cover.u8` nel kiosk.

## Da fare

### Fase 2: insediamenti (in corso)

- [x] **Checkpoint umano 2** superato (2026-10-08). Ritocchi possibili più avanti, se servono per la giocabilità: Borgo più esposto (meno orti irrigui, bosco più vicino), passo più corto.
- [ ] Rivedere quali strade sono tagliafuoco: oggi solo la provinciale e le vie del paese sono non combustibili (vedi Borgo2 nella sezione sulla risoluzione fine).
- [ ] Popolazione: oggi tratti "compiacenti" copiati dal vecchio `demo_traits`. Va ripensata insieme alla preallerta (fase 3).

### Risoluzione fine (approvata il 2026-10-08)

- [ ] Verificare in fase 5, a distanza ravvicinata, il colore del suolo da `cover.u8` (ora visibile: niente più palette VR) e le macchie chiare attorno alle case de Le Coste.
- [ ] Primo piano nel kiosk di tornanti e piazzole (draping a livello di pixel).
- [ ] Strade come tagliafuoco: Borgo2 con vento da N supera o no la SP 12 secondo pochi metri di tracciato (spotting). Decidere se la provinciale è una fascia parafuoco più larga o solo una strada.

### Fase 4: crisi e bilanciamento

- [x] 2026-10-08 Rilevatore di crisi (`crates/rocca/src/crisis.rs`) sullo stato conosciuto:
  - tipi: quartiere scoperto con risposta ancora possibile, vento che gira, mezzo perso;
  - al massimo 2 crisi, distanziate di almeno 15 min;
  - niente crisi prima del primo piano, né per un quartiere lasciato scoperto consapevolmente all'ultima conferma.
- [x] 2026-10-08 Casi «_gira» in `game.json`: a T+45 il vento spinge il fuoco verso la località più vicina tra le altre.
- [x] 2026-10-08 **Layout 2** del territorio (`T4_L2` → `rocca_ventosa`): Il Borgo più esposto, Le Coste in 4 nuclei, casi «_gira».
- [x] 2026-10-08 Crisi **previsione** (bollettino 20 min prima del cambio di vento) e preposizionamento per la prima priorità (`PREEMPT_M` 3,5 km).
- [x] 2026-10-08 Report di fase 4 (`docs/fase4/README.md`): le priorità iniziali cambiano gli esiti in 4 casi; la crisi conta in Coste2_gira (58 case colpite rispondendo subito, 71 rispondendo tardi o ignorandola).
- [x] 2026-10-08 **Checkpoint umano 4** approvato.
- [ ] Bollettino meteo con incertezza (orario ± e probabilità), senza leggere il futuro dell'incendio.
- [x] 2026-10-08 Mezzi bloccati: `Game::unit_status` dice «bloccato: strada tagliata dal fuoco» se un mezzo in movimento è fermo da 5 min (`STUCK_S`); etichetta rossa sulla mappa.
- [ ] (prima versione, superata dal layout 2) Misura «la decisione alla crisi conta» (`examples/crisi.rs`, tabella in `docs/fase4/crisi.md`):
  - effetto netto in Coste2_gira: case colpite 76 se si ignora, 61 rispondendo subito, 47 rispondendo 20 min dopo; famiglie colte in casa 24 se si ignora, 12 rispondendo subito con evacuazione;
  - nella maggior parte dei casi rispondere cambia poco: vanno scelti casi in cui la crisi è un vero dilemma, o rivisto il territorio (piano d'azione: «si torna a case/strade e inneschi»).
- [x] 2026-10-08 Kiosk: fase **Crisi** a ×1 con countdown di 25 s. Il piano attivo prosegue; il proposto si applica alla conferma o alla scadenza (rivalidato), e se non cambia resta il piano attuale. Verificato con screenshot su Coste2_gira (crisi del vento a T+46).
- [ ] Le fasi ×0/×N/×1 sono tempo reale del kiosk; l'headless le simula con strategie (`examples/crisi.rs`). Valutare se spostare il countdown nel `Game` per i replay.
- [x] 2026-10-08 Casi scelti: Coste2_gira, Piano2, Borgo2 (`featured` in `game.json`).
- [ ] Preallerta come osservazione del grafo comportamentale (`HouseholdObs`).
- [x] **Rettificato (2026-10-08):** la «scoperta» del porting dei test era sbagliata. Sui casi veri la minaccia alla porta supera 0,35 in 9 casi su 18; non supera mai 0,55 (celle da 20 m). Il testo originale diceva: sul territorio la minaccia per le persone davanti alle case vale sempre 0, perché ogni casa ha una radura di circa 13 m. Il blocco comportamentale «fuoco alla porta» non scatta mai, e con lui i rami evacua-subito, riparo e ultima risorsa. Test `incident_gaps::the_last_resort_profile…` ignorato con motivazione. Decidere se è calibrazione del comportamento o del territorio.
- [ ] Difesa delle strade: misurare prima l'effetto sulla percorribilità (spec).
- [ ] Rivedere i parametri nuovi: `EMBER_DECAY_M` 700 m, `PROTECTED_EMBER` 0,85 e `PROTECTED_RADIANT` 0,5, soglie del coordinatore.

### Fase 5: nuova UX

- [x] 2026-10-08 Mappa a tutto schermo con schede per quartiere (rango, fuoco, mezzi ora e dopo la conferma, Difendi / più importante / non difendere, Preallerta / Evacua), etichette dei mezzi, rotte attive (verdi) e proposte (bianche tratteggiate), barra in alto, crisi con countdown, pulsante unico, debrief contro «senza ordini» con Riprova / Altro incendio, barra operatore F2 (`docs/fase5/README.md`).
- [x] 2026-10-08 Crisi: «scoperto» solo se il fuoco può arrivare entro 45 min (`URGENT_S`); nessuna crisi negli ultimi 10 min (`LAST_CALL_S`).
- [x] 2026-10-08 Inquadratura calcolata (case + innesco nella zona libera, yaw ±50°); corretto il trigger che non scattava (il gioco parte a T+6 s).
- [x] 2026-10-08 Revisione «studente ignaro» (agente Sonnet, solo sugli screenshot). Corretti:
  - debrief con colonne «tu / senza ordini», «evacuate» al posto di «in salvo», «colte in casa dal fuoco», «mezzi persi»;
  - spiegazione di Preallerta / Evacua (testo e hover);
  - schede con «con il nuovo piano ne perde/riceve N»;
  - niente gergo («sottovento al fronte», «Scoperto»);
  - testo più grande e chiaro;
  - «Nuovo incendio, vicino a …».
- [x] 2026-10-08 Versione per browser (`docs/web.md`): dati compilati nel wasm (`crates/datafs`), terreno disegnato a 20 m e vegetazione al 12 % solo nel browser, workflow Pages. Partita giocata con il mouse nel browser: 60 FPS, ×20 regolare.
- [x] 2026-10-08 Dalla partita giocata: crisi del vento e della previsione solo se il quartiere non ha mezzi; motivo corretto dei mezzi in attesa; «mezzi assegnati»; Evacua non illumina più Preallerta.
- [x] 2026-10-08 Pubblicato su https://mirkodandrea.github.io/propagator_abm/. Il ramo `settimana-protezione-civile` è stato aggiunto ai rami ammessi dall'ambiente `github-pages` (decisione dell'utente: con `gh`, senza merge su `main`).
- [x] 2026-10-08 Playtest di GPT 6.1 Sol, che non conosceva il progetto, avviato da una cartella vuota (`docs/fase5/playtest_gpt/report.md`). Corretti:
  - «Ora X non ha mezzi» nella previsione (prima diceva «ha 1 mezzi»);
  - motivo «N mezzi su M» quando una parte non trova postazione;
  - riga «… ancora a casa»;
  - la preallerta dice che qualcuno può partire da solo;
  - pulsante «Vista iniziale»;
  - schede di dimensione fissa (i pulsanti non saltano più sotto il puntatore);
  - nel debrief, le decisioni del giocatore e il nome del mezzo perso.
- [ ] Dal playtest GPT, ancora aperto: click che a volte sembrano non rispondere (forse solo per l'input automatizzato, da verificare con persone vere); distinguere mezzi assegnati / in viaggio / sul posto; mostrare sulla mappa perché una postazione è irraggiungibile.
- [ ] **Checkpoint umano 5.**
- [x] 2026-10-08 Schede dei quartieri separate quando si sovrappongono sullo schermo; etichette dei mezzi impilate e spostate sotto la scheda che le copre. Resta: una scheda può coprire il fuoco.
- [ ] Coordinatore: con la prima priorità irraggiungibile un mezzo rientra alla base (ora spiegato a schermo). Mandarlo al quartiere successivo annullava l'effetto della crisi in Coste2_gira (58 → 71): provato e annullato.
- [ ] Tempo di pianificazione senza limite: decidere nei playtest se serve (spec: ~40 s indicativi).
- [ ] FPS circa 40 su M4 Pro con vegetazione dimezzata: misurare sulla macchina del chiosco (fase 6).

### Fase 6

- [ ] Playtest (vedi `03-PIANO-DI-AZIONE.md`).

### Debito tecnico

- [ ] `CLAUDE.md` cita `docs/rocca-ventosa/…`, ma i documenti sono in `docs/`.
- [ ] File non tracciati rimasti dai vecchi scenari (`data/scenarios/{mati,pedrogao,rhodes,spotorno}/*.tif`, `data/osm_raw.json`, `data/spotorno_render_terrain.tif`, `dist/`, `results/`): da cancellare a mano, non sono in git.
- [x] 2026-10-08 Kiosk: pannello provvisorio sostituito dalla UX della fase 5.

## Fatto

- [x] 2026-10-08 **Un solo motore, un solo scenario, una sola modalità:**
  - lo scenario è pubblicato in `data/scenarios/rocca_ventosa` (`scenario_factory.py publish`) e `Scenario::load(data)` carica solo quello;
  - rimossi registro, build web, metadati dev e palette VR;
  - `rocca::Game` carica la libreria di comportamento di `data/behaviours` e parte con il fuoco già nello stato;
  - kiosk (`crates/game`): `sim.rs` è solo l'orologio intorno a `rocca::Game`, il nuovo `kiosk` è pianifica/esegui/fine con l'anteprima del piano, e la nuova partita si avvia solo dal pannello operatore (nessun reset per inattività); edifici e anelli seguono i fatti del `Game`;
  - rimossi `demo`, `play`, `text`, `chat`, `telemetry`, `tools/mcp`, gli script del vecchio gioco, gli scenari reali e quelli dev;
  - la squadra AIB viaggia su mezzo sulle carrozzabili: arrivo in 15 min invece di 59;
  - test del modello (`abm`, `fire`) portati su Rocca Ventosa da un subagente: eliminato il test della scialuppa (niente costa), ignorato `the_last_resort…` (vedi fase 4); workspace tutto verde (35 target).

- [x] 2026-10-08 **Fase 3 headless** (`docs/fase3/README.md`):
  - crate `rocca`: `Game`, piano, coordinatore, CLI A/B ed esempi `ab_sweep` e `civili`;
  - `game.json` della factory (`game-cases`): 9 casi, caserma VVF a Il Borgo e squadra AIB dalla SP 12;
  - roster esplicito, 2 autobotti + 1 squadra AIB;
  - preallerta reale in `abm`;
  - difesa fisica in `fire::exposure`;
  - faville che decadono con la distanza;
  - autobotti che pre-bagnano solo con il fronte entro 300 m;
  - 7 test di fase 3 verdi;
  - ETA dal percorso reale sulla rete aperta (13–14 min da Il Borgo a Il Piano per il passo da 8,9 km) e basi da `game.json`.

- [x] 2026-10-08 `cover.u8` collegato al kiosk: loader opzionale `scenario::Cover` (fallback se assente), niente piante su strade/case/orti (−332 piante), colore del suolo dalla copertura a 5 m in `terrain_mesh.rs`.

- [x] 2026-10-08 Risoluzione fine: `tools/factory/fine.py` (terreno a 5 m, sedi stradali, piazzole, `cover.u8`), router con direzione e costo dei tornanti (`roads.py`), tavola `docs/factory/fine/`. Il DEM del fuoco resta identico.
- [x] 2026-10-08 Strada del Passo rifinita: niente più sega di micro-tornanti, pendenza della sede al 14 % (era 25 % p95 sul DEM a 20 m).
- [x] 2026-10-08 `t4_paese` provato nel kiosk Bevy: si carica e gira a 60 FPS, terreno a 5 m incluso. Innesco e testi restano quelli di `demo_borgo`.

- [x] 2026-10-08 Fase 0, audit: `docs/audit.md` (commit `5d07e64`).
- [x] 2026-10-08 Fase 1, ambiente naturale: 4 candidati, sweep e atlante in `docs/factory/fase1/` (commit `f699873`).
- [x] 2026-10-08 `scenario_check` (`crates/abm/src/bin/scenario_check.rs`): verifica di uno scenario con il codice del modello.
- [x] 2026-10-08 Rifinitura di t4: erosione più leggera, niente parete artificiale sul versante SO.
- [x] 2026-10-08 Generatore del paese (`tools/factory/town.py`, `build-town`), layout 1 su t4. Tutti i controlli di `scenario_check` passano: 2 rifugi interni e 4 uscite.
- [x] 2026-10-08 Strade su percorso a costo minimo (`roads.py`, 9 %/14 %). Strada del Passo di 7,9 km.
- [x] 2026-10-08 Sweep accoppiati costruito/naturale con inneschi attorno ai paesi (`town-fires`), tavola e tabella di minaccia (`plate`) in `docs/factory/fase2/`.
