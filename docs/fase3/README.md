# Fase 3: gameplay headless (8 ottobre 2026)

## 1. Realizzato

- **`crates/rocca`, autorità unica nuova:**
  - `Game` esegue fuoco, civili, revisione del coordinatore, mezzi e protezione delle case, in quest'ordine, a passi di 6 s;
  - `commit` applica il piano e `preview` lo calcola senza effetti;
  - legge il territorio da `game.json` e non da `demo::ALL`;
  - non dipende dal vecchio crate `demo`.
  - `plan.rs`: priorità ordinate per quartiere e ordini civili (Nessuno, Preallerta, Evacua), che possono solo salire.
  - `coordinator.rs`: coordinatore deterministico e puro, cioè funzione dello stato attuale (sotto).
  - `bin/rocca.rs`: runner headless e comparatore A/B.
  - `examples/ab_sweep.rs`, `examples/civili.rs`: le tabelle qui sotto.
- **`tools/factory/cases.py`** (comando `game-cases`):
  - 9 casi, uno per innesco dell'atlante, con il vento che spinge il fuoco verso la località;
  - caserma VVF a Il Borgo con 2 autobotti;
  - squadra AIB che entra dal bordo est sulla SP 12.
- **Modifiche al modello**, che toccano anche il vecchio kiosk:
  - `abm`:
    - **preallerta reale** (`prealert_of`): arriva sul canale della famiglia come un ordine. La famiglia diventa consapevole e si prepara a casa (`readied_s`), senza nessuna partenza imposta. Se poi decide di partire, o riceve l'evacuazione, la preparazione residua è più corta e l'ordine arriva entro 2 min.
    - **roster esplicito** (`Suppression::with_roster`).
    - **l'autobotte non pompa finché il fronte è lontano**: pre-bagna solo con fronte attivo entro 300 m (`PREWET_M`, circa gli ultimi 10–20 minuti) o con minaccia già in portata. Prima vuotava il serbatoio sulla vegetazione e restava metà del tempo lontana dalla postazione per rifornirsi. Adeguato 1 test `abm` (`raising_the_refill_threshold…`), che presupponeva il pre-bagnamento a 400 m dal fronte.
  - `fire::exposure`:
    - **difesa fisica**: un mezzo in postazione toglie l'85 % del carico di faville e il 50 % di quello radiante alle case entro 120 m (autobotte con acqua 1, autobotte vuota 0,5, squadra AIB 0,6);
    - **correzione della saturazione da faville segnalata nell'audit**: la densità di faville decade con la distanza (scala 700 m). Prima, sommando centinaia di celle attive, risultavano colpite 160 case su 160 de Il Borgo con il fuoco a 3 km.

**Coordinatore**, ogni 2 min simulati o a ogni conferma:
1. Un quartiere in priorità va coperto se il fronte attivo è entro 1,5 km, o entro 3 km sottovento.
2. Il primo quartiere coperto riceve 2 mezzi, ogni altro 1, e l'eventuale avanzo va al primo.
3. Postazioni sulle case che il fronte raggiungerà prima (distanza ridotta dal vento), a 240 m l'una dall'altra, con strada aperta, minaccia sotto il limite di lavoro e fronte oltre 150 m.
4. Inerzia: la postazione resta finché le sue case restano tra le più minacciate.
5. Motivazione nel registro, ad esempio «Autobotte 1 → Il Piano (priorità 1): case sottovento al fronte, fuoco a 1,0 km, arrivo in 14 min».

## 2. Evidenza

```text
.venv/bin/python tools/scenario_factory.py game-cases
cargo test --release -p rocca                       # 7 test di fase 3, ~2 s
cargo run --release -p rocca --example ab_sweep     # 45 partite, ~1 min
target/release/rocca Piano1 --priorita Piano,Coste --b-priorita Coste,Piano --registro
target/release/rocca Piano1 --priorita Piano --civili "0:Piano=p" --b-civili "0:Piano=e"
```

**Invertire le priorità cambia l'esito** (case colpite a 3 h, stesso fuoco e seme; tabella completa in [ab_sweep.md](ab_sweep.md)):

| caso | nessuna difesa | prima il quartiere minacciato | prima l'altro |
|---|---|---|---|
| Piano1 | Il Piano 55 | Piano > Coste: Piano **8**, Coste 2 | Coste > Piano: Piano 55, Coste 2 |
| Borgo2 | Il Borgo 21 | Borgo > Coste: **4** | Coste > Borgo: 6 |
| Piano3 (va verso Le Coste) | Le Coste 15 | Piano > Coste: **0**, le autobotti fermano il fronte prima | Coste > Piano: 15 |
| Coste3 | Le Coste 8 | Coste > Borgo: **6** | Borgo > Coste: 8 |
| Coste2 | Le Coste 18 | 18 | 18 |

In Coste2 nessun piano salva case: il fronte supera le cascine sparse in meno di un'ora. Cambiano solo le famiglie colte in casa (11 → 9). In Borgo1, Borgo3 e Coste1 il fuoco minaccia poco.

**Preallerta ≠ evacuazione** (Il Borgo, innesco Borgo1 a 800 m, famiglie partite su 160):

| ordine a T+0 | 10' | 20' | 30' | 45' | 90' | pronte a casa |
|---|---|---|---|---|---|---|
| nessuno | 0 | 0 | 0 | 9 | 22 | 0 |
| preallerta | 0 | 0 | 0 | 14 | 45 | 160 |
| evacuazione | 13 | 29 | 82 | 116 | 124 | – |

Con il fuoco in arrivo la preallerta conta (Piano1, Il Piano, 2 h): famiglie colte in casa **1** contro 14 senza ordini e 3 con l'evacuazione a T+0.

Test (`crates/rocca/tests/fase3.rs`):
- stesso seme e stesso piano danno la stessa partita;
- l'inversione delle priorità cambia postazioni e case colpite;
- la difesa riduce l'esposizione simulata;
- la preallerta non manda via nessuno e prepara il 90 % delle famiglie;
- la preallerta riduce le famiglie colte in casa;
- l'anteprima non ha effetti e il commit rivalida;
- gli ordini civili possono solo salire.

## 3. Non risolto

- **Revisione del modello:** la correzione delle faville (700 m) e la difesa (85 %/50 %) sono parametri motivati ma non calibrati su dati. Le case "colpite" sono un fatto del modello di esposizione, non un danno strutturale verificato.
- **Squadra AIB:** si muove a `CREW_SPEED` 3 m/s anche sulla provinciale, quindi arriva in circa 60 min dal bordo est. Da decidere se viaggia su mezzo (velocità dell'autobotte sulle strade carrozzabili).
- **Le Coste è indifendibile** con 3 mezzi nei casi rapidi (Coste2): è una scelta di territorio da rivedere in fase 4. Inoltre 3 casi su 9 minacciano poco.
- **Test rossi:**
  - `abm::incident_gaps::the_last_resort_profile_sends_people_to_open_ground` (scenario reale Spotorno, fragile per sua stessa ammissione) cambia con la correzione delle faville;
  - in `demo` restano rossi i 4 test di sempre (vecchio gioco a turni), più `lessons::l5_engines_and_crew_save_homes_only_on_the_fires_path`, che cambia con le correzioni di faville e autobotti. Va rimosso con `Session`.
  - `game`, `fire` e `rocca` sono verdi.
- **Kiosk:** il kiosk guida ancora il vecchio `game::sim.rs` con `demo::Referee`. Il passaggio a `rocca::Game` è legato alla nuova UX (fase 5), come la rimozione di `Session`, del turno e di `game::sim.rs`.
- **Strade:** la difesa delle strade non è stata introdotta. Prima va misurato il suo effetto sulla percorribilità.
- **Comportamento:** il grafo comportamentale non vede ancora la preallerta come osservazione. L'effetto passa da consapevolezza, preparazione e ritardo dell'ordine.

## 4. Passo successivo (checkpoint)

Approva la fase 3: coordinatore, preallerta, difesa fisica e le due correzioni del modello (faville, acqua delle autobotti). Poi la **fase 4**: rilevatore di crisi, ×0/×N/×1 con timer e commit, e la scelta dei 2–3 casi definitivi (Piano1, Borgo2 e Piano3 sono i candidati, con il vento che gira).
