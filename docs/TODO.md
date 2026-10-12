# Rocca Ventosa — todo list

Aggiornare a ogni iterazione (vedi `CLAUDE.md`). Fase corrente in cima.

## Stato

- **2026-10-12, playtest dei colleghi** (commenti incollati dall'utente, 18 punti). Decisioni dell'utente (2026-10-12): **il nome del giocatore si chiede all'inizio, non più le iniziali, così i personaggi si rivolgono a lui**; «procedi con i fix semplici».
  - [x] 2026-10-12 nome nell'intro («Come ti chiami?»): i livelli si attivano solo dopo aver scritto il nome. Lo usano la volontaria nel tutorial, i consigli in pausa, la sindaca e il meteo nelle battute (`characters::to`) e il titolo del finale. La classifica salva il nome da sola (`score::Entry::name`; le classifiche già salvate con `initials` si leggono ancora). «Nuova partita» dell'operatore cancella il nome; la partita scriptata gioca come «Giulia» e non scrive la classifica (`board::save`);
  - [x] 2026-10-12 cursore a mano e bordo bianco immediato su tutti i pulsanti (`interact_cursor`, `animation_time` 0, `icons::pointing`); pulsanti dei livelli su tutta la larghezza, testo centrato;
  - [x] 2026-10-12 finale: solo «Riprova», che torna all'intro e alla scelta del livello;
  - [x] 2026-10-12 scheda: Difendi, Preallerta ed Evacua su una riga, icona sopra e testo sotto (`icons::tall_button`); «Difendi» ripremuto toglie la difesa; «metti per primo» sotto; le tre spiegazioni sempre in fondo, con più spazio;
  - [x] 2026-10-12 tempo di evacuazione: la scheda dice «~N min per prepararsi, poi ~M min di strada» (`Kiosk::readying`, dai `prep_time_min` della popolazione meno la preparazione già fatta con la preallerta); il testo di «Evacua» non dice più «partono subito» e spiega che qualcuno aspetta (perché non si arriva al 100 %);
  - [x] 2026-10-12 etichette dei mezzi: sempre un'asta del colore del mezzo fino a un punto dove il mezzo si trova; tolta la riga bianca che compariva solo quando l'etichetta veniva spostata;
  - [x] 2026-10-12 legenda sfoltita: solo ciò che è sulla mappa o nelle schede, più una riga sui tempi di evacuazione;
  - [x] 2026-10-12 «Ho capito» allineato (`tutorial.rs`, nel commit `6dfa637`); pannelli laterali uniformi con la barra laterale dell'altra sessione (`6dfa637`);
  - [ ] decidere con l'utente: **distribuzione dei mezzi** (punti 6, 16, 17). Il «2-1-0» è la regola 2 del coordinatore; i paesi non ancora minacciati non ricevono mezzi (regola 1). Spiegarlo meglio, distribuire a rotazione o far scegliere quanti mezzi per paese? Prima l'A/B «concentrare contro distribuire»;
  - [ ] messaggi dei personaggi più visibili (ingrandimento al cambio, bip facoltativo) e storico delle battute (punto 4);
  - [ ] riprodurre headless «senza ordini meglio che con ordini» in un paese (punto 14): le squadre bagnano la vegetazione e il fuoco cambia strada? Poi raccontarlo nel «Perché» o correggere;
  - [ ] freccia del vento più visibile sul territorio (punto 15); primi piani automatici sull'area colpita con `Kiosk::shot` (punto 18);
  - [x] 2026-10-12 caricamento nel browser: barra di avanzamento del download del `.wasm` (`web/index.html::download`, il sed di `scripts/build_web.sh` aggiornato), poi «Avvio della simulazione…». Non provato in un browser (serve `scripts/build_web.sh`);
  - [ ] chiedere ai colleghi se hanno giocato nel browser o nativo (punto 1, «pesante»);
  - [x] 2026-10-12 con la barra laterale, i badge dei paesi dietro la barra avevano una linea che attraversava la barra: linee ritagliate sulla mappa (`bf744a0`);
  - [x] 2026-10-12 schermata iniziale ridisegnata (`kiosk/ui.rs::intro`): cornice con ombra e bordo ambra, separatori, squadra come «pillole» con ritratto, livelli come tre carte verde/ambra/rosso cliccabili per intero (disattivate finché manca il nome);
  - [ ] serve una tastiera al chiosco per il nome (o lo scrive l'operatore).

- **2026-10-12, barra laterale** (richiesta dell'utente: «tutto nella barra tranne la scheda del paese quando si clicca, e il fumetto che può rimanere nella parte opposta, isolato»).
  - [x] 2026-10-12 `kiosk/ui.rs::sidebar`: a destra, larga 400 px, dall'alto: vento, ora con la barra delle ore, stato (in pausa / il tempo scorre / DECISIONE URGENTE con i secondi, lampeggiante alla fine), il pulsante del tempo oppure la scelta della crisi, il piano (righe cliccabili, apre la scheda), il coordinatore / i mezzi, gli ultimi aggiornamenti, la legenda (chiusa); in fondo la vista (zoom e vista iniziale). Sulla mappa restano le etichette dei paesi, la scheda aperta, le etichette dei mezzi, il segno del fuoco e il fumetto in basso a sinistra (alla crisi: «Decidi nella barra a destra»). La camera disegna la mappa solo a sinistra della barra (`Camera::viewport`, `kiosk/view.rs`) e rifà l'inquadratura iniziale sulla nuova misura. Tolti barra in alto (resta solo nel finale), bande «IN PAUSA» / «DECISIONE URGENTE», pannelli sparsi. Il tutorial punta alle sezioni della barra. Prova: stesso comando `KIOSK_SHOT` a 1920×1080;
  - [ ] provarlo con una persona: si nota la crisi nella barra? la lista del piano basta o serve trascinare per riordinare?

- **2026-10-12, mini-tutorial a inizio partita** (richiesta dell'utente). Decisioni dell'utente (2026-10-12): **lo schermo del chiosco è almeno 1080p, il vincolo 900×600 cade**; il tutorial è **non interattivo**: solo «Avanti» e «Salta».
  - [x] 2026-10-12 `crates/game/src/kiosk/tutorial.rs`: cinque passi (fuoco e vento, paesi, scheda con Difendi/Preallerta/Evacua, piano, «Avvia»). La camera si sposta con un movimento graduale (`Kiosk::shot`, `kiosk/view.rs`), il resto dello schermo si scurisce e la volontaria spiega in un fumetto con una freccia. Parte solo per un nuovo visitatore (non a «Riprova» o «Cambia difficoltà»; di nuovo dopo «Nuova partita» dell'operatore); si spegne da F2 o con `KIOSK_TUTORIAL=0`. Prova: `KIOSK_SIZE=1920x1080 KIOSK_CASE=Coste2_gira KIOSK_SPEED=300 KIOSK_WINDOWED=1 KIOSK_SHOT=<dir> target/release/game` → `0_tour_0..4.png`;
  - [ ] provarlo nel browser con una persona: durata (5 passi), testi, se «Salta» viene usato subito;
  - [x] 2026-10-12 (approvato dall'utente) tolti i doppioni del tutorial: «Come si gioca» dall'intro (`how_steps`) e la frase della volontaria nei primi 10 minuti (`TUTORIAL_S`), che ora dà subito le notizie della partita; nell'intro «puoi mettere in pausa quando vuoi» → «puoi fermarlo poche volte per cambiare gli ordini» (le pause sono 3).

- **2026-10-12, riferimenti normativi letti** (richiesta dell'utente): Raccomandazioni AIB 2026 (GU n. 133, 11/6/2026), Codice di prevenzione incendi (DM 3/8/2015, testo coordinato 2021), Direttiva PCM 10/1/2020 sul DOS (GU n. 56, 5/3/2020). Non sono specifiche: servono come lessico e per ancorare i parametri. L'utente ha detto «ok» alla proposta «prima il punto 5, poi i punti 1–3».
  - [x] 2026-10-12 **punto 5 (preallerta ≠ evacuazione secondo il tempo di esodo del Codice M.3.4, rivelazione + allarme + pre-movimento + movimento): già così nel modello.** Un ordine creduto fa partire la preparazione (`block.order_response` → `action.prepare` → `prep_seconds`); la preallerta anticipa la preparazione in casa (`readied_s`) e accorcia il tempo d'allarme (`PREALERTED_ORDER_DELAY_S`). La partenza a T+40–58 degli «aspetta e vedi» con ordine a T+0 è la somma di canale (1,5–20 min) e preparazione (25 min ± 15 %), non un ordine che non fa preparare. Resta una questione di **taratura**, non di struttura: il Codice dà 1′30″–3′30″ per persone sveglie e pratiche dei luoghi che escono da un edificio, non per una famiglia che carica l'auto;
  - [x] 2026-10-12 **punto 1, versione semplice chiesta dall'utente** («basta "aiutaci a gestire questa emergenza" con i vari personaggi che lavorano con noi», niente battute): sottotitolo dell'intro «Aiutaci a gestire questa emergenza.», la volontaria chiede aiuto, sotto «La squadra con cui lavori» con i quattro ritratti e i nomi (`kiosk/ui.rs::intro`, `Who::short`);
  - [ ] ~~punto 1, ruolo reale~~ (superato dalla versione semplice): ~~il giocatore è il Sindaco~~ (2026-10-12, l'utente: la sindaca è già un personaggio) → il giocatore è **in sala operativa del Comune (COC), accanto alla sindaca**: propone priorità e ordini, lei li firma senza passaggi in più (l'ordinanza è sua per legge). Da sistemare anche `Who::Dos`, oggi «Il caposquadra dei vigili del fuoco»: per la Direttiva il DOS dirige le operazioni AIB, non è un caposquadra; il coordinatore è «DOS e Vigili del Fuoco». Unità: «Vigili del Fuoco: difendono le case» (ROS, Direttiva §3: vita e insediamenti sono prioritari e in capo al ROS), «Squadre antincendio boschivo: lavorano sul fronte». Risponde anche a «effetto dei mezzi non spiegato» (playtest 6);
  - [ ] **punto 2, bollettino incendi** nel briefing iniziale: vento, livello di pericolo, quartieri esposti (Raccomandazioni 2026, lettera a);
  - [ ] **punto 3, debrief come «rapporto di intervento»** (Direttiva §4h): tipo d'incendio, dove hanno lavorato le squadre, cosa ha funzionato; in fondo **una** norma di comportamento legata alla partita (stile «Io non rischio»);
  - [ ] punto 4, «concentrare le forze» (principio AIB, Direttiva All. A): prima un A/B a parità di seed, una priorità contro molte; solo se il modello lo premia almeno talvolta, consiglio nella guida e frase nel debrief;
  - [ ] punto 6, regola LACES per le postazioni: valide solo con via di fuga verso un'area sicura; evento visibile «squadra ritirata». Soglie possibili: civili 2,5 kW/m², soccorritori 3 kW/m² (Codice M.3.5, tab. M.3-2);
  - [ ] punto 7, crisi ×1 quando l'incendio diventa «complesso» (Direttiva §3): quartieri minacciati più di quanti le squadre possano coprire (fase 4);
  - [ ] idee per dopo il checkpoint 5: Canadair su richiesta (flotta di Stato via SOUP/COAU, razionata e dirottabile) come decisione di crisi; verifica dei rifugi come «luogo sicuro» (≤ 2,5 kW/m², capienza, Codice S.4.5.1); strade fuori norma per le autobotti (larghezza 3,5 m, pendenza ≤ 10 %, Codice S.9.5) come vincolo della Scenario Factory; mezzi aerei fermi di notte, se una partita arriva al tramonto (DA VERIFICARE);
  - da non mostrare al giocatore: kW/m², indici FWI/FFMC, ICS, radio TBT, procedure COAU.

- **2026-10-12, «Come sono andati i tuoi ordini» nel finale** (richiesta dell'utente: «ok, let's add words»; solo parole, il punteggio non cambia). `Game::order_review` e `Game::needless_evacuations` in `rocca`: per ogni borgo l'ordine civile giudicato da quanto si è avvicinato il fuoco in questa partita. Rosso: famiglie colte in casa (ordine mancante o tardivo). Ambra: evacuazione con il fuoco rimasto oltre `QUIET_FIRE_M` (1,5 km, «Forse bastava la preallerta»), oppure nessun ordine con il fuoco vicino. Verde: evacuazione prudente o in tempo, preallerta sufficiente. «Alle case» = minaccia alla porta ≥ `ALARMING`, la stessa soglia delle famiglie colte. Nel finale sopra «I borghi», le righe rosse per prime; il runner `rocca` le stampa. Test `the_review_judges_each_order_by_the_fire`; schermata `docs/fase5/img/fine_ordini.png` (`KIOSK_CASE=Rocco3`, 1920×1080).
  - [ ] il giudizio è col senno di poi; versione più giusta: confrontare l'ordine con la previsione disponibile in quel momento (la stessa della crisi «previsione»);
  - [ ] se serve: una riga anche per i mezzi in postazione dove il fuoco non è mai arrivato.
- **2026-10-12, playtest 8 (Haiku, strategie headless, dopo `c777919`)**: `docs/fase5/playtest8_strategie/report.md`, 19 numeri riverificati. Nuova ricetta: «preallerta tutti T+0, evacua tutti T+30 + mezzi sul borgo più vicino» ≥ «evacua tutti T+0» in 24/24 confronti (+39/+56/+47); Esperto 10–218, mai sopra ~220; Esperto S5 167 → 149 (probabile `c777919`).
  - [ ] decidere con l'utente: playtest headless con personaggi e screenshot (`KIOSK_AGENT`, click iniettati, tempo fermo tranne alle crisi ×1), proposta del 2026-10-12;
- **2026-10-10, playtest 7 (Haiku, strategie headless)**: `docs/fase5/playtest7_strategie/report.md`. Ricetta quasi ottimale «evacua tutti a T+0 + tutti i mezzi sul borgo più vicino al fuoco» (entro il 5 % del migliore); evacuare non costa nulla; Esperto quasi insensibile alle decisioni (91–299). Il runner `rocca` ora stampa il punteggio del chiosco.
- **2026-10-11, perché evacuare non costa (headless)**: `docs/fase5/intasamento/README.md`, esempi `intasamento` e `intasamento_mezzi`. Le code si formano (fino a 220 auto) ma a Castelvento, lontano dal fuoco: 0 famiglie travolte in 90 partite, mezzi rallentati al più di 1 min. «Preallerta tutti, evacua a T+30» ≥ «evacua tutti a T+0» in 18/18 confronti. Decisione dell'utente (2026-10-11): costo **reale** nel modello, interessato alle famiglie in coda travolte dal fuoco.
  - [x] 2026-10-11 mappare le uscite dei borghi (`docs/fase5/uscite/README.md`, esempi `uscite` e `uscita_condivisa`): i rifugi sono dentro i centri incombustibili (Castelvento, Pian dei Grilli) e il tratto in comune è dentro Castelvento. In 36 casi nessuna uscita in comune viene tagliata; anche dove l'accesso di un borgo è tagliato presto, 0 famiglie colte fuori e al massimo 15 s di calore su 90 letali. Nessun innesco attiva il costo: serve una modifica del territorio;
  - [x] 2026-10-11 l'utente sceglie di modificare il territorio invece dei rifugi: **layout 5** (`T4_L5`, non pubblicato), con San Rocco a 1 km lungo la SP 12 e la strada nella macchia (`Layout.verges`), poi la caserma all'ingresso nord-ovest. Resoconto in `docs/fase5/layout5/README.md`. Ritardare l'ordine ora costa (Rocco2: 692 → 392), ma nessuna famiglia viene travolta in coda e la coda di Castelvento non tocca San Rocco; la caserma fuori non cambia nulla. **La preallerta batte «evacua tutti a T+0» in 9 confronti su 9**;
  - [x] 2026-10-11 l'utente scarta le aree di attesa fuori dai centri («irrealistico per la protezione civile»), approva il fumo sul traffico e chiede vegetazione più vicina / paese più sparso. Fatto: `abm::traffic::smoke_flow` (portata ridotta sui tratti col fumo) e **layout 6** (`T4_L6`, non pubblicato: case sparse di Castelvento lungo la SP 12 nella macchia, San Rocco senza nucleo, `Layout.wooded_roads`), esempio `strategie_civili`. Resoconto in `docs/fase5/layout6/README.md`: su layout 4, 5 e 6, in 0 incendi «evacua tutti» perde più di 20 punti rispetto a «evacua i minacciati»; 0 famiglie colte in strada; al massimo 38 auto ferme. Il traffico non diventa mai il collo di bottiglia a questa scala;
  - [x] 2026-10-11 l'utente tiene il **layout 5** e chiede di provare lì gli ordini portati da squadre limitate. Fatto, spento di serie: `rocca::messengers`, `Game::messengers`, `Abm::tell_in_person`, `MESSAGGERI` in `strategie_civili`. Resoconto `docs/fase5/messaggeri/README.md`: con 4 squadre, 1 famiglia/min, allerta sul telefono e squadre in parti uguali, «evacua tutti» perde più di 20 punti rispetto a «evacua i minacciati» in 9/26, 12/28 e 12/26 incendi (semi 1–3). Con le squadre secondo le priorità il costo sparisce; Esperto resta insensibile (567);
  - [x] 2026-10-12 su richiesta dell'utente le squadre guidano davvero: `abm::drive::Drive` (le stesse regole dei mezzi: A* attorno al fuoco, nuovo calcolo ogni 60 s, rallentate dalle code con `Traffic::emergency_factor`, inversione immediata se il fuoco taglia il tratto davanti), squadre dirottate in viaggio verso un quartiere di priorità più alta solo se una strada ci arriva ancora, nessun quartiere con più squadre che famiglie da avvisare. Test `crates/rocca/tests/messaggeri.rs` (riproducibilità, viaggio guidato, dirottamento). `strategie_civili` gira in parallelo su tutti i core (5′49″ → 44″) e conta per le squadre famiglie avvisate, km, tagli e dirottamenti. Esito con `MESSAGGERI=4,2,1,1`, seme 1, 145 partite: 1 sola riga cambia rispetto al viaggio a tempo fisso (Piano3, «preallerta tutti»: 675 → 707), 0 tagli del fuoco davanti a una squadra; in Terra2_gira Pian dei Grilli e Le Ghiande sono irraggiungibili per tutta la partita;
  - [x] 2026-10-12 punti aperti chiusi (richiesta dell'utente). **Una sola regola di guida**: anche i mezzi a terra usano `abm::drive` (i campi `route`/`at_node`/`planned_at_s`/`route_to` di `Unit` diventano `Unit::drive`); le squadre AIB a piedi percorrono i sentieri a `CREW_SPEED` e, se nessuna strada è aperta, raggiungono il punto a piedi fuori strada, come prima. Correzione in `Drive::plan`: un mezzo arrivato che ha proseguito a piedi non viene riportato al nodo stradale a ogni nuovo calcolo. **Squadre esposte al fuoco** come i mezzi: calore accumulato sopra `IMPASSABLE`, persa a `BURNOVER_S`; porta a porta fisico (la squadra sta alla porta a cui bussa, salta le porte con minaccia ≥ `WORK_LIMIT`, lascia il quartiere se lo sono tutte o se lo è il punto in cui si trova); una squadra senza compiti torna alla sua stazione. Test: `crates/abm/tests/drive.rs` (fuoco dipinto con `ThreatField::paint`: inversione sul tratto tagliato e giro alternativo; quartiere circondato → nessuna strada) e `the_teams_leave_a_district_the_fire_reaches` (Coste2). Effetto dei mezzi sulla prova senza squadre: 12 righe su 145 cambiano (5 meglio, 7 peggio; colte in casa 1007 → 1010); **Coste2_gira, caso del chiosco, peggiora** («evacua tutti» 91 → 30 punti, colte in casa 20 → 22) perché i mezzi non proseguono più su un percorso che il fuoco ha tagliato. Con le squadre: 0 tagli, 0 quartieri lasciati, 0 squadre perse in 145 partite;
  - [x] 2026-10-12 l'utente ricorda l'obiettivo: **«evacua tutti» non deve essere la strategia migliore**. Rifatta la prova sul layout 5 (semi 1–3) con le squadre che guidano: tabella in `docs/fase5/messaggeri/README.md` («Aggiornamento 2026-10-12»), uscite in `docs/fase5/messaggeri/guida/`. «Evacua tutti» perde più di 20 punti in 6/10/7 incendi con A (prima 9/12/12), 16/17/16 con B, 0 con C (21–23 con priorità rovesciate);
  - [ ] decidere con l'utente quale variante portare nel gioco: A (parti uguali, a parità secondo le priorità, allerta sul telefono), B (senza allerta sul telefono: costo più forte), C (secondo le priorità: costa solo con priorità sbagliate);
  - [ ] decidere con l'utente: accettare il peggioramento di Coste2_gira (mezzi che non attraversano più una strada tagliata) o ritarare il caso;
  - [ ] decidere con l'utente: portare la variante A nel gioco? Squadre in parti uguali o secondo le priorità? Poi: riscegliere i livelli sul layout 5, pubblicarlo, mostrare l'avanzamento degli avvisi nelle schede;
  - [ ] il punteggio non conta le famiglie morte o intrappolate dopo la partenza (`rocca::score`): oggi 0, da correggere se un modello le produce;
  - [ ] decidere con l'utente: un ordine di evacuazione deve far preparare la famiglia almeno quanto una preallerta? Oggi `readied_s` cresce solo con la preallerta (`crates/abm/src/lib.rs:1368`): le famiglie «aspetta e vedi» con un ordine a T+0 partono a T+40–58;
  - [ ] se il layout 5 viene adottato: riscegliere i livelli (`Forna3`/`Forna3_gira` bruciano 1 ha), pubblicare e adattare i test;
  - [ ] capire perché su Coste2_gira, dopo T+34, i mezzi non raggiungono i borghi diversi da Le Ghiande.
  - [x] 2026-10-11 decidere con l'utente: costo reale dell'evacuazione (code, famiglie in strada esposte) e/o casi in cui la decisione arriva dopo T+0; rivedere `Coste2_gira` come Esperto.
- **2026-10-10, usabilità schermata per schermata** (richiesta dell'utente): finale che sta nella finestra con pulsanti fissi, segno «Il fuoco è partito qui», mezzi non più nascosti dai badge, banda di crisi, legenda su due colonne, punteggio spiegato, testi del coordinatore e della guida. Prove e tabella in `docs/fase5/usabilita/README.md`. Da verificare con un playtest nel browser (playtest 7).
- **2026-10-09, nuclei più grandi: layout 4 `t4_paese4` adottato** (richiesta e «fai tu» dell'utente): sei distretti, 538 famiglie, borghi organici, nuovi inneschi e casi del kiosk (`Forna3_gira`, `Coste2_gira`, `Rocco3`). Test adattati, 37 target verdi. Note e incognite in `docs/fase5/ingrandimento_nuclei.md`; playtest 5: browser fallito (scheda nascosta), solo partite scriptate native; schede e debrief da sistemare, playtest vero da rifare. Committato e inviato con push il 2026-10-10 (`900351c`).
- **Fase corrente: fase 5, iterazioni 5b e 5c chiuse** (2026-10-09, inviate con push fino ad `a16a043`):
  - 5b: punti 1–10 (`docs/fase5/iterazione5b.md`), compreso il playtest automatico 3 (`docs/fase5/playtest_gpt3/README.md`);
  - 5c: punti 1–5 del playtest 3 (`docs/fase5/iterazione5c.md`).
- **Prossima azione: checkpoint 5, umano** (`docs/fase5/README.md`). Una partita al chiosco per caso (Coste2_gira, Piano2, Borgo2), con prima azione e debrief. Non procedere senza conferma dell'utente; dopo viene la fase 6 (playtest).
- **Playtest 6 (Sonnet 5.5, 2026-10-10, build `3ff28cd`)** fatto, nel browser in primo piano: `docs/fase5/playtest6/report.md`. Partite: Facile 742, Facile 869 (evacuare presto), Medio 360; Esperto solo avviato. Da correggere, verificato sugli screenshot:
  - [x] 2026-10-10 (`docs/fase5/usabilita/README.md`) **debrief più alto della finestra** (824 px CSS): barra in alto sopra il titolo, «Perché» tagliato, pulsanti per rigiocare o cambiare difficoltà irraggiungibili (`p1-06-finale-tagliato.jpg`). Bloccante: tra una partita e l'altra Giulia ha ricaricato la pagina;
  - [x] 2026-10-10 legenda tagliata in basso (ora su due colonne sotto la barra) (`p4-01`);
  - [x] 2026-10-10 il badge di Le Terrazze copre l'etichetta A2 (`p3-02`);
  - [x] 2026-10-10 (banda «DECISIONE URGENTE» con i secondi) crisi persa (P3, 14:45): scaduta senza che Giulia la vedesse. Molto dipende dalla lentezza dello strumento, ma conviene renderla più evidente;
  - [x] 2026-10-10 dal playtest 5: fuoco poco visibile all'inizio (segno sull'innesco), pulsanti grigi senza spiegazione, Preallerta ed Evacua poco distinte (due righe nella scheda);
  - [ ] effetto dei mezzi non spiegato nel «Perché» (P1 e P2: case identiche, 59/76): riguarda modello e racconto, non la schermata.
- **Playtest 5 (Sonnet 5.5)** fatto: `docs/fase5/playtest_sonnet/report.md`. Il punteggio va rivisto, perché la classifica unica non è coerente con un punteggio relativo all'incendio.
- **Tempo reale, pause, punteggio e classifica** fatti (`docs/fase5/tempo_reale.md`); nove difetti trovati giocando, elencati lì e nella sezione «Tempo reale» della fase 5.
- **Playtest 4** (Giulia, prima volta, Haiku 5.5): fatto, difetti da decidere con l'utente (`docs/fase5/playtest_haiku/`, sezione «Playtest 4» della fase 5).
- **Ancora aperti** (vedi «Dal playtest 3» in fondo alla fase 5):
  - ~~layout a 900×600~~ (2026-10-12, l'utente: lo schermo del chiosco è almeno 1080p, vincolo tolto);
  - FPS nel browser a finestra in primo piano;
  - apertura delle schede al tocco;
  - durata di Piano2 (1,8 min di esecuzione);
  - crisi del mezzo perso con la nuova scelta, mai vista in una partita vera.
- **Playtest automatico** (stessi quattro personaggi):
  - copiare `docs/fase5/playtest_gpt3/prompt/` fuori dal repository e lanciare `run.sh` (circa 1 h);
  - servire la build web su `localhost:8765`: `./scripts/build_web.sh` e `python3 -m http.server 8765 --directory target/web`;
  - i resoconti vanno riassunti all'utente in italiano.
- **Per riprendere:**
  - **Comandi:**
    - `cargo test --release --workspace` (37 target verdi);
    - `target/release/rocca <caso> --priorita A,B --b-priorita B,A`;
    - esempi `rocca`: `ab_sweep`, `crisi`, `civili`, `porta`, `durata` (minuti reali per ritmo), `debrief` (righe del «Perché»), `preallerta_vs_evacua` (semi);
    - kiosk: `KIOSK_CASE=Coste2_gira KIOSK_SPEED=300 KIOSK_WINDOWED=1 KIOSK_SHOT=<dir> target/release/game` (partita scriptata fino a «Altro incendio»);
    - kiosk: F2 apre la barra operatore; `KIOSK_FPS=1` scrive gli FPS; `KIOSK_VEG_DENSITY` regola la vegetazione (0,5 di serie); `KIOSK_SCALE=1` forza 1 pixel per punto;
    - kiosk: `KIOSK_LEGEND=1` apre la legenda; `KIOSK_SHOT_ZOOM` e `KIOSK_SHOT_FOCUS=x,y` per i primi piani. La partita scriptata fotografa anche T+1:40 (`3d_esegui_dopo`);
    - velocità di serie ×40, ×120 nei tratti quieti (`kiosk/mod.rs`);
    - modelli della vegetazione: `/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/build_models.py`.
  - **Territorio:** si rigenera con `scenario_factory.py build-town --layout 3`, `town-fires --scenario t4_paese3 --ignitions-from t4_paese2` e `publish --scenario t4_paese3`.

## Decisioni dell'utente

- 2026-10-09: **tempo reale dichiarato** («non è chiaro se è real time o a turni»; «Va bene» alla proposta in 7 punti): pulsante unico play/pausa, ordini immediati durante il gioco, pausa visibile, orologio «ore 14:26», crisi come rallentatore, accelerazione solo se è davvero tutto fermo, intro esplicita.
- 2026-10-09: **massimo 3 pause** a partita (le crisi non contano).
- 2026-10-10: **selettore di difficoltà** all'avvio: Facile = `Rocco3`, Medio = `Forna3_gira`, Esperto = `Coste2_gira` (`levels` in `game.json`, `LEVELS` in `tools/factory/cases.py`). Nel finale «Cambia difficoltà» al posto di «Altro incendio»; classifica unica con il livello accanto.
- 2026-10-10: **punteggio come quota salvata** (0–1000, famiglie 70 %, case 30 %), senza bonus delle pause; classifica unica.
- 2026-10-09: playtest con **Sonnet**.
- 2026-10-09: **gli ordini si cambiano solo in pausa** (o alla crisi), così i cambi di piano sono pochi; **niente pausa subito dopo un'altra** (15 min simulati, `PAUSE_GAP_S`). Niente penalità a punti per i cambi.
- 2026-10-09: **punteggio e classifica:** salvati rispetto a nessun ordine (100 per famiglia, 10 per casa) + 50 per pausa non usata; tre iniziali arcade; classifica **unica** per tutti gli incendi.

- 2026-10-09: **prima di iniziare la partita nessun ordine; appena iniziata un piccolo tutorial.** Scelta dell'utente: stato «pronto» a ×0. Dopo «Inizia», «Riprova» o «Altro incendio» la mappa mostra fuoco e schede con i pulsanti degli ordini disattivati; «Inizia la partita» apre «Come si gioca» (4 passi della volontaria); gli ordini si sbloccano a «Ho capito», poi si pianifica a ×0 e «Avvia». Fatto: `Kiosk::started`/`help`/`can_order` (`kiosk/mod.rs`), `how_to_play` (`kiosk/ui.rs`); la partita scriptata fotografa `0b_pronto` e `0c_come_si_gioca`.
- 2026-10-09: playtest completo con un visitatore alla prima volta e almeno 3 partite; con Codex senza crediti, **farlo fare ad Haiku 5.5**.
- 2026-10-09: «correggi le cose minori per ora», **senza playtest dopo**: punti 6–9 del playtest 3 (fatto).

- 2026-10-09: **«Procedi»**: correggere i punti 1–5 del playtest 3 prima del checkpoint 5 (fatto: iterazione 5c). Commit e push richiesti. I resoconti dei playtest vanno dati all'utente in italiano.
- 2026-10-09: **affrontare le criticità del playtest 2**, più due richieste:
  - **aumentare la velocità tra i turni** (fase Esegui);
  - **vegetazione più leggibile**, con una differenziazione grafica marcata: pini marittimi, castagni, cespugli e prateria riconoscibili.

  «Per il resto lascio le scelte a te.»
- 2026-10-09: **layout 3** («Procedi» alla proposta): macchia e bosco fino al margine sud-ovest di Castelvento, perché il paese possa essere colpito.
- 2026-10-08: **nessuna evacuazione a piedi dalle case isolate** («molto poco realistico»). Nei nuclei sparsi ogni famiglia ha almeno un'auto; in paese è senza auto circa 1 famiglia su 12 (`town.py::vehicles`, applicato anche a `population.json` pubblicato: nessun'altra estrazione casuale cambia).
- 2026-10-08: **checkpoint 4 approvato** («Continua» dopo la proposta): layout 2 e i tre casi del kiosk, cioè Coste2_gira (principale), Piano2 e Borgo2 (introduttivo).
- 2026-10-08: **prestazioni: non ottimizzare ora**, solo ridurre la densità degli alberi (fatto: metà densità, da 24 a circa 40 FPS su M4 Pro).
- 2026-10-08: la difesa delle case deve **ridurre l'esposizione simulata**, non restare un proxy contabile (`Tally::note_defence`).
- 2026-10-08: incendi più vivaci e più lunghi. Vento di prova 40 km/h, 6 h simulate, umidità **3 %** fissa e uniforme. Evitare i bordi: mondo di 8 km con nucleo progettato di 4 km.
- 2026-10-08: terreno scelto **t4 (Crinale e sella)**.
- 2026-10-08: **checkpoint 2 approvato**, layout 1 di `t4_paese` così com'è. Affrontare la risoluzione fine **prima** della fase 3.
- 2026-10-08: **DTM, strade, case e rendering andranno a risoluzione più fine dei 20 m** del propagatore (che resta a 20 m). Avrà effetto sia sulla grafica sia sui sistemi di gioco. Da pianificare nelle prossime iterazioni.
- 2026-10-08: **il terreno fine serve solo a grafica e agenti, non all'incendio.** Il fuoco usa il DEM a 20 m approvato, invariato (non la media del terreno fine).
- 2026-10-08: punto 2 del checkpoint 3: "correggiamo il territorio". Dopo la misura sui casi veri (`docs/fase4/porta.md`), il blocco «fuoco alla porta» scatta (soglia 0,35). Il territorio è stato corretto per il punto 1: layout 2.
- 2026-10-08: **checkpoint 3 approvato.** La squadra AIB viaggia su mezzo. Le Coste indifendibile nei casi rapidi: accettato.
- 2026-10-08: **un solo motore, un solo scenario, una sola modalità.** Non fare riferimento a versioni vecchie del codice: rimosse (`demo`, `play`, `text`, `chat`, `telemetry`, scenari reali e dev, build web, palette VR, vecchio flusso del kiosk).
- 2026-10-08: fase 3 avviata senza aspettare gli screenshot di `cover.u8` ("segui il piano").
- 2026-10-08: **niente altro lavoro grafico ora**: seguire il piano, proseguire con sistemi e gameplay (fase 3).
- 2026-10-08: **risoluzione fine approvata** ("commit and push … continua"): terreno a 5 m e passo ritracciato a 8,9 km nello stesso corridoio. Avanti con `cover.u8` nel kiosk.

## Da fare

### Fase 2: insediamenti (in corso)

- [x] **Checkpoint umano 2** superato (2026-10-08). Ritocchi possibili più avanti, se servono per la giocabilità: Borgo più esposto (meno orti irrigui, bosco più vicino), passo più corto.
- [ ] Rivedere quali strade sono tagliafuoco: oggi solo la provinciale e le vie del paese sono non combustibili (vedi Borgo2 nella sezione sulla risoluzione fine).
- [ ] Popolazione: oggi tratti "compiacenti" copiati dal vecchio `demo_traits`. Va ripensata insieme alla preallerta (fase 3). Auto sistemate il 2026-10-08: case colpite invariate, famiglie colte in casa a Le Coste un po' meno (`docs/fase4/ab_sweep.md` e `crisi.md` rigenerati).

### Risoluzione fine (approvata il 2026-10-08)

- [ ] Verificare in fase 5, a distanza ravvicinata, il colore del suolo da `cover.u8` (ora visibile: niente più palette VR) e le macchie chiare attorno alle case de Le Coste.
- [ ] Primo piano nel kiosk di tornanti e piazzole (draping a livello di pixel).
- [ ] Strade come tagliafuoco: Borgo2 con vento da N supera o no la SP 12 secondo pochi metri di tracciato (spotting). Decidere se la provinciale è una fascia parafuoco più larga o solo una strada.

### Fase 4: crisi e bilanciamento

- [x] 2026-10-08 Rilevatore di crisi (`crates/rocca/src/crisis.rs`) sullo stato conosciuto:
  - tipi: quartiere scoperto con risposta ancora possibile, vento che gira, mezzo perso;
  - al massimo 2 crisi, distanziate di almeno 15 min;
  - niente crisi prima del primo piano, né per un quartiere lasciato scoperto consapevolmente all'ultima conferma.
- [x] 2026-10-08 Casi «_gira» in `game.json`: a T+45 il vento spinge il fuoco verso la località più vicina tra le altre.
- [x] 2026-10-08 **Layout 2** del territorio (`T4_L2` → `rocca_ventosa`): Il Borgo più esposto, Le Coste in 4 nuclei, casi «_gira».
- [x] 2026-10-08 Crisi **previsione** (bollettino 20 min prima del cambio di vento) e preposizionamento per la prima priorità (`PREEMPT_M` 3,5 km).
- [x] 2026-10-08 Report di fase 4 (`docs/fase4/README.md`): le priorità iniziali cambiano gli esiti in 4 casi; la crisi conta in Coste2_gira (58 case colpite rispondendo subito, 71 rispondendo tardi o ignorandola).
- [x] 2026-10-08 **Checkpoint umano 4** approvato.
- [ ] Bollettino meteo con incertezza (orario ± e probabilità), senza leggere il futuro dell'incendio.
- [x] 2026-10-08 Mezzi bloccati: `Game::unit_status` dice «bloccato: strada tagliata dal fuoco» se un mezzo in movimento è fermo da 5 min (`STUCK_S`); etichetta rossa sulla mappa.
- [ ] (prima versione, superata dal layout 2) Misura «la decisione alla crisi conta» (`examples/crisi.rs`, tabella in `docs/fase4/crisi.md`):
  - effetto netto in Coste2_gira: case colpite 76 se si ignora, 61 rispondendo subito, 47 rispondendo 20 min dopo; famiglie colte in casa 24 se si ignora, 12 rispondendo subito con evacuazione;
  - nella maggior parte dei casi rispondere cambia poco: vanno scelti casi in cui la crisi è un vero dilemma, o rivisto il territorio (piano d'azione: «si torna a case/strade e inneschi»).
- [x] 2026-10-08 Kiosk: fase **Crisi** a ×1 con countdown di 25 s. Il piano attivo prosegue; il proposto si applica alla conferma o alla scadenza (rivalidato), e se non cambia resta il piano attuale. Verificato con screenshot su Coste2_gira (crisi del vento a T+46).
- [ ] Le fasi ×0/×N/×1 sono tempo reale del kiosk; l'headless le simula con strategie (`examples/crisi.rs`). Valutare se spostare il countdown nel `Game` per i replay.
- [x] 2026-10-08 Casi scelti: Coste2_gira, Piano2, Borgo2 (`featured` in `game.json`).
- [ ] Preallerta come osservazione del grafo comportamentale (`HouseholdObs`).
- [x] **Rettificato (2026-10-08):** la «scoperta» del porting dei test era sbagliata. Sui casi veri la minaccia alla porta supera 0,35 in 9 casi su 18; non supera mai 0,55 (celle da 20 m). Il testo originale diceva: sul territorio la minaccia per le persone davanti alle case vale sempre 0, perché ogni casa ha una radura di circa 13 m. Il blocco comportamentale «fuoco alla porta» non scatta mai, e con lui i rami evacua-subito, riparo e ultima risorsa. Test `incident_gaps::the_last_resort_profile…` ignorato con motivazione. Decidere se è calibrazione del comportamento o del territorio.
- [ ] Difesa delle strade: misurare prima l'effetto sulla percorribilità (spec).
- [ ] Rivedere i parametri nuovi: `EMBER_DECAY_M` 700 m, `PROTECTED_EMBER` 0,85 e `PROTECTED_RADIANT` 0,5, soglie del coordinatore.

### Fase 5: nuova UX

- [x] 2026-10-09 **Feedback del playtest umano: leggibilità e tempi** (`docs/fase5/playtest_feedback.md`): etichette per i nuclei separati di Le Ghiande, collegamenti alla scheda comune; legenda di numeri/barre/mezzi e combustibili; stato delle famiglie e stime di arrivo nelle schede; viaggio dell'evacuazione dai percorsi reali (distinto da avviso, preparazione e code); aggiornamenti recenti sempre visibili e cronologia completa; indicazioni persistenti e pulsante «Pausa e modifica piano». Verificati test ABM/Rocca e build nativa; controllo visivo con partita scriptata.
- [x] 2026-10-09 (utente) Proporzioni da diorama, solo grafica (`crates/game/src/visual_scale.rs`): edifici ×1,9 in pianta e ×2,5 in altezza, auto ×3, persone ×4,5, mezzi ×7, alberi ×1,6, arbusti ×1,25. Coordinate, fuoco e percorsi invariati.
- [x] 2026-10-09 **Playtest automatico 2** (`docs/fase5/playtest_gpt2/README.md`): quattro personaggi GPT (nuova, quattordicenne, min-maxer, UX). Nessuna strategia dominante su Coste2_gira: AFK 16 / 73, evacua tutti 3 / 71, preallerta e poi cambio piano 1 / 73 (famiglie / case).
#### Iterazione 5b (2026-10-09, in quest'ordine)

1. [x] 2026-10-09 **Velocità tra i turni** (`docs/fase5/durata.md`):
   - ×40 di base, ×120 nei tratti quieti; crisi a ×1 e countdown invariati. Spec §2 e `00-LEGGIMI-PRIMA.md` aggiornati.
   - Esecuzione dimezzata: Coste2_gira da 5,8 a 3,3 minuti reali, Piano2 da 3,6 a 1,8, Borgo2 da 4,1 a 2,1.
   - Corretto `Sim::tick`: un frame lungo eseguiva step oltre la crisi e oltre la fine del caso. Test `game::sim::tests::the_speed_does_not_change_the_game`: a ×20, a ×120 e con frame lenti, stesse crisi e stesso esito.
   - FPS invariati tra ×0 e ×120. Nessuna frase dei personaggi persa.
   - [ ] Da confermare nel playtest 3: Piano2 a 1,8 minuti potrebbe risultare troppo breve per seguire il fuoco.
2. [x] 2026-10-09 **Vegetazione riconoscibile** (`docs/fase5/iterazione5b.md` §2), solo grafica:
   - nuovi modelli `pine` (pino marittimo) e `chestnut` (castagno) in `scripts/build_models.py`, `bush` più basso;
   - un solo modello per gruppo, colori distinti per gruppo e per classe, tinte del suolo per gruppo;
   - legenda con miniature e colori campionati dal rendering.
   - Piante invariate (551 mila), triangoli da 34,0 a 32,4 M. FPS nativi invariati (mediana 30 → 32).
   - [ ] FPS nel browser da misurare a finestra in primo piano: la scheda automatizzata in secondo piano viene rallentata da Chrome.
3. [x] 2026-10-09 **Stima d'arrivo dallo stato reale** (`iterazione5b.md` §3–4):
   - `Game::arrivals` durante Esegui; anteprima solo con un piano da confermare;
   - «~2 min» quando gli estremi coincidono;
   - test `fase3::arrivals_follow_the_units`.
4. [x] 2026-10-09 **Motivo nella scheda:**
   - senza mezzi, la scheda dice perché, da `uncovered` / priorità / minaccia;
   - nell'anteprima, «X tornerà alla base» per i mezzi che il nuovo piano richiama.
5. [x] 2026-10-09 **Debrief con il perché** (`iterazione5b.md` §5):
   - X/Y accanto alle barre;
   - «Perché» da `Game::story` (ordini, partenze, mezzi in postazione, fuoco alla porta);
   - test `fase3::the_story_tells_what_happened` e `docs/fase5/debrief_righe.md`.
6. [x] 2026-10-09 **Legenda** (`iterazione5b.md` §6):
   - sezioni apribili: in cima vegetazione e barra delle famiglie, «Schede dei paesi» chiusa;
   - «famiglie senza via»; testo dei tempi a ×40;
   - chiusa all'avvio, quindi non copre Pian dei Grilli. `KIOSK_LEGEND=1` la apre per le schermate.
7. [x] 2026-10-09 **Schede che si coprono** (`iterazione5b.md` §7):
   - schede compatte durante Esegui; si aprono al passaggio del puntatore; collisioni sulla dimensione compatta;
   - fondo più opaco, testo secondario a 14 punti.
   - [ ] Verificare l'apertura con il tocco su touchscreen.
8. [x] 2026-10-09 **Ritmo a metà partita** (`iterazione5b.md` §8):
   - all'avvio, la volontaria dice da dove parte il fuoco e verso chi lo spinge il vento;
   - dopo 10 minuti simulati, un fatto nuovo ogni 8 s (prossimo arrivo, fronte più vicino, famiglie ancora in casa).
9. [x] 2026-10-09 Minori:
   - «1 mezzo su 2» e «Resta 1 mezzo»;
   - etichette dei mezzi sopra i pannelli fissi, con una linea fino al mezzo.
10. [x] 2026-10-09 **Playtest automatico 3** (`docs/fase5/playtest_gpt3/README.md`):
   - nessuna strategia dominante (AFK 16/73, evacua tutti 3/71, preallerta poi evacuazione 1/71; q4 3/52 difendendo Castelvento);
   - partita di 4 min 38 s per q2 (prima ~9);
   - funzionano «Perché», «grazie a te», la volontaria con fatti nuovi, «Altro incendio».

#### Dal playtest 3 (proposte, da decidere con l'utente)

1–5. [x] 2026-10-09 (utente: «Procedi») corretti nell'iterazione 5c (`docs/fase5/iterazione5c.md`):
   - scelta vera alla crisi del mezzo perso;
   - scheda aperta con soli pulsanti;
   - legenda adattiva con scorrimento visibile, evitata dalle schede;
   - «Priorità N · 0 mezzi ora»;
   - «Alla fine, su N famiglie: …» nel finale.
   La differenza tra preallerta ed evacuazione immediata è rumore del seme (`docs/fase5/preallerta_vs_evacua.md`).
6–9. [x] 2026-10-09 (utente: «correggi le cose minori», niente playtest dopo) corretti:
   - **evacuazione:** la scheda dice «Evacua: ~5 min di strada, le ultime ~102» (mediana e più lontane) al posto di «~4–102 min + attesa»; la legenda spiega che è solo la strada (`kiosk/mod.rs`, `ui.rs`);
   - **sindaca:** «Evacuazione di Le Ghiande avviata.» e «Preallerta a X: tenersi pronti.» (`characters.rs`);
   - **fumetto:** non tagliato né a 1600×1000 né a 900×600 (partita scriptata). Nuovo `KIOSK_SIZE=900x600` per provarlo;
   - **legenda del finale:** campione pieno blu/arancio «barra piena: tu», «contorno: lo stesso incendio senza ordini»;
   - **cronologia del finale:** pallino sulla linea e icona piccola sopra, senza disco; didascalia «Le tue decisioni e gli imprevisti, ora per ora»;
   - **contrasto:** bordo scuro sotto anelli e percorsi (`rings.rs::halo_mesh`, `overlays.rs::spawn_outlined`);
   - **crisi a 900 px:** la riga dei pulsanti va a capo, quindi «Conferma» resta sullo schermo (prima era tagliata).
   - [x] ~~A 900×600 restano due problemi~~ (2026-10-12, vincolo tolto: schermo almeno 1080p): le schede si sovrappongono tra loro e la barra in alto copre «Legenda» e lo zoom.
   - Il punto «ritratti illustrati e diorama low-poly sono due stili» resta come osservazione: non ci sono interventi previsti.

#### Playtest 4: Giulia, prima volta, tre partite (2026-10-09, Haiku 5.5)

Prompt e resoconto in `docs/fase5/playtest_haiku/`. Codex aveva finito i crediti dopo 5 minuti, quindi (utente) l'ha giocato un agente Haiku 5.5 con Chrome DevTools, sulla build di `f6dced3`.

- Esiti (famiglie colte in casa / case colpite): P1 alla cieca 9 / 71; P2 «Riprova» evacuando tutti a T+0, 3 / 73; P3 «Altro incendio» 3 / 24.
- Da correggere (verificati sulle schermate dove indicato):
  - [x] 2026-10-09 finale: colonna «colte in casa», nel «Perché» «N non sono partite»;
  - [x] 2026-10-09 genere: «ci arriva in N min», «Il fuoco ha colpito Autobotte 1»;
  - [x] 2026-10-09 «le ultime ~102 min»;
  - [x] 2026-10-09 sulle schede: «Non difeso», «Difendi: manda i mezzi a proteggere le case», «Preallerta: si tengono pronti · Evacua: partono»;
  - [x] 2026-10-09 crisi «Scoperto» più corta: «Il fuoco può arrivare a X in circa N min. A2 ci arriva in 2 min, ma lascia Y con un mezzo in meno.» Il countdown resta 25 s (spec);
  - [x] 2026-10-09 «×120 · niente di nuovo»: da rivedere nel lavoro su tempo reale e pausa (scatta anche con l'evacuazione in corso);
  - [x] 2026-10-09 gergo: «i mezzi non possono avvicinarsi senza rischi», «con la strada tagliata», «mezzi mancanti:»;
  - [x] 2026-10-09 barra in alto: «da sud 40 km/h»;
  - verificato nel playtest mio (browser): «da confermare» compare, quindi l'ordine in pausa si vede. Il «primo click a vuoto» era dello strumento (eventi senza `buttons`). **Confermato invece:** le schede si spostano di ~20 px a ogni cambio di fumetto o di ordine, quindi un click può finire sul pulsante sbagliato.
- Funziona: barra «a casa / si preparano / in viaggio»; stima della strada; finale con il contorno «senza ordini» e il «Perché».
- «Evacua tutto a T+0» resta la scelta che vince sulle persone (3 famiglie colte in tutte le prove).

#### Tempo reale, pause, punteggio (2026-10-09, `docs/fase5/tempo_reale.md`)

- [x] 2026-10-09 play/pausa unico, ordini immediati durante il gioco, velo «IN PAUSA», orologio «ore 14:26», crisi «rallentato» con cornice, intro e «Come si gioca» aggiornati;
- [x] 2026-10-09 ordini solo in pausa o alla crisi; pausa di nuovo disponibile 15 min simulati dopo «Avvia» / «Riprendi»; pulsante «Pausa tra N min · 2 rimaste»;
- [x] 2026-10-09 3 pause (`MAX_PAUSES`); ×120 solo senza partenze in corso e con il fuoco a più di 1,5 km da chi è in casa (`QUIET_FIRE_M`). Durata: 4,5–5,3 min reali;
- [x] 2026-10-09 `rocca::score` (punteggio e classifica, 4 test) e `kiosk/board.rs` (file o `localStorage`); «Azzera classifica» in F2;
- [x] 2026-10-09 playtest mio nel browser, tre partite: 1/71 → 2/24 → Borgo2 1/6 con 1720 punti;
- [ ] schede che si spostano di ~20 px a ogni cambio di fumetto o di ordine (click sbagliati);
- [ ] primo click a vuoto mentre la telecamera inquadra un nuovo incendio;
- [ ] «Come si gioca» ricompare dopo «Riprova»;
- [ ] etichette dei nuclei sopra i mezzi; «Le Ghiande · nucleo 1» a volte in cima allo schermo;
- [ ] «fuoco a 0,0 km» nei motivi del coordinatore; «si ritira vicino a» con il paese sbagliato;
- [ ] cronologia del finale: segni alle 14:00 sovrapposti;
- [ ] scheda compatta senza l'ordine dato;
- [ ] tre iniziali lente (20 click per «GIU»): griglia di lettere;
- [ ] mezzi fermi a fine partita («acqua», «in arrivo ~1 min» per oltre 20 min simulati).

#### Playtest 5: Giulia con Sonnet 5.5 (2026-10-09, `docs/fase5/playtest_sonnet/report.md`)

Esiti: 3/70 (1330 punti), 3/71 (1470), 2/12 (770).

- [x] 2026-10-10 **punteggio contro esito** (utente: «procedi»): ora è da 0 a 1000 sulla quota salvata rispetto a nessun ordine, 70 % famiglie e 30 % case, senza bonus delle pause (`rocca::score`). La partita 3 (2/12) supera la 2 (3/71). Nuova classifica (`classifica2.json`, `rocca_ventosa_classifica2`);
- [ ] priorità a un paese irraggiungibile alla crisi: i mezzi lasciano un paese per uno che non raggiungono. Serve un avviso prima della conferma;
- [ ] in pausa Evacua è un interruttore: il secondo click toglie l'ordine proposto. Renderlo univoco o mostrare «ordinata alle …»;
- [ ] Preallerta ed Evacua poco distinguibili sulla mappa;
- [ ] il «Perché» non spiega le famiglie che non partono nonostante l'ordine (circa 22 %, comportamento del modello);
- [ ] pulsanti grigi prima di «Ho capito», senza spiegazione;
- [x] 2026-10-10 bonus delle pause non usate tolto;
- [ ] fuoco poco visibile all'inizio; crisi assenti in un caso;
- [ ] finale: numeri piccoli sulle barre; etichette A1 e A2 sovrapposte; schede sopra Castelvento.

- [ ] 2026-10-09 Riprovare con persone: comprensione delle stime di viaggio (non tempo garantito di evacuazione), legenda, etichette dei nuclei e aggiornamenti; verificare il layout su touchscreen (900×600 non più richiesto, 2026-10-12).

- [x] 2026-10-08 Mappa a tutto schermo con schede per quartiere (rango, fuoco, mezzi ora e dopo la conferma, Difendi / più importante / non difendere, Preallerta / Evacua), etichette dei mezzi, rotte attive (verdi) e proposte (bianche tratteggiate), barra in alto, crisi con countdown, pulsante unico, debrief contro «senza ordini» con Riprova / Altro incendio, barra operatore F2 (`docs/fase5/README.md`).
- [x] 2026-10-08 Crisi: «scoperto» solo se il fuoco può arrivare entro 45 min (`URGENT_S`); nessuna crisi negli ultimi 10 min (`LAST_CALL_S`).
- [x] 2026-10-08 Inquadratura calcolata (case + innesco nella zona libera, yaw ±50°); corretto il trigger che non scattava (il gioco parte a T+6 s).
- [x] 2026-10-08 Revisione «studente ignaro» (agente Sonnet, solo sugli screenshot). Corretti:
  - debrief con colonne «tu / senza ordini», «evacuate» al posto di «in salvo», «colte in casa dal fuoco», «mezzi persi»;
  - spiegazione di Preallerta / Evacua (testo e hover);
  - schede con «con il nuovo piano ne perde/riceve N»;
  - niente gergo («sottovento al fronte», «Scoperto»);
  - testo più grande e chiaro;
  - «Nuovo incendio, vicino a …».
- [x] 2026-10-08 Versione per browser (`docs/web.md`): dati compilati nel wasm (`crates/datafs`), terreno disegnato a 20 m e vegetazione al 12 % solo nel browser, workflow Pages. Partita giocata con il mouse nel browser: 60 FPS, ×20 regolare.
- [x] 2026-10-08 Dalla partita giocata: crisi del vento e della previsione solo se il quartiere non ha mezzi; motivo corretto dei mezzi in attesa; «mezzi assegnati»; Evacua non illumina più Preallerta.
- [x] 2026-10-08 Pubblicato su https://mirkodandrea.github.io/propagator_abm/. Il ramo `settimana-protezione-civile` è stato aggiunto ai rami ammessi dall'ambiente `github-pages` (decisione dell'utente: con `gh`, senza merge su `main`).
- [x] 2026-10-08 Playtest di GPT 6.1 Sol, che non conosceva il progetto, avviato da una cartella vuota (`docs/fase5/playtest_gpt/report.md`). Corretti:
  - «Ora X non ha mezzi» nella previsione (prima diceva «ha 1 mezzi»);
  - motivo «N mezzi su M» quando una parte non trova postazione;
  - riga «… ancora a casa»;
  - la preallerta dice che qualcuno può partire da solo;
  - pulsante «Vista iniziale»;
  - schede di dimensione fissa (i pulsanti non saltano più sotto il puntatore);
  - nel debrief, le decisioni del giocatore e il nome del mezzo perso.
- [x] 2026-10-08 Secondo playtest GPT 6.1 Sol, «roast da Redditor sedicenne» (`docs/fase5/playtest_gpt/roast.md`). Corretti:
  - pannello del coordinatore in gioco: ora mostra lo stato attuale dei mezzi, non i motivi vecchi;
  - chi resta a casa dopo l'ordine e perché (misurato: circa il 22 % del Borgo aspetta di vedere il fuoco o difende la casa; è comportamento del modello, non un bug);
  - ritiro e perdita dei mezzi registrati con ora e luogo, anche nel debrief;
  - «metti per primo»;
  - pannelli più opachi;
  - «T+0:42 di 3:00»;
  - crisi scaduta registrata.
- [x] 2026-10-08 Terzo playtest GPT 6.1 Sol, «roast» su UX e grafica (`docs/fase5/playtest_gpt/roast_ux.md`). Ha giocato in parte su una versione vecchia rimasta nella cache del browser: ora la build mette una versione negli URL. Corretti:
  - **pannelli e schede:** le schede evitano i pannelli fissi; linea che collega ogni scheda al suo luogo; pannelli richiudibili;
  - **comandi:** un'unica famiglia di pulsanti (secondari grigi, ordini colorati quando attivi, giallo solo per l'azione principale);
  - **contrasto:** distanza del fuoco e mezzi in difficoltà in etichette colorate su fondo pieno;
  - **legenda** dei segni sulla mappa;
  - **crisi:** in un solo riquadro (testo, «Metti X per primo», «Preallerta/Evacua X», effetto sui mezzi, Conferma) e quartiere evidenziato;
  - **camera:** trascinamento ricavato dal cursore (prima nel browser non funzionava), pulsanti + e −, istruzioni;
  - **debrief:** opaco, due numeri grandi in testa, definizioni richiudibili.
- [x] 2026-10-09 Roast a tema, sei personaggi in sequenza (`docs/fase5/playtest_gpt/p*`).
  - Completati: volontario AIB (realismo), studente daltonico (accessibilità), volontario al chiosco (gestione).
  - Rifatti, perché il profilo Chrome era occupato dalle mie schede di prova: min-maxer, quattordicenne, insegnante.
  - Corretti:
    - stato dei mezzi con «fuoco a N m» e «acqua N%», in arancione prima della ritirata;
    - «finisce il lavoro in corso, poi rientra» al posto di «al lavoro» senza postazione;
    - nota «Da ricordare» su cosa fare davvero all'ordine di evacuazione (debrief e passando sulla riga «a casa»);
    - pausa: «IN PAUSA» e «Conferma e riprendi»;
    - operatore:
      - «Prossima partita» distinta dalla partita in corso;
      - «Nuova partita» chiede una seconda pressione;
      - secondi delle crisi regolabili (25/40/60);
      - la legenda va sotto la barra F2, chiusa sugli schermi piccoli.
- [x] 2026-10-09 Roast rifatti: min-maxer, quattordicenne, insegnante. Corretti:
  - introduzione in tre passi brevi, legenda chiusa all'avvio;
  - i pulsanti delle schede non si spostano più;
  - «da confermare: evacuazione, riceve N mezzi» accanto agli ordini scelti;
  - «fuoco tra le case» al posto di «0,0 km»;
  - conteggi delle famiglie completi («raggiunte dal fuoco»);
  - debrief con il perché per paese e le definizioni sempre visibili.
- [x] 2026-10-09 (utente: «continua con le tue soluzioni») **Costo dell'evacuazione misurato** (`examples/costi.rs`, `docs/fase5/costi_evacuazione.md`). I costi esistono già nel modello (i mezzi rallentati dal traffico, le famiglie esposte in strada), ma con evacuazioni precoci non incidono: i mezzi sono in postazione a T+12 e nessuno viene colto in strada. Conta invece **quando** si evacua: tutto a T+0 lascia 3 famiglie colte in casa, a T+40 10, a T+90 15. Nessuna penalità inventata. Il debrief mostra l'orario degli ordini di evacuazione e le evacuazioni precauzionali.
- Misura originale: Misura su Coste2_gira, seme 1 (`target/release/rocca`):
  - nessun ordine: 73 case colpite, 16 famiglie colte in casa;
  - Il Piano per primo: 27 / 17;
  - evacuare tutto: 73 / 3;
  - tutte e due le cose: 27 / 3.
  
  La priorità decide le case, l'evacuazione le persone, e gli effetti si sommano. «Evacua tutto a T+0» è quindi sempre conveniente (anche al Borgo, dove il fuoco non arriva). Ipotesi da valutare con misura, senza penalità inventate:
  - traffico dell'evacuazione che rallenta i mezzi sulle stesse strade;
  - famiglie in strada esposte se il fuoco taglia il percorso;
  - debrief che mostri le famiglie spostate dove il fuoco non è arrivato.
- [x] 2026-10-09 **Durata:** accelerazione automatica ×3 quando nulla cambia (`Game::is_quiet`: 10 min senza eventi e nessun mezzo in movimento), le crisi restano a ×1. Partita da 9–10 a **4–6 minuti reali** (`examples/durata.rs`, `docs/fase5/durata.md`).
- Prima: Partita di circa 10 minuti reali a ×20, con tratti morti dopo gli ordini (quattordicenne). Opzioni: ×30-40 tra una decisione e l'altra, partita di 2 ore simulate invece di 3, accelerazione automatica quando non c'è nulla da decidere.
- [x] 2026-10-09 (utente) **Schermata introduttiva con logo CIMA e interfaccia adatta al touch** (`docs/fase5/img/intro.jpg`):
  - **schermata iniziale:** situazione, tre passi, «Inizia», nota su PROPAGATOR e Fondazione CIMA; compare all'avvio e dopo «Nuova partita» dell'operatore, non dopo «Riprova» o «Altro incendio»;
  - **comandi:** pulsanti alti almeno 44 px e testo più grande;
  - **camera:** un dito sposta la mappa, due dita fanno lo zoom;
  - **niente spiegazioni solo al passaggio del mouse;**
  - **pagina web:** niente zoom del browser né selezione del testo sul canvas.
  - **Non ancora provato su un touchscreen vero.**
- [x] 2026-10-09 (utente: «troppo testo, gui più grafica») Interfaccia a icone disegnate (`kiosk/icons.rs`):
  - **schede:** cerchio con il numero, fuoco con la distanza, autobotti come icone (piene, da ricevere, barrate se le perde), barra delle famiglie;
  - **pulsanti:** icona più una parola;
  - **mezzi:** badge con icona, sigla, livello dell'acqua e fiamma vicina;
  - **barra in alto:** vento, tempo come barra, stato come icona;
  - **crisi:** countdown ad anello;
  - **finale:** due numeri grandi con «−N grazie a te», barre tu contro senza ordini, linea del tempo con le decisioni;
  - **pannelli laterali:** chiusi all'avvio.
- [x] 2026-10-09 (utente) Località rinominate: Il Borgo → **Castelvento**, Il Piano → **Pian dei Grilli**, Le Coste → **Le Ghiande** (dati, Factory, test). I nomi dei casi (Borgo1, Coste2_gira…) restano invariati come identificativi interni.
- [x] 2026-10-09 **Castelvento colpibile: layout 3** (`docs/fase5/castelvento_layout3.md`, `tools/factory/town.py::T4_L3`). Macchia e latifoglie fino a 20 m dal margine sud-ovest; cambia solo `fuel.i32` (513 celle).
  - **Correzione:** la nota precedente («0 case in tutti i 18 casi») era sbagliata. Già prima Borgo2 colpiva 31 case senza difesa e Borgo1 3.
  - **Ora, Borgo2:** senza ordini ai civili 18 famiglie colte in casa (prima 5); con preallerta o evacuazione 3 (prima 0).
  - **Invariati:** Coste2_gira, Piano2 e 14 casi su 18. Rapporti `docs/fase4/{ab_sweep,crisi,porta}.md` rigenerati.
- [ ] Borgo3 non raggiunge Castelvento: 600 m di latifoglie umide, 11 ha in 3 h. Estendere le latifoglie secche a 750 m non cambia nulla (provato, annullato). Un caso con innesco nella macchia a sud-ovest richiede un nuovo innesco: da decidere con l'utente.
- [x] 2026-10-09 (utente) **Personaggi che parlano**, in basso a sinistra con fumetto (`crates/game/src/kiosk/characters.rs`). Ritratti generati da Codex (gpt-6.1-sol, generazione immagini), ridotti a 256 px in `assets/characters/`:
  - **sindaca**: ordini di preallerta ed evacuazione, famiglie raggiunte in casa, crisi «quartiere scoperto»;
  - **caposquadra VVF (DOS)**: mezzi che si ritirano o vanno fuori servizio, crisi «mezzo perso»;
  - **previsore meteo**: cambi di vento, crisi «previsione» e «vento»;
  - **volontaria di protezione civile**: intro, consiglio di partenza, «nella realtà» del debrief.
  Ogni frase nasce da una voce del registro, da una crisi o dall'esito già prodotti dal motore: nessuna regola nuova. La crisi ora si legge nel fumetto, con countdown e risposte subito sotto; proposta del coordinatore ed eventi spostati in basso a destra. Immagine: `docs/fase5/img/personaggi_crisi.jpg`.
- [ ] Personaggi: verificare in un playtest che il fumetto in basso a sinistra si noti durante la crisi (prima era in alto al centro) e che le frasi a ×20 non si accavallino (coda di 2, minimo 2,5 s ciascuna).
- [ ] Dai roast, ancora aperto: navigazione da tastiera (egui sul canvas), forme oltre ai colori per gli anelli, ~~layout a finestra stretta (900×600)~~ (tolto 2026-10-12), stato perso con la ricarica della pagina, verifica dei percorsi verso le aree di attesa, supporto ai ritardatari dell'evacuazione (scelta di design).
- [ ] Dal playtest GPT, ancora aperto: icone dei mezzi leggibili nella vista generale (oggi si leggono le etichette); indicatori aggregati per le evacuazioni sulla mappa; click che a volte sembrano non rispondere (forse solo per l'input automatizzato, da verificare con persone vere); distinguere mezzi assegnati / in viaggio / sul posto; mostrare sulla mappa perché una postazione è irraggiungibile.
- [ ] **Checkpoint umano 5.**
- [x] 2026-10-08 Schede dei quartieri separate quando si sovrappongono sullo schermo; etichette dei mezzi impilate e spostate sotto la scheda che le copre. Resta: una scheda può coprire il fuoco.
- [ ] Coordinatore: con la prima priorità irraggiungibile un mezzo rientra alla base (ora spiegato a schermo). Mandarlo al quartiere successivo annullava l'effetto della crisi in Coste2_gira (58 → 71): provato e annullato.
- [ ] Tempo di pianificazione senza limite: decidere nei playtest se serve (spec: ~40 s indicativi).
- [ ] FPS circa 40 su M4 Pro con vegetazione dimezzata: misurare sulla macchina del chiosco (fase 6).

### Fase 6

- [ ] Playtest (vedi `03-PIANO-DI-AZIONE.md`).

### Debito tecnico

- [ ] `CLAUDE.md` cita `docs/rocca-ventosa/…`, ma i documenti sono in `docs/`.
- [ ] File non tracciati rimasti dai vecchi scenari (`data/scenarios/{mati,pedrogao,rhodes,spotorno}/*.tif`, `data/osm_raw.json`, `data/spotorno_render_terrain.tif`, `dist/`, `results/`): da cancellare a mano, non sono in git.
- [x] 2026-10-08 Kiosk: pannello provvisorio sostituito dalla UX della fase 5.

## Fatto

- [x] 2026-10-12 finale in un solo pannello (`kiosk/ui.rs::debrief`, `leaderboard`): tolta la barra in alto con vento e ora (`top_bar`) e il pannello del punteggio separato; a sinistra verdetto, numeri, ordini, borghi e cronologia, a destra punteggio, classifica, nota e «Riprova». Il livello è una pillola accanto al titolo. Prova rapida, senza giocare (13 s): `KIOSK_SIZE=1920x1080 KIOSK_CASE=Coste2_gira KIOSK_WINDOWED=1 KIOSK_SHOT_END=1 KIOSK_SHOT=<dir> target/release/game` → `4_fine.png` (`kiosk/mod.rs::shots`: piano di prova, partita simulata fino in fondo, solo il finale).
- [x] 2026-10-12 schermata finale ridisegnata (`kiosk/ui.rs::debrief`, `leaderboard`): cornice dell'intro (sfondo scurito, bordo ambra, ombra), verdetto della sindaca con ritratto e fumetto (`verdetto`), due card grandi famiglie/case con «grazie a te» o «senza ordini», una card per borgo (verde al sicuro / ambra case / rossa famiglie) con «Perché» ripiegabile al posto del muro di testo, cronologia in una card; tolto `compare_bars`. Prova: `KIOSK_SIZE=1920x1080 KIOSK_CASE=Coste2_gira KIOSK_SPEED=300 KIOSK_WINDOWED=1 KIOSK_SHOT=<dir> target/release/game` → `4_fine.png`.
- [x] 2026-10-12 simulazione più veloce: `RUN_SPEED` 40 → 80 e selettore «velocità ×20 / ×40 / ×80 / ×160 / ×300» nella barra laterale (`kiosk/ui.rs::status`, non durante le crisi); resta il campo libero nel pannello operatore F2. Da verificare a occhio: ritmo delle finestre critiche a ×80.

- [x] 2026-10-08 **Un solo motore, un solo scenario, una sola modalità:**
  - lo scenario è pubblicato in `data/scenarios/rocca_ventosa` (`scenario_factory.py publish`) e `Scenario::load(data)` carica solo quello;
  - rimossi registro, build web, metadati dev e palette VR;
  - `rocca::Game` carica la libreria di comportamento di `data/behaviours` e parte con il fuoco già nello stato;
  - kiosk (`crates/game`): `sim.rs` è solo l'orologio intorno a `rocca::Game`, il nuovo `kiosk` è pianifica/esegui/fine con l'anteprima del piano, e la nuova partita si avvia solo dal pannello operatore (nessun reset per inattività); edifici e anelli seguono i fatti del `Game`;
  - rimossi `demo`, `play`, `text`, `chat`, `telemetry`, `tools/mcp`, gli script del vecchio gioco, gli scenari reali e quelli dev;
  - la squadra AIB viaggia su mezzo sulle carrozzabili: arrivo in 15 min invece di 59;
  - test del modello (`abm`, `fire`) portati su Rocca Ventosa da un subagente: eliminato il test della scialuppa (niente costa), ignorato `the_last_resort…` (vedi fase 4); workspace tutto verde (35 target).

- [x] 2026-10-08 **Fase 3 headless** (`docs/fase3/README.md`):
  - crate `rocca`: `Game`, piano, coordinatore, CLI A/B ed esempi `ab_sweep` e `civili`;
  - `game.json` della factory (`game-cases`): 9 casi, caserma VVF a Il Borgo e squadra AIB dalla SP 12;
  - roster esplicito, 2 autobotti + 1 squadra AIB;
  - preallerta reale in `abm`;
  - difesa fisica in `fire::exposure`;
  - faville che decadono con la distanza;
  - autobotti che pre-bagnano solo con il fronte entro 300 m;
  - 7 test di fase 3 verdi;
  - ETA dal percorso reale sulla rete aperta (13–14 min da Il Borgo a Il Piano per il passo da 8,9 km) e basi da `game.json`.

- [x] 2026-10-08 `cover.u8` collegato al kiosk: loader opzionale `scenario::Cover` (fallback se assente), niente piante su strade/case/orti (−332 piante), colore del suolo dalla copertura a 5 m in `terrain_mesh.rs`.

- [x] 2026-10-08 Risoluzione fine: `tools/factory/fine.py` (terreno a 5 m, sedi stradali, piazzole, `cover.u8`), router con direzione e costo dei tornanti (`roads.py`), tavola `docs/factory/fine/`. Il DEM del fuoco resta identico.
- [x] 2026-10-08 Strada del Passo rifinita: niente più sega di micro-tornanti, pendenza della sede al 14 % (era 25 % p95 sul DEM a 20 m).
- [x] 2026-10-08 `t4_paese` provato nel kiosk Bevy: si carica e gira a 60 FPS, terreno a 5 m incluso. Innesco e testi restano quelli di `demo_borgo`.

- [x] 2026-10-08 Fase 0, audit: `docs/audit.md` (commit `5d07e64`).
- [x] 2026-10-08 Fase 1, ambiente naturale: 4 candidati, sweep e atlante in `docs/factory/fase1/` (commit `f699873`).
- [x] 2026-10-08 `scenario_check` (`crates/abm/src/bin/scenario_check.rs`): verifica di uno scenario con il codice del modello.
- [x] 2026-10-08 Rifinitura di t4: erosione più leggera, niente parete artificiale sul versante SO.
- [x] 2026-10-08 Generatore del paese (`tools/factory/town.py`, `build-town`), layout 1 su t4. Tutti i controlli di `scenario_check` passano: 2 rifugi interni e 4 uscite.
- [x] 2026-10-08 Strade su percorso a costo minimo (`roads.py`, 9 %/14 %). Strada del Passo di 7,9 km.
- [x] 2026-10-08 Sweep accoppiati costruito/naturale con inneschi attorno ai paesi (`town-fires`), tavola e tabella di minaccia (`plate`) in `docs/factory/fase2/`.
