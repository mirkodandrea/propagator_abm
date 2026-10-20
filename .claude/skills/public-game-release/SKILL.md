---
name: public-game-release
description: Pubblicare un serious game per chiunque - build web (Rust/Bevy in wasm) con pagina introduttiva, dati separati e profili per dispositivo, build desktop macOS/Windows/Linux in CI con release GitHub, versione mobile/touch, QR e raccolta feedback legata al commit. Usare quando si lavora su build_web, GitHub Actions, release, caricamento, compatibilità con browser o telefono, o distribuzione a un evento.
---

# Pubblicazione: web e desktop

Il gioco si apre da un link o da un QR e deve partire in fretta su qualunque dispositivo.
Le regole della partita sono le stesse ovunque: le differenze tra le piattaforme sono solo
grafiche e di input.

## 1. Build web

- **Uno script solo** (`scripts/build_web.sh`): `cargo build --target
  wasm32-unknown-unknown` con un profilo dedicato, poi `wasm-bindgen --target web` (stessa
  versione del crate nel `Cargo.lock`) e un sito statico in `target/web/`. Lo serve un
  server qualunque, oppure GitHub Pages da una Action.
- **Il codice nativo esce dalla build web:** `std::os::unix`, link simbolici, processi e
  file system. Va messo dietro `#[cfg(not(target_arch = "wasm32"))]` o in binari a parte
  (laboratorio, strumenti). Le librerie C (es. libsm64 per Mario) richiedono un clang che
  compili per wasm, oppure vanno escluse con una feature.
- **Dati separati dal `.wasm`:** si comprimono in gzip con un `manifest.json` (percorso,
  URL con l'hash del contenuto, byte). Si scaricano in parallelo e si conservano in Cache
  Storage: una nuova build del codice non fa riscaricare i dati. La decompressione va al
  browser (`DecompressionStream`), con un ripiego nel wasm (`flate2`).
- **Versione negli URL** (`?v=<hash>`) di `game.js` e `.wasm`, contro le cache.
- **Loader:** solo una barra e i byte scaricati, niente totali stimati sbagliati.
- **Pagina introduttiva** HTML+CSS prima del wasm: hero con screenshot veri del gioco,
  testi brevi e narrativi senza meccaniche né numeri, logo dell'ente. Il wasm si carica solo
  dopo «Gioca» (`import()` dinamico), e dopo la pagina compare comunque la schermata del
  titolo del gioco. Un parametro (`?playtest=1`) salta l'intro per i test.
- **Profilo per dispositivo:** la pagina sceglie (`window.<profilo>`) bassa su telefoni e
  tablet (`(pointer: coarse)` o `userAgentData.mobile`) e media sui computer, con un
  parametro `?q=` per forzarlo. I profili toccano solo la grafica.
- **Input sul web:** il nome si scrive in un campo HTML, così sul telefono compare la
  tastiera. Il canvas disegna un pixel per pixel CSS anche sui Retina, e gli input vanno
  normalizzati alla stessa scala.
- **Memoria:** si misura la memoria lineare del wasm durante uno smoke test, con un limite
  di regressione (es. 450 MiB), e si cerca dove cresce.

## 2. Mobile

- Verticale e orizzontale, interfaccia compatta sotto una soglia (es. 1000×600). Vista
  iniziale nadirale con il nord in alto.
- Gesti: un dito sposta, due dita fanno zoom e rotazione. Trascinamento dei badge con lo
  scorrimento della mappa vicino al bordo. «Tocca la squadra, poi la destinazione». Pulsanti
  «+», «−» e «Isola». `touchCancel` non deve mai trasformarsi in un ordine.
- Pannelli scorrevoli se non entrano nello schermo. La chat diventa una notifica.
- Le prove si fanno in Chromium e WebKit headless, in più viewport (`agent-playtest`). Un
  emulatore non sostituisce un telefono vero, e va detto.

## 3. Desktop e CI

- **GitHub Actions**: una matrice macOS, Windows e Linux, `cargo build --release` con le
  sole feature pubblicabili (`--no-default-features` per escludere moduli nativi
  problematici), poi un bundle zip con i dati. Nel bundle i link simbolici vanno
  dereferenziati (`cp -L`), perché Windows non li ricrea.
- **Release:** con un tag `v*` si crea una release con gli zip e le note «Novità» generate
  dai commit. Una release mobile `download` (prerelease) tiene l'ultima build, e la pagina
  web vi rimanda con il pulsante di download.
- Le build si controllano con `gh run list` e `gh run view --log-failed`. Gli errori tipici
  di Windows:
  - funzioni del CRT che negli header sono inline (es. `localtime_s`), da collegare al
    simbolo reale (`_localtime64_s`);
  - percorsi e link simbolici.
- Il tag si rifà solo dopo una CI verde, puntando a una release pulita.

## 4. All'evento e dopo

- Un **QR** porta alla versione web. Nella versione web, un link «Provami / dai un parere»
  porta a un modulo (Google Form).
- Il **commit della build** si inietta in compilazione: `build.rs` scrive `GAME_COMMIT` con
  `git rev-parse --short=8 HEAD` e un suffisso se l'albero è modificato. Il commit va
  **precompilato nell'URL del modulo** (`entry.<id>=<commit>`), così ogni feedback è legato
  alla versione provata.
- Nessun reset per inattività, se non c'è un chiosco presidiato. La classifica è locale
  (un file per il desktop, lo storage del browser per il web), e il menu dell'operatore la
  azzera.
- Funziona offline una volta caricato. Nessun audio necessario.

## 5. Esempio: Isola di Roccavento

`scripts/build_web.sh`, `web/index.html`, `crates/datafs`, `crates/game/src/quality.rs`,
`.github/workflows/pages.yml` e `desktop.yml`, `docs/web.md` (tutto il dettaglio verificato
del web e del mobile).
