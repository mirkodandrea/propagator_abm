"""Per-scenario place configuration for the real-data bake pipeline.

Every *real* scenario (as opposed to a synthetic ABM-testing lab, which
`generate_synthetic_scenarios.py` builds procedurally) is identified by one
entry here. `fetch_osm.py`, `bake_fire_rasters.py`, `build_render_terrain.py`
and `generate_population.py` all take `--scenario <id>` and read the matching
entry instead of hardcoding Spotorno, so adding a new real place is: add an
entry below, run `clip_cogs.py`, then the usual four scripts in order (see
`data/README.md`).

Coordinates are the scenario's own approximate reference point (matches
`scenario.json::coordinates`) -- not a survey, just enough to place the window
and to seed narrative text. `utm_corner` is the scenario window's SW corner in
the given UTM zone, 20 m grid-aligned to the shared EU COG origin
`(0, 7960000)` (see `data/README.md`).
"""

from dataclasses import dataclass, field


@dataclass(frozen=True)
class Place:
    id: str
    name: str
    location: str
    country: str
    nationality: str
    # A short narrative phrase used in interview prompts, e.g.
    # "the Ligurian coast of Italy". Should read naturally after "a resident
    # of ...".
    region: str
    coordinates: tuple[float, float]  # (lat, lon)
    utm_zone: int
    utm_corner: tuple[float, float]  # (x, y), metres, this UTM zone
    world_size_m: tuple[float, float]
    fire_cellsize_m: float
    description: str
    tags: list[str] = field(default_factory=list)
    authors: list[str] = field(default_factory=lambda: ["Mirko D'Andrea"])
    license: str = "EUPL-1.2"


PLACES: dict[str, Place] = {
    "spotorno": Place(
        id="spotorno",
        name="Spotorno, Liguria",
        location="Spotorno, Italy",
        country="Italy",
        nationality="Italian",
        region="the Ligurian coast of Italy",
        coordinates=(44.2265, 8.4176),
        utm_zone=32,
        utm_corner=(448360.0, 4892080.0),
        world_size_m=(10240.0, 10240.0),
        fire_cellsize_m=20.0,
        description=(
            "Real data scenario: coastal Italian town with synthetic population "
            "and behavioral priors from wildfire evacuation literature"
        ),
        tags=["real-data", "coastal", "wui", "liguria", "italy"],
    ),
    "mati": Place(
        id="mati",
        name="Mati, Attica",
        location="Mati, Greece",
        country="Greece",
        nationality="Greek",
        region="the Attica coast east of Athens",
        coordinates=(38.0525, 23.86833),
        utm_zone=34,
        utm_corner=(746560.0, 4210400.0),
        world_size_m=(10240.0, 10240.0),
        fire_cellsize_m=20.0,
        description=(
            "Real data scenario: dense pine-shaded seaside settlement on the Attica "
            "coast, modelled on the July 2018 Attica wildfires -- Greece's deadliest, "
            "104 dead, most in vehicles gridlocked on narrow dead-end lanes between "
            "the pines and the sea, or on the shoreline itself. Synthetic population "
            "and behavioural priors; not a replay of the historical fire."
        ),
        tags=["real-data", "coastal", "wui", "attica", "greece", "dense-narrow-streets"],
    ),
    "pedrogao": Place(
        id="pedrogao",
        name="Pedrógão Grande, Leiria",
        location="Pedrógão Grande, Portugal",
        country="Portugal",
        nationality="Portuguese",
        region="the pine and eucalyptus hills of central Portugal",
        coordinates=(39.9169, -8.1478),
        utm_zone=29,
        utm_corner=(567720.0, 4413760.0),
        world_size_m=(10240.0, 10240.0),
        fire_cellsize_m=20.0,
        description=(
            "Real data scenario: scattered hamlets in steep eucalyptus and pine "
            "terrain in central Portugal, modelled on the June 2017 Pedrogao Grande "
            "wildfire complex -- 66 dead, most on the N236-1 ('road of death'), "
            "overtaken by a fire front outrunning the cars fleeing on it after dark. "
            "Synthetic population and behavioural priors; not a replay of the "
            "historical fire."
        ),
        tags=["real-data", "inland", "wui", "leiria", "portugal", "steep-terrain", "eucalyptus"],
    ),
    "rhodes": Place(
        id="rhodes",
        name="Lardos, Rhodes",
        location="Lardos, Rhodes, Greece",
        country="Greece",
        nationality="Greek",
        region="the southeastern coast of Rhodes",
        coordinates=(36.0933, 28.017),
        utm_zone=35,
        utm_corner=(586440.0, 3989660.0),
        world_size_m=(10240.0, 10240.0),
        fire_cellsize_m=20.0,
        description=(
            "Real data scenario: touristic villages and beach resorts on the "
            "southeastern coast of Rhodes, modelled on the July 2023 Rhodes "
            "wildfires -- Greece's largest-ever evacuation, roughly 20,000 people "
            "moved from Lardos, Kiotari, Gennadi and nearby resorts, thousands by "
            "boat off beaches the road network could not clear in time. No deaths. "
            "Synthetic population and behavioural priors; not a replay of the "
            "historical fire."
        ),
        tags=["real-data", "coastal", "wui", "rhodes", "greece", "tourist", "island"],
    ),
}


def get(scenario_id: str) -> Place:
    try:
        return PLACES[scenario_id]
    except KeyError:
        known = ", ".join(sorted(PLACES))
        raise SystemExit(
            f"no place config for scenario '{scenario_id}' -- add one to scripts/places.py "
            f"(known: {known})"
        )
