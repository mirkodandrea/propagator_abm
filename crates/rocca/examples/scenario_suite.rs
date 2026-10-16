//! Headless scenario probe. JSON request on stdin, JSON result on stdout.
//! Weather histories are experimental inputs; no kiosk data is changed.
use anyhow::{ensure, Context, Result};
use fire::Weather;
use rocca::{district, Case, Game, Territory, STEP_S};
use serde::Deserialize;
use serde_json::json;
use std::{
    io::{self, Read},
    path::Path,
};

#[derive(Deserialize)]
struct WeatherPoint {
    at_min: i64,
    wind_from_deg: f64,
    wind_kmh: f64,
    moisture_pct: f64,
}
impl WeatherPoint {
    fn weather(&self) -> Weather {
        Weather {
            wind_dir_deg: self.wind_from_deg,
            wind_speed_kmh: self.wind_kmh,
            moisture_pct: self.moisture_pct,
        }
    }
}
#[derive(Deserialize)]
struct Request {
    case: Case,
    history: Vec<WeatherPoint>,
    seed: u64,
    policy: String,
    #[serde(default)]
    native_case: bool,
    #[serde(default)]
    reaction_delay_min: i64,
    #[serde(default)]
    planner: Option<String>,
}
/// Uses observed flames, visible housing clusters and the announced bulletin.
/// No future fire state or hidden weather is inspected.
fn nucleus_targets(g: &Game) -> (Vec<(usize, f32)>, Vec<rocca::Axial>) {
    let flames: Vec<_> = g
        .fire
        .active_cells()
        .iter()
        .map(|c| g.scn.world.centre_of(*c))
        .collect();
    if flames.is_empty() {
        return (vec![], vec![]);
    }
    let now = g.weather();
    let later = g.weather_ahead();
    let mut groups = vec![];
    let mut ranks = vec![];
    for (d, borough) in g.districts.iter().enumerate() {
        let mut best = f32::INFINITY;
        for &p in &borough.nuclei {
            let front = *flames
                .iter()
                .min_by(|a, b| district::dist(**a, p).total_cmp(&district::dist(**b, p)))
                .unwrap();
            let dx = p.x - front.x;
            let dy = p.y - front.y;
            let distance = (dx * dx + dy * dy).sqrt().max(1.0);
            let reach = |w: Weather| {
                let a = (w.wind_dir_deg as f32 + 180.0).to_radians();
                district::reach_m(distance, (dx * a.sin() + dy * a.cos()) / distance)
            };
            let mut priority = reach(now);
            if let Some((at, w)) = later {
                priority = priority.min(reach(w) + (at - g.time_s()) as f32 / 60.0 * 2.0);
            }
            best = best.min(priority);
            groups.push((priority, p, front));
        }
        ranks.push((d, best));
    }
    ranks.sort_by(|a, b| a.1.total_cmp(&b.1));
    groups.sort_by(|a, b| a.0.total_cmp(&b.0));
    let cutoff = groups
        .first()
        .map_or(1000.0, |p| (p.0 * 2.5).clamp(1000.0, 1800.0));
    let mut targets = vec![];
    for (priority, p, front) in groups {
        if priority > cutoff || targets.len() >= g.teams.len() {
            break;
        }
        let length = district::dist(p, front).max(1.0);
        let rear = scenario::Pos {
            x: p.x + 100.0 * (p.x - front.x) / length,
            y: p.y + 100.0 * (p.y - front.y) / length,
        };
        if let Some(h) = g.hexes.nearest(rear) {
            if !targets.contains(&h.at) {
                targets.push(h.at);
            }
        }
    }
    (ranks, targets)
}

fn main() -> Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let r: Request = serde_json::from_str(&input)?;
    ensure!(
        r.planner.as_deref().is_none_or(|p| p == "nuclei"),
        "unknown planner"
    );
    ensure!(
        ["none", "ignition", "evacuate", "adaptive"].contains(&r.policy.as_str()),
        "unknown policy"
    );
    ensure!(
        !r.history.is_empty() && r.history[0].at_min == 0,
        "history must start at zero"
    );
    ensure!(
        r.case.start_min >= 0 && r.case.start_min < r.case.minutes,
        "invalid reporting time"
    );
    let mut previous = -1;
    for w in &r.history {
        ensure!(
            w.at_min > previous && w.at_min < r.case.minutes && w.at_min * 60 % STEP_S == 0,
            "invalid weather time"
        );
        ensure!(
            w.wind_from_deg.is_finite()
                && (0.0..360.0).contains(&w.wind_from_deg)
                && w.wind_kmh.is_finite()
                && (0.0..=100.0).contains(&w.wind_kmh)
                && w.moisture_pct.is_finite()
                && (0.0..=100.0).contains(&w.moisture_pct),
            "invalid weather"
        );
        previous = w.at_min;
    }
    let data = Path::new("data");
    let t = Territory::load(data)?;
    let mut c = r.case.clone();
    c.start_min = 0;
    if r.native_case {
        let initial = t.weather(&c);
        let mut expected = vec![(0, initial)];
        if let Some(point) = t.shifted(&c) {
            ensure!(
                point.1.wind_dir_deg != initial.wind_dir_deg,
                "native shift must change direction"
            );
            expected.push(point);
        }
        ensure!(
            expected.len() == r.history.len(),
            "native case/history mismatch"
        );
        for ((at, w), point) in expected.iter().zip(&r.history) {
            ensure!(
                *at == point.at_min * 60
                    && w.wind_dir_deg == point.wind_from_deg
                    && w.wind_speed_kmh == point.wind_kmh
                    && w.moisture_pct == point.moisture_pct,
                "native case/history mismatch"
            );
        }
    } else {
        c.shift = None;
    }
    ensure!(r.reaction_delay_min >= 0, "negative reaction delay");
    let initial = &r.history[0];
    c.wind_from_deg = initial.wind_from_deg;
    c.wind_kmh = initial.wind_kmh;
    c.moisture_pct = Some(initial.moisture_pct);
    let mut g = Game::with_case(data, t, c, r.seed)?;
    ensure!(
        g.scn.world.contains(r.case.ignition())
            && g.scn.fuel_at(g.scn.world.cell_of(r.case.ignition())) > 0,
        "ignition outside fuel"
    );
    let mut next_weather = 1;
    let mut next_orders = (r.case.start_min + r.reaction_delay_min) * 60;
    let mut first_orders = true;
    let mut moves = 0;
    let mut evacuations = 0;
    let mut orders = vec![];
    let mut evacuated = vec![false; g.districts.len()];
    let mut first_hit = vec![None; g.districts.len()];
    let mut samples = vec![];
    let mut opening = None;
    let mut rejected = 0;
    while !g.is_over() {
        while !r.native_case
            && next_weather < r.history.len()
            && r.history[next_weather].at_min * 60 <= g.time_s()
        {
            g.fire.set_weather(r.history[next_weather].weather())?;
            next_weather += 1;
        }
        if opening.is_none() && g.time_s() >= r.case.start_min * 60 {
            let o = g.outcome();
            opening = Some(
                json!({"minute": g.time_s() as f64 / 60.0, "caught": o.caught(), "homes": o.homes_hit(), "ha": o.hectares}),
            );
        }
        if r.policy != "none" && g.time_s() >= next_orders && g.can_pause() {
            g.pause().map_err(anyhow::Error::msg)?;
            let mut ranked: Vec<_> = g
                .districts
                .iter()
                .enumerate()
                .filter_map(|(i, d)| {
                    district::exposure(d, &g.fire, &g.scn)
                        .map(|e| (i, district::reach_m(e.distance_m, e.downwind)))
                })
                .collect();
            ranked.sort_by(|a, b| a.1.total_cmp(&b.1));
            let cluster_targets = if r.planner.as_deref() == Some("nuclei") {
                let (priorities, targets) = nucleus_targets(&g);
                ranked = priorities;
                Some(targets)
            } else {
                None
            };
            if r.policy == "ignition" && first_orders {
                let at = g
                    .hexes
                    .nearest(r.case.ignition())
                    .context("no road hex")?
                    .at;
                for k in 0..g.teams.len() {
                    match g.move_team(k, at) {
                        Ok(plan) => {
                            moves += 1;
                            let p = plan.hex.at.centre();
                            orders.push(json!({"minute":g.time_s() as f64/60.0,"team":k,"target":[p.x,p.y],"eta_min":plan.minutes}));
                        }
                        Err(_) => rejected += 1,
                    }
                }
            }
            if r.policy == "adaptive" {
                // Defend nearby threatened districts. Match engines by travel
                // time and keep existing assignments instead of permuting them
                // whenever two districts swap rank. Stage on the sheltered side.
                let cutoff = ranked
                    .first()
                    .map_or(900.0, |p| (p.1 * 2.5).clamp(900.0, 1600.0));
                let targets: Vec<_> = cluster_targets.unwrap_or_else(|| {
                    ranked
                        .iter()
                        .filter(|p| p.1 <= cutoff)
                        .take(3)
                        .filter_map(|&(d, _)| {
                            let centre = g.districts[d].centre;
                            let e = district::exposure(&g.districts[d], &g.fire, &g.scn)?;
                            let dx = centre.x - e.fire_at.x;
                            let dy = centre.y - e.fire_at.y;
                            let length = (dx * dx + dy * dy).sqrt().max(1.0);
                            let safe = scenario::Pos {
                                x: centre.x + 180.0 * dx / length,
                                y: centre.y + 180.0 * dy / length,
                            };
                            g.hexes.nearest(safe).map(|h| h.at)
                        })
                        .collect()
                });
                if !targets.is_empty() {
                    let slots: Vec<_> = (0..g.teams.len())
                        .map(|i| targets[i % targets.len()])
                        .collect();
                    let mut used_teams = vec![false; g.teams.len()];
                    let mut used_slots = vec![false; slots.len()];
                    for (k, used) in used_teams.iter_mut().enumerate() {
                        if let Some(at) = g.teams[k].hex {
                            if !g.team_in_danger(k) {
                                if let Some(j) = slots.iter().enumerate().position(|(j, target)| {
                                    !used_slots[j] && at.distance(*target) <= 1
                                }) {
                                    *used = true;
                                    used_slots[j] = true;
                                }
                            }
                        }
                    }
                    let mut options = vec![];
                    for (k, used) in used_teams.iter().enumerate() {
                        if *used {
                            continue;
                        }
                        for (j, at) in slots.iter().enumerate() {
                            if used_slots[j] {
                                continue;
                            }
                            if let Ok(plan) = g.plan_move(k, *at) {
                                options.push((plan.minutes, k, j));
                            }
                        }
                    }
                    options.sort_by(|a, b| {
                        a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2))
                    });
                    for (_, k, j) in options {
                        if moves >= 15 || used_teams[k] || used_slots[j] {
                            continue;
                        }
                        let at = slots[j];
                        match g.move_team(k, at) {
                            Ok(plan) => {
                                used_teams[k] = true;
                                used_slots[j] = true;
                                moves += 1;
                                let p = at.centre();
                                orders.push(json!({"minute":g.time_s() as f64/60.0,"team":k,"target":[p.x,p.y],"eta_min":plan.minutes}));
                            }
                            Err(_) => rejected += 1,
                        }
                    }
                }
            }
            if (r.policy == "evacuate" || r.policy == "adaptive")
                && g.evac.active().is_none()
                && (r.policy == "evacuate" || evacuations < 4)
            {
                if let Some(&(d, _)) = ranked
                    .iter()
                    .find(|&&(d, _)| !evacuated[d] && g.plan_evacuation(d).is_ok())
                {
                    if g.evacuate(d).is_ok() {
                        evacuated[d] = true;
                        evacuations += 1;
                        orders.push(
                            json!({"minute":g.time_s() as f64/60.0,"evacuate":g.districts[d].name}),
                        );
                    } else {
                        rejected += 1;
                    }
                }
            }
            g.resume();
            first_orders = false;
            next_orders = g.time_s() + rocca::PAUSE_COOLDOWN_S;
        }
        g.step()?;
        let o = g.outcome();
        for (i, d) in o.districts.iter().enumerate() {
            if first_hit[i].is_none() && (d.caught > 0 || d.homes_hit > 0) {
                first_hit[i] = Some(g.time_s() as f64 / 60.0);
            }
        }
        if g.time_s() % 600 == 0 {
            samples.push(json!({"minute": g.time_s()/60, "ha":o.hectares,"caught":o.caught(),"homes":o.homes_hit(),"closed_segments":g.closed_roads().len()}));
        }
    }
    let o = g.outcome();
    let districts: Vec<_> = g.districts.iter().zip(&o.districts).enumerate().map(|(i,(d,x))| json!({"name":d.name,"caught":x.caught,"homes":x.homes_hit,"first_hit_min":first_hit[i]})).collect();
    println!(
        "{}",
        json!({"case":r.case.name,"seed":r.seed,"policy":r.policy,"native_case":r.native_case,"reaction_delay_min":r.reaction_delay_min,"planner":r.planner,"moves":moves,"evacuations":evacuations,"orders":orders,"ha":o.hectares,"caught":o.caught(),"homes":o.homes_hit(),"units_lost":o.units_lost,"end_min":o.at_s as f64/60.0,"opening":opening,"rejected_orders":rejected,"districts":districts,"samples":samples})
    );
    Ok(())
}
