//! Measurement for spec §6: what "homes lost" should mean so it responds to the player.
use demo::{spec, Run, ALL};

#[test]
#[ignore]
fn homes_lost_distribution() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap();
    for id in ALL {
        let mut r = Run::new(&dir, spec(id).unwrap(), 42).unwrap();
        r.play(&[]).unwrap();
        let f = r.fire.exposure().fields();
        let n = f.len();
        let mut d: Vec<f32> = f.iter().map(|x| x.damage).collect();
        d.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let alight = f.iter().filter(|x| x.alight).count();
        let rad = f.iter().filter(|x| x.radiant > 0.3).count();
        println!("{id}: n={n} alight={alight} damage p10={:.2} p50={:.2} p90={:.2} max={:.2} radiant>0.3 now={rad}", d[n/10], d[n/2], d[n*9/10], d[n-1]);
    }
}

#[test]
#[ignore]
fn homes_lost_by_town() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap();
    for id in ALL {
        let mut r = Run::new(&dir, spec(id).unwrap(), 42).unwrap();
        let o = r.play(&[]).unwrap();
        println!("{id}: lost={} of {} ha={:.0} caught={} safe={}", o.homes_lost, o.households, o.hectares, o.caught, o.safe);
    }
}

#[test]
#[ignore]
fn homes_lost_radii() {
    use fire::CellFire;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap();
    for id in ALL {
        for order in [false, true] {
            let mut r = Run::new(&dir, spec(id).unwrap(), 42).unwrap();
            let orders: Vec<_> = if order { vec![(0, demo::Order::EvacuateAll)] } else { vec![] };
            r.play(&orders).unwrap();
            let w = r.scn.world;
            let st = r.fire.state();
            let mut line = format!("{id} order={order}:");
            for rad in [60.0, 100.0, 150.0, 250.0] {
                let n = r.agents.households.iter().filter(|h| fire::cells_in_radius(&w, h.home, rad).iter().any(|c| st[c.row * w.fire_cols + c.col] != CellFire::Unburnt)).count();
                line += &format!(" r{rad}={n}");
            }
            println!("{line}");
        }
    }
}
