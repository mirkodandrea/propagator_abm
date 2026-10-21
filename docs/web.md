# Versione per browser

La stessa partita della versione desktop, nel browser, per chi non la scarica. Stesso motore (`rocca::Game`) e stessi dati, scaricati a parte dal `.wasm` (vedi «Dati separati dal codice»; `crates/datafs`).

## Pagina introduttiva (2026-10-18)

`web/index.html` mostra prima una pagina HTML+CSS nello stile di cimafoundation.org (hero con una scena di incendio e un riquadro «Dal gioco», tre card, un paragrafo sulla ricerca, chiusura, crediti). Testi brevi e narrativi, senza meccaniche, durata o numeri (2026-10-19, richiesta dell'utente). `game.js` e il `.wasm` si scaricano solo quando si preme «Gioca» (`import()` dinamico in `start()`); con `?playtest=1` si salta l'intro. Dopo «Gioca» compare la schermata del titolo del gioco, con il suo «Gioca» (decisione dell'utente, 2026-10-18: non va saltata). Immagini in `web/img/`, copiate da `build_web.sh`: screenshot veri del gioco (rifatti il 2026-10-19 con i nuovi effetti grafici; `KIOSK_SNAP`, 1600×1000): `tattica-*` con la UI tattica (ritagliati senza banner e chat, `t=45/90/120`, seme del menu), `scena-*` (`borgo`, `cicatrice`) e `hero` senza interfaccia (`ui=0,marks=0,iso=0`, `KIOSK_GFX=-dof KIOSK_TACTICAL=0 KIOSK_BOOST=0`, `t=80/140/165/170`). Provata solo come pagina statica (desktop e 390 px), non con la build wasm.

## Dati separati dal codice e profilo grafico (2026-10-19)

**Dati.** I file del territorio (`data/scenarios/rocca_ventosa`, `data/behaviours`, `fuels_eu12.json`) non sono più nel `.wasm`. `build_web.sh` li comprime in gzip in `target/web/data/` e scrive `data/manifest.json` (percorso, URL con hash del contenuto, byte). La pagina scarica in parallelo il `.wasm` (compilato in streaming) e i file, li mette in `window.roccaData` e solo allora avvia il gioco; `datafs` li legge di lì (`crates/datafs`). Se il browser ha `DecompressionStream` (Chrome 80, Safari 16.4, Firefox 113) decomprime lui; altrimenti i byte restano gzip (`window.roccaDataGzip`) e li decomprime il wasm (`flate2`, a ogni lettura). Ogni file sta in Cache Storage (`rocca-data`) con l'URL con hash: una nuova build del codice non fa riscaricare i dati, e viceversa. Dati: 27 file, 4,5 MB gzip (15,9 MB grezzi). Con la decompressione nel browser restano in memoria JS circa 16 MB; con quella nel wasm solo i 4,5 MB compressi. Le due strade non sono state provate entrambe su un browser vecchio.

**Profilo.** La pagina sceglie `window.roccaProfile` e `quality::Graphics::from_env` lo legge: **`bassa`** (`Graphics::WEB`, il profilo di sotto) su telefoni e tablet (puntatore primario grossolano, `(pointer: coarse)`, o `userAgentData.mobile`); **`media`** (`Graphics::WEB_MEDIA`) su computer: come `WEB` più ombre del sole (2 cascate, 1024), terreno a passo 2 e vegetazione 0,05. `?q=bassa` o `?q=media` forzano il profilo, per le prove. I tetti del fuoco, il mare e la risoluzione del suolo restano uguali nei due profili. Verificato in Chrome su desktop con entrambi i valori di `?q=` (parte, grafica a vista); **non misurati gli FPS** (dal 2026-10-20 il terreno web usa colore e normale per vertice, senza le texture `ground_*`: WebGL2 ha 16 unità texture, vedi `TODO.md`) e non provato su un telefono vero.

## Telefono e budget grafico (2026-10-15)

La versione web usa un profilo leggero anche sui computer: terreno a 20 m,
vegetazione al 2,5% con sagome semplificate (le mesh dettagliate e le chiome
nascoste non vengono allocate), mare a 60 m vicino all'isola e 600 m al largo.
Niente ombre, HDR, profondità di campo, bloom o MSAA. Il canvas disegna un pixel
per pixel CSS anche sui display Retina; gli input sono normalizzati alla stessa
scala. Le mesh statiche di terreno e mare rilasciano la copia CPU
dopo il caricamento sulla GPU, anche nella versione locale, a geometria e resa
invariate. I ritratti conservano invece la copia CPU: bevy_egui richiede che
l'immagine resti negli asset per mantenere la texture registrata.
Il fuoco conserva al massimo 600 celle con fiamme,
100 particelle di fumo e 80 braci, con lo strato a terra a 20 m.
Sono semplificazioni visive: dati del territorio, simulazione, ordini e punteggio
restano quelli della versione desktop. Il menu web non permette di riattivare effetti pesanti.

L'interfaccia compatta si attiva sotto 1000 px di larghezza o 600 px di altezza:

- vista iniziale dall'alto (nadirale), nord in alto e zoom ampio sull'incendio;
- verticale: istruzioni in basso; orizzontale: tutorial a destra, mappa a sinistra;
- la schermata del titolo è quella del desktop (logo, titolo, sottotitolo, «Gioca»), in scala sulla larghezza;
- apertura, scelta dello scenario, finale e menu scorrono se non entrano nello schermo;
- il nome usa un campo HTML con tastiera del telefono;
- un dito sposta la mappa, due dita fanno zoom, rotazione e spostamento;
- in pausa si trascina il badge della squadra, oppure si tocca la squadra e poi l'esagono;
- durante il trascinamento si vedono percorso, tempo o motivo del rifiuto;
  tenendo il badge vicino al bordo della mappa, la vista scorre in quella
  direzione, più rapidamente avvicinandosi al bordo;
  rilasciare su un pannello annulla l'ordine; una gesture annullata o con due
  dita non invia ordini;
- un tocco sul borgo apre la scheda con persone, tempo e «Evacua»;
- «+», «−» e «Isola» restano disponibili senza gesti o tastiera.

Vista nadirale e icone dei paesi verificate il 2026-10-15 con build web e
test WebKit `--smoke`: avvio, nome, tutorial e rotazione superati; PNG
verticale e orizzontale controllati (`out/playtest/mobile-nadir`).

Lo scorrimento durante il trascinamento usa gli ultimi 48 px della mappa
libera, con velocità progressiva e proporzionata allo zoom. Pannelli e
istruzioni sono esclusi; tornando al centro, rilasciando o annullando il
gesto lo scorrimento si ferma. Verificato con Chromium touch a 390×844,
densità 3×, sui quattro bordi, arresto al centro e sui pannelli, `touchCancel`
e rilascio sul pannello senza ordini (`scripts/playtest_mobile.py --edge-pan`,
PNG in `out/playtest/mobile-edge-pan`).

Le istruzioni e i messaggi vanno a capo; sul web mobile i borghi sono indicati
da icone di casa senza nome, disposte senza sovrapporsi e con un collegamento
al punto sulla mappa. Il nome compare nella scheda aperta al tocco. La pagina rispetta
le aree sicure del telefono e consente di cambiare orientamento senza ricaricare.
Il confronto «senza ordini» del finale avanza a piccoli blocchi (budget di 8 ms,
con un passo indivisibile), lasciando disponibili interfaccia e riavvio. La
previsione mantiene i 16 membri originali, uno per frame nel browser.

```text
./scripts/build_web.sh                                  # sito statico in target/web
python3 -m http.server 8765 --directory target/web      # http://localhost:8765
```

Con `CARGO_TARGET_DIR` impostato il sito va in `$CARGO_TARGET_DIR/web`. Da Bevy 0.20 (2026-10-21): Rust 1.97.1 (`rust-toolchain.toml`), `wasm-bindgen-cli` 0.2.129 (stessa versione di `Cargo.lock`, anche in `pages.yml`), `getrandom` 0.2 con `js` e 0.4 con `wasm_js` per il target wasm, `strip = "symbols"` nel profilo `wasm-release` (senza, il `.wasm` pesava 126 MB; ora 61 MB, `wasm-opt` non installato). Provato in Chrome headless solo con `q=bassa` fino al titolo.

Pubblicazione: `.github/workflows/pages.yml` costruisce e pubblica su GitHub Pages a ogni push su `main`, `settimana-protezione-civile` o `nuovo-gioco`. Indirizzo: https://mirkodandrea.github.io/propagator_abm/

**Versione provata (2026-10-15):** `crates/game/build.rs` registra il commit della compilazione (`GAME_COMMIT`, con `-modificato` se l'albero non è pulito; in CI è quello pubblicato). Compare sul titolo («versione xxxxxxxx») e il pulsante «Dai un feedback» lo scrive da solo nella domanda «Versione del gioco» del Google Form (`entry.2126742218`).

**Cache del download (2026-10-17):** il `.wasm` è salvato in Cache Storage (`rocca-wasm`) con chiave l'URL versionato (`?v=<hash>`). Finché la build non cambia, i caricamenti successivi su telefono non scaricano nulla; con una nuova build si scarica di nuovo e la copia vecchia viene cancellata. Se Cache Storage non c'è o fallisce, si scarica come prima. Non provato su telefono.

Serve un browser con WebGL2. Il primo caricamento resta di circa 53 MiB non compressi:
la riduzione principale riguarda la memoria e la grafica durante il gioco.
Una scheda in secondo piano viene sospesa dal browser. In caso di perdita del
contesto grafico compare un pulsante per ricaricare.

## Verifica touch del gioco attuale

Correzione Safari del 2026-10-15, dopo la segnalazione di «Gioca» non
cliccabile su iPhone: riprodotto con WebKit 26.0 a densità 3×. Il resize
del browser riportava 1170×2532 per un canvas CSS da 390×844, nonostante
l'override 1×; l'interfaccia e i tocchi finivano così su scale diverse.
L'adattatore web sincronizza la risoluzione con le dimensioni CSS del canvas
prima di elaborare gli input; il CSS conserva le dimensioni del contenitore
anche quando winit richiede un resize. La verifica WebKit con tocchi nativi
copre «Gioca», nome HTML, avvio e avanzamento del tutorial e rotazione:
superata, senza errori browser; PNG controllati. Non è una prova su iPhone
fisico né certifica specificamente iOS 27.

Regressione Chromium del 2026-10-17 (flusso con scelta dell'incendio): tutti i controlli
di interazione fino al finale e al riavvio superati, nessun errore browser.
Memoria: 295 MiB nel tutorial, 379–416 MiB al finale; il limite del test è 450 MiB (2026-10-17).
La crescita avviene durante la partita, non al confronto senza ordini; vedi `docs/TODO.md`.

```sh
.venv/bin/python -m playwright install webkit
.venv/bin/python scripts/playtest_mobile.py --browser webkit --smoke --output out/playtest/webkit
```

Verifica precedente del 2026-10-15: `scripts/build_web.sh`, `cargo check -p game` e la
regressione touch completa superati. Chromium headless su macOS, densità 3×:
390×844, 844×390, 320×568, 568×320 e mouse a 1280×800. Nell'ultima esecuzione
(24 s) nessun errore browser; memoria lineare Wasm circa 203 MiB nel tutorial
e 273 MiB al finale (295 MiB in un'altra esecuzione). Il test impone
un limite di regressione di 450 MiB. Vegetazione: circa 10.900 piante e 180.000
triangoli nel profilo web. PNG controllati per tutorial, evacuazione, partita,
finale e telefono piccolo.

Il test copre anche rilascio sul pannello e `touchCancel`: il badge torna al suo
posto e il gesto successivo funziona. La versione di bevy_egui usata lasciava
premuto il suo pulsante mouse virtuale dopo `touchCancel`; l'adattatore web lo
rilascia esplicitamente senza trasformare l'interruzione in un ordine.

```sh
scripts/build_web.sh
.venv/bin/python scripts/playtest_mobile.py
# Per usare un Chromium già installato:
.venv/bin/python scripts/playtest_mobile.py --browser-executable /percorso/chrome-headless-shell
```

Il test usa eventi touch, viewport 390×844 e 844×390 a densità 3×, poi 320×568 e 568×320.
Controlla anche il mouse a 1280×800.
Controlla nome con tastiera HTML, tutorial, selezione con due tocchi, drag-and-drop,
evacuazione, zoom, orientamento, finale e riavvio. Salva PNG, osservazioni e
`report.json` in `out/playtest/mobile`, più **`report.html`**: un file solo, con tutte le immagini incorporate nell'ordine del test, fase, passo e memoria di ciascuna, e l'esito (da aprire nel browser per ispezionare). La memoria registrata è la capacità della
memoria lineare Wasm, non la RAM totale del browser/GPU. L'emulazione desktop
non sostituisce una prova su Safari iOS e Chrome Android su dispositivi reali.


## Runner del vecchio flusso (2026-10-13)

La modalità automatica di `playtest.py` qui sotto si riferisce al vecchio gioco
e va aggiornata; per il flusso attuale usare `playtest_mobile.py`.

Il browser headless rende la stessa UI egui e lo stesso diorama, senza finestre
sul desktop e senza prendere il focus. Il runner usa un profilo effimero: la
classifica dei test resta nel browser del test, non in quella della versione desktop.

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
