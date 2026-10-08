# Rocca Ventosa — todo list

Aggiornare a ogni iterazione (vedi `CLAUDE.md`). Fase corrente in cima.

## Stato

- **Fase corrente: fase 3, gameplay headless, consegnata** (2026-10-08, `docs/fase3/README.md`, crate `crates/rocca`).
- **Prossimo checkpoint umano: checkpoint 3.** Approvare coordinatore, preallerta, difesa fisica e le due correzioni del modello (faville a 700 m, autobotti che pre-bagnano solo con il fronte entro 300 m). Poi la fase 4.

## Decisioni dell'utente

- 2026-10-08: la difesa delle case deve **ridurre l'esposizione simulata**, non restare un proxy contabile (`Tally::note_defence`).
- 2026-10-08: incendi più vivaci e più lunghi. Vento di prova 40 km/h, 6 h simulate, umidità **3 %** fissa e uniforme. Evitare i bordi: mondo di 8 km con nucleo progettato di 4 km.
- 2026-10-08: terreno scelto **t4 (Crinale e sella)**.
- 2026-10-08: **checkpoint 2 approvato**, layout 1 di `t4_paese` così com'è. Affrontare la risoluzione fine **prima** della fase 3.
- 2026-10-08: **DTM, strade, case e rendering andranno a risoluzione più fine dei 20 m** del propagatore (che resta a 20 m). Avrà effetto sia sulla grafica sia sui sistemi di gioco. Da pianificare nelle prossime iterazioni.
- 2026-10-08: **il terreno fine serve solo a grafica e agenti, non all'incendio.** Il fuoco usa il DEM a 20 m approvato, invariato (non la media del terreno fine).
- 2026-10-08: fase 3 avviata senza aspettare gli screenshot di `cover.u8` ("segui il piano").
- 2026-10-08: **niente altro lavoro grafico ora**: seguire il piano, proseguire con sistemi e gameplay (fase 3).
- 2026-10-08: **risoluzione fine approvata** ("commit and push … continua"): terreno a 5 m e passo ritracciato a 8,9 km nello stesso corridoio. Avanti con `cover.u8` nel kiosk.

## Da fare

### Fase 2: insediamenti (in corso)

- [x] **Checkpoint umano 2** superato (2026-10-08). Ritocchi possibili più avanti, se servono per la giocabilità: Borgo più esposto (meno orti irrigui, bosco più vicino), passo più corto.
- [ ] Rivedere quali strade sono tagliafuoco: oggi solo la provinciale e le vie del paese sono non combustibili (vedi Borgo2 nella sezione sulla risoluzione fine).
- [ ] Popolazione: oggi tratti "compiacenti" copiati dal vecchio `demo_traits`. Va ripensata insieme alla preallerta (fase 3).

### Risoluzione fine (approvata il 2026-10-08)

- [ ] Verificare in fase 5 il colore del suolo da `cover.u8`: il kiosk mostra `t4_paese` travestito da `demo_borgo`, scenario dev con palette VR, che non usa `ground_color`. Le macchie chiare attorno alle case de Le Coste restano: vengono da un altro livello, da individuare.
- [ ] Primo piano nel kiosk di tornanti e piazzole (draping a livello di pixel).
- [ ] Strade come tagliafuoco: Borgo2 con vento da N supera o no la SP 12 secondo pochi metri di tracciato (spotting). Decidere se la provinciale è una fascia parafuoco più larga o solo una strada.

### Fase 3: gameplay headless (consegnata, in attesa del checkpoint)

- [ ] **Checkpoint umano 3** (`docs/fase3/README.md`).
- [ ] Kiosk su `rocca::Game`: rimuovere `game::sim.rs`, `demo::Session`, i turni e `demo::Referee` insieme alla nuova UX (fase 5).
- [ ] Squadra AIB: oggi viaggia a `CREW_SPEED` 3 m/s anche sulla provinciale, quindi arriva in circa 60 min dal bordo est. Decidere se si sposta su mezzo.
- [ ] Le Coste (cascine sparse) è indifendibile nei casi rapidi (Coste2), e 3 casi su 9 minacciano poco: scegliere i casi in fase 4.
- [ ] Preallerta come osservazione del grafo comportamentale (`HouseholdObs`): oggi agisce tramite consapevolezza, preparazione e ritardo dell'ordine.
- [ ] Difesa delle strade: misurare prima l'effetto sulla percorribilità (spec).
- [ ] Rivedere i parametri nuovi: decadimento delle faville `EMBER_DECAY_M` 700 m, protezione `PROTECTED_EMBER` 0,85 e `PROTECTED_RADIANT` 0,5, soglie del coordinatore.

### Fasi 4–6

- [ ] Crisi ×1, bilanciamento, nuova UX, playtest (vedi `03-PIANO-DI-AZIONE.md`).

### Debito tecnico

- [ ] `abm::incident_gaps::the_last_resort_profile_sends_people_to_open_ground` è rosso dopo la correzione delle faville (scenario reale Spotorno, fragile per sua stessa ammissione): rimuoverlo con gli scenari reali o ricostruirlo su `t4_paese`.
- [ ] 5 test rossi in `demo` sul vecchio gioco a turni (dal 2026-10-08 anche `lessons::l5_engines_and_crew_save_homes_only_on_the_fires_path`, dopo le correzioni di faville e autobotti) (`balance`, `lessons`, `playtest_feedback`): rimuovere insieme a `Session` in fase 3.
- [ ] `demo_borgo` ha 15 famiglie su celle combustibili (trovate da `scenario_check`). Irrilevante se `demo_borgo` viene rimosso.
- [ ] Il kiosk ha reset per inattività e rotazione dei paesi: da rimuovere con la nuova UX.
- [ ] `CLAUDE.md` cita `docs/rocca-ventosa/…`, ma i documenti sono in `docs/`.

## Fatto

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
