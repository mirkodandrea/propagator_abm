//! How the teams are sent: the coordination policies the laboratory compares.
//!
//! A policy plays the part of the player. At the opening pause and at each
//! review it looks at the game (read-only) and says where each team should
//! be, as a point; the runner turns the point into the nearest hex the team
//! can actually reach, exactly as the kiosk does, and skips teams already
//! there. Nothing here touches the simulation.

use anyhow::{bail, Result};
use abm::suppression::UnitState;
use scenario::Pos;

use crate::district::{self, dist};
use crate::game::{Game, PROTECT_M};
use crate::hex::{Axial, HEX_M};

/// The shape of the fire now, from the burning front only.
#[derive(Debug, Clone, Copy)]
pub struct Geo {
    pub head: Pos,
    pub heel: Pos,
    pub left: Pos,
    pub right: Pos,
    pub centre: Pos,
    /// Unit vector the wind blows toward.
    pub wind_to: (f32, f32),
    /// Unit vector across the wind (to the left of the way it blows).
    pub cross: (f32, f32),
    pub n: usize,
    /// Metres per minute the head advanced since the last look (0 at first).
    pub ros: f32,
}

pub fn wind_to(g: &Game) -> (f32, f32) {
    let to = (g.fire.weather().wind_dir_deg as f32 + 180.0).to_radians();
    (to.sin(), to.cos())
}

pub fn geo(g: &Game, ros: f32) -> Option<Geo> {
    let w = g.scn.world;
    let pts: Vec<Pos> = g.fire.active_cells().iter().map(|c| w.centre_of(*c)).collect();
    if pts.is_empty() {
        return None;
    }
    let wt = wind_to(g);
    let cr = (-wt.1, wt.0);
    let proj = |p: &Pos, v: (f32, f32)| p.x * v.0 + p.y * v.1;
    let ext = |v: (f32, f32), max: bool| {
        *pts.iter().max_by(|a, b| if max { proj(a, v).total_cmp(&proj(b, v)) } else { proj(b, v).total_cmp(&proj(a, v)) }).unwrap()
    };
    let n = pts.len() as f32;
    let centre = Pos { x: pts.iter().map(|p| p.x).sum::<f32>() / n, y: pts.iter().map(|p| p.y).sum::<f32>() / n };
    Some(Geo { head: ext(wt, true), heel: ext(wt, false), left: ext(cr, true), right: ext(cr, false), centre, wind_to: wt, cross: cr, n: pts.len(), ros })
}

fn at(p: Pos, v: (f32, f32), d: f32) -> Pos {
    Pos { x: p.x + v.0 * d, y: p.y + v.1 * d }
}

/// The hex team `t` can reach whose road point is nearest `p`.
pub fn reachable_near(g: &Game, t: usize, p: Pos) -> Option<Axial> {
    g.move_targets(t)
        .into_iter()
        .filter_map(|a| g.hexes.get(a).map(|h| (a, dist(h.target, p))))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0)
}

pub fn alive(g: &Game) -> Vec<usize> {
    (0..g.teams.len()).filter(|&t| g.crews.units[g.teams[t].unit].state != UnitState::Lost).collect()
}

pub trait Strategy {
    fn name(&self) -> String;
    /// Minutes between reviews (a pause each time); `None`: only the opening.
    fn review_min(&self) -> Option<i64> {
        None
    }
    /// Where each team should be: `(team, point)`. Teams not named keep their orders.
    fn plan(&mut self, g: &Game, geo: Option<&Geo>) -> Vec<(usize, Pos)>;
}

// --- the simple ones ---------------------------------------------------------------

pub struct Nothing;
impl Strategy for Nothing {
    fn name(&self) -> String {
        "none".into()
    }
    fn plan(&mut self, _: &Game, _: Option<&Geo>) -> Vec<(usize, Pos)> {
        vec![]
    }
}

/// Everyone on the head (the strategy the balance criterion forbids from
/// dominating), optionally spread across the wind.
pub struct Head {
    pub spread_m: f32,
    pub back_m: f32,
    pub review: Option<i64>,
}
impl Strategy for Head {
    fn name(&self) -> String {
        format!("head(spread={:.0},back={:.0}){}", self.spread_m, self.back_m, self.review.map_or(String::new(), |r| format!("@{r}")))
    }
    fn review_min(&self) -> Option<i64> {
        self.review
    }
    fn plan(&mut self, g: &Game, geo: Option<&Geo>) -> Vec<(usize, Pos)> {
        let Some(geo) = geo else { return vec![] };
        let al = alive(g);
        let n = al.len() as f32;
        al.iter()
            .enumerate()
            .map(|(k, &t)| {
                let off = (k as f32 - (n - 1.0) / 2.0) * self.spread_m;
                (t, at(at(geo.head, geo.wind_to, -self.back_m), geo.cross, off))
            })
            .collect()
    }
}

/// Teams ahead of the head by `lead_m`, spread across the wind: the road the
/// fire has not reached yet, so the engine can wet and the people can cut
/// before it comes.
pub struct Lead {
    pub lead_m: f32,
    pub spread_m: f32,
    pub review: Option<i64>,
}
impl Strategy for Lead {
    fn name(&self) -> String {
        format!("lead(lead={:.0},spread={:.0}){}", self.lead_m, self.spread_m, self.review.map_or(String::new(), |r| format!("@{r}")))
    }
    fn review_min(&self) -> Option<i64> {
        self.review
    }
    fn plan(&mut self, g: &Game, geo: Option<&Geo>) -> Vec<(usize, Pos)> {
        let Some(geo) = geo else { return vec![] };
        let al = alive(g);
        let n = al.len() as f32;
        al.iter()
            .enumerate()
            .map(|(k, &t)| {
                let off = (k as f32 - (n - 1.0) / 2.0) * self.spread_m;
                (t, at(at(geo.head, geo.wind_to, self.lead_m), geo.cross, off))
            })
            .collect()
    }
}

/// Anchor and flank: teams alternate along the two flanks a third of the way
/// from the heel to the head, so the fire is pinched in from behind.
pub struct Flanks {
    /// 0 = at the heel, 1 = at the head.
    pub along: f32,
    pub side_m: f32,
    pub review: Option<i64>,
}
impl Strategy for Flanks {
    fn name(&self) -> String {
        format!("flanks(along={:.2},side={:.0}){}", self.along, self.side_m, self.review.map_or(String::new(), |r| format!("@{r}")))
    }
    fn review_min(&self) -> Option<i64> {
        self.review
    }
    fn plan(&mut self, g: &Game, geo: Option<&Geo>) -> Vec<(usize, Pos)> {
        let Some(geo) = geo else { return vec![] };
        let al = alive(g);
        let len = (geo.head.x - geo.heel.x) * geo.wind_to.0 + (geo.head.y - geo.heel.y) * geo.wind_to.1;
        let base = at(geo.heel, geo.wind_to, len * self.along);
        al.iter()
            .enumerate()
            .map(|(k, &t)| {
                let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
                // how far the flank is from the axis here: the extremes of the front
                let half = if sign > 0.0 { dist(geo.left, geo.centre) } else { dist(geo.right, geo.centre) };
                let row = (k / 2) as f32;
                let along = row * HEX_M * 1.5;
                (t, at(at(base, geo.cross, sign * (half + self.side_m)), geo.wind_to, along))
            })
            .collect()
    }
}

/// Everyone to the village the fire will reach first: homes protection.
pub struct Village {
    /// How far toward the fire from the village centre, metres.
    pub toward_m: f32,
    pub review: Option<i64>,
}
impl Strategy for Village {
    fn name(&self) -> String {
        format!("village(toward={:.0}){}", self.toward_m, self.review.map_or(String::new(), |r| format!("@{r}")))
    }
    fn review_min(&self) -> Option<i64> {
        self.review
    }
    fn plan(&mut self, g: &Game, geo: Option<&Geo>) -> Vec<(usize, Pos)> {
        let Some(geo) = geo else { return vec![] };
        let Some(d) = most_threatened(g) else { return vec![] };
        let al = alive(g);
        let dir = {
            let (dx, dy) = (geo.centre.x - g.districts[d].centre.x, geo.centre.y - g.districts[d].centre.y);
            let l = (dx * dx + dy * dy).sqrt().max(1.0);
            (dx / l, dy / l)
        };
        let n = al.len() as f32;
        let cross = (-dir.1, dir.0);
        al.iter()
            .enumerate()
            .map(|(k, &t)| {
                let off = (k as f32 - (n - 1.0) / 2.0) * HEX_M * 1.2;
                (t, at(at(g.districts[d].centre, dir, self.toward_m), cross, off))
            })
            .collect()
    }
}

/// The district the front reaches soonest (distance shortened where the wind
/// drives the fire at it).
pub fn most_threatened(g: &Game) -> Option<usize> {
    (0..g.districts.len())
        .filter_map(|d| district::exposure(&g.districts[d], &g.fire, &g.scn).map(|e| (d, district::reach_m(e.distance_m, e.downwind))))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0)
}

/// Half the teams lead the head, half guard the village.
pub struct Split {
    pub lead_m: f32,
    pub review: Option<i64>,
}
impl Strategy for Split {
    fn name(&self) -> String {
        format!("split(lead={:.0}){}", self.lead_m, self.review.map_or(String::new(), |r| format!("@{r}")))
    }
    fn review_min(&self) -> Option<i64> {
        self.review
    }
    fn plan(&mut self, g: &Game, geo: Option<&Geo>) -> Vec<(usize, Pos)> {
        let al = alive(g);
        let half = al.len().div_ceil(2);
        let mut lead = Lead { lead_m: self.lead_m, spread_m: HEX_M * 1.5, review: None };
        let mut vil = Village { toward_m: 150.0, review: None };
        let head_teams: Vec<usize> = al[..half].to_vec();
        let mut out = vec![];
        let l = lead.plan(g, geo);
        for (i, &t) in head_teams.iter().enumerate() {
            if let Some(p) = l.get(i) {
                out.push((t, p.1));
            }
        }
        let v = vil.plan(g, geo);
        for (i, &t) in al[half..].iter().enumerate() {
            if let Some(p) = v.get(i) {
                out.push((t, p.1));
            }
        }
        out
    }
}

// --- the planner ---------------------------------------------------------------------

/// The scored, coordinated planner: every review, each reachable hex is given
/// a value (homes it protects, threatened fuel in the engine's reach, being in
/// the head's way) minus a cost (driving time, danger), and teams take the
/// best hexes greedily, never within `min_sep` hexes of each other.
#[derive(Clone)]
pub struct Planner {
    pub review: i64,
    pub w_homes: f32,
    pub w_fuel: f32,
    pub w_block: f32,
    pub w_eta: f32,
    pub min_sep: i32,
    /// Margin to the fire where the hex is not downwind of it, metres.
    pub safe_m: f32,
    /// Minutes of warning the hex must have against the head's advance.
    pub warn_min: f32,
    /// Keep the current hex when its score is at least this share of the best.
    pub stay: f32,
    /// Homes closer than this (effective, wind-shortened) to the fire count as at risk.
    pub risk_m: f32,
    last: Vec<Option<Axial>>,
}

impl Default for Planner {
    fn default() -> Planner {
        Planner { review: 10, w_homes: 0.2, w_fuel: 0.02, w_block: 12.0, w_eta: 0.08, min_sep: 3, safe_m: 120.0, warn_min: 8.0, stay: 0.8, risk_m: 1000.0, last: vec![] }
    }
}

impl Planner {
    fn score(&self, g: &Game, geo: &Geo, at_hex: Axial, eta: f32, active: &[Pos], at_risk: &[Pos]) -> Option<f32> {
        let h = g.hexes.get(at_hex)?;
        let p = h.target;
        let nearest = active.iter().map(|f| dist(*f, p)).fold(f32::INFINITY, f32::min);
        let along = (p.x - geo.head.x) * geo.wind_to.0 + (p.y - geo.head.y) * geo.wind_to.1; // metres ahead of the head
        let cross = ((p.x - geo.head.x) * geo.cross.0 + (p.y - geo.head.y) * geo.cross.1).abs();
        let downwind_of_fire = along > -300.0;
        // a hex the front will reach before the team can work is a trap
        let closing = if downwind_of_fire { geo.ros.max(5.0) * (eta + self.warn_min) } else { 0.0 };
        if nearest < self.safe_m || (downwind_of_fire && nearest - closing < self.safe_m * 0.6) || g.fire.threat().at(p) >= 0.1 {
            return None;
        }
        let homes = at_risk.iter().filter(|q| dist(**q, p) <= PROTECT_M).count() as f32;
        let world = g.scn.world;
        let fuel: f32 = fire::cells_in_radius(&world, p, g.tuning.engine_range_m)
            .into_iter()
            .filter(|c| g.fire.is_suppressible(*c, &g.scn))
            .map(|c| g.fire.threat().at_cell(c))
            .sum();
        // in the head's way: ahead of it by 100-500 m and near its axis
        let block = if (100.0..=500.0).contains(&along) && cross < 350.0 { 1.0 } else { 0.0 };
        Some(self.w_homes * homes + self.w_fuel * fuel + self.w_block * block - self.w_eta * eta)
    }
}

impl Strategy for Planner {
    fn name(&self) -> String {
        format!("planner(rev={},homes={},fuel={},block={},eta={},sep={},safe={:.0},warn={:.0},stay={})", self.review, self.w_homes, self.w_fuel, self.w_block, self.w_eta, self.min_sep, self.safe_m, self.warn_min, self.stay)
    }
    fn review_min(&self) -> Option<i64> {
        Some(self.review)
    }
    fn plan(&mut self, g: &Game, geo: Option<&Geo>) -> Vec<(usize, Pos)> {
        let Some(geo) = geo else { return vec![] };
        if self.last.len() < g.teams.len() {
            self.last.resize(g.teams.len(), None);
        }
        let world = g.scn.world;
        let active: Vec<Pos> = g.fire.active_cells().iter().map(|c| world.centre_of(*c)).collect();
        let at_risk: Vec<Pos> = g
            .districts
            .iter()
            .flat_map(|d| d.households.iter().map(|&i| Pos::from(g.scn.population.households[i].pos)))
            .filter(|h| {
                let r = active
                    .iter()
                    .map(|p| {
                        let dd = dist(*p, *h).max(1.0);
                        district::reach_m(dd, ((h.x - p.x) * geo.wind_to.0 + (h.y - p.y) * geo.wind_to.1) / dd)
                    })
                    .fold(f32::INFINITY, f32::min);
                r < self.risk_m
            })
            .collect();
        let al = alive(g);
        // every (team, hex) pair with its score; the team's own reachable set
        let mut pairs: Vec<(f32, usize, Axial)> = vec![];
        for &t in &al {
            for a in g.move_targets(t) {
                if let Some(h) = g.hexes.get(a) {
                    let eta = g.plan_move(t, a).map_or(f32::INFINITY, |m| m.minutes);
                    if !eta.is_finite() {
                        continue;
                    }
                    if let Some(s) = self.score(g, geo, a, eta, &active, &at_risk) {
                        if s > 0.0 && dist(h.target, geo.head) < 2500.0 {
                            pairs.push((s, t, a));
                        }
                    }
                }
            }
        }
        pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
        // the best available score of each team, to judge staying put
        let best_of = |t: usize| pairs.iter().find(|p| p.1 == t).map_or(0.0, |p| p.0);
        let mut taken: Vec<Axial> = vec![];
        let mut done: Vec<usize> = vec![];
        let mut out = vec![];
        // teams that hold a good hex keep it
        for &t in &al {
            if let Some(cur) = self.last[t] {
                let mine = pairs.iter().find(|p| p.1 == t && p.2 == cur).map(|p| p.0);
                if let Some(s) = mine {
                    if s >= self.stay * best_of(t) && taken.iter().all(|a| a.distance(cur) >= self.min_sep) {
                        taken.push(cur);
                        done.push(t);
                    }
                }
            }
        }
        for &(_, t, a) in &pairs {
            if done.contains(&t) || taken.iter().any(|x| x.distance(a) < self.min_sep) {
                continue;
            }
            taken.push(a);
            done.push(t);
            self.last[t] = Some(a);
            if let Some(h) = g.hexes.get(a) {
                out.push((t, h.target));
            }
        }
        out
    }
}

// --- parsing ---------------------------------------------------------------------------

/// `name` or `name:k=v,k=v`: `none`, `head`, `lead`, `flanks`, `village`,
/// `split`, `planner`.
pub fn parse(s: &str) -> Result<Box<dyn Strategy>> {
    let (name, args) = s.split_once(':').unwrap_or((s, ""));
    let kv: Vec<(&str, f32)> = args
        .split(',')
        .filter(|x| !x.is_empty())
        .map(|x| {
            let (k, v) = x.split_once('=').unwrap_or((x, "0"));
            (k, v.parse::<f32>().unwrap_or(0.0))
        })
        .collect();
    let get = |k: &str, d: f32| kv.iter().find(|(n, _)| *n == k).map_or(d, |x| x.1);
    let review = kv.iter().find(|(n, _)| *n == "rev").map(|x| x.1 as i64);
    Ok(match name {
        "none" => Box::new(Nothing),
        "head" => Box::new(Head { spread_m: get("spread", 0.0), back_m: get("back", 0.0), review }),
        "lead" => Box::new(Lead { lead_m: get("lead", 400.0), spread_m: get("spread", 225.0), review }),
        "flanks" => Box::new(Flanks { along: get("along", 0.35), side_m: get("side", 60.0), review }),
        "village" => Box::new(Village { toward_m: get("toward", 150.0), review }),
        "split" => Box::new(Split { lead_m: get("lead", 400.0), review }),
        "planner" => {
            let d = Planner::default();
            Box::new(Planner {
                review: review.unwrap_or(d.review),
                w_homes: get("homes", d.w_homes),
                w_fuel: get("fuel", d.w_fuel),
                w_block: get("block", d.w_block),
                w_eta: get("eta", d.w_eta),
                min_sep: get("sep", d.min_sep as f32) as i32,
                safe_m: get("safe", d.safe_m),
                warn_min: get("warn", d.warn_min),
                stay: get("stay", d.stay),
                risk_m: get("risk", d.risk_m),
                last: vec![],
            })
        }
        x => bail!("strategia sconosciuta: {x} (none, head, lead, flanks, village, split, planner)"),
    })
}
