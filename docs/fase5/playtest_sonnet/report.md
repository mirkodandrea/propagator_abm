# Playtest 5: Giulia, prima volta, tre partite (Sonnet 5.5, 9 ottobre 2026)

Build `3872a1b`: ordini solo in pausa, 3 pause distanziate, punteggio e classifica. Lo stesso prompt di Giulia (`prompt.md`), giocato da un agente Sonnet con Chrome DevTools, senza accesso al repository. L'agente non poteva scrivere file, quindi questo resoconto è stato salvato dalla sessione principale a partire dal suo testo.

| Partita | Caso | Famiglie colte / case colpite | «Grazie a te» | Punteggio |
|---|---|---|---|---|
| 1, alla cieca | Coste2_gira | 3 / 70 | −13 / −3 | 1330 |
| 2, «Riprova» | Coste2_gira | 3 / 71 | −13 / −2 | 1470 |
| 3, «Altro incendio» | un caso vicino a Pian dei Grilli | 2 / 12 | −6 / −12 | 770 (3ª) |

## Difetti, verificati sulle schermate dove indicato

1. **Il punteggio contraddice l'esito.** La partita 3 è la migliore in assoluto ma vale 770, contro i 1470 della partita 2. Il punteggio misura il vantaggio rispetto a nessun ordine, che cambia da incendio a incendio, e la classifica è unica (verificato su `p3-05-classifica.jpg`). È un conflitto con la scelta «classifica unica»: va deciso con l'utente.
2. **Priorità a un paese irraggiungibile.** Alla crisi del meteo, «Pian dei Grilli per primo» ritira i mezzi da Le Ghiande verso un paese dove «non possono avvicinarsi», e si perdono entrambi. Prima della conferma l'avviso è solo «da confermare». Partite 1 e 2.
3. **In pausa Evacua è un interruttore.** Il secondo click toglie l'ordine proposto; Giulia credeva di aver evacuato e l'evacuazione è partita solo alle 15:38 (verificato nel «Perché»). Contraddice «Un ordine dato non si ritira».
4. **Preallerta ed Evacua poco distinguibili sulla mappa:** cambiano solo una riga piccola e il colore della barra.
5. **Famiglie che non partono senza spiegazione.** A Castelvento, con l'evacuazione ordinata alle 14:20, 36 famiglie su 160 non partono e il «Perché» non dice perché (verificato su `p1-04-finale.jpg`). È il comportamento del modello: circa il 22 % aspetta di vedere il fuoco o difende la casa.
6. **Pulsanti grigi prima di «Inizia la partita» e «Ho capito»:** cliccati più volte senza risposta.
7. **Bonus per le pause non usate:** premia chi non usa la pausa, mentre il tutorial dice «usale bene».
8. **Fuoco poco visibile nei primi 30 secondi:** una macchia piccola a bordo mappa.
9. **Crisi incostanti:** nella partita 3 non ce n'è stata nessuna.

**Difetti tecnici:**
- numeri piccoli nel finale, sovrapposti ai contorni delle barre;
- etichette A1 e A2 sovrapposte;
- le schede coprono il territorio, soprattutto Castelvento.

## Funziona

- Il finale con il confronto «senza ordini», il «Perché» e la frase sull'errore di aspettare il fuoco.
- «Come si gioca» e i personaggi: frasi brevi e al momento giusto.
- La rigiocabilità: «Riprova», «Altro incendio» e la classifica («voglio battere GIU»).

## Curva di apprendimento

Giulia impara «evacua presto», dal finale e dalla barra verde. Non impara invece che la priorità non basta se i mezzi non arrivano: rifà lo stesso errore alla crisi della partita 2.
