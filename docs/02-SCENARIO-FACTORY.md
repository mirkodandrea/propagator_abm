# 02 — Scenario Factory (Python, offline)

**Scopo:** creare **un solo territorio sintetico** convincente e pochi incendi diversi, usando il motore headless per verificare prima le propagazioni e poi le decisioni strategiche. **Non** costruire un generatore universale, né un algoritmo di ottimizzazione complesso. Gli script servono allo sviluppo; lo stand usa solo asset già preparati.

## Pipeline lineare

### A. Orografia e vegetazione, senza case o strade

Generare in **Python** alcuni candidati (prima prova: 3–5), usando un archetipo semplice: valle, crinale, versanti diversamente esposti, tratti pianeggianti e pendii. DEM continuo, pendenze credibili, rumore solo per dare dettaglio.

Distribuire **poche classi di combustibile già supportate dal modello**, correlate a quota/esposizione/pendenza e con patch continue: macchia, bosco/pineta, vegetazione rada, aree aperte. Non basta una texture variopinta: sono i valori del fuel raster a governare l'incendio.

**Umidità:** unico valore **basso e fisso** su tutto il territorio e per tutti gli esperimenti iniziali. Sceglierlo una volta, verificarne il comportamento e registrarlo; **non** usarlo come leva della ricerca. Un'eventuale distribuzione per versante è una possibilità futura, non parte del primo lavoro.

### B. Prove di incendio sul solo ambiente naturale

Per ogni candidato eseguire **sweep headless** attraverso il modello di propagazione già presente. Variabili: **posizione dell'innesco, direzione/intensità del vento, eventuale cambio di vento**. Il resto rimane fermo.

Esportare poche immagini leggibili, non un laboratorio GIS: **DEM**, **fuels**, mappe di **estensione** e **tempi di arrivo** delle fiamme, più casi in cui una diversa direzione del vento cambia chiaramente l'area colpita.

**Checkpoint umano 1 (rapido):** scegliere il paesaggio più credibile e ricco di propagazioni differenti. Se nessuno funziona, cambiare l'archetipo, non aggiungere ancora case.

### C. Posizionare abitati e strade *dopo* aver visto gli incendi

Usare l'atlante delle propagazioni per collocare con plausibilità:

- **Borgo compatto** su terreno adatto; vulnerabilità soprattutto ai margini;
- **case sparse** su un versante con esposizione differente e accessi più lenti;
- facoltativamente, **piccolo nucleo di fondovalle** con funzioni di collegamento;
- **strada principale**, collegamenti secondari e almeno un'alternativa credibile; basi e aree di attesa raggiungibili.

Le abitazioni seguono pendenze ragionevoli e la viabilità; le strade seguono l'orografia e hanno topologia realmente percorribile. Evitare strade inventate a caso e case sistemate deliberatamente in punti impossibili solo per creare difficoltà.

Non usare l'atlante per costruire un unico «percorso del fuoco scritto in anticipo»: cercare **esposizioni diverse per venti differenti**. Tutto il mondo deve restare plausibile anche senza sapere dove partirà l'incendio.

**Checkpoint umano 2 (rapido):** vedere **una sola tavola** con mappa del territorio, case e strade sopra le propagazioni principali; approvare o chiedere piccole modifiche.

### D. Rigenerare i combustibili e ripetere gli incendi

Edifici, spazi aperti e strade interrompono/modificano i combustibili: **i test di B non bastano più**. Riesportare tutti i raster/vettori, verificare rete e popolazione, poi ripetere gli sweep sul mondo completo.

Eseguire test automatici che dimostrino: il caricamento tramite `Scenario::load_by_id`, geometrie e coordinate corrette, case collegate agli abitanti, strade raggiungibili dalle unità, popolazione con percorsi plausibili verso le aree sicure, nessun mismatch fra griglia del fuoco e griglia di rendering.

### E. Testare il gameplay e fissare lo scenario

Con il **coordinatore operativo minimo** già funzionante, eseguire strategie con gli **stessi seed**: difendi Borgo / difendi case sparse / distribuisci / ripianifica alla crisi / non intervenire; aggiungere difesa strada soltanto se l'effetto sul modello è misurabile. Testare separatamente preallerta ed evacuazione.

Cercare un mondo in cui esistano **compromessi**, almeno una **finestra di recupero** e più configurazioni di vento/innesco interessanti. Se non ci sono, **ritoccare abitati e strade** o scegliere un altro ambiente. Solo alla fine congelare **un territorio + 2–3 preset di incendio** per il kiosk.

## Tooling minimo

- **Python** genera DEM, combustibili, vettori, insediamenti, popolazione, report PNG/CSV e seleziona i seed. Può usare librerie geospaziali comuni disponibili nel progetto, senza introdurre dipendenze inutili.
- **Rust** resta il motore di propagazione e ABM. Python lancia un **runner headless batch** (riusare il tooling esistente se presente; altrimenti aggiungere un piccolo comando CLI). **Non duplicare il propagatore in Python.**
- Gli export devono essere **compatibili con il formato `Scenario` esistente** (`scenario.json`, `osm.json` come nome legacy del formato vettoriale, `population.json`, `fuel.i32`, `dem.f64`, raster di rendering). Recuperare dal repository completo eventuali script già esistenti di baking e adattarli anziché riscrivere i formati.

Un solo entry point Python con sottocomandi è sufficiente, per esempio:

```text
python tools/scenario_factory.py nature --seed 4
python tools/scenario_factory.py fires --terrain 4
python tools/scenario_factory.py build-town --terrain 4 --layout 2
python tools/scenario_factory.py verify --scenario rocca_ventosa
python tools/scenario_factory.py evaluate --scenario rocca_ventosa
```

Sono **nomi proposti**, non API già disponibili. Ogni comando salva parametri e seed per rifare esattamente il risultato. Numero di candidati e incendi piccolo all'inizio; aumentarlo **solo quando i tempi di simulazione misurati lo consentono**.

## Definizione di «scenario buono»

Non massimizza case bruciate e non cerca una singola strategia vincente. Supera quattro verifiche semplici:

1. **Plausibile:** terreno, fuel, strade e abitati credibili e coerenti.
2. **Variabile:** cambiare vento o innesco altera davvero quali zone sono minacciate.
3. **Giocabile:** cambiare priorità o intervenire in una finestra critica può cambiare un risultato misurato; almeno un'alternativa comporta un sacrificio.
4. **Leggibile:** nel diorama si distinguono rapidamente due insediamenti, collegamenti, fuoco e mezzi.

Se fallisce, correggere il layout o la situazione e ripetere la prova. La decisione finale **rimane umana**, basata su immagini e risultati riproducibili.
