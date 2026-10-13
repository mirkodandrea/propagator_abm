# Playtest headless «ottimizzatore» con le regole della sindaca (Haiku 5.5, 2026-10-13)

Haiku ha giocato da ottimizzatore con il runner `target/release/rocca` (commit `e9018b4`) sui tre livelli `easy`, `medium` ed `expert`, semi 1–3: 14 strategie, 126 esecuzioni. File: `run.sh`, `raw.txt`, `summary.txt`, `analysis.txt`, `logs/`. Il testo è di Haiku, che non poteva creare questo file; l'ho scritto io (Claude Opus 5.5) aggiungendo le verifiche e le valutazioni.

**Verifica:** ho rieseguito s06, s11 e s12 su Medio ed Esperto, semi 1–3. Coincidono tutti (es. Medio s06 494/617/615, s11 587/617/732, s12 494/679/732).

Priorità «vicine» = San Rocco > Castelvento > Le Terrazze > Fornaci > Le Ghiande > Pian dei Grilli; «lontane» = l'inverso. Una «e» chiesta a T+0 parte all'autorizzazione della sindaca.

| strategia | Facile | Medio | Esperto | P | C | T | M |
|---|---|---|---|---|---|---|---|
| s00 nessun ordine | 0 | 0 | 0 | 1,0 | 1,0 | 1,0 | 5,0 |
| s06 vicine, preallerta tutti + evacua all'autorizzazione | **694** | 575 | 497 | 3,6 | 3,3 | 5,0 | 3,3 |
| s11 vicine, evacua all'autorizzazione (senza preallerta) | 493 | **645** | **548** | 3,3 | 3,4 | 3,6 | 3,4 |
| s12 vicine, solo preallerta a tutti | 657 | 635 | 508 | 3,6 | 3,4 | 5,0 | 5,0 |
| s13 vicine, evacua tutti a T+30 | 493 | 488 | 396 | 2,7 | 3,4 | 3,0 | 3,4 |
| s07 lontane, preallerta + evacua | 412 | 238 | 167 | 2,3 | 1,0 | 4,8 | 3,8 |
| s08 vicine, nessun ordine civile | 252 | 204 | 168 | 1,2 | 3,4 | 1,0 | 5,0 |
| s02/s10 evacua all'autorizzazione, senza priorità / lontane | 451 | 480 | 470 | 3,7 | 1,0 | 3,6 | 3,9 |

(Punti: media dei semi 1–3. Stelle: media delle 9 esecuzioni. Tabella completa in `summary.txt`.)

## Risposte

1. **Nessuna strategia vince sempre.** Il migliore cambia con il livello e con il seme. Facile: s06/s12. Medio: s11/s12. Esperto: s11 nei semi 1 e 2, s04 nel seme 3.
2. **Le priorità contano molto.** s06 contro s07 (priorità invertite): 694/575/497 contro 412/238/167, e la stella Case scende da 3,3 a 1,0. Senza ordini civili, le priorità giuste valgono circa 200 punti (s08 contro s00).
3. **Esperto: si può fare bene, ma non salvare le case.** San Rocco è autorizzata a T+0 in tutti i semi, Castelvento a T+12–26. Il migliore arriva a 668 punti (39 famiglie salvate su 47), ma nessuna partita salva più di 29 case su 100. Evacuare 30 minuti dopo costa circa 150 punti (s13 contro s11).
4. **Stelle:**
   - Misura vale 5 quando non si fa nulla.
   - Case vale 1 senza priorità.
   - Correlazione con il punteggio: Persone 0,91, Tempismo 0,72, Case 0,46, Misura −0,39.
5. **Una regola quasi ovvia:** «priorità ai paesi vicini + preallerta a tutti» (s12) arriva al 93–98 % del migliore su tutti i livelli. Evacuare in più non aggiunge punti in media (s12 600, s06 589, s11 562 sulle 9 esecuzioni).

## Valutazione (Opus)

- **Confermato, ed è il punto importante:** su questi tre livelli dare anche l'evacuazione a chi è già preallertato può peggiorare l'esito. Medio, seme 3: solo preallerta 732, preallerta + evacua 615; seme 2: 679 contro 617. Insegnerebbe «non evacuare mai». Il meccanismo non è verificato. Ipotesi da controllare nell'ordine:
  1. il giro di evacuazione delle pattuglie sostituisce quello di preallerta e le sparpaglia;
  2. le famiglie con l'ordine di evacuazione «aspettano di vedere il fuoco» più a lungo di quelle solo preallertate, che partono da sole;
  3. le code verso il porto, l'unica uscita.

  Sui 82 incendi di `docs/fase6/sindaca` valeva il contrario (preallerta + evacua 615, solo preallerta 539): i tre livelli scelti si comportano diversamente dalla media.
- **Misura a 5 senza ordini:** è voluto, perché l'inazione la puniscono Persone e Tempismo, ma si legge male. Proposta: Misura «—» quando non è stata data nessuna evacuazione, così come Tempismo è «—» quando il fuoco non arriva alle case.
- **Senza priorità nessuna difesa, e i mezzi mandati lontano non fanno niente:** è voluto. Il coordinatore non presidia paesi che il fuoco non minaccia. Non lo considero un difetto da correggere.
