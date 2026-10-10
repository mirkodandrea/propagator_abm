# Report playtest 6 - Rocca Ventosa (Giulia, 16 anni)

LIMITI DEL TEST: ogni screenshot/azione costa 5-30 s reali, e il gioco corre a ~1,4 min simulati al secondo; ho perso la finestra di una decisione critica (P3). Le pause sono state usate con ritardo. Il debrief non e' scorrevole nella mia finestra (824 px css di altezza), quindi non ho potuto usare "rigioca" ne' cambiare difficolta dal finale: tra le partite ho ricaricato la pagina. Partite complete: 3 (+ avvio della 4a).

## Racconto di Giulia
**Partita 1 - Facile (vento da est, fuoco a 0,8 km da San Rocco).** Schermata iniziale: capisco "difendi / avvisa / il tempo scorre". Scelgo Facile perche e' la prima. Tocco i borghi: i bottoni sono grigi e non fanno niente finche non premo "Inizia la partita"; poi mi esce "Come si gioca", lo scorro e premo "Ho capito". Faccio Difendi + Evacua su San Rocco: i 3 mezzi vanno tutti li. Premo Avvia, il tempo vola, non faccio in tempo a leggere i messaggi e non ho dato ordini agli altri borghi. Fine: **742/1000**, famiglie 8 salvate su 11 colte in casa... case 59 salvate su 76. Durata reale ~5 min.
**Partita 2 - Facile, stesso incendio.** Questa volta evacuo San Rocco e Castelvento subito (mi era rimasto il "130 non sono partite" di Castelvento nel "Perche"), poi in pausa a 15:10 evacuo Le Terrazze e dopo il richiamo della volontaria ("A Fornaci 90 famiglie sono ancora in casa, non hanno ricevuto ordini") anche Fornaci. **869/1000**, famiglie 10 su 11, case 59/76 (identico a P1: i mezzi su San Rocco non hanno cambiato il numero di case). ~5 min.
**Partita 3 - Medio (vento da ovest, poi gira a nord-ovest).** Capisco che e' cambiato il vento (freccia) e il borgo piu minacciato (Fornaci, Le Ghiande). Difendo+evacuo Fornaci, evacuo Le Ghiande, preallerta a Castelvento. Alle 14:45 ci fu una CRISI (vento girato verso Castelvento) ma l'ho vista solo nel registro dopo, "tempo scaduto: resta il piano attuale" (limite strumento). Terrazze e San Rocco senza ordini: **360/1000**, 6 vittime a Le Terrazze. ~5 min.

## Cosa ho imparato tra una partita e l'altra
- Dal "Perche" del finale ho preso: "senza ordini nessuno parte" e "gli ordini vanno dati prima"; in P2 ho evacuato presto e il punteggio e' salito da 742 a 869.
- Ho capito che con 3 mezzi e sei borghi non si salva tutto e che le evacuazioni lunghe (~30-130 min) vanno ordinate presto.
- In P3 ho notato la freccia/vento diversi, ma non ho capito perche' il fuoco partisse da un'altra parte ne' ho usato la lezione "chi e' sottovento": ho dato ordini a 3 borghi su 6 e ho perso gli altri.
- Resta oscuro: la differenza pratica tra Preallerta ed Evacua (ho visto che Preallerta ha comunque fatto partire 56 famiglie su 190 in ~1 ora, ma non l'ho capito dal testo), cosa fanno davvero i mezzi di difesa e il loro effetto sulle case.

---
## Traduzione per gli sviluppatori
### Difetti tecnici (in ordine di gravita)
1. **Debrief tagliato in alto e in basso, nessun modo per arrivare ai pulsanti di rigioca/cambia difficolta** (P1, P2, P3, schermata finale; viewport 1745x824 css, DPR 1.1, screenshot 1920x906). Titolo/intestazione sovrapposti alla barra in alto ("Facile ... Concluso" copre l'inizio del pannello), ultima riga arancione tagliata ("Nella realta..."). Rotella del mouse simulata e trascinamento non scorrono. Certezza: alta per il taglio; media sul fatto che anche con mouse reale non scorra (non verificato). Correzione: pannello a altezza massima = viewport con scroll interno, pulsanti "Rigioca"/"Nuova partita" fissi in fondo; ridimensionamento in base all'altezza.
2. **Legenda tagliata in basso** (P4, aperta a inizio partita): l'ultima riga ("preallerta") e' troncata; il pannello e' piu alto dello spazio. Certezza alta. Correzione: scroll o altezza massima.
3. **Decisione critica non raggiungibile con la latenza tipica e perdita silenziosa** (P3, 14:45, "CRISI: vento girato verso Castelvento", scaduta). Con un visitatore reale il problema potrebbe non esistere; comunque nel registro in basso a destra era piccola e il popup non l'ho visto. Certezza media. Correzione: popup centrale a pieno schermo con timer visibile e suono/lampeggio sul bordo.
4. **Etichette delle squadre sovrapposte dalle schede dei borghi** (P3, centro-destra, "A1/SQ" nascosti sotto Le Terrazze/San Rocco). Certezza alta.
5. **Fuoco poco visibile all'inizio** (P1/P2/P3: la macchia e' piccola e spesso sotto il pannello "Ultimi aggiornamenti" in basso a destra nella P1). Certezza alta. Correzione: marker/alone sul punto di innesco prima di Avvia.
6. **Bottoni della scheda grigi prima di "Inizia la partita" senza spiegazione** (P1, scheda Le Terrazze, a ~14:00 pre-start). Certezza alta. Correzione: tooltip "premi Inizia" o attivarli subito.
7. Nota ambientale (non del gioco): i click sul canvas con DPR 1.1 hanno coordinate diverse per UI (pixel dello screenshot) e per picking sulla mappa (pixel css); su un chiosco con DPR 1 non dovrebbe accadere. Non verificato con mouse reale.

### Difetti di comprensione
1. **Preallerta vs Evacua**: il testo "Prealerta: si tengono pronti - Evacua: partono" e' troppo breve. In P3 la preallerta ha fatto partire 56/190 famiglie in un'ora; non e' spiegato. Certezza media. Correzione: nella scheda mostrare "se il fuoco arriva, partono piu in fretta" e stima di famiglie che partono da sole.
2. **Nessun effetto visibile della difesa** (P1/P2): case 59/76 identiche con 3 mezzi su San Rocco in entrambe; il debrief non mostra cosa hanno salvato i mezzi. Certezza media. Correzione: nel "Perche" indicare quante case salvate dai mezzi rispetto a "senza ordini".
3. **Il tutorial "Come si gioca" ha 4 punti lunghi** e dice "hai 3 pause, non una dopo l'altra": non capisco se le pause si ricaricano; la scritta "Pausa tra 15 min · 3 rimaste" non e' immediata. Certezza media.
4. **Punteggio "su 1000" con famiglie/case diverse**: "Famiglie: 8 salvate su 11 (73 %)" e "Le famiglie contano di piu' delle case" ma la prima voce e' relativa a famiglie ancora in casa e non a tutte (11 su 100+...). Il perche' di 742 non e' calcolabile. Certezza media.
5. **Cambio difficolta non spiegato**: in P3 non mi e' chiaro cosa cambia (solo il sottotitolo iniziale); il messaggio della volontaria cita il nuovo punto di innesco, buono.

### Tre cose che funzionano e vanno tenute
1. Schermata iniziale con tre icone e la volontaria che parla: si capisce subito il compito; difficolta chiare.
2. Schede dei borghi con tempi di evacuazione stimati ("le ultime ~46 min") e badge sulla mappa (priorita, mezzi, barra famiglie): un'informazione concreta per decidere.
3. Debrief "Perche" per borgo con orari ("Il fuoco ha raggiunto 1 famiglia alle 16:04") e confronto "senza ordini": mi ha spinto a rigiocare.

### Screenshot salvati (cartella playtest6)
p1-01-inizio, p1-02-piano-sanrocco, p1-03-come-si-gioca, p1-04-tempo-scorre, p1-05-finale, p1-06-finale-tagliato, p2-01-pausa-fuoco-vicino, p2-02-riprendi-piano-aggiornato, p2-02b-tre-evacuazioni, p2-03-finale-869, p3-01-medio-inizio, p3-02-crisi-persa-vento-girato, p3-03-finale-medio-360, p4-01-esperto-legenda-tagliata (tutti .jpg).
