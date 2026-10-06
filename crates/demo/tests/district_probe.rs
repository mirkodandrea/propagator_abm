//! Where and when the fire reaches each district with nobody giving an order,
//! over drawn sessions: the evidence the town layouts were drawn from.
//! `cargo test -p demo --release --test district_probe -- --ignored --nocapture`

#[test]
#[ignore]
fn district_stories() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    for id in demo::ALL {
        let seeds = 16u64;
        for seed in 1..=seeds {
            let draw = demo::draw(id, seed * 7919).unwrap();
            let mut run = demo::Run::new(&dir, draw.spec, draw.seed)?;
            run.play(&[])?;
            let o = run.outcome();
            let mut line = format!("{id} s{seed:>2} shift {:>4} ha {:>3.0} caught {:>3} |", draw.spec.shift.map_or("-".into(), |s| format!("{}", s.at_s / 60)), o.hectares, o.caught);
            for (d, r) in run.referee.districts.iter().zip(&run.referee.reports) {
                let m = |t: Option<i64>| t.map_or("  -".to_string(), |t| format!("{:>3}", t / 60));
                line += &format!(" {:>12}: thr {} reach {} caught {:>3}/{:<3} closest {:>5.0} |", d.name, m(r.threatened_at_s), m(r.reached_at_s), r.caught, r.households, r.closest_m);
            }
            println!("{line}");
        }
    }
    Ok(())
}
/// ASCII map of what burnt by the end (80 m per character), homes as `h`, the
/// ignition as `X`. One session with a shift and one without, per town.
#[test]
#[ignore]
fn burn_maps() -> anyhow::Result<()> {
    use fire::CellFire;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    for id in demo::ALL {
        let mut shown = (false, false);
        for seed in 1..40u64 {
            let draw = demo::draw(id, seed * 7919).unwrap();
            let shifted = draw.spec.shift.is_some();
            if (shifted && shown.1) || (!shifted && shown.0) {
                continue;
            }
            if shifted { shown.1 = true } else { shown.0 = true }
            let mut run = demo::Run::new(&dir, draw.spec, draw.seed)?;
            let mut at = [None::<i64>; 3];
            let marks = [20 * 60, 40 * 60, 60 * 60];
            let mut snaps: Vec<Vec<CellFire>> = vec![];
            while run.time_s() < run.spec.duration_s {
                run.step()?;
                for (k, m) in marks.iter().enumerate() {
                    if at[k].is_none() && run.time_s() >= *m {
                        at[k] = Some(run.time_s());
                        snaps.push(run.fire.state().to_vec());
                    }
                }
            }
            let w = &run.scn.world;
            println!("{id} seed {} shift {:?} wind {:.0}@{:.0} caught {} ha {:.0}", seed * 7919, draw.spec.shift.map(|s| s.at_s / 60), draw.spec.weather.wind_dir_deg, draw.spec.weather.wind_speed_kmh, run.outcome().caught, run.outcome().hectares);
            let fin = run.fire.state().to_vec();
            let homes: Vec<(usize, usize)> = run.agents.households.iter().map(|h| ((h.home.x / 80.0) as usize, ((w.height_m - h.home.y) / 80.0) as usize)).collect();
            let ig = run.spec.ignition;
            for r in 0..50 {
                let mut line = String::new();
                for c in 0..50 {
                    let (x, y) = (c as f32 * 80.0 + 40.0, w.height_m - (r as f32 * 80.0 + 40.0));
                    let cell = w.cell_of(scenario::Pos { x, y });
                    let i = cell.row * w.fire_cols + cell.col;
                    let ch = if (ig.x / 80.0) as usize == c && ((w.height_m - ig.y) / 80.0) as usize == r {
                        'X'
                    } else if homes.contains(&(c, r)) {
                        'h'
                    } else if snaps.first().is_some_and(|s| s[i] != CellFire::Unburnt) {
                        '1'
                    } else if snaps.get(1).is_some_and(|s| s[i] != CellFire::Unburnt) {
                        '2'
                    } else if snaps.get(2).is_some_and(|s| s[i] != CellFire::Unburnt) {
                        '3'
                    } else if fin[i] != CellFire::Unburnt {
                        '#'
                    } else {
                        '.'
                    };
                    line.push(ch);
                }
                println!("{line}");
            }
            if shown == (true, true) {
                break;
            }
        }
    }
    Ok(())
}

/// Hectares burnt at T+20/40/60 against moisture and wind, fire only.
#[test]
#[ignore]
fn fire_speed_sweep() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    for id in demo::ALL {
        let base = demo::spec(id).unwrap();
        for m in [3.0, 4.0, 5.0, 7.0] {
            for kmh in [25.0, 35.0, 45.0] {
                let mut ha = [0.0f32; 3];
                let seeds = 4;
                for seed in 1..=seeds {
                    let mut s = base;
                    s.weather.moisture_pct = m;
                    s.weather.wind_speed_kmh = kmh;
                    let scn = scenario::Scenario::load_by_id(&dir, id)?;
                    let mut f = fire::FireSim::new(&scn, s.weather, seed)?;
                    f.ignite_patch(scn.world.cell_of(s.ignition), s.radius_m, &scn)?;
                    for (k, t) in [20 * 60, 40 * 60, 60 * 60].iter().enumerate() {
                        while f.time_s() < *t {
                            f.advance(60)?;
                        }
                        ha[k] += f.state().iter().filter(|c| **c != fire::CellFire::Unburnt).count() as f32 * 0.04 / seeds as f32;
                    }
                }
                println!("{id} moist {m} wind {kmh}: ha T+20 {:.0} T+40 {:.0} T+60 {:.0}", ha[0], ha[1], ha[2]);
            }
        }
    }
    Ok(())
}

/// Metres the head has run downwind of the ignition at T+10/20/30/45/60, by opening radius.
#[test]
#[ignore]
fn head_run_sweep() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    for id in demo::ALL {
        let base = demo::spec(id).unwrap();
        let scn = scenario::Scenario::load_by_id(&dir, id)?;
        for r in [60.0, 150.0, 250.0] {
            for kmh in [35.0, 45.0] {
                let marks = [10, 20, 30, 45, 60];
                let mut run_m = [0.0f32; 5];
                let seeds = 4;
                for seed in 1..=seeds {
                    let mut s = base;
                    s.weather.wind_speed_kmh = kmh;
                    s.weather.moisture_pct = 5.0;
                    let mut f = fire::FireSim::new(&scn, s.weather, seed)?;
                    let c = scn.world.cell_of(s.ignition);
                    f.ignite_patch(c, r, &scn)?;
                    let o = scn.world.centre_of(c);
                    let to = (s.weather.wind_dir_deg as f32 + 180.0).to_radians();
                    let (ux, uy) = (to.sin(), to.cos());
                    for (k, t) in marks.iter().enumerate() {
                        while f.time_s() < t * 60 {
                            f.advance(60)?;
                        }
                        let w = &scn.world;
                        let mut best = 0.0f32;
                        for (i, st) in f.state().iter().enumerate() {
                            if *st != fire::CellFire::Unburnt {
                                let p = w.centre_of(scenario::Cell { row: i / w.fire_cols, col: i % w.fire_cols });
                                best = best.max((p.x - o.x) * ux + (p.y - o.y) * uy);
                            }
                        }
                        run_m[k] += best / seeds as f32;
                    }
                }
                println!("{id} r {r} wind {kmh}: head run m at T+10/20/30/45/60 = {:?}", run_m.map(|m| m as i32));
            }
        }
    }
    Ok(())
}

/// Who is still caught when everyone is warned at T+0, and why.
#[test]
#[ignore]
fn who_is_caught_after_a_t0_warning() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    for id in demo::ALL {
        let mut by: std::collections::BTreeMap<String, usize> = Default::default();
        for seed in 1..=8u64 {
            let draw = demo::draw(id, seed * 7919).unwrap();
            let mut run = demo::Run::new(&dir, draw.spec, draw.seed)?;
            run.order(demo::Order::EvacuateAll);
            let mut seen = vec![false; run.agents.households.len()];
            while run.time_s() < run.spec.duration_s {
                run.step()?;
                for i in 0..seen.len() {
                    if !seen[i] && run.referee.tally.caught_at(i).is_some() {
                        seen[i] = true;
                        let h = &run.agents.households[i];
                        let key = format!("{:?} heard={} intent={:?} chan={:?} trust<.35={} prep_left={:.0}min", h.status, h.warning_received, h.intent, h.channel, h.trust_authority < 0.35, h.prep_remaining_s / 60.0);
                        *by.entry(key).or_default() += 1;
                    }
                }
            }
        }
        println!("{id} (8 sessions, warn all at T+0):");
        let mut v: Vec<_> = by.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        for (k, n) in v.iter().take(14) {
            println!("  {n:>4}  {k}");
        }
    }
    Ok(())
}

#[test]
#[ignore]
fn refuges() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    for id in demo::ALL {
        let draw = demo::draw(id, 7919).unwrap();
        let run = demo::Run::new(&dir, draw.spec, draw.seed)?;
        println!("{id}: {:?}", run.agents.refuges.iter().map(|r| (r.pos.x as i32, r.pos.y as i32, r.is_exit, (r.burnable_frac * 100.0) as i32)).collect::<Vec<_>>());
    }
    Ok(())
}

#[test]
#[ignore]
fn engines_after_a_defend_order() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let v = demo::Variant { defend_homes: true, ..Default::default() };
    let spec = demo::spec("demo_valle").unwrap();
    let mut run = demo::Run::with_variant(&dir, spec, 3, v)?;
    for _ in 0..30 { run.step()?; }
    for _ in 0..3 { run.order(demo::Order::Defend { kind: abm::suppression::UnitKind::Engine, district: 0 }); }
    let d0 = run.referee.districts[0].centre;
    println!("district 0 centre {:?}; units: {:?}", d0, run.crews.units.iter().map(|u| (u.id, u.kind, u.state, u.pos.x as i32, u.pos.y as i32)).collect::<Vec<_>>());
    for k in 0..40 {
        for _ in 0..10 { run.step()?; }
        let eng: Vec<_> = run.crews.units.iter().filter(|u| u.kind == abm::suppression::UnitKind::Engine).map(|u| format!("{:?}@({},{}) w{:.0}", u.state, u.pos.x as i32, u.pos.y as i32, u.water_l)).collect();
        let defended = run.agents.households.iter().enumerate().filter(|(i, _)| run.referee.tally.defended(*i, run.time_s())).count();
        if k % 4 == 0 { println!("T+{:>3} {:?} defended {defended}", run.time_s() / 60, eng); }
    }
    Ok(())
}

/// When do districts the fire eventually reaches first become threatened
/// (fire within district::THREATENED_M)? Sets how soon a false alarm can be judged.
#[test]
#[ignore]
fn threatened_times() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    for id in demo::ALL {
        let mut ts: Vec<i64> = vec![];
        let mut never = 0;
        for seed in 1..=24u64 {
            let draw = demo::draw(id, seed).unwrap();
            let mut run = demo::Run::new(&dir, draw.spec, draw.seed)?;
            run.play(&[])?;
            for (k, r) in run.referee.reports.iter().enumerate() {
                if k == 2 { if r.threatened_at_s.is_none() { never += 1 } continue; }
                if r.reached_at_s.is_some() {
                    ts.push(r.threatened_at_s.unwrap_or(9999) / 60);
                }
            }
        }
        ts.sort();
        let q = |f: f32| ts[((ts.len() as f32 - 1.0) * f) as usize];
        println!("{id}: reached districts threatened at min p10 {} p50 {} p90 {} max {} (n {}); upwind never threatened {never}/24", q(0.1), q(0.5), q(0.9), ts.last().unwrap(), ts.len());
    }
    Ok(())
}
