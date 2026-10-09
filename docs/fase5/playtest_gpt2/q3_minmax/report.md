# [Playtest] Ho cercato il cheese di Rocca Ventosa: evacuare tutto è forte, ma niente god mode

Ho 16 anni e voglio trovare la build che rende inutile pensare. Tre run sullo **stesso incendio, Coste2_gira, seme 1**, usando **Riprova dopo ciascuna**, anche la terza. Browser visibile; solo gioco, zero codice/documentazione. Click simulati sul canvas. Orari UTC del 9 ottobre; diario completo in `diario.md`.

Accelerazione: F2 inizialmente mostrava ×20; ho osservato anche HUD ×60 e crisi ×1. I primi tentativi di digitare 60 non erano confermati. Nella terza run, a T+1:55, ho verificato **F2 e HUD entrambi ×60**. Countdown lasciato a 25 secondi. Non è una prova dei dieci minuti a velocità normale.

| Run / finale UTC | Strategia realmente applicata | Famiglie colte in casa | Case colpite | Famiglie/case evitate rispetto ad AFK |
|---|---|---:|---:|---:|
| 1 · 11:26 | Nessun ordine; crisi ignorate | 16 | 73 | 0 / 0 |
| 2 · 11:33 | Evacua tutti a T+0; difendi solo Ghiande; crisi ignorate | 3 | 71 | 13 / 2 |
| 3 · 11:40 | Castelvento difeso, Ghiande preallertate; a T+1:02 evacua tutti, Pian primo e Castelvento secondo | 1 | 73 | 15 / 0 |

**11:22, run AFK:** Ghiande evacua spontaneamente. Il gioco non congela i cittadini finché non clicco: bene. Ma al finale ho 16 famiglie colte. AFK build bocciata.

**11:28–11:33, run cheese:** ignoro meteo e richiesta di trasferire A2. Risultato molto migliore sulle persone, soltanto due case risparmiate. A1 viene persa. «Evacua tutto» non significa evacuazione completa: Castelvento si ferma a 124/160, Ghiande a 25/30. Nessun perfect.

**11:35–11:40, run reattiva:** perdo il primo timer per lentezza mia. Poi cambio davvero piano. A T+1:02 il registro dice che Pian non ha postazioni raggiungibili e sicure; A2 lascia Castelvento e torna alla base. Cambiare idea conta, ma non teletrasporta le squadre. Finisco con una sola famiglia colta: meglio della run cheese. Però avevo anche preallertato Ghiande: non posso assegnare il merito al cambio di priorità.

Verdetto tryhard: **scelte reali, dominante non dimostrata**. Nessuna run domina entrambe le metriche; non ho provato altri incendi né ripetuto ogni strategia. L'evacuazione anticipata è una scorciatoia potente rispetto all'AFK, ma tempi, preallerta e accessibilità dei mezzi danno spazio al ragionamento. Non ho visto un esplicito «hai vinto»: qui ottimizzo danni, non una vittoria binaria.

Il finale quantifica bene **quanto** ho cambiato. Il **perché** resta meno chiaro: stesso consiglio sull'evacuazione in tutte e tre le run, anche quando la preallerta più evacuazione tardiva batte l'evacuazione immediata sulle persone. Le barre delle evacuate non hanno numeri. Per un chiosco scolastico vorrei uscire sapendo quale decisione rifare, senza dovermi costruire un foglio Reddit.

---

**Traduzione per gli sviluppatori — gravità decrescente**

1. **Debrief poco specifico.** Finali 11:26/11:33/11:40, fumetto centrale inferiore: identico consiglio generico; non spiega il confronto 3→1 famiglie. Clic sul marcatore T+1:02 senza spiegazione aggiuntiva visibile. Certezza alta sulla schermata, limitata sulle altre interazioni. Proposta: collegare ordini, preallerta, ritardi e famiglie colte con 2–3 eventi causali personalizzati.
2. **Cambio operativo poco anticipato.** Run 3, T+1:02, registro in basso a destra: priorità Pian confermata, A2 rientra perché destinazione irraggiungibile; prima c'era «nessun arrivo previsto», meno esplicito. Certezza alta. Proposta: anteprima «nessuna postazione sicura; mezzo rientrerà» prima di Conferma.
3. **Finale incompleto numericamente.** Colonna verde a destra nei finali: solo barre, nessun n/N evacuate. Certezza alta. Proposta: mostrare evacuate, rimaste e colte per paese, con etichette distinte.

**Tre cose da tenere:** confronto con lo stesso incendio senza ordini; crisi che esplicitano tempi e costo del trasferimento; ordini modificabili con conferma e registro delle conseguenze.
