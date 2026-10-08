# 03 — Piano d'azione per i coding agent

**Metodo:** un agente/fase alla volta, risultati riproducibili e report brevi. **Prima i meccanismi e il territorio, poi la nuova UX.** Non lavorare in parallelo su moduli la cui interfaccia non è ancora stabile.

## Punto di partenza

Il coding agent lavora direttamente **sul repository esistente** e sui quattro documenti aggiornati di questa cartella. Il codice è una base da ispezionare, **non** un insieme di funzionalità da mantenere per forza. Non cercare altre specifiche: quelle necessarie sono qui.

La prima fase serve a verificare **nel repository reale** come funzionano propagazione, scenari, ABM, rendering e test; individuare il percorso di gameplay attuale; classificare i moduli in **RIUSA / ADATTA / RIMUOVI / DA VERIFICARE**. Lo scopo è avere una base più semplice e coerente, con una sola autorità di simulazione tra kiosk e headless. Non dare per scontato che i comandi attuali ai mezzi, gli ordini dei civili o gli effetti di difesa siano già quelli richiesti.

## Ordine del lavoro e risultati richiesti

| Fase | Incarico, in ordine | Output da mostrare / criterio per proseguire |
|---|---|---|
| **0. Audit** | Nel repo completo: compilare/testare baseline, individuare asset/script esistenti, misurare una run headless e chiarire formato scenario, efficacia difesa e semantica ordini civili. | **`audit.md` di una pagina**: RIUSA / ADATTA / RIMUOVI / DA VERIFICARE, comandi/test funzionanti, incognite. Nessuna modifica al gameplay. |
| **1. Ambiente naturale** | Python: DEM + fuels sintetici plausibili; umidità bassa fissa; runner batch di incendi variando vento/innesco. | **3–5 immagini + atlante incendi**; uno o due terreni candidati. **Checkpoint umano 1**. |
| **2. Paese** | Su un candidato: collocare quartieri, case, strade, popolazione; esportare formati esistenti; rigenerare fuels e ripetere sweep. | **Tavola del paese + report validazione** su percorribilità e propagazioni. **Checkpoint umano 2**. |
| **3. Gameplay headless** | Unificare runner; implementare priorità territoriali, coordinatore minimale con postazioni/ETA/inerzia, ordini espliciti preallerta/evacuazione, piano attivo/proposto, comparatore A/B. | **CLI/test**: invertire priorità cambia assegnazioni e almeno alcuni esiti; preallerta distinta da evacuazione; determinismo. |
| **4. Crisi e bilanciamento** | Rilevare poche crisi recuperabili; fasi ×0/×N/×1, timer reale, commit del piano; prove strategiche sul paese; scegliere 2–3 incendi. | **Replay e tabella**: una decisione presa alla crisi conta davvero, nessuna strategia sempre vincente; uno scenario definitivo. |
| **5. Nuova UX** | Riutilizzare il renderer e gli asset attuali; sostituire schermate/interazioni; mappa con priorità, anteprima, timer, ordini ai civili, debrief e **riavvio manuale**. | **Screenshots di tutta la partita**, FPS sulla macchina reale, kiosk offline **presidiato**, con comando operatore per nuova partita/riavvio e **senza reset da inattività**. |
| **6. Playtest finale** | Test con studenti/colleghi ignari del progetto, correzioni e stabilizzazione. | Durata **misurata nei playtest, senza limite rigido di 5 minuti**; ritmo sostenibile anche per partite più lunghe, prima azione intuitiva, almeno un compromesso spiegato dal giocatore, nessun blocco o crash. Riavvio manuale verificato. |

### Nota sulla sequenza

Nelle fasi 1–2 è sufficiente la propagazione **senza coordinatore**: serve prima a scegliere come costruire il paese. La qualità ludica viene verificata dopo la fase 3. Se i test della fase 4 falliscono, **si torna alla disposizione di case/strade e agli inneschi**, non si inventano punti o effetti. Il territorio si considera definitivo solo dopo questa verifica.

## Compiti tecnici essenziali

- **Coordinatore MVP:** genera poche postazioni candidate sul lato esposto di un quartiere; verifica accesso, sicurezza ed ETA; assegna 2 autobotti + 1 squadra con una euristica semplice; lascia almeno una traccia del motivo dell'assegnazione; evita spostamenti avanti e indietro ogni tick.
- **Preview:** usa lo stato attuale, è **senza effetti collaterali**; applicazione solo alla conferma/timer. Rivalutare se il mondo è cambiato durante la finestra ×1.
- **Eventi:** pausa solo su conflitto significativo e ancora gestibile; anti-spam; massimo indicativo di due interruzioni. Nessuna previsione «perfetta» ottenuta leggendo il futuro della simulazione.
- **Civili:** modellare una vera **preallerta distinta dall'ordine di evacuazione**; non rinominare un comando esistente.
- **Strade:** verificare l'effetto di una difesa sulla percorribilità **prima** di introdurre il relativo obiettivo nel kiosk. Una chiusura del traffico non è una difesa. Se la prova fallisce, **segnalare il blocco al revisore** e decidere esplicitamente se estendere il modello o rinviare la funzione.
- **Giudizio:** confronti A/B con stesso seed, effetti delle priorità sulle persone/case/strade, distinzione fra proxy ed eventi effettivamente simulati. Nessuna promessa di «N case salvate» nelle anteprime.

## Che cosa deve consegnare un agente dopo ogni fase

**Massimo una pagina**, sempre con quattro sezioni:

1. **Realizzato:** elenco corto dei moduli/file cambiati.
2. **Evidenza:** comandi per riprodurre i test + PNG/tabella significativa.
3. **Non risolto:** rischi, ipotesi non verificate, eventuale test fallito.
4. **Passo successivo:** esattamente una fase proposta, senza avviarla se è previsto un checkpoint umano.

Non dichiarare una fase completata perché «il codice compila»: devono funzionare i suoi **esempi riproducibili**. L'agente non cambia le scelte di game design senza segnalarle.

## Priorità se manca tempo

**Da non tagliare:** unico mondo leggibile, obiettivi geografici, priorità con effetti misurati, coordinatore automatico, scelta esplicita per i civili, una finestra ×1, anteprima onesta, finale, kiosk stabile. **Tagliare prima:** terzo quartiere, varietà eccessiva di incendi, nuovi mezzi/aerei, proiezioni elaborate, effetti estetici nuovi, sistemi secondari. La difesa stradale resta una funzionalità da **validare e decidere**, non un pulsante fittizio.

## Prompt di avvio — solo fase 0

> Stiamo costruendo Rocca Ventosa, un gioco kiosk presidiato, di durata flessibile (anche oltre 5 minuti). Leggi `00-LEGGIMI-PRIMA.md`, `01-SPEC-GIOCO.md`, `02-SCENARIO-FACTORY.md`, `03-PIANO-DI-AZIONE.md`. Il territorio sarà **sintetico**: prima orografia e vegetazione plausibili; poi incendi headless con **innesco e vento variabili, umidità combustibile bassa/fissa/uniforme**; solo dopo case e strade. Si riutilizza il rendering Bevy attuale ma si rifà la UX. **Esegui solo la FASE 0**: audit e test baseline nel repository completo, classifica cosa riusare, adattare o rimuovere, misura il runner headless, e consegna un `audit.md` conciso con evidenze e proposta concreta per la fase 1. **Non implementare nuove funzionalità né anticipare le fasi successive.** Il responsabile sarà presente al chiosco: non è richiesto alcun reset automatico per inattività; serve invece un riavvio manuale affidabile.

**Check umano minimo:** approvare la pagina `00`, poi il primo agente fa l'audit. I due checkpoint visuali della Factory richiedono una scelta su immagini, non la lettura del codice. Una breve revisione dei risultati headless precede la UX definitiva.
