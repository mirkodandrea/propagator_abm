//! District commanders against drawn sessions (spec §2): who to warn, not just when.
//! `cargo test -p demo --release --test district_sweep -- --ignored --nocapture`

use demo::policy::Policy;

#[test]
#[ignore]
fn district_policies() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let seeds: Vec<u64> = (1..=16).map(|s| s * 7919).collect();
    for id in demo::ALL {
        println!("{id}: policy | caught (mean) | false alarms | households warned | caught on shift / no-shift sessions");
        for p in Policy::district_set() {
            let (mut caught, mut needless, mut warned) = (0.0, 0.0, 0.0);
            let (mut cs, mut ns, mut cn, mut nn) = (0.0, 0, 0.0, 0);
            for &seed in &seeds {
                let draw = demo::draw(id, seed).unwrap();
                let mut run = demo::Run::new(&dir, draw.spec, draw.seed)?;
                let o = p.play(&mut run, &draw)?;
                caught += o.caught as f32;
                needless += run.referee.reports.iter().filter(|r| r.needless()).count() as f32;
                warned += run.agents.households.iter().filter(|h| h.ordered).count() as f32;
                if draw.spec.shift.is_some() { cs += o.caught as f32; ns += 1 } else { cn += o.caught as f32; nn += 1 }
            }
            let n = seeds.len() as f32;
            println!("  {:<32} {:>6.1} {:>6.2} {:>7.0}   {:>5.1} / {:>5.1}", p.name, caught / n, needless / n, warned / n, cs / ns.max(1) as f32, cn / nn.max(1) as f32);
        }
    }
    Ok(())
}

/// How fast a warning loses its value: district 0 at T+0, district 1 at T+k, on
/// sessions where the wind shifts (district 1's danger). And district 0 at T+k
/// where it holds.
#[test]
#[ignore]
fn warning_decay() -> anyhow::Result<()> {
    use demo::policy::Act;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    for id in demo::ALL {
        let mut line = format!("{id}:");
        for k in [0i64, 5, 10, 15, 20] {
            let (mut c1, mut n1, mut c0, mut n0) = (0.0, 0, 0.0, 0);
            for seed in 1..=24u64 {
                let draw = demo::draw(id, seed).unwrap();
                let shift = draw.spec.shift.is_some();
                let p = if shift {
                    Policy::named("x").at(0, Act::WarnDistrict(0)).at(k * 60, Act::WarnDistrict(1))
                } else {
                    Policy::named("x").at(k * 60, Act::WarnDistrict(0))
                };
                let mut run = demo::Run::new(&dir, draw.spec, draw.seed)?;
                let o = p.play(&mut run, &draw)?;
                if shift { c1 += o.caught as f32; n1 += 1 } else { c0 += o.caught as f32; n0 += 1 }
            }
            line += &format!("  T+{k}: shift {:.1} hold {:.1} |", c1 / n1.max(1) as f32, c0 / n0.max(1) as f32);
        }
        println!("{line}");
    }
    Ok(())
}

/// Engines defending a district (option B, defence on): which district and when.
#[test]
#[ignore]
fn defend_sweep() -> anyhow::Result<()> {
    use demo::policy::Act;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let v = demo::Variant { defend_homes: true, ..Default::default() };
    for id in demo::ALL {
        let mut line = format!("{id} homes lost:");
        let ps = vec![
            Policy::none(),
            Policy::named("d0 T+3").at(180, Act::DefendDistrict(0, 3)),
            Policy::named("d0 T+15").at(900, Act::DefendDistrict(0, 3)),
            Policy::named("d1 T+3").at(180, Act::DefendDistrict(1, 3)),
            Policy::named("d2 T+3").at(180, Act::DefendDistrict(2, 3)),
            Policy::named("d0+d1 T+3").at(180, Act::DefendDistrict(0, 2)).at(180, Act::DefendDistrict(1, 1)),
        ];
        for p in &ps {
            let (mut hold, mut nh, mut shift, mut ns) = (0.0, 0, 0.0, 0);
            for seed in 1..=16u64 {
                let draw = demo::draw(id, seed).unwrap();
                let mut run = demo::Run::with_variant(&dir, draw.spec, draw.seed, v)?;
                let o = p.play(&mut run, &draw)?;
                if draw.spec.shift.is_some() { shift += o.homes_lost as f32; ns += 1 } else { hold += o.homes_lost as f32; nh += 1 }
            }
            line += &format!(" | {}: hold {:.1} shift {:.1}", p.name, hold / nh.max(1) as f32, shift / ns.max(1) as f32);
        }
        println!("{line}");
    }
    Ok(())
}
