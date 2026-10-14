# Isola di Roccavento

Gioco strategico in tempo reale sugli incendi boschivi, per il chiosco della *Settimana della Protezione Civile*: studenti delle superiori, italiano, mouse o trackpad, offline.

Un incendio simulato con PROPAGATOR minaccia i borghi di un'isola immaginaria. Dopo una prova guidata (giorno 1), il giocatore affronta un nuovo incendio (giorno 2). Il tempo scorre veloce. Con la **pausa** chiede la previsione all'esperto, sposta le squadre sugli esagoni ed evacua i borghi, uno alla volta. Alla fine riceve le stelle (persone, case, ettari) contro lo stesso incendio senza ordini, e c'è la classifica del giorno.

```
cargo run -p game --release                       # il chiosco
cargo test --release -p rocca                     # test del motore
TRACCIA=1 cargo run --release -p rocca -- tutorial  # il tutorial, headless
```

- Regole: [`docs/01-GIOCO.md`](docs/01-GIOCO.md).
- Territorio e incendi: [`docs/02-TERRITORIO.md`](docs/02-TERRITORIO.md).
- Fasi e prove: [`docs/03-VERIFICHE.md`](docs/03-VERIFICHE.md).
- Lavoro aperto: [`docs/TODO.md`](docs/TODO.md).
- Versione browser: [`docs/web.md`](docs/web.md).
- Istruzioni per i coding agent: [`CLAUDE.md`](CLAUDE.md).

Il territorio è immaginario. Nella realtà si seguono le indicazioni delle autorità.
