# 01 — Specifica del gioco

**Obiettivo:** esperienza strategica breve, di durata da calibrare nei playtest (può superare 5 minuti) per studenti delle superiori alla Settimana della Protezione Civile. **MVP:** un paese, poche risorse, almeno un dilemma significativo. Nessuna meccanica aggiuntiva finché questo non funziona.

## 1. Chi decide cosa

| Attore | Responsabilità |
|---|---|
| **Giocatore** | Seleziona **luoghi geografici** da difendere e li ordina per priorità. Decide separatamente **preallerta** ed **evacuazione** per quartiere. |
| **Coordinatore operativo** | Individua i punti vulnerabili degli obiettivi, sceglie postazioni raggiungibili e assegna/riposiziona i mezzi secondo le priorità e i limiti operativi. |
| **Simulazione** | Determina propagazione, spostamenti, risposta delle famiglie, accessibilità, consumo di risorse, efficacia e ritirate delle squadre. |

Una priorità alta **non garantisce protezione**. Il coordinatore non può ordinare autonomamente evacuazioni né alterare l'ordine strategico scelto. La sicurezza delle squadre prevale sulle priorità.

### Luoghi e azioni

- **Abitato:** «Difendi questo quartiere», con posizione nella classifica. Il coordinatore sceglie **dove** difenderlo, non il giocatore.
- **Strada strategica:** «Tenta di mantenerla percorribile», **solo dopo verifica headless dell'effetto**. Difendere una strada non significa chiuderla al traffico.
- **Popolazione:** pulsanti espliciti **«Preallerta»** e **«Evacua»** riferiti a un quartiere. La preallerta **non** è un'evacuazione anticipata con un altro nome: deve informare/preparare senza imporre automaticamente la partenza.

**MVP iniziale**: due quartieri, due autobotti, una squadra AIB; provare la strada come terzo obiettivo appena il modello la supporta. Ulteriori mezzi, quartieri e aerei soltanto se migliorano il gioco.

## 2. Loop di una partita

1. **Pianifica (×0):** presentazione immediata di fuoco, vento e paese. Il giocatore ordina le priorità e può decidere preallerta/evacuazione. Sulla mappa appare il **piano proposto**.
2. **Esegui (×N):** il tempo scorre accelerato. Il coordinatore adatta le postazioni all'interno delle priorità confermate; unità e cittadini operano nel mondo simulato.
3. **Crisi (×1):** solo se appare un cambiamento strategico significativo, il gioco rallenta automaticamente. Il fuoco continua ad avanzare. Il giocatore ha un breve timer per rivedere le priorità e/o dare ordini ai civili.
4. **Conferma o scadenza:** si applica l'ultimo **piano proposto valido**, compresi i comandi civili selezionati esplicitamente. Se non ha fatto nulla, rimane il piano precedente. La partita torna a ×N.
5. **Conclusione:** il mondo mostra il risultato e un confronto con lo stesso incendio senza ordini. «Riprova» ripete lo stesso caso, «Altro incendio» cambia innesco/meteo sullo **stesso territorio**.

**Valori indicativi**, da testare: 40 s reali di pianificazione iniziale, ×20 durante l'esecuzione, 25 s a ×1 durante una crisi, 0–2 crisi per partita. **Nessun limite rigido alla durata complessiva**: la partita può superare 5 minuti se il ritmo regge. Un buon piano potrebbe non generare nessuna pausa critica; lo scenario deve comunque restare interessante.

## 3. Quando scatta una crisi

**Non a orari fissi e non a ogni notifica.** Un rilevatore osserva lo stato **attualmente conosciuto** e propone una finestra decisionale quando:

- un luogo scoperto sta diventando seriamente minacciato, **oppure** un cambiamento di vento/viabilità rende insufficiente il piano;
- esiste ancora **una risposta realizzabile** (o una scelta tra obiettivi in conflitto);
- il coordinatore non può risolvere il dilemma da solo mantenendo le priorità ricevute.

Per il prototipo bastano pochi trigger: quartiere minacciato ma non coperto; cambio vento che ribalta l'esposizione; strada/risorsa diventata indisponibile. Limitare ripetizioni dello stesso evento e numero di interruzioni. Il messaggio descrive **rischio, tempo stimato e compromesso**, non ordina al giocatore quale soluzione scegliere.

**Esempio:** «Le Coste sono ora a rischio; trasferire l'autobotte richiede circa 15 minuti e lascia scoperto Il Borgo». Le stime vanno derivate da dati disponibili, non dalla conoscenza del futuro effettivo.

## 4. Anteprima e tempo ×1

Durante la finestra decisionale esistono **due piani distinti**:

- **Attivo:** le unità continuano ad eseguirlo e gli abitanti continuano a muoversi a ×1.
- **Proposto:** cambia quando il giocatore riordina le priorità; mostra sulla mappa **destinazioni, tragitti, ETA, coperture possibili e rinunce**. Non muove realmente i mezzi finché non viene confermato.

Il piano proposto si ricalcola usando lo stato più recente quando necessario; il commit verifica che sia ancora possibile. Mai mostrare come garantiti edifici «salvati» o traiettorie future del fuoco. Se un intervento è impossibile, comunicarne la ragione.

## 5. Interfaccia minima

**Riciclare il diorama esistente, non l'UX.** Schermo prevalentemente occupato dalla mappa, con:

- luoghi cliccabili e priorità chiaramente leggibili/modificabili;
- stato delle squadre **sulla mappa**, rotte attive e rotte proposte distinguibili;
- indicazione del vento, del rischio e del tempo (×N / ×1, con countdown);
- due azioni chiare per i cittadini: **Preallerta / Evacua**;
- un solo pulsante centrale **Conferma / Continua**;
- finale molto breve: persone rimaste in pericolo, case **colpite** (non chiamarle «distrutte» se è un proxy), effetto di almeno una scelta e possibilità di riprovare.

Niente controlli manuali dei mezzi, disegno libero di poligoni, lunghe spiegazioni o menù da gestionale. La prima azione deve essere intuitiva; il chiosco sarà **presidiato** e potrà essere introdotto dal responsabile. Gioco completamente offline; **nessun reset automatico per inattività**. Sono previsti avvio di una nuova partita e riavvio **manuali**, con accesso operatore semplice. Il timer delle crisi (×1) rimane attivo come parte del gameplay.

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
