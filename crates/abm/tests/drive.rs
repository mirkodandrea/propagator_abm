//! `abm::drive`: a vehicle turns back when the fire cuts the link ahead, and
//! says so when no open road is left. The fire is painted by hand
//! (`ThreatField::paint`), so the cut is exactly where the test puts it.

use abm::drive::{Drive, Leg, Vehicle};
use abm::network::RoadNetwork;
use abm::suppression::ENGINE_SPEED;
use abm::traffic::Traffic;
use fire::threat::IMPASSABLE;
use fire::ThreatField;
use scenario::{Pos, Scenario};

const CAR: Vehicle = Vehicle { drivable_only: true, road_speed: ENGINE_SPEED, path_speed: ENGINE_SPEED };

fn dist(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn setup() -> (Scenario, RoadNetwork, Traffic, ThreatField) {
    let scn = Scenario::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let net = RoadNetwork::build(&scn);
    let traffic = Traffic::new(&net);
    let threat = ThreatField::new(scn.world);
    (scn, net, traffic, threat)
}

/// Two drivable points a couple of kilometres apart on the same network.
fn trip(scn: &Scenario, net: &RoadNetwork) -> (Pos, Pos) {
    let w = scn.world;
    let a = net.pos(net.nearest(Pos { x: w.width_m * 0.3, y: w.height_m * 0.3 }, true).unwrap());
    let b = net.nearest_reachable(Pos { x: w.width_m * 0.7, y: w.height_m * 0.7 }, true, net.nearest(a, true).unwrap()).unwrap();
    (a, net.pos(b))
}

#[test]
fn the_fire_on_the_link_ahead_turns_the_vehicle_back() {
    let (scn, net, traffic, mut threat) = setup();
    let (start, goal) = trip(&scn, &net);
    let (mut d, mut pos, mut heading) = (Drive::default(), start, 0.0);
    // The first long link of the planned route, past the first two nodes.
    assert_eq!(d.step(&mut pos, &mut heading, goal, 0.0, 0.0, CAR, &net, &traffic, &threat), Leg::Moving);
    let planned = d.route().to_vec();
    let k = (2..planned.len() - 2).find(|&k| dist(net.pos(planned[k - 1]), net.pos(planned[k])) > 35.0).unwrap_or_else(|| panic!("no long link: {} nodes, {:?} -> {:?}, links {:?}", planned.len(), start, goal, planned.windows(2).map(|w| dist(net.pos(w[0]), net.pos(w[1])) as i32).collect::<Vec<_>>()));
    let target = planned[k];
    // Drive until half-way along it.
    let mut now = 0.0;
    while !(d.route().first() == Some(&target) && dist(pos, net.pos(target)) < 0.6 * dist(net.pos(planned[k - 1]), net.pos(target))) {
        assert_eq!(d.step(&mut pos, &mut heading, goal, now, 0.5, CAR, &net, &traffic, &threat), Leg::Moving);
        now += 0.5;
        assert!(now < 3600.0, "never on the long link");
    }
    let back = d.last_node().unwrap();
    let ahead = net.pos(d.route()[0]);
    threat.paint(ahead, 1.0);
    let before = dist(pos, ahead);
    let leg = d.step(&mut pos, &mut heading, goal, now, 0.5, CAR, &net, &traffic, &threat);
    assert_eq!(d.cut_ahead, 1);
    assert!(dist(pos, ahead) > before, "drove on toward the fire: {} -> {} m", before, dist(pos, ahead));
    assert_eq!(d.route().first(), Some(&back), "not heading back to the node passed");
    // And the new way round does not go through the fire.
    if leg == Leg::Moving {
        let through = d.route()[..d.route().len() - 1].iter().any(|&n| threat.at(net.pos(n)) >= IMPASSABLE);
        assert!(!through);
        let mut t = now;
        while d.step(&mut pos, &mut heading, goal, t, 2.0, CAR, &net, &traffic, &threat) == Leg::Moving {
            t += 2.0;
            assert!(t < now + 3600.0, "never arrived the long way round");
        }
        assert!(dist(pos, goal) < 1.0);
    }
}

#[test]
fn a_goal_ringed_by_fire_has_no_road() {
    let (scn, net, traffic, mut threat) = setup();
    let (start, goal) = trip(&scn, &net);
    let g = net.nearest(goal, true).unwrap();
    for e in net.neighbours(g) {
        threat.paint(net.pos(e.to), 1.0);
    }
    let (mut d, mut pos, mut heading) = (Drive::default(), start, 0.0);
    assert_eq!(d.step(&mut pos, &mut heading, goal, 0.0, 2.0, CAR, &net, &traffic, &threat), Leg::NoRoad);
    assert_eq!(pos, start, "moved with no road");
    assert!(!d.can_reach(pos, goal, CAR, &net, &threat));
}
