# Rocca Ventosa — todo list

Aggiornare a ogni iterazione (vedi `CLAUDE.md`). Fase corrente in cima.

## Stato

- **Fase corrente:** risoluzione fine approvata (2026-10-08, `docs/factory/fine/README.md`). **Prossimo lavoro: collegare `cover.u8` al kiosk** (niente piante su strade, case e orti; colore del suolo dalla copertura a 5 m), poi fase 3.
- **Prossimo checkpoint umano:** screenshot del kiosk prima/dopo il collegamento di `cover.u8`.

## Decisioni dell'utente

- 2026-10-08: la difesa delle case deve **ridurre l'esposizione simulata**, non restare un proxy contabile (`Tally::note_defence`).
- 2026-10-08: incendi più vivaci e più lunghi. Vento di prova 40 km/h, 6 h simulate, umidità **3 %** fissa e uniforme. Evitare i bordi: mondo di 8 km con nucleo progettato di 4 km.
- 2026-10-08: terreno scelto **t4 (Crinale e sella)**.
- 2026-10-08: **checkpoint 2 approvato**, layout 1 di `t4_paese` così com'è. Affrontare la risoluzione fine **prima** della fase 3.
- 2026-10-08: **DTM, strade, case e rendering andranno a risoluzione più fine dei 20 m** del propagatore (che resta a 20 m). Avrà effetto sia sulla grafica sia sui sistemi di gioco. Da pianificare nelle prossime iterazioni.
- 2026-10-08: **il terreno fine serve solo a grafica e agenti, non all'incendio.** Il fuoco usa il DEM a 20 m approvato, invariato (non la media del terreno fine).
- 2026-10-08: **risoluzione fine approvata** ("commit and push … continua"): terreno a 5 m e passo ritracciato a 8,9 km nello stesso corridoio. Avanti con `cover.u8` nel kiosk.

## Da fare

### Fase 2: insediamenti (in corso)

- [x] **Checkpoint umano 2** superato (2026-10-08). Ritocchi possibili più avanti, se servono per la giocabilità: Borgo più esposto (meno orti irrigui, bosco più vicino), passo più corto.
- [ ] Rivedere quali strade sono tagliafuoco: oggi solo la provinciale e le vie del paese sono non combustibili (vedi Borgo2 nella sezione sulla risoluzione fine).
- [ ] Popolazione: oggi tratti "compiacenti" copiati dal vecchio `demo_traits`. Va ripensata insieme alla preallerta (fase 3).

### Risoluzione fine (approvata il 2026-10-08)

- [ ] Collegare `cover.u8` al kiosk: piante non su strade, case e orti; colore del suolo dalla copertura a 5 m (`vegetation.rs`, `terrain_mesh.rs`, loader in `scenario`).
- [ ] Primo piano nel kiosk di tornanti e piazzole (draping a livello di pixel).
- [ ] Strade come tagliafuoco: Borgo2 con vento da N supera o no la SP 12 secondo pochi metri di tracciato (spotting). Decidere se la provinciale è una fascia parafuoco più larga o solo una strada.
- [ ] Fase 3: ritarare ETA e basi sul passo da 8,9 km. Le quote degli agenti seguono già le sedi stradali (`Terrain::height_at`).

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
