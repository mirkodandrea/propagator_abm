# Suite di ricerca scenari

La suite usa `rocca::Game` da terminale: non avvia Bevy, non invia input, non apre
finestre, non riavvia processi e non scrive nei dati o nella classifica del kiosk.
Una simulazione alla volta, compilazione con un job. `nice -n 15` richiede priorità
ridotta al sistema; alcuni sandbox la negano.

## Varietà degli impatti (richiesta del 2026-10-16)

La varietà si misura sui **borghi effettivamente colpiti senza ordini**, distinguendo
persone e case e i minuti del primo impatto. Cambiare innesco o vento senza cambiare
questa distribuzione non basta. Il selettore cerca combinazioni di borghi diverse,
pressioni distribuite e impatti ravvicinati oppure successivi.

```sh
python3 tools/factory/scenarios.py generate --impacts --out results/scenario-impacts
python3 tools/factory/scenarios.py search-impacts --out results/scenario-impacts
python3 tools/factory/scenarios.py focus-impacts --source results/scenario-impacts --out results/scenario-impacts/focused
python3 tools/factory/scenarios.py run --out results/scenario-impacts/focused
python3 tools/factory/scenarios.py validate --out results/scenario-impacts/focused
python3 tools/factory/scenarios.py report --out results/scenario-impacts/focused
```

65 proposte su inneschi a nord, sud-est, valico e versante ovest, con umidità
costante e un solo cambio di vento. Durate di 240–360 min; raggio iniziale di
60–100 m, riportato in ogni ricetta. Si parte con il solo riferimento senza ordini
sul seme 2; solo dove emergono impatti distribuiti si aggiungono i semi 1 e 3 e le
quattro politiche. Le varianti mirate sono proposte di ricerca riproducibili: i
loro esiti vengono sempre misurati di nuovo.

Un impatto consistente su un borgo richiede **almeno 10 persone o 2 case colpite**.
I borghi secondari devono rappresentare almeno il **15% di persone + 2×case**,
per non contare una vittima isolata come un secondo fronte. Il filtro iniziale
richiede questi impatti su almeno due borghi nei tre semi di ricerca. Nella conferma
si richiede almeno l'80% dei cinque semi: eventuali eccezioni restano visibili, con
numero e nomi dei borghi **per ogni seme**. Una distribuzione variabile è parte del
modello stocastico; non si promette lo stesso numero di borghi in tutte le partite.
Le soglie di efficacia e sicurezza del piano restano obbligatorie in ogni seme.

Questa modalità ammette anche casi che minacciano persone senza danneggiare case:
si dichiarano i due effetti separatamente. Per la varietà del gruppo selezionato
si usano firme dei borghi colpiti, distribuzioni dei danni e distanze temporali fra
i primi impatti, dando precedenza alla copertura di borghi diversi. Non si forza
una distanza numerica di difficoltà a scapito della varietà geografica richiesta.

Il risultato distingue due qualifiche: `playable` richiede che il piano ordinario
superi tutti i filtri su ogni seme; `eligible` e `forgiving` richiedono anche il test
del ritardo. **I filtri più rigorosi non vengono abbassati per promuovere un caso.**
`variety-candidates.json` presenta separatamente casi giocabili ma meno indulgenti,
con il limite del ritardo dichiarato; `variety-cases.json`, `review-configs/` e
`review-plans/` li rendono rivedibili. Per ogni famiglia di impatti si preferisce un
caso permissivo se disponibile. `game-cases.json` conserva la selezione rigorosa.

La prova `planner=nuclei` usa i nuclei di case sparsi anziché solo il centro medio
del borgo. Considera fiamme osservate e cambio di vento annunciato nel bollettino,
assegna le squadre in base agli ETA, conserva gli incarichi già adatti e usa punti
stradali arretrati rispetto al fronte. Restano 15 spostamenti e 4 evacuazioni e
valgono tutte le regole di pausa del gioco. Non legge stati futuri del fuoco.
È una politica di prova headless, **non un automatismo aggiunto al kiosk**. Le
configurazioni esportate restano `Case` nativi e i piani sono ordini eseguibili dal
giocatore. L'altra politica adattiva mantiene la prova al centro dei borghi, con
assegnazione per ETA e posizioni arretrate; entrambi i metodi sono tracciati nelle
richieste e protetti dal fingerprint del runner.

I tre profili mostrati all’utente sono stati approvati e aggiunti a
`data/scenarios/rocca_ventosa/game.json`: il menu operatore F2 li propone insieme
ad «Attuale (predefinito)». `game` resta `medium`; la selezione si applica alla
prossima nuova visita o con «Vai al giorno 2». La demo in esecuzione non è stata
riavviata. Le fragilità del test di ritardo restano quelle sopra documentate.

## Ricerca di combinazioni giocabili

```sh
python3 tools/factory/scenarios.py generate --playable --out results/scenario-playable
python3 tools/factory/scenarios.py run --out results/scenario-playable
python3 tools/factory/scenarios.py validate --out results/scenario-playable
python3 tools/factory/scenarios.py report --out results/scenario-playable
python3 -m unittest discover -s tools/factory -p 'test_scenarios.py'
```

Il generatore propone 16 combinazioni: 12 esplorazioni su tre zone, più quattro
varianti miti ricavate dalla prima ricerca. L'innesco ovest è a (3600, 2700), più
lontano dalle case rispetto al tutorial. Gli altri sono (5590, 4370) e (5870, 4750).
Ogni caso usa **umidità del combustibile costante** e **al massimo un cambio di
vento**: il runner mantiene il calendario nativo di `Game` e verifica che coincida
con la ricetta. Direzioni **da cui** soffia il vento, velocità in km/h, coordinate
in metri. Segnalazione al minuto 30, durata totale 240 minuti, patch iniziale 60 m.

Le quattro politiche passano tutte per pausa, esagoni, percorsi ed evacuazione del
gioco, senza conoscere l'evoluzione futura:

- `none`: nessun ordine;
- `ignition`: tutte le squadre all'esagono stradale più vicino all'innesco, subito;
- `evacuate`: evacuazioni in sequenza, ordinate per distanza dal fronte e vento;
- `adaptive`: stessa priorità di evacuazione, squadre distribuite ai borghi
  minacciati; riesame ogni ricarica della pausa, **senza ridare ordini identici**,
  con al massimo **15 spostamenti e 4 evacuazioni**.

Un fallimento della politica non dimostra che il caso sia impossibile; significa
che questa ricerca non ne ha dimostrato la giocabilità con il piano provato.

## Filtri e conferma

Le soglie sono criteri di ricerca, non modifiche alle regole o al punteggio. Su
**ogni** seme 1–3, un candidato deve avere:

- almeno 15 persone e 2 case colpite senza ordini: qualcosa è davvero in gioco;
- massimo 2 persone colte e nessuna casa colpita alla segnalazione;
- prima minaccia fra 20 e 150 minuti dopo la segnalazione;
- almeno l'80% delle persone e il 25% delle case risparmiate col piano adattivo,
  nessuna unità persa e rispetto del limite di ordini;
- nessuna soluzione quasi totale con «tutti all'innesco» su tutti i semi.

`validate` prova **tutti** i candidati che superano i filtri anche sui semi 4–5,
con le quattro politiche. Poi ritarda i primi ordini di **10 minuti simulati** su
tutti e cinque i semi: chiede ancora almeno il 60% delle persone e il 10% delle
case risparmiate, senza perdere unità. Una media buona non nasconde un seme che
fallisce i criteri di giocabilità. Il ritardo è una prova di margine d'errore, non una previsione di quanto
il giocatore impiegherà nella pausa iniziale senza limite di tempo.

L'indice serve solo a ordinare i candidati ammessi: peggiori quote residue fra i
semi, persone 50%, case 30%, ettari 20%. Non sostituisce le stelle del gioco. La
selezione cerca fino a tre gradini separati di almeno `max(5, 10% dell'intervallo)`
e favorisce zone e meteo diversi. Non forza un terzo livello quando le prove non
lo sostengono. L'area bruciata può aumentare anche con interventi: le traiettorie
stocastiche divergono; si conservano tutti gli esiti, anche quelli sfavorevoli.

## Risultati e ripresa

- `report.md`: confronto e motivi di esclusione, per seme.
- `analysis.json`, `selected.json`: criteri, politiche, semi aggiuntivi e ritardo.
- `game-cases.json`: **solo** casi selezionati e confermati sui cinque semi e col
  ritardo, nel formato nativo `Case`.
- `plans/<caso>/seed-N.json`: piano con ordini e minuti dalla segnalazione, per ogni seme.
- `configs/<caso>/game.json`: configurazione completa per ciascun caso selezionato,
  con tutorial conservato e `game` impostato sul candidato. File da rivedere e
  installare successivamente: la suite non modifica quelli pubblicati.
- `runs/`: prove atomiche con fingerprint di sorgenti/dati, richiesta, danni già
  presenti alla segnalazione, tempi di prima minaccia per borgo, serie ogni 10 min,
  strade chiuse, perdite, ordini accettati (minuto, squadra, coordinate, ETA o borgo)
  e rifiutati. Questi ordini sono anche un piano concreto da esaminare.

Un riavvio riusa solo prove identiche; errori e timeout non vengono salvati come
risultati validi. Un cambiamento ai sorgenti del motore headless o ai dati invalida i risultati;
grafica, altri esempi/test e funzioni di feedback/punteggio (non invocate dalla prova)
restano fuori dal fingerprint. La conferma sulla versione corrente ha ripetuto le
50 prove dei due selezionati, tutte identiche allo screening precedente, conservato
in `screening-history/`. La precedente ricerca della sola giocabilità aveva eseguito 307 simulazioni;
questa ricerca degli impatti conserva i propri risultati in una cartella distinta.
`--timeout` limita ogni prova (240 s predefiniti); `--limit 1 --seeds 1 --policies none`
permette un controllo rapido. `--names <caso> ...` restringe il lavoro;
`--delay 10 --policies adaptive --seeds 1 2 3 4 5` ripete il test del ritardo.
Un tempo di prima minaccia negativo indica danni precedenti alla segnalazione.

La conferma è headless: divertimento, comprensione e leggibilità richiedono ancora
una prova con persone. Il focus della demo in corso resta sempre intatto.

## Esplorazione di storie meteo a più tappe

`generate` senza `--playable` conserva la ricerca sperimentale originale (12
ricette), con umidità imposta a gradini e più cambi di vento, da salvare in un'altra
cartella, ad esempio `results/scenario-suite`. È una forzante, non un modello di
essiccamento. Il runner applica queste storie anche prima della segnalazione e
non usa l'ensemble. Per queste ricette resta l'indice originale
`persone + 2 × case + ettari / 10`; non vengono esportate come casi nativi giocabili.

## Immagini comparative senza aprire il kiosk

```sh
cargo run --offline --release -p rocca --example scenario_maps -j 1 -- results/scenario-impacts/focused/variety-cases.json results/scenario-impacts/focused/images/data 2
MPLCONFIGDIR=/tmp/rocca-mpl .venv/bin/python tools/factory/scenario_images.py
```

Tre PNG in `results/scenario-impacts/focused/images/`, stessa scala e seme 2,
senza ordini. Il runner esporta arrivi reali del fuoco, esposizione delle case e
impatti sulle persone da `Game`; la mappa non inventa sagome o isocrone. I colori
indicano minuti dalla segnalazione (al min 30 dall'innesco), i dati laterali sono
una singola realizzazione, distinti dalla variabilità sui cinque semi. Terreno,
strade e case vengono dai dati del gioco; rendering Matplotlib Agg, nessuna GUI.
