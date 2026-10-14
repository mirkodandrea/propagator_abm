//! The evacuation's timer against what happens: per district, the time
//! announced, what the timer shows at the order, when the last family leaves
//! and reaches the refuge, and how far the timer strays from a clock (all simulated minutes from the order).
//!
//!   cargo run --release -p rocca --example timer_evac

use std::path::Path;

use rocca::evac::State;
use rocca::Game;

fn main() -> anyhow::Result<()> {
    let data = Path::new("data");
    let base = Game::new(data, "medium", 1)?;
    println!("{:<16} {:>6} {:>9} {:>10} {:>11} {:>10} {:>7}", "borgo", "pers.", "annuncio", "timer t0", "ult.partita", "ult.arriva", "scarto");
    for d in 0..base.districts.len() {
        let mut g = Game::new(data, "medium", 1)?;
        g.pause().ok();
        let Ok(p) = g.evacuate(d) else { continue };
        g.resume();
        let t0 = g.time_s();
        let timer0 = g.evac.remaining_min(d, t0).unwrap_or(0.0);
        let (mut left_at, mut safe_at, mut drift) = (None, None, 0.0f32);
        for _ in 0..2000 {
            g.step()?;
            if let Some(r) = g.evac.remaining_min(d, g.time_s()) {
                // a timer that runs true: remaining + elapsed stays at the start
                drift = drift.max((r + (g.time_s() - t0) as f32 / 60.0 - timer0).abs());
            }
            let e = g.evac.of(d).unwrap();
            if left_at.is_none() && e.ended_s.is_some() {
                left_at = Some((g.time_s() - t0) as f32 / 60.0);
            }
            let moving = g.districts[d].households.iter().any(|&i| matches!(g.evac.families[i].state, State::Queued | State::Road));
            if left_at.is_some() && !moving {
                safe_at = Some((g.time_s() - t0) as f32 / 60.0);
                break;
            }
        }
        println!("{:<16} {:>6} {:>9.0} {:>10.0} {:>11.1?} {:>10.1?} {:>7.2}", g.districts[d].name, p.people, p.total_min(), timer0, left_at, safe_at, drift);
    }
    Ok(())
}
