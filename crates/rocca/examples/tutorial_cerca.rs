//! Search for the tutorial's fire (docs/03-VERIFICHE.md): around each
//! district, ignitions upwind of it; for each, the fire with no orders (does
//! it reach the homes?) and with two VVF teams sent at minute 2 to the hexes
//! nearest the ignition and halfway to the district (is it out, and when?).
//!
//!   cargo run --release -p rocca --example tutorial_cerca [-- VENTO_KMH MINUTI]

use std::path::Path;

use anyhow::Result;
use rocca::{Axial, Case, Game, Territory};
use scenario::Pos;

fn d(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn nearest_hex(g: &Game, p: Pos) -> Axial {
    g.hexes.hexes.iter().min_by(|a, b| d(a.target, p).total_cmp(&d(b.target, p))).unwrap().at
}

/// Hectares, homes hit, people caught, minute the fire went out (if it did).
fn run(data: &Path, t: &Territory, c: &Case, district: usize, orders: bool) -> Result<(f32, usize, u32, Option<i64>, f32)> {
    let mut g = Game::with_case(data, t.clone(), c.clone(), 1)?;
    let mut nearest = f32::INFINITY;
    while g.time_s() < c.duration_s() && !g.fire.active_cells().is_empty() {
        if orders && g.time_s() >= 120 && g.pauses == 0 {
            g.pause().unwrap();
            let ign = c.ignition();
            let centre = g.districts[district].centre;
            let half = Pos { x: (ign.x + centre.x) / 2.0, y: (ign.y + centre.y) / 2.0 };
            let (a, b) = (nearest_hex(&g, ign), nearest_hex(&g, half));
            let _ = g.move_team(0, a);
            let _ = g.move_team(1, if b == a { nearest_hex(&g, centre) } else { b });
            let _ = g.evacuate(district);
            g.resume();
        }
        g.step()?;
        if let Some(e) = rocca::district::exposure(&g.districts[district], &g.fire, &g.scn) {
            nearest = nearest.min(e.distance_m);
        }
    }
    let o = g.outcome();
    let out = g.fire.active_cells().is_empty().then(|| g.time_s() / 60);
    Ok((o.hectares, o.homes_hit(), o.caught(), out, nearest))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let wind: f64 = args.first().map_or(Ok(20.0), |s| s.parse())?;
    let minutes: i64 = args.get(1).map_or(Ok(120), |s| s.parse())?;
    let data = Path::new("data");
    let t = Territory::load(data)?;
    let base = Game::new(data, "tutorial", 1)?;
    let mut jobs = vec![];
    for (k, dd) in base.districts.iter().enumerate() {
        for a in 0..8 {
            for r in [500.0f32, 800.0] {
                let ang = (a as f32 * 45.0).to_radians();
                let ign = Pos { x: dd.centre.x + r * ang.sin(), y: dd.centre.y + r * ang.cos() };
                if !base.scn.world.contains(ign) || !base.scn.is_burnable(base.scn.world.cell_of(ign)) {
                    continue;
                }
                let mut c = t.case("tutorial").unwrap().clone();
                c.ignition = [ign.x, ign.y];
                c.wind_from_deg = a as f64 * 45.0;
                c.wind_kmh = wind;
                c.minutes = minutes;
                c.shift = None;
                jobs.push((k, c));
            }
        }
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows: Vec<String> = std::thread::scope(|s| {
        let hs: Vec<_> = jobs
            .chunks(jobs.len().div_ceil(threads).max(1))
            .map(|chunk| {
                let (t, names) = (&t, &base.districts);
                s.spawn(move || {
                    chunk
                        .iter()
                        .map(|(k, c)| {
                            let a = run(data, t, c, *k, false).unwrap();
                            let b = run(data, t, c, *k, true).unwrap();
                            let m = |x: Option<i64>| x.map_or("-".into(), |v| v.to_string());
                            format!(
                                "{:<16} {:>6.0} {:>6.0} {:>4.0} | {:>5.0} {:>4} {:>4} {:>4} {:>5.0} | {:>5.0} {:>4} {:>4} {:>4}",
                                names[*k].name, c.ignition[0], c.ignition[1], c.wind_from_deg, a.0, a.1, a.2, m(a.3), a.4, b.0, b.1, b.2, m(b.3)
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    println!("{:<16} {:>6} {:>6} {:>4} | {:>5} {:>4} {:>4} {:>4} {:>5} | {:>5} {:>4} {:>4} {:>4}", "borgo", "x", "y", "da", "ha", "case", "pers", "spen", "dist", "ha", "case", "pers", "spen");
    for r in rows {
        println!("{r}");
    }
    Ok(())
}
