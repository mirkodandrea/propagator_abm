//! One game: the single simulation and gameplay authority, stepped by the
//! headless runner, the tests and the kiosk alike (docs/01-GIOCO.md).
//!
//! Order of a step, every [`STEP_S`] simulated seconds: wind, fire, teams,
//! protection of the homes near the teams, evacuation, bookkeeping. The
//! player acts only during a **pause** ([`Game::pause`]), when the world
//! stands still: moving a team to a hex ([`Game::move_team`]) and evacuating
//! a district ([`Game::evacuate`]). The pause has no time limit.
//! Each order has a preview that reads the state and changes nothing
//! ([`Game::plan_move`], [`Game::plan_evacuation`]).

use std::path::Path;

use abm::network::{self, RoadNetwork};
use abm::suppression::{Task, UnitEffect, UnitKind, UnitState, ENGINE_SPEED};
use abm::traffic::Traffic;
use abm::Suppression;
use anyhow::{Context, Result};
use fire::threat::IMPASSABLE;
use fire::{FireSim, Weather};
use scenario::{Pos, Scenario};

use crate::case::{Case, Territory};
use crate::district::{self, dist, District};
use crate::evac::{self, EvacPlan, Evacuations, State};
use crate::hex::{Axial, Hex, HexGrid};

/// Simulated seconds per step.
pub const STEP_S: i64 = 6;
/// Simulated seconds per real second while the game runs: the game goes at
/// ×60 or it is stopped, nothing else (user, 2026-10-14).
pub const RUN_X: f32 = 60.0;
/// Real seconds after a pause before the next can be asked (user,
/// 2026-10-14). The game only runs at [`RUN_X`] between pauses, so it is
/// kept as simulated time: [`PAUSE_COOLDOWN_S`].
pub const PAUSE_COOLDOWN_REAL_S: f32 = 30.0;
/// Simulated time after a pause before the next can be asked: 30 min.
pub const PAUSE_COOLDOWN_S: i64 = (PAUSE_COOLDOWN_REAL_S * RUN_X) as i64;
/// Spegnimento rispetto al modello calibrato (scelta dell'utente 2026-10-13:
/// linea ×3, acqua ×30, portata ×20).
pub const SUPPRESSION: UnitEffect = UnitEffect { line_x: 3.0, water_x: 30.0, reach_x: 20.0, nerve_x: 1.0 };
/// An engine that emptied its tank stays where it is and is full again after
/// this long, simulated: the water is brought to it (user, 2026-10-14).
pub const REFILL_S: f32 = 10.0 * 60.0;
/// How far from its hex's road point a team's people on foot go to work.
pub const CREW_RANGE_M: f32 = 400.0;
/// How much closer to the fire each new line is cut.
pub const CREW_STEP_M: f32 = 80.0;
/// A team standing in a place protects the homes this close to it.
pub const PROTECT_M: f32 = 300.0;

/// How much a team protects the homes around it (0-1): an engine with water
/// does the full job, an empty one part of it.
pub fn protection_of(kind: UnitKind, water_l: f32) -> f32 {
    match kind {
        UnitKind::Engine if water_l > 0.0 => 1.0,
        UnitKind::Engine => 0.5,
        UnitKind::HandCrew => 0.6,
        UnitKind::AirTanker => 0.0,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    pub at_s: i64,
    pub text: String,
}

/// A team: an engine and its crew, from a station.
#[derive(Debug, Clone)]
pub struct Team {
    /// The engine: index into `Game::crews.units`. It wets the fuel it can
    /// reach from the road.
    pub unit: usize,
    /// The people on foot (a hand crew unit riding with the engine): they
    /// cut the vegetation across the fire's way (user, 2026-10-14).
    pub crew_unit: usize,
    pub name: String,
    pub station: usize,
    pub volunteers: bool,
    pub crew: u8,
    /// The hex it was last sent to.
    pub hex: Option<Axial>,
}

/// What sending a team to a hex would be ([`Game::plan_move`]).
#[derive(Debug, Clone)]
pub struct MovePlan {
    pub team: usize,
    pub hex: Hex,
    /// The road it would drive, from where it is.
    pub path: Vec<Pos>,
    pub minutes: f32,
}

/// Facts per district at the end (or now).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DistrictOutcome {
    pub homes: usize,
    pub homes_hit: usize,
    pub people: u32,
    pub caught: u32,
    pub safe: u32,
    pub on_the_road: u32,
    pub self_defence: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    pub at_s: i64,
    pub districts: Vec<DistrictOutcome>,
    pub hectares: f32,
    pub units_lost: usize,
}

impl Outcome {
    pub fn homes_hit(&self) -> usize {
        self.districts.iter().map(|d| d.homes_hit).sum()
    }
    pub fn caught(&self) -> u32 {
        self.districts.iter().map(|d| d.caught).sum()
    }
    pub fn people(&self) -> u32 {
        self.districts.iter().map(|d| d.people).sum()
    }
}

pub struct Game {
    pub scn: Scenario,
    pub fire: FireSim,
    pub net: RoadNetwork,
    /// Empty: there is no civilian traffic model any more; the teams' driving
    /// code reads it for queues and finds none.
    traffic: Traffic,
    pub crews: Suppression,
    pub teams: Vec<Team>,
    pub hexes: HexGrid,
    pub territory: Territory,
    pub case: Case,
    pub seed: u64,
    pub districts: Vec<District>,
    pub evac: Evacuations,
    pub log: Vec<LogEntry>,
    pause_from: Option<i64>,
    /// The pause in course uses up the expert (false: the opening one).
    pause_costs: bool,
    pause_ready_s: i64,
    pub pauses: u32,
    logged: Vec<UnitState>,
}

impl Game {
    /// A new game on the territory in `data_dir`, fire `case`.
    pub fn new(data_dir: &Path, case: &str, seed: u64) -> Result<Game> {
        let territory = Territory::load(data_dir)?;
        let case = territory.case(case).with_context(|| format!("caso {case:?} assente in game.json"))?.clone();
        Game::with_case(data_dir, territory, case, seed)
    }

    /// A game on a fire that need not be in `game.json` (trials).
    pub fn with_case(data_dir: &Path, territory: Territory, case: Case, seed: u64) -> Result<Game> {
        let scn = Scenario::load(data_dir)?;
        let mut fire = FireSim::new(&scn, territory.weather(&case), seed)?;
        fire.ignite_patch(scn.world.cell_of(case.ignition()), case.radius_m, &scn)?;
        let lib = behavior::Library::load_dir_reported(&data_dir.join(behavior::library::DEFAULT_DIR))?.library;
        lib.validate_runtime().map_err(|e| anyhow::anyhow!("behaviour library: {e}"))?;
        let policy = abm::UnitRuntime::build(&lib)
            .map_err(|e| anyhow::anyhow!("unit behaviour: {e}"))?
            .context("unit behaviour: no profile is enabled")?;
        let net = RoadNetwork::build(&scn);
        let first = Pos::from(territory.stations.first().context("game.json: no station")?.pos);
        let main = net.nearest(first, true).context("no drivable road near the first station")?;
        // stations stand on the road network the first one is on
        let stations: Vec<Pos> = territory
            .stations
            .iter()
            .map(|s| net.nearest_reachable(Pos::from(s.pos), true, main).map_or(Pos::from(s.pos), |n| net.pos(n)))
            .collect();
        // each team: its engine, then its people on foot, at the same station
        let roster: Vec<(UnitKind, Pos)> =
            territory.roster.iter().flat_map(|s| [(s.kind.unit_kind(), stations[s.station]), (UnitKind::HandCrew, stations[s.station])]).collect();
        let mut crews = Suppression::with_roster(&scn, &roster, policy)?;
        crews.effect = SUPPRESSION;
        crews.refill_in_place_s = Some(REFILL_S);
        let mut per_station = vec![0; territory.stations.len()];
        let teams: Vec<Team> = territory
            .roster
            .iter()
            .enumerate()
            .map(|(k, s)| {
                per_station[s.station] += 1;
                let st = &territory.stations[s.station];
                let name = format!("{} {}", if st.volunteers { "AIB" } else { "VVF" }, per_station[s.station]);
                crews.units[2 * k].callsign = name.clone();
                crews.units[2 * k + 1].callsign = format!("{name}, a piedi");
                Team { unit: 2 * k, crew_unit: 2 * k + 1, name, station: s.station, volunteers: st.volunteers, crew: s.crew, hex: None }
            })
            .collect();
        let hexes = HexGrid::build(&scn, &net, main);
        let districts = district::of(&scn, &net, main);
        let evac = Evacuations::new(&scn, &net, &districts, territory.evacuation.clone());
        let traffic = Traffic::new(&net);
        let logged = crews.units.iter().map(|u| u.state).collect();
        let mut game = Game {
            scn,
            fire,
            net,
            traffic,
            crews,
            teams,
            hexes,
            territory,
            case,
            seed,
            districts,
            evac,
            log: vec![],
            pause_from: None,
            pause_costs: false,
            pause_ready_s: 0,
            pauses: 0,
            logged,
        };
        // The ignition is applied on the core's next advance: one step puts
        // the opening fire into the state.
        game.step()?;
        // reported late: the fire has been burning for a while
        game.run_until(game.case.start_min * 60)?;
        game.log.clear();
        Ok(game)
    }

    pub fn time_s(&self) -> i64 {
        self.fire.time_s()
    }

    pub fn district_index(&self, name: &str) -> Option<usize> {
        self.districts.iter().position(|d| d.name.eq_ignore_ascii_case(name) || d.name.to_lowercase().contains(&name.to_lowercase()))
    }

    // --- the pause ------------------------------------------------------------

    pub fn paused(&self) -> bool {
        self.pause_from.is_some()
    }

    /// When the pause in course began.
    pub fn pause_started_s(&self) -> Option<i64> {
        self.pause_from
    }

    /// Simulated seconds before a pause can be asked (0: now).
    pub fn pause_ready_in_s(&self) -> i64 {
        (self.pause_ready_s - self.time_s()).max(0)
    }

    pub fn can_pause(&self) -> bool {
        !self.paused() && self.pause_ready_in_s() == 0 && !self.is_over()
    }

    /// Ask the expert: the pause begins, the forecast is due.
    pub fn pause(&mut self) -> Result<(), String> {
        if self.paused() {
            return Err("sei già in pausa".into());
        }
        if self.pause_ready_in_s() > 0 {
            return Err(format!("l'esperto sarà di nuovo disponibile tra {} minuti", (self.pause_ready_in_s() + 59) / 60));
        }
        self.pause_from = Some(self.time_s());
        self.pause_costs = true;
        self.pauses += 1;
        Ok(())
    }

    /// The opening pause: the game starts still, with no time limit, to look
    /// at the scene and give the first orders. It does not use up the expert
    /// (user, 2026-10-14: «parte in pausa»).
    pub fn opening_pause(&mut self) {
        if !self.paused() {
            self.pause_from = Some(self.time_s());
            self.pause_costs = false;
        }
    }

    /// End the pause now (also when its time is up). After the opening
    /// pause the expert is ready at once.
    pub fn resume(&mut self) {
        if self.pause_from.take().is_some() {
            if std::mem::take(&mut self.pause_costs) {
                self.pause_ready_s = self.time_s() + PAUSE_COOLDOWN_S;
            }
        }
    }

    /// Simulated seconds per real second now: the pause stops the world.
    pub fn speed(&self) -> f32 {
        if self.paused() { 0.0 } else { RUN_X }
    }

    // --- the teams --------------------------------------------------------------

    /// What sending team `t` to hex `at` would be, from the state now.
    pub fn plan_move(&self, t: usize, at: Axial) -> Result<MovePlan, String> {
        let team = self.teams.get(t).ok_or("squadra inesistente")?;
        let u = &self.crews.units[team.unit];
        if u.state == UnitState::Lost {
            return Err(format!("{} è fuori servizio", team.name));
        }
        let hex = *self.hexes.get(at).ok_or("lì non arriva nessuna strada")?;
        let from = self.net.nearest(u.pos, true).ok_or("la squadra non è su una strada")?;
        let nodes = network::route(&self.net, from, hex.node, self.fire.threat(), true).ok_or("tutte le strade per arrivarci sono chiuse dal fuoco")?;
        let path: Vec<Pos> = std::iter::once(u.pos).chain(std::iter::once(self.net.pos(from))).chain(nodes.iter().map(|&n| self.net.pos(n))).collect();
        let m: f32 = path.windows(2).map(|w| dist(w[0], w[1])).sum();
        Ok(MovePlan { team: t, hex, path, minutes: m / ENGINE_SPEED / 60.0 })
    }

    /// Send team `t` to hex `at`: it drives there and works the fire around
    /// it. Only during a pause.
    pub fn move_team(&mut self, t: usize, at: Axial) -> Result<MovePlan, String> {
        if !self.paused() {
            return Err("gli ordini si danno solo in pausa".into());
        }
        let plan = self.plan_move(t, at)?;
        let unit = self.teams[t].unit;
        self.crews.assign(unit, Task::Attack { at: plan.hex.target }).map_err(|e| e.to_string())?;
        // the people ride with the engine and work from where it stops
        let crew = self.teams[t].crew_unit;
        if self.crews.units[crew].state != UnitState::Lost {
            let _ = self.crews.assign(crew, Task::Attack { at: plan.hex.target });
        }
        self.teams[t].hex = Some(at);
        let text = format!("{} verso {} ({} min)", self.teams[t].name, self.place_name(plan.hex.target), plan.minutes.ceil());
        self.log.push(LogEntry { at_s: self.time_s(), text });
        Ok(plan)
    }

    /// The road still ahead of team `t`.
    pub fn team_route(&self, t: usize) -> Vec<Pos> {
        self.crews.route_points(self.teams[t].unit, &self.net)
    }

    /// Minutes of driving left for team `t`, if it is driving.
    pub fn team_eta_min(&self, t: usize) -> Option<f32> {
        let m = self.crews.route_remaining_m(self.teams[t].unit, &self.net);
        (m > 1.0).then(|| m / ENGINE_SPEED / 60.0)
    }

    /// What team `t` is doing, in Italian.
    pub fn team_status(&self, t: usize) -> String {
        let u = &self.crews.units[self.teams[t].unit];
        match u.state {
            UnitState::Moving => match self.team_eta_min(t) {
                Some(m) => format!("in viaggio, {} min", m.ceil()),
                None => "in viaggio".into(),
            },
            UnitState::Working if u.note.starts_with("waiting") => "in attesa del fuoco".into(),
            UnitState::Working => "al lavoro".into(),
            UnitState::Refilling => format!("ricarica l'acqua, {} min", ((1.0 - u.water_frac()) * REFILL_S / 60.0).ceil()),
            UnitState::Withdrawing => "si ritira: troppo pericoloso".into(),
            UnitState::Lost => "fuori servizio".into(),
            UnitState::Staged if self.teams[t].hex.is_none() => "in caserma".into(),
            UnitState::Staged => "ferma".into(),
            UnitState::Unavailable | UnitState::Inbound => "non disponibile".into(),
        }
    }

    // --- the evacuation -------------------------------------------------------------

    /// What evacuating district `d` now would be.
    pub fn plan_evacuation(&self, d: usize) -> Result<EvacPlan, String> {
        let dd = self.districts.get(d).ok_or("borgo inesistente")?;
        let plan = self.evac.plan(d, &self.districts, &self.net, &self.fire).ok_or_else(|| format!("da {} tutte le strade sono chiuse dal fuoco", dd.name))?;
        if plan.families == 0 {
            return Err(format!("a {} non c'è più nessuno da evacuare", dd.name));
        }
        Ok(plan)
    }

    /// Order district `d` out. Only during a pause, one district at a time;
    /// an evacuation ordered in this same pause is replaced (the player
    /// changed their mind: nothing has happened yet).
    pub fn evacuate(&mut self, d: usize) -> Result<EvacPlan, String> {
        if !self.paused() {
            return Err("gli ordini si danno solo in pausa".into());
        }
        let plan = self.plan_evacuation(d)?;
        let since = self.pause_from.unwrap_or(i64::MAX);
        if self.evac.active().is_some_and(|e| e.started_s >= since && e.district != d) {
            self.cancel_evacuation()?;
        }
        self.evac.start(&plan, &self.districts, self.time_s())?;
        let text = format!("evacuazione di {}: {} persone verso {} ({} min)", self.districts[d].name, plan.people, plan.path.to, plan.total_min().ceil());
        self.log.push(LogEntry { at_s: self.time_s(), text });
        Ok(plan)
    }

    /// Whether the evacuation in course was ordered in this pause, so it can
    /// still be taken back.
    pub fn can_cancel_evacuation(&self) -> bool {
        let since = self.pause_from.unwrap_or(i64::MAX);
        self.paused() && self.evac.active().is_some_and(|e| e.started_s >= since)
    }

    /// Take back the evacuation ordered in this pause.
    pub fn cancel_evacuation(&mut self) -> Result<usize, String> {
        if !self.can_cancel_evacuation() {
            return Err("si può annullare solo l'evacuazione decisa in questa pausa".into());
        }
        let since = self.pause_from.unwrap_or(i64::MAX);
        let d = self.evac.cancel(since).ok_or("l'evacuazione è già cominciata")?;
        self.log.push(LogEntry { at_s: self.time_s(), text: format!("evacuazione di {} annullata", self.districts[d].name) });
        Ok(d)
    }

    /// Roads the fire has closed, as segments: for the map.
    pub fn closed_roads(&self) -> Vec<(Pos, Pos)> {
        let threat = self.fire.threat();
        let mut out = vec![];
        for (i, &a) in self.net.nodes.iter().enumerate() {
            for e in self.net.neighbours(i as u32) {
                if !e.drivable || (e.to as usize) < i {
                    continue;
                }
                let b = self.net.pos(e.to);
                let mid = Pos { x: (a.x + b.x) * 0.5, y: (a.y + b.y) * 0.5 };
                if threat.at(mid).max(threat.at(b)) >= IMPASSABLE {
                    out.push((a, b));
                }
            }
        }
        out
    }

    // --- the weather ----------------------------------------------------------------

    pub fn weather(&self) -> Weather {
        self.fire.weather()
    }

    /// The bulletin: the change of wind still to come, and when.
    pub fn weather_ahead(&self) -> Option<(i64, Weather)> {
        self.territory.shifted(&self.case).filter(|&(at, _)| at > self.time_s())
    }

    // --- the step -------------------------------------------------------------------

    pub fn step(&mut self) -> Result<()> {
        if let Some((at, w)) = self.territory.shifted(&self.case) {
            if self.time_s() >= at && self.fire.weather().wind_dir_deg != w.wind_dir_deg {
                self.fire.set_weather(w)?;
                self.log.push(LogEntry { at_s: self.time_s(), text: format!("il vento gira: ora soffia da {}", crate::words::compass(w.wind_dir_deg)) });
            }
        }
        self.fire.advance(STEP_S)?;
        for a in self.crews.step(STEP_S as f32, &self.net, &self.traffic, &self.fire, &self.scn) {
            self.fire.queue(a);
        }
        self.crews_follow_the_fire();
        let prot = self.protection();
        self.fire.set_structure_protection(&prot);
        let now = self.time_s();
        for e in self.evac.step(STEP_S as f32, now, &self.districts, &self.net, &self.fire) {
            let text = match e {
                evac::Event::Rerouted { district } => format!("evacuazione di {}: strada chiusa dal fuoco, nuovo percorso", self.districts[district].name),
                evac::Event::Interrupted { district, families } => {
                    format!("evacuazione di {} interrotta: nessuna strada aperta, {families} famiglie in autoprotezione", self.districts[district].name)
                }
                evac::Event::Done { district } => format!("evacuazione di {}: tutti in strada", self.districts[district].name),
            };
            self.log.push(LogEntry { at_s: now, text });
        }
        for k in 0..self.crews.units.len() {
            let u = &self.crews.units[k];
            let was = std::mem::replace(&mut self.logged[k], u.state);
            if was == u.state {
                continue;
            }
            let text = match u.state {
                UnitState::Withdrawing => format!("{} si ritira vicino a {}: il fuoco è troppo vicino", u.callsign, self.place_name(u.pos)),
                UnitState::Lost => format!("il fuoco ha raggiunto {} vicino a {}: fuori servizio", u.callsign, self.place_name(u.pos)),
                _ => continue,
            };
            self.log.push(LogEntry { at_s: now, text });
        }
        Ok(())
    }

    /// People on foot who finished their line cut the next one closer to the
    /// fire, within [`CREW_RANGE_M`] of their hex's road point.
    fn crews_follow_the_fire(&mut self) {
        for t in 0..self.teams.len() {
            let (Some(at), crew) = (self.teams[t].hex, self.teams[t].crew_unit) else { continue };
            let u = &self.crews.units[crew];
            if u.state != UnitState::Staged || u.task != Task::Hold || u.note != "line complete" {
                continue;
            }
            let Some(hex) = self.hexes.get(at) else { continue };
            let here = u.pos;
            let Some(f) = self.fire.active_cells().iter().map(|c| self.scn.world.centre_of(*c)).min_by(|a, b| dist(*a, here).total_cmp(&dist(*b, here))) else { continue };
            let d = dist(f, here);
            if d < 1.0 {
                continue;
            }
            let step = CREW_STEP_M.min(d * 0.5);
            let next = Pos { x: here.x + (f.x - here.x) / d * step, y: here.y + (f.y - here.y) / d * step };
            if dist(next, hex.target) <= CREW_RANGE_M {
                let _ = self.crews.assign(crew, Task::Attack { at: next });
            }
        }
    }

    /// Per home, the protection of the teams standing near it.
    fn protection(&self) -> Vec<f32> {
        let homes = &self.scn.population.households;
        let mut p = vec![0.0f32; homes.len()];
        for u in &self.crews.units {
            // refilling stands where it worked: it still guards the homes
            if !matches!(u.state, UnitState::Working | UnitState::Staged | UnitState::Refilling) {
                continue;
            }
            let v = protection_of(u.kind, u.water_l);
            for (i, h) in homes.iter().enumerate() {
                if dist(Pos::from(h.pos), u.pos) <= PROTECT_M {
                    p[i] = p[i].max(v);
                }
            }
        }
        p
    }

    /// The district nearest `p`, by name.
    pub fn place_name(&self, p: Pos) -> &str {
        self.districts.iter().min_by(|a, b| dist(a.centre, p).total_cmp(&dist(b.centre, p))).map_or("", |d| d.name.as_str())
    }

    /// The fire is out, or the case has run its course; nobody is left on
    /// the road.
    pub fn is_over(&self) -> bool {
        if self.time_s() >= self.case.duration_s() {
            return true;
        }
        let moving = self.evac.active().is_some() || self.evac.families.iter().any(|f| f.state == State::Road);
        self.fire.active_cells().is_empty() && !moving
    }

    pub fn run_until(&mut self, t_s: i64) -> Result<()> {
        while self.time_s() < t_s {
            self.step()?;
        }
        Ok(())
    }

    /// Play to the end with no order: the yardstick of the score.
    pub fn without_orders(data_dir: &Path, case: &str, seed: u64) -> Result<Outcome> {
        let mut g = Game::new(data_dir, case, seed)?;
        while !g.is_over() {
            g.step()?;
        }
        Ok(g.outcome())
    }

    pub fn outcome(&self) -> Outcome {
        let ex = self.fire.exposure().fields();
        let districts = self
            .districts
            .iter()
            .map(|d| {
                let mut o = DistrictOutcome { homes: d.households.len(), ..Default::default() };
                for &i in &d.households {
                    let f = &self.evac.families[i];
                    o.homes_hit += ex[i].alight as usize;
                    o.people += f.people;
                    match f.state {
                        State::Caught => o.caught += f.people,
                        State::Safe => o.safe += f.people,
                        State::Road | State::Stuck => o.on_the_road += f.people,
                        State::SelfDefence => o.self_defence += f.people,
                        State::Home | State::Queued => {}
                    }
                }
                o
            })
            .collect();
        let burnt = self.fire.state().iter().filter(|c| **c != fire::CellFire::Unburnt).count();
        Outcome {
            at_s: self.time_s(),
            districts,
            hectares: burnt as f32 * self.scn.world.cellsize * self.scn.world.cellsize / 10_000.0,
            units_lost: self.crews.units.iter().filter(|u| u.state == UnitState::Lost).count(),
        }
    }
}
