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
    /// Anchors of disconnected hamlets, for map labels.
    pub nuclei: Vec<Pos>,
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
            let homes: Vec<Pos> = households.iter().map(|&i| agents.households[i].home).collect();
            let nuclei = clusters(&homes, 220.0);
            Some(District { name, households, nuclei, centre })
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

/// Connected components of nearby homes. Each anchor is an actual home,
/// nearest its component centre, rather than empty land between hamlets.
fn clusters(homes: &[Pos], gap_m: f32) -> Vec<Pos> {
    let mut seen = vec![false; homes.len()];
    let mut out = Vec::new();
    for start in 0..homes.len() {
        if seen[start] { continue; }
        seen[start] = true;
        let mut group = vec![start];
        let mut cursor = 0;
        while cursor < group.len() {
            let p = homes[group[cursor]];
            for i in 0..homes.len() {
                if !seen[i] && dist(p, homes[i]) <= gap_m {
                    seen[i] = true;
                    group.push(i);
                }
            }
            cursor += 1;
        }
        let n = group.len() as f32;
        let centre = Pos {
            x: group.iter().map(|&i| homes[i].x).sum::<f32>() / n,
            y: group.iter().map(|&i| homes[i].y).sum::<f32>() / n,
        };
        out.push(*group.iter().map(|&i| &homes[i]).min_by(|a, b| dist(**a, centre).total_cmp(&dist(**b, centre))).unwrap());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hamlets_have_anchors_on_homes_and_include_chained_neighbours() {
        let homes = [Pos { x: 0.0, y: 0.0 }, Pos { x: 100.0, y: 0.0 },
            Pos { x: 200.0, y: 0.0 }, Pos { x: 700.0, y: 0.0 }];
        let anchors = clusters(&homes, 110.0);
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].x, 100.0);
        assert_eq!(anchors[1].x, 700.0);
        assert!(clusters(&[], 110.0).is_empty());
    }
}
