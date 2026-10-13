# Evacuazione autorizzata dalla sindaca — prova headless (2026-10-13)

Regole: `docs/01-SPEC-GIOCO.md` §2–§3 e §5. **Solo motore e prove headless: nel chiosco la regola è ancora spenta** (`Game::mayor = None` di default).

**File:** `crates/rocca/src/mayor.rs` (nuovo), `game.rs` (`mayor`, `authorise`, `may_evacuate`, `take_authorisations`, `commit` rifiuta «Evacua» senza autorizzazione, `timing`, `measure`), `crisis.rs` (stima d'avvicinamento `Closing` condivisa con la sindaca), `score.rs` (`Stars`), `examples/sindaca.rs`. Dati completi: [`tutto.md`](tutto.md).

**Prova:** `cargo run --release -p rocca --example sindaca -- --semi 1,2,3`. Sono 36 casi × 3 semi: 82 incendi raggiungono almeno un quartiere. 5,5 min su M4 Pro. Priorità uguali in tutti i piani: i quartieri che lo stesso incendio senza ordini raggiunge (senno di poi, come in `strategie_civili`).

**Regola:** la sindaca autorizza un quartiere quando il fronte, alla velocità con cui si è avvicinato negli ultimi 10 min, arriverebbe alle case entro *N* minuti, oppure quando il fuoco è già alle case.

## Medie su 82 incendi (punti 0–1000; stelle P/C/T/M = Persone, Case, Tempismo, Misura)

| regole | piano | punti | P | C | T | M | colte in casa | minuti reali |
|---|---|---|---|---|---|---|---|---|
| vecchie | nessun ordine | 177 | 1,3 | 2,5 | 1,0 | 5,0 | 23,4 | 2,9 |
| vecchie | evacua tutti subito | 562 | 3,5 | 2,5 | 4,8 | 2,0 | 9,1 | 2,9 |
| sindaca 20 min | evacua appena autorizzato | 355 | 2,3 | 2,5 | 3,5 | 4,5 | 14,8 | 5,1 |
| sindaca 20 min | evacua con 10 min di ritardo | 293 | 2,0 | 2,5 | 2,6 | 4,6 | 17,2 | 7,1 |
| sindaca 20 min | solo preallerta (tutti) | 539 | 3,4 | 2,5 | 4,8 | 5,0 | 9,0 | 5,1 |
| sindaca 20 min | preallerta tutti, evacua appena autorizzato | 615 | 3,8 | 2,5 | 4,8 | 4,4 | 7,4 | 5,1 |
| sindaca 20 min | preallerta ed evacua solo i minacciati (senno di poi) | 620 | 3,8 | 2,5 | 4,6 | 5,0 | 7,1 | 5,1 |

Confronti incendio per incendio (sindaca 20 min; «perde» = più di 20 punti in meno):

| confronto | A perde | B perde |
|---|---|---|
| A preallerta tutti + evacua / B solo preallerta | 5 | 46 |
| A preallerta tutti + evacua / B preallerta ed evacua solo i minacciati | 34 | 10 |
| A preallerta tutti + evacua / B vecchie, evacua tutti subito | 3 | 50 |

Anticipo della sindaca: con 10 min arriva troppo tardi (anticipo reale mediano 5 min; 21 autorizzazioni su 170 dopo il fuoco alle case). Con 20 min: mediana 22 min, 1 su 170 in ritardo, 73 quartieri autorizzati che il fuoco poi non raggiunge. Con 30 min: mediana 34 min, 98 non raggiunti.

## Cosa dicono i numeri

1. **«Evacua tutti» non è più possibile, e «evacua appena autorizzato» non domina**: senza preallerta le famiglie partono tardi (355 punti contro 615).
2. **Il nuovo piano forte è «preallerta, poi evacua quando la sindaca autorizza».** Batte le vecchie regole in 50 incendi su 82 e ne perde 3: le pattuglie non si disperdono più su tutti i quartieri.
3. **La scelta vera diventa *chi* preallertare.** Preallertare tutti costa più di 20 punti in 34 incendi su 82, rispetto a preallertare solo chi sarà raggiunto. Le pattuglie si sparpagliano, e con il layout 7 è la stessa asimmetria di prima. Evacuare all'autorizzazione è quasi sempre giusto (perde solo 5 volte). Il tempismo conta: 10 min di ritardo costano in media 62 punti.
4. **Durata stimata:** 5,1 minuti reali, contro 2,9 senza momenti ×1. Il conto include 40 s di apertura e 45 s per autorizzazione (in media 3), ma non le pause «Decidi» del giocatore.

## Incognite

- **Stelle, scala lineare provvisoria:**
  - le 5 stelle in Persone sono rare (media del miglior piano 3,8);
  - Case resta ferma a 2,5 per tutti i piani, perché qui le priorità sono uguali;
  - Tempismo dà 4,8 anche a «solo preallerta», perché conta la preallerta come ordine.
  Soglie e definizioni sono da tarare.
- **Misura e debrief usano criteri diversi.** Misura conta le evacuazioni dei quartieri mai raggiunti alle case. Il debrief chiama «prudente» un'evacuazione con il fuoco vicino. Va scelto un solo criterio.
- Tutti i piani usano priorità e «minacciati» presi dal senno di poi. Manca una strategia di preallerta selettiva che usi solo ciò che si sa al momento.
- Il rilevatore di crisi del vecchio flusso gira ancora, ma in questa prova non è usato.
- **Test preesistente rotto:** `tests/messaggeri.rs::the_teams_leave_a_district_the_fire_reaches` fallisce anche al commit `14357e6`, senza queste modifiche. Gli altri test di `rocca` passano.

## Proposta

Tengo la soglia della sindaca a **20 minuti**. Poi aggiungo a `sindaca.rs` una «preallerta per vicinanza» (i quartieri entro X km dal fronte o sottovento all'inizio), senza senno di poi, e taro le stelle prendendo come riferimento i piani di questa tabella. Prima di toccare il chiosco serve la tua conferma.
