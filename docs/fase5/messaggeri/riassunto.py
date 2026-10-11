import sys,collections
rows=[l.split('|')[1:-1] for l in open(sys.argv[1]) if l.startswith('| ') and not l.startswith('| caso')]
by=collections.defaultdict(dict)
for r in rows:
    r=[x.strip() for x in r]; by[r[0]][r[3]]=(int(r[4]),int(r[5]),int(r[6]),r[2])
P=["nessun ordine","evacua tutti","evacua i minacciati","preallerta tutti","preallerta tutti, evacua i minacciati"]
wins=collections.Counter(); worse=[]; better=0
for c,d in by.items():
    best=max(P,key=lambda p:d[p][0]); wins[best]+=1
    t=d["evacua tutti"][0]; m=d["evacua i minacciati"][0]
    if t<m-20: worse.append(f"{c} {t}<{m}")
    if t>m+20: better+=1
print(" migliore:",dict(wins))
print(" tutti < minacciati (>20):",len(worse),"| tutti > minacciati:",better, "|", ", ".join(worse))
print(" medi:",{p:round(sum(d[p][0] for d in by.values())/len(by)) for p in P}, "fuori:",sum(d[p][2] for d in by.values() for p in P))
