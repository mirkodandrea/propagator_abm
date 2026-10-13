//! The exits: which road nodes each district's cars drive through when
//! everyone is ordered out at T+0, where the districts' exits merge, and when
//! the fire makes each node impassable. Looking for a case in which the fire
//! cuts a shared exit while cars are still on it.
//!
//!   cargo run --release -p rocca --example uscite -- [--csv dir] [caso...]
//!
//! Per case it prints one summary row; with `--csv` it also writes
//! `<case>_nodes.csv` (node, x, y, cars per district, first/last pass, cut)
//! and `<case>_fire.csv` (burnt cells, x, y, minute) for the map.
use std::collections::HashMap;
use std::io::Write;

use abm::Mode;
use rocca::{Civil, Game, Plan};

const NONE: i64 = i64::MAX;

#[derive(Clone)]
struct NodeUse {
    cars: Vec<u32>,
    first: i64,
    last: i64,
}

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut csv: Option<std::path::PathBuf> = None;
    let mut cases = vec![];
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--csv" {
            csv = Some(args[i + 1].clone().into());
            i += 2;
        } else {
            cases.push(args[i].clone());
            i += 1;
        }
    }
    if cases.is_empty() {
        let probe = Game::new(data, "Rocco3", 1)?;
        cases = probe.territory.cases.iter().map(|c| c.name.clone()).collect();
    }
    if let Some(d) = &csv {
        std::fs::create_dir_all(d)?;
    }

    println!("| caso | ettari | uscita tagliata (borgo @ minuto) | nodo in comune tagliato per primo | auto passate nei 20 min prima | candidato |");
    println!("|---|---|---|---|---|---|");
    for case in &cases {
        let mut g = Game::new(data, case, 1)?;
        // the rules before 2026-10-13: any evacuation at any time (`examples/sindaca.rs` plays the mayor's)
        g.mayor = None;
        let n = g.districts.len();
        let near = g.district_index(&g.case.near.clone()).unwrap_or(0);
        let plan = (0..n).fold(Plan::new(n).with_priorities(&[near]), |p, d| p.with_civil(d, Civil::Evacua));
        g.commit(plan)?;

        let mut district_of = vec![usize::MAX; g.agents.households.len()];
        for (d, dd) in g.districts.iter().enumerate() {
            for &h in &dd.households {
                district_of[h] = d;
            }
        }
        let nodes = g.agents.network.len();
        let node_pos: Vec<scenario::Pos> = (0..nodes).map(|k| g.agents.network.pos(k as abm::network::NodeId)).collect();
        let mut usage: HashMap<usize, NodeUse> = HashMap::new();
        let mut passes: HashMap<usize, Vec<i64>> = HashMap::new();
        let mut cut = vec![NONE; nodes];
        let mut seen_at: HashMap<usize, abm::network::NodeId> = HashMap::new();
        let end = g.case.duration_s();
        while g.time_s() < end {
            g.step()?;
            let now = g.time_s();
            for (ti, t) in g.agents.travellers.iter().enumerate() {
                if t.mode != Mode::Car || t.solo || t.at_node == abm::network::NO_NODE {
                    continue;
                }
                if seen_at.get(&ti) == Some(&t.at_node) {
                    continue;
                }
                seen_at.insert(ti, t.at_node);
                let d = district_of.get(t.household).copied().unwrap_or(usize::MAX);
                if d == usize::MAX {
                    continue;
                }
                let k = t.at_node as usize;
                let u = usage.entry(k).or_insert(NodeUse { cars: vec![0; n], first: now, last: now });
                u.cars[d] += 1;
                u.last = now;
                passes.entry(k).or_default().push(now);
            }
            let threat = g.fire.threat();
            for k in usage.keys() {
                if cut[*k] == NONE && threat.at(node_pos[*k]) >= fire::threat::IMPASSABLE {
                    cut[*k] = now;
                }
            }
        }
        // Nodes never driven are of no interest; but a node first driven
        // after it was checked is still checked above from then on.
        let o = g.outcome();

        // Per district: the earliest cut on any node its cars used.
        let mut exit_cut: Vec<(usize, i64)> = (0..n)
            .filter_map(|d| usage.iter().filter(|(_, u)| u.cars[d] > 0).map(|(k, _)| cut[*k]).min().filter(|&c| c != NONE).map(|c| (d, c)))
            .collect();
        exit_cut.sort_by_key(|x| x.1);
        // Shared nodes (cars of two or more districts) that were cut.
        let mut shared: Vec<(i64, usize, Vec<usize>, usize)> = usage
            .iter()
            .filter(|(k, u)| cut[**k] != NONE && u.cars.iter().filter(|&&c| c > 0).count() >= 2)
            .map(|(k, u)| {
                let ds: Vec<usize> = (0..n).filter(|&d| u.cars[d] > 0).collect();
                let near_cut = passes[k].iter().filter(|&&t| t <= cut[*k] && t > cut[*k] - 20 * 60).count();
                (cut[*k], *k, ds, near_cut)
            })
            .collect();
        shared.sort();
        let ex = exit_cut.iter().map(|(d, c)| format!("{} @{}", g.districts[*d].name, c / 60)).collect::<Vec<_>>().join(", ");
        let (first_shared, close) = shared.first().map_or(("–".to_string(), "–".to_string()), |(c, _, ds, nc)| {
            (format!("T+{} ({})", c / 60, ds.iter().map(|d| g.districts[*d].name.as_str()).collect::<Vec<_>>().join(" + ")), nc.to_string())
        });
        let cand = shared.iter().any(|(c, _, _, nc)| *c >= 30 * 60 && *c <= 90 * 60 && *nc > 0);
        println!("| {case} | {:.0} | {} | {first_shared} | {close} | {} |", o.hectares, if ex.is_empty() { "–".into() } else { ex }, if cand { "sì" } else { "" });

        if let Some(dir) = &csv {
            let mut f = std::fs::File::create(dir.join(format!("{case}_nodes.csv")))?;
            let names: Vec<String> = g.districts.iter().map(|d| d.name.clone()).collect();
            writeln!(f, "node,x,y,{},first_min,last_min,cut_min", names.join(","))?;
            for (k, u) in &usage {
                let c: Vec<String> = u.cars.iter().map(|x| x.to_string()).collect();
                let cm = if cut[*k] == NONE { String::new() } else { format!("{:.1}", cut[*k] as f64 / 60.0) };
                writeln!(f, "{k},{:.1},{:.1},{},{:.1},{:.1},{cm}", node_pos[*k].x, node_pos[*k].y, c.join(","), u.first as f64 / 60.0, u.last as f64 / 60.0)?;
            }
            let mut f = std::fs::File::create(dir.join(format!("{case}_fire.csv")))?;
            writeln!(f, "x,y,min")?;
            let w = g.scn.world;
            for (idx, &t) in g.fire.arrival_times().iter().enumerate() {
                if t != i32::MIN {
                    let p = w.centre_of(scenario::Cell { row: idx / w.fire_cols, col: idx % w.fire_cols });
                    writeln!(f, "{:.0},{:.0},{:.1}", p.x, p.y, t as f64 / 60.0)?;
                }
            }
            let mut f = std::fs::File::create(dir.join(format!("{case}_districts.csv")))?;
            writeln!(f, "name,x,y,households,caught,homes_hit")?;
            for (d, dd) in g.districts.iter().enumerate() {
                writeln!(f, "{},{:.0},{:.0},{},{},{}", dd.name, dd.centre.x, dd.centre.y, dd.households.len(), o.districts[d].caught, o.districts[d].homes_hit)?;
            }
        }
    }
    Ok(())
}
