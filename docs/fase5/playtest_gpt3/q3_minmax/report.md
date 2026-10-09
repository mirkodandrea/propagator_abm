# Ho provato a rompere Rocca Ventosa: lo spam non è la build migliore

Ho 16 anni e davanti a «poche squadre» penso subito: ok, dov'è l'exploit? Quattro run dello **stesso incendio Coste2_gira, seme 1**, passando sempre da **Riprova**, anche dopo l'ultima. Solo gioco visibile, niente codice/documentazione. Browser in primo piano; input simulati su `#rocca` con eventi pointer e mouse. F2: velocità **×40 → ×80**, crisi lasciate a **25 secondi**; ho usato anche Pausa operatore nelle run 3–4. Non sto valutando il ritmo standard del chiosco.

| Run / strategia | Famiglie colte in casa | Case colpite | Evacuate Castelvento / Pian / Ghiande |
|---|---:|---:|---|
| 1: AFK, nessun ordine, crisi ignorate | 16 | 73 | 1/160 · 49/55 · 17/30 |
| 2: evacua tutti T+0, difendi solo Ghiande, crisi ignorate | 3 | 71 | 124/160 · 55/55 · 25/30 |
| 3: preallerta tutti, difendi Ghiande; evacua tutti e promuovi Pian T+1:39 | 1 | 71 | 124/160 · 55/55 · 25/30 |
| 4: preallerta tutti, difendi Ghiande; evacua tutti e promuovi Castelvento T+1:20 | 1 | 71 | 124/160 · 55/55 · 25/30 |

Diario sintetico, 9 ottobre, ora italiana (dettagli in [diario.txt](diario.txt)):

- **15:39–15:43:** parto AFK. A T+0:26 avviso del vento verso Pian; a T+0:42 minaccia Castelvento. Ignoro. Le famiglie partono anche autonomamente. Finale: 16/73. AFK non è gratis.
- **15:44–15:46:** quattro click iniziali, poi lascio scadere tutto. Finale 3/71: enorme vantaggio sulle persone, appena due case risparmiate.
- **15:47–15:51:** preallerta; mi scade la crisi mentre osservo. Tentativi intermedi spostano un'autobotte, poi la cronologia la dà persa vicino Pian a T+1:12. A T+1:39 cambio davvero: Pian primo, ma nessuna postazione sicura; l'autobotte rimasta deve lasciare Ghiande. Finale 1/71.
- **15:52–15:56:** anche qui il tentativo sulla crisi scade; pausa T+1:20, cambio verificato verso Castelvento. Finale ancora 1/71.

**Verdetto da tryhard:** nessuna vittoria garantita dimostrata; nessuna run azzera entrambi i danni. Le scelte contano: 16→3→1 famiglie colte. Cambiare idea modifica destinazioni e può togliere una difesa; farlo tardi non crea magicamente postazioni sicure. Non ho verificato un cambio tempestivo pulito: nelle ultime due run ci sono tentativi intermedi, quindi niente causalità isolata.

La scorciatoia «evacua tutto subito» è forte, ma non batte i miei risultati con preallerta. Il gioco offre ragionamento su vento, viaggi e tempi; la parte case però mi sembra poco gratificante: tre piani attivi, sempre 71. E il finale mi lascia un dubbio educativo: perché preallerta + ordine tardivo dà meno famiglie colte dell'evacuazione immediata, mentre sotto mi avverte di non aspettare? Non dico che sia un bug; manca il ponte esplicativo. Anche «26 partite» contro «25 evacuate» mi fa cercare la famiglia mancante.

---

Traduzione per gli sviluppatori, gravità decrescente:

1. **Spiegazione educativa incompleta.** Finali run 2–4, 15:46/15:51/15:56: riquadro centrale superiore 3→1 famiglie; sezione «Perché» inferiore e avvertenza gialla non spiegano il vantaggio della preallerta rispetto all'ordine immediato. Certezza alta sull'osservazione, bassa sulla causa. Proposta: separare contributi di preallerta, evacuazione, partenze spontanee e difesa, spiegando questo confronto.
2. **Feedback della difesa debole in queste prove.** Finali, contatore case in alto a destra: 73 AFK, 71 nelle tre run attive, anche cambiando destinazione. Certezza alta sui numeri; bilanciamento generale non verificato. Proposta: mostrare case risparmiate per mezzo/nucleo e opportunità perse, prima di modificare il bilanciamento.
3. **Popolazione finale poco riconciliabile.** Run 2 e 4, riga Ghiande: barra destra 25/30 evacuate, testo inferiore 26 partite. Certezza alta; nessun errore matematico dimostrato. Proposta: aggiungere conteggi finali di famiglie in viaggio, rimaste e raggiunte dal fuoco.

Tre cose da tenere: confronto con lo stesso incendio senza ordini; cronologia con tempi e conseguenze; separazione fra persone e case, che impedisce di chiamare «win» un solo numero verde.
