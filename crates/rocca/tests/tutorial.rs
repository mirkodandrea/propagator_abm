//! The tutorial's promise (docs/01-GIOCO.md §1): with no orders its small
//! fire reaches the district; played as the script asks, nobody is caught,
//! no home is hit and the district is evacuated.

use std::path::Path;

use rocca::tutorial::Script;
use rocca::{Game, State};

#[test]
fn the_scripted_tutorial_saves_the_district() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let none = Game::without_orders(&data, "tutorial", 1).unwrap();
    assert!(none.caught() > 0 && none.homes_hit() > 0, "with no orders the fire must reach the homes: {none:?}");
    let mut g = Game::new(&data, "tutorial", 1).unwrap();
    let s = Script::of(&g).unwrap();
    s.play(&mut g).unwrap();
    let o = g.outcome();
    assert_eq!((o.caught(), o.homes_hit()), (0, 0));
    let d = g.district_index(&g.territory.tutorial.clone().unwrap().evacuate).unwrap();
    assert_eq!(g.evac.count(Some(d), State::Safe), o.districts[d].people);
    assert!(o.hectares < none.hectares / 4.0, "{} ha against {}", o.hectares, none.hectares);
}

#[test]
fn orders_only_in_pause_and_one_evacuation_at_a_time() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut g = Game::new(&data, "medium", 1).unwrap();
    let hex = g.hexes.nearest(g.case.ignition()).unwrap().at;
    assert!(g.move_team(0, hex).is_err());
    assert!(g.evacuate(0).is_err());
    g.pause().unwrap();
    g.move_team(0, hex).unwrap();
    let a = g.district_index("Le Terrazze").unwrap();
    let b = g.district_index("Castelvento").unwrap();
    // in the same pause the player may change their mind
    let people = |g: &Game, d: usize| g.districts[d].households.iter().filter(|&&i| g.evac.families[i].state == State::Home).count();
    let homes_a = people(&g, a);
    g.evacuate(a).unwrap();
    assert_eq!(people(&g, a), 0);
    g.evacuate(b).unwrap();
    assert_eq!(people(&g, a), homes_a, "the first choice is taken back whole");
    assert_eq!(g.evac.active().unwrap().district, b);
    g.cancel_evacuation().unwrap();
    assert!(g.evac.active().is_none());
    g.evacuate(a).unwrap();
    g.resume();
    assert!(!g.can_pause(), "the expert needs time before the next pause");
    g.run_until(g.time_s() + rocca::PAUSE_COOLDOWN_S).unwrap();
    assert!(g.can_pause());
    // after the pause the evacuation under way stands: one at a time
    g.pause().unwrap();
    if g.evac.active().is_some() {
        assert!(g.evacuate(b).is_err());
        assert!(g.cancel_evacuation().is_err());
    }
}
