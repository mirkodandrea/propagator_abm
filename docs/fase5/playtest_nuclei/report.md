# Playtest «sei borghi» - Giulia, 16 anni

**Esito: prova interrotta. Non ho completato nessuna partita.** Ho giocato solo l'inizio della prima partita (circa 7 minuti, 23:30-23:37). Non ci sono esiti di famiglie colte in casa né case colpite, né durata reale di una partita. Le partite 2 e 3 non sono state giocate.

## Il racconto di Giulia

**Partita 1, alla cieca.** Apro la pagina e aspetto. Ci mette una decina di secondi, con una mappa sfocata sullo sfondo. Compare la schermata iniziale: una volontaria mi dice che il fuoco è vicino a Fornaci e che ci sono 2 autobotti e 1 squadra per sei borghi. Premo Inizia.

Vedo la mappa e sei schede: Pian dei Grilli, Le Ghiande, Fornaci, Castelvento, Le Terrazze, San Rocco. Ogni scheda ha un numero di famiglie e tre pulsanti. Capisco che devo scegliere dove mandare i mezzi, ma non so cosa cambi tra «Preallerta» ed «Evacua». Il fuoco è a 0,8 km da Fornaci e il vento viene da ovest, quindi scelgo: difendo Fornaci, evacuo Le Ghiande, che è la più vicina al fuoco (0,4 km).

Clicco «Difendi» su Fornaci. Non succede niente: la scheda resta «Non difeso». Riclicco. Niente. La mappa si sposta, quando provo gli eventi simulati, e le schede cambiano posto, ma il pulsante resta senza effetto. Premo «Inizia la partita» e compare un riquadro «Come si gioca», con il pulsante «Ho capito». Lo premo, il riquadro non si chiude. Lo premo di nuovo, provo anche con il click simulato: niente. Ricarico la pagina e la schermata iniziale torna, ma il gioco non risponde ai click. Il browser dice che la scheda non è in primo piano. Apro una nuova scheda e la schermata iniziale si ricarica, ma la scheda continua a risultare nascosta. A quel punto mi fermo, come da istruzioni.

**Cosa ho imparato:** poco. Ho capito che c'è un fuoco, un vento e sei paesi con famiglie. Non ho capito la differenza tra «Preallerta» ed «Evacua», né cosa succede dopo «Avvia».

**Curva di apprendimento:** non c'è stata, perché non ho potuto giocare la seconda partita.

## La mappa con sei borghi

1. **I sei borghi sulla mappa.** Riconosco le schede dei sei paesi, ma non riconosco con certezza le case sul terreno: sono piccole e in gran parte coperte dalle schede. Le scritte «Le Ghiande · nucleo 1..4» sono sul terreno, ma non so cosa significano.
2. **Collegare scheda e paese.** Le schede hanno un nome, quindi posso dire quale paese è quale. Ma sulla mappa non c'è un collegamento visibile tra scheda e paese, tranne che per Le Ghiande, dove la scritta del nucleo sta vicino alla scheda.
3. **Sei quartieri con 3 mezzi.** La scelta resta comprensibile nella parte dei numeri (famiglie, tempo di strada), ma sei schede con tre pulsanti ciascuna sono troppe per una prima lettura. Non ho potuto verificare come si decide con 3 mezzi.
4. **C'è abbastanza da guardare?** Il terreno intorno alle schede c'è, con vegetazione e strade, ma le schede occupano quasi tutta la mappa e il resto sembra vuoto.
5. **Partita più lunga o più confusa?** Non lo posso dire: non ho completato una partita. Il pannello di aiuto appare dopo «Inizia la partita», quindi è tardi.

---

## Traduzione per gli sviluppatori

Ordine di gravità. La certezza è indicata per ogni punto.

1. **[Bloccante, certezza media] Il riquadro «Come si gioca» non si chiude con «Ho capito».** Partita 1, dopo «Inizia la partita», circa 23:35. Schermata: il riquadro in basso a sinistra, con il pulsante «Ho capito». Nessun click, né quello del mouse né quello simulato sul canvas, lo chiude. Ipotesi: la scheda è nascosta e il gioco non elabora gli eventi (vedi punto 2). Da verificare con una scheda in primo piano. Correzione: chiudere il riquadro anche con il tasto Invio o con un click ovunque sul riquadro, e rendere il pulsante grande; in ogni caso, il gioco dovrebbe funzionare anche con la scheda in secondo piano per almeno 30 secondi.
2. **[Bloccante per il test, certezza alta] Il gioco non risponde quando la scheda non è in primo piano.** Il browser riporta «nascosta» anche dopo aver chiuso la scheda precedente e aperta una nuova. Non posso dire se è un problema dell'ambiente di prova o del gioco. Correzione: verificare nel chiosco che la scheda resti visibile; se il gioco dipende dal focus, mostrare un avviso «Riporta il gioco in primo piano».
3. **[Alto, certezza media] I pulsanti Difendi, Preallerta ed Evacua non reagiscono in pianificazione.** Partita 1, schermata «In pausa» prima di «Inizia la partita», con la scheda in primo piano (prima della perdita di focus). Nessun cambiamento sulla scheda dopo il click su «Difendi». Correzione: far vedere subito lo stato scelto (per esempio la scheda che passa a «Difesa: 2 autobotti in arrivo»), oppure disabilitare i pulsanti con una spiegazione chiara.
4. **[Medio, certezza alta] Il testo dice «Premi Avvia», ma il pulsante dice «Inizia la partita».** Partita 1, riquadro «Come si gioca». Correzione: usare la stessa parola nel testo e nel pulsante.
5. **[Medio, certezza alta] Le schede coprono la mappa e le scritte dei nuclei.** Partita 1, schermata di pianificazione. Le schede di Pian dei Grilli e Le Ghiande coprono parte del terreno, e le scritte «Le Ghiande · nucleo 1..4» sono in parte sovrapposte al riquadro di aiuto in basso. Correzione: spostare le scritte dei nuclei sotto le schede o mostrarle solo al passaggio del mouse; ridurre le schede a una riga quando sono molte.
6. **[Basso, certezza bassa] La mappa si sposta da sola quando si invia un click simulato sul canvas.** Partita 1, 23:33. Possibile effetto del mio click sintetico (pointerdown/up trattati come trascinamento), non verificato con un vero click. Correzione: verificare che un click breve non attivi il trascinamento della camera.
7. **[Comprensione, certezza alta] Preallerta ed Evacua non si capiscono senza spiegazione.** Partita 1, schermata di pianificazione. Le schede dicono «Preallerta: si tengono pronti · Evacua: partono», ma la differenza non è visibile prima di premere «Inizia». Correzione: mostrare la differenza nella scheda stessa, con una frase breve per ciascun pulsante.

## Le 3 cose che funzionano

1. **Le schede dei borghi dicono cosa succede se non fai niente.** «Evacua: ~2 min di strada, le ultime ~36 min» è chiaro e aiuta a scegliere. Tenerlo.
2. **La schermata iniziale con la voce della volontaria** spiega il problema in una frase (un fuoco, sei borghi, due autobotti e una squadra). Tenerlo.
3. **Il riquadro «Come si gioca» elenca i quattro punti in modo leggibile**, con icone accanto a ogni comando. Va sistemato nel testo, ma la forma è buona.

## Screenshot

- p1-01-schermata-iniziale.jpg
- p1-02-pianificazione.jpg
- p1-03-mappa-spostata.jpg
- p1-04-come-si-gioca.jpg
- p1-05-overlay-non-si-chiude.jpg
- p1-06-ricaricata-schermata-iniziale.jpg

Diario: diario.md
