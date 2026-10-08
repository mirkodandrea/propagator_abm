# Fase 0 — Audit del repository (8 ottobre 2026)

Branch `settimana-protezione-civile` @ `2ab228d`. Nessun file di codice modificato.

## 1. Realizzato
Lettura dei 4 documenti, build e test di baseline, misura del runner headless, classificazione dei moduli. Unico file aggiunto: questo.

## 2. Evidenza

| Comando | Esito |
|---|---|
| `cargo build --workspace --all-targets` | OK, 17,6 s (incrementale), solo warning |
| `cargo test --workspace --no-fail-fast` | 7 min 28 s. **4 test falliti** su ~330, tutti in `demo` e tutti sul vecchio gameplay a turni: `balance::needless_stops_lose_families_and_a_stamp`, `lessons::l6_call_the_canadair_early`, `playtest_feedback::{recorded_session_three_plane…, fixed_orders_preserve…}`. Gli asserti numerici sono diventati obsoleti dopo i commit `78f1565`/`2ab228d`. |
| Harness di misura fuori dal repo (scratchpad), `demo_borgo` 200×200 @ 20 m, vento 180°/35 km/h, umidità 6 % | Solo fuoco, 2 h simulate: **0,01–0,03 s** con passo 300 s e **0,5–0,7 s** con passo 6 s. Area identica con i due passi (93–119 ha a seconda del seed), quindi è indipendente dal passo. `demo::Run` completo (fuoco + ABM + squadre + referee), 1 h: **~0,4 s**. |

Uno sweep di 100 incendi da 2 h costa meno di 5 s. Il budget di calcolo non vincola la Factory.

## 3. Classificazione

| Componente | Verdetto | Perché |
|---|---|---|
| `propagator-core`, `fire` | **RIUSA** | Il cambio di vento a run in corso (`set_weather`) e l'umidità scalare uniforme ci sono già. `ignite_patch` e `arrival_times()` sono disponibili. Manca solo un dump batch. |
| `scenario` (formato + `load_by_id`) | **RIUSA** | È il contratto per la Factory. Non contiene nulla di specifico di Spotorno. I rifugi sono *calcolati* (≤12 % di combustibile entro 300 m, o uscite a bordo mappa), non scritti a mano. |
| `abm` (famiglie, rete stradale, traffico, minaccia) | **ADATTA** | Rete, code e taglio delle strade per `ThreatField` sono buoni. Mancano tre cose: la **preallerta** (esiste un solo flag `ordered`), un **roster configurabile** (`DEFAULT_ENGINES=3`, `CREWS=3`, `TANKERS=2` sono costanti) e un **effetto fisico della difesa sulle case** (la soppressione agisce solo sul combustibile). |
| `behavior` (Composer) | **DA VERIFICARE** | `abm` lo usa per le decisioni, quindi non si può togliere. Va esteso con un'osservazione "preallertato". L'editor grafico non serve al kiosk. |
| `demo::Run`, `Referee`, `Tally`, `district`, `weather`, `event` | **ADATTA** | `Run` è il candidato a **unica autorità** (passo `STEP_S=6`). `district::post_facing` e `turn_policy` sono semi per il coordinatore. |
| `demo::Session`, `turn.rs`, token/target, `Patrol` | **RIMUOVI** (dopo aver estratto controfattuale, replay e verdetti) | Turni fissi da 8 min, comando diretto dei mezzi e una pattuglia che "avvisa" consegnando l'*ordine di evacuazione*. È l'opposto della spec. |
| `demo::{policy, refusal, cost, trust, sweep}` e i test legati | **RIMUOVI / DA VERIFICARE** | Vecchi design. `lib.rs` stesso li dichiara scartati. |
| `play` + `text` | **ADATTA** | Il replay deterministico (`seed + ordini`) e la mappa testuale vanno bene come runner headless per la fase 3. I comandi vanno cambiati. |
| `game`: terreno, vegetazione, fuoco/fumo, edifici, strade, unità, persone, camera, pick | **RIUSA** | È circa l'80–90 % del crate. Va solo ricollegato: `buildings::damage` legge `Referee`. |
| `game::sim.rs` | **ADATTA** | È un secondo driver di simulazione parallelo a `demo::Run`, con un ordine dei passi reimplementato. Va sostituito dalla chiamata all'autorità unica. |
| `game::kiosk/{mod,ui}.rs`, `command.rs`, regole in `overlays.rs` | **RIMUOVI** | Contengono la macchina a stati attract/briefing, i **reset per inattività** (`IDLE_RESET 60 s`, rotazione `NextTown`), gli ordini diretti ai mezzi e regole di validità e di pericolo duplicate nella GUI. |
| `scripts/generate_demo_scenarios.py` | **ADATTA** | Scrive già tutti i file del formato per città di 4 km × 4 km. È la base della Factory: le città sono fatte a mano, ma la serializzazione si può riusare. |
| `scripts/bake_fuels.py` | **ADATTA** | Legge uno YAML da un percorso assoluto esterno. Va conservato `data/fuels_eu12.json` già cotto. |
| `chat`, `tools/mcp`, `scripts/playtest.sh`, `dist/`, `results/`, dati reali (`spotorno`, `mati`, `pedrogao`, `rhodes`, `data/spotorno_*`) | **RIMUOVI** | LLM o rete, vecchia build web e dati reali fuori scopo. |
| `telemetry`, `assets/brand` | **DA VERIFICARE** | Log eventi per il debrief? Diritti d'uso del logo CIMA? |

## 4. Non risolto / incognite
1. **La difesa delle case oggi è un proxy di scoring.** `Tally::note_defence` segna come protette le case vicine a un'autobotte in postazione, e "casa colpita" significa terreno bruciato entro 150 m. Il fuoco non vede nulla di tutto questo. Bisogna decidere se la difesa debba ridurre davvero l'esposizione (`fire::exposure`) o restare un proxy dichiarato.
2. **Preallerta:** va costruita in `abm` + `behavior` (preparazione anticipata, nessuna partenza imposta), non rinominata.
3. **Difesa delle strade:** non esiste. Il fuoco rende una strada impercorribile tramite `ThreatField ≥ DANGER_CUT`. Il test d'effetto previsto dalla spec è ancora da fare.
4. **Anteprima:** `Session::preview` è analitica e senza effetti collaterali, ma `Session` non è `Clone`. Per un piano proposto rivalidato al commit servirà replay o snapshot.
5. **Python:** non c'è un ambiente dichiarato. Sono disponibili numpy 2.5 e Python 3.14 di sistema; scipy manca. Propongo `requirements.txt` con solo numpy + Pillow.
6. **Conflitti tra documenti:** `CLAUDE.md` cita `docs/rocca-ventosa/…`, ma i file sono in `docs/`. Oggi "Rocca Ventosa" è `demo_borgo`, fatto a mano, e verrà sostituito dal territorio generato. `data/README.md` è obsoleto. Ci sono 4 test rossi (vedi sopra).

## 5. Passo successivo proposto (uno solo)
**Fase 1 — Ambiente naturale.** Aggiungere `tools/scenario_factory.py nature|fires` che riusa la serializzazione di `generate_demo_scenarios.py` (4 km × 4 km, 200×200 @ 20 m, umidità fissa 6 %), più un piccolo bin Rust `fire_sweep` che dumpa i tempi di arrivo come `.i32` per innesco e vento. Output: 3–5 candidati, PNG di DEM, combustibili e propagazioni e un atlante. **Checkpoint umano 1.**
