# Preallerta e poi evacuazione contro evacuazione subito (Coste2_gira)

Domanda del playtest 3 (q3): con il seme 1 «evacua tutti a T+0» lascia 3 famiglie colte in casa, «preallerta e poi evacua» 1. Meccanismo o seme? Comando: `cargo run --release -p rocca --example preallerta_vs_evacua`.

| seme | piano | colte in casa (Castelvento / Pian / Ghiande) | evacuate | case |
|---|---|---|---|---|
| 1 | evacua tutti a T+0 | 0 / 0 / 3 = 3 | 204 | 71 |
| 1 | preallerta tutti, evacua a T+1:20 | 0 / 0 / 1 = 1 | 204 | 71 |
| 1 | preallerta tutti, evacua a T+0:30 | 0 / 0 / 1 = 1 | 204 | 71 |
| 2 | evacua tutti a T+0 | 0 / 0 / 3 = 3 | 202 | 73 |
| 2 | preallerta tutti, evacua a T+1:20 | 0 / 0 / 3 = 3 | 204 | 73 |
| 2 | preallerta tutti, evacua a T+0:30 | 0 / 0 / 2 = 2 | 204 | 73 |
| 3 | evacua tutti a T+0 | 0 / 0 / 3 = 3 | 204 | 65 |
| 3 | preallerta tutti, evacua a T+1:20 | 0 / 0 / 4 = 4 | 204 | 65 |
| 3 | preallerta tutti, evacua a T+0:30 | 0 / 0 / 3 = 3 | 204 | 65 |
| 4 | evacua tutti a T+0 | 0 / 0 / 4 = 4 | 204 | 67 |
| 4 | preallerta tutti, evacua a T+1:20 | 0 / 0 / 5 = 5 | 204 | 67 |
| 4 | preallerta tutti, evacua a T+0:30 | 0 / 0 / 4 = 4 | 204 | 67 |
| 5 | evacua tutti a T+0 | 0 / 0 / 3 = 3 | 204 | 73 |
| 5 | preallerta tutti, evacua a T+1:20 | 0 / 0 / 1 = 1 | 204 | 73 |
| 5 | preallerta tutti, evacua a T+0:30 | 0 / 0 / 1 = 1 | 204 | 73 |
| 6 | evacua tutti a T+0 | 0 / 0 / 2 = 2 | 204 | 73 |
| 6 | preallerta tutti, evacua a T+1:20 | 0 / 0 / 3 = 3 | 204 | 73 |
| 6 | preallerta tutti, evacua a T+0:30 | 0 / 0 / 2 = 2 | 204 | 73 |

**Medie su 6 semi** (famiglie colte in casa): evacua a T+0: 3,0; preallerta ed evacua a T+1:20: 2,8; preallerta ed evacua a T+0:30: 2,2. L'ordine cambia da seme a seme. Le famiglie colte sono sempre a Le Ghiande: sono quelle che non partono nemmeno con l'ordine. **È rumore del seme, non un vantaggio della preallerta.** Il debrief ora dice quante famiglie sono rimaste a casa («Alla fine, su 30 famiglie: 25 in salvo, 1 ancora in viaggio, 4 rimaste a casa»).
