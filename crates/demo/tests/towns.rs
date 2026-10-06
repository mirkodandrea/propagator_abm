//! What is true of the three demo towns: they load, route, react to an order,
//! and each teaches its beat (spec §5, §10) -- asserted as 'fires', not 'runs'.

use demo::{spec, Order, Run, ALL};
use scenario::{Pos, Scenario};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

#[test]
fn the_towns_load_and_their_roads_are_connected() {
    for id in ALL {
        let scn = Scenario::load_by_id(data_dir(), id).unwrap();
        assert_eq!(scn.population.households.len(), scn.metadata.households_count, "{id}");
        assert!((150..=400).contains(&scn.population.households.len()), "{id}: spec §5 size");
        let net = abm::network::RoadNetwork::build(&scn);
        let comps: std::collections::HashSet<_> = (0..net.len())
            .map(|n| n as u32)
            .filter(|&n| net.is_drivable_node(n))
            .map(|n| net.component(n, true))
            .collect();
        // Roads that cross without a shared vertex leave everyone `cutoff`.
        assert_eq!(comps.len(), 1, "{id}: drivable roads are disconnected");
        for h in &scn.population.households {
            assert!(net.nearest(Pos { x: h.pos[0], y: h.pos[1] }, false).is_some(), "{id}: off-network");
        }
    }
}

#[test]
fn an_order_gets_people_out_in_every_town() {
    for id in ALL {
        let mut none = Run::new(&data_dir(), spec(id).unwrap(), 1).unwrap();
        let mut ordered = Run::new(&data_dir(), spec(id).unwrap(), 1).unwrap();
        let a = none.play(&[]).unwrap();
        let b = ordered.play(&[(0, Order::EvacuateAll)]).unwrap();
        assert!(b.safe > a.safe, "{id}: an order at T+0 moved nobody ({} vs {})", b.safe, a.safe);
    }
}

/// Five-seed mean of households the fire reached while still at home.
fn caught(id: &str, order_at: Option<i64>, only: impl Fn(Pos) -> bool + Copy) -> f32 {
    let orders: Vec<_> = order_at.into_iter().map(|t| (t, Order::EvacuateAll)).collect();
    (1..=5u64)
        .map(|seed| {
            let mut r = Run::new(&data_dir(), spec(id).unwrap(), seed).unwrap();
            r.play(&orders).unwrap();
            r.caught_where(only) as f32
        })
        .sum::<f32>()
        / 5.0
}

/// The beat of the two towns whose lesson is *when* the order is given: the
/// earlier it goes out, the fewer families are still at home when the fire
/// arrives -- strictly, at every step, with a margin that is not noise.
#[test]
fn borgo_and_porto_the_earlier_the_order_the_fewer_families() {
    for id in ["demo_borgo", "demo_porto"] {
        let (none, t0, t10, t40) = (
            caught(id, None, |_| true),
            caught(id, Some(0), |_| true),
            caught(id, Some(600), |_| true),
            caught(id, Some(2400), |_| true),
        );
        println!("{id}: none {none}  T+0 {t0}  T+10 {t10}  T+40 {t40}");
        assert!(t0 <= t10 + 1.0 && t10 + 5.0 < t40, "{id}: a late order should cost families ({t0}/{t10}/{t40})");
        assert!(t0 + 5.0 < t40, "{id}: T+0 ({t0}) should clearly beat T+40 ({t40})");
        assert!(t0 * 1.5 < none, "{id}: the counterfactual is too close to the player's best ({t0} vs {none})");
    }
}

/// `demo_valle`: the second hamlet is safe until the wind turns, and an order
/// given any time before the turn is what saves it.
#[test]
fn valle_the_wind_shift_reaches_the_second_hamlet() {
    let b = |p: Pos| p.x > 2400.0;
    let none = caught("demo_valle", None, b);
    let early = caught("demo_valle", Some(0), b);
    println!("valle B: none {none} early {early}");
    assert!(none >= 5.0, "the wind shift no longer threatens hamlet B ({none} caught)");
    assert!(early * 2.0 < none, "an order should save hamlet B ({early} vs {none})");
}

#[test]
fn a_run_is_deterministic() {
    let go = || Run::new(&data_dir(), spec("demo_borgo").unwrap(), 7).unwrap().play(&[(600, Order::EvacuateAll)]).unwrap();
    assert_eq!(go(), go(), "the COMPARE screen depends on this");
}
