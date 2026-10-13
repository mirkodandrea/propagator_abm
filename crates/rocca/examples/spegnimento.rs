//! Quanto spegne e quanto difende il fattore di spegnimento? Stessa partita
//! (caso, seme, priorità sui quartieri colpiti dal fuoco senza ordini, nessun
//! ordine ai civili) giocata con `UnitEffect::all(x)` per ogni x: case in
//! fiamme, danno medio delle case, celle bruciate, acqua e linea.
//!
//!   cargo run --release -p rocca --example spegnimento [--semi 1,2,3] [--x 1,3] [--reach 1]
//! `--reach` moltiplica la portata dei getti (`reach_x`) a parità di `x`.
use abm::suppression::UnitEffect;
use fire::CellFire;
use rocca::{Game, Plan};

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut seeds, mut xs, mut reach) = (vec![1u64, 2, 3], vec![1.0f32, 3.0], 1.0f32);
    let mut i = 0;
    while i + 1 < args.len() {
        match args[i].as_str() {
            "--semi" => seeds = args[i + 1].split(',').map(|s| s.parse().unwrap()).collect(),
            "--reach" => reach = args[i + 1].parse().unwrap(),
            "--x" => xs = args[i + 1].split(',').map(|s| s.parse().unwrap()).collect(),
            _ => {}
        }
        i += 2;
    }
    let cases: Vec<String> = Game::new(data, "Rocco3", 1)?.territory.all_cases().map(|c| c.name.clone()).collect();
    println!("| caso | seme | x | case in fiamme | danno medio case | celle bruciate | acqua (kL) | linea (m) | lanci |");
    println!("|---|---|---|---|---|---|---|---|---|");
    let mut tot = vec![(0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0usize); xs.len()];
    for case in &cases {
        for &seed in &seeds {
            let without = Game::without_orders(data, case, seed)?;
            let hit: Vec<usize> = (0..without.districts.len()).filter(|&d| without.districts[d].homes_hit > 0 || without.districts[d].caught > 0).collect();
            for (k, &x) in xs.iter().enumerate() {
                let mut g = Game::new(data, case, seed)?;
                g.mayor = None;
                g.crews.effect = UnitEffect { reach_x: reach, ..UnitEffect::all(x) };
                let order = hit.clone();
                g.commit(Plan::new(g.districts.len()).with_priorities(&order))?;
                while g.time_s() < g.case.duration_s() {
                    g.step()?;
                }
                let ex = g.fire.exposure().fields();
                let alight = ex.iter().filter(|e| e.alight).count();
                let dmg = ex.iter().map(|e| e.damage as f64).sum::<f64>() / ex.len().max(1) as f64;
                let burnt = g.fire.state().iter().filter(|s| !matches!(s, CellFire::Unburnt)).count();
                let s = g.crews.stats();
                println!("| {case} | {seed} | {x} | {alight} | {dmg:.3} | {burnt} | {:.1} | {:.0} | {} |", s.water_l / 1000.0, s.line_m, s.drops);
                let t = &mut tot[k];
                *t = (t.0 + alight as f64, t.1 + dmg, t.2 + burnt as f64, t.3 + s.water_l / 1000.0, t.4 + s.line_m as f64, t.5 + 1);
            }
        }
    }
    println!();
    for (k, x) in xs.iter().enumerate() {
        let t = tot[k];
        let n = t.5 as f64;
        println!("MEDIA x{x}: case in fiamme {:.1}, danno {:.3}, celle bruciate {:.0}, acqua {:.1} kL, linea {:.0} m", t.0 / n, t.1 / n, t.2 / n, t.3 / n, t.4 / n);
    }
    Ok(())
}
