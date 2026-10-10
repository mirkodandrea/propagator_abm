# Usabilità, schermata per schermata (2026-10-10)

Richiesta dell'utente: «esamina ogni schermata e migliora l'usabilità e la user experience». Ho rivisto le dieci schermate della partita scriptata alla finestra del playtest 6 (1745×824, 1 pixel per punto), più la legenda aperta. Le correzioni riguardano solo l'interfaccia, più una frase del coordinatore. Fisica, punteggio e regole sono invariati.

**Prova:** `KIOSK_CASE=Forna3_gira KIOSK_SPEED=300 KIOSK_WINDOWED=1 KIOSK_SIZE=1745x824 KIOSK_SCALE=1 KIOSK_SHOT=<dir> target/release/game` (con `KIOSK_LEGEND=1` per la legenda). Test: `cargo test --release --workspace`, tutti verdi; anche la build web compila.

| Schermata | Difetto (fonte) | Correzione |
|---|---|---|
| Finale | Pannello più alto della finestra, titolo sotto la barra, «Riprova» e «Cambia difficoltà» irraggiungibili (playtest 6, bloccante) | Il pannello sta tra la barra e il fondo, a sinistra del punteggio. La parte centrale scorre (barra sempre visibile); nota «Nella realtà…» e pulsanti restano fissi in fondo (`prima_finale` → `dopo_finale`) |
| Finale | Con più di 60 case colpite la barra sconfinava nella colonna «evacuate» | La barra si ferma alla sua colonna; il numero dice il resto |
| Finale, punteggio | «8 salvate su 11» non diceva 11 di cosa (playtest 6) | «Senza ordini il fuoco avrebbe colto in casa 49 famiglie: tu ne hai salvate 12», lo stesso per le case, e «Punti: 70 % famiglie, 30 % case» |
| Pronto / pianifica | Fuoco invisibile all'inizio (playtest 5 e 6) | Anello pulsante e cartello «Il fuoco è partito qui» sull'innesco, per i primi 30 min simulati (`dopo_pronto`) |
| Scheda del borgo | Pulsanti grigi senza motivo (playtest 6) | «Gli ordini si sbloccano con «Inizia la partita»» (o «…con «Ho capito»») |
| Scheda del borgo | Preallerta ed Evacua poco distinte (playtest 5 e 6) | Due righe come le fa il modello: «Preallerta: si preparano a casa e qualcuno parte da sé; un'evacuazione dopo li raggiunge prima. Evacua: partono tutti adesso.» (`dopo_scheda_legenda`) |
| Mappa | Etichette dei mezzi nascoste sotto i badge dei borghi (playtest 6) | Un'etichetta sotto un badge va sotto di esso, oppure di fianco se lì c'è un pannello; due etichette non si sovrappongono (`dopo_mezzi`) |
| Crisi | Crisi scaduta senza essere vista (playtest 6) | Banda arancione sotto la barra: «DECISIONE URGENTE · 23 s · scegli in basso a sinistra», lampeggiante negli ultimi 8 s. Nel pannello: titolo, «se non scegli resta il piano attuale», «Con questa scelta i mezzi si spostano così:» (`dopo_crisi`) |
| Legenda | Ultima riga tagliata, nessuno scorrimento (playtest 6) | Due colonne sotto la barra in alto (non copre più vento e orologio); a 824 px entra tutta (`dopo_crisi_legenda`) |
| Aggiornamenti | «mezzi mancanti: Le Ghiande: nessun mezzo rimasto dopo le priorità più alte» | «Le Ghiande: nessun mezzo libero, sono già tutti dove la priorità è più alta», in arancione (`coordinator.rs`, `game.rs`) |
| Come si gioca | «hai 3 pause, non una dopo l'altra» poco chiaro (playtest 6) | «hai 3 pause in tutto, e tra una e l'altra devono passare 15 minuti di gioco» |
| Intro | Le tre tessere avevano altezze diverse | Altezza uguale |

**File:** `crates/game/src/kiosk/ui.rs`, `crates/rocca/src/coordinator.rs` (solo testo), `crates/rocca/src/game.rs` (prefisso del registro).

**Incognite:**
- Non verificato con un mouse reale né nel browser: rotella sul debrief e clic sui badge restano da provare al chiosco.
- La scheda aperta copre la legenda se sono aperte tutte e due (sono entrambe scelte del giocatore).
- Effetto dei mezzi nel «Perché» (playtest 6, P1 e P2 con case uguali): non affrontato, perché riguarda il modello e il racconto, non la schermata.

**Proposta:** rifare il playtest automatico nel browser in primo piano (playtest 7), per verificare finale, crisi e legenda con un visitatore.
