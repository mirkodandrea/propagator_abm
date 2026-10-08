# Rocca Ventosa: il fuoco corre, io faccio il turno allo sportello

POV: ho 14 anni, la classe aspetta dietro e il gioco mi accoglie con legenda, spiegone e tre pannelli. Bro, volevo salvare il paese, non adottare un cruscotto.

Primo click sensato dopo circa **22 secondi**: Difendi Le Coste. Poi Evacua… e scopro che devo pure confermare in fondo. Il bottone diventa blu, io penso «fatto», il gioco pensa «bozza». Prima volta perso.

I pannelli di Il Piano e Le Coste si mangiano a vicenda. Quello del Borgo cambia altezza e mi sposta i pulsanti. Seconda volta perso. Anche il mouse deve evacuare.

Il fuoco almeno si vede avanzare. La crisi dell’autobotte fuori servizio mi sveglia: finalmente qualcosa che capisco al volo. Però tra gli ordini passo un sacco di tempo a guardare famiglie che “aspettano di vedere il fuoco”. Ho già detto Evacua: adesso che clicco, la pazienza?

**Due episodi di noia, due di confusione.** Una crisi lasciata scadere, ma lì ci sono di mezzo anche i miei strumenti: non la vendo come colpa certa del gioco.

Quasi **10 minuti di simulazione reale**, oltre 10 dall’apertura. Dietro c’è una classe, davanti sembra che abbia prenotato un tirocinio.

Finale: 13 famiglie colte in casa dal fuoco in meno. Questo sì, arriva. Poi sotto c’è il verbale della riunione. Voglia di «Altro incendio»? Bassa. Lo premo comunque: incendio vicino a Il Piano invece di Le Coste, distanze diverse, stesso muro di testo. Ho guardato solo l’inizio. Il nuovo incendio c’è; la nuova prima impressione, meh.

---

## Traduzione per gli sviluppatori

Prova del 9 ottobre 2026, orologio locale Europe/Rome (UTC+2). Solo gioco visibile, nessun codice o documento consultato. Scheda selezionata e visibile; interazioni sul canvas con gli eventi mouse/pointer richiesti. Osservazioni a intervalli: i tempi comprendono lettura delle immagini e latenza degli strumenti, quindi non sono un benchmark di un adolescente con mouse fisico. Le valutazioni di noia sono del personaggio, non dati su un campione di studenti.

### Cronometro e limiti

- **00:40:02** apertura; **00:40:24** primo click utile, Difendi Le Coste: circa **22 s**, comprensivi di caricamento e osservazione. Il click preliminare al centro del canvas non aveva prodotto una scelta visibile. Non ho isolato la durata del download.
- **00:40:49** Conferma e avvia. **00:41:43** successivo ordine: **54 s senza click**, con fuoco, mezzi e contatori in evoluzione. Primo episodio di noia; non un blocco immobile.
- **00:41:43–00:42 circa**, T+0:18–0:27: Evacua Il Piano selezionato, ma ordine ancora da confermare. Primo episodio di confusione. La crisi T+0:30 risulta poi scaduta nel registro; non ho osservato direttamente tutto il suo intervallo di 25 s e non posso attribuire la mancata risposta solo alla UI.
- **00:43:16–00:44:07**, T+0:41–0:58: tentativi sul Borgo, pannello e pulsanti che cambiano posizione; secondo episodio di confusione. L'intervallo tra screenshot e click può avere aggravato il problema.
- **00:44:52**, T+1:13: osservata Decisione critica per Autobotte 1 fuori servizio, timer 25 s; **00:45:07** premuto Continua con il piano attuale.
- **00:46:05–00:49:25**: **3 min 20 s senza miei click**, dopo la conferma di Il Piano primo e prima di Difendi Borgo. Nelle osservazioni T+1:47, 2:05, 2:18 e 2:33 il fuoco continua ad avanzare, le evacuazioni rallentano/si fermano per gruppi e la squadra poi si ritira. Secondo episodio di noia. Non significa che fosse impossibile agire: al termine ho effettivamente cambiato priorità.
- Finale osservato tra **00:50:25** (T+2:59) e **00:50:41** (fine): durata dall'avvio **9 min 36 s–9 min 52 s**, dall'apertura **10 min 23 s–10 min 39 s**.
- **00:51:24** Altro incendio: verificata solo la pianificazione iniziale, T+0:00, vicino a Il Piano. Nessuna seconda partita avviata.
- **Momenti morti assoluti verificati: nessuno.** Le attese criticate sono tempi con poca azione evidente per me, non assenza totale di animazione. Totale soggettivo: **2 episodi di noia + 2 di confusione**.

### Critiche, in ordine di gravità

1. **Durata e lunghi tratti di sorveglianza poco guidata.** Nella partita in corso, soprattutto T+1:47–2:33, avevo già ordinato tutte le evacuazioni; molti contatori restavano stabili, un mezzo era fuori servizio, uno bloccato, poi la squadra si ritirava. Non individuavo subito la prossima scelta utile. Il successivo cambio al Borgo ha però assegnato un mezzo: l'azione era possibile. **Certezza alta** su durata e stati osservati, **media** sulla causa della noia. **Proposta:** modalità chiosco da 4–5 minuti e segnalazione visiva di una nuova opportunità di scelta; comprimere i tratti senza nuove decisioni mantenendo visibili le conseguenze.

2. **Pannelli che coprono informazioni e spostano i comandi.** A T+0:13 e T+0:27 Il Piano copre titolo/distanza di Le Coste; le altezze cambiano con le righe sulle famiglie. Nel Borgo Evacua compare a diverse quote tra T+0:35, 0:50 e 0:58. **Certezza alta** sull'aspetto; **media** sul contributo ai click falliti, dato il metodo di input. **Proposta:** pannelli in posizioni fisse, righe di stato compatte con altezza riservata, nessuna sovrapposizione; dettagli espandibili. Screenshot 02.

3. **Selezione di un ordine troppo facile da scambiare per esecuzione.** A T+0:27 Evacua Il Piano è azzurro, ma il testo dice ancora preallertati e appare Conferma il nuovo piano distante, in basso. Il finale registra l'evacuazione solo a T+0:41, dopo la mia nuova selezione/conferma. **Certezza alta** sulla distinzione visibile, **media** sull'intuitività per altri utenti. **Proposta:** accanto al comando mostrare «Da confermare», con collegamento visivo alla conferma; poi feedback breve «Ordine inviato». Screenshot 02.

4. **Prima impressione troppo testuale.** All'apertura e dopo Altro incendio sono simultaneamente visibili legenda di dieci voci, spiegazione di pianificazione/preallerta/evacuazione, tre pannelli e coordinatore. Il tempo fermo aiuta, ma il personaggio non legge tutto. **Certezza alta** sul carico visibile; il primo click di 22 s è solo indicativo perché include strumenti/caricamento. **Proposta:** tre istruzioni brevi in sequenza («Scegli chi difendere», «Avvisa o evacua», «Conferma»), con legenda e spiegazioni disponibili su richiesta. Screenshot 01 e 05.

5. **Finale informativo, ma poco invito a un'altra partita.** Il risultato principale è leggibile: 3 famiglie colte in casa contro 16 senza ordini, 72 case colpite contro 73. Sotto segue una cronologia lunga; non vedo una sfida sintetica per il prossimo incendio. Altro incendio cambia luogo iniziale e distanze, ma ripropone la stessa presentazione testuale. **Certezza alta** sui contenuti, **soggettiva** sulla voglia di rigiocare. **Proposta:** una frase sulle conseguenze della scelta decisiva e una sfida concreta per il nuovo scenario; cronologia espandibile. Nessuna valutazione della varietà oltre il secondo inizio. Screenshot 04 e 05.

## Tre cose da tenere

1. **Fuoco, area bruciata e cambio del vento visibili sulla mappa:** permettono di seguire l'evoluzione senza leggere ogni evento.
2. **Avviso grande delle Decisioni critiche e timer:** la perdita dell'autobotte a T+1:13 rende immediata una conseguenza e riporta l'attenzione sulla partita.
3. **Confronto finale con lo stesso incendio senza ordini:** il vantaggio sulle famiglie colte in casa dal fuoco rende comprensibile il valore delle decisioni, anche quando le case salvate sono poche.
