# Playtest 8: esiste ancora una strategia che vince sempre? (Haiku 5.5, headless, 2026-10-12)

Domanda: dopo il passaggio a mezzi e squadre che guidano con le stesse regole (`c777919`), esiste una strategia che vince sempre? «Evacua tutti a T+0» è ancora la scelta civile migliore? L'Esperto è ancora insensibile alle decisioni? Haiku ha giocato da «ottimizzatore» con il runner headless; comandi, definizioni delle strategie e output grezzi sono in `comandi.md`, `run8.sh`, `extra_semi4-8.sh`, `raw_semi1-3.txt`, `raw_semi4-8.txt`. Questo file l'ho scritto io (Claude Opus 5.5) dal testo di Haiku, che non poteva crearlo.

**Riproduzione:** `bash docs/fase5/playtest8_strategie/run8.sh` (semi 1–3), `extra_semi4-8.sh` (semi 4–8). Esempio:

```
EVAC="0:Castelvento=evacua,0:Pian dei Grilli=evacua,0:Fornaci=evacua,0:San Rocco=evacua,0:Le Terrazze=evacua,0:Le Ghiande=evacua"
target/release/rocca Rocco3 --seme 1 --priorita "San Rocco" --civili "$EVAC"
```

**Verifica:** ho rieseguito S5 e PB su tre livelli × semi 1–3 (18 numeri) e PC al Facile seme 1: coincidono tutti.

## Punteggi (su 1000)

Media dei semi 1–3; tra parentesi la media dei semi 1–8 dove eseguita. «Migliore» = la priorità migliore del playtest 7 (Facile San Rocco, Medio Fornaci, Esperto Le Ghiande, Le Terrazze).

| Strategia | Facile `Rocco3` | Medio `Forna3_gira` | Esperto `Coste2_gira` |
|---|---|---|---|
| S1: evacua tutti T+0, nessuna difesa | 488 (458) | 591 (507) | 157 (183) |
| S2: evacua tutti T+0 + difesa del più vicino | 752 | 707 | 157 |
| S3: solo difesa del più vicino | 355 (282) | 241 (213) | 10 (9) |
| S4: evacua i minacciati T+0, preallerta gli altri, evacua T+30 | 752 (702) | 717 (659) | 157 (174) |
| S5: evacua tutti T+0 + migliore | 752 (706) | 707 (638) | 149 (171) |
| PA: preallerta tutti T+0, evacua i minacciati T+0 + migliore | 752 (699) | 717 (659) | 149 (171) |
| **PB: preallerta tutti T+0, evacua tutti T+30 + migliore** | **752 (745)** | **744 (694)** | **214 (218)** |
| PC: evacua tutti T+0 + priorità invertite | 488 | 591 | 157 |
| PD: evacua tutti T+30 + migliore, senza preallerta | 752 (691) | 718 (642) | 54 (64) |

«Nessuna priorità» coincide con S1 in tutti i semi. Forna3_gira seme 4 vale 0 per ogni strategia (nessuna famiglia a rischio senza ordini).

## Risposte

- **Nessuna strategia vince in ogni seme**, ma PB ha la media più alta sui tre livelli. Eccezioni: Medio seme 3 (PD 680 contro PB 647), Medio seme 5 (S4 811 contro PB 793). Nove strategie non bastano a dimostrare una dominanza.
- **«Evacua tutti a T+0» non è più la scelta civile migliore.** PB ≥ S5 in 24 confronti su 24 (8 semi × 3 livelli): meglio in 16, pari in 8. Differenza media: +39 Facile, +56 Medio, +47 Esperto, vicina alla soglia di rumore di 50. Al Facile, semi 1–3, PB e S5 sono identici.
- **La preallerta conta quando l'evacuazione è ritardata:** all'Esperto, evacuare a T+30 dà 54 senza preallerta (PD) e 214 con (PB).
- **L'Esperto non è insensibile:** medie da 10 (solo difesa) a 218 (PB), da 0 a 336 per seme. Ma nessuna strategia supera ~220, contro 690–750 al Facile.
- **Le priorità cambiano l'esito.** Al Facile, invertirle (PC) torna al punteggio senza difesa (488): seme 1, San Rocco perde 73 case. All'Esperto difendere Le Ghiande e Le Terrazze peggiora il seme 1 (91 → 30; un mezzo perso a T+23 vicino a Pian dei Grilli) e migliora il seme 3 (233 → 307).

## Differenze rispetto al playtest 7

- S1 riproduce il playtest 7 sui tre livelli (488, 591, 157); S5 al Facile e al Medio (752, 707). All'Esperto S5 dà 149 contro 167: probabile effetto di `c777919` (mezzi che non percorrono più una strada tagliata, già annotato in `docs/TODO.md` per Coste2_gira), non verificato qui.
- S2–S4 del playtest 7 non avevano i comandi registrati: le versioni di Haiku sono ricostruzioni (S4 Medio 717 contro 564).
- Il playtest 7 non provò «preallerta tutti, evacua a T+30». Il risultato di PB non è nuovo: coincide con `docs/fase5/intasamento/README.md` (2026-10-11), dove la stessa strategia era ≥ «evacua tutti a T+0» in 18/18 confronti.

## Stranezze

- Esperto seme 1: 23 famiglie a rischio, ognuna vale ~43 punti. PB 122 contro S5 30 sono 3 famiglie.
- Esperto seme 1: due autobotti in difesa fanno peggio di nessuna difesa (30 contro 91).

## Limiti

- Priorità fisse per tutta la partita, ordini civili solo a T+0 e T+30, nessuna risposta alle crisi ×1.
- Fino a 8 semi; differenze sotto ~50 punti sono rumore.
- Il meccanismo per cui PB batte S5 non è stato verificato. Ipotesi di Haiku: meno traffico iniziale. Il README dell'intasamento indica le code a Castelvento, lontano dal fuoco: da controllare.
- La priorità «migliore» del playtest 7 non è stata riottimizzata per PA, PB, PD.

## Lettura del progettista (Claude Opus 5.5)

La ricetta del playtest 7 si è spostata, non è sparita: ora è **«preallerta tutti subito, evacua tutti a T+30, mezzi sul borgo più vicino al fuoco»**, sempre decisa al minuto zero. Il gioco premia ancora un piano uniforme per tutti i quartieri, e l'Esperto resta schiacciato sotto 220. L'obiettivo «evacua tutti non deve essere la strategia migliore» (decisione dell'utente, 2026-10-12) richiede ancora un costo dell'ordine di massa che dipende da dove e quando: è la scelta aperta tra le varianti A/B/C dei messaggeri (`docs/fase5/messaggeri/README.md`).
