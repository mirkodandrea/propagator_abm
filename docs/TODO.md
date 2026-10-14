# Isola di Roccavento — lista di lavoro

Va aggiornata a ogni iterazione, nello stesso commit del lavoro (vedi `CLAUDE.md`). Le voci chiuse si cancellano: la storia è in git. Le fasi sono in `docs/03-VERIFICHE.md`.

## Fase corrente: 1. Prima passata (nuovo progetto, 2026-10-14) — fatta, commit `ab7d0bf`

**Prossimo checkpoint:** il bilanciamento (fase 2). Si parte dalla tabella qui sotto, poi decide l'utente. Le specifiche originali sono in `docs/00-SPECIFICHE.md`.

### Da fare, in ordine

1. [ ] **Tabella di bilanciamento** (`target/release/rocca medium --seme N ...`), semi 1–3, con ritardo di 30 min. Strategie:
   - «nessun ordine»;
   - «tutte le squadre sull'innesco subito»;
   - «evacua tutto, un borgo dopo l'altro, appena possibile»;
   - «piano ragionato».

   Nessuna deve dominare. Oggi il piano ragionato prende Persone 5, Case 2, Ettari 2–3 (semi 1 e 2). Per ripetere le prove conviene un esempio `examples/strategie.rs` che le giochi tutte e stampi la tabella.
2. [ ] **Soglia delle persone colpite:** chi è a casa senza ordine viene contato «colpito» quando il pericolo alla porta arriva a `ALARMING`. È severa: 65 persone a Le Terrazze già alle 14:55. Proporre `IMPASSABLE` o la casa colpita, poi decide l'utente.
3. [ ] **Soglie delle stelle** (`score::{PERSONE, CASE, ETTARI}`), da ricavare dalla tabella del punto 1.
4. [ ] **Persone a piedi delle squadre:** vederle a schermo con `KIOSK_SNAP`, e misurare quanto conta il taglio rispetto all'acqua (partite con e senza `crews_follow_the_fire`).
5. [ ] **Tutorial:** contiene l'incendio ma non lo spegne del tutto (restano poche celle accese) e finisce alla durata del caso (150 min, circa 30 s reali). Valutare la fine a fuoco «contenuto», per esempio senza celle che avanzano.
6. [ ] **Grafica e UX:**
   - esagoni poco visibili sulla terra;
   - percorso dell'evacuazione poco leggibile da lontano;
   - famiglie in strada invisibili;
   - trascinamento delle squadre e timer dell'evacuazione fatti (2026-10-14) ma provati solo con `KIOSK_SNAP`: il drag vero va provato a mano; il timer non conta il viaggio dell'ultima famiglia;
   - le squadre vanno spesso a rifornirsi (idranti lontani): verificare che non sembri un difetto.
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

- 2026-10-14: gli ordini si danno sulla mappa: squadre trascinate sull'esagono, borghi cliccati, «Conferma» in un riquadro accanto; la barra laterale è solo informativa. Timer circolare sul borgo in evacuazione.
- 2026-10-14: l'incendio della partita parte già sviluppato (segnalazione al minuto 30). Le squadre attaccano il fuoco come possono: le persone a piedi tagliano la vegetazione verso il fuoco, l'autobotte bagna il combustibile che raggiunge. Mario va ricollegato per ultimo.
- 2026-10-14: **nuovo progetto.** Si riscrive `rocca` dentro questo repository. Partita in due parti, tutorial scriptato (giorno 1) e partita (giorno 2). Tempo reale ×300, pausa ×60 di 30 s reali con ricarica di 20 min simulati, ordini solo in pausa, previsione dell'esperto in pausa. Squadre = autobotte + 3 persone: 3 VVF nel borgo più grande, 2 volontari in un borgo minore. Esagoni di 500 m adiacenti alle strade. Evacuazione semplificata, un borgo alla volta, con limite di persone al minuto. Niente IBM delle persone. Stelle e classifica tenute: persone, poi case, poi ettari. Comunicazione da riscrivere dopo.
- 2026-10-13: portata dello spegnimento ×20 (`game::SUPPRESSION` = linea 3, acqua 30, portata 20).
- 2026-10-13: territorio dell'isola con valico e costa est rifatta (`02-TERRITORIO.md`).
- 2026-10-12: schermo del chiosco di almeno 1080p.
