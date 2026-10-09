//! The debrief's lines per district (`Game::story`) for a few plans on the
//! kiosk cases: `cargo run --release -p rocca --example debrief`.
use rocca::{Civil, Game, Plan};

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    for case in ["Coste2_gira", "Piano2", "Borgo2"] {
        for (label, civil) in [("nessun ordine", None), ("preallerta", Some(Civil::Preallerta)), ("evacua", Some(Civil::Evacua))] {
            let mut g = Game::new(data, case, 1)?;
            let near = g.district_index(&g.case.near.clone()).unwrap_or(0);
            let mut plan = Plan::new(g.districts.len()).with_priorities(&[near]);
            if let Some(c) = civil {
                plan = plan.with_civil(near, c);
            }
            g.commit(plan)?;
            g.run_until(g.case.duration_s())?;
            println!("## {case}, prima priorità {} ({label})", g.districts[near].name);
            for d in 0..g.districts.len() {
                println!("- {}: {}", g.districts[d].name, g.story(d).join(" "));
            }
        }
    }
    Ok(())
}
