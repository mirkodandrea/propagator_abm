# Playtest 7: esiste una strategia che vince sempre? (Haiku 5.5, headless, 2026-10-10)

Domanda dell'utente: il gioco è giocabile, o «evacua tutto subito» basta? Haiku ha giocato da «ottimizzatore» con il runner headless. Non ha usato la grafica, perché nel browser ogni azione costa 5–30 s. Il runner ora stampa il punteggio del chiosco, calcolato rispetto allo stesso incendio senza ordini (`crates/rocca/src/bin/rocca.rs`).

**Riproduzione:**

```
EVAC="0:Castelvento=evacua,0:Pian dei Grilli=evacua,0:Fornaci=evacua,0:San Rocco=evacua,0:Le Terrazze=evacua,0:Le Ghiande=evacua"
target/release/rocca Rocco3 --seme 1 --priorita "San Rocco" --civili "$EVAC"
```

**Verifica:** ho rieseguito io 8 dei numeri qui sotto (S1, S3 e il migliore su Rocco3; S1 e il migliore su Forna3_gira; S1 su Coste2_gira; seme 4 di Forna3_gira) e coincidono. Un'eccezione: «Castelvento primo + EVAC» su Rocco3 seme 1 dà 461 (Haiku: 437, con un ordine completo diverso).

## Punteggi (su 1000, media dei semi 1–3)

| Strategia | Facile `Rocco3` | Medio `Forna3_gira` | Esperto `Coste2_gira` |
|---|---|---|---|
| S1: evacua tutto a T+0, nessuna difesa | 488 (445/467/553) | 591 (571/603/600) | 157 (91/147/233) |
| S2: evacua tutto + difesa, il più vicino per primo | 732 | 693 | 157 |
| S3: solo difesa, nessun ordine civile | 377 | 220 | 0 |
| S4: misto (evacua i minacciati, preallerta gli altri, evacua dopo) | 744 | 564 | 157 |
| S5: il migliore trovato (EVAC + difesa del solo borgo più minacciato) | **752** (869/665/722) | **707** (703/772/647) | **167** |

Migliori: Facile `--priorita "San Rocco"`, Medio `--priorita "Fornaci"`, Esperto `--priorita "Le Ghiande,Le Terrazze"`, sempre con `--civili "$EVAC"`.

## Risposte

- **Evacuare tutti subito è sempre la scelta civile migliore.** Nessun piano con preallerta o evacuazioni scaglionate lo batte. Il motivo è strutturale: evacuare non costa nulla, né nel modello né nel punteggio.
- **Da sola però non basta: la difesa decide.** Senza difesa si perdono 265 punti al Facile (−35 %) e 116 al Medio (−16 %). La regola «evacua tutto + difendi il borgo più minacciato» arriva entro il 5 % del migliore.
- **Le priorità contano, soprattutto la prima.** Al Facile, mettere Castelvento invece di San Rocco come primo fa scendere il seme 1 da 869 a 461.
- **Il ritardo costa.** Al Facile, evacuare tutti a T+60 invece che a T+0 costa da 110 a 220 punti.
- **Preallerta ≠ evacuazione.** La preallerta da sola è meglio di nessun ordine (Medio: circa 700 contro 220), ma peggio dell'evacuazione su 2 semi su 3.
- **L'Esperto è quasi insensibile alle decisioni** (91–299). Pian dei Grilli brucia (82 case) e Le Ghiande ha 16 famiglie colte in casa anche con l'evacuazione a T+0. Al Castelvento, con l'ordine a T+0, restano a casa 43 famiglie su 190 (comportamento del modello).

## Stranezze

- Forna3_gira seme 4: nessuna famiglia né casa a rischio senza ordini, quindi il punteggio vale 0 qualunque cosa si faccia.
- Esperto seme 5: la difesa peggiora (177 contro 200).
- Medio seme 3: evacuare a T+30 dà 680, più che a T+0 (647). Probabile rumore.

## Limiti

- Le priorità sono fisse da T+0 e non ci sono risposte alle crisi: il dilemma a ×1 non è esplorato.
- Differenze sotto circa 50 punti sono rumore dei semi.
- Il punteggio è relativo al caso senza ordini: se quel caso ha poco rischio, i numeri sono instabili.

## Lettura del progettista (Claude Opus 5.5)

Il gioco ha una «ricetta» quasi ottimale: **evacua tutti a T+0 e manda tutti i mezzi sul borgo più vicino al fuoco**. Il giocatore deve solo indovinare il borgo giusto. La difesa ha un peso vero, ma dopo il primo minuto non resta nulla da decidere. Al livello Esperto le scelte non spostano quasi niente.
