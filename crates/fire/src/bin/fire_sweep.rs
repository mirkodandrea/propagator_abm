//! `fire_sweep`: run a batch of fires on one scenario and dump their arrival
//! times, for the Python scenario factory (`tools/scenario_factory.py fires`).
//! The factory never re-implements the propagator; it writes a job list,
//! calls this, and reads the rasters back.
//!
//! ```text
//! fire_sweep <data_dir> <scenario_id> <jobs.txt> <out_dir> [minutes] [step_s]
//! ```
//!
//! One job per line of `jobs.txt`, `#` comments allowed:
//!
//! ```text
//! name row col radius_m seed moisture_pct wind_from_deg wind_kmh [shift_min wind_from_deg wind_kmh]
//! ```
//!
//! Each job writes `<out_dir>/<name>.i32`: row-major little-endian i32, the
//! ignition time of every fire-grid cell in seconds, `-1` if it never burnt.
//! One CSV row per job goes to stdout.

use anyhow::{bail, Context, Result};
use fire::{FireSim, Weather};
use scenario::{Cell, Scenario};
use std::io::Write;
use std::path::PathBuf;

/// Minutes at which the CSV reports the burnt area.
const MARKS_MIN: [i64; 6] = [30, 60, 120, 180, 240, 360];

struct Job {
    name: String,
    centre: Cell,
    radius_m: f32,
    seed: u64,
    weather: Weather,
    shift: Option<(i64, Weather)>,
}

fn parse(line: &str) -> Result<Job> {
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() != 8 && f.len() != 11 {
        bail!("expected 8 or 11 fields, got {}", f.len());
    }
    let num = |i: usize| -> Result<f64> { f[i].parse::<f64>().with_context(|| format!("field {i}: {:?}", f[i])) };
    let moisture = num(5)?;
    let weather = Weather { wind_dir_deg: num(6)?, wind_speed_kmh: num(7)?, moisture_pct: moisture };
    let shift = if f.len() == 11 {
        Some(((num(8)? * 60.0) as i64, Weather { wind_dir_deg: num(9)?, wind_speed_kmh: num(10)?, moisture_pct: moisture }))
    } else {
        None
    };
    Ok(Job {
        name: f[0].to_string(),
        centre: Cell { row: num(1)? as usize, col: num(2)? as usize },
        radius_m: num(3)? as f32,
        seed: num(4)? as u64,
        weather,
        shift,
    })
}

fn run(scn: &Scenario, job: &Job, minutes: i64, step_s: i64, out: &PathBuf) -> Result<String> {
    let mut sim = FireSim::new(scn, job.weather, job.seed)?;
    sim.ignite_patch(job.centre, job.radius_m, scn)?;
    let cell_ha = (scn.world.cellsize as f64).powi(2) / 10_000.0;
    let mut areas = Vec::new();
    let end_s = minutes * 60;
    while sim.time_s() < end_s {
        if let Some((at, w)) = job.shift {
            if sim.time_s() == at {
                sim.set_weather(w)?;
            }
        }
        // Never step across a mark or the wind change.
        let mut next = (sim.time_s() + step_s).min(end_s);
        for m in MARKS_MIN.iter().map(|m| m * 60).chain(job.shift.map(|(at, _)| at)) {
            if m > sim.time_s() && m < next {
                next = m;
            }
        }
        sim.advance(next - sim.time_s())?;
        if MARKS_MIN.iter().any(|m| m * 60 == sim.time_s()) {
            let burnt = sim.arrival_times().iter().filter(|&&t| t != i32::MIN).count();
            areas.push(format!("{:.1}", burnt as f64 * cell_ha));
        }
    }
    while areas.len() < MARKS_MIN.len() {
        areas.push(String::new());
    }
    let mut bytes = Vec::with_capacity(sim.arrival_times().len() * 4);
    for &t in sim.arrival_times() {
        bytes.extend_from_slice(&(if t == i32::MIN { -1 } else { t }).to_le_bytes());
    }
    std::fs::write(out.join(format!("{}.i32", job.name)), bytes)?;
    let burnt = sim.arrival_times().iter().filter(|&&t| t != i32::MIN).count();
    Ok(format!("{},{},{:.1}", job.name, areas.join(","), burnt as f64 * cell_ha))
}

fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 4 {
        bail!("usage: fire_sweep <data_dir> <scenario_id> <jobs.txt> <out_dir> [minutes=180] [step_s=60]");
    }
    let scn = Scenario::load_by_id(&a[0], &a[1]).with_context(|| format!("loading scenario {}", a[1]))?;
    let jobs = std::fs::read_to_string(&a[2]).with_context(|| format!("reading {}", a[2]))?;
    let out = PathBuf::from(&a[3]);
    std::fs::create_dir_all(&out)?;
    let minutes: i64 = a.get(4).map_or(Ok(180), |s| s.parse())?;
    let step_s: i64 = a.get(5).map_or(Ok(60), |s| s.parse())?;

    let stdout = std::io::stdout();
    let mut w = stdout.lock();
    writeln!(w, "name,{},ha_end", MARKS_MIN.map(|m| format!("ha_{m}min")).join(","))?;
    for (i, line) in jobs.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let job = parse(line).with_context(|| format!("jobs line {}", i + 1))?;
        let row = run(&scn, &job, minutes, step_s, &out).with_context(|| format!("job {}", job.name))?;
        writeln!(w, "{row}")?;
    }
    Ok(())
}
