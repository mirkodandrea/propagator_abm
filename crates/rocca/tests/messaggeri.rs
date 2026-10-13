//! The messenger teams (`rocca::messengers`, headless trial): they drive the
//! roads like the engines, are diverted on the road, and replay exactly.

use std::path::{Path, PathBuf};

use rocca::messengers::Task;
use rocca::{Civil, Game, Messengers, Plan};
use scenario::Pos;

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn game(case: &str, teams: usize) -> Game {
    let mut g = Game::new(&data(), case, 1).unwrap();
    // the teams, not the mayor's rule: orders at any time
    g.mayor = None;
    g.messengers = Some(Messengers { teams, per_min: 2.0, broadcast: false, by_priority: true, reversed: false });
    g
}

fn dist(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Districts by distance from the first station, nearest first.
fn by_distance(g: &Game) -> Vec<usize> {
    let st = Pos::from(g.territory.stations[0].pos);
    let mut d: Vec<usize> = (0..g.districts.len()).collect();
    d.sort_by(|&a, &b| dist(g.districts[a].centre, st).total_cmp(&dist(g.districts[b].centre, st)));
    d
}

fn evacuate(g: &mut Game, prio: &[usize], evac: &[usize]) {
    let mut p = Plan::new(g.districts.len()).with_priorities(prio);
    for &d in evac {
        p = p.with_civil(d, Civil::Evacua);
    }
    g.commit(p).unwrap();
}

#[test]
fn same_seed_same_teams() {
    let run = || {
        let mut g = game("medium", 3);
        let far = by_distance(&g);
        let ds = [far[far.len() - 1], far[far.len() - 2]];
        evacuate(&mut g, &ds, &ds);
        g.run_until(60 * 60).unwrap();
        let teams: Vec<_> = g.teams.iter().map(|t| (t.pos.x, t.pos.y, t.told, t.drive.driven_m, t.task)).collect();
        (g.outcome(), g.log.clone(), teams)
    };
    let (a, b) = (run(), run());
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
    assert_eq!(a.2, b.2);
    assert!(a.2.iter().map(|t| t.2).sum::<usize>() > 0, "nobody was told: {:?}", a.2);
}

#[test]
fn the_way_there_is_driven() {
    let mut g = game("medium", 1);
    let far = *by_distance(&g).last().unwrap();
    let start = Pos::from(g.territory.stations[0].pos);
    evacuate(&mut g, &[far], &[far]);
    let crow = dist(start, g.districts[far].centre);
    let mut arrived = None;
    while arrived.is_none() && g.time_s() < 60 * 60 {
        g.step().unwrap();
        if matches!(g.teams[0].task, Task::Door { .. }) {
            arrived = Some(g.time_s());
        }
    }
    let at = arrived.expect("the team never got there");
    let t = &g.teams[0];
    assert!(t.drive.driven_m >= 0.9 * crow, "drove {} m for {crow} m as the crow flies", t.drive.driven_m);
    assert!(at as f32 >= 0.9 * crow / abm::suppression::ENGINE_SPEED, "there in {at} s for {crow} m");
}

#[test]
fn a_higher_priority_on_the_road_diverts_the_team() {
    let mut g = game("medium", 1);
    let order = by_distance(&g);
    let (near, far) = (order[0], order[order.len() - 1]);
    evacuate(&mut g, &[far], &[far]);
    g.run_until(30).unwrap();
    assert_eq!(g.teams[0].task, Task::Driving { district: far });
    evacuate(&mut g, &[near, far], &[near, far]);
    g.step().unwrap();
    assert_eq!(g.teams[0].diverted, 1);
    assert_eq!(g.teams[0].district(), Some(near));
    assert!(g.log.iter().any(|e| e.text.contains("va invece a")), "{:?}", g.log);
}
