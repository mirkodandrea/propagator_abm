---
name: synthetic-territory
description: Costruire o modificare un territorio sintetico ma credibile per un gioco di rischio naturale (DEM, combustibili/uso del suolo, borghi e case, strade, coste, aree di raccolta) e cercare scenari (inneschi, vento, umidità) che diano impatti vari. Usare quando si ritocca la mappa, si aggiunge una strada o un valico, si sistema il layout dei borghi, si generano varianti da provare o immagini comparative degli scenari.
---

# Territori e scenari sintetici

Un territorio inventato evita di insegnare cose false su un luogo vero. Ma deve
**sembrare** vero a un operatore di protezione civile e far nascere i compromessi del
gioco: due borghi minacciati insieme, squadre che non bastano, un cambio di vento che
ribalta il piano.

## 1. Da dove partire

- Meglio partire da **dati reali** ritagliati che da rumore puro: DEM e combustibili dello
  stesso reticolo, per esempio le COG europee di PROPAGATOR (20 m, `eu_fuel12`; accesso nelle
  memorie). Poi si deformano e si dichiarano immaginari.
- **Formato pubblicato:** raster binari con la stessa griglia (quota, combustibile,
  copertura), vettori in metri in un sistema locale (strade e case in stile OSM, popolazione)
  e un file di gioco (stazioni, squadre, aree di raccolta, tutorial, eventi).
- Il terreno del rendering è un file a parte, più fine e liscio, **derivato** da quello del
  modello. Si interpola solo la terra: si riempie il mare con la terra più vicina, poi si
  rimette la maschera del mare.

## 2. Come si modifica

- **Script autonomi e ripetibili**, uno per intervento (`valico.py`, `costa_est.py`,
  `strade.py`, `borghi.py`, `servizi.py`): NumPy e SciPy, nessuna pipeline gigante.
  Ognuno ha nel docstring la data e la richiesta dell'utente.
- **Idempotenti:** rieseguiti danno lo stesso file, oppure non trovano più niente da fare.
  Se serve, ripartono dal file precedente preso dalla storia git.
- Si toccano **i file pubblicati**, non una copia. Per provare una variante si crea una
  cartella di scenario con **link simbolici** all'originale più i soli file cambiati, e la
  si sceglie con una variabile (in Roccavento `ROCCA_SCENARIO=<id>`). Vale per CLI, test e
  gioco. Niente copie dei dati.
- Dopo ogni modifica del territorio **si rifanno le prove** di bilanciamento
  (`sim-game-balance`), perché le tarature fatte sul territorio vecchio non valgono più.
- Nei bundle desktop i link simbolici vanno dereferenziati (`cp -L`): Windows non li ricrea.

## 3. Che cosa rende credibile un territorio

- **Vegetazione:** le macchie hanno bordi frastagliati (rumore frattale sulla distanza),
  frange di una classe intermedia e qualche isola della vegetazione vecchia. Una fascia
  dritta sembra dipinta. Attorno alle strade la vegetazione va diradata, perché le strade
  si leggano.
- **Coste:** baie, promontori, spiaggia da un lato e scogliera dall'altro, rumore 2-D. Si
  trasforma solo terra in mare, lontano da tutto ciò che è simulato.
- **Strade:**
  - si instradano sul DEM a costo minimo (pendenza al quadrato, tetto di pendenza
    carrabile, tornanti dove servono), e si verifica se il collegamento esiste davvero con
    pendenze da camion;
  - **gli incroci sono vertici comuni**: una rete che unisce le vie solo dove i vertici
    coincidono trasforma un attraversamento in un ponte senza svolta;
  - nessun incrocio a stella;
  - una strada nuova **non è un tagliafuoco**, a meno che non lo si decida: il
    combustibile sotto non si tocca.
- **Borghi:**
  - le case sono orientate sulla strada più vicina, staccate dalla strada e tra loro,
    calcolate sulla **dimensione disegnata** (se il rendering scala le impronte, si usa
    quella);
  - spostamento minimo rispetto alla posizione originale;
  - quartieri diversi;
  - servizi (campo sportivo a misura reale, parcheggi, cimitero) ruotati ognuno a modo
    suo e collegati da una pista.
- **Aree di raccolta** scelte a mano, facili da raggiungere e fuori dal bosco. Mai in
  mezzo alla vegetazione: l'utente l'ha bocciato come irrealistico. Se manca la scelta a
  mano, l'automatica è la più vicina su strada con poco combustibile intorno.
- **Elementi scenici** (rocca, torre, porto, barche) in un anello non combustibile, se non
  devono bruciare.

## 4. Il territorio deve far giocare

- **Ogni zona deve poter essere raggiunta** da qualche evento: non deve essere sempre
  evidente che una zona è al sicuro. Se un gruppo di borghi non viene mai colpito, si
  cambia il territorio, per esempio con un valico di vegetazione molto infiammabile che il
  fuoco attraversa quando gira il vento, non le regole.
- Servono **finestre d'intervento**: strade che permettono di attaccare il fronte più veloce
  entro la prima ora. Non si ottimizza il numero di case colpite.
- Prima di aggiungere code e traffico come costo dell'evacuazione, si misura se a questa
  scala possono davvero diventare il collo di bottiglia.

## 5. Ricerca degli scenari

- Uno scenario è una **ricetta riproducibile**: innesco (patch, raggio), vento (velocità,
  provenienza, un cambio annunciato), umidità, durata, ritardo di segnalazione.
- Si cerca la **varietà degli impatti** senza ordini: quali borghi vengono colpiti, persone
  e case, i minuti del primo impatto.
  - Una soglia di impatto consistente (es. ≥ 10 persone o ≥ 2 case).
  - Un borgo secondario deve pesare almeno una quota minima del danno.
  - Prima un seme, poi la conferma su 5 semi, con le eccezioni visibili seme per seme.
- Per ogni candidato servono anche i filtri del piano: un piano ragionevole deve funzionare
  senza perdere unità, anche con qualche minuto di ritardo. Le fragilità si scrivono, non
  si nascondono.
- Si classificano per difficoltà (facile, medio, difficile, esperto) e si producono
  **immagini comparative** headless (Matplotlib `Agg`): stessa isola, scala e seme, arrivi
  del fuoco, borghi colpiti, innesco e vento. Le stesse immagini fanno da icone nel
  selettore.

## 6. Esempio: Isola di Roccavento

`docs/02-TERRITORIO.md` (com'è fatta e come si modifica); `tools/factory/*.py`;
`tools/factory/SCENARI.md` (ricerca degli scenari: `scenarios.py generate|search-impacts|
focus-impacts|run|validate|report`); immagini: `crates/rocca/examples/scenario_maps.rs` +
`tools/factory/scenario_images.py` (comandi in `CLAUDE.md`).
