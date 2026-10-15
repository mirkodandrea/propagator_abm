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
    assert!(o.hectares < none.hectares / 3.0, "{} ha against {}", o.hectares, none.hectares);
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

#[test]
fn the_engine_keeps_to_the_road_and_its_people_get_off_and_spread() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut g = Game::new(&data, "medium", 1).unwrap();
    let hex = g.hexes.nearest(g.case.ignition()).unwrap().at;
    g.pause().unwrap();
    g.move_team(0, hex).unwrap();
    g.resume();
    let team = g.teams[0].clone();
    assert!(team.crew_units.len() > 1, "the people are separate units");
    let (mut walked, mut apart) = (false, false);
    // Later drives (to refill) leave the crew working on foot: the riding rule
    // is for the trip out.
    for _ in 0..400 {
        g.run_until(g.time_s() + 60).unwrap();
        let e = g.crews.units[team.unit].pos;
        let driving = g.team_eta_min(0).is_some();
        for &c in &team.crew_units {
            let u = &g.crews.units[c];
            if driving && !walked {
                assert!(u.aboard && u.pos == e, "while the engine drives, its people ride in it");
            }
            walked |= !u.aboard && (u.pos.x - e.x).hypot(u.pos.y - e.y) > 20.0;
        }
        let pos: Vec<_> = team.crew_units.iter().filter(|&&c| !g.crews.units[c].aboard).map(|&c| g.crews.units[c].pos).collect();
        apart |= pos.windows(2).any(|w| (w[0].x - w[1].x).hypot(w[0].y - w[1].y) > 10.0);
    }
    assert!(walked, "the people get off and walk to the fire");
    assert!(apart, "they work apart");
}

#[test]
fn the_hexes_shown_for_a_move_are_those_the_game_accepts() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut g = Game::new(&data, "medium", 1).unwrap();
    let mut refused = 0;
    // at 480 min the fire has closed the roads to a hex
    for minutes in [0, 240, 480] {
        g.run_until(minutes * 60).unwrap();
        for t in 0..g.teams.len() {
            let shown = g.move_targets(t);
            let accepted: Vec<_> = g.hexes.hexes.iter().map(|h| h.at).filter(|&at| g.plan_move(t, at).is_ok()).collect();
            refused += g.hexes.hexes.len() - shown.len();
            assert_eq!(shown, accepted, "team {t} at {minutes} min");
        }
    }
    assert!(refused > 0, "the test must see a hex refused");
}
