# 02 — Il territorio

Un solo territorio sintetico e plausibile, riusato con diversi inneschi e venti: l'**Isola di Roccavento**. La scelta finale di territorio e incendi spetta all'utente.

## Com'è fatto

- **Isola di 8×8 km.** La griglia del fuoco è di 20 m (400×400). La riga 0 è il nord, y cresce verso nord, le coordinate sono in metri.
- Il rendering usa un terreno più fine e smussato: i 20 m valgono solo per il fuoco.
- **Dati:** `data/scenarios/rocca_ventosa/`, l'unico scenario del gioco. Contiene `dem.f64`, `fuel.i32`, `cover.u8`, `population.json`, `osm.json`, `game.json` (stazioni, squadre, evacuazione, tutorial, incendi) e il terreno di rendering.
- **Sei borghi:**
  - a est del massiccio: **Castelvento** (190 famiglie, caserma dei vigili), **Fornaci** (90), **Le Terrazze** (30), **San Rocco** (100);
  - a ovest: **Pian dei Grilli** (90) e **Le Ghiande** (38, nuclei sparsi).
- **Aree di raccolta scelte a mano** (2026-10-14, `evacuation.refuges`; agganciate alla rete con `nearest_reachable`; esempio `examples/rifugi.rs`). Prima molti borghi finivano in un'area lontana o oltre il fuoco (Pian dei Grilli a 9 km sopra il valico, Castelvento e San Rocco lungo la SP 12 attraverso l'innesco):

  | Borgo | Area di raccolta | Posizione (m) |
  |---|---|---|
  | Castelvento, Fornaci | il pianoro a nord-ovest (capo della SP 12, terreno aperto) | 3265, 6249 |
  | San Rocco | il porto, a est, lontano dal fuoco | 7900, 3413 |
  | Le Terrazze | l'area di attesa di Castelvento | 5259, 5481 |
  | Pian dei Grilli | l'incrocio della SP 9 con la Costiera, nella piana ovest | 1778, 3582 |
  | Le Ghiande | automatica: l'area di attesa di Pian dei Grilli | 2174, 2536 |

- La **rocca** è in cima, con un anello non combustibile di 130 m.
- Le **aree sicure** dell'evacuazione si ricavano dai dati (`abm::refuge`: nodi stradali con poco combustibile intorno, raggiungibili in auto). Per ogni borgo si sceglie la più vicina su strada, a più di 400 m dalle sue case.
- **Valico** (2026-10-13): una fascia di macchia alta (classe 9, con faville), circa 117 ha, lungo la Strada del Passo. Va da Le Terrazze, passa la sella e arriva alla macchia sopra Pian dei Grilli. Serve perché il fuoco possa arrivare anche ai borghi a ovest: prima nessun incendio colpiva case sia a est sia a ovest. Il fuoco passa a ovest quando il vento gira da nord-est dentro il passo. Larghezza variabile (in media 320 m), bordo frastagliato di macchia media (classe 8) e qualche isola del bosco vecchio, perché nel chiosco non sembri una striscia dipinta; il nucleo lungo la strada è sempre continuo.
- **Costa est** (2026-10-13): a nord del porto una baia sabbiosa, un piccolo promontorio e l'angolo nord-est in diagonale, al posto della costa dritta a 300 m dal bordo della finestra; anche il breve tratto dritto a sud del porto è rifatto. Solo terra diventata mare: porto, strade, case e inneschi non cambiano.
- **Umidità del combustibile:** 5 %, fissa e uniforme. La leva vera è il vento.
- **Squadre** (2026-10-14): 3 squadre dei vigili del fuoco alla caserma di Castelvento, 2 squadre di volontari AIB al presidio di Pian dei Grilli (dal 2026-10-14, dall'altra parte dell'isola rispetto a Castelvento). Ogni squadra è un'autobotte con 3 persone.
- **Esagoni** di 500 m: valgono solo quelli attraversati da una strada carrabile della rete della caserma (`rocca::hex`).

## Incendi (`game.json`)

Il vento è indicato con la direzione **da cui** soffia. Gli inneschi sono in metri.

| Uso | Caso | Innesco | Vento | Durata | Note |
|---|---|---|---|---|---|
| Tutorial | `tutorial` | (3792, 3295), sotto Le Ghiande | da 135°, 20 km/h | 150 min | Seme 1. Senza ordini: 90 ha, 17 case colpite, 45 persone colte. Con il copione: 6 ha, nessuna persona colta. Trovato con `examples/tutorial_cerca.rs`. |
| Partita | `medium` | (5590, 4370) | da 119°, 40 km/h; gira a 50° al minuto 45 | 300 min, segnalato al minuto 30 (`start_min`) | Seme 1. Senza ordini: 638 ha, 137 case colpite, 426 persone colte. |
| (riserva) | `easy`, `expert` | come prima | | 180 min | Non usati dal chiosco. |

## Come si modifica

- La vecchia pipeline di generazione è stata **cancellata** (2026-10-13, resta nella storia git): il territorio non si rigenera da zero.
- Si modificano **direttamente i file pubblicati** (`fuel.i32`, `game.json`, `population.json`, `osm.json`), con uno script ripetibile quando serve. Per esempio `tools/factory/valico.py` ridisegna il valico in `fuel.i32` partendo dal combustibile precedente (dalla storia git) e dà sempre lo stesso file; `tools/factory/costa_est.py` ha tagliato la costa est (`render_terrain.f32`, `dem.f64`, `fuel.i32`, `cover.u8`) e, rieseguito, non trova più nulla da tagliare. Entrambi sono autonomi (numpy, scipy).
- **Per provare una variante:** creare `data/scenarios/<id>/` con link simbolici a `rocca_ventosa` più i soli file cambiati, poi selezionarla con `ROCCA_SCENARIO=<id>` (`scenario::id()`). Vale per la CLI, i test e il chiosco. Niente copie dei dati (richiesta dell'utente). Esempio: `rocca_banco`, il banco di prova dello spegnimento.
- La vegetazione disegnata nel chiosco si legge da `fuel.i32` (`vegetation.rs`), non da `cover.u8`.
- Dopo ogni modifica del territorio si **rifanno le prove** di `03-VERIFICHE.md`.

## Cosa rende buono il territorio

- Gli incendi colpiscono davvero i borghi, anche quelli a ovest, e lasciano **finestre d'intervento** utili. Non si ottimizza per il numero di case colpite.
- Esistono **compromessi**: due borghi minacciati insieme, mezzi che non bastano, un cambio di vento che ribalta il piano.
- Strade e aree sicure esistono e sono raggiungibili. I percorsi delle squadre hanno ETA plausibili.
- Nessuna strategia semplice vince sempre: si verifica con le prove di `03-VERIFICHE.md`.

## Aperto

- Segnalazione in ritardo (2026-10-14, `start_min`). Strategia semplice: «tutte le squadre sull'innesco, evacua Le Terrazze», all'inizio. Piano ragionato: quattro pause; squadre a Le Terrazze, Castelvento e Le Ghiande; evacuazioni a turno di Le Terrazze, Le Ghiande, Pian dei Grilli e Fornaci.

  | Ritardo | Seme | Strategia semplice: ha / case / persone colte / stelle P-C-E | Piano ragionato: stelle P-C-E |
  |---|---|---|---|
  | 0 | 1 / 2 | 2 / 0 / 0 / 5-5-5 · 3 / 0 / 0 / 5-5-5 | — |
  | 20 | 1 / 2 | 497 / 118 / 333 / 2-2-2 · 11 / 0 / 0 / 5-5-5 | — |
  | **30** | 1 / 2 | 481 / 127 / 343 / 1-1-2 · 469 / 126 / 333 / 2-1-2 | 5-2-3 · 5-2-2 |
  | 40 | 1 / 2 | 483 / 124 / 355 / 1-1-2 · 637 / 135 / 333 / 2-1-1 | — |

  Scelto 30. Da verificare con il seme 3, con «evacua tutto» e con piani migliori: oggi le Case restano basse anche col piano ragionato.
