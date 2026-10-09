# Rocca Ventosa — todo list

Aggiornare a ogni iterazione (vedi `CLAUDE.md`). Fase corrente in cima.

## Stato

- **Fase corrente: fase 5, iterazione 5b**: criticità del playtest automatico 2, velocità tra i turni, leggibilità della vegetazione (decisione dell'utente, 2026-10-09). La lista ordinata è in «Iterazione 5b» qui sotto. Le scelte di dettaglio sono lasciate all'agente: misurare, decidere, scriverle qui.
- **Prossimo checkpoint umano: checkpoint 5** (`docs/fase5/README.md`): una partita al chiosco per caso (Coste2_gira, Piano2, Borgo2), con prima azione e debrief. Poi la fase 6 (playtest).
- **Consegna dell'iterazione 5b:**
  - una pagina `docs/fase5/iterazione5b.md` con file modificati, prima/dopo (schermate e tabella), durata della partita e FPS;
  - poi un nuovo playtest automatico con gli stessi quattro personaggi (`docs/fase5/playtest_gpt2/prompt/`). Copiare la cartella fuori dal repository e lanciare `run.sh`, con la build web servita su `localhost:8765`: `./scripts/build_web.sh` e `python3 -m http.server 8765 --directory target/web`.
- **Per riprendere:**
  - **Comandi:**
    - `cargo test --release --workspace` (35 target verdi);
    - `target/release/rocca <caso> --priorita A,B --b-priorita B,A`;
    - esempi `rocca`: `ab_sweep`, `crisi`, `civili`, `porta`;
    - kiosk: `KIOSK_CASE=Coste2_gira KIOSK_SPEED=300 KIOSK_WINDOWED=1 KIOSK_SHOT=<dir> target/release/game` (partita scriptata fino a «Altro incendio»);
    - kiosk: F2 apre la barra operatore; `KIOSK_FPS=1` scrive gli FPS; `KIOSK_VEG_DENSITY` regola la vegetazione (0,5 di serie); `KIOSK_SCALE=1` forza 1 pixel per punto.
  - **Territorio:** si rigenera con `scenario_factory.py build-town --layout 3`, `town-fires --scenario t4_paese3 --ignitions-from t4_paese2` e `publish --scenario t4_paese3`.

## Decisioni dell'utente

- 2026-10-09: **affrontare le criticità del playtest 2**, più due richieste:
  - **aumentare la velocità tra i turni** (fase Esegui);
  - **vegetazione più leggibile**, con una differenziazione grafica marcata: pini marittimi, castagni, cespugli e prateria riconoscibili.

  «Per il resto lascio le scelte a te.»
- 2026-10-09: **layout 3** («Procedi» alla proposta): macchia e bosco fino al margine sud-ovest di Castelvento, perché il paese possa essere colpito.
- 2026-10-08: **nessuna evacuazione a piedi dalle case isolate** («molto poco realistico»). Nei nuclei sparsi ogni famiglia ha almeno un'auto; in paese è senza auto circa 1 famiglia su 12 (`town.py::vehicles`, applicato anche a `population.json` pubblicato: nessun'altra estrazione casuale cambia).
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
- [ ] Popolazione: oggi tratti "compiacenti" copiati dal vecchio `demo_traits`. Va ripensata insieme alla preallerta (fase 3). Auto sistemate il 2026-10-08: case colpite invariate, famiglie colte in casa a Le Coste un po' meno (`docs/fase4/ab_sweep.md` e `crisi.md` rigenerati).

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

- [x] 2026-10-09 **Feedback del playtest umano: leggibilità e tempi** (`docs/fase5/playtest_feedback.md`): etichette per i nuclei separati di Le Ghiande, collegamenti alla scheda comune; legenda di numeri/barre/mezzi e combustibili; stato delle famiglie e stime di arrivo nelle schede; viaggio dell'evacuazione dai percorsi reali (distinto da avviso, preparazione e code); aggiornamenti recenti sempre visibili e cronologia completa; indicazioni persistenti e pulsante «Pausa e modifica piano». Verificati test ABM/Rocca e build nativa; controllo visivo con partita scriptata.
- [x] 2026-10-09 (utente) Proporzioni da diorama, solo grafica (`crates/game/src/visual_scale.rs`): edifici ×1,9 in pianta e ×2,5 in altezza, auto ×3, persone ×4,5, mezzi ×7, alberi ×1,6, arbusti ×1,25. Coordinate, fuoco e percorsi invariati.
- [x] 2026-10-09 **Playtest automatico 2** (`docs/fase5/playtest_gpt2/README.md`): quattro personaggi GPT (nuova, quattordicenne, min-maxer, UX). Nessuna strategia dominante su Coste2_gira: AFK 16 / 73, evacua tutti 3 / 71, preallerta e poi cambio piano 1 / 73 (famiglie / case).
#### Iterazione 5b (2026-10-09, in quest'ordine)

1. [x] 2026-10-09 **Velocità tra i turni** (`docs/fase5/durata.md`):
   - ×40 di base, ×120 nei tratti quieti; crisi a ×1 e countdown invariati. Spec §2 e `00-LEGGIMI-PRIMA.md` aggiornati.
   - Esecuzione dimezzata: Coste2_gira da 5,8 a 3,3 minuti reali, Piano2 da 3,6 a 1,8, Borgo2 da 4,1 a 2,1.
   - Corretto `Sim::tick`: un frame lungo eseguiva step oltre la crisi e oltre la fine del caso. Test `game::sim::tests::the_speed_does_not_change_the_game`: a ×20, a ×120 e con frame lenti, stesse crisi e stesso esito.
   - FPS invariati tra ×0 e ×120. Nessuna frase dei personaggi persa.
   - [ ] Da confermare nel playtest 3: Piano2 a 1,8 minuti potrebbe risultare troppo breve per seguire il fuoco.
2. [x] 2026-10-09 **Vegetazione riconoscibile** (`docs/fase5/iterazione5b.md` §2), solo grafica:
   - nuovi modelli `pine` (pino marittimo) e `chestnut` (castagno) in `scripts/build_models.py`, `bush` più basso;
   - un solo modello per gruppo, colori distinti per gruppo e per classe, tinte del suolo per gruppo;
   - legenda con miniature e colori campionati dal rendering.
   - Piante invariate (551 mila), triangoli da 34,0 a 32,4 M. FPS nativi invariati (mediana 30 → 32).
   - [ ] FPS nel browser da misurare a finestra in primo piano: la scheda automatizzata in secondo piano viene rallentata da Chrome.
3. [x] 2026-10-09 **Stima d'arrivo dallo stato reale** (`iterazione5b.md` §3–4):
   - `Game::arrivals` durante Esegui; anteprima solo con un piano da confermare;
   - «~2 min» quando gli estremi coincidono;
   - test `fase3::arrivals_follow_the_units`.
4. [x] 2026-10-09 **Motivo nella scheda:**
   - senza mezzi, la scheda dice perché, da `uncovered` / priorità / minaccia;
   - nell'anteprima, «X tornerà alla base» per i mezzi che il nuovo piano richiama.
5. [x] 2026-10-09 **Debrief con il perché** (`iterazione5b.md` §5):
   - X/Y accanto alle barre;
   - «Perché» da `Game::story` (ordini, partenze, mezzi in postazione, fuoco alla porta);
   - test `fase3::the_story_tells_what_happened` e `docs/fase5/debrief_righe.md`.
6. [x] 2026-10-09 **Legenda** (`iterazione5b.md` §6):
   - sezioni apribili: in cima vegetazione e barra delle famiglie, «Schede dei paesi» chiusa;
   - «famiglie senza via»; testo dei tempi a ×40;
   - chiusa all'avvio, quindi non copre Pian dei Grilli. `KIOSK_LEGEND=1` la apre per le schermate.
7. [ ] **Schede che si coprono** (scelta all'agente):
   - collisioni tra schede anche con lo zoom;
   - schede compatte durante Esegui;
   - fondo più opaco e testo secondario più leggibile.
8. [ ] **Ritmo a metà partita** (scelta all'agente):
   - la volontaria dice un fatto nuovo (prossimo arrivo, fronte più vicino, famiglie ancora a casa) invece di ripetere il tutorial;
   - «Altro incendio» si presenta con una frase su dove parte e verso chi va (dati del caso: `near`, vento).
9. [ ] Minori:
   - [x] 2026-10-09 «1 mezzi su 2» → «1 mezzo su 2» (coordinatore) e «Resta 1 mezzo» (crisi del mezzo perso);
   - [ ] etichette dei mezzi coperte dal pannello aggiornamenti.

- [ ] 2026-10-09 Riprovare con persone: comprensione delle stime di viaggio (non tempo garantito di evacuazione), legenda, etichette dei nuclei e aggiornamenti; verificare il layout su touchscreen e a 900×600.

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
- [x] 2026-10-08 Secondo playtest GPT 6.1 Sol, «roast da Redditor sedicenne» (`docs/fase5/playtest_gpt/roast.md`). Corretti:
  - pannello del coordinatore in gioco: ora mostra lo stato attuale dei mezzi, non i motivi vecchi;
  - chi resta a casa dopo l'ordine e perché (misurato: circa il 22 % del Borgo aspetta di vedere il fuoco o difende la casa; è comportamento del modello, non un bug);
  - ritiro e perdita dei mezzi registrati con ora e luogo, anche nel debrief;
  - «metti per primo»;
  - pannelli più opachi;
  - «T+0:42 di 3:00»;
  - crisi scaduta registrata.
- [x] 2026-10-08 Terzo playtest GPT 6.1 Sol, «roast» su UX e grafica (`docs/fase5/playtest_gpt/roast_ux.md`). Ha giocato in parte su una versione vecchia rimasta nella cache del browser: ora la build mette una versione negli URL. Corretti:
  - **pannelli e schede:** le schede evitano i pannelli fissi; linea che collega ogni scheda al suo luogo; pannelli richiudibili;
  - **comandi:** un'unica famiglia di pulsanti (secondari grigi, ordini colorati quando attivi, giallo solo per l'azione principale);
  - **contrasto:** distanza del fuoco e mezzi in difficoltà in etichette colorate su fondo pieno;
  - **legenda** dei segni sulla mappa;
  - **crisi:** in un solo riquadro (testo, «Metti X per primo», «Preallerta/Evacua X», effetto sui mezzi, Conferma) e quartiere evidenziato;
  - **camera:** trascinamento ricavato dal cursore (prima nel browser non funzionava), pulsanti + e −, istruzioni;
  - **debrief:** opaco, due numeri grandi in testa, definizioni richiudibili.
- [x] 2026-10-09 Roast a tema, sei personaggi in sequenza (`docs/fase5/playtest_gpt/p*`).
  - Completati: volontario AIB (realismo), studente daltonico (accessibilità), volontario al chiosco (gestione).
  - Rifatti, perché il profilo Chrome era occupato dalle mie schede di prova: min-maxer, quattordicenne, insegnante.
  - Corretti:
    - stato dei mezzi con «fuoco a N m» e «acqua N%», in arancione prima della ritirata;
    - «finisce il lavoro in corso, poi rientra» al posto di «al lavoro» senza postazione;
    - nota «Da ricordare» su cosa fare davvero all'ordine di evacuazione (debrief e passando sulla riga «a casa»);
    - pausa: «IN PAUSA» e «Conferma e riprendi»;
    - operatore:
      - «Prossima partita» distinta dalla partita in corso;
      - «Nuova partita» chiede una seconda pressione;
      - secondi delle crisi regolabili (25/40/60);
      - la legenda va sotto la barra F2, chiusa sugli schermi piccoli.
- [x] 2026-10-09 Roast rifatti: min-maxer, quattordicenne, insegnante. Corretti:
  - introduzione in tre passi brevi, legenda chiusa all'avvio;
  - i pulsanti delle schede non si spostano più;
  - «da confermare: evacuazione, riceve N mezzi» accanto agli ordini scelti;
  - «fuoco tra le case» al posto di «0,0 km»;
  - conteggi delle famiglie completi («raggiunte dal fuoco»);
  - debrief con il perché per paese e le definizioni sempre visibili.
- [x] 2026-10-09 (utente: «continua con le tue soluzioni») **Costo dell'evacuazione misurato** (`examples/costi.rs`, `docs/fase5/costi_evacuazione.md`). I costi esistono già nel modello (i mezzi rallentati dal traffico, le famiglie esposte in strada), ma con evacuazioni precoci non incidono: i mezzi sono in postazione a T+12 e nessuno viene colto in strada. Conta invece **quando** si evacua: tutto a T+0 lascia 3 famiglie colte in casa, a T+40 10, a T+90 15. Nessuna penalità inventata. Il debrief mostra l'orario degli ordini di evacuazione e le evacuazioni precauzionali.
- Misura originale: Misura su Coste2_gira, seme 1 (`target/release/rocca`):
  - nessun ordine: 73 case colpite, 16 famiglie colte in casa;
  - Il Piano per primo: 27 / 17;
  - evacuare tutto: 73 / 3;
  - tutte e due le cose: 27 / 3.
  
  La priorità decide le case, l'evacuazione le persone, e gli effetti si sommano. «Evacua tutto a T+0» è quindi sempre conveniente (anche al Borgo, dove il fuoco non arriva). Ipotesi da valutare con misura, senza penalità inventate:
  - traffico dell'evacuazione che rallenta i mezzi sulle stesse strade;
  - famiglie in strada esposte se il fuoco taglia il percorso;
  - debrief che mostri le famiglie spostate dove il fuoco non è arrivato.
- [x] 2026-10-09 **Durata:** accelerazione automatica ×3 quando nulla cambia (`Game::is_quiet`: 10 min senza eventi e nessun mezzo in movimento), le crisi restano a ×1. Partita da 9–10 a **4–6 minuti reali** (`examples/durata.rs`, `docs/fase5/durata.md`).
- Prima: Partita di circa 10 minuti reali a ×20, con tratti morti dopo gli ordini (quattordicenne). Opzioni: ×30-40 tra una decisione e l'altra, partita di 2 ore simulate invece di 3, accelerazione automatica quando non c'è nulla da decidere.
- [x] 2026-10-09 (utente) **Schermata introduttiva con logo CIMA e interfaccia adatta al touch** (`docs/fase5/img/intro.jpg`):
  - **schermata iniziale:** situazione, tre passi, «Inizia», nota su PROPAGATOR e Fondazione CIMA; compare all'avvio e dopo «Nuova partita» dell'operatore, non dopo «Riprova» o «Altro incendio»;
  - **comandi:** pulsanti alti almeno 44 px e testo più grande;
  - **camera:** un dito sposta la mappa, due dita fanno lo zoom;
  - **niente spiegazioni solo al passaggio del mouse;**
  - **pagina web:** niente zoom del browser né selezione del testo sul canvas.
  - **Non ancora provato su un touchscreen vero.**
- [x] 2026-10-09 (utente: «troppo testo, gui più grafica») Interfaccia a icone disegnate (`kiosk/icons.rs`):
  - **schede:** cerchio con il numero, fuoco con la distanza, autobotti come icone (piene, da ricevere, barrate se le perde), barra delle famiglie;
  - **pulsanti:** icona più una parola;
  - **mezzi:** badge con icona, sigla, livello dell'acqua e fiamma vicina;
  - **barra in alto:** vento, tempo come barra, stato come icona;
  - **crisi:** countdown ad anello;
  - **finale:** due numeri grandi con «−N grazie a te», barre tu contro senza ordini, linea del tempo con le decisioni;
  - **pannelli laterali:** chiusi all'avvio.
- [x] 2026-10-09 (utente) Località rinominate: Il Borgo → **Castelvento**, Il Piano → **Pian dei Grilli**, Le Coste → **Le Ghiande** (dati, Factory, test). I nomi dei casi (Borgo1, Coste2_gira…) restano invariati come identificativi interni.
- [x] 2026-10-09 **Castelvento colpibile: layout 3** (`docs/fase5/castelvento_layout3.md`, `tools/factory/town.py::T4_L3`). Macchia e latifoglie fino a 20 m dal margine sud-ovest; cambia solo `fuel.i32` (513 celle).
  - **Correzione:** la nota precedente («0 case in tutti i 18 casi») era sbagliata. Già prima Borgo2 colpiva 31 case senza difesa e Borgo1 3.
  - **Ora, Borgo2:** senza ordini ai civili 18 famiglie colte in casa (prima 5); con preallerta o evacuazione 3 (prima 0).
  - **Invariati:** Coste2_gira, Piano2 e 14 casi su 18. Rapporti `docs/fase4/{ab_sweep,crisi,porta}.md` rigenerati.
- [ ] Borgo3 non raggiunge Castelvento: 600 m di latifoglie umide, 11 ha in 3 h. Estendere le latifoglie secche a 750 m non cambia nulla (provato, annullato). Un caso con innesco nella macchia a sud-ovest richiede un nuovo innesco: da decidere con l'utente.
- [x] 2026-10-09 (utente) **Personaggi che parlano**, in basso a sinistra con fumetto (`crates/game/src/kiosk/characters.rs`). Ritratti generati da Codex (gpt-6.1-sol, generazione immagini), ridotti a 256 px in `assets/characters/`:
  - **sindaca**: ordini di preallerta ed evacuazione, famiglie raggiunte in casa, crisi «quartiere scoperto»;
  - **caposquadra VVF (DOS)**: mezzi che si ritirano o vanno fuori servizio, crisi «mezzo perso»;
  - **previsore meteo**: cambi di vento, crisi «previsione» e «vento»;
  - **volontaria di protezione civile**: intro, consiglio di partenza, «nella realtà» del debrief.
  Ogni frase nasce da una voce del registro, da una crisi o dall'esito già prodotti dal motore: nessuna regola nuova. La crisi ora si legge nel fumetto, con countdown e risposte subito sotto; proposta del coordinatore ed eventi spostati in basso a destra. Immagine: `docs/fase5/img/personaggi_crisi.jpg`.
- [ ] Personaggi: verificare in un playtest che il fumetto in basso a sinistra si noti durante la crisi (prima era in alto al centro) e che le frasi a ×20 non si accavallino (coda di 2, minimo 2,5 s ciascuna).
- [ ] Dai roast, ancora aperto: navigazione da tastiera (egui sul canvas), forme oltre ai colori per gli anelli, layout a finestra stretta (900×600), stato perso con la ricarica della pagina, verifica dei percorsi verso le aree di attesa, supporto ai ritardatari dell'evacuazione (scelta di design).
- [ ] Dal playtest GPT, ancora aperto: icone dei mezzi leggibili nella vista generale (oggi si leggono le etichette); indicatori aggregati per le evacuazioni sulla mappa; click che a volte sembrano non rispondere (forse solo per l'input automatizzato, da verificare con persone vere); distinguere mezzi assegnati / in viaggio / sul posto; mostrare sulla mappa perché una postazione è irraggiungibile.
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
