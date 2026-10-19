//! Synthetic, simplified territories for the suppression laboratory.
//!
//! A square world of 20 m cells with one fuel model, a clean relief, a few
//! straight roads and one or two villages: just enough to tell *why* a team
//! algorithm does well or badly, with everything else held still. The files
//! are written in the same format as `data/scenarios/<id>/`, so the one
//! authority [`crate::Game`] loads them like any territory; nothing in the
//! game knows it is a laboratory.
//!
//! Every spec gets its own data directory (`<out>/data/<name>/`) holding a
//! scenario called [`SCENARIO`], so one process can play many specs at once
//! (`ROCCA_SCENARIO` is set once and never changes).

use std::f32::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::json;

/// The id every synthetic territory carries inside its own data directory.
pub const SCENARIO: &str = "lab";
/// Fire grid cell, metres (fixed by the PROPAGATOR rasters).
pub const CELL_M: f32 = 20.0;
/// Road vertices are on this lattice, so that every crossing is a shared node.
const GRID_M: f32 = 50.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Relief {
    Flat,
    /// Rises `grade` (rise/run) toward the bearing `uphill_deg`.
    Slope { grade: f32, uphill_deg: f32 },
    /// An east-west ridge across the middle of the world.
    Ridge { height_m: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Fuels {
    Uniform(i32),
    /// Two fuel classes in blobs of about `scale_m`.
    Patchy { a: i32, b: i32, scale_m: f32 },
    /// A belt of non-burnable ground (`width_m` wide, east-west, at `y_frac`)
    /// across an otherwise uniform world.
    Break { fuel: i32, y_frac: f32, width_m: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Roads {
    /// Lines every `spacing_m` both ways: access everywhere.
    Lattice { spacing_m: f32 },
    /// One east-west road in the south, one in the middle, joined by a
    /// north-south one: the usual valley.
    Cross,
    /// A single east-west road in the south and the spurs to the villages:
    /// poor access to the fire.
    Single,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Village {
    pub name: String,
    /// Position as a fraction of the world, (x, y), y north.
    pub at: (f32, f32),
    pub homes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shift {
    pub at_min: i64,
    pub wind_from_deg: f64,
    pub wind_kmh: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SynthSpec {
    pub name: String,
    pub about: String,
    /// The world is `cells` × `cells` cells of 20 m.
    pub cells: usize,
    pub relief: Relief,
    pub fuels: Fuels,
    pub roads: Roads,
    pub villages: Vec<Village>,
    /// Fractions of the world; stations sit on the nearest drivable road.
    pub stations: Vec<(f32, f32)>,
    pub ignition: (f32, f32),
    pub ignition_radius_m: f32,
    pub wind_from_deg: f64,
    pub wind_kmh: f64,
    pub moisture_pct: f64,
    pub shift: Option<Shift>,
    /// Minutes the fire burns before the game starts (the report delay).
    pub start_min: i64,
    /// Length of the case, minutes.
    pub minutes: i64,
}

impl SynthSpec {
    pub fn world_m(&self) -> f32 {
        self.cells as f32 * CELL_M
    }
    pub fn at(&self, f: (f32, f32)) -> (f32, f32) {
        (f.0 * self.world_m(), f.1 * self.world_m())
    }
}

fn hash(ix: i64, iy: i64, seed: u64) -> f32 {
    let mut h = (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ seed.wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h & 0xFF_FFFF) as f32 / 0x100_0000 as f32
}

/// Smooth value noise in [0, 1], feature size `scale`.
fn noise(x: f32, y: f32, scale: f32, seed: u64) -> f32 {
    let (fx, fy) = (x / scale, y / scale);
    let (ix, iy) = (fx.floor() as i64, fy.floor() as i64);
    let (tx, ty) = (fx - ix as f32, fy - iy as f32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (tx, ty) = (s(tx), s(ty));
    let a = hash(ix, iy, seed) * (1.0 - tx) + hash(ix + 1, iy, seed) * tx;
    let b = hash(ix, iy + 1, seed) * (1.0 - tx) + hash(ix + 1, iy + 1, seed) * tx;
    a * (1.0 - ty) + b * ty
}

fn elevation(spec: &SynthSpec, x: f32, y: f32) -> f32 {
    let w = spec.world_m();
    match spec.relief {
        Relief::Flat => 20.0,
        Relief::Slope { grade, uphill_deg } => {
            let u = uphill_deg.to_radians();
            let along = (x - w / 2.0) * u.sin() + (y - w / 2.0) * u.cos();
            (w * grade * 0.5 + grade * along).max(0.0)
        }
        Relief::Ridge { height_m } => {
            let d = (y - w * 0.5) / (w * 0.10);
            20.0 + height_m * (-d * d).exp()
        }
    }
}

type Line = Vec<[f32; 2]>;

/// A straight line from `a` to `b` with a vertex at least every `step` metres.
fn line(a: (f32, f32), b: (f32, f32), step: f32) -> Line {
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let n = (len / step).ceil().max(1.0) as usize;
    (0..=n).map(|i| [a.0 + (b.0 - a.0) * i as f32 / n as f32, a.1 + (b.1 - a.1) * i as f32 / n as f32]).collect()
}

fn snap(v: f32) -> f32 {
    (v / GRID_M).round() * GRID_M
}

struct Road {
    class: &'static str,
    name: String,
    line: Line,
}

/// The main roads and the stations' roads.
fn main_roads(spec: &SynthSpec) -> Vec<Road> {
    let w = spec.world_m();
    let mut out = vec![];
    let mut add = |class: &'static str, name: &str, a: (f32, f32), b: (f32, f32)| out.push(Road { class, name: name.into(), line: line(a, b, GRID_M) });
    match spec.roads {
        Roads::Lattice { spacing_m } => {
            let n = (w / spacing_m).floor() as i32;
            for k in 1..=n {
                let v = snap(k as f32 * spacing_m);
                if v <= 0.0 || v >= w {
                    continue;
                }
                add("secondary", &format!("Via O{k}"), (0.0, v), (w, v));
                add("secondary", &format!("Via N{k}"), (v, 0.0), (v, w));
            }
        }
        Roads::Cross => {
            let (y1, y2, x) = (snap(w * 0.15), snap(w * 0.55), snap(w * 0.5));
            add("secondary", "Strada del Fondo", (0.0, y1), (w, y1));
            add("secondary", "Strada di Mezzo", (snap(w * 0.1), y2), (snap(w * 0.9), y2));
            add("secondary", "Strada della Valle", (x, y1), (x, y2));
        }
        Roads::Single => {
            let y1 = snap(w * 0.15);
            add("secondary", "Strada del Fondo", (0.0, y1), (w, y1));
        }
    }
    out
}

/// The nearest y of a main road to the point (for the village spur), if the
/// roads are east-west there.
fn nearest_ew(roads: &[Road], x: f32, y: f32) -> Option<f32> {
    roads
        .iter()
        .filter(|r| r.line.len() >= 2 && (r.line[0][1] - r.line[1][1]).abs() < 1e-3)
        .filter(|r| x >= r.line[0][0].min(r.line.last().unwrap()[0]) && x <= r.line[0][0].max(r.line.last().unwrap()[0]))
        .map(|r| r.line[0][1])
        .min_by(|a, b| (a - y).abs().total_cmp(&(b - y).abs()))
}

struct Layout {
    streets: Vec<Road>,
    houses: Vec<[f32; 2]>,
    centre: (f32, f32),
}

fn village_layout(spec: &SynthSpec, v: &Village, roads: &[Road]) -> Layout {
    let w = spec.world_m();
    let (cx, cy) = {
        let p = spec.at(v.at);
        (snap(p.0), snap(p.1))
    };
    let s = 50.0 * ((v.homes as f32 / 20.0).sqrt().ceil()).max(1.0) + 50.0;
    let mut streets = vec![];
    let mut k = -s;
    while k <= s + 0.1 {
        streets.push(Road { class: "residential", name: format!("Via {} {}", v.name, streets.len() + 1), line: line((cx - s, cy + k), (cx + s, cy + k), 25.0) });
        streets.push(Road { class: "residential", name: format!("Via {} {}", v.name, streets.len() + 1), line: line((cx + k, cy - s), (cx + k, cy + s), 25.0) });
        k += 50.0;
    }
    // the spur to the nearest east-west main road
    if let Some(ym) = nearest_ew(roads, cx, cy) {
        if (ym - cy).abs() > s {
            let to = if ym < cy { cy - s } else { cy + s };
            streets.push(Road { class: "tertiary", name: format!("Strada per {}", v.name), line: line((cx, ym), (cx, to), GRID_M) });
        }
    }
    // houses 12 m either side of every street, every 25 m, nearest to the centre first
    let mut cand: Vec<[f32; 2]> = vec![];
    let mut k = -s;
    while k <= s + 0.1 {
        let mut t = -s + 12.5;
        while t < s {
            for side in [-12.0, 12.0] {
                cand.push([cx + t, cy + k + side]);
                cand.push([cx + k + side, cy + t]);
            }
            t += 25.0;
        }
        k += 50.0;
    }
    cand.sort_by(|a, b| {
        let d = |p: &[f32; 2]| (p[0] - cx).powi(2) + (p[1] - cy).powi(2) + hash((p[0] * 4.0) as i64, (p[1] * 4.0) as i64, 7);
        d(a).total_cmp(&d(b))
    });
    let mut houses: Vec<[f32; 2]> = vec![];
    for p in cand {
        if houses.len() >= v.homes {
            break;
        }
        // not on another street, not on a neighbour, inside the world
        let on_street = |q: &[f32; 2]| {
            let (dx, dy) = ((q[0] - cx).rem_euclid(50.0), (q[1] - cy).rem_euclid(50.0));
            let near = |d: f32| d.min(50.0 - d) < 8.0;
            near(dx) || near(dy)
        };
        if on_street(&p) || houses.iter().any(|h| (h[0] - p[0]).hypot(h[1] - p[1]) < 18.0) || p[0] < 0.0 || p[1] < 0.0 || p[0] >= w || p[1] >= w {
            continue;
        }
        houses.push(p);
    }
    Layout { streets, houses, centre: (cx, cy) }
}

fn write_raw<T: Copy>(path: &Path, v: &[T]) -> Result<()> {
    let bytes = unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) };
    fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

/// What a built spec leaves on disk and what the laboratory needs from it.
#[derive(Debug, Clone)]
pub struct Built {
    pub data_dir: PathBuf,
    pub homes: usize,
}

/// A symlink where there are any, a copy elsewhere (Windows).
fn link_or_copy(from: &Path, to: &Path) -> Result<()> {
    #[cfg(unix)]
    std::os::unix::fs::symlink(from, to)?;
    #[cfg(not(unix))]
    {
        fn copy(from: &Path, to: &Path) -> Result<()> {
            if from.is_dir() {
                fs::create_dir_all(to)?;
                for e in fs::read_dir(from)? {
                    let e = e?;
                    copy(&e.path(), &to.join(e.file_name()))?;
                }
            } else {
                fs::copy(from, to)?;
            }
            Ok(())
        }
        copy(from, to)?;
    }
    Ok(())
}

/// Write the spec as `<out>/data/<name>/`; `repo_data` is the game's `data/`
/// (behaviour library and fuel table are linked from there).
pub fn build(spec: &SynthSpec, out: &Path, repo_data: &Path) -> Result<Built> {
    let data = out.join("data").join(&spec.name);
    let dir = data.join("scenarios").join(SCENARIO);
    fs::create_dir_all(&dir)?;
    let repo = fs::canonicalize(repo_data).with_context(|| format!("data dir {}", repo_data.display()))?;
    for item in ["behaviours", "fuels_eu12.json"] {
        let link = data.join(item);
        if !link.exists() {
            link_or_copy(&repo.join(item), &link)?;
        }
    }
    let n = spec.cells;
    let w = spec.world_m();

    // relief: the fire grid and the render grid (one more point per side)
    let mut dem = vec![0f64; n * n];
    for r in 0..n {
        for c in 0..n {
            let (x, y) = ((c as f32 + 0.5) * CELL_M, w - (r as f32 + 0.5) * CELL_M);
            dem[r * n + c] = elevation(spec, x, y) as f64;
        }
    }
    let m = n + 1;
    let mut rt = vec![0f32; m * m];
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for r in 0..m {
        for c in 0..m {
            let v = elevation(spec, c as f32 * CELL_M, w - r as f32 * CELL_M);
            rt[r * m + c] = v;
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }

    // fuel
    let mut fuel = vec![0i32; n * n];
    for r in 0..n {
        for c in 0..n {
            let (x, y) = ((c as f32 + 0.5) * CELL_M, w - (r as f32 + 0.5) * CELL_M);
            fuel[r * n + c] = match spec.fuels {
                Fuels::Uniform(f) => f,
                Fuels::Patchy { a, b, scale_m } => {
                    if noise(x, y, scale_m, 11) < 0.5 {
                        a
                    } else {
                        b
                    }
                }
                Fuels::Break { fuel, y_frac, width_m } => {
                    if (y - y_frac * w).abs() < width_m / 2.0 {
                        0
                    } else {
                        fuel
                    }
                }
            };
        }
    }

    // roads
    let mains = main_roads(spec);
    let layouts: Vec<Layout> = spec.villages.iter().map(|v| village_layout(spec, v, &mains)).collect();
    let world = scenario::World { width_m: w, height_m: w, fire_rows: n, fire_cols: n, cellsize: CELL_M };
    let mut all_roads: Vec<&Road> = mains.iter().collect();
    for l in &layouts {
        all_roads.extend(l.streets.iter());
    }
    let mut clear = |cells: Vec<scenario::Cell>| {
        for c in cells {
            fuel[c.row * n + c.col] = 0;
        }
    };
    for r in &all_roads {
        for seg in r.line.windows(2) {
            clear(fire::cells_along(&world, scenario::Pos { x: seg[0][0], y: seg[0][1] }, scenario::Pos { x: seg[1][0], y: seg[1][1] }, 9.0));
        }
    }
    // houses stand on non-burnable cells (as in the real data)
    for l in &layouts {
        for h in &l.houses {
            clear(vec![world.cell_of(scenario::Pos { x: h[0], y: h[1] })]);
        }
    }
    let burnable = |r: usize, c: usize| (1..=12).contains(&fuel[r * n + c]);

    // people
    let (mut dwellings, mut households, mut people) = (vec![], vec![], vec![]);
    let mut roads_json = vec![];
    for (i, r) in all_roads.iter().enumerate() {
        roads_json.push(json!({"id": i + 1, "class": r.class, "drivable": true, "track": false, "name": r.name, "oneway": false, "line": r.line}));
    }
    let mut hydrants = vec![];
    for (vi, l) in layouts.iter().enumerate() {
        hydrants.push(json!({"id": 1000 + vi, "kind": "hydrant", "pos": [l.centre.0, l.centre.1]}));
        for h in &l.houses {
            let id = households.len();
            let cell = world.cell_of(scenario::Pos { x: h[0], y: h[1] });
            // distance to the nearest burnable cell centre, up to 200 m
            let mut d = 200.0f32;
            let span = 10i64;
            for dr in -span..=span {
                for dc in -span..=span {
                    let (rr, cc) = (cell.row as i64 + dr, cell.col as i64 + dc);
                    if rr < 0 || cc < 0 || rr >= n as i64 || cc >= n as i64 || !burnable(rr as usize, cc as usize) {
                        continue;
                    }
                    d = d.min(((dr * dr + dc * dc) as f32).sqrt() * CELL_M);
                }
            }
            let size = 2 + (hash(id as i64, 3, 5) * 3.0) as u8;
            let members: Vec<usize> = (0..size as usize).map(|k| people.len() + k).collect();
            for _ in 0..size {
                people.push(json!({"id": people.len(), "household": id, "age": 40, "walk_speed": 1.2, "needs_assistance": false, "at_home": true}));
            }
            dwellings.push(json!({"osm_id": id, "kind": "residential", "pos": h, "area_m2": 90.0, "levels": 2, "units": 1, "cell": [cell.row, cell.col], "dist_to_fuel_m": d, "fuel_at_site": 0}));
            households.push(json!({
                "id": id, "building": id, "pos": h, "cell": [cell.row, cell.col], "size": size, "vehicles": 1, "dist_to_fuel_m": d,
                "locality": spec.villages[vi].name, "risk_perception": 0.5, "prior_fire_experience": false, "warning_channel": "mobile_alert",
                "trust_authority": 0.6, "intent": "wait_and_see", "prep_time_min": 10.0,
                "defensible_space": 0.2 + 0.6 * hash(id as i64, 9, 5), "has_pets_livestock": false, "status": "normal", "members": members
            }));
        }
    }
    let homes = households.len();

    write_raw(&dir.join("fuel.i32"), &fuel)?;
    write_raw(&dir.join("dem.f64"), &dem)?;
    write_raw(&dir.join("render_terrain.f32"), &rt)?;
    fs::write(dir.join("render_terrain.json"), serde_json::to_vec(&json!({"rows": m, "cols": m, "posting_m": CELL_M, "world_size_m": [w, w], "elev_min": lo, "elev_max": hi}))?)?;
    fs::write(
        dir.join("osm.json"),
        serde_json::to_vec(&json!({"world_size_m": [w, w], "fire_grid": {"rows": n, "cols": n, "cellsize": CELL_M}, "buildings": [], "roads": roads_json, "water": hydrants}))?,
    )?;
    fs::write(dir.join("population.json"), serde_json::to_vec(&json!({"synthetic": true, "seed": 1, "dwellings": dwellings, "households": households, "people": people}))?)?;
    let names: Vec<&str> = spec.villages.iter().map(|v| v.name.as_str()).collect();
    fs::write(
        dir.join("scenario.json"),
        serde_json::to_vec(&json!({"id": SCENARIO, "name": spec.name, "description": spec.about, "world_size_m": [w, w], "fire_grid_size": [n, n], "localities": names}))?,
    )?;

    // game.json: the roster here is only a default, the laboratory sets its own
    let stations: Vec<_> = spec.stations.iter().map(|&f| {
        let p = spec.at(f);
        json!({"name": format!("Presidio {}", spec.name), "pos": [snap(p.0), snap(p.1)]})
    }).collect();
    let (ix, iy) = spec.at(spec.ignition);
    let mut case = json!({
        "name": "lab", "near": names.first().copied().unwrap_or(""), "ignition": [ix, iy], "radius_m": spec.ignition_radius_m,
        "wind_from_deg": spec.wind_from_deg, "wind_kmh": spec.wind_kmh, "moisture_pct": spec.moisture_pct,
        "minutes": spec.minutes, "start_min": spec.start_min
    });
    if let Some(s) = &spec.shift {
        case["shift"] = json!({"at_min": s.at_min, "wind_from_deg": s.wind_from_deg, "wind_kmh": s.wind_kmh});
    }
    let refuges: Vec<_> = layouts
        .iter()
        .zip(&spec.villages)
        .map(|(l, v)| json!({"district": v.name, "name": "il rifugio", "pos": [snap(w * 0.05), snap(l.centre.1.min(w * 0.15))]}))
        .collect();
    fs::write(
        dir.join("game.json"),
        serde_json::to_vec_pretty(&json!({
            "scenario": SCENARIO, "moisture_pct": spec.moisture_pct, "stations": stations,
            "roster": [{"station": 0, "crew": 3}], "game": "lab", "cases": [case],
            "evacuation": {"per_min": 20.0, "prep_min": 5.0, "refuges": refuges}
        }))?,
    )?;
    let _ = PI;
    Ok(Built { data_dir: data, homes })
}

// --- the catalogue ---------------------------------------------------------------

fn base(name: &str, about: &str) -> SynthSpec {
    SynthSpec {
        name: name.into(),
        about: about.into(),
        cells: 200,
        relief: Relief::Flat,
        fuels: Fuels::Uniform(8),
        roads: Roads::Lattice { spacing_m: 600.0 },
        villages: vec![Village { name: "Borgo".into(), at: (0.5, 0.8), homes: 50 }],
        stations: vec![(0.05, 0.15), (0.95, 0.15)],
        ignition: (0.5, 0.3),
        ignition_radius_m: 40.0,
        wind_from_deg: 180.0,
        wind_kmh: 25.0,
        moisture_pct: 7.0,
        shift: None,
        start_min: 20,
        minutes: 240,
    }
}

/// The named scenarios of the laboratory: each isolates one thing a team
/// algorithm has to cope with.
pub fn catalogue() -> Vec<SynthSpec> {
    let mut v = vec![];
    v.push(base("pianura", "pianura uniforme, vento da sud, borgo sottovento a 2 km: il caso di riferimento"));
    let mut s = base("pendio", "salita del 15 % verso il borgo: il fuoco corre in salita");
    s.relief = Relief::Slope { grade: 0.15, uphill_deg: 0.0 };
    v.push(s);
    let mut s = base("crinale", "un crinale tra l'innesco e il borgo: il vento cambia sul pendio");
    s.relief = Relief::Ridge { height_m: 120.0 };
    v.push(s);
    let mut s = base("macchia", "combustibile a chiazze (prateria e macchia): fronte irregolare");
    s.fuels = Fuels::Patchy { a: 2, b: 9, scale_m: 400.0 };
    v.push(s);
    let mut s = base("fascia", "fascia non combustibile davanti al borgo: dove tenere la linea");
    s.fuels = Fuels::Break { fuel: 8, y_frac: 0.6, width_m: 60.0 };
    v.push(s);
    let mut s = base("due-borghi", "due borghi a 1,2 km uno dall'altro, il vento gira dopo un'ora: serve una priorità");
    s.villages = vec![Village { name: "Borgo Est".into(), at: (0.75, 0.75), homes: 40 }, Village { name: "Borgo Ovest".into(), at: (0.25, 0.75), homes: 40 }];
    s.shift = Some(Shift { at_min: 60, wind_from_deg: 120.0, wind_kmh: 25.0 });
    s.minutes = 300;
    v.push(s);
    let mut s = base("strada-unica", "una sola strada a sud: poco accesso al fianco e alla testa");
    s.roads = Roads::Single;
    v.push(s);
    let mut s = base("raffica", "vento forte (40 km/h) e macchia alta: il fronte è troppo veloce per fermarlo");
    s.wind_kmh = 40.0;
    s.fuels = Fuels::Uniform(9);
    s.moisture_pct = 5.0;
    v.push(s);
    let mut s = base("cambio-vento", "il vento gira di 90° dopo 50 minuti: il fianco diventa la testa");
    s.shift = Some(Shift { at_min: 50, wind_from_deg: 270.0, wind_kmh: 25.0 });
    v.push(s);
    v
}

pub fn spec(name: &str) -> Option<SynthSpec> {
    catalogue().into_iter().find(|s| s.name == name)
}
