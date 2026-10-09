# Report playtest: Giulia (16 anni, terza superiore)

Ho giocato tre partite complete, dalla schermata iniziale al «Concluso». Una quarta non l'ho fatta per tempo. Ogni partita dura pochi minuti reali, perché il tempo corre a ×40 e ×120 e io mi sono fermata spesso in pausa.

## Il racconto di Giulia

**Partita 1, alla cieca.** Capisco subito che c'è un fuoco vicino a Le Ghiande e che devo scegliere chi difendere. Tre pulsanti in alto («Scegli chi difendere», «Avvisa o fai evacuare», «Conferma e osserva») non mi dicono la differenza tra i primi due. Clicco «Difendi» su Le Ghiande: compaiono due mezzi e linee tratteggiate. Clicco «Evacua» e non vedo nessun cambiamento. Premo «Avvia», il gioco va in pausa e serve «Conferma e riprendi». Solo dopo capisco che la barra di ogni scheda sono le famiglie (verde in salvo, blu in viaggio, giallo si prepara, grigio a casa).

A T+0:26 arriva una decisione: il vento gira e spinge il fuoco su Pian dei Grilli. Ho 16 secondi. Clicco «Evacua» nella barra, ma la scheda non cambia. Il tempo scade e il gioco dice «tempo scaduto: resta il piano attuale». Nella seconda decisione dò l'ordine su Le Ghiande, ma Castelvento non riceve nessun ordine: il finale dice «159 rimaste a casa».

Esito: 9 famiglie colte in casa, 71 case colpite. Pian dei Grilli 55/55 in salvo, Le Ghiande 21/30 con 2 vittime, Castelvento 1/160. Il finale dice «Aspettare di vedere il fuoco è uno degli errori più pericolosi»: lo capisco, ma non so cosa avrei dovuto fare con Castelvento.

**Partita 2, «Riprova».** Evacuo subito Le Ghiande e Pian dei Grilli. Su Castelvento i pulsanti compaiono solo passando il mouse sopra la scheda. Alla decisione a T+0:42 («Castelvento è minacciato») scelgo «Castelvento per primo»; il tasto «Continua» è diventato «Conferma» e si è spostato, quindi il primo click va a vuoto. Dopo la conferma vedo «Squadra A va a Castelvento (priorità 1)».

Esito: 3 famiglie colte in casa, 73 case colpite. Castelvento 124/160 in salvo, Pian dei Grilli 55/55, Le Ghiande 25/30 con 1 vittima. Non capisco perché 36 famiglie di Castelvento siano rimaste a casa.

**Partita 3, «Altro incendio».** Il fuoco parte da Pian dei Grilli con vento da sud a 40 km/h, come dice la volontaria. Ho capito che il punto di partenza è cambiato, ma la freccia del vento in alto è piccola e senza testo. Evacuo Pian dei Grilli e Le Ghiande all'avvio, poi Castelvento a T+1:35. Il primo click su Castelvento finisce su «Preallerta».

Esito: 3 famiglie colte in casa, 24 case colpite. Castelvento 124/160 (36 a casa), Pian dei Grilli 52/55, Le Ghiande 24/30 con 1 vittima.

## Cosa ho imparato tra una partita e l'altra

- Dopo la 1 ho capito che il tempo conta: evacuare presto riduce molto le famiglie in casa. Nella 2 l'ho applicato subito.
- Dopo la 2 ho capito che «Difendi» manda i mezzi ma non salva le famiglie da sola.
- Nella 3 ho trasferito la lezione (evacuo chi è vicino al fuoco), ma non ho capito cosa cambi davvero il vento.
- Resta oscuro: cosa vogliono dire «si preparano», «in viaggio», «senza via», «intrappolate»; a cosa serve la freccia del vento.
- Non ho aperto «Legenda e tempi», «I mezzi» né «Il coordinatore propone»: non so se servono.

---

## Traduzione per gli sviluppatori

Ordine di gravità. Confidenza: alta / media / bassa.

**Difetti di comprensione**

1. **Ordini in pausa senza effetto visibile (gravità alta, confidenza alta).** In pausa, «Evacua» evidenzia il pulsante ma la scheda non cambia finché non si preme «Conferma» (partita 1 e 3). *Correzione:* sulla scheda scrivere «ordine: evacuazione, da confermare» con il numero di famiglie coinvolte.
2. **Preallerta ed evacuazione non si distinguono (gravità alta, confidenza media).** I due pulsanti hanno lo stesso peso; l'effetto («si preparano» contro «partono») appare solo durante la simulazione. *Correzione:* etichetta con l'effetto sotto ogni pulsante.
3. **«Difendi» e «Nessuna priorità di difesa» senza spiegazione (gravità alta, confidenza alta).** Ripetuto su tutte le schede; non capisco cosa faccia «Difendi» fino alla partita 2. *Correzione:* una frase sul pulsante, per esempio «manda i mezzi a proteggere le case».
4. **Gergo nei messaggi (gravità media, confidenza alta).** «senza mezzi: nessuna postazione raggiungibile e sicura ora», «fuori servizio», «senza via», «intrappolate». *Correzione:* frasi semplici, una per evento.
5. **Due nomi per la stessa cosa nel finale (gravità media, confidenza alta).** La colonna «in casa» mostra 0 per Castelvento e Pian dei Grilli, mentre il «Perché» dice «36 rimaste a casa» e «3 rimaste a casa». *Correzione:* un'unica etichetta, per esempio «colte dal fuoco» e «non evacuate».
6. **Vento illeggibile (gravità media, confidenza media).** La freccia in alto a sinistra non dice da dove soffia. *Correzione:* scritta accanto («vento da sud, 40 km/h»).
7. **Decisioni che scadono mentre leggo (gravità alta, confidenza media).** Con 16-22 secondi non riesco a leggere testo e opzioni; alla scadenza il gioco si ferma e l'avviso «tempo scaduto» compare senza che abbia letto la richiesta. *Correzione:* più tempo, o un promemoria in cronologia con l'opzione applicata di default.
8. **Velocità che cambia da sola (gravità media, confidenza media).** Il tempo passa da ×40 a ×120 e torna a ×40 senza un mio click e senza spiegazione.

**Difetti tecnici e di interazione**

9. **Pulsanti che compaiono solo al passaggio del mouse (gravità alta, confidenza alta).** Nella scheda di Castelvento i pulsanti non ci sono finché il mouse non passa sopra (pianificazione, partita 2). Su touch non esistono. *Correzione:* mostrare sempre i pulsanti.
10. **Schede che si spostano durante il click (gravità alta, confidenza media).** Il pulsante «Evacua» si sposta e il click finisce su «Preallerta» (partita 3, T+0:25; partita 2). Anche «Conferma» cambia testo e posizione (Conferma / Conferma e riprendi / Avvia). *Correzione:* posizione fissa dei pulsanti d'ordine.
11. **Possibile errore mio di coordinate (confidenza bassa, da verificare).** Lo screenshot è 1600×906 e il canvas CSS 1455×824 (devicePixelRatio 1.1). Non ho convertito le coordinate in modo sempre coerente, quindi alcuni click a vuoto potrebbero dipendere da me. Per esempio, l'ordine su Castelvento della partita 1 non risulta: non so se il click non è arrivato o se ho sbagliato io.
12. **Primo click su «Inizia» a vuoto (confidenza bassa, da verificare).** Forse caricamento, forse mio click.
13. **Testi che scompaiono in fretta (gravità media, confidenza media).** I fumetti di volontaria e sindaca restano 3-4 secondi; alcuni testi lunghi li ho letti solo a metà.

---

## Le tre cose che funzionano

1. **Barra per scheda con «a casa / si preparano / in viaggio».** Mostra che le famiglie si muovono.
2. **Stima «Evacua: ~2 min di strada, le ultime ~18».** Prima di avviare, sapere quanto ci vuole è utile.
3. **Finale con il contorno «stesso incendio senza ordini» e il «Perché» per paese.** Confronto chiaro e una lezione reale.

Screenshot in questa cartella: p1-01-inizio.png, p1-02-pausa-avvio.jpg, p1-03-meta-partita.jpg, p1-04-finale.jpg, p2-01-decisione.jpg, p2-02-finale.jpg, p3-01-inizio.jpg, p3-02-finale.jpg. Non ho potuto usare filePath (accesso negato), quindi li ho copiati dalla cartella temporanea degli strumenti.
