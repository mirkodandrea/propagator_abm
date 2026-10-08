# Rocca Ventosa: otto ore al chiosco, F2 e tanta fede

Colleghi, mi hanno consegnato il chiosco con una formazione completa: «F2». Praticamente un corso di sala operativa in due caratteri.

F2 apre davvero la barra. Sopra la legenda. Per leggere il fuoco attivo devo prima spegnere l'operatore: finalmente la prevenzione degli incendi applicata alle finestre.

Ho avviato Borgo1, mandato difesa e preallerta, poi scelto Piano1. La barra dice Piano1, ma sul territorio continua la partita del Borgo. La scelta prepara il prossimo incendio; peccato che la barra non lo dica. Io dopo sette ore e una scolaresca penserei di aver trasferito anche il vento.

«Nuova partita» funziona benissimo: un click e il turno in corso evapora, senza conferma. Due click a raffica? Sempre prontissimo ad azzerare. Per rimettere in piedi il chiosco bastano pochi secondi; per restituire ai ragazzi il turno cancellato, auguri.

La pausa è un piccolo escape room: premo «Pausa», il pulsante sparisce, compare «PIANIFICA» e per ripartire devo premere «Conferma e avvia». Cercavo «Riprendi», ho trovato una seconda inaugurazione.

Ho ristretto la finestra a 900×600: Il Borgo si mangia Il Piano, la legenda si mescola alle schede e il titolo invade i comandi della vista. L'esercitazione diventa: evacuare i pulsanti. Riallargare risolve in pochi secondi.

Ricarica vera a partita inoltrata: ritorno a T+0, incendio iniziale vicino a Le Coste e barra chiusa. Due secondi circa per tornare giocabili, tutto il progresso buttato.

Per onestà: click strani e raffiche non hanno fatto crollare il gioco. Mi sono perso zoomando e trascinando tra gli alberi: «Vista iniziale» mi ha salvato con un click. Lasciato senza input per oltre tre minuti, ha continuato a funzionare. Nessun crash osservato. Le otto ore, però, le deve ancora fare lui: io ho verificato solo questo giro.

---

## Traduzione per gli sviluppatori

Test del 9 ottobre 2026, solo interfaccia su `http://localhost:8765/`: nessun accesso a codice o documentazione. Scheda mantenuta in primo piano; `document.visibilityState` verificato come `visible`. Avviati Borgo1 e Piano1; caricata anche Coste1 dalla barra. Tempi di recupero indicativi dell'interazione automatizzata, non misure su un volontario inesperto. Gli eventi sul canvas sono stati simulati con entrambe le famiglie pointer e mouse; normalmente 150 ms tra eventi, ridotti nelle raffiche. I risultati non certificano la resa degli input su un chiosco fisico.

### 1. Cancellazione immediata del turno dalla barra

- **Visto:** durante Borgo1, con priorità, preallerta e tre mezzi assegnati, selezionato Piano1 e premuto «Nuova partita». Ritorno immediato a T+0:00, ordini azzerati, nuova pianificazione. Nessuna conferma. Ripetuti anche due click consecutivi su «Nuova partita» in Piano1 senza blocchi.
- **Certezza:** alta sul comportamento; il rischio di cancellazione accidentale è una valutazione operativa, non un incidente osservato con studenti.
- **Recupero:** circa 1–3 secondi per una nuova pianificazione; il turno abbandonato non è recuperabile tramite comandi visibili.
- **Correzione:** chiedere conferma solo se una partita è già iniziata, mostrando scenario e tempo che verranno persi; offrire annullamento prima di applicare il reset.

### 2. Layout inutilizzabile in parte a finestra stretta

- **Visto:** Borgo1 in corso, finestra ridimensionata da 1800×906 a 900×600. Schede di Le Coste, Il Piano e Il Borgo sovrapposte; parti dei comandi del Piano coperte. Intestazione sovrapposta alla barra F2 e ai controlli della vista; registro eventi e messaggio inferiore affollati. [Screenshot](02-finestra-stretta.jpg).
- **Certezza:** alta per queste dimensioni. Non testate tutte le risoluzioni del chiosco.
- **Recupero:** ritorno a 1800×906 in circa 3 secondi; partita e avanzamento conservati.
- **Correzione:** disporre schede e pannelli in zone che non si sovrappongano; a dimensioni ridotte usare un elenco laterale scorrevole e una sola scheda espansa.

### 3. Ricarica senza ripristino di partita o configurazione selezionata

- **Visto:** Piano1 in corso a T+1:27 di 3:00, dopo oltre tre minuti senza input. Ricarica esplicita del browser: nuova pianificazione a T+0:00 vicino a Le Coste, F2 chiusa. [Schermata dopo ricarica](04-ricarica-reset.jpg).
- **Certezza:** alta. Non osservato alcun comando visibile per recuperare il turno; non verificata l'esistenza di altri meccanismi.
- **Recupero:** ricarica completata in circa 2 secondi, subito giocabile; scenario da riselezionare e progresso perso.
- **Correzione:** conservare localmente scenario, configurazione e stato; dopo reload offrire «Riprendi turno» oppure «Nuovo gruppo». Se il reset è deliberato, comunicarlo chiaramente.

### 4. Pausa e ripresa hanno nomi e collocazioni incoerenti

- **Visto:** Piano1 avviata, click «Pausa» a circa T+0:12. Il tempo si ferma, stato superiore «PIANIFICA», pulsante Pausa assente dalla barra; ritorna «Conferma e avvia» in basso. Quel comando riprende il tempo conservando T+0:12. [Screenshot](03-pausa-diventa-pianificazione.jpg).
- **Certezza:** alta sul comportamento. Può essere una scelta voluta per rivedere gli ordini, ma la distinzione rispetto all'avvio è poco esplicita.
- **Recupero:** un click sul comando inferiore, circa 1–2 secondi una volta individuato; il tempo per scoprirlo non è stato misurato.
- **Correzione:** mostrare «In pausa — modifica il piano» e «Conferma e riprendi», mantenendo anche una ripresa riconoscibile nella barra.

### 5. Il selettore non distingue scenario attuale e prossimo scenario

- **Visto:** durante Borgo1 selezionato Piano1 senza premere Nuova partita. La barra indica Piano1, mentre vento da nord, mezzi e ordini del Borgo continuano. Il cambio effettivo avviene solo con Nuova partita. [Screenshot](05-selettore-piano-partita-borgo.jpg).
- **Certezza:** alta; non è una prova che la simulazione cambi erroneamente scenario.
- **Recupero:** circa 1–3 secondi premendo Nuova partita, ma ciò cancella il turno; continuare il turno precedente non richiede recupero tecnico.
- **Correzione:** etichettare «Scenario per la prossima partita» e mostrare separatamente «Partita attuale: Borgo1».

### 6. F2 copre la legenda

- **Visto:** già nella pianificazione iniziale a 1800×906, barra operatore sopra titolo e prime voci della legenda; persiste in partita. [Screenshot](01-barra-operatore.jpg).
- **Certezza:** alta.
- **Recupero:** F2 chiude la barra in meno di un secondo, ma nasconde anche i comandi operatore.
- **Correzione:** riservare alla barra una fascia dedicata o spostare la legenda quando è aperta.

## Registro delle altre prove e limiti

- **Avvio e cambi:** Borgo1 avviata con difesa/preallerta; Piano1 avviata senza ordini; Coste1 caricata dopo reload. Nuova partita prepara la pianificazione, poi serve «Conferma e avvia». Nessun blocco nei passaggi osservati.
- **Click a raffica e punti strani:** bordi, terreno, area mezzi, zoom e Nuova partita. Nessun crash o stato evidentemente corrotto. Le raffiche erano simulate, non input fisici di una classe.
- **Mappa:** dodici click sul + e trascinamento prolungato hanno portato la vista tra gli alberi, senza territorio riconoscibile. «Vista iniziale» ha ripristinato l'inquadratura in un click, circa un secondo. Non serve riavviare.
- **Tasti:** inviati Esc, quattro frecce, F11 e F5. Nessuna alterazione evidente nella schermata osservata. F11 non ha prodotto un fullscreen verificabile e F5 non ha prodotto un reload osservabile: gli effetti delle scorciatoie del browser restano non verificati con questo canale. Il reload è stato quindi eseguito esplicitamente tramite il browser, con risultato descritto sopra.
- **Inattività:** nessun input tra 00:15:59 e 00:19:42 locali, circa 3 minuti e 43 secondi. Piano1 passa da circa T+0:12 a T+1:27 a velocità ×20. Osservazioni intermedie senza interazioni: nessun blocco o rallentamento evidente. Non misurati FPS o memoria.
- **Velocità:** ×20 visibile e avanzamento coerente nell'attesa. Tentativo di editarla tramite click e tastiera rimasto a 20: modifica non validata, esclusa dalle critiche perché può dipendere dall'automazione.
- **Non verificato:** turno continuo di otto ore, finale di una partita completa, gestione delle Decisioni critiche e relativo timeout. Nessun crash osservato nelle prove effettuate; non è una garanzia di stabilità prolungata.

## Tre cose che funzionano e vanno tenute

1. **Vista iniziale:** recupera subito l'orientamento dopo zoom e trascinamento estremi, conservando la partita.
2. **Cambio e riavvio rapidi:** Borgo1 → Piano1 e caricamento di Coste1 funzionano senza ricaricare tutti gli asset; mantenere la rapidità aggiungendo protezione dalla cancellazione accidentale.
3. **Simulazione durante l'inattività:** per oltre tre minuti in primo piano tempo e fuoco avanzano regolarmente; nessun intervento di recupero necessario.
