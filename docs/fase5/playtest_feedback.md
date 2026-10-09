# Feedback del playtest umano — 9 ottobre 2026

Implementati i sei punti di leggibilità segnalati, mantenendo il territorio e la simulazione esistenti.

- **Case sparse:** i nuclei separati sono identificati dal nome del paese e collegati alla sua scheda. Le Ghiande ha due gruppi con la soglia visiva di 220 m; gli ordini della scheda valgono per entrambi. Gli ancoraggi sono case reali, non il centro vuoto fra abitazioni (`rocca/src/district.rs`).
- **Schede e legenda:** priorità e famiglie hanno testo esplicito; evacuate/totale e stati della popolazione si leggono accanto alla barra. «Legenda e tempi» spiega colori, numeri, mezzi proposti/assegnati, anelli e tempi simulati; include i colori indicativi di prati, latifoglie, macchia e conifere.
- **Durante l'evento:** velocità e fase sono scritte nella barra superiore. Si possono modificare le schede e confermare durante l'esecuzione, oppure usare «Pausa e modifica piano» e «Conferma e riprendi». Un suggerimento della volontaria resta visibile quando non ci sono nuove battute.
- **Tempi:** le schede mostrano l'intervallo degli arrivi stimati dei mezzi dal coordinatore. Per l'evacuazione mostrano il viaggio dalla casa all'area sicura, ricavato dalla rete e dal percorso attualmente aperto (`Abm::evacuation_journey_s`). Sono tempi di viaggio senza code, rallentamenti e fuoco futuro, non un conto alla rovescia. Avviso, preparazione e decisione di partire sono separati: il modello non consente di promettere un tempo di evacuazione totale. Corretta anche la battuta «Partono adesso».
- **Eventi:** gli ultimi due aggiornamenti sono sempre visibili, con icona e orario; la cronologia completa è consultabile e scorrevole.

## Verifica riproducibile

`cargo test --release -p rocca -p abm`: test della simulazione verdi. Nuovi test: raggruppamento dei nuclei e stime dai percorsi reali senza impartire ordini.

`cargo build --release -p game` e `bash scripts/build_web.sh`: compilazioni riuscite (warning preesistenti).

`KIOSK_CASE=Coste2_gira KIOSK_SPEED=300 KIOSK_WINDOWED=1 KIOSK_SHOT=/tmp/rocca-playtest-ui-final target/release/game`: schermate di pianificazione, anteprima, crisi, debrief e riavvio manuale. Ispezionate le schede e corretto il posizionamento delle etichette dei nuclei rispetto ai pannelli.

Prova nel browser locale a 1280×720: verificati etichette dei nuclei, apertura e scorrimento della legenda, combustibili, feedback durante l’esecuzione e passaggio a «In pausa / Conferma e riprendi». La legenda si sovrappone temporaneamente alla mappa senza spostare le schede; il pulsante centrale lascia spazio al fumetto.

## Prossima prova

Ripetere una partita con persone per verificare che sappiano identificare i nuclei e leggere i tempi, distinguendo viaggio e completamento dell'evacuazione. Da verificare anche touchscreen e finestra stretta (900×600).
