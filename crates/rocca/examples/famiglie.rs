//! Why does a civil order end as it does in one district? The same fire under
//! two plans: «preallerta» to every district at T+0, and the same plus «evacua»
//! as soon as the mayor authorises each one. For the district asked, per
//! household: intent, channel, preparation, when the word arrived (pre-alert
//! or order), when it left home and when the fire caught it at home.
//!
//!   cargo run --release -p rocca --example famiglie -- <caso> <seme> <quartiere> [priorità,...]
use rocca::{Civil, Game, Plan};
use scenario::population::Status;

#[derive(Default, Clone)]
struct Row {
    told: Option<i64>,
    /// When it started packing to go (`Status::Preparing`), and how much it
    /// had readied at home by then.
    packing: Option<(i64, f32)>,
    left: Option<i64>,
    caught: Option<i64>,
    status_caught: Option<Status>,
}

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (case, seed, name) = (a[0].as_str(), a[1].parse::<u64>()?, a[2].as_str());
    let data = std::path::Path::new("data");
    let mut runs = vec![];
    for evacuate in [false, true] {
        let mut g = Game::new(data, case, seed)?;
        let n = g.districts.len();
        let d = g.district_index(name).expect("quartiere");
        let prio: Vec<usize> = a.get(3).map_or(vec![], |p| p.split(',').filter_map(|x| g.district_index(x.trim())).collect());
        let mut plan = Plan::new(n).with_priorities(&prio);
        for x in 0..n {
            plan = plan.with_civil(x, if evacuate && g.may_evacuate(x) { Civil::Evacua } else { Civil::Preallerta });
        }
        g.commit(plan)?;
        let hh = g.districts[d].households.clone();
        let mut rows = vec![Row::default(); hh.len()];
        while g.time_s() < g.case.duration_s() {
            g.step()?;
            if evacuate {
                let mut p = g.active.clone();
                for x in 0..n {
                    if g.may_evacuate(x) {
                        p = p.with_civil(x, Civil::Evacua);
                    }
                }
                if p != g.active {
                    g.commit(p)?;
                }
            }
            let now = g.time_s();
            for (k, &i) in hh.iter().enumerate() {
                let h = &g.agents.households[i];
                let r = &mut rows[k];
                if r.told.is_none() && (h.prealert_received || h.warning_received) {
                    r.told = Some(now);
                }
                if r.packing.is_none() && matches!(h.status, Status::Preparing | Status::Evacuating | Status::Evacuated) {
                    r.packing = Some((now, h.readied_s / 60.0));
                }
                if r.left.is_none() && matches!(h.status, Status::Evacuating | Status::Evacuated) {
                    r.left = Some(now);
                }
                if r.caught.is_none() && g.caught_at(i).is_some() {
                    r.caught = g.caught_at(i);
                    r.status_caught = Some(h.status);
                }
            }
        }
        let doors = g.at_doors_at(d);
        runs.push((evacuate, g, hh, rows, doors));
    }
    for (evacuate, g, hh, rows, doors) in &runs {
        let what = if *evacuate { "preallerta + evacua all'autorizzazione" } else { "solo preallerta" };
        let d = g.district_index(name).unwrap();
        println!(
            "\n== {what}: {name}, ordine civile {:?}, autorizzata a {:?} min, fuoco alle case a {:?} min",
            g.active.civil[d],
            g.authorised_at(d).map(|t| t / 60),
            doors.map(|t| t / 60)
        );
        let caught = rows.iter().filter(|r| r.caught.is_some()).count();
        let left_before = |t: i64| rows.iter().filter(|r| r.left.is_some_and(|l| l <= t)).count();
        let told_by = |t: i64| rows.iter().filter(|r| r.told.is_some_and(|l| l <= t)).count();
        for m in [10, 20, 30, 45, 60, 75, 90] {
            println!("  T+{m:>3}: avvisate {:>3}, partite {:>3}", told_by(m * 60), left_before(m * 60));
        }
        println!("  colte in casa: {caught}");
        for (k, r) in rows.iter().enumerate().filter(|(_, r)| r.caught.is_some()) {
            let h = &g.agents.households[hh[k]];
            println!(
                "    famiglia {:>4}: {:?} {:?} prep {:>3.0} min, avvisata {:?}, prepara da {:?} (pronta {:.0} min), partita {:?}, colta {:?} min ({:?}), stato finale {:?}",
                hh[k],
                h.intent,
                h.channel,
                h.prep_time_min,
                r.told.map(|t| t / 60),
                r.packing.map(|p| p.0 / 60),
                r.packing.map_or(0.0, |p| p.1),
                r.left.map(|t| t / 60),
                r.caught.map(|t| t / 60),
                r.status_caught,
                h.status
            );
        }
    }
    Ok(())
}
