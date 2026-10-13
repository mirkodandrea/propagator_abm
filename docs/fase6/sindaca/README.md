# Evacuazione autorizzata dalla sindaca e stelle — prova headless (2026-10-13)

Regole: `docs/01-SPEC-GIOCO.md` §2, §3 e §5. **Solo motore e prove headless: nel chiosco la regola è ancora spenta** (`Game::mayor = None` di default).

**File:**
- `crates/rocca/src/mayor.rs`, nuovo.
- `game.rs`:
  - `mayor`, `authorise`, `may_evacuate`, `take_authorisations`;
  - `commit` rifiuta «Evacua» senza autorizzazione;
  - `timing` e `measure`.
- `crisis.rs`: stima d'avvicinamento `Closing`, condivisa con la sindaca.
- `score.rs`: `Stars`, con soglie tarate.
- `examples/sindaca.rs`.

Dati per partita: [`tutto.md`](tutto.md) e [`grezzi.csv`](grezzi.csv).

**Prova:** `GREZZI=docs/fase6/sindaca/grezzi.csv cargo run --release -p rocca --example sindaca -- --semi 1,2,3`. Sono 36 casi × 3 semi: 82 incendi raggiungono almeno un quartiere. 3,5 min su M4 Pro. Le priorità sono i quartieri che lo stesso incendio senza ordini raggiunge (senno di poi), tranne in «priorità sbagliate».

**Regola:** la sindaca autorizza un quartiere quando il fronte, alla velocità osservata negli ultimi 10 min, arriverebbe alle case entro **20 min**, oppure quando è già alle case. Con 10 min arriva tardi: anticipo reale mediano 5 min, 21 autorizzazioni su 170 dopo il fuoco. Con 20 min: mediana 22 min, 1 su 170 tardi.

## Medie su 82 incendi (stelle 1–5: Persone, Case, Tempismo, Misura)

| regole | piano | punti | P | C | T | M | colte in casa | min reali |
|---|---|---|---|---|---|---|---|---|
| vecchie | nessun ordine | 177 | 1,2 | 2,9 | 1,0 | 5,0 | 23,4 | 2,9 |
| vecchie | evacua tutti subito | 562 | 3,4 | 2,9 | 4,7 | **1,0** | 9,1 | 2,9 |
| sindaca | evacua appena autorizzato (senza preallerta) | 355 | 2,2 | 2,9 | 3,0 | 4,1 | 14,8 | 5,1 |
| sindaca | evacua 10 min dopo l'autorizzazione | 293 | 1,9 | 2,9 | 2,3 | 4,2 | 17,2 | 7,1 |
| sindaca | solo preallerta (tutti) | 539 | 3,3 | 2,9 | 4,7 | 5,0 | 9,0 | 5,1 |
| sindaca | preallerta tutti, evacua all'autorizzazione | 615 | 3,7 | 2,9 | 4,8 | 4,0 | 7,4 | 5,1 |
| sindaca | **preallerta i vicini (≤ 1,5 km dal fronte, sottovento conta di più), evacua all'autorizzazione** | **618** | 3,8 | 2,9 | 4,7 | 4,0 | 7,4 | 5,5 |
| sindaca | preallerta ed evacua solo i minacciati (senno di poi) | 620 | 3,9 | 2,9 | 4,5 | 4,9 | 7,1 | 5,1 |
| sindaca | preallerta tutti, evacua, **priorità sbagliate** | 418 | 3,2 | **1,2** | 4,6 | 4,1 | 11,1 | 5,1 |

## Cosa dicono i numeri

1. **Nessun piano semplice domina.** Il migliore senza senno di poi è «preallerta i vicini, evacua all'autorizzazione». Batte «preallerta tutti» in 12 incendi e ci perde in 5. Rispetto a chi conosce l'esito perde in 29 incendi e vince in 12. Ripianificare ogni 15 minuti, guardando il fronte, conta.
2. **Ogni stella misura una scelta diversa.**
   - **Persone:** la preallerta.
   - **Case:** le priorità di difesa (1,2 con le priorità sbagliate, 2,9 con quelle giuste).
   - **Tempismo:** il ritardo (evacuare 10 minuti dopo l'autorizzazione costa 62 punti e quasi una stella).
   - **Misura:** evacuare chi poi non viene raggiunto («evacua tutti» = 1).
3. **Il dilemma resta reale.** Evacuare all'autorizzazione salva persone (Persone 3,7 contro 3,3), ma nella metà degli incendi manda in strada un quartiere che il fuoco poi non raggiunge (Misura 4,0 contro 5,0).
4. **Durata stimata: 5–5,5 minuti reali.** Il conto include 40 s di apertura, 45 s per ognuna delle ~3 autorizzazioni e 30 s per ogni «Decidi».

**Soglie delle stelle** (`score.rs`, una stella in più per ogni soglia raggiunta):

| categoria | soglie |
|---|---|
| Persone | 0,2 / 0,45 / 0,65 / 0,85 |
| Case | 0,1 / 0,25 / 0,45 / 0,7 |
| Tempismo | 0,25 / 0,5 / 0,75 / 0,9 |
| Misura | 0,4 / 0,6 / 0,8 / 0,95 |

## Incognite

- **Misura e debrief usano criteri diversi: va deciso.** Misura conta le famiglie evacuate da quartieri dove il fuoco non è mai arrivato alle case. Il debrief giudica «prudente» un'evacuazione con il fuoco a meno di 1,5 km, e «Forse bastava la preallerta» solo oltre. Con il criterio del debrief, sotto la nuova regola un'evacuazione «inutile» capita in 1 partita su 82, quindi Misura non distinguerebbe più nulla. Proposta: tenere il criterio di Misura e far dire al debrief lo stesso fatto in modo neutro, per esempio: «12 famiglie in strada; il fuoco è arrivato a 900 m, non alle case».
- **Le stelle sono assolute, non relative al caso.** In alcuni incendi nessuna casa si può salvare, e anche il piano giusto prende 1 stella in Case.
- **Tempismo è quasi pieno per chi preallerta presto.** Distingue solo chi evacua senza preallerta o in ritardo.
- **Test preesistente rotto:** `tests/messaggeri.rs::the_teams_leave_a_district_the_fire_reaches` fallisce anche a `14357e6`.

## Proposta

1. Decidi il criterio di Misura.
2. Poi attivo la regola nel chiosco: «Evacua» bloccato con il motivo, la sindaca che apre il momento a ×1, «Decidi» a ×1, finale a stelle, ultimi 8 giocatori.

## Nel chiosco (2026-10-13)

Regola attiva in `game.json` (`"mayor": {"lead_s": 1200}`, anche in `tools/factory/cases.py`). Prova: `KIOSK_CASE=Borgo2_gira KIOSK_SPEED=300 KIOSK_SHOT=out/sindaca_shot target/release/game`, più `KIOSK_SNAP` per la scheda.

- **Momento delle scelte a ×1** al posto delle pause: «Avvia» all'inizio, poi «Decidi» (nessun limite, ≥ 15 min simulati tra due), «Conferma» / «Annulla». Si torna da soli a ×N dopo 60 s se non si cambia nulla. Il giocatore non sceglie più la velocità. L'operatore può fermare il mondo da F2.
- **La sindaca apre la crisi** ([immagine](chiosco_sindaca.jpg)): rischio, tempo, famiglie in casa e costo. Countdown di 45 s. «Evacua anche X» per gli altri paesi autorizzati. Le vecchie crisi (vento, previsione, mezzo perso) ora sono notizie.
- **«Evacua» bloccato con il motivo** finché non c'è l'autorizzazione ([immagine](chiosco_scheda.jpg)).
- **Finale** ([immagine](chiosco_fine.jpg)): quattro stelle, ciascuna con il «perché», e l'elenco degli ultimi 8 giocatori con il livello colorato, salvato in `~/.rocca_ventosa/ultimi.json`. Niente punteggio e niente classifica: il vecchio `classifica2.json` non viene più letto.
- **Esempi e test delle vecchie regole:** ora spengono la sindaca in modo esplicito. `rocca … --senza-sindaca`; senza questa opzione, un'evacuazione programmata aspetta l'autorizzazione.

**Incognite:**
- Nel livello Esperto (Borgo2_gira) la sindaca autorizza Castelvento quando il fuoco è a circa 9 minuti dalle case: è la stima della velocità d'avvicinamento all'inizio dell'incendio. Senza preallerta l'evacuazione arriva tardi. Da valutare in un playtest: è un dilemma o un'ingiustizia?
- Non ho ancora provato la build web.
