# Rocca Ventosa — diario di una prima partita

Sono una studentessa di quarta, arrivo da sola e gli strategici non li ho mai giocati. Nessuno mi spiega niente. Orari locali del 9 ottobre 2026; giudico soltanto le schermate della mia partita. Ho usato click simulati sul canvas: i tempi del controllo automatico limitano soprattutto la valutazione del conto alla rovescia.

**15:19:50 — Primo sguardo.** Nei primi trenta secondi capisco il problema: tre paesi, due autobotti e una squadra, devo scegliere chi aiutare. Leggo davvero la volontaria: è una frase breve, non un manuale. La mia prima azione intenzionale è premere «Inizia»; entro nella mappa alle 15:20:36.

**15:20:36 — Quante cose.** Vedo 55, 30 e 160 famiglie. Penso che il numero nel cerchio sarà la priorità; lo verifico scegliendo «Difendi» per Le Ghiande. Evacuo subito quel paese perché il fuoco è a mezzo chilometro. Riconosco i due gruppi «Le Ghiande · nucleo 1/2», ma le schede coprono parecchie case: non riesco ad attribuire con sicurezza ogni gruppo agli altri paesi.

**15:21:22 — La legenda c'è!** Il pulsante in alto a sinistra si trova facilmente. Adesso capisco grigio a casa, giallo preparazione, blu viaggio, verde sicurezza. Leggo che prato secco, macchia, pineta e castagneto bruciano diversamente: quindi il verde non è tutto uguale. Però riconoscere quei quattro tipi nel paesaggio è più difficile che leggere i simboli. Il fondo della legenda è tagliato; non verifico se si possa scorrere.

**15:21:59 — Avvio.** Metto Pian dei Grilli secondo e preallerto lui e Castelvento. Le schede dicono quanti mezzi arrivano e in quanti minuti. Per le famiglie leggo «viaggio ~4–102 min + attesa»: capisco che non spariscono subito, ma non so quanto aspettare per completare l'evacuazione. L'orologio ×40 mi fa pensare che siano minuti del territorio.

**15:22:50–15:24:14 — Mi agito.** Castelvento è minacciato: leggo la sindaca e vedo il conto alla rovescia. Non completo la scelta prima della scadenza; il ritardo dell'automazione impedisce di giudicarne equamente la durata. Finisco poi in pausa, evacuo anche gli altri due paesi e confermo. «Pausa e modifica piano» è comprensibile.

**15:24:44–15:25:47 — Posso ancora intervenire.** Un'autobotte è persa. Leggo il caposquadra, ma gli aggiornamenti a destra li guardo a pezzi: sto seguendo il fuoco. Passando sopra Castelvento ricompaiono i pulsanti; la scheda ingrandita copre parte degli aggiornamenti. Provo la freccia verso l'alto e Castelvento diventa primo. «Da confermare» e il pulsante grande mi fanno capire che manca un passaggio. Confermo.

**15:27:02 — Finale.** Tre famiglie colte in casa, 72 case colpite: non mi sento bravissima. Però «−13 grazie a te» e il confronto senza ordini mi spiegano che ho aiutato. Leggo il «Perché»: evacuazioni, famiglie partite, tempo dei mezzi e primo impatto alle Ghiande. Capisco l'effetto dell'evacuazione anticipata; non verifico cosa sarebbe successo con un'altra priorità.

---

## Traduzione per gli sviluppatori — critiche per gravità

1. **Attribuzione delle case.** Pianificazione, 15:20:36: schede grandi a sinistra, centro e destra nascondono parte dei nuclei (01). Sicurezza alta sull'occlusione, media sull'ambiguità. Proposta: schede laterali e contorni/nome persistenti per ogni nucleo.
2. **Tempo totale di evacuazione incerto.** Scheda Le Ghiande, centro, 15:21:22: «~4–102 min + attesa» (02). Sicurezza alta sul testo, media sull'incomprensione. Distinguere viaggio, preparazione e attesa; mostrare una stima di completamento in minuti simulati.
3. **Lettura ostacolata.** Simulazione, circa 15:25: scheda Castelvento espansa sopra il registro in basso a destra (04). Sicurezza alta. Riservare spazi separati ai comandi e agli aggiornamenti.
4. **Vegetazione difficile da associare.** Legenda, 15:21:22, alto sinistra (02): simboli spiegati, paesaggio per me poco riconoscibile; fondo del pannello troncato. Sicurezza media sull'associazione, alta sul taglio; scorrimento non verificato. Aggiungere identificazione al passaggio del mouse e rendere evidente l'eventuale scorrimento.

**Tre cose da tenere:** introduzione breve con obiettivo concreto; barra delle famiglie con legenda; finale con confronto senza ordini e spiegazione cronologica.
