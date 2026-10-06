//! Spec 5.1: with cost on, is following the forecast a real decision?
//! `cargo test -p demo --release --test cost_sweep -- --ignored --nocapture`

use demo::policy::Policy;
use demo::sweep::{run_grid, Record};
use demo::ALL;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

/// Mean total loss of a policy on a town: money spent plus `w_caught` per family
/// the fire caught at home plus `w_home` per home it reached. The weights are an
/// *analysis* device (the card shows facts and money, never this number).
pub fn mean_loss(rs: &[Record], town: &str, policy: usize, w_caught: f32, w_home: f32) -> f32 {
    let sel: Vec<_> = rs.iter().filter(|r| r.town == town && r.policy == policy).collect();
    sel.iter().map(|r| r.eur + w_caught * r.out.caught as f32 + w_home * r.out.homes_lost as f32).sum::<f32>() / sel.len() as f32
}

/// Spec 5.3: how likely a wind shift should be, per town, for the forecast to be
/// a decision. Reports threatened share, mean loss and regret of each fixed policy.
#[test]
#[ignore = "report"]
fn climate_sweep() {
    use demo::cost::Weights;
    use demo::sweep::regret_by;
    use demo::Variant;
    let policies = Policy::fixed_set();
    let w = Weights::ANALYSIS;
    let climates: [(&str, [(f32, f32); 4]); 3] = [
        ("demo_borgo", [(0.15, 0.85), (0.10, 0.60), (0.05, 0.45), (0.30, 0.70)]),
        ("demo_porto", [(0.15, 0.85), (0.10, 0.60), (0.05, 0.45), (0.30, 0.70)]),
        ("demo_valle", [(0.60, 0.95), (0.30, 0.80), (0.10, 0.70), (0.05, 0.50)]),
    ];
    for (town, cs) in climates {
        for c in cs {
            let v = Variant { shift_p: Some(c), ..Default::default() };
            let rs = run_grid(&data_dir(), &[town], &policies, &[v], 1..=24);
            let threatened = rs.iter().filter(|r| r.policy == 0 && r.out.caught >= 3).count() as f32 / 24.0;
            print!("{town} shift_p {:.2}-{:.2}: threatened {:>3.0}% |", c.0, c.1, threatened * 100.0);
            for (pi, p) in policies.iter().enumerate() {
                let loss = |r: &Record| w.loss(r.eur, r.out.caught, r.out.homes_lost) / 1000.0;
                let mean = rs.iter().filter(|r| r.policy == pi).map(&loss).sum::<f32>() / 24.0;
                let (rg, wins) = regret_by(&rs, town, pi, 0, loss);
                print!(" {}: {:.0}k (regret {:.0}, best {:.0}%)", p.name.replace("evac ", "e"), mean, rg, wins * 100.0);
            }
            println!();
        }
    }
}

#[test]
#[ignore = "report"]
fn forecast_decision() {
    let policies = Policy::fixed_set();
    let rs = run_grid(&data_dir(), &ALL, &policies, &[Default::default()], 1..=24);
    for town in ALL {
        println!("\n== {town}  (24 drawn seeds)");
        println!("{:<18} {:>8} {:>8} {:>10}", "policy", "caught", "homes", "spent EUR");
        for (pi, p) in policies.iter().enumerate() {
            let sel: Vec<_> = rs.iter().filter(|r| r.town == town && r.policy == pi).collect();
            let n = sel.len() as f32;
            println!(
                "{:<18} {:>8.1} {:>8.1} {:>10.0}",
                p.name,
                sel.iter().map(|r| r.out.caught as f32).sum::<f32>() / n,
                sel.iter().map(|r| r.out.homes_lost as f32).sum::<f32>() / n,
                sel.iter().map(|r| r.eur).sum::<f32>() / n
            );
        }
        println!("  mean total loss (EUR k) = spent + w x caught + 20k x homes, by w (EUR k per family caught):");
        print!("  {:<16}", "w");
        for w in [5.0f32, 10.0, 20.0, 40.0, 80.0] {
            print!(" {:>8}", w);
        }
        println!();
        for (pi, p) in policies.iter().enumerate() {
            print!("  {:<16}", p.name);
            for w in [5.0f32, 10.0, 20.0, 40.0, 80.0] {
                print!(" {:>8.0}", mean_loss(&rs, town, pi, w * 1000.0, 20_000.0) / 1000.0);
            }
            println!();
        }
    }
}
