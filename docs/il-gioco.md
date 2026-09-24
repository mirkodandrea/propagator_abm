# Il gioco, in breve

Un *serious game* sull'interazione fra le persone e un incendio boschivo. Il
giocatore è il direttore delle operazioni di spegnimento; i civili sono
individui simulati uno per uno; l'incendio è un modello di propagazione che
gira al secondo su dati reali.

## Gli scenari

Il simulatore non è legato a un posto solo. Alla partenza si sceglie da un
elenco; oggi ne sono presenti tredici.

Quattro sono **finestre reali** di 10,24 × 10,24 km, costruite dagli stessi
dati (carta europea dei combustibili a 12 classi, DEM, OpenStreetMap) e ognuna
con le proprie condizioni iniziali di vento, umidità e innesco:

| | Dove | A cosa è ispirato |
|---|---|---|
| `spotorno` | Spotorno, Liguria *(predefinito)* | nessun incendio storico: è la finestra di riferimento |
| `mati` | Mati, Attica | Attica 2018 — vittime in auto su strade strette senza uscita |
| `pedrogao` | Pedrógão Grande, Leiria | Pedrógão Grande 2017 — fronte che supera le auto in fuga sulla N236-1 |
| `rhodes` | Lardos, Rodi | Rodi 2023 — evacuazione anche via mare dalle spiagge |

La popolazione è sintetica in tutti e quattro (750 famiglie, ~1.570 persone),
collocata sugli edifici che esistono davvero. Nessuno scenario è la riproduzione
dell'incendio storico: è la stessa geografia con un incendio nuovo.

Gli altri nove sono **banchi di prova sintetici** — una strada sola, un unico
sbocco congestionato, una strada tagliata, accesso difficile per i mezzi, due
intensità di fuoco, e due scenari di scala fino a 1.667 famiglie. Servono a
isolare un meccanismo alla volta, che su una finestra reale non è possibile.

## Cosa gira sotto

**L'incendio** è il nucleo di PROPAGATOR (CIMA), su combustibile e pendenza
reali: vento, umidità e morfologia decidono la propagazione, e i tizzoni
possono accendere focolai staccati davanti al fronte. In due ore un fronte
percorre qualche centinaio di metri.

**Le persone.** Ogni famiglia passa per quattro stadi — percezione, decisione,
preparazione, movimento — e la letteratura sull'evacuazione dice che la
varianza sta in mezzo, non agli estremi: quanto ci mettono a decidere, e se la
strada è ancora aperta quando si muovono. Una famiglia vede il fumo o non lo
vede, riceve l'avviso sul proprio canale (dal telefono al passaparola), decide
se partire, aspettare, restare a difendere o chiudersi in casa, impiega il suo
tempo a prepararsi, e poi si muove in auto o a piedi sul grafo stradale vero.

Le auto stanno in coda: ogni tratto ha una capacità di deflusso e una capacità
di accumulo, e quando è pieno la coda si propaga all'indietro attraverso gli
incroci. Chi trova la strada tagliata abbandona l'auto. Chi era fuori casa
all'innesco — circa un quarto della popolazione — è un agente a sé, che può
mettersi in salvo o tornare verso la famiglia.

Oltre ai punti di raccolta il modello conosce i **ripari di ripiego** (radure,
parcheggi, la battigia): posti raggiungibili subito, dove però non si è "in
salvo", solo al riparo.

## Cosa fa il giocatore

**L'ordine di evacuazione**, generale o su un'area. Non è un teletrasporto:
arriva a ciascuno sul proprio canale, da un minuto e mezzo a una ventina di
minuti. È comunque la leva più forte del modello — su Spotorno, senza ordine se
ne va circa un settimo delle famiglie, con l'ordine generale due terzi.

**Le squadre**, otto in tutto, con vincoli diversi e non intercambiabili:

| | Si muove su | Agisce con | Limite |
|---|---|---|---|
| Squadra a terra ×3 | strade, piste, poi a piedi | linea tagliafuoco | 120 m/h nella macchia |
| Autobotte ×3 | solo strade carrabili | acqua | 2.500 L, 60 m di manichetta |
| Canadair ×2 | linea d'aria | 6.137 L a lancio | 25 minuti dalla richiesta |

Una squadra ordinata in un punto non sopravvivibile si ritira: la sicurezza
prevale sull'ordine, e ogni rifiuto è motivato a schermo.

Ci sono poi due leve minori sulla viabilità: chiudere una strada al traffico
civile, e chiedere un trasporto via mare dove la costa c'è.

Due cose che il modello rende evidenti e che valgono anche fuori: **bagnare le
fiamme non produce effetto** — si lavora sul combustibile davanti al fronte — e
una linea tagliata trecento metri davanti a un fronte che lancia tizzoni a
trecento metri viene scavalcata. Allontanarla non risolve, perché intanto i
fianchi la aggirano.

## Guardare dentro

Tutto è ispezionabile: una famiglia, una persona, un mezzo. Il pannello mostra
cosa ha percepito, cosa ha deciso, quanto manca alla partenza, quanta acqua
resta.

Si può anche **parlare con un agente**: selezionarlo e fargli domande, con una
risposta generata da un LLM a partire dai soli dati di quell'agente — i suoi
tratti, quello che percepisce da dove si trova, e la sua riga del registro
eventi. Non il totale dei salvati, non la posizione del fronte, non le decisioni
degli altri. Aprire un colloquio mette in pausa l'incidente, perché la risposta
vale per un istante preciso.

## Niente punteggio

Non c'è voto finale né schermata di vittoria. C'è il resoconto di quello che è
successo: quante famiglie sono uscite e quando, per dove, quante case hanno
preso danno, chi è rimasto tagliato fuori.

La domanda non è "hai vinto" ma "cosa sarebbe cambiato dando l'ordine dieci
minuti prima". Si può rispondere perché lo stesso identico incendio si rigioca
cambiando una variabile sola: gli inneschi sono registrati con il loro istante,
e un riavvio li ripete.
