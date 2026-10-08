# Ho min-maxato Rocca Ventosa: il build «quattro click e ciao» batte il mio esercito

Ho 16 anni, apro un gestionale antincendio e ovviamente cerco il meta. Tre run complete sullo **stesso incendio**, scenario `Coste2_gira`, seme 1: «Riprova» dopo ogni finale, anche il terzo. Browser visibile, solo gameplay; zero codice o documentazione. Barra operatore F2: prima run inizialmente ×20, poi ×100 circa a T+0:42; seconda e terza ×100. Le crisi mostravano ×1 e un conto alla rovescia di circa 15 secondi. Tutte lasciate scadere.

| Run / strategia | Famiglie colte in casa dal fuoco | Case colpite | Evacuate Borgo / Piano / Coste | Mezzi persi |
|---|---:|---:|---|---:|
| AFK: nessun ordine | 16 | 73 | 1/160 · 49/55 · 17/30 | 0 |
| Evacua tutti a T+0; difendi solo Il Piano | 3 | 27 | 124/160 · 55/55 · 25/30 | 0 |
| Solo difesa: Coste > Piano > Borgo; nessun ordine ai cittadini | 15 | 71 | 1/160 · 49/55 · 15/30 | 1 |

Il mio piano «difendo tutto, sono un genio» salva due case rispetto all'AFK e perde un'autobotte. Il piano da quattro click salva **46 case e 13 famiglie** rispetto all'AFK. Skill issue del mio build, numeri alla mano.

Le scelte contano? Sì, parecchio. Esiste il pulsante win universale? Non l'ho dimostrato: tre strategie, un solo incendio, nessuna vittoria perfetta. Ho cambiato insieme evacuazioni e difesa: non posso assegnare tutto il merito a un singolo tasto.

Però il finale sembra un invito allo spam «Evacua»: ho evacuato 124 famiglie al Borgo, dove risultano zero case colpite persino senza ordini, e il riepilogo non mi presenta alcun costo di quella scelta. Il tryhard sente odore di scorciatoia; il bilanciamento globale resta da verificare.

Poi il vero boss: i pannelli. A T+2:26 Il Piano copre parte della scheda di Le Coste. Sto proteggendo cittadini o giocando a cercare il testo sotto il testo?

Anche l'orologio fa il misterioso: «T+3:00 di 3:00», poi «Dopo 3 ore». Scrivere l'unità sarebbe un buff gratuito.

Verdetto: qui vince il piano semplice e anticipato, non la quantità di mezzi che mando al barbecue. Per un chiosco educativo ha senso premiare la prevenzione; rendete visibili anche i compromessi, prima che Reddit dichiari risolto il gioco dopo una sola mappa.

---

## Traduzione per gli sviluppatori — in ordine di gravità

1. **Informazioni operative coperte da altri pannelli.** Nella seconda partita, mappa a T+2:26, la scheda ampliata del Piano si sovrappone a Le Coste e ne nasconde parte dell'intestazione e del testo. Screenshot [05-pannelli-sovrapposti.png](05-pannelli-sovrapposti.jpg). **Certezza alta** per questa vista; non verificato su altri schermi o livelli di zoom. **Correzione:** evitare collisioni fra schede, oppure trasferire i dettagli in un pannello laterale con un riepilogo compatto ancorato ai luoghi. Verificare il layout con tutte le evacuazioni attive.

2. **Il riepilogo rende invisibile il compromesso dell'evacuazione precauzionale.** Nel finale della seconda partita, Il Borgo registra 124/160 evacuate e zero case colpite sia nel piano sia nel confronto senza ordini. I risultati principali mostrano case e famiglie colte in casa, senza un indicatore di costo o disagio dell'evacuazione. Screenshot [03-finale-evacuazione.png](03-finale-evacuazione.jpg). **Certezza alta** su ciò che presenta questa schermata; **media** sul rischio didattico di incoraggiare l'evacuazione indiscriminata; **non verificata** una strategia dominante su altri incendi. Le due modifiche della run sono combinate, quindi il beneficio non isola l'effetto dell'evacuazione. **Correzione:** nel debrief spiegare proporzionalità, incertezza e motivi delle evacuazioni, mostrando gli eventuali costi già simulati. Se assenti, valutare compromessi comprensibili con esperti di Protezione Civile, senza penalizzare arbitrariamente la sicurezza. Confrontare in test separati evacuazione totale, selettiva e sola difesa del Piano.

3. **Unità dell'orologio poco esplicita.** La barra iniziale e in partita mostra «T+0:00 di 3:00»; il finale chiarisce «Dopo 3 ore di incendio». Le crisi, invece, parlano di minuti e secondi di risposta. Screenshot [01-pianificazione.png](01-pianificazione.jpg) e i tre finali. **Certezza alta** sulla notazione osservata; non sostengo che la simulazione temporale sia errata. **Correzione:** scrivere «tempo simulato: 0 h 00 min / 3 h» e distinguere chiaramente il countdown reale della decisione. ×100 è un ausilio operatore usato nel test, non un difetto del gioco.

## Tre cose che funzionano e vanno tenute

1. **Confronto con lo stesso incendio senza ordini**, con risultati per luogo e registro delle decisioni: rende verificabile l'effetto del piano. L'AFK coincide esattamente con il confronto, 16 famiglie e 73 case. Prove: [02-finale-afk.png](02-finale-afk.jpg), [04-finale-difesa.png](04-finale-difesa.jpg).
2. **Cittadini con comportamenti diversi.** Dopo l'ordine alcuni partono, altri aspettano o difendono casa; nella seconda run rimangono tre famiglie colte in casa. L'evacuazione non è una magia istantanea e la differenza fra persone e immobili è concreta.
3. **Anteprima del coordinatore e logistica automatica leggibile.** La pianificazione mostra assegnazioni, tempi e motivi per lasciare mezzi in attesa; in partita ho visto rifornimento, ritirata e perdita di un'autobotte. È adatto a far scegliere priorità agli studenti senza chiedere di guidare ogni mezzo.
