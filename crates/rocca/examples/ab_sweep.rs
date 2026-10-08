//! A/B over every case of the territory: for each fire, the locality it was
//! picked for (`near`) ranked first or second against each other locality, and
//! no defence at all. Same seed throughout. Markdown table on stdout.
//!
//!   cargo run --release -p rocca --example ab_sweep

use rocca::{Game, Outcome, Plan, Territory};

fn run(case: &str, prio: &[usize]) -> anyhow::Result<(Outcome, Vec<usize>)> {
    let mut g = Game::new("out/factory/data".as_ref(), "t4_paese", case, 1)?;
    g.commit(Plan::new(g.districts.len()).with_priorities(prio))?;
    g.run_until(g.case.duration_s())?;
    let mut on = vec![0; g.districts.len()];
    for p in g.posts.iter().flatten() {
        on[p.district] += 1;
    }
    Ok((g.outcome(), on))
}

fn main() -> anyhow::Result<()> {
    let t = Territory::load("out/factory/data".as_ref(), "t4_paese")?;
    let g = Game::new("out/factory/data".as_ref(), "t4_paese", &t.cases[0].name, 1)?;
    let names: Vec<String> = g.districts.iter().map(|d| d.name.clone()).collect();
    let short = |k: usize| names[k].split(' ').next_back().unwrap().to_string();
    println!("| caso | piano | case colpite ({}) | colti in casa | mezzi a fine caso |", (0..names.len()).map(short).collect::<Vec<_>>().join(" / "));
    println!("|---|---|---|---|---|");
    for c in &t.cases {
        let near = g.district_index(&c.near).unwrap();
        let fmt = |o: &Outcome| o.districts.iter().map(|d| d.homes_hit.to_string()).collect::<Vec<_>>().join(" / ");
        let fmtc = |o: &Outcome| o.districts.iter().map(|d| d.caught.to_string()).collect::<Vec<_>>().join(" / ");
        let (none, _) = run(&c.name, &[])?;
        println!("| {} | nessuna difesa | {} | {} | – |", c.name, fmt(&none), fmtc(&none));
        for other in (0..names.len()).filter(|&k| k != near) {
            for prio in [[near, other], [other, near]] {
                let (o, on) = run(&c.name, &prio)?;
                println!("| | {} > {} | {} | {} | {} |", short(prio[0]), short(prio[1]), fmt(&o), fmtc(&o), on.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" / "));
            }
        }
    }
    Ok(())
}
