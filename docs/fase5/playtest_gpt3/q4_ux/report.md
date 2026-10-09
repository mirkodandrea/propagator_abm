# Rocca Ventosa: salvo il paese, ma prima sposto il condominio di pannelli

Ho 16 anni, gioco a strategici, gestionali e simulatori: davanti a una mappa voglio capire cosa succede, non fare l’archeologo delle finestre. Partita intera fino a **T+3:00**, il 9 ottobre, 15:58–16:07. Diario dettagliato in `diario.md`. Solo UX e grafica, nessun codice consultato.

L’intro funziona: tre passi, icone comprensibili, **Inizia** giallo che praticamente ti prende il mouse. Gli ordini rispondono con colori pieni, numeri di priorità e percorsi: finalmente un click che lascia una ricevuta.

Poi arrivano le schede dei paesi, dimensione appartamento. Coprono case, etichette e mezzi; passando sopra Le Ghiande la scheda si allunga proprio davanti al nucleo e al fuoco. Zoomare migliora gli alberi, ma non convince Castelvento a smettere di sedersi sul badge A2. La legenda aggiunge un altro palazzo e termina con una voce tagliata.

Il premio va al fumetto che comincia **«nde, con il fuoco…»**. Anche a legenda chiusa. Bellissima modalità indovina il soggetto durante un’emergenza.

La mappa è un diorama gradevole: rilievi, strade sottili, tetti e boschi. Fuoco arancio, bruciato bruno e fumo grigio raccontano bene l’avanzata. Pini e alberi tondi si distinguono; riconoscere tutte le categorie della legenda è meno immediato. Anelli e percorsi verdi/azzurri si perdono un po’ sul terreno verde. I badge dei mezzi dominano le casette e A1/SQ arrivano a sovrapporsi. Le persone individuali non le ho riconosciute: quante famiglie stanno scappando lo capisco dalle barre, non guardando la scena.

Alle crisi il mio occhio fa turismo: paese evidenziato, spiegazione a sinistra, timer e scelte in basso, stato in alto. Il timer l’ho visto; le mie azioni sono arrivate dopo la scadenza per i tempi degli strumenti, quindi niente recensione inventata del tipo «il gioco ignora i click».

Trascinamento, +, − e casa funzionano nelle prove simulate. Rotella senza cambiamento evidente: **inconcludente**, non prova di un bug. Touch non verificato. Ritratti illustrati e mondo low-poly convivono, ma sembrano due direzioni artistiche vicine di banco.

Finale utile: numeroni, confronto e spiegazioni per paese. La legenda bianca «tu» richiede però di collegarla alle barre blu/arancio. Gli scudi della timeline sembrano bottoncini: ne clicco uno, non vedo risposta. Per il chiosco ci siamo, ma vorrei vedere il territorio mentre lo difendo.

---

**Traduzione per gli sviluppatori — gravità decrescente**

1. **Testo troncato.** 16:06:22, simulazione T+2:32, fumetto basso sinistro: frase visibile da «nde…», anche a legenda chiusa. Certezza alta sull’effetto, causa non verificata. Correzione: contenere e mandare a capo tutto il testo nel fumetto, verificando i messaggi lunghi.
2. **Occlusioni.** 16:02:37, scheda Le Ghiande espansa sopra nucleo 1/fronte; 16:01:11 e 16:05:58, Castelvento sopra badge; 16:06:22, A1/SQ sovrapposti. Alta. Separare schede dalla scena o renderle richiudibili; evitare collisioni tra etichette.
3. **Legenda tagliata.** 16:00:13 e 16:05:30, pannello sinistro: ultima voce fuori area, Pian dei Grilli coperto. Alta; scorrimento reale non verificato. Proporre pannello adattivo con scrollbar evidente.
4. **Gerarchia della crisi dispersa.** 16:03:02/16:04:24: quattro zone distanti per stato, paese, spiegazione e scelta. Alta come osservazione, media come impatto. Riunire spiegazione, timer e azioni accanto al paese interessato.
5. **Mappa poco autosufficiente.** Vista larga e zoom, 16:02–16:06: badge sproporzionati alle case, famiglie leggibili soprattutto nelle barre, anelli tenui sul verde; vegetazione non tutta riconoscibile. Media. Aumentare contrasto dei percorsi, ridurre badge e distinguere vegetazione con forme oltre ai colori.
6. **Finale ambiguo.** 16:07: legenda «tu» bianca contro barre colorate; uno scudo cliccato senza risposta visibile. Alta sul test, media sull’ambiguità. Usare «barra piena/contorno» e distinguere marker informativi da pulsanti.
7. **Stile misto.** Ritratti illustrati contro diorama low-poly durante tutta la partita. Giudizio soggettivo, certezza media. Armonizzare palette e semplificazione dei ritratti.

**Tre cose da tenere:** intro e pulsanti evidenti; feedback degli ordini e barre delle famiglie; fuoco leggibile e finale con confronto spiegato.
