# Dati del gioco

Un solo territorio, un solo scenario: **Rocca Ventosa** (`scenarios/rocca_ventosa/`), generato dalla Scenario Factory (`tools/scenario_factory.py`) e pubblicato con `publish`.

```text
data/
├── fuels_eu12.json            tabella dei combustibili eu_fuel12 (fuoco)
├── behaviours/                grafi di comportamento di famiglie, persone e mezzi
└── scenarios/rocca_ventosa/
    ├── scenario.json          id, nome, quartieri
    ├── game.json              casi (innesco, vento, durata), caserme, roster
    ├── dem.f64, fuel.i32      griglia del fuoco, 400 × 400 a 20 m
    ├── render_terrain.*       terreno a 5 m per grafica e agenti
    ├── cover.u8, cover.json   copertura del suolo a 5 m (grafica)
    ├── osm.json               edifici, strade, acqua (metri)
    ├── population.json        famiglie e persone
    └── params.json            seme e parametri della Factory
```

Rigenerazione:

```text
.venv/bin/python tools/scenario_factory.py build-town
.venv/bin/python tools/scenario_factory.py town-fires
.venv/bin/python tools/scenario_factory.py publish
```

Il gioco, il runner headless (`target/release/rocca`) e il kiosk (`target/release/game`) leggono tutti questa cartella attraverso `rocca::Game`.
