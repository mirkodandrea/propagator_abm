# Durata della partita al chiosco

Comando: `cargo run --release -p rocca --example durata`. Piano semplice: prima la località vicina all'innesco, con evacuazione; ogni crisi conta 25 s reali a ×1. Esclusi la pianificazione iniziale (circa 40 s) e il debrief.

Iterazione 5b (2026-10-09): esecuzione da ×20 a **×40**, tratti quieti da ×60 a **×120** (`RUN_SPEED`, `QUIET_BOOST` in `crates/game/src/kiosk/mod.rs`).

| caso | ritmo | minuti reali | di cui crisi | quota «tranquilla» | crisi a | frasi | frasi perse |
|---|---|---|---|---|---|---|---|
| Coste2_gira | ×20 / ×60 | 5.8 | 0.8 | 68 % | T+0:26, T+0:42 | 3 | 0 |
| Coste2_gira | ×40 / ×120 | 3.3 | 0.8 | 68 % | T+0:26, T+0:42 | 3 | 0 |
| Piano2 | ×20 / ×60 | 3.6 | 0.0 | 91 % | — | 0 | 0 |
| Piano2 | ×40 / ×120 | 1.8 | 0.0 | 91 % | — | 0 | 0 |
| Borgo2 | ×20 / ×60 | 4.1 | 0.0 | 82 % | — | 1 | 0 |
| Borgo2 | ×40 / ×120 | 2.1 | 0.0 | 82 % | — | 1 | 0 |

Verifiche:

- **Esiti indipendenti dalla velocità:** `cargo test --release -p game sim::` gioca Coste2_gira con risposta alle crisi a ×20 e 40 FPS, a ×120 con 40 FPS, con 4 FPS e con frame da 84 s simulati. Crisi agli stessi istanti ed esito identico. Prima della correzione il test falliva: un frame lungo eseguiva step oltre la crisi (crisi viste a T+26:42 invece di T+26:06) e oltre la fine del caso. Ora `Sim::tick` si ferma sullo step che apre una crisi e alla fine del caso.
- **Passo per frame:** a 40 FPS ×40 avanza 1 s simulato per frame, ×120 ne avanza 3: meno di uno step (6 s). Uno step costa circa 1 ms nativo.
- **FPS:** partita scriptata nativa (`KIOSK_SHOT`, Coste2_gira, finestra 3200×2000 su M4 Pro): circa 30 FPS istantanei sia a ×0 sia a ×40/×120. La velocità non pesa sul rendering. Partita completa in 285 s reali, contro circa 9 minuti prima.
- **Frasi dei personaggi:** con la coda da 2 e 2,5 s minimi non se ne perde nessuna nei tre casi (tabella).

Nel playtest 2 la partita reale durava circa 9 minuti: i giocatori si fermano a pianificare e a leggere. Con i nuovi valori ci si aspetta circa la metà. Va confermato nel playtest automatico 3.
