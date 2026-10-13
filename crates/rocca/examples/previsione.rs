//! How good is the forecast «dove andrà il fuoco» (`rocca::forecast`)?
//!
//! For every case with a change of wind and seeds 1-3: the same opening plan
//! (the case's locality first, no civil orders), a forecast at the
//! announcement and one when the wind turns, then the game runs on and the
//! forecast's 50 % area at +1 h and +3 h is compared with what the game
//! actually burnt by then (crews working: the forecast is «senza
//! interventi»). Also the same ensemble without the announced change, to
//! show what the announcement adds, and the wall-clock of each forecast
//! (all cores, and one member on one core).
//!
//!   cargo run --release -p rocca --example previsione

use std::time::Instant;

use fire::CellFire;
use rocca::forecast::{Epoch, Forecast, HORIZONS_S, MEMBERS};
use rocca::{Game, Plan, Territory};

/// (forecast cells, actual cells, hit rate = share of the actual burnt
/// area inside the forecast, false alarms = share of the forecast that did
/// not burn).
fn skill(f: &Forecast, h: usize, burnt: &[bool]) -> (usize, usize, f64, f64) {
    let (mut fc, mut ac, mut both) = (0, 0, 0);
    for (i, &b) in burnt.iter().enumerate() {
        let p = f.likely(h, i);
        fc += p as usize;
        ac += b as usize;
        both += (p && b) as usize;
    }
    let hit = if ac > 0 { both as f64 / ac as f64 } else { f64::NAN };
    let fa = if fc > 0 { (fc - both) as f64 / fc as f64 } else { f64::NAN };
    (fc, ac, hit, fa)
}

fn main() -> anyhow::Result<()> {
    let data = std::path::Path::new("data");
    let t = Territory::load(data)?;
    let cases: Vec<String> = t.cases.iter().map(|c| c.name.clone()).collect();
    println!("Ensemble di {MEMBERS} membri, orizzonti +1 h e +3 h, isocrona al 50 %, senza interventi.\n");
    println!("| caso | seed | emessa | tempo 16 membri (tutti i core) | tempo 1 membro | orizzonte | prevista ha | bruciata ha | presi | falsi allarmi | senza cambio: presi / falsi |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for case in &cases {
        for seed in 1..=3u64 {
            let mut g = Game::new(data, case, seed)?;
            let ha = g.scn.world.cellsize * g.scn.world.cellsize / 10_000.0;
            let near = g.district_index(&g.case.near.clone()).unwrap_or(0);
            let mut prio = vec![near];
            prio.extend((0..g.districts.len()).filter(|&d| d != near));
            g.commit(Plan::new(g.districts.len()).with_priorities(&prio))?;
            // forecasts at the boundary-condition changes; for a case
            // without a change, one at the time the others are announced
            let mut issued: Vec<(Forecast, Forecast, f64, f64)> = vec![];
            // per forecast and horizon: the cells actually burnt by then
            let mut seen: Vec<[Option<Vec<bool>>; 2]> = vec![];
            let mut last = None;
            let end = g.case.duration_s() + HORIZONS_S[1];
            let lone_at = 25 * 60;
            while g.time_s() < end {
                g.step()?;
                for (k, (f, ..)) in issued.iter().enumerate() {
                    for (h, &dt) in HORIZONS_S.iter().enumerate() {
                        if seen[k][h].is_none() && g.time_s() >= f.issued_s + dt {
                            seen[k][h] = Some(g.fire.state().iter().map(|c| *c != CellFire::Unburnt).collect());
                        }
                    }
                }
                let e = g.forecast_epoch();
                let due = (e.is_some() && e != last) || (g.case.shift.is_none() && issued.is_empty() && g.time_s() >= lone_at);
                last = e;
                if !due {
                    continue;
                }
                let t0 = Instant::now();
                let f = g.forecast()?;
                let all = t0.elapsed().as_secs_f64();
                let t1 = Instant::now();
                let mut one = g.forecast_job()?;
                one.step()?;
                let single = t1.elapsed().as_secs_f64();
                let flat = g.forecast_job()?.without_change().run()?;
                issued.push((f, flat, all, single));
                seen.push([None, None]);
            }
            for ((f, flat, all, single), burnt) in issued.iter().zip(&seen) {
                for (h, &dt) in HORIZONS_S.iter().enumerate() {
                    let Some(burnt) = &burnt[h] else { continue };
                    let (fc, ac, hit, fa) = skill(f, h, burnt);
                    let (_, _, hit0, fa0) = skill(flat, h, burnt);
                    let what = match (f.epoch, g.case.shift.is_some()) {
                        (_, false) => "senza cambio",
                        (Epoch::Announced, _) => "annuncio",
                        (Epoch::Shifted, _) => "giro del vento",
                    };
                    println!(
                        "| {case} | {seed} | T+{} min ({what}) | {:.0} ms | {:.1} ms | +{} h | {:.0} | {:.0} | {:.0} % | {:.0} % | {:.0} % / {:.0} % |",
                        f.issued_s / 60,
                        all * 1000.0,
                        single * 1000.0,
                        dt / 3600,
                        fc as f32 * ha,
                        ac as f32 * ha,
                        hit * 100.0,
                        fa * 100.0,
                        hit0 * 100.0,
                        fa0 * 100.0
                    );
                }
            }
        }
    }
    Ok(())
}
