//! Headless runner.
//!
//!   rocca <caso> [--seme N] [--dati data]
//!                [--squadra "min:squadra=x;y,..."] [--evacua "min:Borgo,..."]
//!   rocca tutorial [--seme N]
//!   --tune chiave=valore,...: tarature dello spegnimento (`lab --help`, `Tuning`)
//!   --mira vicino|avanti: which cells the engines wet (default avanti)
//!   --inizio M: the fire is reported M minutes after ignition (the case's
//!   `start_min` otherwise); order minutes count from then.
//!
//! Orders are given at their simulated minute: the runner pauses (if the
//! pause is ready; otherwise as soon as it is), gives every order due and
//! resumes. Teams are numbered from 0 in `game.json`'s roster order. Ends with
//! the outcome per district and the stars against the same fire with no
//! orders.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use rocca::score;
use rocca::tutorial::Script;
use rocca::{Axial, Game, Outcome};
use scenario::Pos;

#[derive(Debug, Clone)]
enum Order {
    Team(usize, Pos),
    Evacuate(String),
}

fn parse(teams: &str, evac: &str) -> Result<Vec<(i64, Order)>> {
    let mut out = vec![];
    for item in teams.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        let (t, rest) = item.split_once(':').context("--squadra: atteso min:squadra=x;y")?;
        let (k, xy) = rest.split_once('=').context("--squadra: atteso squadra=x;y")?;
        let (x, y) = xy.split_once(';').context("--squadra: atteso x;y")?;
        out.push((t.parse::<i64>()? * 60, Order::Team(k.parse()?, Pos { x: x.parse()?, y: y.parse()? })));
    }
    for item in evac.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        let (t, d) = item.split_once(':').context("--evacua: atteso min:Borgo")?;
        out.push((t.parse::<i64>()? * 60, Order::Evacuate(d.to_string())));
    }
    out.sort_by_key(|o| o.0);
    Ok(out)
}

fn play(g: &mut Game, orders: &[(i64, Order)]) -> Result<()> {
    let mut next = 0;
    while !g.is_over() {
        if next < orders.len() && g.time_s() >= orders[next].0 && !g.paused() && g.can_pause() {
            g.pause().map_err(anyhow::Error::msg)?;
            while next < orders.len() && orders[next].0 <= g.time_s() {
                let r = match &orders[next].1 {
                    Order::Team(t, p) => g.move_team(*t, g.hexes.nearest(*p).map_or(Axial::of(*p), |h| h.at)).map(|m| format!("{} → esagono {:?}, {:.0} min", g.teams[*t].name, m.hex.at, m.minutes)),
                    Order::Evacuate(d) => {
                        let d = g.district_index(d).with_context(|| format!("borgo {d:?} sconosciuto"))?;
                        g.evacuate(d).map(|p| format!("evacua {}: {} persone verso {}, {:.0} min", g.districts[d].name, p.people, p.path.to, p.total_min()))
                    }
                };
                match r {
                    Ok(s) => eprintln!("{} {s}", rocca::words::clock(g.time_s())),
                    Err(e) => eprintln!("{} ordine rifiutato: {e}", rocca::words::clock(g.time_s())),
                }
                next += 1;
            }
            g.resume();
        }
        g.step()?;
        trace(g);
    }
    Ok(())
}

/// With `TRACCIA=1`, every 10 simulated minutes: burnt hectares, burning
/// cells, nearest fire to each district, teams.
fn trace(g: &Game) {
    if std::env::var("TRACCIA").is_err() || g.time_s() % 600 != 0 {
        return;
    }
    let burnt = g.fire.state().iter().filter(|c| **c != fire::CellFire::Unburnt).count() as f32 * 0.04;
    let near: Vec<String> = g
        .districts
        .iter()
        .filter_map(|d| rocca::district::exposure(d, &g.fire, &g.scn).map(|e| format!("{} {:.0}", d.name, e.distance_m)))
        .collect();
    let teams: Vec<String> = (0..g.teams.len()).map(|t| format!("{}: {}", g.teams[t].name, g.team_status(t))).collect();
    eprintln!("{} {:.0} ha, {} celle accese | {} | {}", rocca::words::clock(g.time_s()), burnt, g.fire.active_cells().len(), near.join(", "), teams.join(", "));
}

fn table(g: &Game, o: &Outcome) {
    println!("{:<16} {:>5} {:>6} {:>7} {:>6} {:>6} {:>6} {:>6}", "borgo", "case", "colp.", "persone", "salve", "strada", "autop.", "colte");
    for (d, x) in g.districts.iter().zip(&o.districts) {
        println!("{:<16} {:>5} {:>6} {:>7} {:>6} {:>6} {:>6} {:>6}", d.name, x.homes, x.homes_hit, x.people, x.safe, x.on_the_road, x.self_defence, x.caught);
    }
    println!("fine {}  ettari {:.0}  case colpite {}  persone colte {}  squadre perse {}  pause {}", rocca::words::clock(o.at_s), o.hectares, o.homes_hit(), o.caught(), o.units_lost, g.pauses);
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let case = args.next().context("uso: rocca <caso> [--seme N] [--squadra ...] [--evacua ...]")?;
    let (mut seed, mut data, mut teams, mut evac) = (2u64, PathBuf::from("data"), String::new(), String::new());
    let mut start: Option<i64> = None;
    let mut aim = abm::suppression::EngineAim::default();
    let mut tune = rocca::game::Tuning::default();
    while let Some(a) = args.next() {
        let mut v = || args.next().with_context(|| format!("{a}: manca il valore"));
        match a.as_str() {
            "--seme" => seed = v()?.parse()?,
            "--dati" => data = PathBuf::from(v()?),
            "--squadra" => teams = v()?,
            "--evacua" => evac = v()?,
            "--mira" => {
                aim = match v()?.as_str() {
                    "vicino" => abm::suppression::EngineAim::Hottest,
                    "avanti" => abm::suppression::EngineAim::Ahead,
                    x => bail!("--mira: vicino|avanti, non {x:?}"),
                }
            }
            "--tune" => {
                for kv in v()?.split(',') {
                    let (k, x) = kv.split_once('=').context("--tune: chiave=valore")?;
                    rocca::lab::run::set_dial(&mut tune, k, x.parse()?)?;
                }
            }
            "--inizio" => start = Some(v()?.parse::<i64>()?),
            x => bail!("argomento sconosciuto: {x}"),
        }
    }
    let t_start = std::time::Instant::now();
    let (g, label) = if case == "tutorial" {
        let t = rocca::Territory::load(&data)?;
        let c = t.tutorial.context("game.json: nessun tutorial")?.case;
        let mut g = Game::new(&data, &c, seed)?;
        let s = Script::of(&g).context("tutorial incompleto")?;
        for st in &s.steps {
            if st.expect == rocca::tutorial::Expect::Over {
                while !g.is_over() && !g.evacuation_finished() {
                    g.step()?;
                    trace(&g);
                }
            } else {
                Script { steps: vec![st.clone()] }.play(&mut g)?;
            }
        }
        (g, c)
    } else {
        let t = rocca::Territory::load(&data)?;
        let mut c = t.case(&case).with_context(|| format!("caso {case:?} sconosciuto"))?.clone();
        if let Some(s) = start {
            c.start_min = s;
        }
        let mut g = Game::with_case(&data, t, c.clone(), seed)?;
        g.set_tuning(tune);
        g.crews.engine_aim = aim;
        let t0 = g.time_s();
        let orders: Vec<_> = parse(&teams, &evac)?.into_iter().map(|(at, o)| (at + t0, o)).collect();
        play(&mut g, &orders)?;
        let none = Game::with_case(&data, g.territory.clone(), c, seed).and_then(|mut n| {
            while !n.is_over() {
                n.step()?;
            }
            Ok(n.outcome())
        })?;
        report(&g, &none, t_start);
        return Ok(());
    };
    let none = Game::without_orders(&data, &label, seed)?;
    report(&g, &none, t_start);
    Ok(())
}

fn report(g: &Game, none: &Outcome, t0: std::time::Instant) {
    let mine = g.outcome();
    table(g, &mine);
    println!("senza ordini: ettari {:.0}  case colpite {}  persone colte {}", none.hectares, none.homes_hit(), none.caught());
    let st = score::stars(&mine, none);
    let s = |x: Option<u8>| x.map_or("—".to_string(), |v| v.to_string());
    println!("stelle: persone {}  case {}  ettari {}   ({:.1} s)", s(st.persone), s(st.case), s(st.ettari), t0.elapsed().as_secs_f32());
}
