# Aggrega raw.txt: media dei semi 1-3 per livello e strategia.
import re, sys, collections
path = sys.argv[1]
rows = collections.OrderedDict()
pat = re.compile(r"(\w+) \| seme (\d) \| (\S+) \| punteggio A: (\d+).*?\| autorizzazioni della sindaca A: (.*?) \| stelle A: persone (\d) · case (\d) · tempismo (\d) · misura (\d)")
for line in open(path, encoding="utf-8"):
    m = pat.search(line)
    if not m:
        print("NON PARSATA:", line.strip()); continue
    L, S, N, sc, auth, p, c, t, mi = m.groups()
    rows.setdefault((N, L), []).append((int(sc), int(p), int(c), int(t), int(mi), S, auth))
print(f"{'strategia':<40}{'liv':<8}{'punti':>6}{'pers':>6}{'case':>6}{'temp':>6}{'mis':>6}   sem:punti")
for (N, L), v in rows.items():
    n = len(v)
    avg = lambda i: sum(x[i] for x in v) / n
    per = " ".join(f"{x[5]}:{x[0]}" for x in v)
    print(f"{N:<40}{L:<8}{avg(0):>6.0f}{avg(1):>6.1f}{avg(2):>6.1f}{avg(3):>6.1f}{avg(4):>6.1f}   {per}")
