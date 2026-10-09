//! How long a kiosk game lasts in real time, with the pacing rule of
//! `crates/game/src/kiosk` applied to a plain plan: ×base, ×base·3 while the
//! game is quiet (`Game::is_quiet`, 10 min), crises at ×1 for 25 s. Also
//! counts the lines the characters would have to drop: their queue holds 2
//! and each stays at least 2.5 real seconds (`kiosk/characters.rs`).
use rocca::{Civil, Game, Plan, STEP_S};

/// Speeds compared: before and after iteration 5b.
const PACES: [(f64, f64); 2] = [(20.0, 60.0), (40.0, 120.0)];
const CRISIS_S: f64 = 25.0;
const QUEUE: usize = 2;
const MIN_S: f64 = 2.5;

/// Log entries a character reads out (`characters::from_log`).
fn spoken(text: &str) -> bool {
    ["il vento gira: ", "evacuazione: ", "preallerta: "].iter().any(|p| text.starts_with(p)) || text.contains(" si ritira vicino a ") || text.contains("fuori servizio")
}

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    println!("| caso | ritmo | minuti reali | di cui crisi | quota «tranquilla» | crisi a | frasi | frasi perse |");
    println!("|---|---|---|---|---|---|---|---|");
    for case in ["Coste2_gira", "Piano2", "Borgo2"] {
        for (base, quiet_speed) in PACES {
            let mut g = Game::new(data, case, 1)?;
            let n = g.districts.len();
            let near = g.district_index(&g.case.near.clone()).unwrap_or(0);
            g.commit(Plan::new(n).with_priorities(&[near]).with_civil(near, Civil::Evacua))?;
            let (mut real, mut crisis_real, mut quiet, mut steps) = (0.0f64, 0.0f64, 0usize, 0usize);
            let mut crises = vec![];
            // the speech queue, in real seconds
            let (mut seen, mut lines, mut dropped) = (g.log.len(), 0usize, 0usize);
            let mut queue: Vec<f64> = vec![];
            let mut shown_at = f64::NEG_INFINITY;
            while g.time_s() < g.case.duration_s() {
                g.step()?;
                steps += 1;
                if g.take_crisis().is_some() {
                    crises.push(format!("T+{}:{:02}", g.time_s() / 3600, g.time_s() % 3600 / 60));
                    real += CRISIS_S;
                    crisis_real += CRISIS_S;
                }
                let speed = if g.is_quiet(10 * 60) {
                    quiet += 1;
                    quiet_speed
                } else {
                    base
                };
                real += STEP_S as f64 / speed;
                for e in &g.log[seen..] {
                    if spoken(&e.text) {
                        lines += 1;
                        if queue.len() >= QUEUE {
                            queue.remove(0);
                            dropped += 1;
                        }
                        queue.push(real);
                    }
                }
                seen = g.log.len();
                while !queue.is_empty() && real - shown_at >= MIN_S {
                    queue.remove(0);
                    shown_at = real;
                }
            }
            println!(
                "| {case} | ×{base:.0} / ×{quiet_speed:.0} | {:.1} | {:.1} | {:.0} % | {} | {lines} | {dropped} |",
                real / 60.0,
                crisis_real / 60.0,
                100.0 * quiet as f64 / steps as f64,
                if crises.is_empty() { "—".into() } else { crises.join(", ") }
            );
        }
    }
    Ok(())
}
