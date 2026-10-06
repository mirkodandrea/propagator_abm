//! Spec 5.7: refusals are a type, and the strings the model still returns map onto it.

use abm::suppression::{Task, UnitKind};
use demo::refusal::{check, Intent, Refusal};
use demo::{spec, Run};
use scenario::Pos;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

#[test]
fn every_error_assign_returns_is_classified() {
    let s = spec("demo_borgo").unwrap();
    let mut r = Run::new(&data_dir(), s, 1).unwrap();
    let at = s.ignition;
    let id = |kind| r.crews.units.iter().find(|u| u.kind == kind).unwrap().id;
    let (engine, crew, tanker) = (id(UnitKind::Engine), id(UnitKind::HandCrew), id(UnitKind::AirTanker));
    let mut seen = std::collections::HashSet::new();
    let mut note = |res: Result<(), &'static str>| {
        let msg = res.expect_err("the order should have been refused");
        let c = Refusal::from_assign(msg).unwrap_or_else(|| panic!("unclassified refusal: {msg:?}"));
        seen.insert(c);
    };
    note(r.crews.assign(999, Task::Hold));
    note(r.crews.assign(tanker, Task::Drop { at })); // not requested yet
    note(r.crews.assign(engine, Task::Line { from: at, to: at }));
    note(r.crews.assign(crew, Task::Drop { at }));
    r.crews.request_air();
    note(r.crews.assign(tanker, Task::Attack { at }));
    for c in [Refusal::NoSuchUnit, Refusal::NotOnIncident, Refusal::EngineCannotCutLine, Refusal::OnlyAircraftDrop, Refusal::AircraftCannotWorkLine] {
        assert!(seen.contains(&c), "{c:?} was never produced by assign");
    }
    // `UnitLost` is the only assign error a quick test cannot construct (a unit has
    // to be burnt over), so classify its string directly.
    assert_eq!(Refusal::from_assign("unit is lost"), Some(Refusal::UnitLost));
}

#[test]
fn check_gives_the_same_answers_without_the_english() {
    let s = spec("demo_borgo").unwrap();
    let r = Run::new(&data_dir(), s, 1).unwrap();
    let id = |kind| r.crews.units.iter().find(|u| u.kind == kind).unwrap().id;
    let (engine, crew, tanker) = (id(UnitKind::Engine), id(UnitKind::HandCrew), id(UnitKind::AirTanker));
    let go = |unit, intent, p| check(&r.crews, &r.agents, &r.fire, &r.scn, unit, intent, p);
    let at = s.ignition;
    assert_eq!(go(999, Intent::Attack, at), Err(Refusal::NoSuchUnit));
    assert_eq!(go(tanker, Intent::Drop, at), Err(Refusal::NotOnIncident));
    assert_eq!(go(engine, Intent::CutLine, at), Err(Refusal::EngineCannotCutLine));
    assert_eq!(go(crew, Intent::Drop, at), Err(Refusal::OnlyAircraftDrop));
    assert_eq!(go(crew, Intent::Attack, Pos { x: -50.0, y: 10.0 }), Err(Refusal::OutsideScenario));
    // Ground with nothing burnable within reach has nothing to cut or wet (or no
    // road to it). Not every town has any: look for some, and say if none is found.
    let w = &r.scn.world;
    let bare = (0..w.fire_rows).step_by(5).flat_map(|row| (0..w.fire_cols).step_by(5).map(move |col| scenario::Cell { row, col })).map(|c| w.centre_of(c)).find(|&p| {
        fire::cells_in_radius(w, p, 120.0).iter().all(|&c| !r.scn.is_burnable(c))
    });
    if let Some(p) = bare {
        assert!(matches!(go(crew, Intent::Attack, p), Err(Refusal::NoFuelHere | Refusal::NoRoad)), "{:?}", go(crew, Intent::Attack, p));
    }
    // And the order that is fine is fine.
    assert_eq!(go(crew, Intent::Attack, at), Ok(()));
    assert_eq!(Refusal::ALL.len(), 11);
}
