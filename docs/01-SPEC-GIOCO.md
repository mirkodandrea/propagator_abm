# 01 — Specifica del gioco

**Obiettivo:** esperienza strategica breve, di durata da calibrare nei playtest (può superare 5 minuti) per studenti delle superiori alla Settimana della Protezione Civile. **MVP:** un paese, poche risorse, almeno un dilemma significativo. Nessuna meccanica aggiuntiva finché questo non funziona.

## 1. Chi decide cosa

| Attore | Responsabilità |
|---|---|
| **Giocatore** | Seleziona **luoghi geografici** da difendere e li ordina per priorità. Decide separatamente **preallerta** ed **evacuazione** per quartiere; l'evacuazione solo dove la sindaca l'ha autorizzata (§3). |
| **Sindaca** | Autorità che firma l'ordinanza. **Autorizza l'evacuazione di un quartiere** quando la simulazione lo dice seriamente minacciato (§3) e apre un momento delle scelte. Non sceglie al posto del giocatore. |
| **Coordinatore operativo** | Individua i punti vulnerabili degli obiettivi, sceglie postazioni raggiungibili e assegna/riposiziona i mezzi secondo le priorità e i limiti operativi. |
| **Simulazione** | Determina propagazione, spostamenti, risposta delle famiglie, accessibilità, consumo di risorse, efficacia e ritirate delle squadre. |

Una priorità alta **non garantisce protezione**. Il coordinatore non può ordinare autonomamente evacuazioni né alterare l'ordine strategico scelto. La sicurezza delle squadre prevale sulle priorità.

### Luoghi e azioni

- **Abitato:** «Difendi questo quartiere», con posizione nella classifica. Il coordinatore sceglie **dove** difenderlo, non il giocatore.
- **Strada strategica:** «Tenta di mantenerla percorribile», **solo dopo verifica headless dell'effetto**. Difendere una strada non significa chiuderla al traffico.
- **Popolazione:** pulsanti espliciti **«Preallerta»** e **«Evacua»** riferiti a un quartiere. La preallerta **non** è un'evacuazione anticipata con un altro nome: deve informare/preparare senza imporre automaticamente la partenza. **La preallerta è sempre disponibile; «Evacua» solo per i quartieri già autorizzati dalla sindaca** (decisione 2026-10-13: «evacua tutti» non deve essere la strategia ovvia, e l'ordinanza spetta per legge al sindaco). Un'autorizzazione concessa resta valida fino alla fine della partita.

**MVP iniziale**: due quartieri, due autobotti, una squadra AIB; provare la strada come terzo obiettivo appena il modello la supporta. Ulteriori mezzi, quartieri e aerei soltanto se migliorano il gioco.

## 2. Loop di una partita

**Decisione 2026-10-13: due sole velocità, il mondo non si ferma mai.** Il gioco sceglie da solo la velocità alta; le scelte si fanno in un unico **momento delle scelte a ×1**, che sostituisce sia la pianificazione a ×0 sia le pause a mondo fermo. Si entra nel momento delle scelte in due modi: lo chiede il giocatore (pulsante «Decidi», ex «Pausa») o lo apre la sindaca autorizzando un'evacuazione (§3).

1. **Primo momento delle scelte (×1):** presentazione di fuoco, vento e paese; il fuoco è ancora piccolo, quindi è di fatto la pianificazione, ma il mondo è già vivo. Il giocatore ordina le priorità e può dare la preallerta. Sulla mappa appare il **piano proposto**. Finisce con «Conferma».
2. **Esegui (×N):** il tempo scorre accelerato, a velocità scelta dal gioco (non dal giocatore). Il coordinatore adatta le postazioni all'interno delle priorità confermate; unità e cittadini operano nel mondo simulato. Gli eventi che non aprono un momento delle scelte (strada tagliata, case raggiunte, mezzi arrivati) arrivano come notizie.
3. **Momento delle scelte chiesto dal giocatore (×1):** il fuoco continua ad avanzare. Il giocatore rivede le priorità, dà preallerte ed evacua i quartieri già autorizzati. Si esce con «Conferma» o «Annulla»; senza azioni, torna da solo a ×N dopo ~60 s reali. Limiti attuali (decisione 2026-10-09, da rivedere nei playtest perché ora il mondo non si ferma): al massimo 3 a partita, a ≥15 minuti simulati l'uno dall'altro.
4. **Momento delle scelte aperto dalla sindaca (×1):** «Fornaci può essere evacuata». Countdown reale (~45 s); stesse scelte del punto 3. Non consuma le richieste del giocatore.
5. **Conferma o scadenza:** si applica l'ultimo **piano proposto valido**, compresi i comandi civili selezionati esplicitamente. Se non ha fatto nulla, rimane il piano precedente. La partita torna a ×N.
6. **Conclusione:** il mondo mostra il risultato e un confronto con lo stesso incendio senza ordini. «Riprova» ripete lo stesso caso, «Altro incendio» cambia innesco/meteo sullo **stesso territorio**.

La velocità cambia solo il tempo reale, mai i risultati: tutto è misurato in tempo simulato, e il runner headless gioca le stesse regole ignorando le velocità (registra però dove si aprono i momenti delle scelte, per stimare la durata reale di un caso).

**Valori storici** (prima della decisione 2026-10-13; ×N resta valido, la pianificazione a ×0 no): 40 s reali di pianificazione iniziale, ×40 durante l'esecuzione (fino a ×120 nei tratti senza eventi), 25 s a ×1 durante una crisi, 0–2 crisi per partita. Aggiornamento del 2026-10-09: nei playtest ×20 (×60 nei tratti quieti) dava circa 9 minuti reali e un calo d'attenzione dopo gli ordini. L'utente ha chiesto di aumentare la velocità tra i turni. **Valori adottati nell'iterazione 5b: ×40, ×120 nei tratti quieti** (nessun evento da 10 min simulati e nessun mezzo in movimento). Il tempo di esecuzione si dimezza, per esempio Coste2_gira da 5,8 a 3,3 minuti reali, con le stesse crisi agli stessi istanti (`docs/fase5/durata.md`). **Nessun limite rigido alla durata complessiva**: la partita può superare 5 minuti se il ritmo regge. Un buon piano potrebbe non generare nessuna pausa critica; lo scenario deve comunque restare interessante.

## 3. Quando scatta una crisi

**Non a orari fissi e non a ogni notifica.** Un rilevatore osserva lo stato **attualmente conosciuto** e propone una finestra decisionale quando:

- un luogo scoperto sta diventando seriamente minacciato, **oppure** un cambiamento di vento/viabilità rende insufficiente il piano;
- esiste ancora **una risposta realizzabile** (o una scelta tra obiettivi in conflitto);
- il coordinatore non può risolvere il dilemma da solo mantenendo le priorità ricevute.

Per il prototipo bastano pochi trigger: quartiere minacciato ma non coperto; cambio vento che ribalta l'esposizione; strada/risorsa diventata indisponibile. Limitare ripetizioni dello stesso evento e numero di interruzioni. Il messaggio descrive **rischio, tempo stimato e compromesso**, non ordina al giocatore quale soluzione scegliere.

**Esempio:** «Le Coste sono ora a rischio; trasferire l'autobotte richiede circa 15 minuti e lascia scoperto Il Borgo». Le stime vanno derivate da dati disponibili, non dalla conoscenza del futuro effettivo.

### Autorizzazione della sindaca (2026-10-13)

La crisi tipica diventa l'**autorizzazione all'evacuazione di un quartiere**, uno alla volta: scatta quando la simulazione dice che il fronte attivo raggiungerà il quartiere entro una soglia di tempo (punto di partenza: i ~20 minuti simulati della crisi prevista attuale), calcolata con lo stesso criterio usato da grafica e debrief. Se due quartieri sono minacciati insieme arrivano due autorizzazioni vicine: è lì che nascono i compromessi (pattuglie, code verso il porto, chi era già preallertato parte prima).

Da verificare headless **prima** della UX:

- la soglia arriva quando evacuare è ancora possibile, ma non così presto da rendere l'evacuazione sempre la scelta ovvia;
- «evacua appena autorizzato» **non domina** «preallerta mirata + evacuazione selettiva» né «solo preallerta» (`strategie_civili`, layout attuale, semi 1–3);
- durata reale stimata per caso (tempo a ×N + momenti a ×1).

Gli altri trigger (cambio di vento, strada o risorsa persa) restano notizie finché un test non mostra che serve una decisione.

## 4. Anteprima e tempo ×1

Durante la finestra decisionale esistono **due piani distinti**:

- **Attivo:** le unità continuano ad eseguirlo e gli abitanti continuano a muoversi a ×1.
- **Proposto:** cambia quando il giocatore riordina le priorità; mostra sulla mappa **destinazioni, tragitti, ETA, coperture possibili e rinunce**. Non muove realmente i mezzi finché non viene confermato.

Il piano proposto si ricalcola usando lo stato più recente quando necessario; il commit verifica che sia ancora possibile. Mai mostrare come garantiti edifici «salvati» o traiettorie future del fuoco. Se un intervento è impossibile, comunicarne la ragione.

## 5. Interfaccia minima

**Riciclare il diorama esistente, non l'UX.** Schermo prevalentemente occupato dalla mappa, con:

- luoghi cliccabili e priorità chiaramente leggibili/modificabili;
- stato delle squadre **sulla mappa**, rotte attive e rotte proposte distinguibili;
- indicazione del vento, del rischio e del tempo (×N / ×1, con countdown quando il momento delle scelte l'ha aperto la sindaca);
- due azioni chiare per i cittadini: **Preallerta / Evacua**; «Evacua» visibilmente bloccato, con il motivo («serve l'autorizzazione della sindaca»), finché il quartiere non è autorizzato;
- un pulsante **Decidi** durante ×N (con le richieste rimaste) e un solo pulsante centrale **Conferma** nel momento delle scelte;
- finale molto breve: persone rimaste in pericolo, case **colpite** (non chiamarle «distrutte» se è un proxy), effetto di almeno una scelta e possibilità di riprovare;
- **tempi e motivi dallo stato reale:** arrivi dei mezzi da ciò che stanno facendo, non dall'anteprima; quando un luogo prioritario resta senza mezzi, il motivo si legge nella sua scheda, già prima della conferma;
- **vegetazione riconoscibile a colpo d'occhio** (decisione 2026-10-09): pini marittimi (pineta), castagni (latifoglie), cespugli (macchia) e prateria distinti per forma e colore anche nella vista generale; una legenda breve dice perché contano (l'erba corre, la macchia brucia forte, il bosco è più lento). Solo grafica: il combustibile simulato non cambia.

Niente controlli manuali dei mezzi, disegno libero di poligoni, lunghe spiegazioni o menù da gestionale. La prima azione deve essere intuitiva; il chiosco sarà **presidiato** e potrà essere introdotto dal responsabile. Gioco completamente offline; **nessun reset automatico per inattività**. Sono previsti avvio di una nuova partita e riavvio **manuali**, con accesso operatore semplice. Il timer dei momenti delle scelte aperti dalla sindaca (×1) rimane attivo come parte del gameplay; il mondo non si ferma mai per il giocatore (l'operatore può fermarlo dalla barra F2).

## 6. Cosa rendere significativo prima di rifinire

L'esperienza funziona soltanto se, con **stesso territorio, stesso incendio e stessi seed**:

1. Invertire due priorità cambia assegnazioni **e almeno in alcuni casi il risultato**.
2. La scelta di concentrarsi su un obiettivo ha un costo osservabile su un altro.
3. Agire alla finestra critica può ancora migliorare il risultato; reagire troppo tardi può non bastare.
4. Preallerta ed evacuazione sono distinte e producono conseguenze coerenti sul comportamento degli abitanti.
5. Le conseguenze sono leggibili dalla mappa e comprensibili nel debrief.

Valutare i **danni e i tempi realmente prodotti dal modello**, separando eventuali proxy di gioco. Non introdurre effetti inventati solo per rendere vincenti le scelte. Gli indicatori numerici esatti e il tuning verranno fissati dopo i primi sweep.

## 7. Limiti della prima versione

Il gioco **non** è un addestramento operativo per il pubblico. Non include procedure reali da seguire in caso di incendio. Lo scenario è dichiaratamente fittizio, pur basato su una simulazione. Nessun punteggio artificiale per «risorse comando», nessun obiettivo astratto globale e nessun sistema che faccia automaticamente tutte le scelte per il giocatore.
