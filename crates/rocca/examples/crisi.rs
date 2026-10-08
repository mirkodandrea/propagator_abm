//! Does a decision taken at the crisis matter? For every case, the same
//! opening plan (the locality the fire was picked for first, then the
//! others), then three ways of meeting the crises the game raises:
//! ignore them, answer at once (put the threatened place first), or answer
//! 20 minutes late. Same seed throughout. Markdown table on stdout.
//!
//!   cargo run --release -p rocca --example crisi

use rocca::crisis::Kind;
use rocca::{Game, Outcome, Plan, Territory};

#[derive(Clone, Copy, PartialEq)]
enum Strategy {
    Fermo,
    Pronto,
    Tardi,
    /// Put the place first and order its evacuation, at once.
    Evacua,
}

fn run(case: &str, s: Strategy) -> anyhow::Result<(Outcome, Vec<String>)> {
    let mut g = Game::new("data".as_ref(), case, 1)?;
    let near = g.district_index(&g.case.near.clone()).unwrap();
    let mut prio = vec![near];
    prio.extend((0..g.districts.len()).filter(|&d| d != near));
    let mut plan = Plan::new(g.districts.len()).with_priorities(&prio);
    g.commit(plan.clone())?;
    let mut due: Vec<(i64, usize)> = vec![];
    let mut seen = vec![];
    while g.time_s() < g.case.duration_s() {
        g.step()?;
        if let Some(c) = g.take_crisis() {
            seen.push(format!("T+{} {:?}", c.at_s / 60, c.kind));
            if let Kind::Scoperto { district } | Kind::Previsione { district } | Kind::Vento { district } = c.kind {
                let delay = match s {
                    Strategy::Fermo => continue,
                    Strategy::Pronto | Strategy::Evacua => 0,
                    Strategy::Tardi => 20 * 60,
                };
                due.push((g.time_s() + delay, district));
            }
        }
        while let Some(i) = due.iter().position(|(t, _)| *t <= g.time_s()) {
            let (_, d) = due.remove(i);
            plan.priorities.retain(|&x| x != d);
            plan.priorities.insert(0, d);
            if s == Strategy::Evacua {
                plan.civil[d] = rocca::Civil::Evacua;
            }
            g.commit(plan.clone())?;
        }
    }
    Ok((g.outcome(), seen))
}

fn main() -> anyhow::Result<()> {
    let t = Territory::load("data".as_ref())?;
    println!("| caso | crisi sollevate | case colpite: ignora / subito / 20' dopo / subito + evacua | colti in casa: idem |");
    println!("|---|---|---|---|");
    for c in &t.cases {
        let (a, seen) = run(&c.name, Strategy::Fermo)?;
        let (b, _) = run(&c.name, Strategy::Pronto)?;
        let (l, _) = run(&c.name, Strategy::Tardi)?;
        let (e, _) = run(&c.name, Strategy::Evacua)?;
        println!(
            "| {} | {} | {} / {} / {} / {} | {} / {} / {} / {} |",
            c.name,
            if seen.is_empty() { "–".into() } else { seen.join("; ") },
            a.homes_hit(), b.homes_hit(), l.homes_hit(), e.homes_hit(),
            a.caught(), b.caught(), l.caught(), e.caught()
        );
    }
    Ok(())
}
