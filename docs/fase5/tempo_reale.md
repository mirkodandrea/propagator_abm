# Tempo reale, pause, punteggio e classifica (9 ottobre 2026)

Richiesta dell'utente: «non è chiaro se è un gioco real time o a turni». Poi: «un certo numero max di pause» e «un punteggio ed una leaderboard». Il punteggio è calcolato sui «salvati rispetto a nessun ordine» più le pause non usate, l'inserimento del nome usa tre iniziali e la classifica è unica per tutti gli incendi.

## Cosa cambia

| Prima | Ora |
|---|---|
| «Avvia», «Pausa e modifica piano», «Conferma e riprendi», «Conferma» | un solo pulsante play/pausa: «▶ Avvia» / «⏸ Pausa · 3 rimaste» / «▶ Riprendi» |
| durante il gioco un ordine era un'anteprima da confermare | **durante il gioco l'ordine vale al click** (`ui.rs::draw`). L'anteprima con conferma resta in pausa e nelle crisi, come chiede la spec |
| pausa = schermata uguale | mappa scurita e «IN PAUSA · il fuoco è fermo finché non premi «Riprendi»» |
| «T+0:26 / 3 h · ×40» | «ore 14:26 · fine 17:00 · Il tempo scorre · ×40» (`rocca::words::clock`, usato anche nel «Perché» e nella cronologia) |
| crisi: «Decisione · ×1» | «Rallentato · ×1», cornice arancione, «Il tempo è rallentato, non fermo: il fuoco avanza ancora» |
| pause illimitate | **3 pause** (`MAX_PAUSES`); le crisi non contano |
| ×120 automatico anche con un paese in fuga | ×120 solo se nessuno è in partenza e il fuoco è a più di 1,5 km da chi è in casa (`Game::is_quiet`, `QUIET_FIRE_M`) |
| nessun punteggio | `rocca::score`: 100 per famiglia e 10 per casa salvate rispetto allo stesso incendio senza ordini, 50 per pausa non usata (solo se si è salvato qualcosa), mai negativo. Classifica unica dei primi 10, tre iniziali, file `~/.rocca_ventosa/classifica.json` (`KIOSK_SCORES` per spostarlo) o `localStorage` nel browser. «Azzera classifica» nella barra F2, con doppia pressione |

**Aggiornamento, stessa sera (utente):** «si cambia solo in pausa, così in automatico rendiamo possibili pochi cambi piani» e «una pausa non può essere immediatamente dopo un'altra». Durante il gioco le schede mostrano gli ordini ma non li cambiano («Ordini: si cambiano in pausa, tra N min»). Gli ordini immediati della riga sopra sono stati **tolti**. Dopo «Avvia» o «Riprendi» la pausa torna disponibile solo dopo 15 minuti simulati (`PAUSE_GAP_S`, circa 22 s reali a ×40): il pulsante mostra «Pausa tra N min · 2 rimaste». Le crisi restano decisioni a sé, con le loro risposte.

Intro e «Come si gioca» dicono che il tempo scorre da solo, che gli ordini si cambiano solo in pausa e quante pause ci sono.

## Prova

- `cargo test --release --workspace`: 37 target verdi, compresi 4 test nuovi in `rocca::score`.
- **Durata** (`cargo run --release -p rocca --example durata`, ×40): Coste2_gira 5,3 min reali, Piano2 4,5, Borgo2 4,5 (prima 3,3 / 1,8 / 2,1). Il ×120 non scatta più in questi tre casi, perché il territorio è piccolo e il fuoco resta sempre vicino a qualcuno in casa. Si risolve anche il dubbio su Piano2 troppo breve.
- **Playtest mio nel browser** (Chrome, build web), tre partite:
  1. Coste2_gira alla cieca, versione precedente: 1 famiglia colta / 71 case;
  2. Coste2_gira, nuova versione, difendendo prima Pian dei Grilli ed evacuando durante il gioco: 2 / 24 (−14 / −49);
  3. Borgo2: 1 / 6, **1720 punti**, primo in classifica.

  Schermate: `img/tr_pausa_avvio.jpg`, `img/tr_ordine_subito.jpg`, `img/tr_pausa.jpg` e `img/tr_classifica.jpg`.

## Trovato giocando, ancora aperto

1. **Le schede si spostano** di circa 20 px quando cambia il fumetto o si dà un ordine: un click può finire sul pulsante sbagliato.
2. **Inquadratura dopo un nuovo incendio:** per un attimo la telecamera si muove e il primo click va a vuoto (partita 3, preallerta persa).
3. **«Come si gioca» ricompare dopo «Riprova».**
4. **Etichette sovrapposte:** «Le Ghiande · nucleo 1» copre A1 e SQ e a volte va in cima allo schermo.
5. **Testi del coordinatore:** «fuoco a 0,0 km» invece di «fuoco tra le case»; «Squadra A si ritira vicino a Pian dei Grilli» quando era a Le Ghiande.
6. **Cronologia del finale:** preallerta e difesa alle 14:00 si sovrappongono.
7. **Scheda compatta:** non mostra l'ordine dato alla popolazione.
8. **Tre iniziali:** «GIU» ha richiesto 20 click. Una griglia di lettere sarebbe più rapida.
9. **Mezzi fermi:** a fine partita 2, A2 è rimasto «acqua» e SQ «in arrivo ~1 min» per oltre 20 minuti simulati.

## Proposta per il passo successivo

Correggere 1, 2 e 3 (click sbagliati e attese inutili) prima del checkpoint 5.
