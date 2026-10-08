# [r/italy] Rocca Ventosa: il vero incendio sono le informazioni a schermo

Ho 16 anni e alla Settimana della Protezione Civile ho provato Rocca Ventosa. Una partita intera. Volevo finire in fretta, sono diventato il custode di uno screensaver con responsabilità comunali.

Scegli chi difendere, ordini le evacuazioni, poi guardi il bosco cuocere. In alto «tempo accelerato ×20». Ok bro, ma quanto manca? Il cronometro sale, la mia voglia di fissare gli alberi scende. Nessuna durata residua nella vista di gioco: anche il finale è un evento a sorpresa.

Arriva la «Decisione critica»: 25 secondi per leggere il papiro sopra, cercare i pulsantini nei tre paesi e confermare sotto. Praticamente una verifica di comprensione del testo con il barbecue acceso. Per portare Il Borgo dal terzo al primo posto clicchi due volte «più importante»: pure l'emergenza deve fare le scale.

Poi il Piano mi dice fuoco a 0,0 km. Il coordinatore sotto parla ancora di 0,7 km e arrivo in 3 minuti. Il fuoco è in diretta, il briefing arriva con Poste Italiane.

Evacuo Il Borgo: 124 famiglie in salvo, 36 ancora a casa per un bel pezzo. Il pulsante è già selezionato, il pannello non mi dice perché restino lì. Devo coordinarle o indovinare la loro lore?

La Squadra A annuncia «si ritira: troppo pericoloso». Al finale: Squadra A persa, raggiunta dal fuoco. Scusate, mi sono perso l'episodio fra «andiamo via» e «non siamo andati via». Il riepilogo mi dà il risultato, ma il passaggio me lo devo scrivere io.

E i dati del fuoco? Testo rossiccio sopra pannelli trasparenti con il bosco dietro. Sto cercando la distanza dell'incendio o facendo il test della vista per il patentino?

Alla fine ho evitato 15 case colpite e 13 famiglie colte in casa rispetto al confronto senza ordini. Bene. Ora evacuate anche le informazioni vecchie dal pannello, grazie.

---

## Traduzione per gli sviluppatori

Osservazioni da una sola partita, giocata con il mouse in Chrome, scheda visibile. Nessun accesso a codice o documentazione. Le priorità iniziali erano Le Coste → Il Piano → Il Borgo; evacuazione dei primi due e preallerta del Borgo. A T+0:30 ho evacuato Il Borgo e portato la sua priorità al primo posto; a T+1:00 ho messo Il Piano al primo posto. Finale a T+3:00. Ordine seguente per gravità del problema osservato; le cause interne non sono state verificate.

1. **«Il briefing arriva con Poste Italiane» / «evacuate le informazioni vecchie».** Nella vista in corso a T+2:07, Il Piano indicava fuoco a 0,0 km; il pannello «Il coordinatore» riportava ancora 0,7 km e arrivo in 3 minuti. Anche Il Borgo aveva distanze diverse nei due pannelli. Il testo del coordinatore sembra conservare la valutazione del piano confermato, ma non mostra un orario che lo renda evidente. **Correzione:** aggiornare le informazioni operative oppure etichettarle esplicitamente «Valutazione alla conferma, T+1:00», separando storico e stato corrente. Prova: [02-stato-e-anteprima.png](roast_02-stato-e-anteprima.jpg).

2. **«Mi sono perso l'episodio» della Squadra A.** Fra T+2:28 e T+2:39 la sua etichetta mostrava il ritiro per pericolo; il finale la elencava fra i mezzi persi. Il riepilogo delle decisioni non spiega la sequenza che porta dal ritiro alla perdita. Non è prova di un errore nella simulazione: è un problema di spiegazione dell'esito. **Correzione:** registrare e mostrare gli eventi di ritiro e perdita con orari, posizione e motivo; nel debrief collegare la perdita al percorso o alla situazione osservata. Prova del risultato: [03-finale.png](roast_03-finale.jpg).

3. **«Indovinare la loro lore».** Da circa T+1:11 fino al finale, Il Borgo mostrava 124 famiglie in salvo, zero in strada e 36 ancora a casa, nonostante l'evacuazione ordinata a T+0:30. Il suo pannello non spiegava il motivo; il finale conferma 124 evacuate su 160. Non deduco che siano bloccate per un bug, né che siano state colpite: il finale indica zero famiglie colte in casa al Borgo. **Correzione:** rendere visibili le ragioni del mancato completamento dell'evacuazione e le eventuali azioni disponibili; se alcune famiglie non partiranno, esplicitarlo. Prove: [02-stato-e-anteprima.png](roast_02-stato-e-anteprima.jpg), [03-finale.png](roast_03-finale.jpg).

4. **«Verifica di comprensione» / «l'emergenza deve fare le scale».** Alla crisi di T+0:30 il timer partiva da 25 secondi. L'avviso era in alto, i controlli nei pannelli dei paesi e la conferma in basso. Per promuovere Il Borgo dal terzo al primo posto ho premuto due volte «più importante». La crisi successiva, relativa al vento a T+0:46, è scaduta senza una mia modifica. **Correzione:** affiancare all'avviso azioni immediate con anteprima delle conseguenze; consentire la selezione diretta della posizione in priorità e lasciare un riepilogo visibile quando una crisi scade. Il limite temporale può restare parte della sfida. La disposizione dei controlli è visibile anche in [01-pianificazione.png](roast_01-pianificazione.jpg); non ho salvato uno screenshot della crisi.

5. **«Test della vista».** Nella pianificazione e nella vista in corso, le distanze del fuoco erano rosse/arancioni su pannelli scuri semitrasparenti, con il terreno visibile dietro. La lettura risultava poco netta; non ho misurato il contrasto. **Correzione:** usare sfondi più opachi e testo più contrastato, mantenendo il colore come indicatore aggiuntivo del rischio. Prove: [01-pianificazione.png](roast_01-pianificazione.jpg), [02-stato-e-anteprima.png](roast_02-stato-e-anteprima.jpg).

6. **«Screensaver con responsabilità comunali» / «quanto manca?».** Dopo le modifiche ai piani, ho trascorso lunghi intervalli osservando la stessa vista con aggiornamenti del fuoco e dei contatori. La barra mostrava tempo trascorso e ×20, ma non il termine della partita; il finale ha poi dichiarato «Dopo 3 ore di incendio». **Correzione:** indicare fin dall'inizio l'orizzonte delle tre ore e il tempo residuo, e rendere più evidenti i cambiamenti che richiedono attenzione durante l'attesa. Prove: [02-stato-e-anteprima.png](roast_02-stato-e-anteprima.jpg), [03-finale.png](roast_03-finale.jpg).
