# Playtest automatico 3: dopo l'iterazione 5b (9 ottobre 2026)

Gli stessi quattro personaggi del playtest 2 (GPT 6.1 Sol con Codex), nel browser, sulla build web di `0d3bd83`. Nessun accesso a codice o documentazione. Prompt in `prompt/`: sono quelli del playtest 2, con la durata della partita corretta da 10 a 5 minuti. Lanciati da `~/dev/experiments/playtest_gpt3` con `run.sh` dalle 15:18 alle 16:23.

| Personaggio | Partite | Esito (famiglie colte / case colpite) | Durata osservata |
|---|---|---|---|
| `q1_nuovo` studentessa alla prima volta | 1, Coste2_gira | 3 / 72, «−13 grazie a te» capito | ~7 min, dall'intro al finale |
| `q2_kid` quattordicenne con la fila dietro | 1 + inizio di «Altro incendio» | 3 / 72. Engagement «6/10», come nel playtest 2 | **4 min 38 s** dall'avvio al finale, pause comprese |
| `q3_minmax` cerca la strategia dominante | 4 sullo stesso incendio, a ×80 dalla barra operatore | AFK 16 / 73; evacua tutti 3 / 71; preallerta e poi evacuazione 1 / 71 (due varianti) | — |
| `q4_ux` gamer, solo UX e grafica | 1 | 3 / **52**, con Castelvento evacuato e in priorità | ~9 min, esplorando |

**Nessuna strategia dominante**, come nel playtest 2: le scelte cambiano gli esiti (16 → 3 → 1 famiglie colte). Le case colpite cambiano poco nelle prove di q3 (71–73), ma q4 arriva a 52 difendendo Castelvento: la difesa conta se va sul paese giusto.

## Funziona (citato da più tester)

- Intro breve con obiettivo concreto, pulsanti grandi con icone (tutti e quattro).
- Confronto finale con «lo stesso incendio senza ordini» e «grazie a te» (tutti e quattro).
- Il nuovo **«Perché»** nel finale: q1 lo legge e capisce l'effetto dell'evacuazione anticipata, q3 lo usa per analizzare. q2 invece smette di leggere.
- Barra delle famiglie con legenda; fuoco, bruciato e fumo leggibili.
- La volontaria con un fatto nuovo a metà partita: «4 famiglie sono ancora in casa» riaggancia q2.
- Il nuovo incendio di «Altro incendio» si capisce subito (q2).
- Pini e alberi tondi si distinguono (q4). Tutte e quattro le categorie della legenda invece non ancora (q1, q4).

## Difetti, in ordine di gravità (verificati sulle schermate dove indicato)

1. **La «decisione» del mezzo perso non ha scelte** (q2, schermata 03): timer e solo «Continua». Verificato: la crisi `MezzoPerso` ha solo il pulsante di chiusura. Chiamarla «Aggiornamento critico», senza timer, oppure offrire una vera alternativa.
2. **La scheda aperta copre il nucleo e il fronte** (q1 schermata 04, q4 diario 16:02). Le schede compatte non si coprono più tra loro, ma quella aperta si allunga sulla scena. Badge dei mezzi coperti dalle schede (A2 sotto Castelvento, schermata 01 di q4) e A1/SQ sovrapposti.
3. **Legenda tagliata in fondo** (q1, q4; verificato nella schermata 01 di q4): ultima voce a metà. Lo scorrimento c'è, ma non si vede. Copre Pian dei Grilli quando è aperta.
4. **Priorità senza mezzi** (q2): Castelvento in priorità 3 con «nessun mezzo rimasto». Il pulsante «Difendi» promette più di quanto dà. Prima della conferma dire chiaramente «0 mezzi».
5. **Ponte educativo nel finale** (q3): preallerta + evacuazione tardiva dà 1 famiglia colta, evacuazione immediata ne dà 3. Intanto la nota in fondo dice di non aspettare. Serve una riga che spieghi la differenza, o almeno la verifica che non sia rumore del seme. «26 partite» contro «25/30 evacuate» va riconciliato: una famiglia era ancora in viaggio o è stata raggiunta.
6. **Evacuazione: «viaggio ~4–102 min + attesa»** (q1): non si capisce quando sarà finita.
7. **Frasi lunghe della sindaca** sugli ordini (q2): vengono saltate. «Evacuazione di Le Ghiande avviata» basterebbe.
8. **Fumetto tagliato a sinistra** («nde, con il fuoco…», q4 alle 16:06). **Non verificato:** la schermata 01, alle 16:05, mostra lo stesso testo intero. Da controllare con frasi lunghe e finestre strette.
9. Minori (q4):
   - nella legenda del finale, «tu» in bianco contro barre colorate;
   - gli scudi della cronologia sembrano pulsanti;
   - ritratti illustrati e diorama low-poly sono due stili;
   - anelli e percorsi poco contrastati sul verde.

## Durata

Con ×40 / ×120 la partita giocata da q2 dura 4 min 38 s dall'avvio al finale, pause comprese; prima erano circa 9 minuti. Nessun tester segnala che sia troppo breve. Piano2 non è stato giocato.

## Non verificato

- **FPS nel browser:** i tester non li hanno misurati.
- **Touchscreen** e **countdown alle crisi:** gli strumenti automatici sono arrivati dopo la scadenza in quasi tutte le crisi, quindi non giudicano se 25 s bastino.
