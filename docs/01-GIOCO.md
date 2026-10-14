# 01 — Il gioco: Isola di Roccavento

Regole **in vigore** dal 2026-10-14 (nuovo progetto). Prevalgono sul codice: se non coincidono, segnalarlo. Ogni cambio di regola è una decisione dell'utente, da scrivere qui con la data (lo storico è nella storia git).

## In una frase

Uno strategico in tempo reale, sul modello di StarCraft, per il chiosco della *Settimana della Protezione Civile* (studenti delle superiori). Un incendio simulato minaccia i borghi di un'isola. Il giocatore **mette in pausa** per chiedere la previsione all'esperto, poi **sposta le squadre** su una griglia esagonale e **evacua i borghi**, uno alla volta. La simulazione decide gli esiti. Poche parole: la comunicazione (personaggi, messaggi, debriefing) si riscrive in un secondo momento.

**Obiettivo educativo:** incertezza, tempi, risorse limitate, adattarsi. Non è un addestramento e non insegna procedure operative.

## 1. Struttura della partita

1. **Apertura:** nome del giocatore, «Inizia» (oppure «Salta la prova guidata») e la classifica di oggi.
2. **Tutorial, giorno 1** (`rocca::tutorial`): un incendio piccolo vicino a Le Ghiande, con dinamiche reali. Istruzioni passo passo: Pausa, scelta della squadra, scelta dell'esagono e conferma (due squadre), evacuazione di Le Ghiande, Riprendi. Il gioco accetta **solo** l'azione richiesta e il mondo resta fermo finché il giocatore non la compie, quindi l'esito è scriptato. Poi il fuoco corre da solo: le squadre lo contengono e il borgo è al sicuro. Senza ordini, lo stesso incendio colpisce 17 case e 45 persone.
3. **Giorno 2:** nuovo giorno, nuovo incendio (`game.json` → `game`, oggi il caso `medium`, 5 ore). Il controllo passa al giocatore. L'incendio è **segnalato in ritardo**: quando la partita comincia brucia già da `start_min` minuti (2026-10-14).
4. **Fine:** stelle e classifica del giorno.

## 2. Il tempo e la pausa

- Nessun turno. Il tempo scorre a **×300**: 1 s reale vale 5 min simulati (`rocca::RUN_X`).
- **Pausa = chiedere all'esperto scientifico.** Il fuoco non si ferma, rallenta a **×60** (`PAUSE_X`). La pausa dura **30 min simulati** (30 s reali, `PAUSE_S`) oppure finisce prima con «Riprendi». Dopo, l'esperto torna disponibile tra **20 min simulati** (`PAUSE_COOLDOWN_S`).
- **Tutti gli ordini si danno solo in pausa:** spostare le squadre ed evacuare.
- In pausa compaiono:
  - la **previsione**: isocrone al 50 % di 16 simulazioni dal fuoco attuale, a +1 h e +3 h, senza interventi (`rocca::forecast`);
  - il **bollettino meteo**: vento attuale e cambio annunciato;
  - l'**umidità del combustibile**.
- Durante il tutorial il tempo è fermo mentre un passo aspetta il giocatore.
- La partita finisce quando il fuoco è spento e nessuno è in strada, oppure alla durata del caso.

## 3. Le squadre

- **Una squadra** è un'autobotte con 3 persone. Ci sono 3 squadre dei **vigili del fuoco** alla caserma di Castelvento, il borgo più grande, e 2 squadre di **volontari AIB** al presidio di San Rocco, un borgo minore (`game.json`: `stations`, `roster`).
- Si spostano su una **griglia esagonale** di 500 m tra centri adiacenti (`rocca::hex`). Si può scegliere solo un esagono attraversato da una strada carrabile della rete principale: la squadra guida fino al punto stradale dell'esagono più vicino al centro.
- Si sceglie la squadra (icona sulla mappa o barra laterale), poi l'esagono. Il gioco mostra **percorso e tempo di viaggio**, evitando le strade chiuse dal fuoco (`Game::plan_move`). Poi si **conferma**.
- Arrivata, la squadra **attacca il fuoco come può** (2026-10-14), con il modello di spegnimento conservato (`abm::suppression`):
  - l'**autobotte** aumenta l'umidità del combustibile che riesce a raggiungere dalla strada (acqua, portata della manichetta);
  - le **persone a piedi** tagliano la vegetazione di traverso alla via del fuoco. Finita una linea, ne tagliano un'altra più vicina al fuoco, entro 400 m dal punto stradale dell'esagono (`CREW_RANGE_M`).

  Viaggiano insieme all'autobotte e insieme ricevono gli ordini. La squadra **protegge anche le case** entro 300 m (`PROTECT_M`). Si rifornisce da sola, si ritira se il pericolo è troppo alto e può essere persa.
- Evacuazione e difesa sono **slegate**: le squadre non evacuano e l'evacuazione non muove squadre.

## 4. L'evacuazione (modello semplificato)

Non c'è più il modello individuale delle persone (IBM). Ogni casa è una famiglia con un numero di persone (`rocca::evac`).

- **Un borgo alla volta.** Il successivo si può evacuare quando l'ultima famiglia del borgo precedente è partita.
- Prima della conferma il gioco mostra il **percorso** verso l'area sicura più vicina fuori dal borgo e il **tempo**: preparazione (5 min) + partenze + viaggio.
- **Limite rigido:** al massimo **20 persone al minuto** escono dal borgo (`evacuation.per_min`). Le famiglie più vicine al punto di partenza partono per prime. In auto si viaggia a 8 m/s.
- **Strade chiuse dal fuoco:** sono segnate in rosso sulla mappa. Ogni minuto il percorso viene ricontrollato.
  - Se il fuoco taglia la strada, si cerca un altro percorso.
  - Se non ce n'è nessuno, l'evacuazione si **interrompe** e chi è ancora a casa passa in **autoprotezione**.
  - Una famiglia già in strada con la via tagliata cerca un'altra strada, altrimenti si ferma.
- **Quando il fuoco raggiunge le persone** (stessa regola per lo schermo e per le stelle):
  - a casa senza ordine: il pericolo alla porta arriva a `ALARMING`, oppure la casa è colpita;
  - in autoprotezione: la casa è colpita, oppure il pericolo alla porta arriva a `IMPASSABLE`;
  - in strada: il pericolo dove si trovano arriva a `IMPASSABLE`.

## 5. Stelle e classifica

- **Tre stelle da 1 a 5** (`rocca::score`), ognuna come quota risparmiata rispetto allo stesso incendio senza ordini:
  - **Persone:** persone non raggiunte dal fuoco;
  - **Case:** case non colpite;
  - **Ettari:** ettari non bruciati.

  Le soglie sono provvisorie.
- **Classifica del giorno:** prima le stelle Persone, poi Case, poi Ettari; a pari merito vince chi ha giocato prima (2026-10-14). Compare in apertura e nel finale. L'operatore la azzera.

## 6. Interfaccia

- La mappa occupa quasi tutto lo schermo; il rendering 3D del diorama è conservato.
- **Barra in alto:**
  - giorno e ora;
  - vento e umidità;
  - «Pausa: chiedi all'esperto» con il tempo di ricarica, oppure «PAUSA · ancora N min» e «Riprendi»;
  - in pausa, la spiegazione della previsione e il meteo.
- **Barra laterale:** la scelta in corso (squadra o borgo) con anteprima e «Conferma» / «Evacua»; l'elenco delle squadre con il loro stato; l'elenco dei borghi; i conteggi.
- **Sulla mappa:**
  - etichette cliccabili di squadre e borghi;
  - esagoni, in pausa o con una squadra scelta;
  - esagono scelto e percorso, tratteggiati in bianco;
  - strade percorse dalle squadre, in ciano;
  - evacuazione, in blu: tratteggiata se proposta, continua se in corso;
  - strade chiuse, in rosso;
  - isocrone della previsione e freccia del vento.
- **Avvisi:** l'ultimo evento, in una riga.
- **Controlli:**
  - mouse o trackpad;
  - trascinare per spostare la mappa, un clic senza trascinare per scegliere;
  - rotella o pizzico per lo zoom, tasto destro o Shift + trascinare per ruotare;
  - Home per la vista iniziale.

  Funziona offline e senza audio, in italiano.
- **Operatore** (F2 o «Menu»): nuova visita, vai al giorno 2, seme 1–3, «Ferma il mondo», vista iniziale, livello grafico, azzera la classifica.
- **Chiosco presidiato:** nessun reset per inattività.
- **Mario** (easter egg, `docs/MARIO.md`): **scollegato** dal 2026-10-14. Usava le persone dell'IBM e i vecchi stati del chiosco. Da decidere se ricollegarlo.

## 7. Fuori dal gioco (per ora)

- Personaggi, sindaca, preallerta, pattuglie porta a porta, coordinatore automatico, debriefing testuale: tolti il 2026-10-14. La comunicazione si riscrive dopo.
- Mezzi aerei e squadre a piedi: non previsti.
- Il territorio è dichiaratamente immaginario: nella realtà si seguono le indicazioni delle autorità.
