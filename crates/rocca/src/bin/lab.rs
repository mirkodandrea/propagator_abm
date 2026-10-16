//! Laboratorio di spegnimento: analisi headless degli algoritmi delle squadre.
//!
//!   lab [--out DIR] [--dati data] [--scenari a,b|tutti] [--lista]
//!       [--strategie "head;lead:lead=400;planner"] [--squadre 1,2,4]
//!       [--semi 1-3] [--tune chiave=valore,...] [--sweep chiave=v1,v2,...]
//!       [--mira avanti|vicino|entrambe] [--senza-una] [--serie]
//!       [--thread N]
//!
//! Strategie (separate da «;»): none, head, lead, flanks, village, split,
//! planner; i parametri dopo «:» (es. `lead:lead=600,spread=300,rev=20`).
//! Ogni corsa è confrontata con lo stesso fuoco, stesso seme, senza ordini.
//! Scrive in DIR: runs.csv (una riga per corsa, tutte le misure), report.md,
//! e con --serie series.csv e units.csv (serie temporali del fuoco e delle unità).

use std::path::PathBuf;

use abm::suppression::EngineAim;
use anyhow::{bail, Context, Result};
use rocca::game::Tuning;
use rocca::lab::run::{run_all, set_dial, Job};
use rocca::lab::{report, synth};

fn seeds(s: &str) -> Result<Vec<u64>> {
    let mut out = vec![];
    for part in s.split(',') {
        if let Some((a, b)) = part.split_once('-') {
            out.extend(a.parse::<u64>()?..=b.parse::<u64>()?);
        } else {
            out.push(part.parse()?);
        }
    }
    Ok(out)
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut out, mut data) = (PathBuf::from("out/suppress-lab"), PathBuf::from("data"));
    let (mut scen, mut strats, mut teams, mut seed_s) = ("tutti".to_string(), "head;lead;flanks;village;split;planner".to_string(), "1,2,4".to_string(), "1-3".to_string());
    let mut tune = Tuning::default();
    let mut sweeps: Vec<(String, Vec<f64>)> = vec![];
    let (mut aim, mut loo, mut series, mut list) = ("avanti".to_string(), false, false, false);
    let mut threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    while let Some(a) = args.next() {
        let mut v = || args.next().with_context(|| format!("{a}: manca il valore"));
        match a.as_str() {
            "--out" => out = PathBuf::from(v()?),
            "--dati" => data = PathBuf::from(v()?),
            "--scenari" => scen = v()?,
            "--strategie" => strats = v()?,
            "--squadre" => teams = v()?,
            "--semi" => seed_s = v()?,
            "--tune" => {
                for kv in v()?.split(',') {
                    let (k, x) = kv.split_once('=').context("--tune: chiave=valore")?;
                    set_dial(&mut tune, k, x.parse()?)?;
                }
            }
            "--sweep" => {
                let s = v()?;
                let (k, xs) = s.split_once('=').context("--sweep: chiave=v1,v2")?;
                sweeps.push((k.to_string(), xs.split(',').map(str::parse).collect::<Result<_, _>>()?));
            }
            "--mira" => aim = v()?,
            "--senza-una" => loo = true,
            "--serie" => series = true,
            "--lista" => list = true,
            "--thread" => threads = v()?.parse()?,
            x => bail!("argomento sconosciuto: {x}"),
        }
    }
    if list {
        for s in synth::catalogue() {
            println!("{:<14} {}", s.name, s.about);
        }
        return Ok(());
    }
    let specs: Vec<synth::SynthSpec> = if scen == "tutti" {
        synth::catalogue()
    } else {
        scen.split(',').map(|n| synth::spec(n).with_context(|| format!("scenario sconosciuto: {n} (--lista)"))).collect::<Result<_>>()?
    };
    std::env::set_var("ROCCA_SCENARIO", synth::SCENARIO);
    let mut dirs = vec![];
    for s in &specs {
        let b = synth::build(s, &out, &data)?;
        eprintln!("{}: {} case, mondo {:.0} m", s.name, b.homes, s.world_m());
        dirs.push(b.data_dir);
    }
    let strategies: Vec<String> = strats.split(';').map(str::trim).filter(|x| !x.is_empty()).map(String::from).collect();
    let teams: Vec<usize> = teams.split(',').map(str::parse).collect::<Result<_, _>>()?;
    let seeds = seeds(&seed_s)?;
    let aims: Vec<EngineAim> = match aim.as_str() {
        "avanti" => vec![EngineAim::Ahead],
        "vicino" => vec![EngineAim::Hottest],
        "entrambe" => vec![EngineAim::Ahead, EngineAim::Hottest],
        x => bail!("--mira: avanti|vicino|entrambe, non {x}"),
    };
    // the tunings: the base one, and one dial at a time
    let mut tunings: Vec<(Tuning, String)> = vec![(tune, String::new())];
    for (k, xs) in &sweeps {
        for &x in xs {
            let mut t = tune;
            set_dial(&mut t, k, x)?;
            if t != tune {
                tunings.push((t, format!("{k}={x}")));
            }
        }
    }
    let mut jobs = vec![];
    for (spec, dir) in specs.iter().zip(&dirs) {
        for strat in &strategies {
            for &n in &teams {
                for &seed in &seeds {
                    for (t, tag) in &tunings {
                        for &aim in &aims {
                            let job = Job { scenario: spec.name.clone(), data_dir: dir.clone(), strategy: strat.clone(), teams: n, seed, tuning: *t, tune_tag: tag.clone(), aim, without: None };
                            if loo && n >= 2 && tag.is_empty() {
                                for k in 0..n {
                                    jobs.push(Job { without: Some(k), ..job.clone() });
                                }
                            }
                            jobs.push(job);
                        }
                    }
                }
            }
        }
    }
    eprintln!("{} corse su {} thread", jobs.len(), threads);
    let t0 = std::time::Instant::now();
    let runs = run_all(jobs, threads, series, true);
    eprintln!("fatto in {:.0} s", t0.elapsed().as_secs_f32());
    report::write_csv(&out, &runs)?;
    let md = report::write_markdown(&out, &runs)?;
    println!("{md}");
    eprintln!("scritti {}/runs.csv e report.md", out.display());
    Ok(())
}
