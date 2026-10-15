//! Where the fire goes with no orders: the minute each fire cell ignites,
//! counted from the ignition (the game begins at the case's `start_min`), as little-endian f32 (NaN where unburnt; row 0
//! north), for maps of the fronts.
//!
//!   cargo run --release -p rocca --example fronte -- CASO SEME MINUTI OUT.f32

use std::path::Path;

use rocca::Game;

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (case, seed, minutes, out) = (&a[0], a[1].parse()?, a[2].parse::<i64>()?, &a[3]);
    let mut g = Game::new(Path::new("data"), case, seed)?;
    g.run_until(minutes * 60)?;
    let bytes: Vec<u8> = g
        .fire
        .arrival_times()
        .iter()
        .flat_map(|&t| if t == i32::MIN { f32::NAN } else { t as f32 / 60.0 }.to_le_bytes())
        .collect();
    std::fs::write(out, bytes)?;
    println!("{case} seme {seed}: fuoco a {} min, celle accese {}", g.fire.time_s() / 60, g.fire.arrival_times().iter().filter(|&&t| t != i32::MIN).count());
    Ok(())
}
