---
name: game-snapshot-harness
description: Costruire e usare un harness che porta un gioco (Bevy o simile) a uno stato descritto in una riga e ne salva lo screenshot su file, con finestra invisibile e senza rubare il focus. Usare per verificare grafica e UX senza giocare, confrontare prima/dopo di uno shader, misurare FPS per stato, produrre immagini per il sito o per playtest su screenshot.
---

# Screenshot di stati arbitrari

Per giudicare grafica e UX **non si gioca una partita**. Una riga di specifica porta il gioco
a uno stato preciso: fase, tempo simulato, ordini, selezione e camera. Il gioco salva il
PNG ed esce.

## 1. Come costruirlo

- **Variabili d'ambiente:** cartella di uscita (`KIOSK_SNAP=<dir>`) e specifica
  (`KIOSK_SNAP_SPEC`). Gli stati sono separati da `;`, le coppie `chiave=valore` da `,`.
- **Chiavi utili:**

  | Chiave | Cosa fa |
  |---|---|
  | `name` | nome del file |
  | `phase` | fase del gioco (titolo, tutorial, gioco, fine) |
  | `step` | passo del tutorial |
  | `t` | minuti simulati, raggiunti in modo headless alla massima velocità |
  | `pause` | partita in pausa |
  | `move` | ordini, più d'uno uniti da `+` |
  | `evac` | borgo da evacuare |
  | `team`, `hex` | squadra e destinazione in anteprima |
  | `select`, `hover` | borgo selezionato o sotto il puntatore |
  | `at` | fuoco della camera: casa, fuoco, borgo, squadra, coordinate |
  | `dist`, `yaw`, `pitch` | inquadratura |
  | `ui=0`, `marks=0`, `iso=0` | spengono interfaccia, segni sulla mappa e isocrone |

- Gli **ordini passano dalle stesse funzioni del giocatore** (l'autorità di simulazione):
  nessuna scorciatoia che renda uno stato impossibile nel gioco.
- **Il tempo va solo avanti.** Uno stato con `t` minore del precedente, o di un'altra fase,
  rifà la partita da capo. Il mondo è fermo allo scatto.
- **Assestamento:** si aspetta qualche secondo reale prima dello scatto, perché camera,
  dissolvenze e particelle arrivino a regime. Per un bersaglio lontano la camera può non
  arrivare in tempo: va ripetuto lo stato, oppure la camera va posizionata direttamente.
- **Finestra invisibile e senza focus** (`visible: false`, `focused: false`) e volume a 0
  quando l'harness è attivo. Verificato su macOS: il PNG si scrive senza che compaia nulla.
- **Misura delle prestazioni per stato** (in Roccavento `KIOSK_SNAP_BENCH=<s>`, in lavorazione
  in un'altra sessione al 2026-10-20: verificare che esista): vsync spento, tempi
  dei frame su N secondi, una riga `bench:` con FPS medi e p95. Così il prima/dopo di
  un'ottimizzazione si fa sugli stessi stati.
- Si scrive la documentazione delle chiavi nel modulo e in `CLAUDE.md`, perché le altre
  sessioni la trovino.

## 2. Come si usa

- **Solo quando l'utente lo chiede in quel messaggio.** Il permesso vale per quella
  richiesta. Altrimenti si verifica compilando e leggendo il codice, e si dice
  esplicitamente che la grafica non è stata guardata. Lo stesso vale per qualunque
  browser.
- **Dopo ogni scatto si guarda l'immagine** (Read). Non si dichiara mai riuscita una
  modifica visiva senza averla vista.
- **Più angoli, non solo nadirale.** Molti difetti (esagoni frastagliati sul terreno, segni
  sotto la chioma, z-fighting) si vedono solo da viste oblique e basse. Nadirale serve per
  i layout di borghi e strade.
- **Per giudicare la scena** si spengono UI, segni e isocrone (`ui=0,marks=0,iso=0`) e gli
  effetti che alterano i colori (in Roccavento `KIOSK_TACTICAL=0 KIOSK_BOOST=0`: la pausa
  tattica desatura). **Per giudicare la UI** si accendono solo gli elementi in esame.
- **Non coprire il soggetto:** un mezzo sotto l'area della previsione non si vede. Si
  sceglie un `t` e un'inquadratura in cui l'elemento è libero.
- **Confronto prima/dopo:** stessi stati, stessa specifica, due cartelle. Se serve
  mostrarli all'utente, si fa un `report.html` con le immagini affiancate o a schede.
- **Immagini per il sito:** stati di scena senza UI, più qualche vista tattica ritagliata,
  a risoluzione fissa (es. 1600×1000).

## 3. Esempio: Isola di Roccavento

`crates/game/src/kiosk/snap.rs` (chiavi documentate in testa al file), `main.rs` (finestra
invisibile, volume 0). Comando ed elenco delle chiavi in `CLAUDE.md`:

```
KIOSK_WINDOWED=1 KIOSK_SIZE=1600x1000 KIOSK_SCALE=1 KIOSK_TACTICAL=0 KIOSK_BOOST=0 \
KIOSK_SNAP=<cartella> KIOSK_SNAP_SPEC='name=fuoco,t=80,at=fire,ui=0,marks=0,iso=0;name=pausa,t=40,pause=1,team=0,hex=5300:4600' \
cargo run -p game --release
```
