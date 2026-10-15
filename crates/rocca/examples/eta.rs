//! Driving time of every team, at the start of a case, to a few places of the
//! island: the hex whose road point is nearest each. For comparing road layouts
//! (`ROCCA_SCENARIO=<id>`).
//!
//!   cargo run --release -p rocca --example eta [-- CASO]

use std::path::Path;

use scenario::Pos;
use rocca::Game;

const PLACES: &[(&str, f32, f32)] = &[
    ("innesco medium", 5590.0, 4370.0),
    ("testa a 45' (medium)", 4750.0, 4400.0),
    ("testa a 75' (medium)", 4300.0, 4280.0),
    ("a sud dell'innesco", 5600.0, 4050.0),
    ("cresta x 6000", 6000.0, 3600.0),
    ("sella x 6450", 6450.0, 3450.0),
    ("Le Terrazze", 4800.0, 4880.0),
    ("Costiera x 6000", 6000.0, 2950.0),
];

fn main() -> anyhow::Result<()> {
    let case = std::env::args().nth(1).unwrap_or_else(|| "medium".into());
    let g = Game::new(Path::new("data"), &case, 1)?;
    print!("{:<22} {:>12}", "luogo", "punto strada");
    for t in &g.teams {
        print!(" {:>8}", t.name.chars().take(8).collect::<String>());
    }
    println!();
    for &(name, x, y) in PLACES {
        let Some(hex) = g.hexes.nearest(Pos { x, y }) else { continue };
        print!("{:<22} {:>5.0},{:<6.0}", name, hex.target.x, hex.target.y);
        for t in 0..g.teams.len() {
            match g.plan_move(t, hex.at) {
                Ok(p) => print!(" {:>8.1}", p.minutes),
                Err(_) => print!(" {:>8}", "—"),
            }
        }
        println!();
    }
    Ok(())
}
