# Analisi aggiuntiva su raw.txt: dominanza per seme, correlazione stelle-punteggio, arrivo del fuoco.
import re, sys, collections, statistics as st
pat = re.compile(r"(\w+) \| seme (\d) \| (\S+) \| punteggio A: (\d+).*?\| autorizzazioni della sindaca A: (.*?) \| stelle A: persone (\d) · case (\d) · tempismo (\d) · misura (\d)")
rows = []
for line in open("raw.txt", encoding="utf-8"):
    m = pat.search(line)
    if m:
        L, S, N, sc, auth, p, c, t, mi = m.groups()
        rows.append(dict(L=L, S=int(S), N=N, sc=int(sc), auth=auth, st=(int(p), int(c), int(t), int(mi))))
print("n esecuzioni:", len(rows))
# 1. miglior strategia per livello e seme
by = collections.defaultdict(dict)
for r in rows: by[(r["L"], r["S"])][r["N"]] = r["sc"]
for k in sorted(by):
    best = max(by[k].values())
    winners = [n for n, v in by[k].items() if v == best]
    print(f"{k[0]:<7} seme {k[1]}: max {best:>4} {winners}")
# 2. stelle vs punteggio: correlazione di Pearson su tutte le esecuzioni
def pearson(x, y):
    mx, my = st.mean(x), st.mean(y)
    sx = sum((a-mx)**2 for a in x) ** .5; sy = sum((b-my)**2 for b in y) ** .5
    return sum((a-mx)*(b-my) for a, b in zip(x, y)) / (sx*sy) if sx and sy else float("nan")
sc = [r["sc"] for r in rows]
for i, nome in enumerate(["persone", "case", "tempismo", "misura"]):
    print(f"r(punteggio, {nome}) = {pearson(sc, [r['st'][i] for r in rows]):.2f}")
# 3. quante volte misura varia a parità di punteggio
g = collections.defaultdict(set)
for r in rows: g[r["sc"]].add(r["st"])
print("punteggi con stelle diverse a parità di punteggio:", sum(1 for v in g.values() if len(v) > 1), "su", len(g))
# 4. autorizzazioni per livello (s00, nessun ordine: il fuoco non dipende dagli ordini)
for L in ["easy", "medium", "expert"]:
    for r in rows:
        if r["L"] == L and r["N"] == "s00_nessun_ordine": print("autorizzazioni", L, r["S"], ":", r["auth"])
