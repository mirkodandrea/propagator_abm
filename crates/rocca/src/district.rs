//! The places the player ranks: the localities the Scenario Factory gave
//! each household, in the order the scenario lists them.

use abm::Abm;
use fire::FireSim;
use scenario::{Pos, Scenario};

#[derive(Debug, Clone)]
pub struct District {
    pub name: String,
    /// Indices into `Abm::households`.
    pub households: Vec<usize>,
    /// Centroid of the homes.
    pub centre: Pos,
}

pub fn of(scn: &Scenario, agents: &Abm) -> Vec<District> {
    let pop = &scn.population.households;
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
            let households: Vec<usize> =
                pop.iter().enumerate().filter(|(_, h)| h.locality.as_deref() == Some(name.as_str())).map(|(i, _)| i).collect();
            if households.is_empty() {
                return None;
            }
            let n = households.len() as f32;
            let centre = Pos {
                x: households.iter().map(|&i| agents.households[i].home.x).sum::<f32>() / n,
                y: households.iter().map(|&i| agents.households[i].home.y).sum::<f32>() / n,
            };
            Some(District { name, households, centre })
        })
        .collect()
}

pub fn dist(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// What is known now about the fire near a district, from the burning front
/// only (cells behind it threaten nobody) and never from the fire's future.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exposure {
    /// The burning cell nearest to any home of the district.
    pub fire_at: Pos,
    pub distance_m: f32,
    /// Cosine between the wind's heading and the fire -> district direction:
    /// 1 when the wind blows the fire straight at the homes.
    pub downwind: f32,
}

/// How soon the front can reach a home, as an effective distance: straight
/// distance shortened up to threefold where the wind blows the front at it.
/// A ranking, not a forecast: the coordinator only needs which homes first.
pub fn reach_m(d: f32, cos_downwind: f32) -> f32 {
    d / (1.0 + 2.0 * cos_downwind.max(0.0))
}

fn wind_to(fire: &FireSim) -> (f32, f32) {
    let to = (fire.weather().wind_dir_deg as f32 + 180.0).to_radians();
    (to.sin(), to.cos())
}

pub fn exposure(d: &District, agents: &Abm, fire: &FireSim, scn: &Scenario) -> Option<Exposure> {
    let w = &scn.world;
    let mut best: Option<(f32, Pos)> = None;
    for c in fire.active_cells() {
        let p = w.centre_of(*c);
        for &i in &d.households {
            let dd = dist(p, agents.households[i].home);
            if best.is_none_or(|b| dd < b.0) {
                best = Some((dd, p));
            }
        }
    }
    let (distance_m, fire_at) = best?;
    let (ux, uy) = wind_to(fire);
    let (dx, dy) = (d.centre.x - fire_at.x, d.centre.y - fire_at.y);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    Some(Exposure { fire_at, distance_m, downwind: (dx * ux + dy * uy) / len })
}

/// Each home of the district with its [`reach_m`] from the burning front,
/// soonest first. Empty when nothing is burning.
pub fn homes_by_threat(d: &District, agents: &Abm, fire: &FireSim, scn: &Scenario) -> Vec<(Pos, f32)> {
    let w = &scn.world;
    let (ux, uy) = wind_to(fire);
    let cells: Vec<Pos> = fire.active_cells().iter().map(|c| w.centre_of(*c)).collect();
    if cells.is_empty() {
        return vec![];
    }
    let mut out: Vec<(Pos, f32)> = d
        .households
        .iter()
        .map(|&i| {
            let h = agents.households[i].home;
            let r = cells
                .iter()
                .map(|p| {
                    let dd = dist(*p, h).max(1.0);
                    reach_m(dd, ((h.x - p.x) * ux + (h.y - p.y) * uy) / dd)
                })
                .fold(f32::INFINITY, f32::min);
            (h, r)
        })
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out.dedup_by(|a, b| dist(a.0, b.0) < 1.0);
    out
}
