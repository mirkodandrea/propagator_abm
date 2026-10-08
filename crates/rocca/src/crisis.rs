//! When the game asks the player to decide (01-SPEC-GIOCO §3).
//!
//! A crisis is proposed only from what is **known now** — where the front is,
//! how fast it has been closing, where the units are and how long they would
//! take — never from the simulation's future. It fires when:
//!
//! - a district is threatened and the plan leaves it uncovered, and a unit
//!   could still get there before the fire does (**scoperto**);
//! - the weather forecast announces a wind change that would drive the
//!   present front at a district (**previsione**): a forecast is known
//!   information, the fire's future is not;
//! - the wind has turned and now drives the fire at a district (**vento**);
//! - a unit has been lost (**mezzo perso**).
//!
//! Each crisis fires once, at most [`MAX_CRISES`] per game and at least
//! [`MIN_GAP_S`] apart. The message states risk, estimated time and the
//! trade-off; it never says what to choose.

use abm::suppression::UnitState;
use scenario::Pos;

use crate::coordinator::{self, Post, View};
use crate::district;
use crate::plan::Plan;
use crate::words;

pub const MAX_CRISES: usize = 2;
pub const MIN_GAP_S: i64 = 15 * 60;
/// How far ahead the forecast announces a wind change.
pub const FORECAST_LEAD_S: i64 = 20 * 60;
/// No crisis for a fire further than this from the district's homes.
pub const CRISIS_M: f32 = 2000.0;
/// An uncovered district is a crisis only if the fire could be on it within
/// this long: beyond it, it is not yet a decision under time pressure.
pub const URGENT_S: f32 = 45.0 * 60.0;
/// No crisis this close to the end of a game: nothing could still change.
pub const LAST_CALL_S: i64 = 10 * 60;
/// Approach speed assumed until two observations exist, m/s (~1 km/h).
const DEFAULT_APPROACH: f32 = 0.3;
/// Window over which the approach speed is measured, simulated seconds.
const APPROACH_WINDOW_S: i64 = 10 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Scoperto { district: usize },
    Previsione { district: usize },
    Vento { district: usize },
    MezzoPerso { unit: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Crisis {
    pub at_s: i64,
    pub kind: Kind,
    /// Risk, estimated time and trade-off, in Italian.
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Detector {
    raised: Vec<Kind>,
    last_s: Option<i64>,
    /// Per district: (time, distance of the front from the nearest home).
    seen: Vec<Vec<(i64, f32)>>,
    wind_from: Option<f64>,
    lost_seen: Vec<bool>,
    /// Districts the player left uncovered knowingly: already threatened and
    /// uncovered when the plan was last committed. Not news, not a crisis.
    acknowledged: Vec<usize>,
}

fn minutes(s: f32) -> String {
    format!("{:.0}", (s / 60.0).max(1.0).ceil())
}

impl Detector {
    /// How fast the front has been closing on district `d`, m/s, over the
    /// last [`APPROACH_WINDOW_S`].
    fn approach(&self, d: usize, now: i64, distance: f32) -> f32 {
        let past = self.seen.get(d).and_then(|h| h.iter().find(|(t, _)| now - *t <= APPROACH_WINDOW_S));
        match past {
            Some(&(t, dd)) if now > t => ((dd - distance) / (now - t) as f32).max(0.0),
            _ => DEFAULT_APPROACH,
        }
    }

    /// Look at the state now; return a crisis if one is due. Call at each
    /// coordinator review, after the active plan's posts are known.
    /// `forecast`: a wind change the weather service has announced, as
    /// (when, wind from, degrees).
    pub fn check(&mut self, v: &View, active: &Plan, posts: &[Option<Post>], now: i64, forecast: Option<(i64, f64)>) -> Option<Crisis> {
        let n = v.districts.len();
        if self.seen.len() != n {
            self.seen = vec![vec![]; n];
        }
        let wind = v.fire.weather().wind_dir_deg;
        let turned = self.wind_from.is_some_and(|w| (w - wind).abs() > 1.0);
        self.wind_from = Some(wind);

        let mut candidates: Vec<Crisis> = vec![];
        // what is known about each district now
        let mut known = vec![];
        for d in 0..n {
            let e = district::exposure(&v.districts[d], v.agents, v.fire, v.scn);
            if let Some(e) = e {
                let speed = self.approach(d, now, e.distance_m);
                self.seen[d].push((now, e.distance_m));
                self.seen[d].retain(|(t, _)| now - *t <= APPROACH_WINDOW_S);
                known.push(Some((e, speed)));
            } else {
                known.push(None);
            }
        }

        // units lost
        if self.lost_seen.len() != v.crews.units.len() {
            self.lost_seen = vec![false; v.crews.units.len()];
        }
        for (k, u) in v.crews.units.iter().enumerate() {
            if u.state == UnitState::Lost && !self.lost_seen[k] {
                self.lost_seen[k] = true;
                candidates.push(Crisis {
                    at_s: now,
                    kind: Kind::MezzoPerso { unit: k },
                    text: format!("{} è stato raggiunto dal fuoco ed è fuori servizio. Restano {} mezzi per gli stessi luoghi.", u.callsign,
                        v.crews.units.iter().filter(|x| x.state != UnitState::Lost && !x.kind.is_air()).count()),
                });
            }
        }

        // The forecast: which district the present front would be driven at.
        if let Some((at, from)) = forecast {
            if now < at && at - now <= FORECAST_LEAD_S {
                let to = (from as f32 + 180.0).to_radians();
                let (ux, uy) = (to.sin(), to.cos());
                let target = (0..n)
                    .filter_map(|d| known[d].map(|(e, _)| (d, e)))
                    .map(|(d, e)| {
                        let (dx, dy) = (v.districts[d].centre.x - e.fire_at.x, v.districts[d].centre.y - e.fire_at.y);
                        let len = (dx * dx + dy * dy).sqrt().max(1.0);
                        (d, e, (dx * ux + dy * uy) / len)
                    })
                    .filter(|(_, e, cos)| *cos > coordinator::DOWNWIND_COS && e.distance_m <= 2.0 * CRISIS_M)
                    .min_by(|a, b| a.1.distance_m.total_cmp(&b.1.distance_m));
                // a district the plan already covers is news, not a decision
                let target = target.filter(|(d, _, _)| !posts.iter().flatten().any(|p| p.district == *d));
                if let Some((d, e, _)) = target {
                    let name = &v.districts[d].name;
                    let units = 0;
                    candidates.push(Crisis {
                        at_s: now,
                        kind: Kind::Previsione { district: d },
                        text: format!(
                            "Previsione meteo: tra circa {} min il vento girerà e soffierà da {}. Spingerebbe il fuoco verso {name}, ora a {}. Oggi {name} ha {units} mezzi.",
                            (at - now) / 60,
                            words::compass(from),
                            words::km(e.distance_m)
                        ),
                    });
                }
            }
        }

        for d in 0..n {
            let Some((e, speed)) = known[d] else { continue };
            if e.distance_m > CRISIS_M {
                continue;
            }
            let name = &v.districts[d].name;
            let arrive_s = e.distance_m / speed.max(0.05);
            let urgent = arrive_s <= URGENT_S;
            let covered = posts.iter().flatten().any(|p| p.district == d);
            if turned && e.downwind > coordinator::DOWNWIND_COS && !covered {
                candidates.push(Crisis {
                    at_s: now,
                    kind: Kind::Vento { district: d },
                    text: format!(
                        "Il vento è girato: ora spinge il fuoco verso {name}, a {}. Nel piano attuale {name} non ha mezzi.",
                        words::km(e.distance_m),
                    ),
                });
            }
            if covered || !urgent || self.acknowledged.contains(&d) {
                continue;
            }
            // Is there an answer? The quickest unit that could take a post here.
            let best = (0..v.crews.units.len())
                .filter(|&k| {
                    let u = &v.crews.units[k];
                    !u.kind.is_air() && !matches!(u.state, UnitState::Lost | UnitState::Withdrawing)
                })
                .filter_map(|k| coordinator::eta_to_district(v, k, d).map(|eta| (k, eta)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let Some((k, eta)) = best else { continue };
            if eta >= arrive_s {
                continue; // too late for anyone: not a decision any more
            }
            let u = &v.crews.units[k];
            let cost = match posts.get(k).and_then(|p| p.as_ref()) {
                Some(p) => format!("e lascia {} con un mezzo in meno (priorità {})", v.districts[p.district].name, active.rank(p.district).map_or(0, |r| r + 1)),
                None => "ed è libero ora".to_string(),
            };
            candidates.push(Crisis {
                at_s: now,
                kind: Kind::Scoperto { district: d },
                text: format!(
                    "{name} è minacciato: fuoco a {}, potrebbe arrivare in circa {} min. Spostare {} richiede circa {} min {cost}.",
                    words::km(e.distance_m),
                    minutes(arrive_s),
                    u.callsign,
                    minutes(eta)
                ),
            });
        }

        if self.raised.len() >= MAX_CRISES || self.last_s.is_some_and(|t| now - t < MIN_GAP_S) {
            return None;
        }
        let c = candidates.into_iter().find(|c| !self.raised.contains(&c.kind))?;
        self.raised.push(c.kind);
        self.last_s = Some(now);
        Some(c)
    }

    /// The player has just committed a plan: what is threatened and left
    /// uncovered now was their choice.
    pub fn acknowledge(&mut self, v: &View, posts: &[Option<Post>]) {
        self.acknowledged = (0..v.districts.len())
            .filter(|&d| !posts.iter().flatten().any(|p| p.district == d))
            .filter(|&d| district::exposure(&v.districts[d], v.agents, v.fire, v.scn).is_some_and(|e| e.distance_m <= CRISIS_M))
            .collect();
    }

    pub fn raised(&self) -> usize {
        self.raised.len()
    }
}

/// Where a crisis is, for the camera and the map marker.
pub fn place(v: &View, c: &Crisis) -> Option<Pos> {
    match c.kind {
        Kind::Scoperto { district } | Kind::Previsione { district } | Kind::Vento { district } => Some(v.districts[district].centre),
        Kind::MezzoPerso { unit } => Some(v.crews.units[unit].pos),
    }
}
