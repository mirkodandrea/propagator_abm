//! Shrubs throw firebrands, and what that cost.
//!
//! `scripts/bake_fuels.py` diverges from CIMA's eu12 table in one place: the
//! three shrub classes carry `spotting: true` and `prob_ign_by_embers: 0.4`,
//! where upstream flags conifers alone. The reason is in that script's own
//! comment; this file is the measurement, because a fork of somebody else's
//! calibrated table is only defensible with numbers under it.
//!
//! Two of these assert and one reports. What is asserted is the part that
//! cannot be allowed to rot silently: that the fuel which actually carries
//! these fires generates embers, and that a fire in calm air still cannot spot
//! (the kernel's landing distance is proportional to wind speed, so zero wind
//! must mean zero spotting -- that is what lets `abm`'s detector tests hold a
//! front to contiguous growth).

use fire::{cells_along, CellFire, FireSim, Intervention, Weather};
use scenario::{Cell, Pos, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

const START_RADIUS_M: f32 = 250.0;
const RUN_S: i64 = 120 * 60;
const STEP_S: i64 = 60;
const SEEDS: [u64; 5] = [42, 1, 2, 3, 4];

/// The shrub classes, `eu_fuel12` ids 7-9.
const SHRUBS: std::ops::RangeInclusive<i32> = 7..=9;

fn burnt(sim: &FireSim) -> usize {
    sim.state().iter().filter(|s| **s != CellFire::Unburnt).count()
}

/// The shipped scenario with shrub spotting forced off, i.e. upstream's table.
/// Only a test may do this: the game reads the baked file.
fn without_shrub_spotting() -> Scenario {
    let mut scn = Scenario::load(data_dir()).unwrap();
    for d in scn.fuel_defs.iter_mut() {
        if SHRUBS.contains(&d.id) {
            d.spotting = false;
            d.prob_ign_by_embers = 0.0;
        }
    }
    scn
}

fn run(scn: &Scenario, w: Weather, seed: u64, act: impl FnOnce(&mut FireSim)) -> FireSim {
    let plan = fire::plan_ignition(scn, w.wind_dir_deg, START_RADIUS_M);
    let mut sim = FireSim::new(scn, w, seed).unwrap();
    sim.ignite_patch(plan.centre, plan.radius_m, scn).unwrap();
    act(&mut sim);
    while sim.time_s() < RUN_S {
        sim.advance(STEP_S).unwrap();
    }
    sim
}

fn mean_burnt(scn: &Scenario, w: Weather, act: impl Fn(&mut FireSim)) -> f32 {
    let total: usize = SEEDS.iter().map(|s| burnt(&run(scn, w, *s, &act))).sum();
    total as f32 / SEEDS.len() as f32
}

/// The fuel that carries these fires must be able to start one somewhere else.
///
/// Shrub is 706 of the 1,226 cells that burnt on Spotorno under upstream's
/// table, against 146 conifer, and on `mati` it is 712 of 971 against 3. With
/// generation flagged on conifers alone, the core's spotting model was switched
/// on, ran on every burning cell, and produced nothing at all on two of the four
/// real windows -- an always-negative of exactly the kind houses-never-burn and
/// wetting-the-flames are, where the mechanism reads as present and cannot fire.
#[test]
fn shrub_fuel_generates_embers() {
    let scn = Scenario::load(data_dir()).unwrap();
    let shrubs: Vec<&scenario::fuels::FuelDefRaw> =
        scn.fuel_defs.iter().filter(|d| SHRUBS.contains(&d.id)).collect();
    assert_eq!(shrubs.len(), 3, "the eu12 table no longer has three shrub classes");
    for d in shrubs {
        assert!(
            d.spotting,
            "{} does not throw embers: the bake has been re-copied from \
             upstream's table over the override in scripts/bake_fuels.py",
            d.name
        );
        assert!(d.prob_ign_by_embers > 0.0, "{} cannot be lit by an ember", d.name);
    }

    // And it shows up in the fire, not only in the table.
    let w = Weather::default();
    let with = mean_burnt(&scn, w, |_| {});
    let without = mean_burnt(&without_shrub_spotting(), w, |_| {});
    println!(
        "two hours on the shipped ignition, mean of {} seeds: {without:.0} cells \
         upstream, {with:.0} with shrub spotting ({:.1}x)",
        SEEDS.len(),
        with / without
    );
    assert!(
        with > without * 1.2,
        "shrub spotting changed the burnt area by less than 20% ({with:.0} vs \
         {without:.0}), which is inside the seed spread: it is flagged in the \
         table and doing nothing in the kernel"
    );
}

/// No wind, no spotting -- and it has to be the *code* saying so, not the
/// parameters, because `abm`'s spot-fire detector tests hold a front to
/// contiguous growth by running in calm air.
///
/// The kernel's median landing distance is linear in wind speed, so at zero the
/// draw collapses to under one cell and every ember is discarded as ordinary
/// contact spread.
#[test]
fn calm_air_cannot_spot() {
    let scn = Scenario::load(data_dir()).unwrap();
    let calm = Weather { wind_speed_kmh: 0.0, ..Weather::default() };
    let sim = run(&scn, calm, 42, |_| {});
    let spotted = sim.ember_ignited_cells();
    assert!(burnt(&sim) > 200, "the calm fire did not establish, so this proves nothing");
    assert_eq!(spotted, 0, "{spotted} cells were lit by embers in still air");
}

/// Where a cut line still holds, now that the front throws fire over it.
///
/// Reported rather than asserted: the answer is "nowhere within reach at this
/// calibration", which is a finding about the scenario and not a property to
/// pin. A 60 m line 300 m downwind saves ~15% of the burnt area where it saved
/// ~24% under upstream's table, and pushing it further out saves *less*, not
/// more -- past about 500 m the fire that arrives is no longer the one the line
/// was cut against, and the flanks have gone round it. The median ember at this
/// intensity lands about 320 m downwind, so there is no offset that is both
/// beyond the embers and in front of the fire.
#[test]
#[ignore = "calibration sweep, not an assertion"]
fn line_offset_sweep() {
    let w = Weather::default();
    for (label, scn) in [
        ("upstream", without_shrub_spotting()),
        ("shrub spotting", Scenario::load(data_dir()).unwrap()),
    ] {
        let plan = fire::plan_ignition(&scn, w.wind_dir_deg, START_RADIUS_M);
        let free = mean_burnt(&scn, w, |_| {});
        println!("\n{label}: {free:.0} cells free-burning, mean of {} seeds", SEEDS.len());
        for ahead in [300.0f32, 500.0, 800.0, 1200.0] {
            let line = band_ahead(&scn, plan.centre, ahead);
            if line.len() < 50 {
                println!("  {ahead:5.0} m ahead: only {} burnable cells, skipped", line.len());
                continue;
            }
            let held = mean_burnt(&scn, w, |sim| {
                sim.queue(Intervention::fireline(line.clone()));
            });
            println!(
                "  {ahead:5.0} m ahead ({:3} cells): {held:6.0} cells = {:3.0}% of free",
                line.len(),
                held / free * 100.0
            );
        }
    }
}

/// A 1.4 km band across the downwind side of `centre`, `ahead_m` in front of it.
/// The wind blows *from* north, so downwind is decreasing y.
fn band_ahead(scn: &Scenario, centre: Cell, ahead_m: f32) -> Vec<Cell> {
    let c = scn.world.centre_of(centre);
    let a = Pos { x: c.x - 700.0, y: c.y - ahead_m };
    let b = Pos { x: c.x + 700.0, y: c.y - ahead_m };
    cells_along(&scn.world, a, b, 30.0).into_iter().filter(|c| scn.is_burnable(*c)).collect()
}
