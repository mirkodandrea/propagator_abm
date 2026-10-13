//! Effetto dello spegnimento sul solo fuoco: ettari bruciati nel tempo per ogni
//! combinazione di fattori `UnitEffect`, a parità di caso e seme. Nessuna
//! misura su case o civili. `0:0:0` è il controllo senza spegnimento.
//!
//!   ROCCA_SCENARIO=rocca_banco cargo run --release -p rocca --example spegnimento -- \
//!       [--semi 1,2,3] [--cfg linea:acqua:portata,...]
//!
//! `game` è `rocca::game::SUPPRESSION`; `CASI=a,b` limita i casi. Le squadre
//! hanno come priorità i quartieri che il fuoco senza ordini raggiunge. Il banco
//! (`tools/factory/banco.py`) ha terreno piatto e combustibile uniforme.
use abm::suppression::UnitEffect;
use fire::CellFire;
use rocca::{Game, Plan};

const MARKS_MIN: [i64; 3] = [60, 120, 180];

fn parse(c: &str) -> UnitEffect {
    if c == "game" {
        return rocca::game::SUPPRESSION;
    }
    let f: Vec<f32> = c.split(':').map(|v| v.parse().unwrap()).collect();
    UnitEffect { line_x: f[0], water_x: f[1], reach_x: f[2], ..UnitEffect::ONE }
}

fn burnt_ha(g: &Game) -> f64 {
    let n = g.fire.state().iter().filter(|s| !matches!(s, CellFire::Unburnt)).count();
    n as f64 * (g.scn.world.cellsize * g.scn.world.cellsize) as f64 / 1e4
}

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut seeds, mut cfgs) = (vec![1u64, 2, 3], vec!["0:0:0".to_string(), "1:1:1".to_string(), "game".to_string()]);
    let mut i = 0;
    while i + 1 < args.len() {
        match args[i].as_str() {
            "--semi" => seeds = args[i + 1].split(',').map(|s| s.parse().unwrap()).collect(),
            "--cfg" => cfgs = args[i + 1].split(',').map(String::from).collect(),
            _ => {}
        }
        i += 2;
    }
    let cases: Vec<String> = match std::env::var("CASI") {
        Ok(v) if !v.is_empty() => v.split(',').map(String::from).collect(),
        _ => rocca::case::Territory::load(data)?.all_cases().map(|c| c.name.clone()).collect(),
    };
    println!("| caso | seme | linea:acqua:portata | ha a 60' | ha a 120' | ha a 180' | acqua (kL) | linea (m) |");
    println!("|---|---|---|---|---|---|---|---|");
    let mut tot = vec![([0.0f64; 3], 0.0f64, 0.0f64, 0usize); cfgs.len()];
    for case in &cases {
        for &seed in &seeds {
            let without = Game::without_orders(data, case, seed)?;
            let hit: Vec<usize> = (0..without.districts.len()).filter(|&d| without.districts[d].homes_hit > 0 || without.districts[d].caught > 0).collect();
            for (k, cfg) in cfgs.iter().enumerate() {
                let mut g = Game::new(data, case, seed)?;
                g.mayor = None;
                g.crews.effect = parse(cfg);
                g.commit(Plan::new(g.districts.len()).with_priorities(&hit))?;
                let mut at = [0.0f64; 3];
                for (m, &min) in MARKS_MIN.iter().enumerate() {
                    let end = (min * 60).min(g.case.duration_s());
                    while g.time_s() < end {
                        g.step()?;
                    }
                    at[m] = burnt_ha(&g);
                }
                let s = g.crews.stats();
                println!("| {case} | {seed} | {cfg} | {:.0} | {:.0} | {:.0} | {:.1} | {:.0} |", at[0], at[1], at[2], s.water_l / 1000.0, s.line_m);
                let t = &mut tot[k];
                for m in 0..3 {
                    t.0[m] += at[m];
                }
                *t = (t.0, t.1 + s.water_l / 1000.0, t.2 + s.line_m as f64, t.3 + 1);
            }
        }
    }
    println!();
    let base = tot[0].0[2] / tot[0].3 as f64;
    for (k, cfg) in cfgs.iter().enumerate() {
        let t = &tot[k];
        let n = t.3 as f64;
        println!("MEDIA {cfg}: ha {:.0} / {:.0} / {:.0} (60'/120'/180'), {:+.0} % a 180' rispetto alla prima; acqua {:.1} kL, linea {:.0} m", t.0[0] / n, t.0[1] / n, t.0[2] / n, (t.0[2] / n / base - 1.0) * 100.0, t.1 / n, t.2 / n);
    }
    Ok(())
}
