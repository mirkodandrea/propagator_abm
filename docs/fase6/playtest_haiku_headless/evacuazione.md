# Perché evacuare peggiorava l'esito (2026-10-13)

Il playtest headless di Haiku ([report.md](report.md)) aveva trovato un caso strano: sul Medio, seme 3, aggiungere l'evacuazione alla preallerta peggiorava l'esito, 732 → 615 punti.

**Causa: un difetto del modello delle famiglie, non una scelta di bilanciamento.**
- Una famiglia preallertata che aspetta in casa intanto si prepara (`readied_s`, `abm/src/lib.rs`). Quando decide di partire, le resta poco da fare.
- Una famiglia che ha ricevuto l'**ordine** di evacuazione e «aspetta di vedere il fuoco» (intento WaitAndSee) non si preparava affatto, perché la riga contava solo la preallerta.
- Nel Medio, seme 3, la sindaca autorizza San Rocco già a T+0. L'ordine di evacuazione arriva quindi senza una preallerta prima.

Prova: `cargo run --release -p rocca --example famiglie -- medium 3 "San Rocco" "San Rocco,Castelvento,Le Terrazze,Fornaci,Le Ghiande,Pian dei Grilli"`. Con l'evacuazione, le famiglie colte in casa passano da 6 a 11. Tutte e 5 in più sono WaitAndSee che cominciano a prepararsi solo quando vedono il fuoco, con zero minuti già fatti. Per esempio, la famiglia 401 ha l'allerta sul telefono a T+1, inizia a prepararsi a T+69 e viene colta a T+74.

**Correzione** (una riga in `abm/src/lib.rs`): anche un ordine arrivato prepara la famiglia mentre aspetta. «Evacua» resta diverso da «Preallerta»: con l'ordine la maggior parte parte subito, mentre la preallerta non fa partire nessuno per forza.

## Effetto

Livelli, priorità «vicine», punti prima → dopo:

| livello, seme | solo preallerta | preallerta + evacua | evacua all'autorizzazione | evacua a T+30 |
|---|---|---|---|---|
| easy 1 | 920 → 920 | 893 → 893 | 803 → 920 | 803 → 920 |
| easy 2 | 413 → 413 | 553 → 553 | 273 → 553 | 273 → 553 |
| easy 3 | 637 → 637 | 637 → 637 | 404 → 520 | 404 → 520 |
| medium 1 | 494 → 494 | 494 → 564 | 587 → 657 | 424 → 494 |
| medium 2 | 679 → 679 | 617 → 700 | 617 → 741 | 494 → 617 |
| medium 3 | 732 → 732 | 615 → 732 | 732 → 872 | 545 → 615 |
| expert 1 | 393 → 393 | 416 → 439 | 533 → 579 | 299 → 323 |
| expert 2 | 668 → 668 | 653 → 668 | 668 → 698 | 564 → 578 |
| expert 3 | 462 → 462 | 423 → 482 | 443 → 521 | 326 → 365 |

82 incendi (`examples/sindaca.rs`, semi 1–3), medie prima → dopo:
- solo preallerta: 539 → 539;
- evacua all'autorizzazione senza preallerta: 355 → 404;
- evacua 10 min dopo l'autorizzazione: 293 → 338;
- preallerta tutti + evacua: 615 → 627;
- preallerta i vicini + evacua: 618 → 632.

L'ordine delle strategie è lo stesso: nessuna nuova strategia dominante.

**Ora** «preallerta + evacua» non è mai sotto la sola preallerta sui livelli. Ritardare costa ancora. Sui livelli l'evacuazione all'autorizzazione senza preallerta diventa spesso la migliore, perché preallertare tutti sparpaglia le pattuglie. Sugli 82 incendi resta debole in media, quindi non domina.

**Da sapere:**
- `docs/fase6/sindaca/README.md` e le soglie delle stelle sono stati calcolati prima della correzione. I numeri cambiano di poco (+10–50 punti per i piani con evacuazione) e non ho ritarato le soglie.
- In `abm/tests/incident_gaps.rs` 5 test falliscono anche senza la correzione: sono rotture preesistenti.
