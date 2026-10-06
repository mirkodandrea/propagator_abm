//! Sizing the opening fire of each demo town (spec §5, finding 4/41):
//! five-seed means, ignite_patch, no order vs order at T+0 vs T+20.
//! `cargo test -p demo --release -- --ignored sizing --nocapture`

use demo::{spec, Order, Run, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

fn mean(id: &str, orders: &[(i64, Order)]) -> (f32, f32, f32, f32, f32) {
    let mut a = (0.0, 0.0, 0.0, 0.0, 0.0);
    for seed in 1..=5u64 {
        let mut r = Run::new(&data_dir(), spec(id).unwrap(), seed).unwrap();
        let o = r.play(orders).unwrap();
        a.0 += o.safe as f32 / 5.0;
        a.1 += o.in_danger as f32 / 5.0;
        a.2 += o.homes_lost as f32 / 5.0;
        a.3 += o.hectares / 5.0;
        a.4 += o.caught as f32 / 5.0;
    }
    a
}

#[test]
#[ignore]
fn sizing() {
    for id in ALL {
        let n = Run::new(&data_dir(), spec(id).unwrap(), 1).unwrap().agents.households.len();
        println!("\n{id}: {n} households");
        println!("  {:14} {:>6} {:>9} {:>6} {:>7} {:>7}", "orders", "safe", "danger", "lost", "ha", "caught");
        for (label, orders) in [
            ("none", vec![]),
            ("T+0", vec![(0, Order::EvacuateAll)]),
            ("T+10", vec![(600, Order::EvacuateAll)]),
            ("T+20", vec![(1200, Order::EvacuateAll)]),
        ] {
            let m = mean(id, &orders);
            println!("  {label:14} {:6.0} {:9.0} {:6.1} {:7.1} {:7.1}", m.0, m.1, m.2, m.3, m.4);
        }
    }
}
