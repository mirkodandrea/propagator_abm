# Playtest automatico 2: giocabilità, engagement, UX (9 ottobre 2026)

Quattro personaggi giocati da GPT 6.1 Sol (Codex) nel browser, sulla build web di `76623e3` con il layout 3, senza accesso a codice o documentazione. Rapporti e schermate in questa cartella.

| Personaggio | Partite | Esito |
|---|---|---|
| `q1_nuovo` studentessa alla prima volta | 1, Coste2_gira | 3 famiglie colte, 71 case; «−13 grazie a te» capito |
| `q2_kid` quattordicenne con la fila dietro | 1 + inizio di «Altro incendio» | engagement «6/10»; prima scelta dopo ~50 s; un minuto di calo d'attenzione |
| `q3_minmax` cerca la strategia dominante | 3 sullo stesso incendio | AFK 16 / 73 · evacua tutti 3 / 71 · preallerta e poi cambio piano 1 / 73 (famiglie / case): **nessuna strategia dominante** |
| `q4_ux` gamer, solo UX e grafica | 1 | pulsanti e fuoco chiari; schede che coprono mappa e tra loro |

**Funziona (citato da più tester):**
- il confronto con «lo stesso incendio senza ordini»;
- le crisi con countdown e costo esplicito;
- le etichette «nucleo 1/2»;
- i personaggi quando dicono un fatto (mezzo perso, famiglia raggiunta);
- pausa e conferma;
- pulsanti grandi con icone.

## Difetti, in ordine (verificati su schermate e codice)

1. **Stima d'arrivo falsa quando la difesa è fallita.** In q1, a T+2:37, Pian dei Grilli mostra «arrivi stimati ~2–2 min» mentre la squadra si ritira e le autobotti sono perse o bloccate. La stima viene dall'anteprima del piano (`Post.eta_s`), non dallo stato reale del mezzo (`ui.rs:790`).
   - Correggere con lo stato effettivo: «in postazione», «in arrivo ~N min», oppure «nessun mezzo: motivo».
   - Scrivere «~2 min», non «~2–2».
2. **Priorità senza effetto, motivo solo nel registro.** «Priorità 1 · nessun arrivo previsto»: il «non raggiungibile» compare solo negli aggiornamenti. In q3 il rientro alla base di A2 si scopre dopo la conferma.
   - Correggere: il motivo dentro la scheda e nell'anteprima prima della conferma («nessuna postazione sicura: A2 tornerà alla base»).
3. **Debrief senza perché.**
   - Il consiglio è uguale in ogni partita.
   - Le barre «evacuate» non hanno numeri.
   - La linea del tempo non lega ordini ed esiti.

   Correggere: X/Y accanto alle barre, poi 2–3 righe causali per paese (per esempio «preallerta a T+0 → partite in 12 min; 3 famiglie colte perché…»).
4. **Schede che coprono la mappa e tra loro.** Succede con lo zoom (q4); anche la legenda copre Pian dei Grilli. Il testo piccolo è poco leggibile sul fondo traslucido.
5. **Legenda poco trovabile.** Vegetazione e chiave della barra ci sono, ma in fondo al pannello che scorre: q1 non le ha viste. Inoltre «18 senza via» non dice che sono famiglie.
6. **Attesa a metà partita.** La volontaria ripete il tutorial («Il piano è in corso…») e i tester la ignorano.
   - Correggere: frasi con un fatto nuovo, come il prossimo arrivo, il fronte più vicino o chi è ancora a casa.
   - Spazio vuoto: indicare che cosa si può ancora fare di utile.
7. **«Altro incendio» entra senza presentarsi** (q2): manca una frase su dove parte e verso chi va.

**Minori:** «1 mezzi su 2»; un'etichetta di mezzo coperta dal pannello aggiornamenti (q1, 05-ritiro).

**Limiti della prova:**
- click simulati, con latenza: due crisi sono scadute per lentezza degli strumenti, non per il gioco;
- q3 a ×60;
- rotella e touch non verificati.

## Proposta

Un'iterazione sui punti 1, 2, 3 e 5, cioè i dati che il motore già produce e che l'interfaccia mostra male o in modo errato. Il punto 4 (disposizione delle schede) e il 6 (ritmo) vanno decisi con te dopo una prova al chiosco.
