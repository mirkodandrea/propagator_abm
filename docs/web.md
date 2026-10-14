# Versione per browser

La stessa partita del kiosk, nel browser, per farla provare a chi non è al chiosco. Stesso motore (`rocca::Game`) e stessi dati, compilati dentro il `.wasm` (`crates/datafs`).

Differenze solo grafiche, per WebGL:
- terreno disegnato a 20 m (i dati restano a 5 m);
- vegetazione al 12 %;
- niente MSAA.

Il confronto «senza ordini» del debrief avanza di qualche passo per frame invece che in un thread.

```text
./scripts/build_web.sh                                  # sito statico in target/web
python3 -m http.server 8765 --directory target/web      # http://localhost:8765
```

Pubblicazione: `.github/workflows/pages.yml` costruisce e pubblica su GitHub Pages a ogni push su `main`, `settimana-protezione-civile` o `nuovo-gioco`. Indirizzo: https://mirkodandrea.github.io/propagator_abm/

Serve un computer con mouse e un browser recente; il primo caricamento scarica circa 50 MB. Una scheda in secondo piano viene messa in pausa dal browser.

## Playtest headless per agenti (2026-10-13)

Il browser headless rende la stessa UI egui e lo stesso diorama, senza finestre
sul desktop e senza prendere il focus. Il runner usa un profilo effimero: la
classifica dei test resta nel browser del test, non in quella del chiosco.

```sh
uv pip install --python .venv/bin/python -r scripts/requirements-playtest.txt
.venv/bin/python -m playwright install chromium
scripts/build_web.sh
.venv/bin/python scripts/playtest.py --level Facile --output out/playtest/facile
```

Il comando esegue titolo → nome → livello → tutorial (Esc) → preallerta da una
scheda sulla mappa → Avvia → decisioni → finale → Riprova. Salva PNG e stato
JSON di apertura, piano, crisi, finale e ritorno ai livelli, più `report.json`
con azioni e durata, e `browser.log`. Fallisce se il motore o il browser
segnalano errori, se manca un controllo o se il flusso non arriva al finale.
Il server statico parte su una porta locale libera e si chiude alla fine.
`--url` permette di usare un server già avviato; `--size 1280x720` cambia il
viewport. `--browser-executable <path>` usa un Chromium già installato.

La modalità `?playtest=1` tiene fermo il tempo finché l'agente chiede di
avanzare. `advance` esegue i passi originali di 6 s alla massima velocità CPU,
senza attendere il tempo reale, fino al limite richiesto, alla prima crisi o
alla fine. Cede al rendering e all'input ogni ~40 ms. Non salta fisica, eventi
o decisioni. In una crisi l'agente sceglie sulla UI; `elapse` fa scadere i
conti alla rovescia senza aspettare. Il confronto senza ordini avanza più
passi per frame. Senza `?playtest=1`, i tempi del gioco restano quelli normali.

Per una sessione persistente guidata da un agente:

```sh
.venv/bin/python scripts/playtest.py --interactive --output out/playtest/agente
```

Un oggetto JSON per riga su stdin; una risposta JSON per riga su stdout.
La prima risposta contiene `ready` e lo stato. `state.controls` elenca nomi,
rettangoli e coordinate dei controlli egui (pixel CSS, scala 1). I click
passano realmente dal canvas a winit ed egui. Per controlli con nomi uguali,
`index` sceglie l'occorrenza; si possono anche usare coordinate da screenshot.
`Mappa:<borgo>` apre la scheda sulla mappa; `Borgo:<borgo>` identifica gli
item trascinabili nel piano.

```json
{"op":"state"}
{"op":"screenshot","name":"titolo"}
{"op":"click","label":"Gioca"}
{"op":"type","text":"Giulia"}
{"op":"click","label":"Medio"}
{"op":"key","key":"Escape"}
{"op":"click","label":"Mappa:Castelvento"}
{"op":"click","label":"Preallerta"}
{"op":"click","label":"Avvia"}
{"op":"advance","seconds":10800}
{"op":"screenshot","name":"decisione"}
{"op":"elapse","seconds":46}
{"op":"drag","from":[1260,500],"to":[1280,350]}
{"op":"wheel","y":-300}
{"op":"quit"}
```

`--script <file.json>` esegue un array delle stesse operazioni. `advance` è
accettato solo in esecuzione, fuori da tutorial e conferma evacuazione.
Ogni risposta osserva la fase, il tempo simulato, gli ordini, l'autorizzazione
per borgo e gli errori. Leggere sempre i PNG dopo una prova grafica.

Per la velocità il runner abilita la GPU headless (Metal su macOS).
`--software-rendering` usa SwiftShader sulle macchine senza GPU; è più lento.
Le schermate iniziali e gli scatti richiedono qualche frame di assestamento,
non attese di secondi fisse. Verifica del 2026-10-13 su M4 Pro: flusso completo
con screenshot e Riprova in 37,3 s (Facile) e 37,9 s (Medio), senza errori
browser; test Rust di invarianza di tempi delle crisi ed esito superato anche
con batch accelerati da 40 e 10.000 passi.
