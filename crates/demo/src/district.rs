//! The districts of a demo town: the unit the commander warns, defends and is
//! judged by (`docs/demo-spec-gameplay.md` §2).
//!
//! A district is data, not geometry the game invents: it is the `locality` the
//! generator gave each household (`scripts/generate_demo_scenarios.py`), in the
//! order the scenario lists its localities. Each town has three, at different
//! bearings from the fire, so *which* one the wind threatens is the decision.
//!
//! What a district knows about the fire is measured from the fire's own state,
//! one pass per step, and is the same in the live game and the headless twin
//! (both step a [`crate::Referee`]).

use abm::Abm;
use fire::FireSim;
use scenario::{Pos, Scenario};

/// Fire closer than this to a district's nearest home: the district is
/// *threatened* (the HUD turns amber). The cry-wolf radius, so "threatened" and
/// "the warning was justified" are one rule. Measured on the demo towns
/// (`tests/district_probe.rs`): at 800 m every district of every town was
/// "threatened" at T+0; at 300 m the upwind district of each town never is,
/// and the downwind ones are by T+15.
pub const THREATENED_M: f32 = crate::trust::NEEDLESS_RADIUS_M;
/// Fire closer than this: worth watching (the HUD shows the distance).
pub const WATCH_M: f32 = 1500.0;

#[derive(Debug, Clone)]
pub struct District {
    pub name: String,
    /// Indices into `Abm::households`.
    pub households: Vec<usize>,
    /// Centroid of the homes.
    pub centre: Pos,
    /// Distance from the centre to the furthest home.
    pub radius_m: f32,
    /// A handful of homes spread over the district, used to measure how far
    /// the fire is (every home would cost a few hundred times more per step).
    probes: Vec<Pos>,
}

/// How a district stands, for the map chip and the end card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// No fire within [`WATCH_M`].
    Calm,
    /// Fire within [`WATCH_M`].
    Watch,
    /// Fire within [`THREATENED_M`].
    Threatened,
    /// The fire is at the homes: the threat field is alarming at one of them.
    Reached,
}

/// One district's story so far. Everything the end card says about it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Report {
    pub households: usize,
    /// When the commander first warned it (all of it or by name).
    pub warned_at_s: Option<i64>,
    /// When fire first came within [`THREATENED_M`] of a home.
    pub threatened_at_s: Option<i64>,
    /// When the fire first reached a home (threat alarming there) or caught a
    /// family of this district.
    pub reached_at_s: Option<i64>,
    /// When an engine first started defending homes here.
    pub defended_at_s: Option<i64>,
    /// Nearest the fire has come, metres (∞ until it burns anything).
    pub closest_m: f32,
    /// Distance from the burning edge to the nearest home now, metres.
    pub fire_now_m: f32,
    /// Households the fire caught at home.
    pub caught: usize,
    /// Households at a refuge or out of the window.
    pub safe: usize,
    /// Households on the road.
    pub moving: usize,
}

impl Report {
    pub fn level(&self) -> Level {
        if self.reached_at_s.is_some() {
            Level::Reached
        } else if self.fire_now_m <= THREATENED_M {
            Level::Threatened
        } else if self.fire_now_m <= WATCH_M {
            Level::Watch
        } else {
            Level::Calm
        }
    }

    /// Warned, and the fire never came within [`THREATENED_M`]: a false alarm.
    pub fn needless(&self) -> bool {
        self.warned_at_s.is_some() && self.threatened_at_s.is_none()
    }

    /// Minutes between the warning and the fire reaching the homes; negative if
    /// the warning came after. `None` unless both happened.
    pub fn lead_min(&self) -> Option<i64> {
        Some((self.reached_at_s? - self.warned_at_s?) / 60)
    }
}

/// The town's districts, in the order the scenario lists its localities.
/// Households with no locality (none in the demo towns) are left out.
pub fn of(scn: &Scenario, agents: &Abm) -> Vec<District> {
    let pop = &scn.population.households;
    debug_assert_eq!(pop.len(), agents.households.len());
    let mut names: Vec<String> = scn.metadata.localities.clone();
    for h in pop {
        if let Some(l) = &h.locality {
            if !names.contains(l) {
                names.push(l.clone());
            }
        }
    }
    names
        .into_iter()
        .filter_map(|name| {
            let households: Vec<usize> = pop
                .iter()
                .enumerate()
                .filter(|(_, h)| h.locality.as_deref() == Some(name.as_str()))
                .map(|(i, _)| i)
                .collect();
            if households.is_empty() {
                return None;
            }
            let homes: Vec<Pos> = households.iter().map(|&i| agents.households[i].home).collect();
            let n = homes.len() as f32;
            let centre = Pos { x: homes.iter().map(|p| p.x).sum::<f32>() / n, y: homes.iter().map(|p| p.y).sum::<f32>() / n };
            let radius_m = homes.iter().map(|p| dist(*p, centre)).fold(0.0, f32::max);
            // Probes: the homes furthest out in eight directions, and the centre.
            let mut probes = vec![centre];
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let (ux, uy) = (a.cos(), a.sin());
                if let Some(p) = homes.iter().max_by(|a, b| {
                    let pa = (a.x - centre.x) * ux + (a.y - centre.y) * uy;
                    let pb = (b.x - centre.x) * ux + (b.y - centre.y) * uy;
                    pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    probes.push(*p);
                }
            }
            Some(District { name, households, centre, radius_m, probes })
        })
        .collect()
}

fn dist(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

impl District {
    /// Distance from the nearest of `cells` (world positions) to the district,
    /// metres. Measured to the probes, so it can over-read by up to the gap
    /// between two probes on the far side; on the side facing a fire the
    /// probe is the outermost home, which is the one that matters.
    pub fn distance_to(&self, cells: &[Pos]) -> f32 {
        let mut best = f32::INFINITY;
        for c in cells {
            for p in &self.probes {
                best = best.min(dist(*c, *p));
            }
        }
        best
    }

    /// Where the next engine sent to defend this district is posted: the home
    /// nearest `from` (the fire) that is at least `spacing` from every post
    /// already taken, so three engines spread along the edge facing the fire
    /// instead of queueing at one door. Falls back to the nearest home.
    pub fn post_facing(&self, agents: &Abm, from: Pos, taken: &[Pos], spacing: f32) -> Pos {
        let mut homes: Vec<Pos> = self.households.iter().map(|&i| agents.households[i].home).collect();
        homes.sort_by(|a, b| dist(*a, from).partial_cmp(&dist(*b, from)).unwrap_or(std::cmp::Ordering::Equal));
        homes
            .iter()
            .copied()
            .find(|p| taken.iter().all(|t| dist(*p, *t) >= spacing))
            .or_else(|| homes.first().copied())
            .unwrap_or(self.centre)
    }
}

/// Step every district's report: call once after each model step.
pub fn note(districts: &[District], reports: &mut [Report], tally: &crate::Tally, agents: &Abm, fire: &FireSim, scn: &Scenario) {
    use scenario::population::Status;
    let now = fire.time_s();
    let active: Vec<Pos> = fire.active_cells().iter().map(|c| scn.world.centre_of(*c)).collect();
    let threat = fire.threat();
    for (d, r) in districts.iter().zip(reports.iter_mut()) {
        r.households = d.households.len();
        r.fire_now_m = d.distance_to(&active);
        r.closest_m = r.closest_m.min(r.fire_now_m);
        if r.threatened_at_s.is_none() && r.closest_m <= THREATENED_M {
            r.threatened_at_s = Some(now);
        }
        // Reached: the threat is alarming at a home, or the fire has caught a
        // family here by any of the tally's routes (trapped on the road, a
        // house lit by embers). One rule with "caught", or the end card can
        // say a district was spared while counting families caught in it.
        if r.reached_at_s.is_none()
            && d.households.iter().any(|&i| tally.caught_at(i).is_some() || threat.at(agents.households[i].home) >= fire::threat::ALARMING)
        {
            r.reached_at_s = Some(now);
        }
        let (mut caught, mut safe, mut moving) = (0, 0, 0);
        for &i in &d.households {
            caught += tally.caught_at(i).is_some() as usize;
            match agents.households[i].status {
                Status::Evacuated => safe += 1,
                Status::Evacuating => moving += 1,
                _ => {}
            }
        }
        (r.caught, r.safe, r.moving) = (caught, safe, moving);
    }
}

/// Fresh reports, one per district, before anything has happened.
pub fn reports(districts: &[District]) -> Vec<Report> {
    districts
        .iter()
        .map(|d| Report { households: d.households.len(), closest_m: f32::INFINITY, fire_now_m: f32::INFINITY, ..Report::default() })
        .collect()
}

/// A warning this many minutes before the fire reached a district counts as
/// "in time" on the end card: about the preparation the median family needs.
pub const IN_TIME_MIN: i64 = 10;

/// The end card's three medals. Facts about the session, not a score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Badges {
    /// Every district the fire reached had been warned at least
    /// [`IN_TIME_MIN`] minutes before -- and at least one was reached.
    pub in_time: bool,
    /// No district was warned that the fire never came near.
    pub no_false_alarm: bool,
    /// An engine was defending a district before the fire reached it.
    pub homes_defended: bool,
}

pub fn badges(reports: &[Report]) -> Badges {
    let reached: Vec<&Report> = reports.iter().filter(|r| r.reached_at_s.is_some()).collect();
    Badges {
        in_time: !reached.is_empty() && reached.iter().all(|r| r.lead_min().is_some_and(|m| m >= IN_TIME_MIN)),
        no_false_alarm: reports.iter().any(|r| r.warned_at_s.is_some()) && !reports.iter().any(|r| r.needless()),
        homes_defended: reached.iter().any(|r| matches!((r.defended_at_s, r.reached_at_s), (Some(d), Some(a)) if d <= a)),
    }
}
