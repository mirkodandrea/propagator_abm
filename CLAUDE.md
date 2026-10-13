# Isola di Roccavento — guida per i coding agent

## Fonti di verità

Leggere all'inizio di ogni sessione:

1. **`docs/01-GIOCO.md`**: le regole in vigore (che cosa deve fare il gioco).
2. **`docs/02-TERRITORIO.md`**: l'isola, i livelli, come si modifica il territorio.
3. **`docs/03-VERIFICHE.md`**: le **fasi** con i loro checkpoint e la batteria di prove.
4. **`docs/TODO.md`**: la fase corrente, il lavoro aperto e le decisioni recenti.

I documenti dicono **cosa** costruire; il codice mostra **come**. Se non coincidono, prevalgono i documenti: segnalare il conflitto. Ogni nuova regola o decisione dell'utente va scritta, con la data, nel documento giusto. Lo storico non si conserva nei documenti: sta nella storia git. Documenti di servizio: `docs/MARIO.md` (easter egg), `docs/web.md` (versione browser), `docs/factory/<tema>/` (prove sul territorio).

## Il gioco in breve

Gioco strategico offline per un chiosco presidiato alla *Settimana della Protezione Civile*, per studenti delle superiori. Italiano, mouse o trackpad, nessun audio necessario. Un incendio su un'isola immaginaria con sei borghi. Il giocatore ordina le priorità di difesa, dà preallerta ed evacuazione (quest'ultima autorizzata dalla sindaca) e cambia il piano **mentre il tempo scorre**. Il gioco rallenta a ×1 da solo solo in pochi momenti critici. Un coordinatore deterministico muove i mezzi. La simulazione decide gli esiti. Il finale mostra stelle e classifica del giorno. Dettagli in `docs/01-GIOCO.md`.

## Architettura

**Una sola autorità di simulazione e gameplay: `rocca::Game`**, condivisa da runner headless, test e chiosco. Nella grafica non vanno fisica, punteggio o logica strategica.

| Crate | Contenuto |
|---|---|
| `propagator-core` | Il nucleo PROPAGATOR (automa cellulare del fuoco, checkpoint). |
| `fire` | `FireSim`, esposizione delle case (`exposure`), pericolo per chi è all'aperto (`threat`), ensemble per la previsione (`ensemble`). |
| `scenario` | Lettura dei dati dello scenario; `scenario::id()` legge `ROCCA_SCENARIO`. |
| `abm` | Famiglie e persone, strade e traffico, percorsi, squadre e spegnimento (`suppression`), uscita organizzata. |
| `behavior` | Libreria dei comportamenti modificabili (`data/behaviours`). |
| `rocca` | Il gioco: `Game`, `plan`, `coordinator`, `crisis`, `mayor`, `messengers`, `forecast`, `score`, `district`. CLI `rocca`, esempi headless, test. |
| `game` | Il chiosco Bevy: `sim.rs` (orologio attorno a `Game`), `kiosk/` (flusso, UI egui, tutorial, snap), rendering del diorama, `forecast_view`, Mario. |
| `datafs`, `libsm64-native` | Accesso ai dati (anche dal web); Mario. |

Dati: `data/scenarios/rocca_ventosa/` (territorio e `game.json` con i **tre livelli, gli unici incendi**: niente incendi di prova, decisione 2026-10-13). Python per il territorio: script autonomi in `tools/factory/` (`valico.py`, `banco.py`, …) che modificano i dati pubblicati.

## Come si lavora

- **Fasi, una alla volta** (`docs/03-VERIFICHE.md`). Ogni fase si chiude con un checkpoint umano: la fase successiva non parte senza conferma. Le richieste puntuali dell'utente si fanno subito, ma non fanno avanzare la fase da sole.
- Prima di cambiare **regole, bilanciamento o territorio**: prova headless e tabella, poi decide l'utente. Una meccanica nuova va accesa nel chiosco solo dopo la sua conferma, e va detto esplicitamente quando resta spenta.
- Il bilanciamento si giudica con un criterio: **nessuna strategia semplice deve dominare** («evacua tutti», «evacua appena autorizzato»). Le prove usano gli stessi semi (1–3) e i tre livelli.
- **Resoconti all'utente in italiano**, al massimo una pagina: realizzato, evidenza (comando e tabella o immagine), non risolto, una proposta.
- **`docs/TODO.md`** va aggiornata nello **stesso commit** del lavoro: fase corrente e prossimo checkpoint in cima, aperti, decisioni con data. Le voci chiuse si cancellano: la storia è in git.
- **Conservare ciò che serve, adattare ciò che può servire, scartare il resto.** Nessuna riscrittura totale senza una ragione verificata. Le API si cambiano dopo averne controllato gli utilizzatori.

### Altre sessioni nello stesso repository

Spesso più sessioni Claude lavorano in parallelo sullo stesso checkout (branch `settimana-protezione-civile`).

- Prima di mettere file in stage, guardare `git diff`: **non committare modifiche altrui**. Se un file è misto, chiedere alla sessione che lo sta modificando, o all'utente.
- Coordinarsi con `ListAgents` e `SendMessage`: chi tocca cosa, chi committa per primo.
- I sotto-agenti con `isolation: worktree` partono da `main`, non dal branch di lavoro. Farli lavorare nel checkout principale, su file distinti, senza commit, e fare il commit alla fine.
- Prima di un commit su file misti, verificare che l'albero in stage compili.

## Comandi

```
cargo build --release -p rocca --examples          # prove headless (docs/03-VERIFICHE.md)
cargo test --release -p rocca -p abm -p fire
target/release/rocca medium --priorita "Le Terrazze,Castelvento" --b-priorita "Castelvento,Le Terrazze"
cargo run -p game --release                        # il chiosco
scripts/build_web.sh                               # versione browser (docs/web.md)
```

Variabili utili del chiosco:

- `KIOSK_CASE`: il caso da giocare. `KIOSK_SEED` non esiste: il seme si sceglie dal menu operatore.
- `KIOSK_SPEED`: la velocità.
- `KIOSK_WINDOWED=1 KIOSK_SIZE=1280x720 KIOSK_SCALE=1`: finestra e dimensioni.
- `KIOSK_QUALITY=alta|media|bassa`, `KIOSK_GFX`: livello grafico.
- `KIOSK_FPS=1`: misura degli FPS.
- `KIOSK_TUTORIAL=0`: senza tutorial.
- `KIOSK_LEGEND=1`: legenda aperta.
- `KIOSK_SCORES=<file>`: classifica in un altro file, per le prove (non toccare `~/.rocca_ventosa/`).
- `ROCCA_SCENARIO=<id>`: un altro territorio.

### Screenshot di stati arbitrari (`KIOSK_SNAP`)

Per controllare grafica e UX **non giocare una partita**: `kiosk/snap.rs` porta il gioco a uno stato descritto in una riga e salva il PNG. Per gli snap nativi serve la build `--release` e una finestra vera. Per giocare e acquisire PNG **senza schermo né focus**, usare `scripts/playtest.py` (`docs/web.md`): browser headless, input reale, avanzamento rapido fino alla prossima decisione. `KIOSK_SHOT` gioca invece una partita scriptata intera; `KIOSK_SHOT_END` mostra solo il finale.

```
KIOSK_SNAP=<cartella> KIOSK_SNAP_SPEC='name=a,t=40,at=district:Castelvento,dist=700,pitch=-35;name=b,t=120,phase=fine' cargo run -p game --release
```

- Gli stati sono separati da `;`, i campi da `,`. Chiavi:
  - `name`, `case`;
  - `t`: minuti simulati, raggiunti headless;
  - `plan`: `scripted`, `none` o `keep`;
  - `prio`, `pre`, `evac`: borghi per nome, separati da `+`;
  - `phase`: `esegui` (a mondo fermo), `pianifica`, `fine`, `title` o `intro`;
  - `conferma`: il borgo per cui «Evacua» è stato premuto (conferma e vie di fuga visibili);
  - `at`: `home`, `fire`, `district:<nome>`, `post:<n>`, `unit:<n>`, oppure `x:y` in metri;
  - `dist`, `yaw`, `pitch` (gradi): l'inquadratura;
  - `select`: la scheda aperta.
- All'avvio vengono stampati su stderr casi e borghi disponibili. Il tempo va solo avanti: un `t` minore del precedente, o un altro `case`, rifà la partita da capo.
- Il mondo resta fermo: si muovono solo le animazioni della UI. Le crisi nate durante il salto vengono scartate. Il piano è quello scriptato, o quello dato con `prio`, `pre` ed `evac`. La previsione del fuoco compare da sola con un `t` dopo l'annuncio del cambio di vento (casi `medium` ed `expert`, circa t=30).
- Ogni stato resta in posa 2 s prima dello scatto. Per le prove di grafica conviene un solo avvio con più stati.
- Inquadrature di dettaglio: `at=district:…` o `post:…` con `dist` 600–900 e `pitch` da −35 a −45; `at=fire` per il fronte; con `at=home` resta la camera di partenza.
- **Dopo un avvio con `KIOSK_SNAP` guardare sempre l'immagine** (Read): «0 panic» nel log non dice com'è la grafica.

## Trappole note

Sono costate tempo e non si vedono dal codice. Prima di fidarsi, verificare che valgano ancora.

- **Le case non bruciano nell'automa:** stanno su celle non combustibili, come strade e persone. L'esposizione delle case è uno strato a parte (`fire::exposure`). Per chi sta all'aperto c'è un altro strato, `fire::threat`; i due non sono intercambiabili.
- **Un innesco di una sola cella** non attecchisce in circa il 20 % dei casi: usare sempre `FireSim::ignite_patch`.
- **Tutto ciò che si accumula va integrato sul tempo simulato**, non sul numero di update. Un controllo di invarianza al passo deve scendere anche sotto il ritmo interno del modello.
- **`wind_from_deg` / `w_dir` indica da dove soffia il vento** (il sorgente del kernel trae in inganno).
- **`vegetation_changes` è sparso tramite NaN:** una griglia piena di zeri riclassifica tutta la mappa.
- **Le strade dipinte come non combustibili** rendono immune chi è in coda: a questa scala il traffico non è mai il collo di bottiglia.
- **Il nodo carrabile più vicino** spesso è un'isola della rete: usare `nearest_reachable`. Mai un A* a ogni sotto-passo: si ricalcola quando cambia l'obiettivo o allo scadere di `REROUTE_S`.
- **Rendering:**
  - una striscia stesa sul terreno con il winding sbagliato è invisibile (avvolgerla `a, a+2, a+1`, e controllare in che verso corrono le righe del reticolo);
  - le linee drappeggiate vanno ricampionate, circa ogni 5 m;
  - i segni vanno alzati sopra la chioma (+20 m o più);
  - un `Material` personalizzato non riceve la nebbia.
- **Riavvio:** vanno ripuliti gli stati del rendering che si ricordano qualcosa (case annerite, particelle di fumo, veicoli). Tutto il resto si ricalcola da `Sim` a ogni frame.
- **Scorciatoie da tastiera:** devono rispettare `egui` `wants_keyboard_input()`, altrimenti scattano mentre si scrive il nome.
- **Un meccanismo nuovo deve essere inerte quando è spento**, anche a T+0. Una derivazione giusta su un territorio può essere sbagliata sul successivo: va verificata su tutti.
- **Ciò che il giocatore vede e ciò che il finale conta** devono venire dalla stessa regola.
