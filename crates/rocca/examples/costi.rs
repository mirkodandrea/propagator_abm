//! What an evacuation costs: engines slowed by the traffic, families caught on
//! the road. Same fire and seed, the evacuation of everything ordered at
//! different times, with the right priority (Pian dei Grilli first).
use rocca::{Civil, Game, Plan};
use scenario::population::Status;

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let case = std::env::args().nth(1).unwrap_or_else(|| "Coste2_gira".into());
    println!("| evacua tutto a | case colpite | colte in casa | in strada alla fine | bloccate/vittime fuori casa | arrivo 1° mezzo a Pian dei Grilli |");
    println!("|---|---|---|---|---|---|");
    for at_min in [None, Some(0), Some(20), Some(40), Some(60), Some(90)] {
        let mut g = Game::new(data, &case, 1)?;
        let n = g.districts.len();
        let piano = g.district_index("Grilli").unwrap();
        let mut plan = Plan::new(n).with_priorities(&[piano]);
        g.commit(plan.clone())?;
        let mut first_on_post: Option<i64> = None;
        let end = g.case.duration_s();
        while g.time_s() < end {
            if Some(g.time_s() / 60) == at_min && g.active.civil.iter().all(|c| *c == Civil::Nessuno) {
                for d in 0..n {
                    plan = plan.with_civil(d, Civil::Evacua);
                }
                g.commit(plan.clone())?;
            }
            g.step()?;
            if first_on_post.is_none() && g.posts.iter().flatten().any(|p| p.district == piano && rocca::district::dist(g.crews.units[p.unit].pos, p.at) < 150.0) {
                first_on_post = Some(g.time_s());
            }
        }
        let o = g.outcome();
        let road = g.agents.households.iter().filter(|h| h.status == Status::Evacuating).count();
        let away_bad = (0..g.agents.households.len())
            .filter(|&i| matches!(g.agents.households[i].status, Status::Trapped | Status::Casualty) && g.caught_at(i).is_none())
            .count();
        println!(
            "| {} | {} | {} | {} | {} | {} |",
            at_min.map_or("mai".into(), |m| format!("T+{m}")),
            o.homes_hit(),
            o.caught(),
            road,
            away_bad,
            first_on_post.map_or("–".into(), |t| format!("T+{}", t / 60))
        );
    }
    Ok(())
}
