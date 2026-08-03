# Behaviour gaps found by scenario-building against real incidents

Scope: what building three real scenarios against documented EU wildfire
disasters (rather than only Spotorno, which has no historical fire to check
against) exposed about `crates/abm` and `crates/behavior`. Each gap below is
tied to a specific, sourced fact about a real incident and a specific place in
the current model — the same discipline as the "hard-won findings" in
`CLAUDE.md`: a plausible-sounding assumption is a claim, and the only thing
that checks a claim is comparing it against a real event.

None of this is a replay of a historical fire — per-scenario ignition, wind and
moisture are still set by the player, exactly as for Spotorno. What's real is
the terrain, the fuel, the buildings and the road network; a scenario baked
here means "the same kind of place the disaster happened in," not "the same
fire."

## The three scenarios

| id | Real event | Deaths | Why this one |
|---|---|---:|---|
| `mati` | Attica wildfires, Greece, 23–26 Jul 2018 | 104 | Greece's deadliest fire: no organised evacuation order, dense dead-end streets, most victims died in gridlocked cars or on the shoreline they were trying to reach on foot |
| `pedrogao` | Pedrógão Grande complex, Portugal, 17–24 Jun 2017 | 66 | 47 of 64 initial deaths were on one road (the N236-1, "road of death"), overtaken after dark by a front that outpaced the cars fleeing on it |
| `rhodes` | Rhodes wildfires, Greece, Jul 2023 | 0 | Greece's largest-ever evacuation (~20,000 people), thousands moved off beaches by coastguard and private boats when the road network alone could not clear the area in time — the success case, deliberately, for contrast |

Sources: [2018 Attica wildfires (Wikipedia)](https://en.wikipedia.org/wiki/2018_Attica_wildfires),
[VOA: Six convicted amid fury over 2018 wildfires that killed 104](https://www.voanews.com/a/six-convicted-amid-fury-over-2018-wildfires-that-killed-104-at-greek-resort-/7589252.html),
[Wildfire Today: The open wounds of Pedrógão Grande](https://wildfiretoday.com/the-open-wounds-of-pedrogao-grande/),
[CNN: Portugal wildfire, victims burned in cars as they fled](https://amp.cnn.com/cnn/2017/06/18/europe/portugal-fire/index.html),
[CNBC: Rhodes wildfire forces thousands of evacuations](https://www.cnbc.com/2023/07/23/tourists-flee-greek-island-rhodes-wildfire-thousands-evacuated.html),
[Rhodes Guide: Rhodes island wildfires, July 2023](https://www.rhodesguide.com/magazine/article/rhodes-wildfires-072023/).

Baking them surfaced a real data signal worth keeping in mind when using them:
the fraction of households within 100 m of burnable fuel — the population
generator's own proximity measure — is **6% for `mati`** (dense suburb-to-coast
building stock dominates over open fuel), **63% for `rhodes`** and **88% for
`pedrogao`** (scattered hamlets in continuous fuel, the classic high-WUI
profile). Every civilian-model threshold shipped today was tuned once, against
Spotorno's own ratio — see the open question below.

`mati`'s window (10.24 km, centred on the coordinates of Mati itself) runs from
sea level up to 1,097 m, which is Mount Penteli — the real fire's ignition
area, ~14 km from Mati by the actual historical run. That is deliberate: the
window spans the same corridor the 2018 fire ran down. It also means most of
the scenario's 42,569 baked buildings are the dense inland suburbs (Chalandri,
Marousi, Kifisia, Vrilissia) rather than the narrow coastal grid that actually
burned; a 750-household sample sits mostly in that inland stock rather than in
Mati's own pine-shaded lanes. Anyone using this scenario to study the Mati
disaster specifically should place ignition and attention at the coastal
(south) edge of the window, not the centroid.

---

## Gaps

### 1. Fire can outrun cars; the calibrated model can't produce that

Pedrógão Grande's fire was documented advancing at 15 km/h by nightfall,
overtaking vehicles on the N236-1. Mati's fire covered roughly 14 km in a few
hours. But finding 4 in `CLAUDE.md` measured the shipped calibration at
**500–800 m of spread in a full two-hour window** — the reason ignitions have
to be placed close and the reason the scenario length is capped at 1–3
simulated hours. A model tuned this way structurally cannot reproduce "the
fire arrived faster than the evacuation," which is the actual cause of death
in both loss-of-life incidents here. This isn't a bug in the calibration
(finding 4 measured it deliberately, against Spotorno's own wind and fuel), but
it is a ceiling: building `pedrogao` or `mati` scenarios to actually stress the
civilian model against their own historical dynamic needs wind speeds, fuel
moisture or slope steep enough to push realised spread rate into that range —
untested, because every existing fire test measures the Spotorno tuning.

### 2. Spotting ignites new cells in the core; nothing about evacuation knows it

`crates/propagator-core/src/kernel.rs` genuinely models spotting: embers are
drawn per burning cell (`compute_spotting`, kernel.rs:298+), a landing distance
is computed from wind and intensity, and a landing cell actually ignites
(`p_c` at kernel.rs:362) — a real, disconnected new fire, not a
scoring artefact. But `crates/fire::exposure`'s ember field and
`crates/fire::threat`'s ember cap (findings 6–7) are separate, downstream
scoring layers over the *existing* burning mask — they do not feed back into
which cells the core has actually spot-ignited ahead of the mapped front.

The practical gap: a household's or a unit's threat/exposure read is computed
from the fire mask the renderer already has, so an ember that jumps a road and
starts a **new**, still-tiny fire is present in the simulation the instant it
happens — the core knows — but nothing in `abm` treats "a new ignition appeared
somewhere not contiguous with the mapped front" as a distinct, alarming event
the way Pedrógão and Mati residents experienced it (fire behind you that
wasn't there when you left, cutting the road you were relying on). The
routing field (finding 8) will pick it up once it changes the fire mask enough
to matter, but there is no faster or separate "spot fire just started here"
signal a household or a commander could react to sooner than the field
refresh.

### 3. No lever to close a road to civilian traffic

Investigators found police failed to close the N236 in time at Pedrógão
Grande, which is given as a specific reason the death toll there was as high
as it was — traffic kept moving onto a road that was about to be cut by fire,
because nobody with the authority to close it did. Searching `crates/abm` and
`crates/game` for anything resembling a road-closure action turns up nothing:
the only road-obstruction concept is the fire model's own passability check
(`is_drivable_node`, used in `refuge.rs:55`) — the model closes a road only
when the fire has actually cut it, never pre-emptively, and never as a
commander decision.

This is a distinct lever from cutting a fireline (which acts on *fuel*, not on
*traffic*) and distinct from the evacuation order (which tells people to
leave, not which road not to take). The commander here has no way to say "keep
people off that road," which is exactly the failure mode Pedrógão Grande is
the textbook case of. It would sit naturally alongside the existing
intervention roster in `crates/abm/src/suppression.rs` and
`crates/game/src/command.rs`, as a third kind of order alongside evacuate and
dispatch — a link removed from the routing graph (finding 8's Dijkstra field)
for a duration, at a cost of reduced route diversity, exactly mirroring how a
fire cutting a road already forces rerouting.

### 4. The sea is not a place; boats are not a mode

Rhodes' evacuation moved thousands of people off beaches by coastguard vessel
and private boat when the road network could not clear the area fast enough —
described in press coverage as the country's largest-ever evacuation
operation. Mati's fatalities include people who died trying to reach the
shoreline on foot through smoke-filled lanes; some who reached open water
survived by swimming clear of the fire.

`crates/abm/src/refuge.rs` defines a `Refuge` purely as a `NodeId` on the
`RoadNetwork` (refuge.rs:37-45); `choose()` only considers nodes reachable on
the road graph (`net.is_drivable_node`, refuge.rs:55). Travel is exactly two
modes, `Mode::Foot` and `Mode::Car` (`crates/abm/src/lib.rs:99-102`). There is
no notion of the sea, a beach, or a boat anywhere in `crates/abm` — the module
doc for `refuge.rs` even notes that its road-node selection happens to surface
"the waterfront" and "the port," which is incidental to those being
low-fuel, low-elevation road-network points, not a modelled maritime
evacuation. Two real, opposite behaviours are both invisible to the model
today: reaching open water as an unplanned refuge of last resort (Mati), and a
coordinated, commander-ordered maritime evacuation as a real alternative
capacity to the road network (Rhodes) — which, for a coastal scenario like
Spotorno itself, is not a hypothetical: Spotorno's own shipped refuges already
include the waterfront (finding 9), and nothing about the model currently lets
that be a boat pickup rather than a dead-end road with a nice view.

### 5. No transient population: tourists, campers, beachgoers

Rhodes' crisis was substantially about a non-resident population: tourists
with no local knowledge, often no vehicle of their own, staying in hotels and
resorts, needing a different warning channel (a hotel front desk or PA, not a
"registered resident's mobile alert") and a different evacuation mode (boat
pickup at a beach) from a household evacuating its own home. Grepping
`crates/`, and `scripts/` for `tourist`, `transient`, `visitor` or `floating`
turns up nothing that represents population: the only hits are narrative-only
strings (an interview persona line in `crates/chat/src/persona.rs`, descriptive
tags in `scripts/places.py`) and unrelated UI comments.

The population model (`scripts/generate_population.py`, `crates/abm`) draws
every person from a **dwelling** — a household tied to a residential building.
`crates/behavior`'s "separated people" domain (`Domain::Person`,
`crates/behavior/src/domain.rs`) covers someone away from their own
household's home, which is a different problem: it is still a resident with a
home to return to or a family to reunite with (finding on reunification), not
a visitor with no home in the scenario at all. A beach resort, a campsite (the
same gap the Gironde 2022 fires would have surfaced, had it been one of the
three built here) or a hotel currently generates zero population beyond
whatever residential buildings happen to be nearby — there is no floating
occupancy tied to tourist infrastructure, no warning channel modelled for it,
and no reason for it to route to a boat rather than a road refuge even if one
existed (see gap 4).

### 6. Warning-channel failure is per-household and independent; real failures are systemic

The shipped model assigns each household one of five warning channels at
generation time (`scripts/generate_population.py:216-221`,
`p=[0.45, 0.20, 0.10, 0.20, 0.05]` over mobile alert / neighbour / siren /
self-observed / none) with independent per-channel delays
(`channel_delay_s`, `crates/abm/src/lib.rs:1584-1591`: 90 s mobile alert up to
1,200 s for none). Every household's warning is late or on time for its own,
private reason.

Real incidents show a correlated failure instead. Reporting on Pedrógão Grande
specifically cites the fire knocking out communication networks as a
contributing cause of the death toll — not some households having a bad
channel, but the channel infrastructure itself failing under the fire, for
everyone in reach at once, at the point in the incident when it mattered most.
Nothing in `crates/abm` models a shared piece of infrastructure (a cell tower,
a repeater) that the fire itself can degrade or take out, which would turn a
population's independent draw of channels into a correlated one exactly when
the fire is closest. This is the same class of gap as gap 3: the model treats
information flow as reliable machinery that only individual households can be
slow on, never a shared resource the incident itself can break.

### 7. "Shelter" means the household's own building; no shelter of last resort elsewhere

The household action set includes `Shelter` (`crates/behavior/src/value.rs:
111-119`), but it is explicitly "not a departure" —
(`crates/abm/src/lib.rs:885-887`) the household stays in its own building, and
survival is gated on that building's own defensible space
(`h.defensible_space`, lib.rs:922-935). This is a real, literature-backed
behaviour (concrete structures outperforming late evacuation is Black Saturday
and Camp Fire evidence CLAUDE.md's `docs/modelling.md` intro already cites)
and it is correctly modelled for the case it covers.

What it doesn't cover is the case both Mati and Rhodes actually produced:
someone caught **outside**, past the point where any building is reachable,
improvising a refuge that is not their home — a pool, open beach, or the sea
itself in Mati; a stretch of open ground or the water's edge on Rhodes. There
is no representable "shelter of last resort, not at home" location or
behaviour today; a household or separated person whose evacuation fails mid-
route has nowhere in the model to go except the refuge they were already
routing to (or, per gap 4, no maritime option at all).

---

## What ships fine as-is

Two things worth naming so they don't get relitigated: the perception model's
distance-and-bearing-to-fire and structure-exposure fields (findings 6–7) are
exactly the inputs a Mati or Pedrógão household actually had, and the
commander's order being a lever rather than a teleport (arriving over a real
channel with a real delay) is the correct shape for "no organised evacuation
order was given," which is literally what happened at Mati — the player
simply not issuing one reproduces it, no new mechanic required.

## Open question this raises

Every civilian-model threshold shipped today (`risk_perception`,
`prep_time_min`, the block defaults in `crates/behavior`) was tuned once,
against Spotorno's own building stock, road network and WUI ratio. `mati` (6%
of households near burnable fuel), `pedrogao` (88%) and `rhodes` (63%) are
three very different environments by the same measure, and nothing has run the
shipped behaviour library against any of them yet — whether the hand-written
model and the shipped graphs produce sane evacuation timelines outside
Spotorno's own WUI profile is measured nowhere. The honest next step, in the
same spirit as every other finding in `CLAUDE.md`, is running
`SPOTORNO_SCENARIO=mati` / `pedrogao` / `rhodes` through the existing
self-test and timeline instrumentation rather than assuming the numbers
transfer.
