//! The forecast «dove andrà il fuoco» (`rocca::forecast`): due only around
//! the change of wind, deterministic, and free of side effects on the game.

use std::path::{Path, PathBuf};

use rocca::forecast::{Epoch, Forecast};
use rocca::Game;

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn game_at(case: &str, seed: u64, min: i64) -> Game {
    let mut g = Game::new(&data(), case, seed).unwrap();
    g.run_until(min * 60).unwrap();
    g
}

fn same(a: &Forecast, b: &Forecast) -> bool {
    a.issued_s == b.issued_s && a.prob == b.prob
}

#[test]
fn forecast_is_deterministic_and_reads_only() {
    // `medium` turns the wind at 45 min, announced 20 min ahead
    let g = game_at("medium", 1, 30);
    assert_eq!(g.forecast_epoch(), Some(Epoch::Announced));
    let a = g.forecast().unwrap();
    let b = g.forecast().unwrap();
    assert!(same(&a, &b), "same state, same forecast");
    // one member at a time (the web build) gives the same as all at once
    let mut job = g.forecast_job().unwrap();
    while !job.step().unwrap() {}
    assert!(same(&a, &job.finish()), "stepwise and threaded runs agree");
    // the +3 h isochrone contains the +1 h one and today's fire
    assert!(a.likely_cells(1) >= a.likely_cells(0) && a.likely_cells(0) > 0);
    // asking changed nothing: the game goes on as one never asked
    let mut asked = g;
    asked.run_until(60 * 60).unwrap();
    let never = game_at("medium", 1, 60);
    assert_eq!(asked.outcome(), never.outcome());
    assert_eq!(asked.fire.state(), never.fire.state());
}

#[test]
fn forecast_due_only_around_the_change_of_wind() {
    assert_eq!(game_at("medium", 1, 20).forecast_epoch(), None);
    assert_eq!(game_at("medium", 1, 46).forecast_epoch(), Some(Epoch::Shifted));
    assert_eq!(game_at("medium", 1, 80).forecast_epoch(), None);
    assert_eq!(game_at("easy", 1, 30).forecast_epoch(), None);
}
