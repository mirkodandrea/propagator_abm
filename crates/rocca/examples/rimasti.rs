//! Who stays at home after an evacuation order, and in what state.
use rocca::{Civil, Game, Plan};
use std::collections::BTreeMap;

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    for case in ["Coste2_gira", "Borgo2"] {
        let mut g = Game::new(data, case, 1)?;
        let n = g.districts.len();
        let borgo = g.district_index("Borgo").unwrap();
        g.commit(Plan::new(n).with_priorities(&[borgo]).with_civil(borgo, Civil::Evacua))?;
        g.run_until(90 * 60)?;
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        let mut cars = [0usize; 2];
        for &i in &g.districts[borgo].households {
            let h = &g.agents.households[i];
            *by.entry(format!("{:?}", h.status)).or_default() += 1;
            if !matches!(h.status, scenario::population::Status::Evacuated) {
                cars[(h.vehicles > 0) as usize] += 1;
            }
        }
        println!("{case}: {by:?}  non evacuate senza auto/con auto: {cars:?}");
    }
    Ok(())
}
