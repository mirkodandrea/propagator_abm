# Rocca Ventosa: evacuate anche i pannelli, stanno tutti nello stesso posto

Da studente di design con deuteranopia, mi iscrivo al chiosco della Protezione Civile e scopro che l’emergenza comprende il layout.

Ho finito la partita: 71 case colpite, 3 famiglie sorprese in casa. Il confronto senza ordini segna 73 e 16. Le decisioni contano. Anche poterle leggere dovrebbe contare.

A 1366×768 l’introduzione si mangia la legenda e i controlli della vista litigano con la barra del tempo. Scelgo «Difendi» e Le Coste sale sopra Il Piano. Persino nella vista larga, durante la simulazione i pannelli si coprono. Ottimo esercizio di priorità: prima decidi quale informazione riesci a vedere.

La palette organizza fuoco arancione, allarmi rossi/arancioni, difesa verde, preallerta gialla ed evacuazione blu. Il gioco non è completamente ostaggio dei colori: ci sono distanze, etichette e numeri. Però tre stati in legenda sono cerchi colorati; le tracce verdi attraversano un territorio verde. La legenda spiega la tinta, non risolve la ricerca visiva. Servono forme e tratteggi distinti, non un esame di campionari.

Il testo principale si legge; il problema è il condominio di note, contatori, registro eventi e pannelli semitrasparenti. Alberi e fiamme fanno da carta da parati alle informazioni operative. La crisi almeno ha un fondo uniforme: poi ti dà 25 secondi per leggere, scegliere fra priorità/preallerta/evacuazione e confermare. Ho catturato 23 e 17 secondi; il fuoco continua a ×1. Chi legge lentamente sta giocando anche contro il proprio ritmo di lettura.

Tab e Invio, provati in pianificazione e in corsa, non mi hanno portato ai comandi. Nessun focus visibile sui pulsanti: il canvas è una sala operativa con la porta della tastiera chiusa.

Sul touch, le istruzioni «rotella» e «tasto destro» sembrano un biglietto lasciato da un mouse. Ci sono +/−, bene; i gesti equivalenti non sono spiegati. Non certifico il touchscreen: solo viewport touch emulato, niente dispositivo fisico. Nemmeno zoom 125–150% o deuteranopia DevTools sono verificati: le scorciatoie non hanno dato uno zoom confermabile e il filtro visivo non era esposto dagli strumenti.

Al chiosco c’è un operatore. Spero aiuti gli studenti a ragionare sul rischio, senza dover interpretare anche l’interfaccia.

---

## Traduzione per gli sviluppatori

Valutazione dell’interfaccia visibile e delle interazioni, senza accesso a codice o documentazione. Il personaggio del post non equivale a un test fisiologico con un partecipante daltonico. Partita completa fino a T+3:00; prove aggiuntive separate per acquisire la crisi. Click sul canvas tramite sequenze pointer e mouse, distanziate di circa 150 ms: i limiti dell’automazione non sono attribuiti al gioco.

Critiche ordinate per gravità:

1. **Accesso da tastiera non trovato.** In pianificazione, Tab→Invio non ha attivato «Conferma e avvia»; in corsa, due Tab→Invio non hanno dato accesso visibile agli ordini. L’albero di accessibilità restituito dal browser esponeva un canvas focalizzabile, senza i pulsanti disegnati. **Certezza alta per queste sequenze, media sull’assenza di qualsiasi scorciatoia:** non ho testato ogni tasto possibile. **Correzione:** comandi semanticamente esposti, ordine di tabulazione, focus evidente, attivazione con Invio/Spazio; istruzioni per eventuali scorciatoie. Verificare anche con tecnologie assistive: qui non è stato fatto un test con screen reader.

2. **Informazioni operative occluse.** A 1366×768 il pannello iniziale copre il lato destro della legenda; in alto controlli vista e stato temporale si sovrappongono. Dopo aver selezionato la difesa delle Coste, il relativo pannello si sovrappone a Il Piano. A 1800×906 i dettagli delle famiglie continuano a coprirsi in corsa, per esempio T+0:10. **Certezza alta.** **Correzione:** separare riepiloghi e controlli dalla mappa in una zona impaginata; prevenire collisioni dei pannelli, consentire scorrimento senza coprire gli ordini. Prova: `01-sovrapposizioni-1366.jpg`, `02-pannelli-in-corsa.jpg`.

3. **Tempo di lettura vincolato nella crisi.** Nella prova aggiuntiva, a T+0:26 la previsione meteo presenta una descrizione, tre azioni e «Continua con il piano attuale». Catturati 23 e 17 secondi residui; barra e numeri rendono il tempo leggibile. Lo stato mostra ×1 e il testo avverte che il fuoco continua. Nessun controllo di estensione è visibile. Nella partita completa, le scadenze lasciano il piano attuale, come riportato dagli eventi. **Certezza alta sui comportamenti osservati; media sull’impatto per lettori lenti, non testati con partecipanti.** **Correzione:** modalità chiosco con durata regolabile o pausa di lettura, eventualmente controllata dall’operatore; separare lettura e tempo di scelta. Prova: `04-decisione-critica.jpg`. I 25 secondi iniziali sono indicati nella consegna; le acquisizioni iniziano a 23.

4. **Densità e contrasto variabile del testo.** In corsa coesistono legenda, tre pannelli cittadini, mezzi, etichette sulla mappa e registro eventi. Testo secondario visivamente circa 16 px, comandi circa 18 px nella vista acquisita: stime da screenshot, non misure tipografiche. I fondi trasparenti lasciano passare vegetazione e incendio, e le sovrapposizioni aggiungono testo su testo. **Certezza alta sulla densità e trasparenza; giudizio qualitativo sul contrasto.** **Correzione:** fondi opachi per dati essenziali, riepiloghi brevi, dettagli su richiesta, ingrandimento del testo indipendente dalla camera. Non è stato misurato un rapporto di contrasto WCAG.

5. **Ridondanza incompleta dei colori sulla mappa.** La legenda usa cerchi verdi per case difese, gialli per preallerta, blu per evacuazione. Percorsi verdi su terreno prevalentemente verde; distanza dal fuoco su badge rossi/arancioni. Etichette, numeri, rombo per famiglia minacciata e cartello rettangolare per area sicura mitigano il problema. **Certezza alta sulla codifica; media sull’effettiva confondibilità con deuteranopia, non emulata.** **Correzione:** icone diverse per i tre stati, percorsi con tratteggi e contorni ad alto contrasto, livello di urgenza scritto oltre alla distanza. Non sostituire semplicemente rosso/verde con un’altra coppia di tinte.

6. **Istruzioni della camera dipendenti dal mouse.** Sempre in alto: trascinamento, rotella per zoom, tasto destro per rotazione; presenti anche +, − e «Vista iniziale». I pulsanti cittadini sono visivamente alti circa 32–36 px e ravvicinati. **Certezza alta sulle istruzioni; bassa sull’usabilità reale al tocco:** viewport con `hasTouch` attivo, ma niente touchscreen fisico né prova valida di pinch/rotazione. Il cambio di emulazione ha ricaricato il gioco: non lo attribuisco a un gesto del giocatore. **Correzione:** documentare e verificare trascinamento/pinch, offrire rotazione tramite pulsanti, ampliare bersagli e spaziature; testare su hardware del chiosco.

7. **Zoom del browser: verifica incompleta, non bug dimostrato.** Tentate scorciatoie Control+ e Meta+ dopo il ridimensionamento; nessun cambiamento visivo che consentisse di confermare 125% o 150%. **Certezza alta sul limite della prova, nessuna conclusione sul comportamento del gioco a quei livelli.** **Azione proposta:** test reale con percentuale visibile nel menu browser, verificando testo, collisioni e accesso ai comandi a entrambi i livelli. Lo zoom della camera non dimostra lo zoom del testo.

L’emulazione di deuteranopia di Chrome **non è stata applicata**: il connettore DevTools disponibile non esponeva «Emulate vision deficiencies» e il controllo grafico di Chrome non era utilizzabile. Nessun risultato qui deriva da quel filtro.

## Tre cose che funzionano e vanno tenute

1. **Ordini con etichette, distanze numeriche e stati scritti:** «Evacua», «Preallerta», «fuori servizio» e conteggi offrono informazioni oltre al colore.
2. **Pianificazione iniziale a tempo fermo**, con spiegazione della differenza tra preallerta ed evacuazione e avviso che quest’ultima non si annulla.
3. **Riepilogo finale su fondo opaco, con numeri e confronto senza ordini:** rende comprensibili le conseguenze del piano. La partita completata termina con 71 case colpite e 3 famiglie colte in casa, contro 73 e 16 nel confronto. Prova: `03-finale.jpg`.
