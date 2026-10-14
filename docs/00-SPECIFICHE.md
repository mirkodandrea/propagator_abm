# 00 — Specifiche del nuovo progetto (2026-10-14)

Il documento raccoglie le specifiche date dall'utente all'avvio del nuovo progetto e le risposte alle domande che sono seguite. Le regole in vigore, dettagliate, sono in `01-GIOCO.md`. Se i due documenti divergono, prevale la decisione più recente dell'utente, che va scritta con la data.

## Specifiche dell'utente (testo originale)

> Stiamo partendo con un progetto nuovo quindi dobbiamo riscrivere claude.md e il readme e documenti in docs. Usiamo il codice esistente come riferimento per il modello fisico, il modello di comportamento degli agenti di spegnimento, la grafica e alcune parti dell'interfaccia grafica.
>
> - la partita sarà divisa in due parti:
>   1. breve tutorial: parte il gioco guidato da istruzioni passo passo e scriptato, in cui l'incendio viene spento e le case evacuate. (incendio piccolo ma dinamiche realistiche)
>   2. seconda parte giocata: il controllo passa al giocatore. Nuovo giorno, nuovo incendio.
>
> Il nuovo gioco è una sorta di starcraft con in più la gestione dell'evacuazione e l'incendio simulato. In generale, renderlo meno "verbose", elimina tutta la parte di comunicazione e la riscriviamo a posteriori.
>
> - no turn based, tempo "reale" con possibilità di pausa temporizzata per fare decisioni. Quando c'è la pausa il fuoco va a 60x, quando non c'è la pausa va a 300x. Tutte le cose possono essere fatte solo in pausa, ovvero decidere per evacuazione e spostamento di risorse.
> - la pausa corrisponde a chiedere all'esperto scientifico una valutazione della previsione del comportamento potenziale, espressa come simulazione per le prossime tot ore usando isocrone, unita ad info di previsione meteo e umidità del combustibile. Ho un tempo di "ricarica" per richiederla nuovamente.
> - evacua ha l'hard limit sul numero di persone evacuate (rate) e ci mostra il tempo necessario ed il percorso. Si può evacuare solo un abitato alla volta -- rendere evidenti i percorsi di evacuazione e le strade chiuse dal fuoco. C'è la possibilità che una evacuazione si debba interrompere perché ad esempio il fuoco interrompe la strada di evacuazione. A quel punto o c'è il re-routing oppure si interrompe, e la gente va in self-defense.
> - quando metti in pausa puoi decidere se evacuare o spostare le risorse per l'attacco al fuoco.
> - ci sono una caserma dei pompieri e un presidio dei volontari, da cui puoi spostare le risorse
> - lo spostamento di risorse ha un tempo
> - muovi le squadre, ognuna con autobotte e tot pompieri. Queste sono rappresentate da icone che sposti, e quando sposti ti dice il tempo di spostamento e il percorso, anche in base alla possibilità che alcune strade siano bloccate. Poi devi confermare.
> - si possono spostare su una griglia esagonale, o simili, per limitare gli spostamenti.
> - a questo punto sono slegate le possibilità di evacuare e di attaccare il fuoco
> - rimuovere modellazione ibm delle persone, utilizzare dei modelli semplificati per l'evacuazione.

## Risposte e decisioni (2026-10-14)

| Tema | Decisione |
|---|---|
| Dove | Si riscrive il crate `rocca` dentro questo repository, sul branch `nuovo-gioco`. |
| Pausa | 30 s reali (30 min simulati a ×60); ricarica di 20 min simulati. |
| Tutorial | Il giocatore esegue i passi indicati e il gioco accetta solo l'azione richiesta. Piccolo incendio sulla stessa isola. |
| Seconda parte | Per ora un solo incendio (il caso `medium`). I livelli verranno dopo. |
| Squadre | Ogni squadra è un'autobotte con 3 pompieri o volontari. 3 squadre alla caserma dei vigili del fuoco, nel borgo più grande (Castelvento); 2 squadre di volontari in un borgo minore (San Rocco). |
| Esagoni | 500 m come dimensione di riferimento; si possono scegliere gli esagoni adiacenti alle strade. |
| Attacco | Le squadre attaccano il fuoco come possono: pompieri e volontari tagliano la vegetazione verso il fuoco, l'autobotte aumenta l'umidità dove riesce a raggiungere. |
| Apertura | L'incendio parte già sviluppato: segnalazione in ritardo di 30 min (la tabella è in `02-TERRITORIO.md`). |
| Giudizio | Si tengono stelle e classifica. Contano, nell'ordine, le persone salvate, poi le case, poi gli ettari bruciati. |
| Mario | Va ricollegato per ultimo. |
| Comunicazione | Personaggi, messaggi e debriefing si riscrivono a posteriori. |
