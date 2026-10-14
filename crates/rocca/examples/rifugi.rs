//! Where each district's evacuation goes at the start of a case.
//!
//!   cargo run --release -p rocca --example rifugi [-- CASO...]

use std::path::Path;

use rocca::Game;

fn main() -> anyhow::Result<()> {
    let data = Path::new("data");
    let cases: Vec<String> = std::env::args().skip(1).collect();
    let cases = if cases.is_empty() { vec!["medium".to_string(), "tutorial".to_string()] } else { cases };
    for c in cases {
        let g = Game::new(data, &c, 1)?;
        println!("== {c}");
        println!("{:<16} {:<34} {:>13} {:>7} {:>6} {:>6}", "borgo", "destinazione", "pos", "len m", "guida", "totale");
        for (d, dd) in g.districts.iter().enumerate() {
            match g.plan_evacuation(d) {
                Ok(p) => {
                    let end = *p.path.points.last().unwrap();
                    println!("{:<16} {:<34} {:>6.0},{:<6.0} {:>7.0} {:>6.1} {:>6.1}", dd.name, p.path.to, end.x, end.y, p.path.length_m(), p.drive_min, p.total_min());
                }
                Err(e) => println!("{:<16} {e}", dd.name),
            }
        }
    }
    Ok(())
}
