# Fase 4: crisi e bilanciamento (8 ottobre 2026)

## 1. Realizzato

- **Un solo motore, un solo scenario, una sola modalità** (decisione dell'utente):
  - `rocca::Game` è l'unica autorità e `data/scenarios/rocca_ventosa` l'unico territorio;
  - il codice dei giochi precedenti è rimosso;
  - il kiosk è un orologio intorno al `Game`.
- **Rilevatore di crisi** (`crates/rocca/src/crisis.rs`), solo su informazioni note:
  - **scoperto**: un quartiere minacciato senza mezzi, quando un mezzo può ancora arrivare prima del fuoco;
  - **previsione**: il bollettino meteo annuncia il cambio di vento 20 min prima e il gioco dice quale quartiere il fronte attuale verrebbe a minacciare. È una previsione, non il futuro della simulazione;
  - **vento**: il vento è girato;
  - **mezzo perso**.
  - Al massimo 2 crisi, a 15 min l'una dall'altra.
  - Nessuna crisi prima del primo piano, né per un quartiere lasciato scoperto consapevolmente all'ultima conferma.
- **Kiosk:** fase Crisi a ×1 con 25 s di countdown. Il piano attivo prosegue; il proposto si applica alla conferma o alla scadenza, rivalidato; se il giocatore non tocca nulla resta il piano attuale.
- **Coordinatore:** il quartiere in **prima** priorità viene coperto in anticipo se il fuoco è entro 3,5 km, qualunque sia il vento: il preposizionamento è una scelta del giocatore. Le priorità successive restano legate alla minaccia attuale.
- **Territorio, layout 2** (`town.py`, `T4_L2`, pubblicato come `rocca_ventosa`):
  - Il Borgo ha una cintura irrigua più piccola, quindi il bosco arriva ai margini (le aree di attesa restano rifugi misurati);
  - Le Coste diventa **4 nuclei di 6 case**, ciascuno difendibile da un mezzo, al posto di 24 case sparse;
  - i 9 casi «_gira» hanno il vento che a T+45 spinge il fuoco verso un'altra località.
- **Squadra AIB su mezzo** sulle carrozzabili: arriva in 15 min invece di 59.

## 2. Evidenza

```text
.venv/bin/python tools/scenario_factory.py build-town --layout 2
.venv/bin/python tools/scenario_factory.py town-fires --scenario t4_paese2 --ignitions-from t4_paese
.venv/bin/python tools/scenario_factory.py publish --scenario t4_paese2
cargo test --release --workspace                      # 35 target verdi
cargo run --release -p rocca --example ab_sweep       # priorità invertite → ab_sweep.md
cargo run --release -p rocca --example crisi          # strategie alla crisi → crisi.md
cargo run --release -p rocca --example porta          # minaccia alla porta → porta.md
```

**Priorità iniziali** (case colpite a 3 h, Il Borgo / Il Piano / Le Coste, stesso fuoco e seme):

| caso | nessuna difesa | scelta buona | scelta cattiva | perché |
|---|---|---|---|---|
| Coste2_gira | 0 / 55 / 18 | Piano > Coste: 0 / **9** / 13 | Coste > Piano: 0 / 55 / 16 | il fuoco nasce sulla strada del passo: chi va a Le Coste resta tagliato fuori da Il Piano quando il vento gira |
| Piano2 | 0 / 7 / 17 | Piano > Coste: 0 / 0 / **3** | Coste > Piano: 0 / 0 / 17 | difendere Il Piano ferma il fuoco prima de Le Coste; non è ovvio |
| Piano1 | 0 / 55 / 0 | Piano prima: 0 / **7** / 0 | Borgo > Piano: 0 / 55 / 0 | preposizionare a Il Borgo, che non è minacciato, costa Il Piano |
| Borgo2 | 31 / 0 / 0 | Borgo > Piano: **6** | Coste > Borgo: 12 | difesa fisica efficace su un paese compatto |

**Alla crisi** (Coste2_gira, partendo da Le Coste in testa; dettagli in [crisi.md](crisi.md)):

| risposta | case colpite | colti in casa |
|---|---|---|
| ignorare | 71 | 14 |
| Il Piano in testa alla previsione (T+26) | **58** | 17 |
| la stessa scelta 20 min dopo | 71 | 14 |
| Il Piano in testa + evacuazione alla previsione | 58 | **11** |

Rispondere subito conta, rispondere tardi no. Nessuna risposta alla crisi recupera la scelta iniziale giusta (22 case). Negli altri casi «_gira» la risposta cambia poco: i mezzi sono già dove servono, oppure la strada è tagliata.

**Minaccia alla porta** ([porta.md](porta.md)): supera la soglia di serie del blocco «fuoco alla porta» (0,35) in 9 casi su 18, con fino a 16 case. Non supera mai 0,55: con celle da 20 m, la cella che brucia accanto a una casa è a 20 m, spesso oltre il raggio radiante di erba e macchia.

## 3. Non risolto

- **Effetto della crisi:** è netto in un solo caso (Coste2_gira). Per il gioco basta un caso definitivo, ma andrebbe verificato con altri semi.
- **Previsione meteo:** il bollettino nel gioco è perfetto, perché annuncia esattamente il cambio del caso. Si può aggiungere incertezza (orario ± e probabilità) senza leggere il futuro dell'incendio.
- **Ordine al rientro:** i mezzi senza postazione tornano alla base anche quando la strada è tagliata. Restano «in movimento» per ore: da mostrare come «bloccato» nella UX.
- **Kiosk:** il pannello è ancora provvisorio (fase 5).

## 4. Passo successivo (checkpoint)

Proposta di **casi definitivi**:
1. **Coste2_gira** come caso principale: dilemma Il Piano / Le Coste e crisi del vento con previsione.
2. **Piano2** come secondo caso: scelta non ovvia, difendere un luogo ne protegge un altro.
3. **Borgo2** come caso introduttivo: la difesa funziona, il paese è compatto.

Se approvi, la fase 5 (nuova UX) parte da questi tre.
