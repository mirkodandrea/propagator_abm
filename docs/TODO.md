# Rocca Ventosa — todo list

Aggiornare a ogni iterazione (vedi `CLAUDE.md`). Fase corrente in cima.

## Stato

- **Fase corrente:** checkpoint 2 superato (layout 1 di `t4_paese` approvato il 2026-10-08). **Prossimo lavoro: risoluzione fine** (sezione sotto), poi fase 3.
- **Prossimo checkpoint umano:** a fine lavoro sulla risoluzione fine, con tavola e confronto 20 m / risoluzione fine.

## Decisioni dell'utente

- 2026-10-08: la difesa delle case deve **ridurre l'esposizione simulata**, non restare un proxy contabile (`Tally::note_defence`).
- 2026-10-08: incendi più vivaci e più lunghi. Vento di prova 40 km/h, 6 h simulate, umidità **3 %** fissa e uniforme. Evitare i bordi: mondo di 8 km con nucleo progettato di 4 km.
- 2026-10-08: terreno scelto **t4 (Crinale e sella)**.
- 2026-10-08: **checkpoint 2 approvato**, layout 1 di `t4_paese` così com'è. Affrontare la risoluzione fine **prima** della fase 3.
- 2026-10-08: **DTM, strade, case e rendering andranno a risoluzione più fine dei 20 m** del propagatore (che resta a 20 m). Avrà effetto sia sulla grafica sia sui sistemi di gioco. Da pianificare nelle prossime iterazioni.

## Da fare

### Fase 2: insediamenti (in corso)

- [x] **Checkpoint umano 2** superato (2026-10-08). Ritocchi possibili più avanti, se servono per la giocabilità: Borgo più esposto (meno orti irrigui, bosco più vicino), passo più corto.
- [ ] Rivedere quali strade sono tagliafuoco: oggi solo la provinciale e le vie del paese sono non combustibili. Da rivedere con la risoluzione fine.
- [ ] Rifinire la Strada del Passo: tratto rettilineo artificiale verso y ≈ 2950, pendenze p95 del 25 % sul DEM a 20 m (manca il rilevato stradale).
- [ ] Popolazione: oggi tratti "compiacenti" copiati dal vecchio `demo_traits`. Va ripensata insieme alla preallerta (fase 3).
- [ ] Provare `t4_paese` nel kiosk Bevy (mondo da 8 km, mai provato).

### Risoluzione fine (PROSSIMO LAVORO, deciso il 2026-10-08)

- [ ] Separare la griglia del fuoco (20 m) dalla griglia di DTM, render, strade e case (per esempio 5 m). Il formato prevede già `render_terrain` con un proprio `posting_m`.
- [ ] La Factory genera il DTM fine e lo ricampiona a 20 m per il propagatore. Combustibile, strade e case vanno rasterizzati coerentemente sulle due griglie.
- [ ] Verificare gli effetti sui sistemi di gioco: rete stradale e lunghezze dei link, posizioni di case e famiglie, minaccia ed esposizione campionate a 20 m, draping di strade e marker (TECHNICAL-FINDINGS 11–13).
- [ ] Verificare il rendering: mesh a 8 km × 5 m (2,56 M vertici) contro le prestazioni del kiosk. Valutare LOD o un render limitato al nucleo.

### Fase 3: gameplay headless

- [ ] Unificare il runner (una sola autorità, `demo::Run` come base). Rimuovere il driver duplicato `game::sim.rs`.
- [ ] Roster configurabile: 2 autobotti + 1 squadra AIB. Oggi è fisso a 3 + 3 + 2 tanker e le basi coincidono con i rifugi, cioè anche con le uscite a bordo mappa.
- [ ] Coordinatore minimo: postazioni, ETA, inerzia, traccia del motivo.
- [ ] Difesa che riduce l'esposizione reale (decisione 2026-10-08).
- [ ] Preallerta distinta dall'evacuazione in `abm` + `behavior`.
- [ ] Piano attivo e piano proposto, comparatore A/B a parità di seed.

### Fasi 4–6

- [ ] Crisi ×1, bilanciamento, nuova UX, playtest (vedi `03-PIANO-DI-AZIONE.md`).

### Debito tecnico

- [ ] 4 test rossi in `demo` sul vecchio gioco a turni (`balance`, `lessons`, `playtest_feedback`): rimuovere insieme a `Session` in fase 3.
- [ ] `demo_borgo` ha 15 famiglie su celle combustibili (trovate da `scenario_check`). Irrilevante se `demo_borgo` viene rimosso.
- [ ] Il kiosk ha reset per inattività e rotazione dei paesi: da rimuovere con la nuova UX.
- [ ] `CLAUDE.md` cita `docs/rocca-ventosa/…`, ma i documenti sono in `docs/`.

## Fatto

- [x] 2026-10-08 Fase 0, audit: `docs/audit.md` (commit `5d07e64`).
- [x] 2026-10-08 Fase 1, ambiente naturale: 4 candidati, sweep e atlante in `docs/factory/fase1/` (commit `f699873`).
- [x] 2026-10-08 `scenario_check` (`crates/abm/src/bin/scenario_check.rs`): verifica di uno scenario con il codice del modello.
- [x] 2026-10-08 Rifinitura di t4: erosione più leggera, niente parete artificiale sul versante SO.
- [x] 2026-10-08 Generatore del paese (`tools/factory/town.py`, `build-town`), layout 1 su t4. Tutti i controlli di `scenario_check` passano: 2 rifugi interni e 4 uscite.
- [x] 2026-10-08 Strade su percorso a costo minimo (`roads.py`, 9 %/14 %). Strada del Passo di 7,9 km.
- [x] 2026-10-08 Sweep accoppiati costruito/naturale con inneschi attorno ai paesi (`town-fires`), tavola e tabella di minaccia (`plate`) in `docs/factory/fase2/`.
