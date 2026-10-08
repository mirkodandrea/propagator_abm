//! Civil orders over time: no order, pre-alert, or evacuation at T+0 for one
//! district, same fire and seed. Households gone (on the road or out),
//! preparing to leave, and ready at home after a pre-alert.
//!
//!   cargo run --release -p rocca --example civili -- Borgo1 Borgo
use rocca::{Civil, Game, Plan};
use scenario::population::Status;
fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    for civil in [Civil::Nessuno, Civil::Preallerta, Civil::Evacua] {
        let mut g = Game::new("data".as_ref(), &a[1], 1)?;
        let d = g.district_index(&a[2]).unwrap();
        g.commit(Plan::new(g.districts.len()).with_civil(d, civil))?;
        let mut s = format!("{:<10}", format!("{civil:?}"));
        for m in [5, 10, 15, 20, 30, 45, 60, 90] {
            g.run_until(m * 60)?;
            let ids = &g.districts[d].households;
            let left = ids.iter().filter(|&&i| matches!(g.agents.households[i].status, Status::Evacuating | Status::Evacuated)).count();
            let prep = ids.iter().filter(|&&i| g.agents.households[i].status == Status::Preparing).count();
            let ready = ids.iter().filter(|&&i| g.agents.households[i].readied_s > 0.0).count();
            s += &format!(" | {m:>2}' via {left:>2} prep {prep:>2} pronti {ready:>2}");
        }
        let o = g.outcome().districts[d];
        println!("{s} | colti {}", o.caught);
    }
    Ok(())
}
