//! Does suppression actually do anything, and how much?
//!
//! These are calibration tests as much as regression tests. The unit model in
//! `abm::suppression` decides how much water an engine, a hand crew and a
//! Canadair can put on the ground per minute; whether those numbers add up to
//! a fire that can be fought is a property of the *core's* response to added
//! moisture and removed fuel, and it has to be measured rather than assumed.
//!
//! Everything here runs against the shipped scenario's own ignition and
//! weather, because a suppression effect measured on flat uniform fuel would
//! tell us nothing about a 35 km/h tramontana on Ligurian macchia.

use fire::{cells_along, cells_in_radius, CellFire, FireSim, Intervention, Weather};
use scenario::{Cell, Pos, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data dir")
}

const START_RADIUS_M: f32 = 250.0;
/// The full initial-attack window. Not negotiable downward: the head of this
/// fire is only ~300 m past the ignition patch at T+45 min, so a shorter run
/// measures a line the fire never reached and reports every intervention as
/// worth about 1%.
const RUN_S: i64 = 120 * 60;
/// How far ahead of the ignition an intervention is laid. Measured, like
/// everything else here: the front crosses row +15 (300 m) at about T+60 min
/// and row +46 (920 m) by T+120, so 300 m is the offset a line both gets
/// finished before and actually gets tested by.
const AHEAD_M: f32 = 300.0;
/// Half-length of the band, metres. The fire's flanks spread ~500 m either
/// side over two hours, so a shorter line is simply outflanked.
const HALF_LEN_M: f32 = 700.0;
const STEP_S: i64 = 60;
/// The two calibration tests below average over these rather than reading one
/// draw, and that is not fussiness. `realizations = 1` is one sample of a
/// stochastic model (finding 3), and since the shrub classes started throwing
/// embers it is a *wider* one: seed 42 alone put the value of a cut line
/// anywhere between 60% and 93% of the free-burning area, so a threshold
/// tightened around it fails on a fuel change that did nothing wrong. Five
/// seeds is ~0.4 s here and is what the assertions are sized against.
const SEEDS: [u64; 5] = [42, 1, 2, 3, 4];

struct Setup {
    scn: Scenario,
    ignition: Cell,
    weather: Weather,
}

impl Setup {
    fn load() -> Setup {
        let scn = Scenario::load(data_dir()).expect("load scenario");
        let weather = Weather::default();
        let plan = fire::plan_ignition(&scn, weather.wind_dir_deg, START_RADIUS_M);
        Setup { scn, ignition: plan.centre, weather }
    }

    fn start(&self) -> FireSim {
        self.start_seeded(42)
    }

    fn start_seeded(&self, seed: u64) -> FireSim {
        let mut sim = FireSim::new(&self.scn, self.weather, seed).expect("core");
        sim.ignite_patch(self.ignition, START_RADIUS_M, &self.scn)
            .expect("ignite");
        sim
    }

    /// Run to `RUN_S`, calling `act` before each step so an intervention can be
    /// timed rather than dumped at t=0.
    fn run_seeded(&self, seed: u64, act: &mut impl FnMut(&mut FireSim, i64)) -> FireSim {
        let mut sim = self.start_seeded(seed);
        while sim.time_s() < RUN_S {
            let now = sim.time_s();
            act(&mut sim, now);
            sim.advance(STEP_S).expect("advance");
        }
        sim
    }

    /// Mean burnt cells over [`SEEDS`].
    fn mean_burnt(&self, mut act: impl FnMut(&mut FireSim, i64)) -> f32 {
        let total: usize =
            SEEDS.iter().map(|s| burnt_cells(&self.run_seeded(*s, &mut act))).sum();
        total as f32 / SEEDS.len() as f32
    }
}

fn burnt_cells(sim: &FireSim) -> usize {
    sim.state().iter().filter(|s| **s != CellFire::Unburnt).count()
}

/// A band across the downwind side of the ignition, [`AHEAD_M`] ahead of it.
/// The wind blows *from* north (see the project's wind-direction note), so
/// downwind is south, which is increasing row on the raster.
fn band_ahead(scn: &Scenario, centre: Cell) -> Vec<Cell> {
    let c = scn.world.centre_of(centre);
    let a = Pos { x: c.x - HALF_LEN_M, y: c.y - AHEAD_M };
    let b = Pos { x: c.x + HALF_LEN_M, y: c.y - AHEAD_M };
    // Only the burnable cells: clearing rock or wetting a road is not work
    // anyone would do, and counting it would flatter the intervention.
    cells_along(&scn.world, a, b, 30.0)
        .into_iter()
        .filter(|c| scn.is_burnable(*c))
        .collect()
}

#[test]
fn fireline_ahead_of_the_front_holds_it() {
    let s = Setup::load();
    let free = s.mean_burnt(|_, _| {});

    // A cut line 300 m downwind, 60 m wide, 1.4 km across: far more than the
    // crews in `abm::suppression` can cut in two hours, which is why it is
    // measured separately -- this is the ceiling a commander is playing against.
    let line = band_ahead(&s.scn, s.ignition);
    assert!(line.len() > 100, "line is only {} cells", line.len());

    let held = s.mean_burnt({
        let line = line.clone();
        move |sim, t| {
            if t == 0 {
                sim.queue(Intervention::fireline(line.clone()));
            }
        }
    });

    println!(
        "fireline: {free:.0} cells free, {held:.0} held ({:.0}% of free), mean of {} seeds",
        held / free * 100.0,
        SEEDS.len()
    );
    // Measured at 1,645 of 1,936 cells over two hours -- a 15% saving for 1.4 km
    // of 60 m line, mean of five seeds. It was 24% before the shrub classes
    // started throwing embers, and the reason it fell is worth knowing rather
    // than tuning away: the median ember here lands about 320 m downwind
    // (`d ~ U * I^(1/3)`, 35 km/h over a 60 MW/m front), so a line at 300 m sits
    // *inside* the ember shadow and gets jumped. Moving it further out does not
    // help either -- see `fire/tests/spotting.rs`, which sweeps the offset --
    // because the fire that clears it is no longer the one the line was cut
    // against. Held loosely so a fuel or weather retune does not fail it for
    // being a slightly different good answer.
    assert!(
        held < free * 0.92,
        "a 60 m cut line 300 m downwind saved almost nothing ({held:.0} vs {free:.0})"
    );
    // Not zero: the fire still burns everything upwind of the line, and
    // spotting crosses it. What must not happen is the *whole map* going
    // non-burnable, which is what a non-NaN vegetation_changes fill does.
    assert!(
        held > free / 4.0,
        "fire nearly vanished ({held:.0} vs {free:.0}): the fuel map was probably \
         wiped rather than the line cut"
    );
}

#[test]
fn a_fireline_is_local() {
    // The regression for the bug the NaN fill fixed: cells away from the line
    // must keep their fuel, and the line's own cells must lose it.
    let s = Setup::load();
    let line = band_ahead(&s.scn, s.ignition);
    let mut sim = s.start();
    sim.queue(Intervention::fireline(line.clone()));
    sim.advance(STEP_S).expect("advance");

    let cleared = sim.cleared().iter().filter(|c| **c).count();
    assert_eq!(cleared, line.len(), "cleared set is not exactly the line");
    assert!(
        cleared * 200 < sim.cleared().len(),
        "{cleared} of {} cells cleared -- far more than the line",
        sim.cleared().len()
    );
    for c in &line {
        assert!(sim.is_cleared(*c));
    }
}

#[test]
fn water_buys_less_than_a_line_and_more_when_it_is_heavier() {
    let s = Setup::load();
    let free = s.mean_burnt(|_, _| {});
    let swath = band_ahead(&s.scn, s.ignition);
    let cell_m2 = (s.scn.world.cellsize * s.scn.world.cellsize) as f64;
    let loads = |n: f64| n * 6137.0 / (swath.len() as f64 * cell_m2);

    let wet = |lpm2: f64| {
        s.mean_burnt({
            let swath = swath.clone();
            move |sim, t| {
                if t == 0 {
                    sim.queue(Intervention::water(swath.clone(), lpm2));
                }
            }
        })
    };

    // Eight Canadair loads spread over 1.4 km of front -- a realistic sortie
    // for a pair of aircraft -- against twenty, which is more than the two
    // could physically deliver in the window. The point of the pair is the
    // *shape*: coverage below the core's moisture of extinction slows the
    // fire, coverage above it stops that band of fuel outright.
    let (light, heavy) = (loads(8.0), loads(20.0));
    let (slowed, stopped) = (wet(light), wet(heavy));
    let pts = fire::intervention::MOISTURE_POINTS_PER_LITRE;
    println!(
        "drops over {} cells: {free:.0} free · {:.2} L/m² (+{:.0} pts) -> {slowed:.0} · \
         {:.2} L/m² (+{:.0} pts) -> {stopped:.0}  (mean of {} seeds)",
        swath.len(),
        light,
        light as f32 * pts,
        heavy,
        heavy as f32 * pts,
        SEEDS.len(),
    );
    // The light drop is deliberately *not* asserted to beat free-burning any
    // more. It never robustly did: 95% of the free-burning area before shrub
    // spotting and 100% after, both inside the seed-to-seed spread, so the "a
    // light drop saves 8%" figure this file used to report was one draw of a
    // wide distribution. What survives is the ordering -- more water is better
    // than less -- which is the shape the aircraft model actually turns on.
    assert!(
        stopped < slowed,
        "a heavier drop was not better than a lighter one ({stopped:.0} vs {slowed:.0})"
    );
    // Water is temporary: it decays at 1%/min, so even a saturating drop should
    // not hold as much ground over two hours as cutting the same band would.
    assert!(
        stopped > free / 4.0,
        "water alone extinguished the run ({stopped:.0} of {free:.0}) -- it decays, \
         and should never beat a cut line"
    );
}

#[test]
fn added_moisture_decays_like_the_core() {
    let s = Setup::load();
    let mut sim = s.start();
    let cells = cells_in_radius(&s.scn.world, s.scn.world.centre_of(s.ignition), 100.0);
    let probe = cells[0];
    sim.queue(Intervention::water(cells, 1.0));
    sim.advance(1).expect("advance");

    let fresh = sim.added_moisture_at(probe);
    assert!(
        (fresh - fire::intervention::MOISTURE_POINTS_PER_LITRE).abs() < 0.5,
        "1 L/m² should read as {} points, got {fresh}",
        fire::intervention::MOISTURE_POINTS_PER_LITRE
    );

    // 1%/minute, so an hour leaves ~55% of it.
    sim.advance(3600).expect("advance");
    let aged = sim.added_moisture_at(probe);
    let ratio = aged / fresh;
    println!("added moisture: {fresh:.1} pts fresh, {aged:.1} pts after 1 h ({ratio:.2})");
    assert!(
        (ratio - 0.55).abs() < 0.05,
        "decay over an hour was {ratio:.3}, expected ~0.55"
    );
}

#[test]
fn wetting_the_flames_is_wasted_water() {
    // Documents the finding rather than defending an assertion about area: a
    // cell the core has lit stays lit, so water on the burning front cannot
    // put anything out. `is_suppressible` is the filter that keeps the unit
    // model from spending its tank there.
    let s = Setup::load();
    let mut sim = s.start();
    for _ in 0..10 {
        sim.advance(60).expect("advance");
    }
    let active: Vec<Cell> = sim.active_cells().to_vec();
    assert!(!active.is_empty(), "no active front to test against");
    let suppressible = active
        .iter()
        .filter(|c| sim.is_suppressible(**c, &s.scn))
        .count();
    assert_eq!(
        suppressible, 0,
        "burning cells must never be offered as suppression targets"
    );

    // And the cells just ahead of it are.
    let ahead: Vec<Cell> = active
        .iter()
        .filter_map(|c| {
            let row = c.row + 3;
            (row < s.scn.world.fire_rows).then_some(Cell { row, col: c.col })
        })
        .filter(|c| sim.is_suppressible(*c, &s.scn))
        .collect();
    println!(
        "{} active cells, {} suppressible cells three rows downwind",
        active.len(),
        ahead.len()
    );
    assert!(!ahead.is_empty(), "nothing downwind is suppressible");
}
