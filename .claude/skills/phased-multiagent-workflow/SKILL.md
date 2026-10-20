---
name: phased-multiagent-workflow
description: Metodo di lavoro per progetti lunghi con l'utente e più sessioni Claude in parallelo sullo stesso checkout - fasi con checkpoint umano, documenti come fonte di verità, TODO aggiornato nello stesso commit, resoconti brevi in italiano, igiene git con modifiche altrui, uso di sotto-agenti economici e worktree. Usare a inizio sessione, prima di ogni commit, quando si delega a sotto-agenti, prima di un compact/clear, o quando l'utente chiede «cosa resta da fare».
---

# Lavorare per fasi, con più sessioni

## 1. Fonti di verità

- **I documenti dicono che cosa, il codice come.** Se non coincidono, prevalgono i documenti
  e il conflitto va segnalato. Se i documenti sono chiaramente vecchi, si legge il codice e
  si propone la correzione.
- Ogni nuova regola o decisione dell'utente si scrive **con la data** nel documento giusto
  (specifiche, regole, territorio, verifiche). Lo storico non si tiene nei documenti: sta
  in git. Le voci chiuse si cancellano.
- Quando il progetto cambia rotta («partiamo da zero»), si riscrivono le istruzioni da
  capo, invece di accumulare eccezioni. Parola d'ordine: pulizia. Un motore, uno scenario,
  una modalità. Il codice vecchio si toglie invece di tenerlo dietro un flag.

## 2. Fasi e checkpoint

- Una fase alla volta, ognuna con un **checkpoint umano**: la successiva non parte senza
  conferma. Le richieste puntuali dell'utente si fanno subito, ma da sole non fanno
  avanzare la fase.
- `docs/TODO.md` riporta in cima la fase corrente e il prossimo checkpoint, poi i lavori
  aperti e le decisioni con la data. **Si aggiorna nello stesso commit del lavoro.**
- **Resoconto all'utente**, in italiano, al massimo una pagina:
  1. **Realizzato**: file e moduli toccati.
  2. **Evidenza**: il comando per riprodurre, più una tabella o un'immagine.
  3. **Non risolto**: rischi, test falliti, ciò che non è stato guardato.
  4. **Una sola proposta** per il passo successivo.
- Se l'utente chiede un report prima di iniziare, si fa il report e ci si ferma.
- **Si tiene traccia di ogni richiesta** di un elenco incollato: si risponde punto per
  punto con fatto e da fare. Una correzione chiesta e non fatta è l'errore che irrita di
  più. Prima di dire «fatto» lo si verifica da sé (un test, un'immagine da più angoli).
- Prima di un `/compact` o `/clear` si salvano in TODO e nella memoria le informazioni
  utili, se l'utente lo chiede: «aggiorna todo e memoria, faccio clear».

## 3. Più sessioni sullo stesso checkout

- Spesso altre sessioni modificano lo stesso albero. Prima di mettere in stage:
  - `git status` e **`git diff` di ogni file**;
  - si committano **solo le proprie modifiche**; sui file misti si usa `git add -p` (si può
    fare in modo non interattivo, con le risposte in pipe, conoscendo l'ordine degli hunk);
  - si verifica che l'**albero in stage compili** e che i test passino.
- **Coordinarsi:** `ListAgents` e `SendMessage` per sapere chi tocca che cosa e chi
  committa per primo. Se un'altra sessione sta cambiando la stessa parte (per esempio la
  geometria della vegetazione), si aspetta che abbia finito.
- Se l'utente dice «committa tutto, anche delle altre sessioni», lo si fa solo se HEAD con
  tutte le modifiche compila. Si descrive nel messaggio di commit che cosa viene da dove.
- **Push solo quando richiesto.** «commit» non vuol dire «commit and push».

## 4. Sotto-agenti

- Si delega quando riduce il costo o il tempo: **Haiku** per playtest e prove ripetitive,
  **Sonnet** per modifiche circoscritte e playtest più fini, **Opus** per riprogettare un
  algoritmo. Il prompt è autosufficiente: domanda, vincoli («non modificare codice», «non
  committare»), comandi, dove scrivere, formato della risposta.
- I sotto-agenti con `isolation: worktree` partono da `main`, non dal branch di lavoro.
  Sullo stesso checkout si danno a ognuno file distinti, senza commit, e si committa alla
  fine. Finito il lavoro, i worktree si rimuovono.
- Il risultato di un sotto-agente non lo vede l'utente: va riassunto, con le tabelle.

## 5. Stile con l'utente

- Si risponde nella lingua dell'utente (qui italiano). Frasi brevi, tabelle, niente enfasi.
- Prima di cambiare regole, bilanciamento o territorio: prova headless e tabella, poi
  decide l'utente (`sim-game-balance`).
- Non rubare il focus: niente finestre né browser in primo piano senza una richiesta
  (`game-snapshot-harness`).
- Le domande vanno fatte solo quando la scelta spetta davvero all'utente, con una
  raccomandazione.
