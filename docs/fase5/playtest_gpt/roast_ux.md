# r/italy — Rocca Ventosa: il vero incendio è la gerarchia dell’interfaccia

Ho 16 anni, gioco a strategici e gestionali e alla Settimana della Protezione Civile ho provato Rocca Ventosa. Ho finito una partita. Pensavo di difendere tre paesi, invece ho fatto ranked contro i pannelli.

«Conferma e avvia» è un bel bottone giallo gigante. «Preallerta» ed «Evacua», cioè le cose con cui dai ordini alle famiglie, sembrano due parole rimaste lì dopo un copia-incolla. Il gioco ti insegna la priorità delle priorità: prima trova il pulsante.

Arriva la decisione critica: testo in alto, comandi sparsi sui paesi, previsione dei mezzi in basso a sinistra, conferma in fondo. Hai 25 secondi. Aim trainer, ma per i bulbi oculari.

Le distanze dal fuoco sono rosse su pannelli scuri trasparenti, con bosco e strade sotto. Anche il mezzo bloccato scrive in rosso. L’allarme ha attivato la modalità stealth.

Provo la camera: due trascinamenti senza uno spostamento visibile; con la rotella cambio vista e le schede finiscono sopra il riepilogo. Pure l’interfaccia ha il traffico, solo che qui si tamponano i testi.

La collina si capisce, il bosco pure, il fuoco arancione lo trovi subito. Poi arrivano anelli blu e verdi, percorsi colorati e una frecciona bianca: manca la legenda, ho sbloccato il DLC «interpreta gli elastici». Case piccole, mezzi e persone difficili da distinguere nella vista generale: per capire chi sta facendo cosa leggo le targhette. Simulatore di sottotitoli con montagna inclusa.

Lo stile fa una cosa buffa: paesaggio da plastico, comandi da etichetta, conferma da pulsantone arcade. Tre giochi nella stessa lobby.

Finale: tabella, cronologia e definizioni, tutto su un pannellone trasparente davanti alla mappa. Ho ottenuto 2 case colpite in meno e 13 famiglie colte in casa in meno; per leggere bene il resto serve la build commercialista.

Il terreno e il fuoco hanno presenza. Ora evacuate un po’ di testo dallo schermo, grazie.

---

## Traduzione per gli sviluppatori

Osservazione della sola interfaccia, su Chrome a 1800×1050 inclusa la cornice del browser. Una partita completa da pianificazione a T+3:00, mantenendo la scheda in primo piano. Nessun accesso a codice o documentazione. I click nativi hanno funzionato, quindi non è servita la simulazione di eventi. Gravità in ordine decrescente; le posizioni si riferiscono agli screenshot.

1. **«Si tamponano i testi» — sovrapposizioni dopo il cambio di vista.** In pianificazione, dopo trascinamento e rotella verso l’alto, le schede del Piano e delle Coste si trovano in basso a sinistra sopra il pannello di anteprima. Le righe si sovrappongono e il paese di sinistra è in parte fuori dall’inquadratura. **Correzione:** riservare spazio ai pannelli fissi, mantenere le schede entro l’area utile e gestire le collisioni tra etichette; consentire di richiudere l’anteprima. [Evidenza](ux/02-camera-sovrapposizioni.jpg).

2. **«Aim trainer per i bulbi oculari» — flusso della crisi disperso.** A T+0:30, avviso e conto alla rovescia in alto, azioni distribuite su tre schede, proposta dei mezzi in basso a sinistra e pulsante di prosecuzione in basso al centro. Il risultato di una modifica richiede di rileggere più zone entro il limite di tempo. **Correzione:** raggruppare nella crisi minaccia, azioni pertinenti, variazione prevista dei mezzi e conferma; evidenziare sulla mappa il luogo interessato. [Evidenza](ux/03-decisione-critica.jpg).

3. **«L’allarme in modalità stealth» — contrasto debole degli stati importanti.** Il rosso delle distanze nelle schede e dell’etichetta «Autobotte 2 · bloccata: strada tagliata dal fuoco» in basso a destra è poco leggibile sui fondi scuri trasparenti. Anche le etichette verdi dei mezzi competono con la vegetazione. **Correzione:** usare fondi più opachi, testo chiaro e icone/indicatori separati per urgenza e stato; verificare il contrasto sul terreno più chiaro e più scuro. [Evidenza](ux/04-mappa-stati.jpg).

4. **«Prima trova il pulsante» — riconoscibilità disomogenea dei comandi.** Nella pianificazione, «Preallerta» ed «Evacua» appaiono come testo semplice sotto ciascun paese; «non difendere» ha invece un piccolo fondo grigio e l’avvio un grande bottone giallo. I click producono feedback: numero/bordo della priorità e sfondo blu degli ordini selezionati. Il problema è soprattutto capire prima del click cosa sia un controllo. **Correzione:** dare a tutte le azioni forma e area cliccabile riconoscibili, con stati distinti per disponibile, selezionato e disabilitato. Conservare il feedback già presente. [Evidenza](ux/01-pianificazione.jpg).

5. **«DLC interpreta gli elastici» e «simulatore di sottotitoli» — significato della mappa poco autonomo.** In pianificazione e in corso compaiono anelli di più colori e tracciati chiari/verdi, senza legenda visibile. Nella vista generale case, mezzi e persone hanno una scala che rende difficile distinguere le attività senza leggere etichette e conteggi. Il fumo grigio e il terreno bruciato aggiungono ulteriore dettaglio attorno al fronte. **Correzione:** legenda compatta per gli elementi effettivamente mostrati, simboli diversi oltre al colore, icone dei mezzi leggibili alla distanza corrente, percorsi filtrabili e indicatori aggregati per le evacuazioni. [Evidenze](ux/01-pianificazione.jpg), [mappa in corso](ux/04-mappa-stati.jpg).

6. **«Due trascinamenti senza spostamento» — controlli della camera poco scopribili.** Due trascinamenti nativi col pulsante sinistro sul terreno, uno prima dell’avvio e uno durante la partita, non hanno prodotto spostamenti visibili. Il primo uso della rotella ha cambiato nettamente l’inquadratura; ulteriori tentativi durante la partita non hanno mostrato una variazione evidente. «Vista iniziale» in alto a destra ha ripristinato la vista. Queste sono osservazioni della sessione, non una diagnosi della causa. **Correzione:** mostrare i gesti supportati e fornire controlli espliciti di zoom/spostamento con limiti comprensibili; mantenere il ripristino. [Vista risultante](ux/02-camera-sovrapposizioni.jpg).

7. **«Build commercialista» ed «evacuate il testo» — finale denso e poco separato dallo sfondo.** A T+3:00 il pannello centrale contiene tabella con intestazioni su più righe, risultato giallo, perdita del mezzo in rosso, cronologia e definizioni in carattere più piccolo. La mappa resta visibile attraverso il pannello e il registro eventi resta a destra. Il confronto è utile, ma richiede una lettura impegnativa. **Correzione:** aprire con pochi indicatori grandi, separare chiaramente le colonne del confronto, rendere opaco il fondo e mettere cronologia/definizioni in dettagli espandibili. [Evidenza](ux/05-finale.jpg).

8. **«Tre giochi nella stessa lobby» — linguaggio visivo dei controlli incoerente.** Il plastico 3D usa forme semplici e colori naturali; sopra convivono azioni simili a etichette, piccoli pulsanti grigi, selezioni blu e grandi conferme gialle. **Correzione:** adottare una famiglia coerente di controlli, spaziature e dimensioni, riservando il giallo all’azione primaria. Conservare lo stile semplice della scena. [Evidenza](ux/01-pianificazione.jpg).

### Tre cose della grafica da tenere

- **Morfologia del terreno:** rilievi, versanti verdi e zone più aride rendono riconoscibile la collina anche nella vista generale.
- **Vegetazione distribuita sul territorio:** boschi densi e aree aperte distinguono visivamente le parti della mappa senza dipendere dalle schede.
- **Fronte del fuoco e traccia bruciata:** l’arancione acceso attira subito l’occhio; la superficie scura lasciata dietro permette di seguire l’avanzamento. È la parte della simulazione più leggibile direttamente sulla mappa.
