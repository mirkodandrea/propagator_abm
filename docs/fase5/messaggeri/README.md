# Ordini portati casa per casa, sul layout 5 (headless, 2026-10-11)

Decisione dell'utente (2026-10-11): restare sul layout 5 e provare lì la proposta di `../layout6/README.md`. Si tratta di squadre limitate che portano gli ordini, così ordinare tutti ritarda l'avviso al borgo minacciato. **Prova headless: nel gioco l'opzione è spenta e nulla cambia.**

**Cosa cambia:**
- `crates/rocca/src/messengers.rs` e `game.rs`: con `Game::messengers` impostato, un ordine o una preallerta apre un «giro» del quartiere.
  - Le famiglie si avvisano a voce, dalla più vicina al centro, al ritmo delle squadre assegnate.
  - Chi ha già ricevuto la preallerta sente l'ordine sul proprio canale, entro 120 s come prima.
  - Con `broadcast`, anche chi ha l'allerta sul telefono lo riceve così (45 % delle famiglie).
- `crates/abm/src/lib.rs`, `Abm::tell_in_person`: avviso consegnato subito, senza passare dal canale.
- `crates/rocca/examples/strategie_civili.rs`: variabile `MESSAGGERI=squadre,famiglie_al_minuto,broadcast,per_priorità`.

**Riproduzione:**

```
DEST=<cartella> sh docs/fase5/layout5/installa_l5.sh
cd <cartella>/l5 && MESSAGGERI=4,1,1,0 <repo>/target/release/examples/strategie_civili --semi 1 > A_seme1.md
python3 docs/fase5/messaggeri/riassunto.py A_seme1.md
```

Tutti i 36 incendi. «Minacciati» sono i quartieri che lo stesso incendio senza ordini raggiunge, ed è anche la priorità dei mezzi.

## Risultati (seme 1, incendi che colpiscono qualcuno: 26)

| messaggeri | evacua tutti peggio di evacua i minacciati (> 20 punti) | punti medi: evacua tutti / evacua i minacciati / preallerta tutti |
|---|---|---|
| nessuno (canali, come oggi) | 0 | 671 / 671 / 632 |
| **A: 4 squadre, 1 famiglia/min, allerta sul telefono, squadre divise in parti uguali** | **9** | 629 / **662** / 588 |
| B: come A, senza allerta sul telefono | 16 | 513 / 635 / 465 |
| C: come B, tutte le squadre sul quartiere con priorità più alta | 0 | 646 / 646 / 607 |
| D: 8 squadre, 2 famiglie/min, allerta sul telefono, parti uguali | 2 | 664 / 670 / 625 |

**Variante A su altri semi:** 12 incendi su 28 (seme 2) e 12 su 26 (seme 3). «Evacua tutti» non batte mai «evacua i minacciati» di più di 20 punti, salvo 2 casi nel seme 2.

1. **Ordinare tutti ora costa davvero.** Nella variante A perde fino a 350 punti (Coste3, seme 3) e 33–50 in media. Il costo nasce dove il borgo minacciato riceve l'avviso più tardi, perché le squadre sono altrove. Non è una regola del punteggio.
2. **Facile (Rocco3):** 779 con «evacua tutti», 867 con «evacua i minacciati».
3. **Esperto (Coste2_gira):** resta 567 con ogni piano civile. Il fuoco è troppo rapido perché l'avviso conti.
4. **Medio (Forna3_gira):** sul layout 5 non colpisce nessuno, i livelli vanno riscelti.
5. **Squadre secondo le priorità (C): il costo sparisce**, perché qui le priorità sono già i quartieri giusti, scelti col senno di poi. Con la regola C il costo ricadrebbe su priorità sbagliate (non misurato).
6. La preallerta a tutti consuma le squadre come un ordine e perde il vantaggio che aveva.
7. Famiglie colte in strada: 0–2 per variante. La coda resta senza costo.

## Incognite

- I parametri (4 squadre, 1 famiglia al minuto, ordine dal centro verso l'esterno) sono ipotesi. Da verificare con chi conosce i piani comunali, e anche se e quando IT-alert si usa per gli incendi.
- La preallerta portata casa per casa forse non è realistica: si potrebbe fare con altoparlanti e media, senza squadre.
- Nel gioco servirebbe mostrarlo: le squadre di avviso e quanta parte di ogni quartiere è stata avvisata.

## Proposta (una, da decidere)

Variante A nel gioco, sul layout 5: 4 squadre divise in parti uguali e allerta sul telefono attiva. Insieme vanno riscelti i livelli (Medio) e va mostrato l'avanzamento degli avvisi su ogni scheda di quartiere. Prima di farlo, decidere fra parti uguali (A) e priorità (C).
