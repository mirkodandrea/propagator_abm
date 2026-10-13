# 01 — Il gioco: Isola di Roccavento

Regole **in vigore**. Prevalgono sul codice: se non coincidono, segnalarlo. Ogni cambio di regola è una decisione dell'utente, da scrivere qui con la data (lo storico è nella storia git).

## In una frase

Un gioco strategico da chiosco per la *Settimana della Protezione Civile*, per studenti delle superiori. Un incendio minaccia i borghi di un'isola. Il giocatore decide **chi difendere per primo** e **chi avvisare**: preallerta ed evacuazione, quest'ultima solo con l'autorizzazione della sindaca. Un coordinatore automatico muove i mezzi. La simulazione decide che cosa succede davvero: un buon piano aiuta, ma non garantisce nulla.

**Obiettivo educativo:** incertezza, tempi, risorse limitate, adattarsi. Non è un addestramento e non insegna procedure operative.

## 1. Chi decide cosa

| Attore | Ruolo |
|---|---|
| **Giocatore** | Ordina i borghi da **difendere** (priorità 1, 2, 3…). Dà **preallerta** ed **evacuazione** per borgo. Non comanda i singoli mezzi. |
| **Sindaca** | Firma l'ordinanza: **autorizza l'evacuazione** di un borgo quando il fronte potrebbe arrivare alle sue case entro 20 min simulati (`Mayor::lead_s`), o quando il fuoco è già lì. Un borgo alla volta. L'autorizzazione vale fino alla fine. Non sceglie al posto del giocatore. |
| **Coordinatore** (`rocca::coordinator`) | Traduce le priorità in postazioni raggiungibili per 2 autobotti (caserma di Castelvento) e 1 squadra AIB (dal traghetto, al porto). Si adatta alla situazione, ma non cambia mai le priorità. La sicurezza delle squadre viene prima di tutto. |
| **Pattuglie** (`rocca::messengers`) | 4 pattuglie portano gli avvisi porta a porta, 1 famiglia al minuto ciascuna. Chi ha l'allerta sul telefono la riceve subito. |
| **Simulazione** | Fuoco (PROPAGATOR), famiglie, traffico, squadre, danni. È l'unica fonte dei fatti. |

## 2. Gli ordini

- **Difesa:** borghi ordinati per priorità. Una priorità alta non garantisce la protezione. La difesa riduce l'esposizione reale delle case, non è un punteggio.
- **Preallerta:** sempre disponibile. Le famiglie si preparano a casa e qualcuna parte da sé. Un'evacuazione successiva le raggiunge prima. **Non** è un'evacuazione con un altro nome.
- **Evacua:** solo nei borghi autorizzati. Prima mostra sulla mappa le **vie di fuga** verso l'area sicura (`Game::evacuation_routes`), poi chiede **conferma** a ×1 per al massimo 20 s reali. Allo scadere l'ordine non parte.
- Gli ordini alle famiglie **non si ritirano**.
- **Uscita organizzata:** al massimo **6 famiglie al minuto** da tutta l'isola (`exit_per_min`). Chi ha l'ordine ed è pronto aspetta il turno. Chi vede il fuoco o lo ha alla porta parte comunque.
- Strade da difendere: **non** sono un obiettivo. Lo diventano solo se una prova headless mostra un effetto sulla percorribilità. Chiudere una strada non equivale a difenderla.

## 3. Il tempo

1. **Apertura:** il fuoco aspetta (×0) finché il giocatore sceglie il livello. Poi c'è la pianificazione a ×1, che si chiude con **«Avvia»**.
2. **Esecuzione:** il tempo scorre veloce, a velocità scelta dal gioco: ×80, ×240 nei tratti senza eventi (`RUN_SPEED`, `QUIET_BOOST`). Il giocatore **cambia il piano mentre il tempo scorre** e il cambio vale subito. **Non ci sono pause né un pulsante «Decidi».**
3. **Momenti ×1 automatici**, con conto alla rovescia di 45 s reali, il titolo che dice perché e il personaggio che spiega. Si aprono solo per:
   - **autorizzazione della sindaca** (`Kind::Autorizzazione`);
   - **cambio di vento**, annunciato 20 min prima dal meteo o appena avvenuto (`Previsione`, `Vento`);
   - **fuoco vicino**: il fronte può arrivare entro ~30 min alle case di un borgo senza preallerta né evacuazione (`Vicino`).

   Nel momento ×1 il piano attivo continua e quello proposto non muove nulla. Si applica con «Conferma»; allo scadere si applica il piano proposto, se è cambiato. Le altre situazioni (borgo senza mezzi, mezzo perso, strada tagliata) arrivano come **notizie**.
4. **Fine:** a fine incendio (180 min simulati) compare il finale.

Tutto è misurato in **tempo simulato**: la velocità cambia solo la durata reale. La durata della partita non ha un limite fisso e si valuta nei playtest.

## 4. Previsione del fuoco

Dall'annuncio del cambio di vento fino a 30 min dopo il giro, la mappa mostra dove il fuoco può arrivare **tra 1 ora e tra 3 ore**. Sono isocrone al 50 % di 16 simulazioni dal fuoco attuale, con il vento attuale e quello annunciato perturbati, **senza interventi** delle squadre (scritto nella legenda). Usa solo lo stato presente e il bollettino, mai il futuro della partita. Codice: `fire::ensemble`, `rocca::forecast`, `game::forecast_view`.

## 5. Giudizio e finale

- **Quattro stelle da 1 a 5** (`rocca::score`), calcolate da fatti simulati rispetto allo stesso incendio senza ordini:
  - **Persone:** famiglie risparmiate;
  - **Case:** case non colpite;
  - **Tempismo:** anticipo degli avvisi sull'arrivo del fuoco;
  - **Misura:** famiglie mandate in strada da borghi dove il fuoco non è arrivato alle case.
- **Finale semplice:** stelle grandi, posto nella classifica del giorno, al massimo tre righe di debriefing (la frase della sindaca, il «perché» più utile, l'ordine più discutibile). Pulsanti «Riprova» e «Schermata iniziale».
- **Classifica dei migliori del giorno**, per livello (`score::Board`): somma delle stelle; a pari merito vince chi ha più stelle Persone, poi chi ha giocato prima. Compare a lato della schermata iniziale e nel finale. L'operatore la azzera.
- Grafica e debrief usano **gli stessi criteri** del punteggio. Le case colpite si chiamano «colpite», non «distrutte», se il dato è un proxy.
- Lezione accettata (2026-10-13): «evacua appena autorizzato» rende spesso di più, perché l'ordinanza si esegue. Il costo resta visibile in Misura e nel debrief. Il dilemma vero è **chi preallertare, chi difendere e quando**.

## 6. Interfaccia

- La mappa occupa quasi tutto lo schermo. Il rendering 3D del diorama è conservato.
- **Barra laterale:** stato (livello, vento, ora), messaggio ×1 o conferma dell'evacuazione, **piano a tre colonne trascinabili** (Difesa ordinata | Preallerta | Evacuazione, più «Senza ordini»), mezzi, ultimi aggiornamenti, «Abbandona».
- **Sulla mappa:** schede dei borghi (cliccabili, con i tre pulsanti Difendi / Preallerta / Evacua), etichette dei mezzi, vie di fuga, previsione, freccia del vento, legenda in un angolo.
- **Personaggi:** sindaca, vigili del fuoco, meteo, volontaria. Parlano con il nome del giocatore.
- **Apertura:** nome del giocatore, tre livelli (Facile, Medio, Esperto) e classifica di oggi. Il tutorial si salta con «Salta» o con Esc.
- **Controlli:** mouse o trackpad. Rotella o scroll a due dita per lo zoom, click e trascina per spostare, tasto destro (o Shift) e trascina per ruotare. Home riporta alla vista iniziale. Funziona senza audio, offline, in italiano.
- **Operatore** (F2 o «Menu» in alto a sinistra): nuova partita, scelta del caso, livello grafico (Alta / Media / Bassa), «Ferma il mondo», «Vista iniziale», «Azzera classifica».
- **Chiosco presidiato:** avvio, nuova partita e riavvio sono **manuali**. Nessun reset per inattività.
- **Mario** è un easter egg (tasto M o Start dal titolo, `docs/MARIO.md`): non fa parte del gioco.

## 7. Fuori dal gioco

Niente controllo diretto dei mezzi, disegno di poligoni, menù da gestionale o lunghe spiegazioni. Nessun punteggio di «risorse di comando» e nessun sistema che decida al posto del giocatore. Il territorio è dichiaratamente immaginario: nella realtà si seguono le indicazioni delle autorità.
