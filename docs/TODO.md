# Isola di Roccavento — lista di lavoro

Va aggiornata a ogni iterazione, nello stesso commit del lavoro (vedi `CLAUDE.md`). Le voci chiuse si cancellano: la storia è in git. Le fasi sono in `docs/03-VERIFICHE.md`.

## Fase corrente: 1. Prima passata (nuovo progetto, 2026-10-14)

**Prossimo checkpoint:** l'utente rivede motore, chiosco e documenti, e dice le modifiche.

### Fatto in questa passata (da rivedere)

- `rocca` riscritto: `game` (pausa ×60 / ×300, ricarica, ordini solo in pausa), `hex` (500 m, solo esagoni con strada), `evac` (evacuazione semplificata, un borgo alla volta, 20 persone/min, rerouting, interruzione e autoprotezione), `tutorial` (copione), `score` (Persone, Case, Ettari), `forecast` (alla pausa, con il bollettino). Tolti `coordinator`, `crisis`, `mayor`, `messengers`, `plan`, gli esempi e i test vecchi.
- `game.json`: 3 VVF a Castelvento e 2 AIB a San Rocco (3 persone per squadra), caso `tutorial`, `game: medium` a 300 min, regole dell'evacuazione.
- Chiosco: flusso Apertura → Tutorial → Giorno 2 → Gioco → Fine; `kiosk/ui.rs`, `kiosk/marks.rs` (esagoni, percorsi, strade chiuse), `kiosk/view.rs` (clic sulla mappa), `kiosk/snap.rs` nuovi. Tolti `people`, `life`, `agents`, i personaggi e il vecchio tutorial.

### Aperto

- [ ] Apertura: segnalazione dopo 30 min (`start_min`, tabella in `02-TERRITORIO.md`). Da completare: seme 3, la strategia «evacua tutto», piani migliori per le Case.
- [ ] Molte persone «colpite» presto a Le Terrazze (18 alle 14:40): la soglia `ALARMING` per chi è a casa senza ordine è severa. Va decisa con il bilanciamento.
- [ ] Il tutorial contiene l'incendio, ma non lo spegne del tutto: restano poche celle accese e la prova finisce alla durata del caso (150 min, circa 30 s reali). Valutare la fine a fuoco «contenuto».
- [ ] Le squadre vanno spesso a rifornirsi (idranti lontani): verificare che non sembri un difetto.
- [ ] Grafica:
  - esagoni poco visibili sulla terra;
  - percorso dell'evacuazione poco leggibile da lontano;
  - le famiglie in strada non si vedono;
  - le icone delle squadre sono etichette egui: valutare il trascinamento della squadra sull'esagono.
- [ ] Soglie delle stelle provvisorie (`score::{PERSONE, CASE, ETTARI}`).
- [ ] Persone a piedi delle squadre (linee di taglio verso il fuoco): da vedere a schermo (`units.rs` le disegna come squadra AIB a piedi) e da misurare quanto contano rispetto all'acqua.
- [ ] Mario scollegato: va ricollegato, ma per ultimo (utente, 2026-10-14).
- [ ] Il crate `abm` contiene ancora il modello IBM delle persone (`Abm`, traffico, comportamenti), non più usato dal gioco. Da togliere, tenendo `network`, `drive`, `suppression`, `refuge`.
- [ ] `scripts/playtest.py` va aggiornato al nuovo flusso (lo stato pubblicato è cambiato).
- [ ] Versione web non provata (`scripts/build_web.sh`): il confronto senza ordini si calcola alla fine, nel frame.

## Test rotti noti

- `abm/tests/incident_gaps.rs`: 5 test pensati per un territorio senza mare.
- `game models::tests::the_island_models_stand_on_the_ground` (punto più basso della rocca a −2,6 m).

## Decisioni in vigore (con data)

- 2026-10-14: l'incendio della partita parte già sviluppato (segnalazione al minuto 30). Le squadre attaccano il fuoco come possono: le persone a piedi tagliano la vegetazione verso il fuoco, l'autobotte bagna il combustibile che raggiunge. Mario va ricollegato per ultimo.
- 2026-10-14: **nuovo progetto.** Si riscrive `rocca` dentro questo repository. Partita in due parti, tutorial scriptato (giorno 1) e partita (giorno 2). Tempo reale ×300, pausa ×60 di 30 s reali con ricarica di 20 min simulati, ordini solo in pausa, previsione dell'esperto in pausa. Squadre = autobotte + 3 persone: 3 VVF nel borgo più grande, 2 volontari in un borgo minore. Esagoni di 500 m adiacenti alle strade. Evacuazione semplificata, un borgo alla volta, con limite di persone al minuto. Niente IBM delle persone. Stelle e classifica tenute: persone, poi case, poi ettari. Comunicazione da riscrivere dopo.
- 2026-10-13: portata dello spegnimento ×20 (`game::SUPPRESSION` = linea 3, acqua 30, portata 20).
- 2026-10-13: territorio dell'isola con valico e costa est rifatta (`02-TERRITORIO.md`).
- 2026-10-12: schermo del chiosco di almeno 1080p.
