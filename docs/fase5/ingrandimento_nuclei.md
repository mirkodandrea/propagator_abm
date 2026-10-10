# Ingrandimento dei nuclei: layout 4 (`t4_paese4`)

2026-10-09, richiesta dell'utente («ingrandimento dei nuclei, con diversi quartieri», «borghi italiani», poi «fai tu, e poi fai un giro di playtest»). **Pubblicato** in `data/scenarios/rocca_ventosa` al posto del layout 3 (recuperabile con `git checkout -- data`).

## Cosa c'è
- **Generatore** (`tools/factory/town.py`, `Lane`): vicoli curvi che si staccano dalla provinciale, case a distanze e arretramenti irregolari e girate lungo la curva, senza sovrapporsi a strade o edifici civici. Le piste non sono dipinte come interruzione del combustibile.
- **Sei distretti:** Castelvento 190 famiglie, Pian dei Grilli 90, Fornaci 90 (nuovo), San Rocco 100 (nuovo), Le Terrazze 30 (nuovo, versante a macchia), Le Ghiande 38. Totale **538 famiglie, 1600 persone, 479 edifici** (layout 3: 245, 670, 207). La SP 12 attraversa Fornaci, Castelvento e San Rocco con curve dolci.
- **Inneschi** scelti dal generatore, 3 per località (18 casi + 18 «gira»), non più quelli dei tre paesi. Nomi: Borgo = Castelvento, Piano = Pian dei Grilli, Coste = Le Ghiande, Forna, Rocco, Terra.
- **Casi del kiosk** (`FEATURED`): `Forna3_gira` (principale: 5 distretti toccati), `Coste2_gira` (non ovvio: 650 ha), `Rocco3` (introduttivo: San Rocco).

## Decisioni prese (utente: «fai tu»)
1. **Rifugi:** restano 2 nel paese (Castelvento, Pian dei Grilli). Il criterio del modello (≤12 % di combustibile in 300 m, rifugi a ≥600 m) richiede fasce irrigue enormi, che tagliavano la valle e non facevano più arrivare il fuoco a Castelvento. Le aree di attesa di Fornaci e San Rocco sono disegnate ma non sono rifugi del modello.
2. **Le Terrazze** restano un quartiere tranquillo (fuoco a <150 m nel 0–7 % delle prove): serve a non sprecare mezzi.
3. **Nuclei non combustibili piccoli** per Fornaci e San Rocco, per non fermare il fuoco.

## Prova (`build-town --layout 4`, `verify`, `town-fires`, `rocca`, `cargo test`)
- `verify` verde. Famiglie con fuoco a <150 m entro 6 h (media per prova, atlante a 9 inneschi): 26,4 (layout 3) → 24,6.
- Gli effetti delle priorità restano reali. `Forna3_gira`: Fornaci prima → 13 case colpite a Fornaci e 23 alle Terrazze; San Rocco prima → 32 a Fornaci e 0 alle Terrazze (`target/release/rocca Forna3_gira --priorita ...`).
- `Coste2_gira` cambia poco con le priorità (il fuoco è troppo veloce): da rivedere se deve essere il caso «non ovvio».
- **Castelvento è colpita di rado** (`Borgo2`: 4 case; layout 3: 33). Il borgo al centro è il più protetto: le fasce non combustibili e il nuovo San Rocco lo schermano da est.

## Test adattati (37 target verdi)
Geometria cambiata, non la fisica: `fase3.rs` usa `Piano3` per priorità e preallerta (`Piano1` ora tocca poco Pian dei Grilli) e 190 famiglie; vento di prova 225° (`exposure`, `evacuation`, `authored_behaviour`) e 315° (`authored_unit_policy`) perché a 135° `plan_ignition` sceglie un'area dove il fuoco ristagna; `smoke.rs` accende in fuoco continuo (5×5); il test del rifornimento cerca combustibile attorno all'autopompa.

## Incognite
- Castelvento troppo al sicuro: valutare di riaprire il suo lato est (fascia verso San Rocco) o un caso più duro.
- Il criterio dei rifugi del modello impedisce un rifugio per quartiere.
- Sei schede coprono molta mappa a 1600×1000 (vedi playtest 5).
- Esito A/B su `Coste2_gira`, durata delle partite e prestazioni (479 edifici) da misurare al chiosco.

## Playtest 5 (2026-10-09)
- **Browser (Haiku 5.5 come Giulia): fallito, nessuna partita.** La scheda risultava nascosta (`visibilityState = hidden`) e il gioco non risponde finché non torna in primo piano: limite dell'ambiente di prova, già noto («FPS nel browser a finestra in primo piano»). Resoconto parziale in `playtest_nuclei/report.md`: nessun esito valido. Segnalato però: il testo dice «premi Avvia» mentre il pulsante iniziale dice «Inizia la partita».
- **Partite scriptate native** (`KIOSK_SHOT`, `Forna3_gira`, `Rocco3`, `Coste2_gira`): girano fino al debrief con sei distretti. Con la sola pianificazione automatica `Rocco3` termina con 8 famiglie colte in casa e 53 case colpite, `Coste2_gira` con 3 e 109 (non sono partite giocate).
- **Difetti visti:** a 1600×1000 le sei schede coprono buona parte della mappa e quasi tutti i borghi in fase di pianificazione; nel debrief i pulsanti «Riprova» / «Altro incendio» toccano il bordo inferiore e il punteggio copre il titolo; il testo di apertura diceva «tre paesi» (corretto in «sei borghi»).
- **Da rifare:** un playtest vero con la finestra in primo piano (browser o nativo) dopo aver sistemato schede e debrief.

## Interfaccia a badge e pannello del piano (2026-10-10, richiesta dell'utente)
Le schede grandi sono sostituite da **badge** sulla mappa (priorità, nome, distanza del fuoco, mezzi, ordine civile, famiglie in salvo): un clic apre la scheda completa accanto al badge, una sola alla volta («Chiudi» o clic di nuovo). In alto a sinistra c'è **«Il piano»**: priorità in ordine con mezzi e ordini, «proposta» o «in vigore», chi resta alla base, chi è senza ordini; una riga apre la scheda. Le etichette dei nuclei compaiono solo per il distretto aperto; l'apertura si chiude a ogni cambio di fase (Avvia, pausa, crisi). I pulsanti dicono «Prima» e «Non difendere» (la X senza parole sembrava «chiudi»). Codice: `kiosk/ui.rs` (`district_chips`, `plan_panel`), `Kiosk::selected`. Prova: partite scriptate `Forna3_gira`, `Rocco3` (borghi visibili, nessuna scheda sovrapposta); 37 target verdi. Da fare: playtest vero con la finestra in primo piano; touch (il badge è alto 66 px).
