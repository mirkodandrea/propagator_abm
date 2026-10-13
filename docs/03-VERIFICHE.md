# 03 — Fasi e verifiche

Le fasi vanno fatte **una alla volta**: ognuna finisce con un **checkpoint umano**, e la successiva non parte senza conferma dell'utente. Il lavoro dentro la fase si tiene in `docs/TODO.md`. Le prove si eseguono sempre con gli strumenti del repository e con seed salvati.

## Fasi (dal 2026-10-13)

| Fase | Cosa | Si chiude con | Checkpoint |
|---|---|---|---|
| **A. Territorio con valico** | Il fuoco dei tre livelli deve coinvolgere anche i borghi a ovest, con finestre d'intervento utili. Si lavora su inneschi, vento, valico e vegetazione (`02-TERRITORIO.md`). Testi `about` dei livelli. | Mappe del fuoco dei tre livelli (semi 1–3) senza ordini e con il piano ovvio. Tabella: borghi colpiti, orari di arrivo, ettari. | L'utente approva territorio e livelli. |
| **B. Taratura dello spegnimento** | `SUPPRESSION` in `rocca::game` (linea, acqua per cella, portata) e `abm::suppression::ENGINE_CELLS`, misurati sul banco `rocca_banco` (`examples/spegnimento.rs`). La difesa deve contare senza rendere i borghi invulnerabili. Un innesco vicino alla caserma non deve spegnersi sempre. | Tabella: case in fiamme e celle bruciate per configurazione, sul banco e sui tre livelli. | L'utente sceglie i valori. |
| **C. Ritaratura headless** | Tutta la batteria qui sotto sul territorio e con lo spegnimento approvati: sindaca, stelle, capacità di uscita, previsione, momenti ×1, priorità invertite. Riscrivere i test di `fase3.rs` che non valgono più. | Una pagina con le tabelle e le proposte: soglie delle stelle, `lead_s`, `exit_per_min`. | L'utente approva il bilanciamento. |
| **D. Playtest e rifinitura UX** | Partite vere con persone che non conoscono il progetto. Screenshot di tutti gli stati con `KIOSK_SNAP`. Correzioni di comprensibilità: piano a colonne, conferma dell'evacuazione, ritmo dei ×1, trackpad, previsione. | Resoconto del playtest: durata reale, prima azione, compromesso spiegato dal giocatore, blocchi. | L'utente decide le correzioni. |
| **E. Pronto per il chiosco** | Macchina reale: FPS e livello grafico, prova di stabilità (molte partite di fila, riavvii), funzionamento offline, riavvio manuale, scelta tra versione nativa e web, touchscreen o mouse. | Lista di controllo firmata per l'evento. | Ultimo checkpoint. |

Le fasi A e B si possono condurre in parallelo, in sessioni diverse, perché toccano parti diverse. La fase C le aspetta entrambe.

**Consegna di ogni fase: al massimo una pagina in italiano, in quattro sezioni.**
1. Realizzato: file e moduli toccati.
2. Evidenza: comandi per riprodurre, più una tabella o un'immagine.
3. Non risolto: rischi e test falliti.
4. Una sola proposta per il passo successivo.

Una fase non è finita perché «compila»: i suoi esempi riproducibili devono funzionare.

## Batteria di prove headless

Eseguire dalla radice del repository, in release (`cargo build --release -p rocca --examples`). Per provare un territorio diverso: `ROCCA_SCENARIO=<id>`.

| Prova | Comando | Cosa guardare |
|---|---|---|
| Test del motore | `cargo test --release -p rocca -p abm -p fire` | Tutti verdi, oppure fallimenti noti e annotati in TODO. |
| Bilanciamento con la sindaca | `target/release/examples/sindaca --semi 1,2,3` (`GREZZI=f.csv` per i dati grezzi) | «Evacua appena autorizzato» non deve vincere per ovvietà: il dilemma vero è chi preallertare. Stelle medie per piano. Autorizzazioni non in ritardo sul fuoco. |
| Strategie civili | `target/release/examples/strategie_civili` (`PRIORITA=sbagliate`, `MESSAGGERI=…`) | «Evacua tutti» deve perdere più di 20 punti contro «solo i minacciati» in una parte degli incendi. |
| Capacità di uscita | `USCITA=<n> target/release/examples/sindaca …` e `target/release/examples/partenze` | La capacità non deve insegnare che evacuare è peggio che preallertare. Code visibili. |
| Previsione | `target/release/examples/previsione` | Quota del bruciato reale presa a +1 h e +3 h, falsi allarmi, tempi in ms. |
| Spegnimento | `ROCCA_SCENARIO=rocca_banco target/release/examples/spegnimento --cfg linea:acqua:portata` | Case in fiamme e celle bruciate per configurazione. |
| Partita singola / A/B | `target/release/rocca <caso> --priorita A,B --b-priorita B,A [--civili "0:X=preallerta"]` | Stesso seme: invertire le priorità cambia postazioni ed esiti, almeno in alcuni casi. |
| Durata reale | `examples/sindaca` (colonna «minuti reali»), da aggiornare con i momenti ×1 nuovi | Durata stimata per livello. Quanti ×1 per partita. |

## Verifiche che non si possono saltare

- **Effetti reali:** invertire le priorità deve talvolta cambiare gli esiti, non solo le icone. Un piano migliore deve poter servire, senza riuscire sempre.
- **Preallerta ≠ evacuazione:** effetti distinti sui civili, misurati.
- **Nessuna strategia semplice domina:** né «evacua tutti» né «evacua appena autorizzato» deve bastare per avere 5 stelle ovunque.
- **Anteprima senza effetti:** a ×1 il piano proposto non muove nulla prima della conferma; al commit viene rivalidato.
- **Stessi criteri ovunque:** grafica, debrief e stelle usano gli stessi fatti. Distinguere fatti simulati e proxy.
- **Riproducibilità:** stessi seme, stato, ordini e tempi simulati danno lo stesso risultato. Gli effetti dipendono dal tempo simulato, non dal numero di update. Gli inneschi devono attecchire davvero.
- **Strade e squadre:** percorsi raggiungibili, ETA plausibili, nessun A* a ogni tick, aree sicure esistenti e raggiungibili.
- **Nessuna informazione dal futuro:** crisi, sindaca e previsione usano solo lo stato presente e il bollettino.
- **Grafica** (con `KIOSK_SNAP`, guardando le immagini): coordinate, winding delle mesh, strade drappeggiate, segni visibili sopra la vegetazione, effetti puliti dopo il riavvio.
- **Chiosco:** nessun timeout per inattività; avvio e riavvio manuali affidabili; funziona offline e senza audio.
