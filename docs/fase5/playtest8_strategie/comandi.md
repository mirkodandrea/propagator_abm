# Playtest 8: comandi e output grezzi

Tutti i numeri del report vengono da questi comandi. Binario: `target/release/rocca` (compilato 2026-10-12 12:26, non ricompilato; ultimo commit c59decf). Nessun file di codice modificato.

## 1. Riferimento (coincide con il playtest 7)

```
EVAC="0:Castelvento=evacua,0:Pian dei Grilli=evacua,0:Fornaci=evacua,0:San Rocco=evacua,0:Le Terrazze=evacua,0:Le Ghiande=evacua"
target/release/rocca Rocco3 --seme 1 --priorita "San Rocco" --civili "$EVAC"
```

Output (estratto): `punteggio A: 869 su 1000 (famiglie salvate 10 di 11, case risparmiate 59 di 76)`.

## 2. Batteria principale: semi 1-3, tutte le strategie, tre livelli

Script: `run8.sh` (eseguito con `./run8.sh > raw_semi1-3.txt`, circa 6,5 min). Output completo: `raw_semi1-3.txt`.

Definizioni usate (ricostruite: il playtest 7 non registra i comandi esatti di S2-S4):
- Piu vicino (S2, S3, S4): Facile `San Rocco`; Medio `Fornaci`; Esperto `Pian dei Grilli` (0,4 km dal fuoco al T+28 nel registro di `Coste2_gira --seme 1 --registro`).
- Migliore pl7 (S5, PA, PB, PD): Facile `San Rocco`; Medio `Fornaci`; Esperto `Le Ghiande,Le Terrazze`.
- Minacciati (ordine di evacuazione a T+0 in S4, PA): quartieri con case colpite > 0 nel caso senza ordini, seme 1: Facile Castelvento, San Rocco; Medio Fornaci, San Rocco, Le Terrazze; Esperto Pian dei Grilli, Le Ghiande.
- Priorita invertita (PC): Facile `Pian dei Grilli` (0 case colpite nel caso senza ordini in tutti i semi 1-3); Medio e Esperto `Castelvento` (Esperto: 0 case colpite in tutti i semi 1-3; Medio: 0, 0 e 3 case colpite nei semi 1, 2, 3).
- PA = preallerta tutti a T+0, evacua i minacciati a T+0, migliore priorita. PB = preallerta tutti a T+0, evacua tutti a T+30, migliore priorita. PD = evacua tutti a T+30, migliore priorita. PE = evacua tutti a T+0 senza priorita (uguale a S1, controllo di determinismo).

Comando tipo (esempio Esperto, S5, seme 1):
```
target/release/rocca Coste2_gira --seme 1 --priorita "Le Ghiande,Le Terrazze" --civili "$EVAC"
```

## 3. Semi 4-8 (extra, per ridurre il rumore)

Script: `extra_semi4-8.sh`, output `raw_semi4-8.txt`. Strategie: S1, S3, S4, S5, PA, PB, PD. In questo script S3 e S4 usano la migliore priorita (non la piu vicina): per Facile e Medio coincide, per l Esperto no. Nei semi 1-3 S3/S4 dell Esperto usano Pian dei Grilli.

## 4. Punteggi grezzi (una riga per run; livello, seme, strategia)

```
Rocco3 seme 1 | S1 evacua tutti T+0, nessuna priorita |  445
Rocco3 seme 1 | S2 evacua tutti T+0 + difesa piu vicino |  869
Rocco3 seme 1 | S3 solo difesa piu vicino, nessun ordine |  360
Rocco3 seme 1 | S4 misto (evacua minacciati T+0, preallerta altri, evacua altri T+30) + piu vicino |  869
Rocco3 seme 1 | S5 evacua tutti T+0 + migliore S5 pl7 |  869
Rocco3 seme 1 | PA preallerta tutti T+0, evacua minacciati T+0 + migliore |  869
Rocco3 seme 1 | PB preallerta tutti T+0, evacua tutti T+30 + migliore |  869
Rocco3 seme 1 | PC evacua tutti T+0 + priorita invertite |  445
Rocco3 seme 1 | PD evacua tutti T+30 + migliore (ritardo) |  869
Rocco3 seme 1 | PE evacua tutti T+0, nessuna priorita (= S1) |  445
Forna3_gira seme 1 | S1 evacua tutti T+0, nessuna priorita |  571
Forna3_gira seme 1 | S2 evacua tutti T+0 + difesa piu vicino |  703
Forna3_gira seme 1 | S3 solo difesa piu vicino, nessun ordine |  217
Forna3_gira seme 1 | S4 misto (evacua minacciati T+0, preallerta altri, evacua altri T+30) + piu vicino |  731
Forna3_gira seme 1 | S5 evacua tutti T+0 + migliore S5 pl7 |  703
Forna3_gira seme 1 | PA preallerta tutti T+0, evacua minacciati T+0 + migliore |  731
Forna3_gira seme 1 | PB preallerta tutti T+0, evacua tutti T+30 + migliore |  774
Forna3_gira seme 1 | PC evacua tutti T+0 + priorita invertite |  571
Forna3_gira seme 1 | PD evacua tutti T+30 + migliore (ritardo) |  703
Forna3_gira seme 1 | PE evacua tutti T+0, nessuna priorita (= S1) |  571
Coste2_gira seme 1 | S1 evacua tutti T+0, nessuna priorita |  91
Coste2_gira seme 1 | S2 evacua tutti T+0 + difesa piu vicino |  91
Coste2_gira seme 1 | S3 solo difesa piu vicino, nessun ordine |  30
Coste2_gira seme 1 | S4 misto (evacua minacciati T+0, preallerta altri, evacua altri T+30) + piu vicino |  91
Coste2_gira seme 1 | S5 evacua tutti T+0 + migliore S5 pl7 |  30
Coste2_gira seme 1 | PA preallerta tutti T+0, evacua minacciati T+0 + migliore |  30
Coste2_gira seme 1 | PB preallerta tutti T+0, evacua tutti T+30 + migliore |  122
Coste2_gira seme 1 | PC evacua tutti T+0 + priorita invertite |  91
Coste2_gira seme 1 | PD evacua tutti T+30 + migliore (ritardo) |  0
Coste2_gira seme 1 | PE evacua tutti T+0, nessuna priorita (= S1) |  91
Rocco3 seme 2 | S1 evacua tutti T+0, nessuna priorita |  467
Rocco3 seme 2 | S2 evacua tutti T+0 + difesa piu vicino |  665
Rocco3 seme 2 | S3 solo difesa piu vicino, nessun ordine |  315
Rocco3 seme 2 | S4 misto (evacua minacciati T+0, preallerta altri, evacua altri T+30) + piu vicino |  665
Rocco3 seme 2 | S5 evacua tutti T+0 + migliore S5 pl7 |  665
Rocco3 seme 2 | PA preallerta tutti T+0, evacua minacciati T+0 + migliore |  665
Rocco3 seme 2 | PB preallerta tutti T+0, evacua tutti T+30 + migliore |  665
Rocco3 seme 2 | PC evacua tutti T+0 + priorita invertite |  467
Rocco3 seme 2 | PD evacua tutti T+30 + migliore (ritardo) |  665
Rocco3 seme 2 | PE evacua tutti T+0, nessuna priorita (= S1) |  467
Forna3_gira seme 2 | S1 evacua tutti T+0, nessuna priorita |  603
Forna3_gira seme 2 | S2 evacua tutti T+0 + difesa piu vicino |  772
Forna3_gira seme 2 | S3 solo difesa piu vicino, nessun ordine |  325
Forna3_gira seme 2 | S4 misto (evacua minacciati T+0, preallerta altri, evacua altri T+30) + piu vicino |  772
Forna3_gira seme 2 | S5 evacua tutti T+0 + migliore S5 pl7 |  772
Forna3_gira seme 2 | PA preallerta tutti T+0, evacua minacciati T+0 + migliore |  772
Forna3_gira seme 2 | PB preallerta tutti T+0, evacua tutti T+30 + migliore |  811
Forna3_gira seme 2 | PC evacua tutti T+0 + priorita invertite |  603
Forna3_gira seme 2 | PD evacua tutti T+30 + migliore (ritardo) |  772
Forna3_gira seme 2 | PE evacua tutti T+0, nessuna priorita (= S1) |  603
Coste2_gira seme 2 | S1 evacua tutti T+0, nessuna priorita |  147
Coste2_gira seme 2 | S2 evacua tutti T+0 + difesa piu vicino |  147
Coste2_gira seme 2 | S3 solo difesa piu vicino, nessun ordine |  0
Coste2_gira seme 2 | S4 misto (evacua minacciati T+0, preallerta altri, evacua altri T+30) + piu vicino |  147
Coste2_gira seme 2 | S5 evacua tutti T+0 + migliore S5 pl7 |  111
Coste2_gira seme 2 | PA preallerta tutti T+0, evacua minacciati T+0 + migliore |  111
Coste2_gira seme 2 | PB preallerta tutti T+0, evacua tutti T+30 + migliore |  184
Coste2_gira seme 2 | PC evacua tutti T+0 + priorita invertite |  147
Coste2_gira seme 2 | PD evacua tutti T+30 + migliore (ritardo) |  0
Coste2_gira seme 2 | PE evacua tutti T+0, nessuna priorita (= S1) |  147
Rocco3 seme 3 | S1 evacua tutti T+0, nessuna priorita |  553
Rocco3 seme 3 | S2 evacua tutti T+0 + difesa piu vicino |  722
Rocco3 seme 3 | S3 solo difesa piu vicino, nessun ordine |  391
Rocco3 seme 3 | S4 misto (evacua minacciati T+0, preallerta altri, evacua altri T+30) + piu vicino |  722
Rocco3 seme 3 | S5 evacua tutti T+0 + migliore S5 pl7 |  722
Rocco3 seme 3 | PA preallerta tutti T+0, evacua minacciati T+0 + migliore |  722
Rocco3 seme 3 | PB preallerta tutti T+0, evacua tutti T+30 + migliore |  722
Rocco3 seme 3 | PC evacua tutti T+0 + priorita invertite |  553
Rocco3 seme 3 | PD evacua tutti T+30 + migliore (ritardo) |  722
Rocco3 seme 3 | PE evacua tutti T+0, nessuna priorita (= S1) |  553
Forna3_gira seme 3 | S1 evacua tutti T+0, nessuna priorita |  600
Forna3_gira seme 3 | S2 evacua tutti T+0 + difesa piu vicino |  647
Forna3_gira seme 3 | S3 solo difesa piu vicino, nessun ordine |  180
Forna3_gira seme 3 | S4 misto (evacua minacciati T+0, preallerta altri, evacua altri T+30) + piu vicino |  647
Forna3_gira seme 3 | S5 evacua tutti T+0 + migliore S5 pl7 |  647
Forna3_gira seme 3 | PA preallerta tutti T+0, evacua minacciati T+0 + migliore |  647
Forna3_gira seme 3 | PB preallerta tutti T+0, evacua tutti T+30 + migliore |  647
Forna3_gira seme 3 | PC evacua tutti T+0 + priorita invertite |  600
Forna3_gira seme 3 | PD evacua tutti T+30 + migliore (ritardo) |  680
Forna3_gira seme 3 | PE evacua tutti T+0, nessuna priorita (= S1) |  600
Coste2_gira seme 3 | S1 evacua tutti T+0, nessuna priorita |  233
Coste2_gira seme 3 | S2 evacua tutti T+0 + difesa piu vicino |  233
Coste2_gira seme 3 | S3 solo difesa piu vicino, nessun ordine |  0
Coste2_gira seme 3 | S4 misto (evacua minacciati T+0, preallerta altri, evacua altri T+30) + piu vicino |  233
Coste2_gira seme 3 | S5 evacua tutti T+0 + migliore S5 pl7 |  307
Coste2_gira seme 3 | PA preallerta tutti T+0, evacua minacciati T+0 + migliore |  307
Coste2_gira seme 3 | PB preallerta tutti T+0, evacua tutti T+30 + migliore |  336
Coste2_gira seme 3 | PC evacua tutti T+0 + priorita invertite |  233
Coste2_gira seme 3 | PD evacua tutti T+30 + migliore (ritardo) |  161
Coste2_gira seme 3 | PE evacua tutti T+0, nessuna priorita (= S1) |  233
Rocco3 seme 4 | S1 |  389
Rocco3 seme 4 | S5 |  638
Rocco3 seme 4 | S3 |  171
Rocco3 seme 4 | PA |  638
Rocco3 seme 4 | PB |  716
Rocco3 seme 4 | PD |  638
Rocco3 seme 4 | S4 |  638
Forna3_gira seme 4 | S1 |  0
Forna3_gira seme 4 | S5 |  0
Forna3_gira seme 4 | S3 |  0
Forna3_gira seme 4 | PA |  0
Forna3_gira seme 4 | PB |  0
Forna3_gira seme 4 | PD |  0
Forna3_gira seme 4 | S4 |  0
Coste2_gira seme 4 | S1 |  200
Coste2_gira seme 4 | S5 |  216
Coste2_gira seme 4 | S3 |  16
Coste2_gira seme 4 | PA |  216
Coste2_gira seme 4 | PB |  249
Coste2_gira seme 4 | PD |  116
Coste2_gira seme 4 | S4 |  216
Rocco3 seme 5 | S1 |  467
Rocco3 seme 5 | S5 |  747
Rocco3 seme 5 | S3 |  420
Rocco3 seme 5 | PA |  747
Rocco3 seme 5 | PB |  747
Rocco3 seme 5 | PD |  747
Rocco3 seme 5 | S4 |  747
Forna3_gira seme 5 | S1 |  612
Forna3_gira seme 5 | S5 |  776
Forna3_gira seme 5 | S3 |  374
Forna3_gira seme 5 | PA |  811
Forna3_gira seme 5 | PB |  793
Forna3_gira seme 5 | PD |  776
Forna3_gira seme 5 | S4 |  811
Coste2_gira seme 5 | S1 |  200
Coste2_gira seme 5 | S5 |  182
Coste2_gira seme 5 | S3 |  15
Coste2_gira seme 5 | PA |  182
Coste2_gira seme 5 | PB |  248
Coste2_gira seme 5 | PD |  15
Coste2_gira seme 5 | S4 |  182
Rocco3 seme 6 | S1 |  525
Rocco3 seme 6 | S5 |  692
Rocco3 seme 6 | S3 |  211
Rocco3 seme 6 | PA |  692
Rocco3 seme 6 | PB |  780
Rocco3 seme 6 | PD |  692
Rocco3 seme 6 | S4 |  780
Forna3_gira seme 6 | S1 |  571
Forna3_gira seme 6 | S5 |  728
Forna3_gira seme 6 | S3 |  194
Forna3_gira seme 6 | PA |  765
Forna3_gira seme 6 | PB |  801
Forna3_gira seme 6 | PD |  728
Forna3_gira seme 6 | S4 |  765
Coste2_gira seme 6 | S1 |  194
Coste2_gira seme 6 | S5 |  208
Coste2_gira seme 6 | S3 |  13
Coste2_gira seme 6 | PA |  208
Coste2_gira seme 6 | PB |  208
Coste2_gira seme 6 | PD |  91
Coste2_gira seme 6 | S4 |  208
Rocco3 seme 7 | S1 |  467
Rocco3 seme 7 | S5 |  617
Rocco3 seme 7 | S3 |  209
Rocco3 seme 7 | PA |  559
Rocco3 seme 7 | PB |  671
Rocco3 seme 7 | PD |  496
Rocco3 seme 7 | S4 |  496
Forna3_gira seme 7 | S1 |  550
Forna3_gira seme 7 | S5 |  714
Forna3_gira seme 7 | S3 |  214
Forna3_gira seme 7 | PA |  764
Forna3_gira seme 7 | PB |  864
Forna3_gira seme 7 | PD |  714
Forna3_gira seme 7 | S4 |  764
Coste2_gira seme 7 | S1 |  233
Coste2_gira seme 7 | S5 |  233
Coste2_gira seme 7 | S3 |  0
Coste2_gira seme 7 | PA |  233
Coste2_gira seme 7 | PB |  233
Coste2_gira seme 7 | PD |  133
Coste2_gira seme 7 | S4 |  233
Rocco3 seme 8 | S1 |  350
Rocco3 seme 8 | S5 |  702
Rocco3 seme 8 | S3 |  177
Rocco3 seme 8 | PA |  702
Rocco3 seme 8 | PB |  790
Rocco3 seme 8 | PD |  702
Rocco3 seme 8 | S4 |  702
Forna3_gira seme 8 | S1 |  549
Forna3_gira seme 8 | S5 |  765
Forna3_gira seme 8 | S3 |  198
Forna3_gira seme 8 | PA |  784
Forna3_gira seme 8 | PB |  860
Forna3_gira seme 8 | PD |  765
Forna3_gira seme 8 | S4 |  784
Coste2_gira seme 8 | S1 |  165
Coste2_gira seme 8 | S5 |  82
Coste2_gira seme 8 | S3 |  0
Coste2_gira seme 8 | PA |  82
Coste2_gira seme 8 | PB |  165
Coste2_gira seme 8 | PD |  0
Coste2_gira seme 8 | S4 |  82
```

## 5. Controllo: il riferimento del playtest 7 ricalcolato

S1 (evacua tutti T+0, nessuna priorita): Facile 445/467/553, Medio 571/603/600, Esperto 91/147/233 (coincide con pl7: 488, 591, 157). S5 Facile 869/665/722 (pl7: 752), Medio 703/772/647 (pl7: 707). S5 Esperto 30/111/307, media 149 (pl7: 167; differenza non spiegata, vedi report).

## 6. Esempio di log (Esperto, seme 1, S5 contro PB)

```
target/release/rocca Coste2_gira --seme 1 --priorita "Le Ghiande,Le Terrazze" --civili "$EVAC" --registro
# primi eventi: Autobotte 1 colpita dal fuoco vicino a Pian dei Grilli al T+23; Le Ghiande: 1 mezzo su 2 a T+24
# S5 punteggio 30; PB (preallerta T+0, evacua T+30) punteggio 122
```
