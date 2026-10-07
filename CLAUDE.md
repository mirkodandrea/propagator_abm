# propagator_abm — wildfire incident-commander kiosk demo

An interactive kiosk for the *Settimana della Protezione Civile* (Rome): students
act as incident commander of one fictional Lazio town, Rocca Ventosa, in a
turn-based game of scarce resources, while individual families respond to a
real second-scale wildfire model (CIMA PROPAGATOR). The kiosk is
`cargo run --release -p game`; the headless `play` binary plays the same game in
text for blind agent playtests. Specs: `docs/demo-spec.md` (index and
contract), `docs/demo-spec-gameplay.md`, `docs/demo-spec-playtest.md`,
`docs/demo-spec-presentation.md`. Earlier designs are scrapped; the specs are
the only version.

## Layout

```
crates/scenario/   baked assets + coordinate frames (no Bevy)
crates/fire/       PROPAGATOR integration, exposure, threat, interventions
crates/behavior/   authored behaviour graphs (the decision layer abm runs on)
crates/abm/        civilians, roads, traffic queues, suppression units
crates/demo/       the session model: turns, resources, targets, Referee, Run, verdict
crates/text/       all player-facing Italian, shared by play and game (to build)
crates/play/       headless text front end for blind playtests (to build)
crates/game/       Bevy kiosk: shell (kiosk/), terrain, buildings, people, fire
data/              the baked demo towns and the behaviour library
assets/            CIMA brand mark, embedded meshes
```

Dependency direction is strict: `scenario`/`behavior` are leaves; `fire` ->
`scenario`; `abm` -> all three; `demo` -> model crates; `game` -> everything.
Headless tests (`cargo test --release`) must stay window-free. `demo::Run` is the
headless twin of a live session; both step in `demo::STEP_S` chunks.

Ownership: gameplay owns the model crates and `data/`; presentation owns
`crates/game`, `assets/` and shaders. The contract is the `demo::` types.

## Rendering and input rules

- World frame: metres, +x east, +y north, origin SW corner. Bevy is Y-up, north
  is -Z; the flip lives only in `crates/game/src/frame.rs` (`to_bevy`/`to_world`).
- `Terrain::height_at()` (5 m field) places anything on the ground.
- **Diorama:** the 4 km window is the tile. `plinth.rs` adds strata walls and the
  table; one fixed sun, no clock; depth of field is the tilt-shift read. Houses
  are drawn at `TOY_SCALE` so the town reads from 2 m.
- **No single-key shortcuts** (finding 25): `UiFocus::keyboard` is held true for
  the whole session. The operator corner (hold top-right) is the only hidden UI.
- Env harnesses (test/ops only): `KIOSK_SHOT=<dir>` walks a session and
  photographs it, with `KIOSK_TOWN=borgo|valle|porto`, `KIOSK_WINDOWED=1`,
  `KIOSK_PLAY_S=<s>`, `KIOSK_SHOT_ZOOM=<k>`, `KIOSK_SHOT_FOCUS=x,y`;
  `KIOSK_FPS=1` logs the frame rate; `KIOSK_IDLE_S=<s>` sets the idle reset
  (default 60).
- **The game is turns of resource → target orders** on Rocca Ventosa's three
  districts (a household's `locality`); every number the player sees, in the
  kiosk or in `play`, comes from one `demo::Referee` that the twin also keeps.
- Assets: `scripts/build_town_models.py` (Blender) bakes the town kit to
  `assets/models/town.json`; `scripts/generate_demo_scenarios.py` builds the
  towns.

## Hard-won findings — do not rediscover these

Each of these cost real debugging time and is invisible from the code.

**2. Houses can never burn in the CA.** Buildings sit on non-vegetated fuel
cells, which are non-burnable by definition, so a house cell never enters the
fire mask — a 48 ha fire produced *zero* burning household cells at every
timestep. This is a silent always-negative, not an error. Structure threat is
therefore its own layer (`crates/fire/src/exposure.rs`). The same applies to
any "did the fire reach X" query: roads and people are on non-burnable cells
too.

**3. Single-cell ignitions fizzle ~20% of the time** at `realizations=1`. Over
seeds 1–20, four never established. Seed 42 was one of them, which made a
correct integration look broken. Always use `FireSim::ignite_patch`. Regression
test: `crates/fire/tests/seeds.rs`. A 20-realization Python ensemble hides this
completely.

**5. Anything accumulated per update call is a bug.** Damage originally
accrued per `update()`, making structure loss depend on the caller's step size
— the game steps every 2 s, batch tests every 300 s, a 150× difference for the
same fire. Integrate over simulated time. Test:
`damage_is_independent_of_step_size`.

**7. There are two threat layers, and they are not interchangeable.**
`fire::exposure` answers "is this *building* being destroyed" — slow,
integrated, at ~750 fixed points, ember reach out to 2.5 km.
`fire::threat::ThreatField` answers "is it survivable to *stand here*" —
instantaneous, sampled anywhere, ember reach capped at 400 m. Firebrands
destroy houses hours later at distances that do not threaten a pedestrian;
using one field for both makes either the evacuation absurdly panicky or the
structure loss absurdly local.

**11. A back-facing ribbon is invisible, not inside-out.** The road network
existed, was built correctly, logged nothing wrong and drew *nothing at all*
for two commits: the quad strip alternates right/left across the centreline, so
the obvious index order (`a, a+1, a+2`) winds it clockwise seen from above, the
faces point at the ground, and `StandardMaterial`'s back-face culling discards
the lot. Wind ground-facing ribbons `a, a+2, a+1` — verified for the roads and
the ignition rings. Any new draped strip needs the same check, and the check is
"can I see it", because nothing else will tell you.

It came back on the far-terrain skirt, and copying working code is what did it:
`terrain_mesh` walks its lattice **north to south** (row 0 is the DEM's north
edge) while `far_terrain` walks south to north, so the same index order winds
the two meshes opposite ways. The whole skirt, and every distant house, was
culled — the world outside the window was sky — while the logs happily reported
314 k triangles. The only survivors were the cardboard trees, whose material
sets `cull_mode: None`. **A lattice's winding depends on which way its rows
run, not on the index expression that produced it.**

**12. Draping only samples the terrain where there is a vertex.** OSM ways carry
vertices where the road *bends*, so a straight run over a ridge can be a single
200 m segment — which drapes as a chord straight through the hill and
disappears into it. Resample to the render posting (5 m) before offsetting.
Same trap for any polyline laid on the ground.

**13. Anything drawn on the ground is under the canopy.** The vegetation is
5–15 m of actual plants, so a marker lifted the half-metre that clears
z-fighting is rendered perfectly and seen never. The ignition rings sit at
+20 m, next to the household beacons (+22 m) and refuge markers (+30 m). A
"correct" overlay that nobody can see looks exactly like a broken one.

**14. `vegetation_changes` is sparse *by NaN*, and getting that wrong is
silent.** The core writes every **non-NaN** cell of that grid into the fuel map.
The first version of `flush_interventions` built it with `Grid2::filled(rows,
cols, 0.0)` and wrote the line into it, which reclassified the entire 512×512
window as non-vegetated — so *any* intervention stopped the fire everywhere at
once. It survived review because the only test asserted the fire got smaller,
which it certainly did. Regression: `a_fireline_is_local`.

**17. The nearest drivable node is usually the wrong one.** OSM tags plenty of
inland farm track as drivable, and those stubs connect to nothing, so
`net.nearest(p, true)` for a point up in the macchia routinely returns an island.
A* then explores the whole component and fails, which is how the first engine
dispatch left every engine parked at staging with no error anywhere. Road
components are now labelled once at build (`RoadNetwork::component`) and units
ask `nearest_reachable`, which is O(1) and means "drive as close as the road
network gets" — the hose then bridges the rest, or the note says it cannot.

**18. An A* per sub-step is not free, and "route is empty" is not "needs a
route".** A unit that has arrived has an empty route for the rest of the
incident. Re-planning on that condition ran a 61 k-node search per unit per
4 s sub-step and took the model from ~1 ms to minutes per test. The re-plan
trigger is the *target* moving or `REROUTE_S` elapsing.

**19. Closing the last metres by fractions never terminates.** The crew walk-in
was `while on_foot > 0.0 { f = on_foot/d; move f; on_foot -= d*f }`, which in f32
leaves a rounding residue every iteration and spins forever — a hang, in a model
whose tests otherwise finish in a second. One straight move per sub-step covers
at most 4.4 m and needs no loop at all.

**21. A restart has to clear the *latched* view state, and only that.** Almost
every view here is recomputed from `Sim` each frame and needs no help — the
`generation` bump does it, which is why that counter is monotonic across
restarts rather than reset. The exceptions are the things that deliberately
remember: `buildings::Structure::alight_at_s` (a latch, so a house keeps burning
down after the front passes), the smoke and ember particles (simulated in the
view), and the vehicle entities (indexed into an append-only `travellers` list).
Miss one and the new run opens with the old run's charred buildings, drifting
plume, or cars parked on roads that never burnt. `SimRestarted` fans out to the
three `reset` systems; `SPOTORNO_SELFTEST=1` is what checks they ran.

**22. A custom `Material` gets no fog, and the sea is too big to get away with
it.** `StandardMaterial` applies `FogSettings` at the end of its own fragment
shader; nothing applies it for you. The water shader went without, so the one
surface in the scene that ignores the atmosphere was also the widest: it held
the same saturated blue out to the edge of its mesh while the coast beside it
hazed away properly, and the straight line where it stopped read as a slab of
blue laid over the horizon. `shaders/water.wgsl::apply_scene_fog` transcribes
`bevy_pbr::pbr_functions::apply_fog` — transcribes, because importing that
module also pulls in `pbr_bindings`, which redeclares `StandardMaterial`'s
`@group(2)` on top of the material's own.

**23. Any boundary where one surface hands off to another is a straight line
on the horizon.** A straight world-space line viewed obliquely is straight on
screen from every angle, so two surfaces pretending to be one sea can never be
made to agree — the sea/`far_terrain` handoff was rebuilt twice before the
answer turned out to be one sea: `crates/game/src/sea.rs` meshes the near water
at 20 m and continues it in four coarse bands to the same 25 km the skirt
reaches, each band's spacing derived from its own span so it lands exactly on
the inner mesh's edge. The outer boundary is trimmed to a **disc**, not the box
the lattice is built on: a horizon the same distance away in every direction
cannot resolve into a ruled line.

**25. A keyboard shortcut collides silently, and the loser is whichever system
happens to read the key second — or neither, because both run.** Three of these
shipped at once. `b` was bound to the Entities browser *and* the behaviour
composer, and both systems fired on the same press, so opening the composer also
toggled the panel behind it. `a` and `d` armed an attack and a drop *and* panned
the camera west and east, because the camera panned on WASD — which reads as the
order having a mysterious side effect, not as two bindings. And nothing was
gated on egui's keyboard focus at all, so typing a household id into the
Entities search box armed an attack, dropped a load, ordered a general
evacuation and restarted the incident, one letter at a time. Bevy's
`ButtonInput` has no notion of a binding table or of who has focus: every system
that reads it is reading the raw keyboard, so **nothing warns you, nothing
errors, and there is no place a conflict shows up except by playing**. The
answers are all structural rather than clever: one system (`menu::menubar`)
decides input ownership for the frame, every shortcut system is scheduled after
it and returns early on `UiFocus::typing()`, and the camera moved to the arrow
keys because it was the binding with a full mouse gesture already covering it.
The menu bar then makes the whole table *visible*, which is the only thing that
stops the next collision being found the same way.

**34. A derivation that is right on the window you developed it against can be
badly wrong on the next one, and the failure is a moved baseline rather than an
error.** The warning-infrastructure model (`abm::comms`) sites masts on the
highest road nodes, which is where masts visibly are and which covers 745 of
750 households on Spotorno. On `mati` it covered 316 and on `pedrogao` 139 — so
two of the four real scenarios would have opened with most of the town already
out of signal, quietly changing every evacuation figure taken on them and
looking exactly like the model working. This is finding 33 with the sting
moved: there the authored fact was wrong everywhere and here the *derived* one
was right where it was written, so nothing about developing it could have shown
it. A real operator sites for coverage, not for altitude, and the fix was to
derive it that way — greedy maximum coverage of the households, ties on
elevation — which reaches 750/750 on all four. **Run a new derivation against
every window before believing it, and make the check an assertion**
(`the_fire_takes_out_the_warning_network` pins full coverage at T+0 for exactly
this reason).

The second half is the general one. A mechanism added to a model that already
has published numbers has to be **provably inert**, and "off by default" is not
enough when the mechanism has a state at T+0. `CommsNet::covered` therefore
models the *loss* of service rather than service: somewhere no mast ever
reached is unaffected by one going down. That makes "nothing has burnt yet ⇒
nothing has changed" a property of the code rather than of the parameters.

**37. A new fire is a blob, not a cell.** Spot-fire detection asks whether a
newly burning cell has anything already alight within 120 m of it. A single
ignition patch of 200 m radius is four hundred metres across, so its far edges
are further apart than the gap and the first version reported one new fire as
**eleven** — and the per-cell dedupe that looked like the fix (skip a candidate
near one already recorded this tick) cannot work, because the blob is wider than
the radius at which two cells count as the same event. Flood-fill the detached
set against itself and record one spot per connected component, at its centroid.
The same shape of mistake is waiting in any "count the new things" pass over a
raster.

**39a. A script that rebuilds a registry from a hand-written list drops
whatever was added after the list.** `generate_synthetic_scenarios.py` wrote
`data/scenarios.json` as `[spotorno, *generated]`, which was correct while
Spotorno was the only real place — and `mati`, `pedrogao` and `rhodes` were
baked later, so the next run of it removed three scenarios from the selector
and printed "preserved Spotorno" while doing so. Nothing failed:
`Scenario::load_by_id` reads the per-scenario directory rather than the
registry, so every model test kept passing on windows the game could no longer
offer. It derives the real list from the directory now. Same lesson as finding
33, one layer out.

**39. A count of vehicles on a link means nothing until the link has a
length, and a road with no capacity has infinite capacity.** The congestion
term was `speed *= 1/(1 + 0.06·(n-1))`, floored at 0.15, with `n` the vehicles
sharing a link — and links here are OSM polyline segments, **8 m at the median
on Spotorno**. So three cars nose to tail on one segment, which is a
standstill, read as an 11% slowdown, while the coefficient needed 18 cars on
one link to halve speed and 95 to reach the floor. Measured over a full
evacuation the peak was **3 cars on any link, factor 0.89**: inert by
construction on the real window, and firing on empty road in the synthetic
labs, whose links are 320–633 m. Worse, cars only scaled their own speed —
they interpenetrated, nobody blocked anybody, and a bottleneck's discharge
rate was unbounded, so a thousand cars on one street would have crawled
simultaneously and cleared together. **A uniform slowdown is not a jam**, and
no coefficient turns one into the other.

`abm::traffic` is a spatial queue instead: per *directed* link (a northbound
queue is not a southbound one) a free-flow time, a **flow capacity** in veh/s
and a **storage capacity** in vehicles, and a car leaves a link only when it
is at the head, has covered it, the discharge gate has come round, **and the
link it wants next has room**. That last clause is the whole model — it is
what makes a queue spill back through a junction instead of evaporating.
Capacities in series do not compound, which is why applying this to 8 m
segments is sound rather than absurd; storage on one is a single vehicle,
which is what 8 m of a lane holds. `RoadNetwork::build` had been discarding
the `class` the bake carries since the beginning, so every drivable edge from
the A10 to a farm track had one speed and one capacity; it now carries
`RoadClass`, and speed, lanes and saturation flow come off it.

Three things fell out that would not have been guessed. **Rank in a queue
cannot be a served counter**: the obvious O(1) trick is `ticket - served`, and
it is wrong here because a car can leave from the *middle* of a line — burnt
over in it, or turned round by a `last_resort` branch — which a departures-
from-the-front counter cannot describe. Rank is recomputed from the FIFO order
each sub-step. **Spillback has to be decided before the car commits**: release
first and check for room afterwards and the car sits in a junction that has no
storage, and unbounded junction storage is exactly how a spillback model stops
spilling back (`peek_next_link` exists for this and for nothing else).
And **the lab could not have shown any of it**: `congestion_funnel` shipped
with 80 households, which is 51 cars over an 85-minute departure spread and a
peak of 11 on the road at once. No traffic model of any kind queues with that,
so for as long as it stood the lab could only ever report that congestion did
not happen — the same shape as finding 9's refuges and finding 35's threshold,
a test that cannot fail. It is 1,000 households now and its exit is one
residential street, and `a_queue_forms_at_the_single_exit` asserts the queue
*fires* rather than that the model runs.

On Spotorno the whole change moves the shipped evacuation figures by **at most
one household at any timestep**, measured against a worktree at the previous
commit. That is the honest answer rather than a disappointing one: 250 cars on
a 62,662-edge network is not a traffic problem, and a model that produced a
jam there would be wrong.

**40. `x.max(dt)` handles the coarse caller and silently mishandles the fine
one.** The decision layer integrated over `DECISION_S.max(dt_s)` with a comment
saying it existed so "a coarse caller does not get a slower evacuation than a
fine one" — which it did, in one direction. `DECISION_S` is 5 s and the
decision fires on the first step at or past the deadline, so at a 2 s step it
ran every **6** s and charged 5, and preparation advanced at 83% of real time;
at 4 s it ran every 8 and charged 5, and advanced at 63%. The game steps at
~2 s and every batch measurement in `crates/fire/tests` uses 10 s or more, so
the shipped game had been preparing households slower than every number ever
taken on it, in the one direction nobody would check. Nothing errored, nothing
diverged visibly, and the two populations of caller never met. Measuring the
interval that actually elapsed rather than assuming it is the whole fix, and
the sweep that found it (`traffic::step_size_sweep`) is the artefact worth
keeping: 2 s through 10 s now agree exactly, where before 4 s was 47
households behind 6 s. **A step-size invariance check has to sweep below the
model's own internal cadence, not just above it.**

**41. Somebody else's calibrated table is still a claim about your window.**
`FireSim::new` has set `do_spotting = true` since the beginning, and the core's
ember model is a real one — Poisson emission per burning cell, a landing
distance scaled by wind and fireline intensity, a delayed ignition at the far
end. It could not fire. Generation is gated on `fuels.spotting[from]`, and both
eu12 tables in `propagator_sim` flag that on **conifers alone**; conifers are
3.2% of the Spotorno window against 7.2% shrub, and of the cells that actually
burnt in two hours 706 were shrub and 146 conifer. On `mati` it was 712 shrub
against **3** conifer. Two of the four real scenarios produced **zero** spot
fires over a full incident — so `block.spot_fire`, the `reacts-to-events`
profile and the whole "a new fire starts behind you" mechanism were wired to
something that could not happen, which is the same always-negative as houses
never burning (finding 2) and wetting the flames (finding 16), arriving this
time through a data file nobody had reason to doubt.

The asymmetry is what gives it away. *Receiving* is not gated the same way:
the landing test is `P_C0 * (1 + prob_ign_by_embers)`, so a zero there is the
base probability rather than a veto and any burnable cell can catch an ember.
The table therefore said Mediterranean maquis is downwind-only — it can be lit
by a firebrand and can never throw one — which is not a defensible statement
about the fuel that carries these fires. `scripts/bake_fuels.py` now overrides
ids 7–9 to `spotting: true, prob_ign_by_embers: 0.4`, in the bake rather than
in the JSON, because a hand-edited `data/fuels_eu12.json` is silently reverted
by the next run of the script that generates it (finding 39a, one file over).

It is a fork of CIMA's table and it moved everything: **49.0 → 81.7 ha** on the
shipped two-hour scenario, **38.8 → 149.9 ha** on `mati`, and structure loss
from 1 building to 33. Nothing about the ember *range* was touched and nothing
needed to be — `d_median ∝ U · I^(1/3)` already gives a shrub run a shorter
throw than a crowning conifer one through its lower intensity. The evidence is
`crates/fire/tests/spotting.rs`, which asserts the shrub classes still carry
the flag (a re-copy from upstream is a plausible, silent regression), asserts
that calm air still cannot spot, and sweeps what a cut line is now worth.

The operational consequence is the one worth knowing: **a line 300 m ahead of
this fire sits inside its own ember shadow.** The median firebrand at 35 km/h
over a 60 MW/m front lands about 320 m downwind, so the line gets jumped, and
moving it further out is worse rather than better — at 800 m the fire that
arrives is not the one the line was cut against and the flanks have gone round
it. There is no offset that is both beyond the embers and in front of the fire,
which is a real thing about wind-driven fire in maquis rather than a bug, and
it is now the shape of the suppression game.


**43. A layer calibrated for a 10 km window saturates a 4 km town, and on
screen that is a lie, not a blur.** `fire::exposure` throws embers 2.5 km
(finding 7) — right for structure loss on Spotorno, but in a demo town every
house is inside that reach of every fire, so the building view, which tinted
houses "threatened" and then "alight" from it, showed the whole town burning
while the end card, which counts homes by burnt ground within 150 m, said no
home was hit. The view now reads the session's books (`demo::Referee`: a home
the tally counts as hit flares and chars; a threatened *district* takes a warm
cast). **Anything the player sees and anything the end card counts must come
from the same rule** — the same lesson as `reached` vs `caught` in
`demo::district` (a district "spared" with families caught in it).

**44. A fair judgement and a timely one can be different times, and then the
mechanism has no right setting.** Cry-wolf judges an order needless if the fire
has not come within 300 m of the households it moved after `JUDGE_AFTER_S`.
On the district towns the districts the fire really reaches are first
threatened at T+21–29 median, up to T+46, while a warning stops paying by about
T+12. At ten minutes the meter punished every *correct* early warning — the
mechanism taught the opposite of its lesson, and nothing errored; at thirty it
is fair but can no longer cost an order that matters. Measure when the thing
being judged actually happens (`district_probe::threatened_times`) before
choosing when to judge it; here the answer was to judge in hindsight, on the
end card, and leave the in-session mechanism off.

**45. Refuges are measured, so a synthetic town can have none.** A refuge is a
drivable node with at most 12 % burnable fuel within 300 m (`abm::refuge`). A
small town ringed by gardens and maquis never qualifies, so the only refuges
were the map-edge exits — and Porto's single exit was the end of the road
through the fire, so warned families drove into it and were counted trapped.
Nothing failed; the evacuation just did not work. Each demo town now has an
*area di attesa* cleared wide enough to pass the test, and
`districts::every_town_has_three_districts_and_an_assembly_area` asserts a
non-exit refuge exists.

## Working agreements

- Prefer measuring to reasoning; read the screenshot, a correct overlay nobody
  can see looks broken (finding 13).
- A new mechanism must be provably inert, not merely off (finding 34).
- Anything accumulated per update call is a bug (finding 5).
