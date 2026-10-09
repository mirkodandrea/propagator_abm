
Regole comuni:
- Non hai accesso al codice né alla documentazione del gioco e non devi cercarli: giudichi solo quello che vedi e fai giocando.
- Ogni critica deve essere VERA e basata su ciò che hai visto; se una cosa non l'hai verificata, dillo. Niente insulti personali né volgarità pesanti: il bersaglio è il gioco.
- Il gioco si chiama «Rocca Ventosa». È pensato per un chiosco presidiato alla Settimana della Protezione Civile, per studenti delle superiori: si difende un territorio da un incendio scegliendo priorità e ordini ai cittadini. È in italiano e si gioca col mouse (o col touch).
- Aprilo nel browser su http://localhost:8765 e aspetta il caricamento (~50 MB). La scheda deve restare in primo piano, perché il browser mette in pausa le schede nascoste. Se i click veri sul canvas non funzionano, simulali inviando a `#rocca` sia `pointermove/pointerdown/pointerup` sia `mousemove/mousedown/mouseup` (clientX/clientY, button 0), con circa 150 ms tra un evento e l'altro.
- Una partita dura circa 5 minuti reali. Alle «decisioni critiche» c'è un conto alla rovescia.
- Annota man mano, con l'orario, cosa vedi, cosa pensi e cosa fai, in `diario.md` in questa cartella.
- Alla fine scrivi in questa cartella `report.md`, al massimo ~1200 parole:
  1. il racconto di Giulia, in prima persona, una sezione per partita, con esito (famiglie colte in casa / case colpite) e durata reale;
  2. «Cosa ho imparato tra una partita e l'altra»: la curva di apprendimento;
  3. sotto una riga `---`, la «traduzione per gli sviluppatori». Per ogni difetto indica cosa hai visto, in quale partita e schermata, quando e dove sullo schermo, quanto ne sei sicuro e una correzione proposta, in ordine di gravità. Separa i difetti di comprensione da quelli tecnici;
  4. le 3 cose che funzionano e vanno tenute.
- Salva 6-10 screenshot significativi in questa cartella, con nomi parlanti (es. `p1-01-inizio.jpg`).
