# Isola di Roccavento — lista di lavoro

Va aggiornata a ogni iterazione, nello stesso commit del lavoro (vedi `CLAUDE.md`). Le voci chiuse si cancellano: la storia è in git. Le fasi sono descritte in `docs/03-VERIFICHE.md`.

## Fase corrente: A (territorio con valico) e B (taratura dello spegnimento), in parallelo

**Prossimi checkpoint:** l'utente approva territorio e livelli (A) e sceglie i valori dello spegnimento (B). Poi si passa alla fase C.

### A. Territorio con valico (sessione del territorio)

- [ ] mappe del fuoco dei tre livelli, semi 1–3, senza ordini e con il piano ovvio; tabella con borghi colpiti, orari ed ettari (`docs/factory/valico/`);
- [ ] riscrivere i testi `about` dei livelli in `game.json` per il valico;
- [ ] decidere con l'utente se `prove.json` resta o se si tengono solo i tre livelli (lo usano molti test ed esempi);
- [ ] con lo spegnimento attuale ogni innesco entro 800 m dalla caserma si spegne se le autobotti ci vanno per prime, quindi l'ovest non entra in gioco: decidere insieme a B, oppure spostare gli inneschi;
- [ ] `tools/factory/cases.py` non riproduce più `game.json`/`prove.json`: aggiornarlo o dichiararlo superato; `tools/factory/island.py` è obsoleto (da rimuovere?).

### B. Taratura dello spegnimento (sessione dello spegnimento)

- [x] 2026-10-13 banco di prova `rocca_banco` (`tools/factory/banco.py`: terreno piatto, un solo combustibile, paesi veri; `examples/spegnimento.rs --cfg linea:acqua:portata`, `ROCCA_SCENARIO=rocca_banco`). Ettari bruciati a 180 min, media di 30 partite: senza spegnimento 187; 1:1:1 186; 3:30:3 165; 3:30:5 147; 3:30:10 108 (−42 %); 3:30:20 84; 3:30:30 51. La linea ×1, ×3 o ×10 non cambia nulla; l'acqua per cella satura oltre ×10; conta la portata dei getti. Raggio dei getti = 60 m × portata (×10 = 600 m, ×20 = 1200 m). **Scelta dell'utente 2026-10-13: portata ×20**; nel gioco `game::SUPPRESSION` = linea 3, acqua 30, portata 20;
- [ ] **le squadre devono poter attaccare il fronte, non solo stazionare davanti alle case** (richiesta dell'utente 2026-10-13): oggi il coordinatore le manda alle postazioni dei quartieri; servono compiti d'attacco al fronte (autobotti e squadre a mano sul perimetro del fuoco), da misurare sul banco `rocca_banco` come ettari bruciati; il banco attuale misura solo il comportamento nel gioco, non il massimo possibile;
- [ ] misurare la portata ×20 anche sui tre livelli; rifare `strategie_civili` («evacua tutti» non deve dominare).

### C. Ritaratura headless (dopo A e B)

Tutti i numeri raccolti finora valgono per il territorio precedente al valico e per lo spegnimento ×3.

- [ ] `examples/sindaca.rs`: bilanciamento con la sindaca, soglia `lead_s` (oggi 20 min), soglie delle stelle `score::{PERSONE, CASE, TEMPISMO, MISURA}`;
- [ ] `strategie_civili` (`PRIORITA=sbagliate`): «evacua tutti» non deve dominare;
- [ ] capacità di uscita (`USCITA`, `examples/partenze.rs`); oggi è 6 famiglie al minuto, attiva ma **da confermare dall'utente**. Lo script di riassunto di prima è stato cancellato con lo storico: rifarlo in `tools/` se serve;
- [ ] `examples/previsione.rs`: affidabilità a +1 h e +3 h, e tempi;
- [ ] quanti momenti ×1 per partita (sindaca, vento, `Vicino`) e durata reale per livello: aggiungere il conteggio a `examples/sindaca.rs`;
- [ ] riscrivere il test «invertire le priorità cambia postazioni ed esiti» (rimosso il 2026-10-13 su richiesta dell'utente, fallisce con lo spegnimento attuale) e un test del giudizio sugli ordini (`Game::order_review`);
- [ ] stelle assolute: dove nessuna casa si salva anche il piano giusto prende 1 in Case; Tempismo è quasi pieno per chi preallerta presto; proposta: Misura «—» se non c'è stata nessuna evacuazione (oggi 5 stelle senza fare nulla);
- [ ] il punteggio non conta le famiglie colte dopo la partenza (`rocca::score`): oggi zero;
- [ ] difesa delle strade: misurare prima l'effetto sulla percorribilità (spec).

### D. Playtest e rifinitura UX

- [ ] da provare a mano: trascinamento nel piano a colonne, conferma dell'evacuazione con le vie di fuga, ritmo dei ×1 automatici, zoom con il trackpad, Esc nel tutorial, scrittura del nome dopo un click fuori;
- [ ] previsione, domande per l'utente: etichetta «tra 1 ora» sotto il cartellino di un borgo; nessuna previsione a Facile (vento fisso); resta 30 min dopo il giro (`SHOWN_AFTER_SHIFT_S`); versione «con interventi»;
- [ ] a 1280×720 la card di apertura è più alta dello schermo e copre «Menu»;
- [ ] «Segui il fuoco» non si attiva più a mano (è solo automatico): va bene?
- [ ] i personaggi si notano durante un ×1? La crisi si nota nella barra laterale?
- [ ] Preallerta ed Evacua poco distinguibili sulla mappa; servono indicatori delle evacuazioni;
- [ ] il «Perché» non spiega l'effetto dei mezzi né le famiglie che non partono nonostante l'ordine;
- [ ] scheda di osservazione per chi assiste ai playtest (durata, prima azione, compromesso spiegato, blocchi, domande finali);
- [ ] playtest con studenti e colleghi che non conoscono il progetto.

### E. Pronto per il chiosco

- [ ] FPS sulla macchina del chiosco (`KIOSK_FPS=1`) e scelta del livello grafico; il livello «Bassa» ha la vegetazione troppo rada? Il costo GPU delle nuvole non è misurato;
- [ ] cambio del livello grafico dal menu operatore non provato a schermo;
- [ ] prova di stabilità: molte partite di fila, «Riprova» e «Nuova partita», nessun effetto residuo dopo il riavvio, memoria stabile;
- [ ] versione nativa o browser; build web con le nuove meccaniche non provata (`scripts/build_web.sh`); Mario sul web non visto a schermo;
- [ ] touchscreen sì o no; il nome lo scrive il giocatore o l'operatore;
- [ ] sul chiosco cancellare i vecchi `~/.rocca_ventosa/classifica2.json` e `ultimi.json`, non più letti;
- [ ] `.mcp.json` avvia un server MCP «spotorno» del vecchio scenario: serve ancora?

## Test rotti noti

- `abm/tests/incident_gaps.rs`: 5 test pensati per un territorio senza mare;
- `rocca/tests/messaggeri.rs::the_teams_leave_a_district_the_fire_reaches` (falliva già a `14357e6`);
- `game models::tests::the_island_models_stand_on_the_ground` (punto più basso della rocca a −2,6 m).

## Decisioni in vigore (con data)

Le regole complete sono in `docs/01-GIOCO.md`. Qui le decisioni recenti, per orientarsi.

- 2026-10-13: niente pause né «Decidi». Gli ordini valgono subito; ×1 automatico solo per autorizzazione della sindaca, cambio di vento e fuoco vicino a un borgo non avvisato. Il testo del ×1 è generato.
- 2026-10-13: «Evacua» solo con l'autorizzazione della sindaca (fronte entro 20 min); prima mostra le vie di fuga e chiede conferma (×1, 20 s).
- 2026-10-13: piano a tre colonne trascinabili; classifica dei migliori del giorno per livello; finale semplificato a stelle.
- 2026-10-13: previsione stocastica a +1 h e +3 h, isocrone al 50 %, senza interventi.
- 2026-10-13: capacità di uscita a 6 famiglie al minuto (versione B: chi vede il fuoco parte comunque). Attiva, da confermare.
- 2026-10-13: il territorio è l'isola con il valico; nuovi livelli Facile, Medio ed Esperto.
- 2026-10-13: «evacua appena autorizzato» può vincere ai punti; il suo costo resta nella stella Misura.
- 2026-10-13: riscritte da zero le istruzioni (`CLAUDE.md`, `docs/01–03`) e cancellato lo storico; nuove fasi A–E.
- 2026-10-12: 4 pattuglie portano gli avvisi porta a porta (1 famiglia al minuto ciascuna, allerta sul telefono attiva).
- 2026-10-12: schermo del chiosco di almeno 1080p; il tutorial ha solo «Avanti» e «Salta».
- 2026-10-08: la difesa delle case riduce l'esposizione reale, non un punteggio.
