# 01 — Il gioco: Isola di Roccavento

Regole **in vigore** dal 2026-10-14 (nuovo progetto). Prevalgono sul codice: se non coincidono, segnalarlo. Ogni cambio di regola è una decisione dell'utente, da scrivere qui con la data (lo storico è nella storia git).

## In una frase

Uno strategico in tempo reale, sul modello di StarCraft, per il chiosco della *Settimana della Protezione Civile* (studenti delle superiori). Un incendio simulato minaccia i borghi di un'isola. Il giocatore **mette in pausa** per chiedere la previsione all'esperto, poi **sposta le squadre** su una griglia esagonale e **evacua i borghi**, uno alla volta. La simulazione decide gli esiti. Poche parole: la comunicazione (personaggi, messaggi, debriefing) si riscrive in un secondo momento.

**Obiettivo educativo:** incertezza, tempi, risorse limitate, adattarsi. Non è un addestramento e non insegna procedure operative.

## 1. Struttura della partita

1. **Apertura:** nome del giocatore, «TUTORIAL» (prova guidata) o «INIZIA» (salta al giorno 2) e la classifica di oggi.
2. **Tutorial, giorno 1** (`rocca::tutorial`): un incendio piccolo vicino a Le Ghiande, con dinamiche reali. **Parte in pausa**, come il gioco vero (2026-10-14). Passi: la scena e la previsione («Avanti»); la vita dei borghi (anello che si svuota con le case raggiunte dal fuoco, da verde a giallo a rosso; «Avanti», 2026-10-15); i due ordini, trascinando il cono dei vigili di Castelvento e quello dei volontari di Pian dei Grilli sull'esagono indicato; spiegazione degli esagoni delle squadre (giallo fisso sugli esagoni assegnati; «Avanti», 2026-10-15); «Riprendi»; il mondo corre da solo per **30 min simulati** (15 s reali a ×120, `tutorial::RUN_MIN`) e **si ferma da solo**, e il testo dice che nella partita vera la pausa la deve premere il giocatore; evacuazione di Le Ghiande; «Riprendi» **fino alla fine dell'evacuazione**: il tutorial finisce lì (2026-10-14), non a fuoco spento; la barra del tempo dura quanto il caso `tutorial` (60 min, la fine reale è alle 11:51). Il gioco accetta **solo** l'azione richiesta e il mondo resta fermo finché il giocatore non la compie, quindi l'esito è scriptato. Senza ordini, nei 60 min colpisce 3 case e 20 persone.
3. **Giorno 2:** nuovo giorno, nuovo incendio (`game.json` → `game`, oggi il caso `medium`, 5 ore). Il controllo passa al giocatore. La partita **parte in pausa**, senza limite di tempo, per esplorare la scena e dare i primi ordini; «Via!» fa partire il tempo e non consuma l'esperto (2026-10-14). L'incendio è **segnalato in ritardo**: quando la partita comincia brucia già da `start_min` minuti (2026-10-14).
4. **Fine:** stelle e classifica del giorno.

## 2. Il tempo e la pausa

- Nessun turno. Il tempo scorre a **×120** o è **fermo**, nient'altro (2026-10-15, richiesta dell'utente; prima ×60): 1 s reale vale 2 min simulati (`rocca::RUN_X`).
- **Pausa = chiedere all'esperto scientifico.** In pausa il **mondo si ferma** (2026-10-14: «pausa è stop, non tempo rallentato»). La pausa **non ha limite di tempo** (2026-10-14, richiesta dell'utente): finisce quando il giocatore preme «Riprendi». **Nessuna attesa tra una pausa e la successiva** (2026-10-16, richiesta dell'utente: tolti `PAUSE_COOLDOWN_REAL_S` e `PAUSE_COOLDOWN_S`).
- **In pausa il piano si può cambiare:** una squadra si sposta di nuovo trascinando la sua icona; un'evacuazione ordinata nella stessa pausa si annulla, oppure si sostituisce scegliendo un altro borgo (`Game::cancel_evacuation`). Dopo la ripresa no (2026-10-14).
- **Tutti gli ordini si danno solo in pausa:** spostare le squadre ed evacuare.
- In pausa compaiono:
  - la **previsione**: isocrone al 50 % di 16 simulazioni dal fuoco attuale, a +1, +2 e +3 h, in **tre sfumature di rosso** (+1 h la più intensa, +3 h la più chiara), sottili e con l'etichetta («tra 1 ora»…; 2026-10-16). Le etichette non si sovrappongono sullo schermo, senza interventi (`rocca::forecast`);
  - il **bollettino meteo**: vento attuale e cambio annunciato;
  - l'**umidità del combustibile**.
- Durante il tutorial il tempo è fermo mentre un passo aspetta il giocatore.
- La partita finisce quando il fuoco è spento e nessuno è in strada, oppure alla durata del caso.

## 3. Le squadre

- **Una squadra** è un'autobotte con 3 persone. Sono tutte squadre **AIB**, senza distinzione di colore o di nome (2026-10-16): «Squadra A»…«Squadra E», in giallo. 3 stanno al presidio di Castelvento, il borgo più grande, e 2 al presidio di Pian dei Grilli, un borgo minore a ovest, dall'altra parte dell'isola rispetto a Castelvento (`game.json`: `stations`, `roster`).
- Si spostano su una **griglia esagonale** di 150 m tra centri adiacenti (`rocca::hex`; 2026-10-15, dimezzati da 300 m). Si può scegliere solo un esagono attraversato da una strada carrabile della rete principale: la squadra guida fino al punto stradale dell'esagono più vicino al centro.
- Ogni squadra ha una **grande icona** (autobotte e numero), non più un cono, sopra il punto dove è assegnata: sull'esagono dell'ultimo ordine, oppure alla sua sede (`kiosk::pins`). Sul cono, un'etichetta con autobotte e numero. Il veicolo non ha icone sopra (2026-10-14).
- In pausa si **trascina l'icona della squadra** sull'esagono scelto: durante il trascinamento il gioco mostra **percorso e tempo di viaggio**, evitando le strade chiuse dal fuoco (`Game::plan_move`). **Lasciata l'icona, l'ordine è dato**, senza conferma (2026-10-14). Anche un clic sul cono e poi sull'esagono funziona.
- Il badge della squadra mostra la **lettera** (non un numero) e **lampeggia in rosso** quando è in pericolo (`Game::team_in_danger`); si ritira da sola (regola già in vigore, `UnitState::Withdrawing`). La zona coperta dalle squadre schierate è tracciata solo con il **contorno** sottile, in giallo AIB, non con gli esagoni pieni (2026-10-16).
- Con una squadra scelta o trascinata, il chiosco **illumina gli esagoni dell'area d'intervento** (fino a 2 esagoni dal suo, `TEAM_REACH_HEXES`, 2026-10-15), in azzurro; non si illuminano se l'esagono è rifiutato (rosso).
- Arrivata, la squadra **attacca il fuoco come può** (2026-10-14), con il modello di spegnimento conservato (`abm::suppression`):
  - l'**autobotte** aumenta l'umidità del combustibile entro **100 m** da dove sta (acqua, manichetta). Durante l'intervento si sposta **sulle strade degli esagoni di competenza** (il suo e quelli intorno) verso il combustibile minacciato, senza avvicinarsi a meno di 80 m dalle fiamme (2026-10-15, richiesta dell'utente);
  - l'**autobotte va solo su strade carrabili**; le persone viaggiano a bordo, scendono dove si ferma e vanno **a piedi in linea retta** (1,1 m/s, rallentate dalla pendenza) al lavoro. Sono unità **separate**: si dispongono a ventaglio di 60 m (`CREW_SPREAD_M`) intorno al punto stradale e ognuna taglia la propria linea. A un nuovo ordine chi è a piedi torna all'autobotte, che parte quando sono tutti a bordo (2026-10-15, richiesta dell'utente);
  - le **persone a piedi** tagliano la vegetazione di traverso alla via del fuoco. Finita una linea, ne tagliano un'altra più vicina al fuoco, entro **2 esagoni** intorno (300 m) dal punto stradale dell'esagono (`CREW_RANGE_M`; 2026-10-15, prima 400 m).

  Viaggiano insieme all'autobotte e insieme ricevono gli ordini. La squadra **protegge anche le case** entro 300 m (`PROTECT_M`). Quando l'autobotte ha finito l'acqua **resta dov'è** e la ricarica è implicita: torna piena dopo **10 min simulati** (`REFILL_S`); intanto continua a proteggere le case vicine (2026-10-14, prima andava agli idranti). Si ritira se il pericolo è troppo alto e può essere persa.
- Evacuazione e difesa sono **slegate**: le squadre non evacuano e l'evacuazione non muove squadre.

## 4. L'evacuazione (modello semplificato)

Non c'è più il modello individuale delle persone (IBM). Ogni casa è una famiglia con un numero di persone (`rocca::evac`).

- **Un borgo alla volta.** Il successivo si può evacuare quando l'ultima famiglia del borgo precedente è partita.
- **Passando sopra il borgo** il gioco mostra già il **percorso** (tratteggiato) verso l'area sicura più vicina fuori dal borgo e il **tempo**: fino all'arrivo dell'ultima famiglia all'area sicura, cioè preparazione (5 min), partenze e viaggio. Si mostrano solo persone e minuti, senza le parti né la destinazione (2026-10-14, semplificazione). Poi si clicca il borgo e si preme «Evacua» nel riquadro.
- **Aree di raccolta scelte a mano** (2026-10-14): ogni borgo (tranne Le Ghiande) ha la sua, facile da raggiungere e fuori dal bosco del valico (`evacuation.refuges` in `game.json`, elenco in `02-TERRITORIO.md`). Se la strada è chiusa dal fuoco, o il borgo non ne ha una, vale l'area sicura automatica più vicina. Senza l'elenco il meccanismo è inerte.
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

  Le soglie sono provvisorie. Dal 2026-10-16 il finale spiega quantità e percentuale del danno evitato, distingue peggioramenti e propone cosa osservare alla prossima prova; include tre messaggi di protezione civile per i cittadini, con fonte ufficiale, anche offline. La scala più generosa è pronta **solo come proposta**, con prove in [`04-VALUTAZIONE.md`](04-VALUTAZIONE.md): le soglie del gioco non cambiano prima della scelta dell'utente.
- **Classifica del giorno:** prima le stelle Persone, poi Case, poi Ettari; a pari merito vince chi ha giocato prima (2026-10-14). Compare in apertura e nel finale. L'operatore la azzera.

## 6. Interfaccia

- La mappa occupa quasi tutto lo schermo; il rendering 3D del diorama è conservato.
- **Pausa:** un pulsante rotondo e grande, in basso al centro, **dentro il pannello del tempo** (2026-10-16: prima in alto a sinistra) (giorno e ora, barra di avanzamento grande). Pronto: anello ambra e simbolo di pausa. In pausa: simbolo di avvio, anello verde pieno, nessun conto alla rovescia. In gioco: sempre pronto, nessun conto alla rovescia (2026-10-16). In generale si usano icone, anelli e numeri al posto delle frasi.
- **Barra in alto:**
  - giorno e ora;
  - vento e umidità;
  - vento (freccia vista dalla camera) e umidità (goccia); in pausa la legenda delle isocrone e il cambio di vento previsto (orologio, ora, freccia).
  - in pausa, la spiegazione della previsione e il meteo.
- **Barra laterale:** solo informazioni, nessun ordine: l'elenco delle squadre con il loro stato; l'elenco dei borghi; i conteggi. Gli ordini si danno sulla mappa (2026-10-14).
- **Sulla mappa:**
  - icone delle squadre, dove sono assegnate, da trascinare (in pausa), e etichette dei borghi: passandoci sopra, percorso e persone e minuti dell'evacuazione; al clic il riquadro con persone, minuti e «Evacua» / «Annulla»;
  - sul borgo in evacuazione, un **timer circolare**: l'anello si svuota col tempo che resta fino all'arrivo dell'ultima famiglia, con i minuti al centro. Parte dai minuti annunciati e scende di un minuto per minuto simulato: annuncio e timer vengono dalla stessa regola (`Evacuations::remaining_min`, 2026-10-14); a fine evacuazione una spunta, se il fuoco l'ha interrotta un avviso;
  - la **chat di gruppo** (2026-10-16): in alto a sinistra (2026-10-16, spostata dall'utente), righe brevi e colloquiali con ritratto piccolo, ora ed emoji, dette da squadre, sindaca ed esperto su ciò che accade (partenza, arrivo, acqua finita, ritirata, evacuazione partita/deviata/completata, vento). Sul telefono compare solo come notifica in alto. Il testo viene dal nucleo (`Game::chat`);
  - i segni sulla mappa (esagoni, percorsi, strade chiuse, isocrone) hanno un **materiale loro** (`flat`): colore pieno e acceso, senza luce né nebbia, con un bordo scuro, disegnati sopra la vegetazione (2026-10-14);
  - esagoni, in pausa o con una squadra scelta;
  - esagono scelto e percorso, tratteggiati in bianco;
  - strade percorse dalle squadre, in ciano;
  - evacuazione, in **verde** (il colore della via di fuga, diverso dal ciano delle squadre): tratteggiata se proposta, continua se in corso; lo stesso verde nel riquadro e nel timer (2026-10-14);
  - strade chiuse, in rosso;
  - isocrone della previsione e freccia del vento.
- **Avvisi:** l'ultimo evento, in una riga.
- **Schermate** (`kiosk::screens`, `kiosk::characters`; stile e idee del vecchio chiosco, 2026-10-14). A parlare sono **volti** in fumetto, con il ruolo sopra: la **volontaria** (guida: apertura, tutorial, giorno 2, finale), l'**esperto** e i **vigili del fuoco** (solo nella riga delle persone con cui si lavora):
  - **apertura:** carta centrale con logo CIMA e titolo, la volontaria che dice che cosa è in gioco (squadre e borghi), «Le persone con cui lavori», «Come ti chiami?», «Inizia» e «Salta la prova guidata», avvertenza sul territorio immaginario; accanto, «Classifica di oggi»;
  - **tutorial:** il fumetto della volontaria in alto, al centro (2026-10-16, prima in basso: ora lì sta il pannello del tempo; grande, testo breve; `**azione**` in arancione e `__luogo__` in blu, 2026-10-15), con il passo (n/N), «Avanti» nei passi di sola lettura e «tocca a te» negli altri; a ogni passo la **camera vola** a ciò di cui si parla (l'innesco, la squadra e l'esagono, il borgo) e per 7 s una **cornice ambra** lampeggiante evidenzia ciò che si deve toccare (pulsante della pausa in basso al centro, bandiera e esagono, borgo); un trascinamento della mappa interrompe il volo; la mappa resta usabile, ma il gioco accetta solo l'azione richiesta; «Salta il tutorial» o Esc;
  - **giorno 2:** carta con il fumetto della volontaria e «Inizia»;
  - **finale:** «Com'è andata, <nome>», le stelle grandi di Persone, Case, Ettari con che cosa contano, il verdetto della volontaria e il confronto con il fuoco senza ordini; a destra il posto in classifica, la riga di avvertenza sulla realtà («quando arriva l'ordine di evacuazione si parte subito») e «Nuova partita».
- **Web su telefono** (2026-10-15, richiesta dell'utente): supporto verticale e orizzontale, profilo grafico leggero e memoria ridotta; pannelli e messaggi compatti e leggibili, controlli touch con trascinamento delle squadre e alternativa «tocca squadra, poi destinazione». Dettagli e verifiche in `web.md`. Le regole della partita restano condivise.
- **Controlli:**
  - mouse o trackpad;
  - trascinare per spostare la mappa, un clic senza trascinare per scegliere;
  - rotella o pizzico per lo zoom, tasto destro o Shift + trascinare per ruotare;
  - Home per la vista iniziale.

  Funziona offline e senza audio, in italiano.
- **Operatore** (F2 o «Menu»): selettore scenario (Attuale, Attraversamento del valico, Pressione su quattro borghi, Rotazione da est), nuova visita, vai al giorno 2, seme 1–3 (predefinito 2, utente 2026-10-15), «Ferma il mondo», vista iniziale, livello grafico, azzera la classifica.
  Il selettore operatore si applica con «Nuova visita» o «Vai al giorno 2», senza interrompere la partita alla sola selezione. La scelta resta per le visite successive nella stessa sessione; al riavvio torna `medium`, lo scenario attuale predefinito. Tutorial invariato. Classifiche e confronto senza ordini usano il caso effettivamente giocato, anche se l’operatore ha già selezionato il successivo.
- **Chiosco presidiato:** nessun reset per inattività.
- **Mario** (easter egg, `docs/MARIO.md`): **scollegato** dal 2026-10-14. Usava le persone dell'IBM e i vecchi stati del chiosco. Da decidere se ricollegarlo.

## 7. Fuori dal gioco (per ora)

- Sindaca, preallerta, pattuglie porta a porta, coordinatore automatico: tolti il 2026-10-14. La comunicazione si riscrive dopo.
- Mezzi aerei e squadre a piedi: non previsti.
- Il territorio è dichiaratamente immaginario: nella realtà si seguono le indicazioni delle autorità.
