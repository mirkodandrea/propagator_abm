//! Orders, the engine on the road, the hexes shown for a move and the group chat.

use std::path::Path;

use rocca::{Game, State};

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
    assert!(g.can_pause(), "no wait for the next pause");
    g.run_until(g.time_s() + 20 * 60).unwrap();
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

#[test]
fn the_group_chat_tells_only_what_helps_to_decide() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut g = Game::new(&data, "medium", 1).unwrap();
    assert!(g.chat.is_empty(), "nothing is said before the first order");
    g.pause().unwrap();
    g.move_team(0, g.hexes.nearest(g.case.ignition()).unwrap().at).unwrap();
    g.evacuate(g.district_index("Le Terrazze").unwrap()).unwrap();
    assert!(g.chat.is_empty(), "an order is not echoed");
    g.resume();
    g.run_until(g.time_s() + 6 * 3600).unwrap();
    for m in &g.chat {
        println!("{} {:?} {}", rocca::words::clock(m.at_s), m.who, m.text);
    }
    use rocca::Kind;
    assert!(g.chat.iter().any(|m| matches!(m.kind, Kind::EvacDone | Kind::HomesHit)), "the district's fate is told");
    assert!(g.chat.windows(2).all(|w| w[0].at_s <= w[1].at_s), "in time order");
    let same = |a: &rocca::ChatMsg, b: &rocca::ChatMsg| a.who == b.who && a.kind == b.kind && a.text == b.text;
    assert!(!g.chat.windows(2).any(|w| same(&w[0], &w[1])), "no line twice in a row");
}

#[test]
fn the_forecast_arrival_time_agrees_with_its_zones() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut g = Game::new(&data, "medium", 1).unwrap();
    g.run_until(40 * 60).unwrap();
    g.pause().unwrap();
    let f = g.forecast().unwrap();
    assert_eq!(f.eta_s.len(), f.rows * f.cols);
    for h in 0..rocca::forecast::HORIZONS {
        for i in 0..f.eta_s.len() {
            assert_eq!(f.likely(h, i), f.eta_s[i] <= rocca::forecast::HORIZONS_S[h] as f32, "cell {i}, horizon {h}");
        }
    }
    assert!(f.eta_s.iter().any(|t| t.is_finite() && *t > 0.0));
}
