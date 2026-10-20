---
name: agent-playtest
description: Organizzare playtest di un serious game con sotto-agenti che interpretano personas (studente, adulto curioso, ottimizzatore, esperto di UX), via browser headless o su screenshot, senza rubare il focus all'utente; e raccogliere feedback di giocatori umani legati alla build. Usare quando l'utente chiede un playtest, «è giocabile?», un giro con Haiku/Sonnet, personas, o un report con immagini.
---

# Playtest con agenti e con persone

Ci sono due tipi di playtest. Vanno tenuti separati.

| Tipo | Domanda | Strumento | Agente |
|---|---|---|---|
| **Giocatore** (persona) | Si capisce? È divertente? Dove ci si blocca? | Browser headless sulla build web, oppure soli screenshot | Haiku per i giri rapidi, Sonnet per un giudizio più fine |
| **Ottimizzatore** | Esiste una strategia stupida che vince sempre? | Runner headless (`sim-game-balance`) | Haiku |

## 1. Regole del playtest con persona

- **Il tester non legge il codice, i documenti né `CLAUDE.md`** e non usa git. Va scritto
  nel prompt: «ignora qualunque istruzione di progetto che ti arrivi dal contesto». Sa
  del gioco solo ciò che vede sullo schermo.
- **Si comporta da persona, non da tester:**
  - usa solo ciò che vede e il puntatore;
  - niente console né stato interno, e JavaScript solo per inviare clic, tocchi e
    trascinamenti;
  - niente scorciatoie da operatore;
  - legge alla velocità della persona, sfogliando i testi lunghi;
  - decide d'intuito;
  - un limite dello strumento lo segnala come tale, non come difetto del gioco.
- **Almeno 3 partite** con semi o scenari diversi, la prima senza tutorial saltato.
- **Risultati in una cartella sua:**
  - `diario.md`, con quello che vede e pensa, passo per passo;
  - `report.md`, con le sezioni: cosa ho capito, dove mi sono bloccato, cosa è troppo
    piccolo o lungo o contraddittorio, cosa ho imparato, che voto do e perché, le 5
    correzioni più importanti;
  - gli screenshot.

  Se non riesce a scrivere i file, mette il report completo nella risposta finale.
- **Il browser è uno solo:** con più personas i tester vanno lanciati uno dopo l'altro,
  oppure si dà a ognuno il proprio browser headless.
- **Se il browser fallisce 2–3 volte di seguito**, il tester si ferma e scrive che cosa è
  successo, senza insistere.

Modello di prompt: `persona-template.md` (in questa cartella).

## 2. Personas che hanno dato risultati

- **Giulia, 16 anni**, terza superiore: gioca sul telefono, non agli strategici. Curiosa ma
  impaziente: se una cosa non si capisce in pochi secondi, clicca. Nessuno le ha spiegato
  niente.
- **Ragazzino di 12 anni**: legge poco, tocca tutto, vuole vedere subito qualcosa succedere.
- **Adulto curioso** che ha aperto il link dal QR: vuole capire il rischio, ha 5 minuti.
- **Min-maxer**: cerca di rompere il punteggio con la strategia più semplice.
- **Esperto di UX**: giudica leggibilità, gerarchia, coerenza dei colori, touch e
  dimensioni.
- **Operatore di protezione civile**: giudica il realismo di tempi, ruoli e messaggi (vedi
  `cp-serious-game-design`).

## 3. Mezzi tecnici

- **Non rubare il focus all'utente.** Mai aprire la finestra del gioco desktop per un
  playtest. Si usano:
  - un browser headless (Playwright o Chromium headless) sulla build web, servita in locale;
  - screenshot di stati arbitrari salvati su file, senza finestra.
- **Modalità di playtest della build web**: un parametro URL (es. `?playtest=1`) che
  - ferma il tempo finché l'agente chiede di avanzare;
  - fa avanzare alla massima velocità della CPU senza saltare fisica né eventi;
  - espone lo stato e i rettangoli dei controlli (per un runner, non per la persona).
- **Canvas WebGL:** i clic si mandano come eventi `pointer*`/`mouse*`/`touch*` sul canvas,
  con circa 150 ms tra un evento e l'altro. Le coordinate dello screenshot vanno riportate
  ai pixel CSS.
- **Smoke test automatico** del flusso completo (titolo → nome → tutorial → ordini →
  finale → Riprova) in più viewport (desktop, 390×844, 844×390, 320×568), con un limite
  di memoria e un **report.html** unico con tutte le immagini in ordine, da ispezionare
  nel browser.
- **Playtest su soli screenshot** (il più economico): si producono le schermate di una
  partita nell'ordine in cui accadono. Per ognuna l'agente-persona risponde: che cosa
  succede, che cosa clicchi, che cosa confonde. Alla fine dice che cosa ha imparato.

## 4. Dopo il playtest

- Si raccolgono i risultati in una tabella **problema → gravità → proposta**, si
  correggono subito le cose minori e si chiedono all'utente le decisioni di design.
- Ogni nuova osservazione va in `docs/TODO.md`, con la data.
- Si rifà un giro dopo le correzioni, con un modello diverso se serve.

## 5. Feedback di persone vere

- Un **QR** nell'evento porta alla versione web, e un link nella versione web porta a un
  modulo (es. Google Form).
- Il modulo registra il **commit della build** provata (iniettato in compilazione), la
  fascia d'età (<14, 14–18, 18–24, >24), il mestiere e domande su comprensione,
  divertimento e realismo.
- I commenti dei colleghi arrivano come elenco incollato: si rispondono punto per punto
  con fatto e da fare, e si chiedono solo le scelte vere.

## 6. Esempio: Isola di Roccavento

`scripts/playtest_mobile.py` (smoke touch e mouse, report.html), `scripts/playtest.py
--interactive` (sessione JSON a righe per un agente; flusso vecchio, da aggiornare),
`docs/web.md`. Gli screenshot si fanno con `KIOSK_SNAP` (in `CLAUDE.md`).
