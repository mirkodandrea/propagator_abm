# Iterazione 5b: velocità, vegetazione leggibile, correzioni del playtest 2

Stato: **in corso.** Fatti i punti 1, 2, 3, 4 e 6 di `docs/TODO.md` e parte del 9. Restano i punti 5, 7, 8 e il resto del 9, poi il playtest automatico 3.

## 1. Velocità tra i turni

- Velocità portata a **×40, ×120 nei tratti quieti**. Crisi a ×1 e countdown invariati (`crates/game/src/kiosk/mod.rs`).
- Tempo di esecuzione dimezzato (`docs/fase5/durata.md`):

| caso | prima (×20/×60) | dopo (×40/×120) |
|---|---|---|
| Coste2_gira | 5,8 min | 3,3 min |
| Piano2 | 3,6 min | 1,8 min |
| Borgo2 | 4,1 min | 2,1 min |

- **Corretto un difetto** in `Sim::tick` (`crates/game/src/sim.rs`): con frame lunghi si eseguivano step oltre la crisi, che arrivava a T+26:42 invece di T+26:06, e oltre la fine del caso.
  - Ora il frame si ferma sullo step che apre una crisi.
  - Prova: `cargo test --release -p game sim::`. Lo stesso gioco a ×20 e a ×120, con frame da 1/40 s, 0,25 s e 84 s simulati, dà crisi e esito identici. Senza la correzione il test fallisce.
- FPS uguali a ×0 e a ×120. Nessuna frase dei personaggi persa.

## 2. Vegetazione riconoscibile (solo grafica)

| gruppo | prima | dopo |
|---|---|---|
| Pineta (11–12) | `pine`, simile all'`oak`; 18 % di cipressi | pino marittimo: fusto alto, nudo, leggermente inclinato e rossastro; chioma a ombrello piatta e irregolare, verde scuro-bluastro. Niente cipressi. |
| Latifoglie (4–5) | `oak` e olivi (tutta la classe 4) | castagno: fusto corto, chioma grande, tonda e lobata, verde medio brillante. Classe 4 più verde e un po' più fitta. |
| Macchia (7–9) | `bush` verde scuro | cespugli bassi e senza fusto, verde oliva. Densità da 0,75× (classe 7) a 1,15× (classe 9). |
| Prateria (1–3) | erba quasi color suolo | paglierina; classe 3 più chiara e dorata. Alberi isolati invariati. |
| Suolo | tinte tenui | tinta del gruppo, più chiara, sfumata come prima (bilineare e deformata). |

- **File:** `scripts/build_models.py`, rigenerato in Blender: `assets/models/meshes.json`, `.blend` e `preview.png`. Poi `crates/game/src/vegetation.rs`, `terrain_mesh.rs` e `models.rs` (budget del castagno nel test).
- **Budget:**
  - piante: 550 983 contro 551 493 prima, densità invariata;
  - triangoli: 32,4 M contro 34,0 M;
  - modelli: pino 112 triangoli, castagno 156, cespuglio 100.
- **FPS nativi** (M4 Pro, finestra 3200×2000, pianificazione): mediana 32 dopo, 30 prima.

Vista generale, prima e dopo:

![prima](img/5b/veg_generale_prima.jpg)
![dopo](img/5b/veg_generale_dopo.jpg)

Da vicino (pineta, castagneto, prato), prima e dopo:

![prima](img/5b/veg_vicino_prima.jpg)
![dopo](img/5b/veg_vicino_dopo.jpg)

Macchia e prato, prima e dopo:

![prima](img/5b/macchia_prima.jpg)
![dopo](img/5b/macchia_dopo.jpg)

Browser (12 % di vegetazione):

![browser](img/5b/veg_browser.jpg)

## 6 (parte). Legenda

![legenda](img/5b/legenda.jpg)

- La legenda è divisa in sezioni apribili. In cima ci sono «Vegetazione: come brucia» e «Barra delle famiglie», poi «Segni sulla mappa»; «Schede dei paesi» è chiusa.
- Ogni tipo di vegetazione ha una miniatura con la stessa sagoma della mappa e colori campionati dalle schermate.
- Ogni tipo dice perché conta, secondo la tabella `data/fuels_eu12.json`:
  - erba: 120 m/h, senza faville;
  - macchia: 140 m/h, con faville;
  - pineta: 200 m/h, con faville;
  - latifoglie: nel territorio avanzano lente (fase 2: Borgo3, 11 ha in 3 h).
- «senza via» diventa «famiglie senza via». Il testo dei tempi dice ×40.
- `KIOSK_LEGEND=1` apre la legenda all'avvio, per le schermate.

## 3–4. Stima d'arrivo dallo stato reale, con il motivo

- **Durante Esegui** (piano invariato), la riga «Difesa» della scheda viene da `rocca::Game::arrivals`, che legge lo stato dei mezzi assegnati al paese:
  - «1 mezzo in postazione»;
  - «1 mezzo in arrivo ~5 min», con i minuti rimasti sul percorso che il mezzo sta facendo;
  - «1 mezzo bloccato dal fuoco»;
  - «A1 si ritira: troppo pericoloso».
- **Quando c'è un piano da confermare** (Pianifica, Crisi, «Pausa e modifica piano») resta l'anteprima del coordinatore. Aggiunge chi verrà richiamato, per esempio «Squadra A tornerà alla base».
- **Senza mezzi**, la riga dice perché, con le parole del coordinatore:
  - «non è tra le priorità»;
  - «nessun mezzo rimasto dopo le priorità più alte»;
  - «nessuna postazione raggiungibile e sicura ora»;
  - «nessun mezzo ora, il fuoco non minaccia ancora».
- Un intervallo con estremi uguali si scrive «~2 min».
- **Prova:** test `fase3::arrivals_follow_the_units`. Un mezzo inviato alla prima priorità è prima in arrivo, con minuti che non crescono, poi in postazione.
- **Punto 9 (parte):** «1 mezzo su 2» nel coordinatore e «Resta 1 mezzo» nella crisi del mezzo perso.

![schede durante Esegui, T+0:05](img/5b/schede_esegui.jpg)
![anteprima alla crisi](img/5b/scheda_anteprima_crisi.jpg)

## Incognite

- **FPS nel browser non misurati:** la scheda di Chrome automatizzata era in secondo piano e veniva rallentata (0,1 frame/s). Da misurare nel playtest 3 a finestra in primo piano.
- **Durata di Piano2** (1,8 min di esecuzione): forse troppo breve per seguire il fuoco. Da valutare nel playtest 3.
- **Pineta «può correre veloce»:** lo dice la tabella (v0 200 m/h). Nei casi del kiosk i fronti che arrivano ai paesi sono di erba e macchia. Da controllare che la frase non contraddica ciò che il giocatore vede.
