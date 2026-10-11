# Perché evacuare tutti non costa nulla (headless, 2026-10-11)

Domanda: il playtest 7 ha trovato che «evacua tutti a T+0» è gratis. Il modello delle code (`abm::traffic`: capacità, spazio, code che risalgono) esiste già, rallenta i mezzi (`emergency_factor`) e può uccidere chi è in auto (`crates/abm/src/lib.rs`, esposizione alla fiamma dei viaggiatori). Allora perché non pesa?

**Riproduzione:**

```
cargo run --release -p rocca --example intasamento -- [--semi 1,2,3]   # tabella completa: semi1-3.md
cargo run --release -p rocca --example intasamento_mezzi -- Rocco3      # mezzi spostati durante l'ondata
```

Stesso incendio e seme per gruppo di righe, priorità migliore del playtest 7, cambia solo il piano civile. 3 livelli × semi 1–6 × 5 piani = 90 partite.

## Cosa succede

| | evacua tutti T+0 | preallerta tutti, evacua tutti T+30 |
|---|---|---|
| auto in coda (picco) | ~32, a T+30 | **~220**, a T+34 |
| dove | Castelvento (190 famiglie) | Castelvento 124, Fornaci 49, San Rocco 24 |
| pericolo massimo su un'auto (fiamma = 0,55) | ≤ 0,40 | ≤ 0,40 |
| famiglie colte fuori casa | **0** | **0** |
| ritardo dei mezzi | 0 min | 0–1 min |

1. **Le code si formano, ma lontano dal fuoco.** La più grande è sempre a Castelvento, che negli incendi dei tre livelli non è minacciato. Nei borghi minacciati le code sono piccole e si svuotano prima che arrivi il fuoco. In 90 partite il pericolo su un'auto non supera mai 0,40 e **nessuna famiglia viene travolta in strada**.
2. **I mezzi non incontrano la coda.** Partono dalla caserma di Castelvento a T+0, prima che le famiglie finiscano di prepararsi, e poi restano in postazione: i minuti di movimento sono identici con qualsiasi piano. Anche spostandoli durante l'ondata (priorità cambiata a T+34 con 207 auto ferme), arrivano al più 1 minuto dopo.
3. **La preallerta ha già un valore.** «Preallerta tutti, evacua tutti a T+30» non fa mai peggio di «evacua tutti a T+0» (18 confronti su 18) e fa meglio in 10: Medio +17…+73, Esperto +29…+73. Le famiglie preallertate partono pronte, quindi in coda ma in fretta. Il playtest 7 non l'aveva visto perché il suo piano misto era diverso. Resta però una ricetta fissa, e la coda da 220 auto che crea non costa nulla.

## Conclusione

Il costo reale c'è già nel modello, ma **la geografia non lo attiva**: il fuoco non attraversa mai una strada mentre c'è la coda, e i mezzi non devono mai passarci in mezzo. Non serve un'altra regola: serve che strade, abitati e incendi si incrocino.

## Incognite

- `intasamento_mezzi` su Coste2_gira: dopo T+34 nessun mezzo raggiunge i borghi diversi da Le Ghiande entro 90 min, con o senza traffico. Probabilmente le strade sono tagliate o i mezzi bloccati: da verificare a parte.
- Le differenze sotto circa 50 punti sono rumore dei semi.

## Proposta (una)

Mappare per ogni caso **quale strada usa ogni borgo per uscire, dove le uscite si uniscono e quando il fuoco le raggiunge**. Poi cercare tra gli inneschi già disponibili un caso in cui il fuoco tagli un'uscita in comune tra T+30 e T+60. Così evacuare un borgo che non serve intaserebbe la strada di uno minacciato, che potrebbe restare bloccato in coda: è il costo implicito cercato. Se nessun innesco lo fa, si torna alla viabilità del territorio (fase 2), con checkpoint umano. Lo stesso caso è un candidato per il nuovo Esperto.
