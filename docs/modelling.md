# How the agent models work

Scope: `crates/abm` — two populations of agents and both of the commander's
levers over them.

| Part | Agents | The commander's lever |
|---|---|---|
| **Civilians** (`lib.rs`, `network.rs`, `refuge.rs`) | households, people, vehicles | *when and where to order an evacuation* |
| **Suppression** (`suppression.rs`) | hand crews, engines, air tankers | *which unit works where, and when to call for aircraft* |

They share the road network, the threat field, and every invariant below; they
are otherwise independent models, and the split is worth keeping in mind because
the two ask opposite routing questions (see [Routing: a field *and* a
search](#routing-a-field-and-a-search)).

For the fire model itself see `crates/fire`; for what suppression actually does
to the fire — the two boundary-condition fields and their calibration — see
[What the work does to the fire](#what-the-work-does-to-the-fire). For coordinate
frames and data provenance see `CLAUDE.md` and `data/README.md`.

The civilian model is wildfire-specific and built from scratch (not a port of the
igad flood ABM). It follows the standard evacuation-literature decomposition:
**perception → decision → preparation → movement**, because post-fire studies
(Black Saturday, Camp Fire, Mati) show that's where survival actually gets
decided — not the fire's behaviour at the property, but how long people took
to decide and whether the road was still open when they moved.

Three invariants the code holds to everywhere, civilians and units alike:

- **Nothing accumulates per call.** Every rate is per simulated second, so a
  2 s game step and a 300 s test step produce the same outcome
  (`step_size_invariance`, and `water_is_independent_of_step_size` for the
  units).
- **Randomness is per-agent, not per-call.** Thresholds are jittered from a
  hash of the agent's id (`hash01`), never a sequential RNG draw, so one
  agent's behaviour can't change because another agent's did or the step
  size changed.
- **Houses never burn in the CA.** People sit on the same non-burnable cells
  as their houses, so danger to an agent never comes from the fire mask —
  only from `fire::ThreatField` (instantaneous, "is it survivable to stand
  here") and `fire::exposure` (integrated, "is this building being
  destroyed"). See CLAUDE.md finding #7.

---

# Part 1 — Civilians

---

## The two population units

- **`HouseholdAgent`** is the decision-making unit. Families evacuate
  together — a household with one slow or assistance-needing member is slow
  as a whole, because a group's speed is its slowest member's.
- **`PersonAgent`** is the inspectable unit: one capsule per person in the
  3D view. Each carries `age`, `walk_speed`, `needs_assistance`, and a
  `traveller` link once it's moving.

Both are baked into the scenario by `scripts/generate_population.py`, not
generated at runtime — the ABM only *reads* `risk_perception`,
`trust_authority`, `intent`, `warning_channel`, `vehicles`,
`defensible_space`, and `prep_time_min` off each household.

People who were away from home when the scenario starts (`at_home == false`)
begin walking to a refuge immediately, on foot, from wherever they happened
to be — they don't drive home into the fire first. That's deliberately not
always realistic (real people do try to reunite with family), and is flagged
as an open question in CLAUDE.md.

---

## Stage 1 — Perception

Runs every `DECISION_S` = 5 simulated seconds, in `Abm::decide`. Each
household computes a `cue` in 0–1 from three independent readings, taking the
max:

| Signal | Source |
|---|---|
| `danger` | `fire::ThreatField` sampled at the house — "is it survivable to stand here right now" |
| `ex.radiant.max(ex.ember)` | `fire::exposure::StructureExposure` for this household's building — the slower, integrated destruction risk |
| `0.75 * seen²` | a coarse **visible-fire** field: distance to the nearest active fire cell, capped at 2,500 m, squared so only a fire that's genuinely close registers strongly |

The visible-fire field (`fire_dist`) is its own thing, separate from both
threat layers: it's what a household can *see* on the ridge, not what's
close enough to hurt them, rebuilt every route refresh with a 200 m two-pass
chamfer distance transform (cheap: 51×51 cells).

The raw cue is scaled by `attention = 0.6 + 0.8 * risk_perception` — two
households looking at the same smoke column read it differently depending on
their baked risk perception. The cue then rises instantly but decays slowly
(`0.98` retention per tick) once alarmed, so a momentary flare doesn't reset
someone back to calm.

Independently, the commander's evacuation order arrives over the household's
own **warning channel**, each with a fixed delay from `channel_delay_s`:

| Channel | Delay |
|---|---|
| Mobile alert | 90 s |
| Siren | 180 s |
| Neighbour | 420 s |
| Self-observed | 600 s |
| None | 1200 s |

So `order_evacuation` is a lever, not a teleport — issuing the order flips
`ordered = true` immediately, but `warning_received` only flips once that
household's own delay has elapsed. This is the mechanism behind the
CLAUDE.md line "the commander's order is a lever, not a teleport."

A household becomes `Status::Warned` once `warning_received` **or**
`cue > 0.10 + 0.20*(1 - risk_perception) + jitter` — i.e. it can self-alert
from the smoke alone, order or not.

---

## Stage 2 — Decision: the three intents

`Intent` is a baked household trait, and it's the main behavioural policy
knob:

| Intent | Departure condition | Character |
|---|---|---|
| `LeaveEarly` | any status change away from `Normal` | Leaves on the first credible signal, order or not. |
| `WaitAndSee` | a **credible order** (received + `trust_authority > 0.35`) OR `cue > 0.22 + 0.15*(1 - risk_perception)` | The dangerous majority — needs a direct cue or a trusted order. |
| `StayDefend` | `cue > 0.55 + 0.25*jitter` OR the house is already alight | Stays to fight the fire, and leaves late or not at all — the population most likely to die in the road. While not yet departing, status becomes `Defending`. |

If `depart` is true and the household is `Warned` or `Defending` and not
already travelling, it enters `Preparing` with a jittered `prep_remaining_s`
(see below), then launches once that counts down to zero.

### Trapped-at-home path

Independent of the above: if a household hasn't launched a traveller and the
fire's `danger` at the house crosses `IMPASSABLE` (or the structure is
alight), the household becomes `Trapped` and accumulates `shelter_s`.
Sheltering in a house is survivable far longer than being caught in the open
(`SHELTER_SURVIVAL_S` = 900 s vs. `LETHAL_EXPOSURE_S` = 90 s for a traveller),
scaled up further by `defensible_space` (0.7–1.5×). Exceed the survivable
window and the household — and every member not already travelling —
becomes a `Casualty`.

---

## Stage 3 — Preparation

`prep_time_min` is the single biggest lever on outcome (per the module
doc): it's why "we told them to go" and "they went" are not the same event.
Actual prep time is:

```
prep_seconds = max(60, prep_time_min * 60 * scale * jitter)
jitter = 0.85 .. 1.15, hashed from household id
scale:
  LeaveEarly  → 0.8   (less to pack, already inclined to go)
  WaitAndSee  → 1.0
  StayDefend  → 0.5   (already outside with the hoses out — but far too late)
```

Once `prep_remaining_s` reaches zero, `Abm::launch` fires: it filters
household members down to those actually at home and not already
evacuated/casualty, picks a mode, and creates a `Traveller`.

### Car vs. foot

A car is only used if all of: the household owns a vehicle, its nearest
drivable node is within 400 m to walk to, and that node has a live
**car** route to a refuge (`routes_car.reachable`). Otherwise the whole
household walks. Foot speed is the *slowest* member's `walk_speed`, halved
(×0.55) for anyone needing assistance, capped at 2.0 m/s.

---

## Stage 4 — Movement

`Abm::move_travellers` runs every `MAX_SUBSTEP_S` = 4 s slice of the frame's
`dt`, so a 30 s game-loop step at high time-scale can't shoot a car past a
junction.

**Heat load.** While `danger >= IMPASSABLE` at the traveller's position, it
accumulates `heat_s` at rate `danger`; otherwise it recovers at
`HEAT_RECOVERY` = 0.5/s. Cross `LETHAL_EXPOSURE_S` = 90 s and the traveller
(and every member riding with it) becomes a `Casualty` on the spot — this is
the model's only route to death while actively evacuating, distinct from the
trapped-at-home path above.

**Speed.** Base speed is the traveller's `free_speed` (car: `CAR_SPEED` =
11 m/s ≈ 40 km/h, hill-town speed, not motorway; foot: per-person), capped to
`APPROACH_SPEED` = 1.6 m/s while crossing open ground to the network. Then:

- **Slope** (foot, off-network only): `(1 - slope/45°)` clamped to
  [0.35, 1.0] — a 20° hillside roughly halves walking speed.
- **Congestion** (car, on-network): `1 / (1 + CONGESTION * (occupants-1))`
  clamped to a 0.15 floor, `CONGESTION` = 0.06. Occupancy is read from the
  *previous* sub-step and rebuilt fresh each sub-step, so results don't
  depend on the order agents happen to be processed in.
- **Smoke/heat**: `(1 - danger*0.6)` clamped to [0.3, 1.0] — thick smoke
  slows people well before it stops them outright.

**Routing.** Travellers don't pathfind individually — see [Routing: a field
*and* a search](#routing-a-field-and-a-search). `RouteField` (in `network.rs`)
is a single multi-source Dijkstra run *from every refuge at once*, refreshed
every `ROUTE_REFRESH_S` = 60 s against the current `ThreatField`. Every node in
the network gets a cost-to-nearest-refuge and a next-hop in one solve — 6.4 ms
for 61k nodes, both modes, vs. ~750 separate per-agent searches. An edge whose
danger crosses `IMPASSABLE` is dropped from the graph entirely; a merely-smoky
edge is kept but its cost is inflated `(1 + 40*danger)×`, so a longer clean route
beats a short dangerous one, but the model won't strand someone over minor smoke.
Two separate fields exist for car and foot, since a footpath network is a
superset of the drivable one.

A traveller advances hop-by-hop by consulting `RouteField.next` at each node
it reaches (`pick_next_hop`):

- **Standing on a refuge** (`cost == 0`) → `mark_safe`.
- **No route out** (`next == NO_NODE`) → a car with no drivable escape but a
  live *foot* escape **abandons the vehicle** and continues walking
  (`free_speed` drops to 1.1 m/s) — this is the mechanism behind "cars
  abandoned on a cut road." If foot is also unreachable, the traveller (and
  household) become `Cutoff`/`Trapped` and stop.
- Otherwise, advance toward `next`, tracking which edge (for congestion) it's
  currently on.

**Leaving the map.** OSM ways run slightly past the 10.24 km window edge —
the A10 and the Aurelia both do — and driving off the map counts as reaching
safety (CLAUDE.md finding #10): any traveller whose position leaves
`scn.world` is marked safe.

---

## Refuges (`refuge.rs`)

Refuges are **measured, not authored** — the same lesson learned the hard
way with ignition placement. A candidate road node qualifies if:

- it's **drivable**, and
- either it sits within `EDGE_M` = 120 m of the window edge (an exit), or
  fewer than `MAX_BURNABLE_FRAC` = 12% of fire cells within
  `CLEAR_RADIUS_M` = 300 m carry burnable fuel.

Candidates are sorted cleanest-first and greedily kept `MIN_SPACING_M` =
600 m apart, up to 12 refuges. On the shipped scenario this selects the
waterfront, the dense town core, the port, and the larger car parks — real
Ligurian assembly points, not an eyeballed guess. A refuge the fire has
already reached is excluded live, at solve time (`threat.at(refuge) >=
DANGER_CUT`), so a refuge can stop counting mid-scenario.

---

## The evacuation lever

The commander's lever over the civilian model is **when and where to issue the
evacuation order.**

- `order_evacuation(centre, radius_m)` — flags every household within
  `radius_m` of `centre` as `ordered` (used by the UI's click-a-zone flow,
  `ui.rs`).
- `order_evacuation_all()` — orders everyone, regardless of distance (bound
  to the `e` key, and to `SPOTORNO_ORDER_AT` for scripted runs).

Both only set `ordered = true` and stamp `ordered_at_s`; everything from
there — the channel delay, whether the household trusts the order, whether
it's the type to wait for one at all — plays out through perception and
decision above. There's no way to target the order more precisely (e.g. "all
`WaitAndSee` households") — it's geographic only, which matches what a real
incident commander can actually do (broadcast to an area, not to a
psychographic segment).

---

# Part 2 — Suppression (`suppression.rs`)

The second lever, and the only one that acts on the fire rather than on the
people. A `Unit` is an agent in the same sense a household is: a position on the
real network, a state, a task it was given, consumables it runs out of, and a
safety rule that will override the order it was handed.

There is no perception stage. A unit does what it is told, until it is told
something else or the fire makes the order unsurvivable — which is the honest
model of a crew under command, and it is what puts the decisions on the player
instead of on a policy the player cannot see.

## The three kinds, and why they are not interchangeable

| | Moves on | Acts by | Runs out of | Held back by |
|---|---|---|---|---|
| **Hand crew** ×3 (`Squadra A–C`) | roads *and* tracks, then on foot | cutting line — permanent fuel removal | nothing | `LINE_M_PER_H` = 120 m/h in macchia |
| **Engine** ×3 (`Autobotte 1–3`) | drivable roads only | water | `ENGINE_TANK_L` = 2,500 L, in 6 min of pumping | `ENGINE_REACH_M` = 60 m of hose |
| **Air tanker** ×2 (`Canadair 1–2`) | straight lines, over everything | `TANKER_LOAD_L` = 6,137 L per swath | nothing, but every cycle costs minutes | `AIR_RESPONSE_S` = 25 min to arrive at all |

Those three constraints are the whole game. The engine is fast and useless away
from a road; the crew reaches anywhere and cuts line slower than the fire
spreads; the aircraft hits anything but takes 25 minutes to show up and its water
wears off. Every number is sourced in its own doc comment — published hand-line
production rates for Mediterranean shrub, a real `autobotte`'s tank and pump, a
CL-415's load and cruise — because a serious game that invents its production
rates is just a game.

The roster is deliberately thin (`DEFAULT_ENGINES`/`_CREWS`/`_TANKERS`): a
Ligurian initial attack is two or three engines and a couple of volunteer squads,
with air support requested and waited for. A commander who can solve the scenario
with what is already on scene is not being asked anything.

Units stage at the **measured refuges** (`refuge.rs`), sorted nearest-the-fire
first by `sim.rs`'s `staging()`. A refuge is already known to be out of the fuel
and reachable by vehicle, which is exactly what a staging area needs to be — the
same "measure it, don't author it" rule that picks the refuges and the ignition.

## Tasks

`Task` is the commander's entire vocabulary:

| Task | Who | What happens |
|---|---|---|
| `Hold` | any | Stand by where you are. |
| `Attack { at }` | crew, engine | Work the fire there. Kind-specific — see below. |
| `Line { from, to }` | crew only | Cut a fuel break along that alignment. |
| `Drop { at }` | air only | One load there, then back for another until re-tasked. |
| `Return` | any | Back to staging and wait. |

`Suppression::assign` is the only way in, and it **returns the reason** an order
cannot be taken rather than failing silently: "an engine cannot cut line — send a
hand crew", "not on the incident: request air support first". The UI shows those
verbatim. `request_air()` is separate, because asking for aircraft is its own
decision with its own 25-minute price.

An **inbound** aircraft can be briefed before it arrives (`Unit::assignable` is
true while `Unit::on_scene` is false), and `arrive_if_due` puts it straight to
work when it lands. That is what happens on a real incident, and it is the
difference between air support that starts working the moment it is overhead and
air support that circles waiting to be noticed.

### `Attack` means something different per kind

- **Engine**: drive as close to the point as the *drivable* network gets, then
  wet the suppressible fuel within hose reach of **where it ended up** — not of
  where it was sent. An engine parked 600 m short because that is where the
  tarmac ends is still doing the most useful thing available to it: wetting the
  fuel beside the road it is standing on. `Unit::note` says the ordered point was
  out of reach, so the shortfall is visible rather than silent. Water goes on the
  four hottest cells in range (`reachable_targets`), which is what makes the tank
  matter: 2,500 L over 1,600 m² is ~45 moisture points, just past extinction, so
  an engine can genuinely hold a couple of cells and nothing wider.
- **Hand crew**: direct attack by a hand crew *is* cutting line at the fire's
  edge, so the order is rewritten into a `Line` across the fire's approach
  (`crew_alignment`: perpendicular to the nearest burning cell, 90 minutes of
  production long — as much as the crew can actually finish) and runs through the
  same code. Two models of one activity would be one too many.

### Consumables and cycles

- **Engine**: pumps at `ENGINE_PUMP_LPM` = 400 L/min, so a tank is six minutes of
  work; then it drives itself to the nearest of the map's 102 **hydrants**,
  refills at `HYDRANT_LPM` = 1,000 L/min, and resumes the task it was on
  (`begin_refill` stores it in `resume`). Ninety minutes of tasking is therefore
  seven tank-loads and six round trips, which is most of what an engine's day is.
- **Air tanker**: `AirLeg` cycles `ToTarget → drop → ToWater → Scooping →
  ToTarget`, scooping from the 13 mapped **open water** bodies at `SCOOP_S` = 90 s
  per pass, flying at `TANKER_SPEED` = 80 m/s. With water close, that is a drop
  every ~2.5 minutes.
- **Hand crew**: cuts at `LINE_M_PER_H` along the alignment, position tracking
  the cut head so the crew is drawn where the work is. `line_done_m` is its own
  field rather than derived from position, so a crew that withdraws and comes back
  does not start over.

## Safety overrides orders

- A unit will not work where the threat field reads `WORK_LIMIT` = 0.35 or above
  — *below* the civilians' `IMPASSABLE` = 0.55, deliberately. Firefighters are not
  civilians who happen to be braver; they are people with a stated safety margin,
  and they disengage while they still can. State becomes `Withdrawing`, the note
  says why, and the unit heads for staging.
- It can still be caught: `heat_s` accumulates above `IMPASSABLE` exactly as a
  civilian's does, and `BURNOVER_S` = 90 s of it is `UnitState::Lost`. That is
  reachable only by the fire moving onto the unit, never by obedience — which is
  the property `a_unit_sent_into_the_fire_withdraws_instead_of_dying` pins.

## Routing: a field *and* a search

The two models need opposite things from the same graph, and this is the clearest
place in the project to see why one algorithm is not "the" right one:

| | Civilians | Units |
|---|---|---|
| Destination | the *same* one for everybody — nearest refuge | a *different* one each — wherever the commander pointed |
| Solution | one multi-source Dijkstra from all refuges, 6.4 ms, once a minute (`network::solve`) | one A* per unit (`network::route`), throttled to `REROUTE_S` = 60 s |
| Count | 750 agents, one solve | ~8 units, one search each |

A per-agent search for 750 civilians would be absurd; a refuge field cannot
express "go *there*". Both treat danger the same way — an edge past `DANGER_CUT`
is dropped, a smoky one is inflated `(1 + 40·danger)×` — except that `route`
allows the *destination* to be dangerous, because that is frequently exactly
where the work is.

Two subtleties that cost real debugging (CLAUDE.md findings #17–18):

- **`nearest_reachable`, not `nearest`.** OSM tags plenty of inland farm track as
  drivable, and those stubs connect to nothing, so the nearest drivable node to a
  point up in the macchia is routinely an island. Road components are labelled
  once at build (`RoadNetwork::component`, a flood fill per travel mode), so
  "can this unit get there at all" is an O(1) question and the answer is "drive as
  close as the network gets".
- **An empty route is not a request for one.** A unit that has arrived has an
  empty route for the rest of the incident; re-planning on that condition ran a
  61 k-node A* per unit per sub-step. The trigger is the *target* moving or
  `REROUTE_S` elapsing.

Movement is sub-stepped at `SUBSTEP_S` = 4 s for the same reason the civilians'
is, and it matters more here: a unit's duty cycle is a chain of transitions —
arrive, pump dry, drive to a hydrant, fill, drive back — and each is only noticed
at a step boundary. Before the sub-step loop, water delivered differed by 20%
between a 2 s and a 60 s step even though every rate was already per-second.

## What the work does to the fire

`Suppression::step` **returns** its interventions rather than applying them; the
caller (`sim.rs`) hands them to `FireSim::queue`, and the core applies them as one
merged boundary-condition event on its next advance. That is one 2 s step of
latency, and it buys the units the ability to read the threat field and fire state
immutably while deciding.

Both routes into the core already existed as boundary conditions
(`fire::intervention`), so suppression is expressed in the model's own terms:
`Fireline` → `vegetation_changes` (fuel set to a non-burnable class, permanent),
`Water` → `additional_moisture` (percentage points, decaying 1%/min, ~69 min half
life).

**Suppression acts on the fuel ahead of the front, never on the flames.** The
kernel never un-lights a cell — burn-out is `fire`'s own ageing layer — so wetting
the burning edge does exactly nothing. Every unit targets
`FireSim::is_suppressible` (burnable, unburnt, not already cut). This is the same
class of always-negative as houses never burning: the intuitive action reads as
fighting the fire and is measurably worthless.

The calibration that decides whether any of this matters is
`MOISTURE_POINTS_PER_LITRE` = 30, derived from fuel load rather than guessed, and
measured against the core's 30-point moisture of extinction
(`crates/fire/tests/suppression.rs`, 2 h on the shipped scenario):

| Action | Coverage | Moisture added | Area vs. free-burning |
|---|---|---|---|
| 60 m cut line, 1.4 km, 300 m ahead | — | — | 734 of 1,226 cells (**−40%**) |
| 8 Canadair loads over the same band | 0.56 L/m² | +17 pts | −8% |
| 20 loads (more than two aircraft could deliver) | 1.40 L/m² | +42 pts | −34% |

Water buys time; cut fuel holds ground. Which is the operationally correct answer,
and it is what makes both unit types worth having.

## Does it change the outcome?

`suppression_changes_the_outcome` (ignored by default; `cargo test -p abm
--release -- --ignored --nocapture`) plays the same fire three ways:

```
2 h at seed 42, tramontana 35 km/h, 6% moisture
  no suppression               49.0 ha
  everything at one point      45.4 ha   (487 m line, 648 kL)
  aircraft re-tasked every 5 min  38.7 ha   (491 m line, 648 kL)
```

The middle row is the intuitive plan and it wastes most of the water: the front
passes the drop point and every load after that lands in the black. Re-reading the
head of the fire off the map every few minutes is worth another 15% of the
scenario for the same litres — which is the skill the feature is there to
exercise.

---

---

# Aggregate state

`Abm::stats()` walks households, people, and travellers once per call and
buckets them by `Status`/`TravelState` for the HUD and debrief: `aware`,
`preparing`, `moving`, `safe`, `defending`, `cutoff`, `casualties` (household
counts), plus `people_safe`/`people_moving`/`people_at_risk` and
`cars_moving`/`on_foot`. `median_evacuation_s()` reports the median
departure-to-refuge time over households (not solo travellers, who started
walking from an arbitrary point at T+0 and would just measure the map).

`Suppression::stats()` does the same for the units — counts by state
(`staged`/`responding`/`working`/`refilling`/`withdrawing`/`inbound`/`lost`, plus
`unrequested` aircraft) and the cumulative work: `water_l`, `line_m`, `drops`.
`air_eta_s()` reports how long until the next aircraft is overhead. Per-unit,
`Unit::note` carries *why* a unit is not achieving what it was told to — the most
useful thing the model knows about the map, and the reason it is a field rather
than a log line.

# Where the numbers live

All the tunable constants above are `const`s at the top of
`crates/abm/src/lib.rs`, `network.rs` and `suppression.rs` — there's no config
file. Changing one is a one-line edit; there's no runtime override. The best
current end-to-end read on how they compose is the representative-run table and
the shipped-scenario numbers in CLAUDE.md.

# Testing the models without a window

Both models are deliberately Bevy-free, which is what makes a two-hour incident
with 750 households, 1,577 people and 8 units cost about a second:

```bash
cargo test --release -p abm                              # both models, ~2 s
cargo test --release -p abm -- --ignored --nocapture     # evacuation timeline,
                                                         # routing cost, and the
                                                         # suppression comparison
cargo test --release -p fire -- --ignored --nocapture     # fire-side calibration
SPOTORNO_SELFTEST=1 cargo run --release -p game           # the Bevy-only half:
                                                         # orders, resets, restart
SPOTORNO_ATTACK_AT=300 cargo run --release -p game        # unattended initial attack
```

The one thing the headless tests *cannot* cover is the wiring: resources, events,
and the reset systems that a restart depends on. That is what `SPOTORNO_SELFTEST`
is for, and it asserts the silent failures specifically — a restart that leaves
the previous run's water on the fire, or its cut fuel, or a unit still under
orders.
