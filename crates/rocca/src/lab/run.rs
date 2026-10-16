//! Playing one job headless and a whole batch in parallel.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use abm::suppression::EngineAim;
use anyhow::{Context, Result};

use super::strategy::{self, Strategy};
use super::telemetry::{Baseline, Metrics, Recorder, Sample, UnitSample};
use crate::case::{Kind, Slot};
use crate::game::{Game, Tuning};
use crate::Territory;

/// One run: a scenario, a policy, a number of teams, a seed, a tuning.
#[derive(Debug, Clone)]
pub struct Job {
    pub scenario: String,
    pub data_dir: PathBuf,
    pub strategy: String,
    pub teams: usize,
    pub seed: u64,
    pub tuning: Tuning,
    /// What differs from the default tuning, for the tables («», or «k=v»).
    pub tune_tag: String,
    pub aim: EngineAim,
    /// The team (of `teams`) left out of the roster: the leave-one-out run.
    pub without: Option<usize>,
}

impl Job {
    pub fn key(&self) -> (String, String, usize, String, String) {
        (
            self.scenario.clone(),
            self.strategy.clone(),
            self.teams,
            format!("{}{}", self.tune_tag, if self.aim == EngineAim::Hottest { " aim=vicino" } else { "" }),
            self.without.map_or(String::new(), |k| format!("senza squadra {k}")),
        )
    }
}

pub struct RunOut {
    pub job: Job,
    pub metrics: Metrics,
    pub series: Vec<Sample>,
    pub unit_series: Vec<UnitSample>,
    pub pauses: u32,
}

fn roster(territory: &Territory, teams: usize, without: Option<usize>) -> Vec<Slot> {
    (0..teams)
        .filter(|k| Some(*k) != without)
        .map(|k| Slot { kind: Kind::Engine, station: k % territory.stations.len(), crew: 3 })
        .collect()
}

fn open(job: &Job, teams: usize, without: Option<usize>) -> Result<Game> {
    let mut territory = Territory::load(&job.data_dir)?;
    territory.roster = roster(&territory, teams, without);
    let case = territory.case("lab").context("scenario without the case «lab»")?.clone();
    let mut g = Game::with_case(&job.data_dir, territory, case, job.seed)?;
    g.set_tuning(job.tuning);
    g.crews.engine_aim = job.aim;
    Ok(g)
}

/// The same fire with every team idle.
pub fn baseline(job: &Job) -> Result<Baseline> {
    let mut g = open(job, job.teams.max(1), None)?;
    let mut rec = Recorder::new(&g, false);
    while !g.is_over() {
        g.step()?;
        rec.observe(&g);
    }
    let o = g.outcome();
    Ok(Baseline {
        ha: o.hectares,
        homes_hit: o.homes_hit(),
        reach_m: rec.metrics_reach(),
        curve: rec.curve().to_vec(),
        burnt: g.fire.state().iter().map(|c| *c != fire::CellFire::Unburnt).collect(),
    })
}

fn issue(g: &mut Game, strat: &mut dyn Strategy, rec: &mut Recorder, ros: f32) {
    let geo = strategy::geo(g, ros);
    for (t, p) in strat.plan(g, geo.as_ref()) {
        let Some(a) = strategy::reachable_near(g, t, p) else { continue };
        if g.teams[t].hex == Some(a) {
            continue;
        }
        let again = g.teams[t].hex.is_some();
        if let Ok(plan) = g.move_team(t, a) {
            rec.orders += 1;
            rec.reorders += again as u32;
            rec.order_eta_min.push(plan.minutes);
        }
    }
}

pub fn run(job: &Job, base: &Baseline, keep_series: bool) -> Result<RunOut> {
    let mut g = open(job, job.teams, job.without)?;
    let mut strat = strategy::parse(&job.strategy)?;
    let mut rec = Recorder::new(&g, keep_series);
    let review_s = strat.review_min().map(|m| m * 60);
    g.opening_pause();
    issue(&mut g, strat.as_mut(), &mut rec, 0.0);
    g.resume();
    let t0 = g.time_s();
    let (mut next, mut last_reach, mut last_t) = (review_s.map(|r| t0 + r), 0.0f32, t0);
    while !g.is_over() {
        if let Some(due) = next {
            if g.time_s() >= due {
                let ros = (rec.reach_m() - last_reach) / ((g.time_s() - last_t).max(1) as f32 / 60.0);
                last_reach = rec.reach_m();
                last_t = g.time_s();
                g.pause().map_err(anyhow::Error::msg)?;
                issue(&mut g, strat.as_mut(), &mut rec, ros.max(0.0));
                g.resume();
                next = Some(due + review_s.unwrap());
            }
        }
        g.step()?;
        rec.observe(&g);
    }
    let metrics = rec.finish(&g, base);
    Ok(RunOut { job: job.clone(), metrics, series: rec.series, unit_series: rec.unit_series, pauses: g.pauses })
}

/// Run every job on `threads` threads: the baselines first (one per scenario
/// and seed), then the jobs. Errors are reported and the job skipped.
pub fn run_all(jobs: Vec<Job>, threads: usize, keep_series: bool, progress: bool) -> Vec<RunOut> {
    let mut keys: Vec<(String, u64)> = jobs.iter().map(|j| (j.scenario.clone(), j.seed)).collect();
    keys.sort();
    keys.dedup();
    let bases: Mutex<HashMap<(String, u64), Arc<Baseline>>> = Mutex::new(HashMap::new());
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(k) = keys.get(i) else { break };
                let Some(j) = jobs.iter().find(|j| (j.scenario.clone(), j.seed) == *k) else { continue };
                match baseline(j) {
                    Ok(b) => {
                        bases.lock().unwrap().insert(k.clone(), Arc::new(b));
                    }
                    Err(e) => eprintln!("baseline {} seme {}: {e:#}", k.0, k.1),
                }
            });
        }
    });
    let bases = bases.into_inner().unwrap();
    let out: Mutex<Vec<(usize, RunOut)>> = Mutex::new(vec![]);
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let total = jobs.len();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(j) = jobs.get(i) else { break };
                let Some(b) = bases.get(&(j.scenario.clone(), j.seed)) else { continue };
                if std::env::var_os("LAB_VERBOSE").is_some() {
                    eprintln!("start {} {} x{} seme {}", j.scenario, j.strategy, j.teams, j.seed);
                }
                match run(j, b, keep_series) {
                    Ok(r) => out.lock().unwrap().push((i, r)),
                    Err(e) => eprintln!("{} {} x{} seme {}: {e:#}", j.scenario, j.strategy, j.teams, j.seed),
                }
                let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                if std::env::var_os("LAB_VERBOSE").is_some() {
                    eprintln!("end {} {} x{} seme {}", j.scenario, j.strategy, j.teams, j.seed);
                }
                if progress && (n % 10 == 0 || n == total) {
                    eprintln!("  {n}/{total}");
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by_key(|x| x.0);
    v.into_iter().map(|x| x.1).collect()
}

/// Turn one dial by name (the names of [`Tuning`]'s fields).
pub fn set_dial(t: &mut Tuning, key: &str, v: f64) -> Result<()> {
    match key {
        "engine_range_m" => t.engine_range_m = v as f32,
        "engine_look_s" => t.engine_look_s = v as i64,
        "engine_park_threat" => t.engine_park_threat = v as f32,
        "engine_park_flame_m" => t.engine_park_flame_m = v as f32,
        "refill_s" => t.refill_s = v as f32,
        "crew_range_m" => t.crew_range_m = v as f32,
        "crew_line_offset_m" => t.crew_line_offset_m = v as f32,
        "crew_line_step_m" => t.crew_line_step_m = v as f32,
        "crew_spread_m" => t.crew_spread_m = v as f32,
        "protect_m" => t.protect_m = v as f32,
        "line_x" => t.line_x = v as f32,
        "water_x" => t.water_x = v as f32,
        "nerve_x" => t.nerve_x = v as f32,
        "prewet_m" => t.prewet_m = v as f32,
        "soaked_pts" => t.soaked_pts = v as f32,
        "reengage" => t.reengage = v != 0.0,
        "crew_relaunch" => t.crew_relaunch = v != 0.0,
        "park_ahead" => t.park_ahead = v != 0.0,
        "share_fuel" => t.share_fuel = v != 0.0,
        "relocate_x" => t.relocate_x = v as f32,
        "crew_line_ahead" => t.crew_line_ahead = v != 0.0,
        k => anyhow::bail!(
            "taratura sconosciuta: {k} (engine_range_m engine_look_s engine_park_threat engine_park_flame_m refill_s crew_range_m crew_line_offset_m crew_line_step_m crew_spread_m protect_m line_x water_x nerve_x prewet_m soaked_pts reengage crew_relaunch park_ahead share_fuel relocate_x crew_line_ahead; 0/1 per gli interruttori)"
        ),
    }
    Ok(())
}
