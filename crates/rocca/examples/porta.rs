//! How close does the fire come to the doors? For every case, no plan: the
//! peak threat to a person at each home (`fire::ThreatField`, the field the
//! behaviour graph's `obs.threat` reads) and how many homes ever pass a few
//! thresholds. Markdown table on stdout.
//!
//!   cargo run --release -p rocca --example porta

use rocca::{Game, Territory};

fn main() -> anyhow::Result<()> {
    let t = Territory::load("data".as_ref())?;
    println!("| caso | case con minaccia > 0 | ≥ 0,12 (allarme) | ≥ 0,35 | ≥ 0,55 (fuoco alla porta) | massimo |");
    println!("|---|---|---|---|---|---|");
    for c in &t.cases {
        let mut g = Game::new("data".as_ref(), &c.name, 1)?;
        let mut peak = vec![0.0f32; g.agents.households.len()];
        while g.time_s() < g.case.duration_s() {
            g.step()?;
            for (i, h) in g.agents.households.iter().enumerate() {
                peak[i] = peak[i].max(g.fire.threat().at(h.home));
            }
        }
        let n = |x: f32| peak.iter().filter(|&&p| p >= x).count();
        println!("| {} | {} | {} | {} | {} | {:.2} |", c.name, peak.iter().filter(|&&p| p > 0.0).count(), n(0.12), n(0.35), n(0.55),
            peak.iter().cloned().fold(0.0, f32::max));
    }
    Ok(())
}
