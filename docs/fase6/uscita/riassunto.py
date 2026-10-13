"""Riassunto della capacità di evacuazione (2026-10-13).

Legge i grezzi di `examples/sindaca.rs` (GREZZI=g<cap>.csv, cap 0 = nessun
limite) e stampa, per capacità: punti medi dei piani con la sindaca, quante
volte «preallerta tutti, evacua appena autorizzato» perde > 20 punti contro
«preallerta ed evacua solo i minacciati» e contro «preallerta i vicini
(1,5 km)», e i punti medi del vecchio «evacua tutti subito».

    python3 docs/fase6/uscita/riassunto.py <cartella con g*.csv>
"""
import csv
import sys
from collections import defaultdict
from pathlib import Path

TUTTI = "preallerta tutti, evacua appena autorizzato"
MINACCIATI = "preallerta ed evacua solo i minacciati"
VICINI = "preallerta i vicini (1,5 km), evacua appena autorizzato"
SUBITO = "evacua appena autorizzato"
SBAGLIATE = "preallerta tutti, evacua appena autorizzato, priorità sbagliate"
VECCHIO = "evacua tutti subito"

cartella = Path(sys.argv[1])
righe = []
for f in sorted(cartella.glob("g*.csv"), key=lambda p: int(p.stem[1:])):
    cap = int(f.stem[1:])
    punti = defaultdict(dict)  # (caso, seme) -> piano -> punti
    for r in csv.DictReader(open(f)):
        punti[(r["caso"], r["seme"])][r["piano"]] = int(r["punti"])
    def media(p):
        v = [x[p] for x in punti.values() if p in x]
        return sum(v) / len(v)
    def perde(a, b):
        return sum(1 for x in punti.values() if a in x and b in x and x[b] - x[a] > 20)
    righe.append((
        "nessuno" if cap == 0 else f"{cap}/min",
        len(punti),
        *(round(media(p)) for p in (VECCHIO, SUBITO, TUTTI, VICINI, MINACCIATI, SBAGLIATE)),
        perde(TUTTI, MINACCIATI),
        perde(TUTTI, VICINI),
        perde(VECCHIO, MINACCIATI),
    ))

print("| capacità | incendi | evacua tutti subito (vecchie regole) | evacua appena autorizzato | preallerta tutti + evacua aut. | preallerta vicini 1,5 km + evacua aut. | preallerta ed evacua solo i minacciati | priorità sbagliate | «preallerta tutti» perde > 20 contro «solo i minacciati» | … contro «vicini 1,5 km» | vecchio «evacua tutti» perde > 20 contro «solo i minacciati» |")
print("|---" * 11 + "|")
for r in righe:
    print("| " + " | ".join(str(x) for x in r) + " |")
