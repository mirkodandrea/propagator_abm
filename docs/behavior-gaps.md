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

## Status

Gaps 2–7 have been closed **on the agent side**, entirely inside the behaviour
graph system: five mechanisms in `crates/abm`, eight new blocks, four new
actions and fourteen new observation fields in `crates/behavior`, and three new
profiles that ship at **zero share**. Gap 1 is a fire-calibration question and
is untouched — it is not an ABM gap and pretending otherwise would have meant
tuning the fire to make a behaviour look good.

| Gap | Where it lives now | Ships |
|---|---|---|
| 2 · spot fires | `abm::spot`, `block.spot_fire`, `block.person_spot_fire` | off |
| 3 · road closure | `Abm::close_road`, `network::solve_with`, `block.road_closed` | live, no closure ordered |
| 4 · the sea, boats | `abm::haven`, `abm::orders::BoatLift`, `block.person_boat_pickup` | off |
| 5 · transient population | `Capability::Transient`, `block.visitors`, profile `holiday-let` | share 0 |
| 6 · correlated warning failure | `abm::comms`, `block.no_signal` | live, inert until a mast burns |
| 7 · shelter of last resort | `abm::haven`, `block.last_resort`, `block.person_last_resort` | off |

Everything is off or inert by construction, because every figure in
`crates/fire/tests` and every number quoted in `CLAUDE.md` was measured before
these existed. `crates/abm/tests/incident_gaps.rs` is the evidence for both
halves: that each branch *fires* when a profile turns it on, and that the
shipped profiles produce the same run they did.

**What the derivations actually found** (`cargo test -p abm --release --
--ignored haven_report`):

```
scenario      refuges   havens    shore   masts
spotorno           12      102       38       4
mati               12      160        0       4
pedrogao           12       18        0       3
rhodes             12       81       42       6
```

Two things in that table are findings rather than output. **`mati` has no coast
in its window at all** — the DEM's minimum is 140 m — so a scenario built around
people who died trying to reach a shoreline cannot express reaching it; the
window covers the corridor the fire ran down and stops inland of where it
killed people. And **`pedrogao` has 18 havens against Spotorno's 102**, which is
the 88%-of-households-near-fuel number from above showing up as the thing it
actually means: in that landscape there is nowhere to go.

**Four measurements worth keeping** (`--ignored incident_mechanism_report`,
2 h, general order at T+0, one profile forced across the whole population):

```
spotorno                                 safe sheltering    dead ppl safe  lifted
wait-and-see / walk-out                   557          0       0     1228       0
reacts-to-events / walk-out               564          0       0     1237       0
reacts-to-events / to-the-water           564          0       0     1237       0
reacts-to-events / to-the-water +boats    564          0       0     1231     361

rhodes
wait-and-see / walk-out                   576          0       0     1278       0
reacts-to-events / walk-out               580          1       0     1285       0
reacts-to-events / to-the-water           580          1       0     1285       0
reacts-to-events / to-the-water +boats    581          0       0     1282     355
```

The boat lift moves 355–361 people and saves nobody: those people were reaching
a land refuge anyway, and six of them are *worse* off for having walked to the
water. That is the honest answer for these two windows, where the waterfront is
already a refuge and the road network is not saturated, and it is exactly the
comparison the mechanism exists to allow — Rhodes' lift mattered because 20,000
people could not clear the area by road, and neither shipped window has that
problem at this population. Testing it properly needs a scenario where the road
capacity actually binds.

**And one measurement that reframes the rest.** At the shipped calibration the
threat at a house peaks around **0.3** over a two-hour incident, **no structure
ever ignites**, and **no household's route is ever cut** — on any of the four
real scenarios. Every branch downstream of "the fire is on the property",
including the `action.evacuate_now` and `action.shelter` branches that shipped
long before this work, is therefore almost entirely inert. A last-resort
behaviour inherits that: `block.last_resort` fires exactly when
`block.fire_at_the_door` does, which is why it takes that block's output as an
*input* rather than testing a threshold of its own. The first version did test
its own threshold, at the same 0.35 the shipped block uses, and it was an
always-negative — finding 26, built fresh.

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

**Closed.** `abm::spot` maintains the mask needed to recognise one: a newly
burning cell with nothing already alight within 120 m of it, flood-filled into
one record per blob so a 400 m patch is one new fire and not eleven.
`HouseholdObs`/`PersonObs` carry the distance and the *age*, and the age is
half the value — a spot fire twenty minutes old is already in everyone's threat
field, and one from two minutes ago is the reason to go now. `block.spot_fire`
and `block.person_spot_fire` are the two assumptions. What it deliberately does
*not* do is ask the core which cells it spotted: non-contiguity is the
criterion, so a second ignition the player lit counts too, which is what a
resident would see.

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

**Closed.** `Abm::close_road(centre, radius, duration)` and
`network::solve_with`, which takes a per-link closed mask. It binds the civilian
*car* field and nothing else: a pedestrian walks down a closed road and a
suppression unit asking `network::route` never sees it, because a barricade is a
traffic order. The households it strands read `road_closed` and
`block.road_closed` is what they do about it — walk it, or decide it is too far
and stay in the house, which is the cost of the lever and is the output worth
wiring somewhere visible. There is **no map tool**: three already contend for
left-click and a fourth needs a rule rather than a patch, so the order is placed
through the control API (`POST /control/close_road`) for now.

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

**Closed.** `abm::haven` adds a second class of destination — measured, not
authored, exactly as refuges are — and the criterion refuges do not have is
**buildings**: non-vegetated fuel does not distinguish a car park from the old
town, and a lane with houses alight on both sides is not open ground. A haven
adjacent to water is a `HavenKind::Water` one, and the water is derived from the
two rasters the fire model already loads (non-burnable cell at or below 2 m),
because OSM does not carry the coastline. `Goal::Haven`/`Goal::Shore` are two
more route fields, `TravelState::Sheltering` is where somebody ends up, and it
is deliberately *not* `Safe` — a model that counted reaching the water as
evacuating would say Mati was a success. `abm::orders::BoatLift` is the Rhodes
half: requested, on station after a delay, taking people off the beach at a rate
integrated over simulated time.

One thing the first version got wrong and the data caught: "within 80 m of
water" put a haven 27 m up on the cliff the Aurelia is cut into. Near the sea
and able to get into the sea are different predicates.

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

**Closed, without re-baking a population.** `Capability::Transient` makes a
visiting party a *profile* rather than a population field, which means a share
of households — hashed, like every other profile assignment — and asking "what
does this town look like in August" is a number in a file. What it changes is
small and specific: no vehicle, a warning that arrives through whoever runs the
place (`MANAGED_DELAY_S`, and not over the mobile network), and a higher
threshold for acting without being told, because a resident reads a column of
smoke over that ridge and a visitor cannot. `block.visitors` is the assumption
and `holiday-let` is the profile, at share 0.

The route to a boat is gap 4's, and the two compose: a visiting party with no
car, told by the front desk, walking to a pickup at the beach is now
expressible in three overrides.

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

**Closed.** `abm::comms` derives masts from the road network and the population,
and a mast goes down on *threat* rather than on burning — it stands on cleared
ground, which is non-vegetated, which never enters the fire mask (finding 2
again). A household whose mobile alert was coming over a mast that is now down
falls back to the no-channel delay, and every household under that mast falls
back together, which is the correlation no per-household draw can produce.
`block.no_signal` is what they do about it, and its interesting output is
"giving up on it": the households that *trust* official instructions are the
ones waiting for a message that will never arrive.

Two things the data changed here. Siting the masts on the six highest road nodes
— which is where masts visibly are — covered 745 of 750 households on Spotorno
but only 316 on `mati` and 139 on `pedrogao`, so two scenarios would have
started with most of the town already out of signal and silently moved the
baseline. Real operators site for coverage, so the derivation does: greedy
maximum coverage, ties broken on elevation, and 750/750 on every real window.
And `covered()` models the *loss* of service rather than service — somewhere no
mast ever reached is unaffected by one going down — which is what makes the
whole mechanism provably inert until the fire breaks something.

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

**Closed by the same machinery as gap 4.** `ActionKind::ShelterNearby` and
`WalkToOpenGround` send a household or a person to the nearest haven on foot;
`MakeForShore`/`WalkToShore` send them to the water. Sheltering at a haven gives
relief from flame exposure — all of it at the water's edge, because a person in
the sea is out of the fire's reach for as long as they can stand it, and rather
less in a car park, because a car park in a firestorm is survivable and is not
safe. `block.last_resort` and `block.person_last_resort` decide **where**, not
whether, which is why both take the moment as an input from the block that
already owns that threshold.

---

## What ships fine as-is

Two things worth naming so they don't get relitigated: the perception model's
distance-and-bearing-to-fire and structure-exposure fields (findings 6–7) are
exactly the inputs a Mati or Pedrógão household actually had, and the
commander's order being a lever rather than a teleport (arriving over a real
channel with a real delay) is the correct shape for "no organised evacuation
order was given," which is literally what happened at Mati — the player
simply not issuing one reproduces it, no new mechanic required.

## Open questions these raise

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

And the six new ones, all of them consequences of closing the gaps above:

- **Nothing is ever caught, so the last-resort behaviours cannot be measured.**
  Threat at a house peaks near 0.3, no structure ignites and no route is cut,
  so `block.last_resort` fires only when a profile lowers
  `block.fire_at_the_door`'s own threshold — which is what the test does, and
  says so. Whether making for open ground beats sheltering in the house is
  therefore *unmeasured*, and it is the question the block exists to answer.
  It needs a calibration where the fire actually reaches people, which is gap 1.
- **A boat lift currently costs six lives and saves none**, on the two windows
  that have a shore. That is a real result for those windows and not a general
  one; it needs a scenario where the road network cannot clear the population,
  which neither shipped coastal window is at 750 households.
- **`mati` has no coast in its window.** A scenario about people who died
  trying to reach the shoreline cannot express reaching it. Re-baking with the
  window pushed south is a `places.py` edit and a pipeline re-run.
- **A road closure has no map tool.** It is an area, and placing an area needs a
  fourth left-click tool with a rule behind it. Until then it is
  `POST /control/close_road`, which is fine for a scientist and useless for a
  player.
- **Nothing on the map shows any of this.** A mast that has burnt, a road that
  is closed, a beach with people on it and a spot fire that just started are all
  in `Sim` and none of them is drawn — which is the same complaint the composer's
  Live tab already has about authored policies.
- **A haven is a point, not a capacity.** Two hundred people at one car park is
  two hundred people at one car park, and the model has nothing to say about
  what that is like or when it stops being safe.
