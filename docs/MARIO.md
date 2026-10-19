# Mario (solo build nativa)

**Ricollegato il 2026-10-16.** Si apre con **Ctrl+Maiusc+M** (combinazione nascosta; il tasto Start del pad non lo apre più dal 2026-10-17) solo a mondo fermo: nel titolo o nella pausa della partita (non nel tutorial). Mentre Mario è attivo il mondo parte fermo e il pannello permette 0x, 1x, 5x, 10x, 30x, 60x (la pausa del gioco non scade), la UI del gioco è nascosta e la camera della mappa è sospesa. Dei vecchi attori restano solo le squadre (collisione e saluto con F / L3); persone e auto non esistono più. Non provato a schermo (solo test e build).

Durante la pausa, **Ctrl+Maiusc+M** apre Mario.
Parte davanti all'ingresso della rocca, usando posizione, orientamento e scala
di `island.json`; negli scenari senza rocca usa la strada più vicina alla vista.
Ctrl+Maiusc+M o Esc torna al piano e ripristina camera, viewport e pausa.

| Azione | Tastiera | DualSense |
|---|---|---|
| Movimento | WASD | Stick sinistro |
| Camera | Frecce | Stick destro |
| Zoom camera | Rotella, Q avvicina / E allontana | L1 avvicina / R1 allontana |
| Wing Cap (attiva / disattiva) | V | ○ |
| Salto / nuoto / boost durante il volo | Spazio | × |
| Pugno / calcio | J | □ |
| Accovacciarsi / schianto a terra | Shift | L2 o R2 |
| Scegliere posizione sulla mappa | R | △ |

Il pannello mostra il controller rilevato, energia e ora della simulazione.
La camera può orbitare fino alla vista quasi verticale e variare distanza da
12 a 240 m.
Il Wing Cap resta attivo finché lo disattivi con V / ○; alla morte o al
riavvio Mario riparte senza. Un triplo salto in corsa avvia il volo, guidato
con WASD / stick sinistro. Ogni pressione di Spazio / × durante il volo dà un
impulso di velocità (+40 unità
SM64, massimo 120); a terra resta il salto. Ali e fisica del volo sono gestite da libsm64.

Con R / △ o «Scegli posizione sulla mappa» si apre una vista verticale dell’intera
mappa e Mario resta fermo durante la scelta. Muovi il mouse o WASD / stick sinistro
per spostare il mirino; frecce / stick destro spostano la vista. Rotella, Q/E e L1/R1
regolano lo zoom. Un clic sulla mappa, Spazio / × o «Posiziona» conferma;
Esc, ○, R / △ o «Annulla» torna a Mario senza spostarlo. Puoi scegliere qualsiasi
punto, anche in mare; nei punti occupati si cerca terreno libero entro circa 21 m,
altrimenti la selezione resta aperta. Dopo la conferma si torna alla camera di Mario.
Decisione dell’utente: 2026-10-13.

Il tempo del gioco avanza a **×1**, attraverso `kiosk::step` e `Sim::tick`.
Una crisi o il fine partita chiude Mario e restituisce il controllo al gioco.
L'uscita aggiorna l'anteprima degli ordini, senza applicare il piano proposto
né consumare un'altra pausa. Le interazioni con persone e veicoli sono reazioni
visive temporanee: non modificano agenti, ordini o punteggi.

La collisione usa le mesh effettivamente disegnate: terreno, edifici,
strade, rocca e torre; persone, auto e mezzi operativi aggiornano i collisori
a 30 Hz. Gli alberi sono solo visivi e non bloccano Mario né la camera.
Il mare usa la stessa quota/onda della scena, con fondale sommerso
per consentire il nuoto. Il fuoco attivo e gli edifici ancora in fiamme
provocano danni e la reazione di bruciatura di libsm64. Il modello si ricrea
alla morte o alla conferma del riposizionamento.

Avvio: `cargo run -p game --release`.
La ROM originale USA `.z64` viene letta solo all'apertura di Mario da
`data/mario64/Super Mario 64 (USA).z64`. `MARIO64_ROM` permette un altro percorso.
La ROM viene verificata prima di passarla alla libreria C; non è incorporata
nel programma. Il codice vendorizzato e la sua revisione sono documentati in
`crates/libsm64-native/vendor/README.md`.

La build wasm esclude modulo, libreria nativa, controlli e ROM. Nessun download
è richiesto in compilazione: la sorgente libsm64 è vendorizzata e `cc` la compila
solo per la dipendenza nativa.

Verifiche: `cargo test -p libsm64-native -- --nocapture` (fisica, nuoto, danni,
ownership e cleanup con ROM locale); `cargo test -p game --release mario`
(coordinate, camera e clock ×1/ritorno alla pausa); `cargo check -p game
--target wasm32-unknown-unknown` (nessuna dipendenza libsm64).

Screenshot riproducibile della modalità: `KIOSK_SNAP=out/mario
KIOSK_SNAP_SPEC='name=rocca,phase=pianifica,mario=1' cargo run -p game
--release`. Il campo `mario=1` è solo nativo; il programma salva il PNG e termina.
