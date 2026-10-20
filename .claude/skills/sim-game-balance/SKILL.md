---
name: sim-game-balance
description: Bilanciare un gioco basato su simulazione con prove headless riproducibili. Usare prima di cambiare regole, parametri, territorio, capacità delle squadre o soglie del punteggio; quando si chiede «il gioco è troppo facile/difficile», «esiste una strategia che vince sempre», o si vuole un laboratorio con scenari sintetici e sweep di parametri.
---

# Bilanciamento con prove headless

**Criterio:** nessuna strategia semplice deve dominare. Se «evacua tutti subito» o
«tutte le squadre sull'innesco» prendono il massimo, il gioco non ha un dilemma e non
insegna niente. Una meccanica che funziona ma non sposta il bilanciamento non è un
risultato: va riportato come tale.

## 1. Gli strumenti da avere

1. **Runner headless** che gioca una partita da riga di comando con ordini a orari
   simulati. Per esempio `rocca medium --squadra "20:0=5590;4400" --evacua "20:Le Terrazze"`.
   Stampa una tabella per zona, gli esiti e le stelle, sempre accanto al **riferimento
   senza ordini sullo stesso seme**. Gira alla massima velocità: niente finestre, pochi
   secondi a partita.
2. **Strategie di confronto** scritte una volta e rieseguibili (un esempio o uno script):
   - nessun ordine;
   - tutto sull'innesco alla prima pausa;
   - evacua tutto appena possibile;
   - una strategia combinata «ragionevole»;
   - una strategia sbagliata (priorità invertite).
3. **Laboratorio sintetico** per gli algoritmi e i parametri: territori semplici (piano,
   pendio, crinale, fascia non combustibile, strada unica, vento che gira), le strategie
   × il numero di squadre × i semi, sweep di un parametro alla volta (`--sweep k=v1,v2`), il
   valore marginale di una squadra (`--senza-una`) e una telemetria completa in CSV
   (ettari risparmiati, tempo per stato, acqua usata e «tenuta», ritirate, perdite).
4. **Sempre gli stessi semi** (1–3, o 1–5 per le differenze piccole). Si riporta la media
   e il valore per seme. Le differenze dentro il rumore si dichiarano rumore.

## 2. Il ciclo di una modifica

1. **Prima la misura:** tabella strategie × semi con il codice attuale.
2. Si implementa la modifica **dietro un interruttore**, spento. Si verifica che, spento, il
   gioco sia identico: N corse su N uguali.
3. Si misura acceso: stessa tabella, stessi semi. Si confronta con il «prima» e con il
   riferimento senza ordini.
4. **Resoconto all'utente** con la tabella e una proposta. **Decide l'utente**: la meccanica
   si accende nel gioco solo dopo la sua conferma. Se resta spenta, va detto esplicitamente.
5. Se cambia il territorio o lo scenario, tutte le tarature vanno rifatte su quello nuovo.

## 3. Come leggere i risultati

- Il confronto con lo stesso evento senza ordini viene prima del valore assoluto.
- Se una strategia peggiora rispetto al «senza ordini» (es. tutti sull'innesco nei semi
  2–3), abbassare le soglie del punteggio non la trasforma in un successo.
- Se una leva non costa nulla (evacuare tutti senza conseguenze), si cerca il costo
  **fisico** che manca: persone in strada vicino al fronte, tempo di preparazione,
  limite di partenze. Prima di aggiungere code o traffico, si verifica che a questa
  scala possano davvero diventare il collo di bottiglia.
- Se il gioco è troppo facile dopo un miglioramento degli algoritmi automatici, si
  ricalibra la capacità (portata dell'acqua, efficacia delle linee, ricarica) con lo
  stesso laboratorio e la stessa tabella. Si annota il motivo storico: la capacità era
  stata alzata perché le squadre non spegnevano mai.
- Tabelle chiare: una riga per strategia e seme, colonne con le grandezze fisiche
  (persone, case, ettari) prima delle stelle. Se l'utente non capisce una tabella, va
  rifatta più semplice, con il parametro in esame come prima colonna.

## 4. Playtest «ottimizzatore» con un agente

Si affida a un sotto-agente economico (Haiku o Sonnet) **solo il runner**. Il prompt
contiene:

- la domanda («esiste una strategia stupida che vince sempre?»);
- le regole in breve;
- la sintassi del runner con un esempio;
- le strategie minime da provare;
- i semi;
- il divieto di modificare codice o dati e di ricompilare;
- la cartella in cui scrivere script, output grezzi e report.

L'agente risponde in italiano con una tabella strategia × seme, la soglia del rumore e
le sue conclusioni. Il playtest dei giocatori veri sta in `agent-playtest`.

## 5. Esempio: Isola di Roccavento

- `target/release/rocca`: il runner.
- Esempi in `crates/rocca/examples/`: `strategie`, `score_review`, `scenario_suite`,
  `tutorial_cerca`.
- `target/release/lab`: il laboratorio (`docs/05-LABORATORIO.md`).
- Batteria di prove in `docs/03-VERIFICHE.md`.

Proposta di soglie per le stelle, provata con 4 strategie su 3 semi: `docs/04-VALUTAZIONE.md`.
