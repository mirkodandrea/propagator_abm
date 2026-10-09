# Rocca Ventosa — quattordicenne con la classe dietro

Bro, il bosco brucia e io sto facendo amministrazione comunale 💀. Il fuoco che avanza è bello da guardare. Il meteo che cambia e l’autobotte persa mi svegliano. Però tra schede, numerini e volontaria che ripete le istruzioni, il mio cervello fa skip.

Due momenti di noia, due volte perso nelle crisi. Non è tutto fermo: le fiamme si muovono. È che dopo aver dato gli ordini resto a fissare barre. La sindaca versione «ho firmato» la salto; versione «una famiglia raggiunta dal fuoco» la leggo. Meteo e vigili hanno qualcosa da dirmi, la volontaria durante la partita sembra il tutorial rimasto aperto.

Finale: 3 famiglie colte in casa, 71 case colpite. «−13 grazie a te» mi fa pensare: potevo fare meglio. Voglia di **Riprova: media**. **Altro incendio: bassa**, con venti compagni dietro. L’ho premuto: altra mappa, altre distanze, stessa spiegazione. Il nuovo incendio entra senza presentarsi. Engagement: **6/10**, picchi buoni, troppo cruscotto.

Tempi osservati, **UTC** (in Italia +2 ore):

- **11:07:19**: prima schermata giocabile osservata. **11:07:39**: «Inizia», dopo **20 s**. Prima priorità circa **11:08:09**, quindi **~50 s** dalla prima osservazione. Questi tempi includono strumenti e annotazioni: non sono una misura affidabile della reazione di un ragazzo. Caricamento non misurato.
- **11:08:13–11:09:02**: piano avviato; guardo mezzi, fuoco e 10/30 evacuate. Nessun vuoto visivo totale.
- **11:09–11:12**: due crisi meteo, due scadenze mancate; finisco anche in pausa. La latenza degli strumenti ha contribuito. Countdown «23» verificato nella seconda crisi.
- **11:13:36–11:14 circa**: priorità senza arrivi; poi autobotte persa. Qui leggo i vigili.
- **11:15:11–11:16:15**: due cali d’attenzione nello stesso tratto d’attesa, circa un minuto senza miei click. Fuoco attivo, ma Castelvento resta 124/160 e Le Ghiande 25/30 nelle due osservazioni. Non ho verificato se altri ordini avrebbero migliorato l’esito.
- **11:16:52**: finale, **9 min 13 s** dopo «Inizia», comprese pause. **11:17:38**: visto l’inizio di «Altro incendio»; seconda partita non completata.

---

## Traduzione per gli sviluppatori — gravità decrescente

1. **Priorità e difesa effettiva poco distinguibili.** Mappa, 11:12–11:13:36: scheda Pian dei Grilli, alto sinistra, «Priorità 1» ma «nessun arrivo previsto». Il motivo «non è raggiungibile ora» appare nel registro in basso a destra. Mi accorgo lì che la scelta non produce la difesa attesa. **Certezza alta** sul feedback, nessuna diagnosi sul motore. Proposta: vicino alla priorità mostrare «Non raggiungibile» e motivo, con segnale immediato alla conferma.

2. **Attesa con poca nuova informazione utile.** Mappa, 11:15:11–11:16:15: ordini dati, contatori di due paesi invariati nelle catture, volontaria in basso sinistra torna alle istruzioni generiche. Animazione presente, attenzione in calo. **Certezza media**, valutazione personale; non dimostra assenza di azioni possibili. Proposta: indicare prossimo evento/arrivo e quali interventi restano utili; mantenere l’accelerazione osservata a ×60.

3. **Attenzione dispersa durante le crisi.** 11:09–11:12: fumetto basso sinistra, countdown ancora più sotto, stato «Decisione» in alto e schede sparse. Due scadenze mancate. **Certezza alta** sulla disposizione, **bassa** sull’impatto nei tempi umani per la latenza. Proposta: unire minaccia, scelta e countdown in un solo blocco; verificarlo con studenti dal vivo.

4. **Dialoghi ripetitivi e ingombro.** Mappa 11:09:02 e 11:16:15: volontaria con quattro righe d’istruzioni; grandi schede coprono territorio a sinistra e destra. Ignoro quel fumetto; leggo invece perdita del mezzo e famiglia raggiunta. **Certezza alta**. Proposta: una riga di aggiornamento concreto, istruzioni richiamabili e schede comprimibili.

5. **Rilancio poco motivante.** Finale 11:16:52, pannello centrale statico: confronto utile, ma nessun prossimo tentativo suggerito. Nuova mappa 11:17:38: distanze cambiate, nessun richiamo evidente alla novità. **Certezza media**, preferenza personale. Proposta: suggerimento legato agli ordini fatti e breve presentazione della nuova minaccia.

**Tre cose da tenere:** fuoco e mezzi visibili in movimento; crisi del vento con countdown e conseguenze; confronto finale con lo stesso incendio senza ordini.
