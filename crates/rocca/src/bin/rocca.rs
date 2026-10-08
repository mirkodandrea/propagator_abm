//! Headless runner and A/B comparator.
//!
//!   rocca <caso> [--seme N] [--minuti M] [--priorita Borgo,Coste]
//!                [--civili "0:Coste=preallerta,30:Coste=evacua"]
//!                [--b-priorita ...] [--b-civili ...] [--dati out/factory/data]
//!                [--scenario t4_paese] [--registro]
//!
//! Plan A, and plan B if any `--b-*` is given (B = A plus what they change),
//! on the same fire, seed and territory. Civil orders are given at their minute; priorities from T+0.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use rocca::{Civil, Game, Outcome, Plan};

struct Script {
    priorities: Vec<String>,
    civil: Vec<(i64, String, Civil)>,
}

fn parse_civil(s: &str) -> Result<Vec<(i64, String, Civil)>> {
    let mut out = vec![];
    for item in s.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        let (t, rest) = item.split_once(':').context("ordine civile: atteso minuto:quartiere=ordine")?;
        let (d, o) = rest.split_once('=').context("ordine civile: atteso quartiere=ordine")?;
        let c = match o.trim() {
            "preallerta" | "p" => Civil::Preallerta,
            "evacua" | "e" => Civil::Evacua,
            x => bail!("ordine civile sconosciuto: {x}"),
        };
        out.push((t.trim().parse::<i64>()? * 60, d.trim().to_string(), c));
    }
    out.sort_by_key(|x| x.0);
    Ok(out)
}

fn play(data: &std::path::Path, scenario: &str, case: &str, seed: u64, minutes: i64, s: &Script) -> Result<(Game, Outcome)> {
    let mut g = Game::new(data, scenario, case, seed)?;
    let idx = |g: &Game, n: &str| g.district_index(n).with_context(|| format!("quartiere {n:?} sconosciuto"));
    let prio: Vec<usize> = s.priorities.iter().map(|n| idx(&g, n)).collect::<Result<_>>()?;
    let mut plan = Plan::new(g.districts.len()).with_priorities(&prio);
    let mut next = 0;
    while next < s.civil.len() && s.civil[next].0 == 0 {
        plan = plan.with_civil(idx(&g, &s.civil[next].1)?, s.civil[next].2);
        next += 1;
    }
    g.commit(plan.clone())?;
    let end = minutes * 60;
    while g.time_s() < end {
        if next < s.civil.len() && s.civil[next].0 <= g.time_s() {
            plan = plan.with_civil(idx(&g, &s.civil[next].1)?, s.civil[next].2);
            next += 1;
            g.commit(plan.clone())?;
            continue;
        }
        g.step()?;
    }
    let o = g.outcome();
    Ok((g, o))
}

fn table(g: &Game, a: &Outcome, b: Option<&Outcome>) {
    println!("{:<12} {:>5} | {:>13} {:>13} {:>13} {:>13} {:>9}", "quartiere", "fam.", "case colpite", "colti in casa", "evacuati", "in strada", "a casa");
    let cell = |x: usize, y: Option<usize>| match y {
        Some(y) if y != x => format!("{x} → {y}"),
        _ => format!("{x}"),
    };
    for (k, d) in g.districts.iter().enumerate() {
        let (x, y) = (&a.districts[k], b.map(|b| &b.districts[k]));
        println!(
            "{:<12} {:>5} | {:>13} {:>13} {:>13} {:>13} {:>9}",
            d.name,
            x.households,
            cell(x.homes_hit, y.map(|y| y.homes_hit)),
            cell(x.caught, y.map(|y| y.caught)),
            cell(x.evacuated, y.map(|y| y.evacuated)),
            cell(x.on_the_road, y.map(|y| y.on_the_road)),
            cell(x.at_home, y.map(|y| y.at_home)),
        );
    }
    println!("ettari bruciati: {:.0}{}   mezzi persi: {}", a.hectares, b.map_or(String::new(), |b| format!(" → {:.0}", b.hectares)), a.units_lost);
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut case = None;
    let (mut seed, mut minutes) = (1u64, 180i64);
    let (mut data, mut scenario) = (PathBuf::from("out/factory/data"), "t4_paese".to_string());
    let mut a = Script { priorities: vec![], civil: vec![] };
    let mut b = Script { priorities: vec![], civil: vec![] };
    let (mut has_b, mut registro) = (false, false);
    let (mut b_prio, mut b_civil) = (false, false);
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut val = || it.next().cloned().with_context(|| format!("{x}: valore mancante"));
        match x.as_str() {
            "--seme" => seed = val()?.parse()?,
            "--minuti" => minutes = val()?.parse()?,
            "--dati" => data = val()?.into(),
            "--scenario" => scenario = val()?,
            "--priorita" => a.priorities = val()?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            "--civili" => a.civil = parse_civil(&val()?)?,
            "--b-priorita" => {
                has_b = true;
                b_prio = true;
                b.priorities = val()?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
            }
            "--b-civili" => {
                has_b = true;
                b_civil = true;
                b.civil = parse_civil(&val()?)?
            }
            "--registro" => registro = true,
            c if !c.starts_with("--") => case = Some(c.to_string()),
            c => bail!("opzione sconosciuta {c}"),
        }
    }
    // B is A with only what the --b-* options change
    if !b_prio {
        b.priorities = a.priorities.clone();
    }
    if !b_civil {
        b.civil = a.civil.clone();
    }
    let case = case.context("uso: rocca <caso> [opzioni]; vedi l'intestazione di src/bin/rocca.rs")?;
    let t = std::time::Instant::now();
    let (ga, oa) = play(&data, &scenario, &case, seed, minutes, &a)?;
    let ob = if has_b { Some(play(&data, &scenario, &case, seed, minutes, &b)?.1) } else { None };
    println!("caso {case}, seme {seed}, {minutes} min simulati ({:.1} s)", t.elapsed().as_secs_f32());
    if registro {
        for e in &ga.log {
            println!("  T+{:>3} min  {}", e.at_s / 60, e.text);
        }
    }
    table(&ga, &oa, ob.as_ref());
    Ok(())
}
