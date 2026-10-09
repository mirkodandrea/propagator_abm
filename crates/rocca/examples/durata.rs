//! How long a kiosk game lasts in real time: ×20, ×20·3 while the game is
//! quiet (`Game::is_quiet`, 10 min), crises at ×1 for 25 s. The pacing rule
//! of `crates/game/src/kiosk`, applied to a plain plan.
use rocca::{Civil, Game, Plan, STEP_S};

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    println!("| caso | minuti reali a ×20 | con l'accelerazione | quota del tempo «tranquillo» |");
    println!("|---|---|---|---|");
    for case in ["Coste2_gira", "Piano2", "Borgo2"] {
        let mut g = Game::new(data, case, 1)?;
        let n = g.districts.len();
        let near = g.district_index(&g.case.near.clone()).unwrap_or(0);
        g.commit(Plan::new(n).with_priorities(&[near]).with_civil(near, Civil::Evacua))?;
        let (mut plain, mut paced, mut quiet) = (0.0f64, 0.0f64, 0usize);
        let mut steps = 0usize;
        while g.time_s() < g.case.duration_s() {
            g.step()?;
            steps += 1;
            if g.take_crisis().is_some() {
                plain += 25.0;
                paced += 25.0;
            }
            let dt = STEP_S as f64;
            plain += dt / 20.0;
            if g.is_quiet(10 * 60) {
                quiet += 1;
                paced += dt / 60.0;
            } else {
                paced += dt / 20.0;
            }
        }
        println!("| {case} | {:.1} | {:.1} | {:.0} % |", plain / 60.0, paced / 60.0, 100.0 * quiet as f64 / steps as f64);
    }
    Ok(())
}
