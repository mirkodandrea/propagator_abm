# Layout 5 e messaggeri nel gioco (2026-10-12)

Decisione dell'utente (2026-10-12): **variante A** di `../messaggeri/README.md` nel gioco. L'utente ha osservato che le prove sull'evacuazione totale non erano arrivate al chiosco: il territorio pubblicato era ancora il layout 4 e i messaggeri erano spenti.

**Cosa cambia:**
- `data/scenarios/rocca_ventosa`: ora è il **layout 5** (`t4_paese5`), pubblicato con `scenario_factory.py publish`.
- `game.json` ha `messengers`, letto da `Territory` e messo da `Game::new` in `Game::messengers`. Valori: 4 squadre, 1 famiglia al minuto, allerta sul telefono, squadre in parti uguali (a parità secondo le priorità). Chiosco e runner headless usano lo stesso valore. `tools/factory/cases.py::MESSENGERS` lo scrive.
- `strategie_civili`: senza `MESSAGGERI` usa le squadre del `game.json`; `MESSAGGERI=0` le spegne.
- Livelli riscelti (`cases.py::LEVELS`, `FEATURED`):

| livello | prima | ora | perché |
|---|---|---|---|
| Facile | Rocco3 | **Rocco3** | un paese, il vento non gira |
| Medio | Forna3_gira | **Terra1_gira** | sul layout 5 Forna3_gira si accende in un solo seme su tre |
| Esperto | Coste2_gira | **Borgo2_gira** | Coste2_gira dà 567 punti con qualunque piano civile |

**Riproduzione:**

```
cargo run --release -p rocca --example strategie_civili -- --semi 1 > A_pubblicato_seme1.md
python3 docs/fase5/messaggeri/riassunto.py A_pubblicato_seme1.md
cargo run --release -p rocca --example durata
```

## Risultati (gioco pubblicato, semi 1/2/3)

| | seme 1 | seme 2 | seme 3 |
|---|---|---|---|
| incendi in cui «evacua tutti» perde più di 20 punti rispetto a «evacua i minacciati» | 6 | 10 | 7 |
| media «evacua tutti» / «evacua i minacciati» / «preallerta tutti» | 624 / 640 / 581 | 562 / 587 / 569 | 572 / 596 / 596 |

Sono gli stessi numeri della prova headless (`../messaggeri/README.md`, A: 6/10/7). Nei livelli, punti con «evacua tutti» contro «evacua i minacciati»:

| livello | seme 1 | seme 2 | seme 3 |
|---|---|---|---|
| Facile, Rocco3 | 823 / 867 | 635 / 678 | 663 / 663 |
| Medio, Terra1_gira | 538 / 644 | 513 / 609 | 750 / 806 |
| Esperto, Borgo2_gira | 424 / 556 | 253 / 428 | 483 / 575 |

Durata a ×80 (×240 nei tratti quieti), un piano semplice: 2,2–3,1 minuti reali di esecuzione, più la pianificazione; 0–2 crisi. Partita scriptata nel chiosco su Borgo2_gira a 1920×1080: nessun errore.

**Test:** 39 target verdi. Due test adattati al territorio nuovo, senza cambiare cosa verificano:
- `fase3::inverting_priorities_changes_posts_and_outcomes`: guarda le postazioni a T+30, non a T+180, quando un'autobotte può essere fra il rifornimento e la postazione;
- `authored_unit_policy::raising_the_refill_threshold…`: osserva l'autobotte dall'inizio. Sul layout 5 quella «impaziente» trova un fronte e va a fare rifornimento prima del minuto 15, quando il test cominciava a guardare.

## Incognite

- ~~Le squadre di avviso non si vedono nel chiosco~~ e ~~la scheda non dice che gli avvisi li portano poche squadre~~: risolto, vedi «Aggiornamento».
- Piano3, Le Ghiande in priorità 1: l'autobotte 1 resta in «rifornimento» da T+50 a T+170. Da capire.
- Build web non rifatta (`scripts/build_web.sh`).
- 4 squadre e 1 famiglia al minuto restano ipotesi.

## Aggiornamento: pattuglie e avvisi nel chiosco (2026-10-12)

Richiesta dell'utente: «indicare sui paesi come procede l'allertamento» e «anche i mezzi in movimento con la grafica». Nelle schermate le squadre di avviso si chiamano **pattuglie** (Polizia Locale), per non confonderle con la Squadra A dell'antincendio boschivo; anche le frasi del registro.
- **Badge del paese**: dopo la campanella o l'uscita, «172/190» famiglie che hanno sentito l'ordine, finché non le hanno sentito tutte; poi, come prima, le famiglie evacuate (`Game::alerted`).
- **Scheda**: mentre il tempo scorre, «Avvisate N famiglie su M: le pattuglie bussano porta a porta»; sotto i pulsanti, una riga sulle 4 pattuglie e sull'allerta sul telefono.
- **Mappa**: un'auto della Polizia Locale per pattuglia (`toy::patrol`: banda blu, lampeggiante, altoparlante; `units::update_teams`), nascosta quando è parcheggiata in caserma, scura se il fuoco la raggiunge. Etichetta con l'asta come i mezzi: «P1», o «×3» per le pattuglie vicine. Riga nella legenda.

Prova: `KIOSK_SIZE=1920x1080 KIOSK_CASE=Borgo2_gira KIOSK_SPEED=300 KIOSK_WINDOWED=1 KIOSK_TUTORIAL=0 KIOSK_SHOT=<dir> target/release/game` → `3_esegui.png` (Castelvento «🔔 172/190», «×3» e «P3» sulla mappa), `2_anteprima.png` (scheda).

## Proposta (una)

Rifare la build web e provare la partita con una persona: si capisce che ordinare tutti i paesi rallenta gli avvisi?
