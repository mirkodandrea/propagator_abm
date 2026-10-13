# 02 — Il territorio

Un solo territorio sintetico e plausibile, riusato con diversi inneschi e venti: l'**Isola di Roccavento**. La scelta finale di territorio e livelli spetta all'utente.

## Com'è fatto

- **Isola di 8×8 km.** La griglia del fuoco è di 20 m (400×400). La riga 0 è il nord, y cresce verso nord, le coordinate sono in metri.
- Il rendering usa un terreno più fine e smussato: i 20 m valgono solo per il fuoco.
- **Dati:** `data/scenarios/rocca_ventosa/`, l'unico scenario del gioco. Contiene `dem.f64`, `fuel.i32`, `cover.u8`, `population.json`, `osm.json`, `game.json` (i tre livelli) e il terreno di rendering.
- **Sei borghi:**
  - a est del massiccio: **Castelvento** (190 famiglie, caserma dei vigili), **Fornaci** (90), **Le Terrazze** (30), **San Rocco** (100);
  - a ovest: **Pian dei Grilli** (90) e **Le Ghiande** (38, nuclei sparsi).
- La **rocca** è in cima, con un anello non combustibile di 130 m.
- Il **porto** a est è l'unica uscita: lì arriva la squadra AIB. Le aree di raccolta sono a Castelvento e a Pian dei Grilli.
- **Valico** (2026-10-13): una fascia di macchia alta (classe 9, con faville), circa 117 ha, lungo la Strada del Passo. Va da Le Terrazze, passa la sella e arriva alla macchia sopra Pian dei Grilli. Serve perché il fuoco possa arrivare anche ai borghi a ovest: prima nessun incendio colpiva case sia a est sia a ovest. Il fuoco passa a ovest quando il vento gira da nord-est dentro il passo. Larghezza variabile (in media 320 m), bordo frastagliato di macchia media (classe 8) e qualche isola del bosco vecchio, perché nel chiosco non sembri una striscia dipinta; il nucleo lungo la strada è sempre continuo.
- **Costa est** (2026-10-13): a nord del porto una baia sabbiosa, un piccolo promontorio e l'angolo nord-est in diagonale, al posto della costa dritta a 300 m dal bordo della finestra; anche il breve tratto dritto a sud del porto è rifatto. Solo terra diventata mare: porto, strade, case e inneschi non cambiano.
- **Umidità del combustibile:** 3 %, fissa e uniforme. La leva vera è il vento.
- Mezzi: 2 autobotti alla caserma di Castelvento, 1 squadra AIB al porto.

## Livelli (`game.json`)

Tutti durano 180 minuti con vento a 40 km/h. Gli inneschi sono in metri; il vento è indicato con la direzione **da cui** soffia.

| Livello | Caso | Innesco | Vento | Cosa succede (seme 1, senza priorità) |
|---|---|---|---|---|
| Facile | `easy` | (5600, 4600), imbocco del valico | fisso da 70° | 258–338 ha; Le Terrazze ~15:02, Le Ghiande ~15:20, Pian dei Grilli in bilico (0,1 km) |
| Medio | `medium` | (5590, 4370) | da 119°, gira a 50° al minuto 45 | 337–418 ha; Le Terrazze 14:29, Le Ghiande ~15:35, Pian dei Grilli 16:28 (86 case) |
| Esperto | `expert` | (5870, 4750) | da 119°, gira a 70° al minuto 45 | 273–301 ha; Castelvento 14:32, Le Terrazze 14:51, Le Ghiande colpita, Pian dei Grilli in bilico |

Numeri dei semi 1–3, senza ordini, con lo spegnimento del 13/10 sera; con la priorità «Le Terrazze, Castelvento» i tre incendi restano sotto i 25 ha in quasi tutti i semi (vedi «Aperto»).

I testi `about` dei livelli vanno riscritti per il valico. **Gli incendi sono solo questi tre** (decisione 2026-10-13): prove headless e test usano i livelli, non esistono incendi di prova.

## Come si modifica

- La vecchia pipeline di generazione è stata **cancellata** (2026-10-13, resta nella storia git): il territorio non si rigenera da zero.
- Si modificano **direttamente i file pubblicati** (`fuel.i32`, `game.json`, `population.json`, `osm.json`), con uno script ripetibile quando serve. Per esempio `tools/factory/valico.py` ridisegna il valico in `fuel.i32` partendo dal combustibile precedente (dalla storia git) e dà sempre lo stesso file; `tools/factory/costa_est.py` ha tagliato la costa est (`render_terrain.f32`, `dem.f64`, `fuel.i32`, `cover.u8`) e, rieseguito, non trova più nulla da tagliare. Entrambi sono autonomi (numpy, scipy).
- **Per provare una variante:** creare `data/scenarios/<id>/` con link simbolici a `rocca_ventosa` più i soli file cambiati, poi selezionarla con `ROCCA_SCENARIO=<id>` (`scenario::id()`). Vale per la CLI, i test e il chiosco. Niente copie dei dati (richiesta dell'utente). Esempio: `rocca_banco`, il banco di prova dello spegnimento.
- La vegetazione disegnata nel chiosco si legge da `fuel.i32` (`vegetation.rs`), non da `cover.u8`.
- Dopo ogni modifica del territorio si **rifanno le prove** di `03-VERIFICHE.md`.

## Cosa rende buono il territorio

- Gli incendi dei livelli colpiscono davvero i borghi, anche quelli a ovest, e lasciano **finestre d'intervento** utili. Non si ottimizza per il numero di case colpite.
- Esistono **compromessi**: due borghi minacciati insieme, mezzi che non bastano, un cambio di vento che ribalta il piano.
- Strade e aree sicure esistono e sono raggiungibili. I percorsi delle squadre hanno ETA plausibili.
- Nessuna strategia semplice vince sempre: si verifica con le prove di `03-VERIFICHE.md`.

## Aperto

- Con lo spegnimento in taratura, ogni innesco entro 800 m dalla caserma si spegne subito se le autobotti ci vanno per prime: con la priorità giusta l'ovest non entra mai in gioco. Si decide insieme alla taratura dello spegnimento; un'alternativa è spostare gli inneschi.
