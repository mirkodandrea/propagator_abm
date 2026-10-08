# Versione per browser

La stessa partita del kiosk, nel browser, per farla provare a chi non è al chiosco. Stesso motore (`rocca::Game`) e stessi dati, compilati dentro il `.wasm` (`crates/datafs`).

Differenze solo grafiche, per WebGL:
- terreno disegnato a 20 m (i dati restano a 5 m);
- vegetazione al 12 %;
- niente MSAA.

Il confronto «senza ordini» del debrief avanza di qualche passo per frame invece che in un thread.

```text
./scripts/build_web.sh                                  # sito statico in target/web
python3 -m http.server 8765 --directory target/web      # http://localhost:8765
```

Pubblicazione: `.github/workflows/pages.yml` costruisce e pubblica su GitHub Pages a ogni push su `main` o `settimana-protezione-civile`. Indirizzo: https://mirkodandrea.github.io/propagator_abm/

Serve un computer con mouse e un browser recente; il primo caricamento scarica circa 50 MB. Una scheda in secondo piano viene messa in pausa dal browser.
