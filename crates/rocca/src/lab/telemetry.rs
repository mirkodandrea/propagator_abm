//! Everything the laboratory measures about a run, collected as the game
//! steps. The recorder only reads [`Game`]; it never changes it.
//!
//! Two kinds of output: a flat list of named numbers per run ([`Metrics`],
//! the row of `runs.csv`), and optional time series (`series.csv` for the
//! fire, `units.csv` for each team's engine and people).

use std::collections::BTreeMap;

use abm::suppression::{Task, UnitKind, UnitState};
use fire::CellFire;
use scenario::Pos;

use crate::district::dist;
use crate::game::Game;

/// Named numbers, in the order they were added.
#[derive(Debug, Clone, Default)]
pub struct Metrics(pub Vec<(String, f64)>);

impl Metrics {
    pub fn put(&mut self, k: impl Into<String>, v: f64) {
        self.0.push((k.into(), v));
    }
    pub fn get(&self, k: &str) -> Option<f64> {
        self.0.iter().find(|(n, _)| n == k).map(|(_, v)| *v)
    }
}

/// Minutes (since the game began) at which the area burnt is read.
pub const CURVE_MIN: [i64; 7] = [15, 30, 60, 90, 120, 180, 240];

/// Wet enough to count as a wetted cell, percentage points of moisture.
const WET_PTS: f32 = 8.0;
/// Threat at which a team is, by the game's own reckoning, in trouble.
const HOT: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Sector {
    Ahead,
    Flank,
    Behind,
}

/// What a unit has been doing, summed over the run.
#[derive(Debug, Clone, Default)]
pub struct UnitLog {
    pub callsign: String,
    pub kind: Option<UnitKindTag>,
    pub secs_in: BTreeMap<&'static str, f64>,
    pub notes: BTreeMap<&'static str, f64>,
    pub min_fire_m: f32,
    pub max_threat: f32,
    pub max_heat_s: f32,
    pub secs_hot: f64,
    pub relocations: u32,
    pub sector_s: [f64; 3],
    pub first_work_s: Option<i64>,
    pub gap_at_arrival_m: Option<f32>,
    pub water_used_l: f32,
    pub line_cut_m: f32,
    pub lost: bool,
    pub withdrawals: u32,
    /// Seconds working with another team's unit of the same kind close by
    /// (engines within twice the hose reach, people within 60 m): overlap.
    pub secs_crowded: f64,
    pub secs_working: f64,
    last_state: Option<UnitState>,
    last_task: Option<Task>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKindTag {
    Engine,
    Crew,
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub t_min: f64,
    pub burning: usize,
    pub burnt_ha: f32,
    pub reach_m: f32,
    pub homes_hit: usize,
    pub homes_threatened: usize,
    pub cleared_cells: usize,
    pub wet_cells: usize,
    pub litres: f64,
    pub wind_from: f64,
    pub wind_kmh: f64,
}

#[derive(Debug, Clone)]
pub struct UnitSample {
    pub t_min: f64,
    pub unit: usize,
    pub team: usize,
    pub kind: &'static str,
    pub state: &'static str,
    pub pos: Pos,
    pub fire_m: f32,
    pub threat: f32,
    pub heat_s: f32,
    pub water_frac: f32,
    pub water_used_l: f32,
    pub line_cut_m: f32,
    pub note: &'static str,
}

pub struct Recorder {
    t0: i64,
    ignition: Pos,
    pub units: Vec<UnitLog>,
    team_of: Vec<Option<usize>>,
    pub series: Vec<Sample>,
    pub unit_series: Vec<UnitSample>,
    keep_series: bool,
    sample_s: i64,
    reach_m: f32,
    ever_wet: Vec<bool>,
    curve: Vec<(i64, f32)>,
    pub orders: u32,
    pub reorders: u32,
    pub order_eta_min: Vec<f32>,
    first_water_s: Option<i64>,
    first_contact_s: Option<i64>,
    last_litres: f64,
}

impl Recorder {
    pub fn new(g: &Game, keep_series: bool) -> Recorder {
        let mut team_of = vec![None; g.crews.units.len()];
        for (t, team) in g.teams.iter().enumerate() {
            team_of[team.unit] = Some(t);
            for &c in &team.crew_units {
                team_of[c] = Some(t);
            }
        }
        let units = g
            .crews
            .units
            .iter()
            .map(|u| UnitLog {
                callsign: u.callsign.clone(),
                kind: match u.kind {
                    UnitKind::Engine => Some(UnitKindTag::Engine),
                    UnitKind::HandCrew => Some(UnitKindTag::Crew),
                    UnitKind::AirTanker => None,
                },
                min_fire_m: f32::INFINITY,
                ..Default::default()
            })
            .collect();
        Recorder {
            t0: g.time_s(),
            ignition: g.case.ignition(),
            units,
            team_of,
            series: vec![],
            unit_series: vec![],
            keep_series,
            sample_s: 60,
            reach_m: 0.0,
            ever_wet: vec![false; g.fire.state().len()],
            curve: vec![],
            orders: 0,
            reorders: 0,
            order_eta_min: vec![],
            first_water_s: None,
            first_contact_s: None,
            last_litres: 0.0,
        }
    }

    pub fn burnt_ha(g: &Game) -> f32 {
        let n = g.fire.state().iter().filter(|c| **c != CellFire::Unburnt).count();
        n as f32 * g.scn.world.cellsize * g.scn.world.cellsize / 10_000.0
    }

    /// Observe the game after one step ([`crate::STEP_S`] seconds).
    pub fn observe(&mut self, g: &Game) {
        let dt = crate::STEP_S as f64;
        let now = g.time_s();
        let world = g.scn.world;
        let active: Vec<Pos> = g.fire.active_cells().iter().map(|c| world.centre_of(*c)).collect();
        for p in &active {
            self.reach_m = self.reach_m.max(dist(*p, self.ignition));
        }
        let w = g.fire.weather();
        let to = (w.wind_dir_deg as f32 + 180.0).to_radians();
        let wind_to = (to.sin(), to.cos());

        for (k, u) in g.crews.units.iter().enumerate() {
            let log = &mut self.units[k];
            if log.kind.is_none() {
                continue;
            }
            if u.state == UnitState::Working && !u.aboard {
                log.secs_working += dt;
                let close = if u.kind == UnitKind::Engine { 2.0 * g.tuning.engine_range_m } else { 60.0 };
                let crowded = g.crews.units.iter().enumerate().any(|(j, o)| {
                    j != k && o.kind == u.kind && o.state == UnitState::Working && !o.aboard && self.team_of[j].is_some() && self.team_of[j] != self.team_of[k] && dist(o.pos, u.pos) < close
                });
                if crowded {
                    log.secs_crowded += dt;
                }
            }
            *log.secs_in.entry(u.state.label()).or_default() += dt;
            if matches!(u.state, UnitState::Working | UnitState::Moving | UnitState::Refilling | UnitState::Staged) && !u.note.is_empty() {
                *log.notes.entry(u.note).or_default() += dt;
            }
            let threat = g.fire.threat().at(u.pos);
            log.max_threat = log.max_threat.max(threat);
            log.max_heat_s = log.max_heat_s.max(u.heat_s);
            if threat >= HOT {
                log.secs_hot += dt;
            }
            if u.state == UnitState::Lost {
                log.lost = true;
            }
            if u.state == UnitState::Withdrawing && log.last_state != Some(UnitState::Withdrawing) {
                log.withdrawals += 1;
            }
            // the engine moves on its own to a better place: count task changes with no new order
            if u.kind == UnitKind::Engine {
                if let (Some(Task::Attack { at: a }), Task::Attack { at: b }) = (log.last_task, u.task) {
                    if dist(a, b) > 30.0 {
                        log.relocations += 1;
                    }
                }
            }
            log.last_task = Some(u.task);
            // distance to the nearest flame, for the units that matter, every other step
            let near = if u.aboard || active.is_empty() || (now / crate::STEP_S) % 2 != 0 {
                None
            } else {
                active.iter().map(|p| dist(*p, u.pos)).fold(None, |m: Option<f32>, d| Some(m.map_or(d, |x| x.min(d))))
            };
            if let Some(d) = near {
                log.min_fire_m = log.min_fire_m.min(d);
                if u.state == UnitState::Working && matches!(u.task, Task::Attack { .. } | Task::Line { .. }) {
                    // where from the flame it stands, against the wind
                    let f = active.iter().copied().min_by(|a, b| dist(*a, u.pos).total_cmp(&dist(*b, u.pos))).unwrap();
                    let cos = ((u.pos.x - f.x) * wind_to.0 + (u.pos.y - f.y) * wind_to.1) / d.max(1.0);
                    let s = if cos > 0.5 { Sector::Ahead } else if cos < -0.5 { Sector::Behind } else { Sector::Flank };
                    log.sector_s[s as usize] += dt * 2.0;
                }
            }
            if u.state == UnitState::Working && log.first_work_s.is_none() && !u.aboard {
                log.first_work_s = Some(now);
                log.gap_at_arrival_m = near.or_else(|| active.iter().map(|p| dist(*p, u.pos)).fold(None, |m: Option<f32>, d| Some(m.map_or(d, |x| x.min(d)))));
                if self.first_contact_s.is_none() {
                    self.first_contact_s = Some(now);
                }
            }
            log.last_state = Some(u.state);
            log.water_used_l = u.water_used_l;
            log.line_cut_m = u.line_cut_m;
            if u.water_used_l > 0.0 && self.first_water_s.is_none() {
                self.first_water_s = Some(now);
            }
        }

        // wetted cells, for «did the water hold»
        if now % 60 == 0 {
            for (i, &m) in g.fire.added_moisture().iter().enumerate() {
                if m >= WET_PTS {
                    self.ever_wet[i] = true;
                }
            }
        }

        let min = (now - self.t0) / 60;
        if now % 60 == 0 {
            if let Some(&m) = CURVE_MIN.iter().find(|&&m| m == min) {
                self.curve.push((m, Self::burnt_ha(g)));
            }
        }

        if self.keep_series && now % self.sample_s == 0 {
            let exp = g.fire.exposure().fields();
            let wet = g.fire.added_moisture().iter().filter(|m| **m >= WET_PTS).count();
            self.series.push(Sample {
                t_min: (now - self.t0) as f64 / 60.0,
                burning: active.len(),
                burnt_ha: Self::burnt_ha(g),
                reach_m: self.reach_m,
                homes_hit: exp.iter().filter(|e| e.alight).count(),
                homes_threatened: exp.iter().filter(|e| e.damage > 0.1 && !e.alight).count(),
                cleared_cells: g.fire.cleared().iter().filter(|c| **c).count(),
                wet_cells: wet,
                litres: g.fire.litres_applied,
                wind_from: w.wind_dir_deg,
                wind_kmh: w.wind_speed_kmh,
            });
            if now % (self.sample_s * 2) == 0 {
                for (k, u) in g.crews.units.iter().enumerate() {
                    let Some(team) = self.team_of[k] else { continue };
                    let fire_m = active.iter().map(|p| dist(*p, u.pos)).fold(f32::INFINITY, f32::min);
                    self.unit_series.push(UnitSample {
                        t_min: (now - self.t0) as f64 / 60.0,
                        unit: k,
                        team,
                        kind: u.kind.label(),
                        state: u.state.label(),
                        pos: u.pos,
                        fire_m: if fire_m.is_finite() { fire_m } else { -1.0 },
                        threat: g.fire.threat().at(u.pos),
                        heat_s: u.heat_s,
                        water_frac: u.water_frac(),
                        water_used_l: u.water_used_l,
                        line_cut_m: u.line_cut_m,
                        note: u.note,
                    });
                }
            }
        }
        self.last_litres = g.fire.litres_applied;
    }

    /// The row of `runs.csv` for this run.
    pub fn finish(&self, g: &Game, base: &Baseline) -> Metrics {
        let mut m = Metrics::default();
        let out = g.outcome();
        let minutes = (g.time_s() - self.t0) as f64 / 60.0;
        let ha = out.hectares as f64;
        m.put("minutes", minutes);
        m.put("ha", ha);
        m.put("homes", out.districts.iter().map(|d| d.homes).sum::<usize>() as f64);
        m.put("homes_hit", out.homes_hit() as f64);
        m.put("units_lost", out.units_lost as f64);
        m.put("base_ha", base.ha as f64);
        m.put("base_homes_hit", base.homes_hit as f64);
        m.put("saved_ha", base.ha as f64 - ha);
        m.put("saved_pct", if base.ha > 0.0 { 100.0 * (base.ha as f64 - ha) / base.ha as f64 } else { 0.0 });
        m.put("saved_homes", base.homes_hit as f64 - out.homes_hit() as f64);
        m.put("base_reach_m", base.reach_m as f64);
        m.put("reach_m", self.reach_m as f64);
        // growth against the baseline at fixed times
        for &(t, h) in &self.curve {
            let b = base.curve.iter().find(|(bt, _)| *bt == t).map_or(f32::NAN, |x| x.1);
            m.put(format!("ha_{t}"), h as f64);
            m.put(format!("ha_{t}_vs_base"), if b > 0.0 { 100.0 * (h - b) as f64 / b as f64 } else { f64::NAN });
        }
        // cells saved and cells burnt that the baseline spared (chaos)
        let burnt: Vec<bool> = g.fire.state().iter().map(|c| *c != CellFire::Unburnt).collect();
        let cell_ha = g.scn.world.cellsize * g.scn.world.cellsize / 10_000.0;
        let saved = burnt.iter().zip(&base.burnt).filter(|(a, b)| !**a && **b).count();
        let extra = burnt.iter().zip(&base.burnt).filter(|(a, b)| **a && !**b).count();
        m.put("cells_saved_ha", saved as f64 * cell_ha as f64);
        m.put("cells_extra_ha", extra as f64 * cell_ha as f64);

        // resources
        let engines: Vec<&UnitLog> = self.units.iter().filter(|u| u.kind == Some(UnitKindTag::Engine)).collect();
        let crews: Vec<&UnitLog> = self.units.iter().filter(|u| u.kind == Some(UnitKindTag::Crew)).collect();
        let litres: f64 = engines.iter().map(|u| u.water_used_l as f64).sum();
        let line: f64 = crews.iter().map(|u| u.line_cut_m as f64).sum();
        m.put("litres", litres);
        m.put("line_m", line);
        let saved_ha = (base.ha - out.hectares).max(0.0) as f64;
        m.put("litres_per_ha_saved", if saved_ha > 0.5 { litres / saved_ha } else { f64::NAN });
        m.put("line_m_per_ha_saved", if saved_ha > 0.5 { line / saved_ha } else { f64::NAN });
        m.put("first_water_min", self.first_water_s.map_or(f64::NAN, |s| (s - self.t0) as f64 / 60.0));
        m.put("first_contact_min", self.first_contact_s.map_or(f64::NAN, |s| (s - self.t0) as f64 / 60.0));
        m.put("orders", self.orders as f64);
        m.put("reorders", self.reorders as f64);
        m.put("order_eta_min", mean(&self.order_eta_min.iter().map(|x| *x as f64).collect::<Vec<_>>()));

        // time budgets (share of the run, per unit class)
        let dur = (g.time_s() - self.t0).max(1) as f64;
        for (tag, group) in [("engine", &engines), ("crew", &crews)] {
            if group.is_empty() {
                continue;
            }
            for s in ["staged", "responding", "working", "refilling", "withdrawing", "lost"] {
                let v: f64 = group.iter().map(|u| u.secs_in.get(s).copied().unwrap_or(0.0)).sum::<f64>() / (group.len() as f64 * dur);
                m.put(format!("{tag}_{s}_share"), v);
            }
        }
        // why a unit stood idle or poorly placed, minutes per unit, by note
        let mut notes: BTreeMap<&'static str, f64> = BTreeMap::new();
        for u in self.units.iter().filter(|u| u.kind.is_some()) {
            for (k, v) in &u.notes {
                *notes.entry(k).or_default() += v;
            }
        }
        for (k, v) in notes {
            m.put(format!("note[{}]_min", k.replace(',', ";")), v / 60.0);
        }
        // safety
        let mf = self.units.iter().filter(|u| u.kind == Some(UnitKindTag::Engine)).map(|u| u.min_fire_m).fold(f32::INFINITY, f32::min);
        m.put("engine_min_fire_m", if mf.is_finite() { mf as f64 } else { f64::NAN });
        m.put("max_heat_s", self.units.iter().map(|u| u.max_heat_s as f64).fold(0.0, f64::max));
        m.put("max_threat", self.units.iter().map(|u| u.max_threat as f64).fold(0.0, f64::max));
        m.put("secs_hot_per_unit", self.units.iter().filter(|u| u.kind.is_some()).map(|u| u.secs_hot).sum::<f64>() / self.units.iter().filter(|u| u.kind.is_some()).count().max(1) as f64);
        m.put("withdrawals", self.units.iter().map(|u| u.withdrawals as f64).sum());
        m.put("engine_relocations", engines.iter().map(|u| u.relocations as f64).sum());
        let sec: [f64; 3] = std::array::from_fn(|i| self.units.iter().filter(|u| u.kind.is_some()).map(|u| u.sector_s[i]).sum());
        let tot = sec.iter().sum::<f64>().max(1.0);
        m.put("work_ahead_share", sec[Sector::Ahead as usize] / tot);
        m.put("work_flank_share", sec[Sector::Flank as usize] / tot);
        m.put("work_behind_share", sec[Sector::Behind as usize] / tot);
        for (tag, group) in [("engine", &engines), ("crew", &crews)] {
            let sec: [f64; 3] = std::array::from_fn(|i| group.iter().map(|u| u.sector_s[i]).sum());
            m.put(format!("{tag}_behind_share"), sec[Sector::Behind as usize] / sec.iter().sum::<f64>().max(1.0));
            let w: f64 = group.iter().map(|u| u.secs_working).sum();
            m.put(format!("{tag}_crowded_share"), group.iter().map(|u| u.secs_crowded).sum::<f64>() / w.max(1.0));
        }
        m.put("gap_at_arrival_m", mean(&self.units.iter().filter_map(|u| u.gap_at_arrival_m.map(|x| x as f64)).collect::<Vec<_>>()));

        // did the water and the lines hold?
        let (w_total, w_held, w_failed) = held(g, |i| self.ever_wet[i]);
        m.put("wet_cells", w_total as f64);
        m.put("wet_failed_share", ratio(w_failed, w_total));
        m.put("wet_held_share", ratio(w_held, w_total));
        let cleared = g.fire.cleared().to_vec();
        let (c_total, c_held, _) = held(g, |i| cleared[i]);
        m.put("line_cells", cleared.iter().filter(|c| **c).count() as f64);
        m.put("line_tested", c_total as f64);
        m.put("line_held_share", ratio(c_held, c_total));
        // how much of the water and line went where nothing burned near it at all
        let idle_wet = (0..burnt.len()).filter(|&i| self.ever_wet[i]).count() - w_total;
        m.put("wet_untested_share", ratio(idle_wet, w_total + idle_wet));
        m
    }

    pub fn reach_m(&self) -> f32 {
        self.reach_m
    }

    pub fn metrics_reach(&self) -> f32 {
        self.reach_m
    }

    pub fn curve(&self) -> &[(i64, f32)] {
        &self.curve
    }
}

fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 { f64::NAN } else { a as f64 / b as f64 }
}

fn mean(v: &[f64]) -> f64 {
    let v: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 }
}

/// Among the cells picked by `pick`, those the fire came to test: a neighbour
/// (8-connected) burnt. Of those, `held` never burnt, `failed` did.
fn held(g: &Game, pick: impl Fn(usize) -> bool) -> (usize, usize, usize) {
    let w = g.scn.world;
    let st = g.fire.state();
    let burnt = |r: i64, c: i64| r >= 0 && c >= 0 && r < w.fire_rows as i64 && c < w.fire_cols as i64 && st[r as usize * w.fire_cols + c as usize] != CellFire::Unburnt;
    let (mut total, mut held, mut failed) = (0, 0, 0);
    for r in 0..w.fire_rows {
        for c in 0..w.fire_cols {
            let i = r * w.fire_cols + c;
            if !pick(i) {
                continue;
            }
            let me = st[i] != CellFire::Unburnt;
            let tested = me || (-1..=1).any(|dr| (-1..=1).any(|dc| (dr, dc) != (0, 0) && burnt(r as i64 + dr, c as i64 + dc)));
            if !tested {
                continue;
            }
            total += 1;
            if me {
                failed += 1;
            } else {
                held += 1;
            }
        }
    }
    (total, held, failed)
}

/// The same fire with no orders: the yardstick of every run (same scenario,
/// same seed).
#[derive(Debug, Clone)]
pub struct Baseline {
    pub ha: f32,
    pub homes_hit: usize,
    pub reach_m: f32,
    pub curve: Vec<(i64, f32)>,
    pub burnt: Vec<bool>,
}
