# Rocca Ventosa — guida per i coding agent

## Fonte di verità

Il lavoro parte **dal repository esistente** e da questi **quattro documenti aggiornati** (da leggere in ordine):

1. `docs/rocca-ventosa/00-LEGGIMI-PRIMA.md` — decisioni e parametri da approvare.
2. `docs/rocca-ventosa/01-SPEC-GIOCO.md` — regole, coordinatore, fasi decisionali e UX.
3. `docs/rocca-ventosa/02-SCENARIO-FACTORY.md` — territorio sintetico e prove headless.
4. `docs/rocca-ventosa/03-PIANO-DI-AZIONE.md` — fasi, output e checkpoint umani.

**Non cercare, recuperare o ricreare documenti di specifica non presenti nel repository.** I documenti qui sopra definiscono **cosa** costruire. Il codice presente serve a capire **come** farlo, non stabilisce i requisiti di gameplay. Se codice e specifiche non coincidono, prevalgono le specifiche; segnala il conflitto durante l'audit.

`docs/rocca-ventosa/TECHNICAL-FINDINGS.md`, se presente, è **solo una raccolta facoltativa di note tecniche**: verificare sempre nel codice che i dettagli siano ancora validi. Non è una specifica funzionale.

## Todo list: `docs/TODO.md`

`docs/TODO.md` è la lista di lavoro condivisa del progetto. **Leggerla all'inizio di ogni sessione e aggiornarla alla fine di ogni iterazione**, nello stesso commit del lavoro:

- spuntare ciò che è stato completato (con data e riferimento a file/commit o report);
- aggiungere i nuovi compiti, le incognite emerse e le decisioni prese dall'utente (con data);
- tenere in cima la fase corrente e il prossimo checkpoint umano.

Non cancellare le voci completate: spostarle in fondo, nella sezione «Fatto».

## Obiettivo

Realizzare un gioco strategico offline per la *Settimana della Protezione Civile*, rivolto principalmente a studenti delle superiori. Il visitatore gestisce le priorità di difesa di un territorio durante un incendio e sperimenta risorse limitate, incertezza e conseguenze delle decisioni.

- **Un solo territorio sintetico e plausibile**, con diversi inneschi e condizioni di vento.
- **Il giocatore decide le priorità geografiche** e impartisce esplicitamente ordini di preallerta o evacuazione per quartiere. Non controlla i singoli mezzi.
- **Un coordinatore operativo deterministico** traduce quelle priorità in postazioni, percorsi e assegnazioni realistiche; adatta la tattica senza modificare le priorità del giocatore.
- **La simulazione determina gli effetti reali** su fuoco, squadre, civili, viabilità e abitazioni. Un'alta priorità non garantisce la riuscita.
- **Ciclo di gioco**: pianificazione iniziale a ×0 → simulazione accelerata → eventuali decisioni critiche a ×1 con timer reale e mondo ancora in movimento → debrief. Il piano proposto è un'anteprima, non un futuro garantito.
- **Durata flessibile**, anche oltre cinque minuti; calibrare ritmo e tempi attraverso i playtest, non cambiando arbitrariamente la fisica.
- **Chiosco presidiato**: avvio, nuova partita e riavvio **manuali**. Nessun reset automatico per inattività. Il countdown delle decisioni rimane parte del gioco.
- **Riutilizzare il più possibile la grafica Bevy** (diorama, asset, effetti); **rifare la UX**. Italiano, mouse, offline; non richiedere audio.

## Regola per il codice esistente

**Conservare ciò che serve, adattare ciò che può servire, scartare ciò che appartiene a un gameplay differente.** Non mantenere componenti soltanto perché esistono già e non intraprendere una riscrittura totale senza una ragione verificata.

Durante l'audit classificare i componenti con una tabella **RIUSA / ADATTA / RIMUOVI / DA VERIFICARE** e indicare brevemente perché:

- **Candidati al riuso**: modello di propagazione, griglie/scenari, ABM, strade e instradamento, popolazione e mezzi, rendering Bevy, asset, effetti, replay e test headless.
- **Da rivalutare e probabilmente sostituire**: flusso di gioco, turni, comandi diretti ai mezzi, interfaccia e vecchi sistemi di scelta degli obiettivi. In particolare, controllare i percorsi `demo::Session` e runner del kiosk prima di scegliere cosa mantenere.
- **Da costruire o estendere**: Scenario Factory Python, obiettivi geografici ordinati, coordinatore, piani attivo/proposto, preallerta realmente distinta dall'evacuazione, finestre critiche ×1, nuova UX e test delle strategie.

Il risultato deve avere **una sola autorità di simulazione e gameplay**, condivisa dal runner headless e dal kiosk. Nessuna fisica, regola di scoring o logica strategica duplicata nell'interfaccia grafica. Preservare le API esistenti solo se aiutano il nuovo disegno; è consentito eliminarle o cambiarle dopo averne verificato gli utilizzatori.

## Metodo di lavoro

**Una fase alla volta, senza avviare automaticamente la successiva.** Seguire `03-PIANO-DI-AZIONE.md`:

0. **Audit, senza implementazioni**: esplorare il repository *così com'è*, eseguire compilazione/test di baseline, identificare formati e comandi disponibili, verificare l'efficacia degli interventi. Consegnare `audit.md` di una pagina con classificazione RIUSA/ADATTA/RIMUOVI/DA VERIFICARE, test e incognite. **Attendere revisione umana**.
1. **Ambiente naturale**: Python genera orografia e combustibili plausibili, con umidità bassa, fissa e uniforme. Simulare incendi headless variando **solo innesco e vento**; presentare mappe e un breve atlante. **Checkpoint umano**.
2. **Insediamenti**: collocare case, quartieri, viabilità e aree sicure *dopo* aver osservato gli incendi. Rieseguire gli incendi, perché strade e abitati cambiano la continuità dei combustibili. **Checkpoint umano**.
3. **Gameplay headless**: priorità, coordinatore minimo, ordini dei civili, piano attivo/proposto, confronti A/B a parità di seed.
4. **Finestre critiche e bilanciamento**: eventi significativi e recuperabili, ×0/×N/×1, countdown, commit e prove che ripianificare può contare.
5. **UX**: conservare il rendering e sostituire il flusso di interazione; mostrare priorità, anteprime, stato delle unità, eventi e debrief. Riavvio manuale.
6. **Playtest**: comprensibilità, conseguenze misurabili, prestazioni, stabilità e durata effettiva, senza limite rigido di cinque minuti.

Per ogni fase consegnare **al massimo una pagina** con: file modificati, prova riproducibile (comando + immagine/tabella), incognite, unica proposta per il passo successivo. Non superare i checkpoint senza conferma.

## Scenario Factory: ordine da rispettare

**Orografia → vegetazione → incendi headless → atlante delle propagazioni → abitati e strade → nuova simulazione → verifica strategica.**

Python gestisce generazione e analisi; il motore Rust esistente esegue incendi e ABM. Usare i formati letti dal repository; non duplicare il propagatore in Python. Generare pochi candidati ispezionabili, con seed e parametri salvati. La selezione finale del territorio è umana. Non ottimizzare per il numero di case colpite, ma per compromessi reali e finestre d'intervento utili.

## Verifiche che non si possono saltare

- **Effetti reali**: invertire le priorità deve talvolta cambiare gli esiti, non solo le icone. Un piano migliore alla crisi deve poter essere utile, senza riuscire sempre.
- **Preallerta ≠ evacuazione**: verificare effetti distinti sui civili. Non rinominare semplicemente un comando esistente.
- **Difesa delle strade condizionale**: introdurla come obiettivo solo se il modello mostra un effetto sulla percorribilità o sulle conseguenze. Chiudere una strada non equivale a difenderla.
- **Anteprima senza effetti collaterali**: durante ×1 le operazioni già attive proseguono; il piano proposto non muove davvero le squadre prima di conferma/scadenza. Rivalidarlo al commit.
- **Stato e misure coerenti**: case su celle non combustibili e pericolo per persone non si valutano necessariamente con la stessa maschera del fuoco; distinguere fatti simulati da proxy di scoring. La grafica e il debrief devono usare gli stessi criteri.
- **Riproducibilità**: seed, stato, ordini e tempi simulati devono dare risultati replicabili; gli effetti accumulati dipendono dal tempo simulato, non dal numero di update. Verificare scenari con inneschi che si stabiliscono realmente.
- **Strade e squadre**: percorsi raggiungibili, ETA plausibili, nessun ricalcolo A* a ogni tick senza motivo; rifugi effettivamente esistenti e accessibili.
- **Rendering**: controllare coordinate, winding delle mesh, draping delle strade, marker visibili oltre la vegetazione e pulizia di effetti persistenti dopo riavvio.
- **Gestione operatore**: nessun timeout di inattività. Avvio/riavvio manuale affidabili e funzionamento offline senza audio.

**Prima azione autorizzata: solo fase 0, audit del repository. Non implementare feature finché l'audit non è stato revisionato.**
