# Capacità di evacuazione globale (2026-10-13)

Richiesta dell'utente: un massimo di famiglie evacuate insieme, come **capacità globale in famiglie/minuto** ripartita tra i quartieri in evacuazione (polizia locale agli incroci, pullman per chi non ha l'auto).

**Modello** (`abm::Abm::set_exit_capacity`, `game.json` → `exit_per_min`, **assente = nessun limite, com'è ora nel chiosco**). Una famiglia che ha ricevuto l'ordine ed è pronta si mette in coda (la prima pronta esce per prima). La coda esce al ritmo della capacità, con al massimo un minuto di partenze accumulate. Non aspetta il turno chi parte senza l'ordine, chi scappa perché il fuoco è alla porta e, nella versione B, chi vede il fuoco abbastanza da partire da solo. Chi aspetta resta a casa ed è esposto come chi non è partito. Test: `fase3::the_exit_capacity_limits_the_departures_on_the_order`.

**Partenze senza limite** (`examples/partenze.rs`, `partenze.md`): nel minuto di punta 24–48 famiglie, nei 10 minuti più carichi 110–130. Con 6/min (`partenze_6.md`) restano in coda fino a 50–59 famiglie, cioè ~10 minuti di attesa.

**Prova**: `examples/sindaca.rs` con `USCITA=<cap>`, 82 incendi (36 casi × semi 1–3, solo quelli in cui il fuoco tocca qualcuno), sindaca a 20 min; riassunto con `riassunto.py <cartella>`. Punti medi:

| capacità | A: chi vede il fuoco aspetta comunque il turno | | | | B: chi vede il fuoco parte da solo | | | |
|---|---|---|---|---|---|---|---|---|
| | evacua tutti subito (vecchie regole) | preallerta tutti + evacua aut. | solo i minacciati | solo preallerta | evacua tutti subito | preallerta tutti + evacua aut. | solo i minacciati | solo preallerta |
| nessuna | 624 | 621 | 636 | 532 | 624 | 621 | 636 | 532 |
| 10/min | 622 | 619 | 634 | 532 | 624 | 620 | 636 | 532 |
| 6/min | 589 | 612 | 625 | 532 | 623 | 619 | 634 | 532 |
| 4/min | 455 | 591 | 595 | 532 | 621 | 619 | 634 | 532 |
| 2/min | 214 | 448 | 469 | 532 | 602 | 605 | 633 | 532 |

Dati: `attesa/` (A) e `scappa/` (B).

**Cosa si vede**
- **B (realistica)**: la capacità quasi non conta, nemmeno a 2/min. Chi è in pericolo vede il fuoco e parte da solo. In coda restano soltanto le famiglie lontane dal fronte, a cui l'attesa non costa nulla. È lo stesso risultato della fase 5 (`propagator-painted-roads-immune`): a questa scala il traffico non è mai il collo di bottiglia. Nessuno viene colto in strada in nessuna prova.
- **A**: la capacità pesa molto su «evacua tutti subito», che perde 35 punti a 6/min e 170 a 4/min; con la sindaca però quel piano non è più giocabile. Sui piani con la sindaca pesa poco fino a 6/min. Sotto i 4/min evacuare diventa *peggio* che preallertare soltanto (448 contro 532 a 2/min), perché chi ha ricevuto l'ordine resta a casa ad aspettare mentre chi è solo preallertato scappa da solo. È una lezione sbagliata da dare agli studenti.
- In nessuna delle due versioni cresce il distacco tra «preallerta tutti, evacua quando autorizzato» e «solo i minacciati» (34–36 incendi su 82 con più di 20 punti di scarto, contro 34 senza limite). La capacità non crea la scelta «chi evacuare per primo», perché la sindaca autorizza già un quartiere alla volta.

**Proposta, attivata il 2026-10-13** (`game.json` `exit_per_min: 6`; da confermare dall'utente): versione B a **6 famiglie/min**, come realismo visibile (le famiglie in coda si leggono nella scheda del quartiere e nella conferma dell'evacuazione), non come leva di bilanciamento. Il bilanciamento resta affidato all'autorizzazione della sindaca e al criterio di Misura. **Da rifare sul nuovo territorio** (valico, vedi `docs/TODO.md`).

    cargo build --release -p rocca --examples
    USCITA=6 GREZZI=g6.csv ./target/release/examples/sindaca --semi 1,2,3 > u6.md
    python3 docs/fase6/uscita/riassunto.py <cartella>
