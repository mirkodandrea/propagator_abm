# Rocca Ventosa: ho difeso il territorio, ora difendiamo la mappa dai pannelli

Ho 16 anni, gioco a strategici e gestionali: tre paesi, poche squadre, incendio. Perfetto, il mio nuovo problema dopo il compito di matematica. Test solo visivo, partita completa il 9 ottobre, 13:44–13:53. Avvia alle 13:46; finale verso le 13:52, con passaggio automatico osservato da ×20 a ×60.

L'inizio funziona: italiano comprensibile, tre azioni illustrate, bottone giallo che praticamente alza la mano. Difendi, Preallerta, Evacua hanno icone e colori riconoscibili. Alle 13:45 scelgo le Ghiande: bordo giallo, priorità numerata, pulsante blu e mezzi assegnati. Finalmente dei click che rispondono senza chiedermi di consultare un oracolo.

Poi apro la legenda. Utile, spiega perfino che i chilometri non sono tempi d'arrivo. Peccato che si parcheggi sopra Pian dei Grilli. Il manuale è accurato; il paese sotto può attendere.

La mappa ha rilievi, boschi fitti, arbusti e strade. Da vicino distinguo alberi a chioma e conifere; case geometriche e vegetazione mantengono uno stile coerente. Arancione acceso, bruciato scuro e fumo grigio fanno capire dove avanza l'incendio. Però capire chi sta facendo cosa è un altro sport: leggo A1/A2, anelli e percorsi, ma persone e mezzi 3D sono troppo piccoli o coperti per seguire bene l'azione. Non invento giudizi sulle loro proporzioni precise.

Alle 13:50 ingrandisco: Pian dei Grilli si mette SOPRA il titolo delle Ghiande. Simulatore di emergenze con emergenza condominiale tra schede. I pannelli traslucidi lasciano inoltre passare alberi, strade e targhette dietro il testo piccolo: bello l'effetto vetro, meno bello cercare gli arrivi stimati.

Trascinamento, +, − e casa provati: vista mobile e reset utilissimo. Rotella simulata provata separatamente senza cambiamento visibile; rotella fisica e touch non verificati. Input tramite eventi pointer/mouse sul canvas: non attribuisco al gioco eventuali anomalie da simulazione.

La decisione critica richiama bene l'occhio con arancione, fumetto e barra inferiore. Ho visto il countdown a 2, non la durata iniziale. Conferme della sindaca e perdita dell'autobotte sono chiare nei fumetti. Il finale ha numeroni e confronto senza ordini: ottima idea. Le barre verdi «evacuate», però, hanno dimenticato i numeri. Dopo una partita di contatori, il diploma finale è un righello.

---

## Traduzione per gli sviluppatori — gravità decrescente

1. **Schede sovrapposte.** 13:50:06, T+1:13, zoom con cinque +: Grilli copre titolo/parte alta delle Ghiande sul bordo sinistro (04). Certezza alta. Evitare collisioni; usare schede compatte o un elenco fisso, con collegamenti ai paesi.
2. **Mappa operativa coperta.** Pianificazione 13:44 e partita: schede sopra abitati; a 13:50 il mezzo coinvolto è quasi nascosto. Alta per occlusione, media per leggibilità delle persone. Ridurre pannelli, dare una vista mappa libera e simboli operativi leggibili allo zoom generale.
3. **Legenda sopra una scheda.** 13:45:18, pianificazione, alto sinistra (02): nasconde Grilli e comandi. Alta. Riservarle spazio o ricomporre le schede all'apertura.
4. **Contrasto del testo secondario.** Schede e coordinatore, 13:46 e 13:50: terreno e targhette visibili dietro testi piccoli verdi/blu/grigi. Alta sull'interferenza, media sulla difficoltà soggettiva. Fondo più opaco e testo secondario più grande/chiaro.
5. **Finale senza valori delle evacuazioni.** T+3:00, colonna destra «evacuate» (05): barre senza conteggi o percentuali. Alta. Mostrare X/Y accanto a ogni barra.

**Tre cose da tenere:** pulsanti grandi con icone e feedback cromatico; fuoco/bruciato e vegetazione distinguibili nello stile geometrico; finale con confronto esplicito rispetto all'incendio senza ordini.
