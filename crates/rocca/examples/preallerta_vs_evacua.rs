//! Playtest 3 (q3): on Coste2_gira, «evacuate everyone at once» left 3
//! families caught at home, «pre-alert everyone, evacuate at T+1:20» left 1.
//! Mechanism or seed? The same plans over several seeds, caught per district.
use rocca::{Civil, Game, Plan};

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    println!("| seme | piano | colte in casa (Castelvento / Pian / Ghiande) | evacuate | case |");
    println!("|---|---|---|---|---|");
    for seed in 1..=6 {
        for (label, first, late) in [("evacua tutti a T+0", Civil::Evacua, None), ("preallerta tutti, evacua a T+1:20", Civil::Preallerta, Some(80 * 60)), ("preallerta tutti, evacua a T+0:30", Civil::Preallerta, Some(30 * 60))] {
            let mut g = Game::new(data, "Coste2_gira", seed)?;
            let n = g.districts.len();
            let ghiande = g.district_index("Ghiande").unwrap();
            let mut plan = Plan::new(n).with_priorities(&[ghiande]);
            for d in 0..n {
                plan = plan.with_civil(d, first);
            }
            g.commit(plan.clone())?;
            if let Some(t) = late {
                g.run_until(t)?;
                let mut p = g.active.clone();
                for d in 0..n {
                    p = p.with_civil(d, Civil::Evacua);
                }
                g.commit(p)?;
            }
            g.run_until(g.case.duration_s())?;
            let o = g.outcome();
            let caught: Vec<String> = o.districts.iter().map(|d| d.caught.to_string()).collect();
            let evac: usize = o.districts.iter().map(|d| d.evacuated).sum();
            println!("| {seed} | {label} | {} = {} | {evac} | {} |", caught.join(" / "), o.caught(), o.homes_hit());
        }
    }
    Ok(())
}
