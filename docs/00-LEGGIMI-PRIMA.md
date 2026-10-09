# Rocca Ventosa — v0.4 | Da leggere prima di avviare gli agenti

**8 ottobre 2026 · Stato: proposta da approvare · Tempo di lettura: ~2 minuti**

## In una frase

Un gioco strategico da stand (durata flessibile, anche oltre 5 minuti) in cui il visitatore stabilisce **quali luoghi proteggere e con quali priorità**; un coordinatore automatico organizza le squadre, mentre incendio e popolazione evolvono nella simulazione. Quando emerge un dilemma, il gioco rallenta e il visitatore può modificare il piano.

## Decisioni già concordate

- **Pubblico:** studenti delle superiori; chiosco **presidiato dall'autore**, offline, italiano, mouse, nessun audio necessario. **Nessun reset automatico per inattività**: nuova partita e riavvio sono manuali.
- **Obiettivo educativo:** sperimentare incertezza, tempi, risorse limitate e necessità di adattarsi; non insegnare procedure operative.
- **Un solo territorio sintetico**, plausibile, riutilizzato con diversi inneschi e venti. Non generiamo città nuove a ogni partita.
- **Rendering:** conservare il più possibile quello esistente (obiettivo indicativo ~90%); **UX da rifare**.
- **Comandi del giocatore:** ordinare la priorità di difesa di abitati e, se validato, strade. **Preallerta ed evacuazione** sono due decisioni esplicite, separate dalla difesa.
- **Coordinatore:** sceglie settori vulnerabili, postazioni, unità e percorsi; si adatta tatticamente ma non cambia le priorità del giocatore.
- **Tempo:** pianificazione iniziale a mondo fermo; simulazione accelerata; finestre automatiche per crisi significative, durante le quali il mondo continua a **×1** con un breve timer reale. **Nessun limite rigido di 5 minuti alla partita**: durata e ritmo vengono valutati nei playtest.
- **Anteprima:** mostra **cosa il coordinatore intende fare** (percorsi, tempi, coperture possibili), mai l'esito futuro garantito.

## Scenario Factory: sequenza di lavoro

**Ordine definitivo di lavoro:**

1. Generare **solo orografia e vegetazione** plausibili con Python.
2. Provare incendi headless, variando **inneschi e vento**. Umidità dei combustibili **bassa, fissa e uniforme**.
3. Esaminare le mappe di propagazione, poi **collocare abitati e strade** nei luoghi adatti a produrre decisioni interessanti, senza perdere plausibilità.
4. Ripetere gli incendi sul mondo costruito: case e strade cambiano i combustibili.
5. Provare priorità, evacuazioni e coordinatore; ritoccare il territorio finché emergono compromessi e finestre di intervento reali.

**Non** serve un grande ottimizzatore procedurale. Bastano pochi candidati, report semplici e scelta umana.

## Impostazioni iniziali proposte (non promesse scientifiche)

| Scelta | Prima prova |
|---|---|
| Struttura del paese | Borgo compatto + case sparse sul versante; possibile terza zona di fondovalle |
| Obiettivi | Priorità **ordinate** (1°, 2°, 3°), non tutti «alta» |
| Risorse iniziali | 2 autobotti + 1 squadra AIB |
| Pianificazione iniziale | ~40 secondi, simulazione ×0 |
| Simulazione ordinaria | ×40, ×120 nei tratti quieti (iterazione 5b, `docs/fase5/durata.md`) |
| Finestra critica | ~25 secondi reali, simulazione ×1; 0–2 finestre tipiche |
| Strada da difendere | Inclusa **solo quando un test dimostra effetti reali** sulla percorribilità |
| Durata della partita | Non fissata a priori: può superare 5 minuti; verificare ritmo e attenzione durante i playtest |
| Gestione del chiosco | Presidiato, riavvio manuale; **nessun reset per inattività** |

Questi valori sono **parametri di playtest**, non obiettivi rigidi di durata. Il timer di una **finestra decisionale** rimane una meccanica di gioco e non va confuso con un reset automatico. Non modificare la fisica del fuoco per raggiungere una durata prefissata.

## Come usare i documenti

1. Leggere questa pagina e segnalare soltanto eventuali scelte sbagliate.
2. Consultare [01-SPEC-GIOCO.md](01-SPEC-GIOCO.md) per le regole; [02-SCENARIO-FACTORY.md](02-SCENARIO-FACTORY.md) per il territorio; [03-PIANO-DI-AZIONE.md](03-PIANO-DI-AZIONE.md) per incaricare gli agenti.
3. **Non avviare tutti gli agenti insieme:** partire dall'audit tecnico (fase 0). Ogni fase consegna prove e un breve resoconto prima della successiva.

**Fonti di lavoro:** i quattro documenti aggiornati e il repository esistente. Non occorrono altri documenti. Il codice va analizzato per decidere cosa **riusare, adattare o scartare**; i comportamenti attuali non sono vincoli di gameplay.

### Cosa basta approvare ora

L'unica approvazione necessaria **prima del primo agente** riguarda la direzione descritta sopra. La forma esatta della mappa, le costanti di tempo e le postazioni delle squadre verranno decise con le prove. Sono previsti solo **due checkpoint visuali rapidi** sul territorio: dopo l'atlante degli incendi naturali e dopo la proposta di abitati/strade.
