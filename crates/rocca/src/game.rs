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
use abm::suppression::{Task, UnitEffect, UnitKind, UnitState};
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
/// ×120 or it is stopped, nothing else (user, 2026-10-14).
pub const RUN_X: f32 = 120.0;
/// Real seconds after a pause before the next can be asked (user,
/// 2026-10-14). The game only runs at [`RUN_X`] between pauses, so it is
/// kept as simulated time: [`PAUSE_COOLDOWN_S`].
pub const PAUSE_COOLDOWN_REAL_S: f32 = 10.0;
/// Simulated time after a pause before the next can be asked: 20 min.
pub const PAUSE_COOLDOWN_S: i64 = (PAUSE_COOLDOWN_REAL_S * RUN_X) as i64;
/// Spegnimento rispetto al modello calibrato (scelta dell'utente 2026-10-13:
/// linea ×3, acqua ×30, portata ×20).
pub const SUPPRESSION: UnitEffect = UnitEffect {
    line_x: 3.0,
    water_x: 30.0,
    reach_x: ENGINE_RANGE_M / abm::suppression::ENGINE_REACH_M,
    nerve_x: 1.0,
};
/// How far from where it actually stands an engine wets the fuel (user,
/// 2026-10-15; was 1.5 hexes = 450 m from the hex's road point). To reach the
/// fire it moves along the roads of its hexes of activity
/// ([`Game::engines_follow_the_fire`]).
pub const ENGINE_RANGE_M: f32 = 100.0;
/// How often a team's engine looks for a better place to stand, simulated seconds.
pub const ENGINE_LOOK_S: i64 = 60;
/// An engine will not park where the threat is above this.
pub const ENGINE_PARK_THREAT: f32 = 0.1;
/// ... nor closer than this to a burning cell, metres.
pub const ENGINE_PARK_FLAME_M: f32 = 80.0;
/// An engine that emptied its tank stays where it is and is full again after
/// this long, simulated: the water is brought to it (user, 2026-10-14).
pub const REFILL_S: f32 = 10.0 * 60.0;
/// How far from its hex's road point a team's people on foot go to work: the hexes
/// around its own (user, 2026-10-15; was 400 m with hexes of 500 m).
pub const CREW_RANGE_M: f32 = crate::hex::TEAM_REACH_HEXES as f32 * crate::hex::HEX_M;
/// How far from the nearest flame the team's line is cut, metres.
pub const CREW_LINE_OFFSET_M: f32 = 100.0;
/// Spacing of the points that trace the curve, metres.
pub const CREW_LINE_STEP_M: f32 = 30.0;
/// The people of a team fan out around the hex's road point by this much,
/// metres, and each then works on their own.
pub const CREW_SPREAD_M: f32 = 60.0;
/// Under this distance from the nearest flame the team's bar starts to shrink.
pub const TEAM_FIRE_CRIT_M: f32 = 300.0;
/// Under this distance (or when it withdraws) the team is in danger.
pub const TEAM_DANGER_M: f32 = 120.0;
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
    /// The people on foot, one unit each: they ride in the engine along the
    /// carriageways, get off where it stops, and cut the vegetation across the
    /// fire's way, each on their own (user, 2026-10-14 and 2026-10-15).
    pub crew_units: Vec<usize>,
    pub name: String,
    pub station: usize,
    pub volunteers: bool,
    pub crew: u8,
    /// The hex it was last sent to.
    pub hex: Option<Axial>,
    /// The order is waiting for the people still on foot to get back aboard.
    pub pending: Option<Pos>,
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
        // each team: its engine, then its people on foot (one unit each), at the same station
        let roster: Vec<(UnitKind, Pos)> = territory
            .roster
            .iter()
            .flat_map(|s| std::iter::once((s.kind.unit_kind(), stations[s.station])).chain((0..s.crew.max(1)).map(|_| (UnitKind::HandCrew, stations[s.station]))))
            .collect();
        let mut crews = Suppression::with_roster(&scn, &roster, policy)?;
        crews.effect = SUPPRESSION;
        crews.refill_in_place_s = Some(REFILL_S);
        let mut per_station = vec![0; territory.stations.len()];
        let mut next = 0;
        let teams: Vec<Team> = territory
            .roster
            .iter()
            .map(|s| {
                per_station[s.station] += 1;
                let st = &territory.stations[s.station];
                let name = format!("{} {}", if st.volunteers { "AIB" } else { "VVF" }, per_station[s.station]);
                let unit = next;
                crews.units[unit].callsign = name.clone();
                let crew_units: Vec<usize> = (unit + 1..unit + 1 + s.crew.max(1) as usize).collect();
                next = unit + 1 + crew_units.len();
                for (j, &c) in crew_units.iter().enumerate() {
                    let u = &mut crews.units[c];
                    u.callsign = format!("{name}, a piedi {}", j + 1);
                    u.carrier = Some(unit);
                    u.aboard = true;
                }
                Team { unit, crew_units, name, station: s.station, volunteers: st.volunteers, crew: s.crew, hex: None, pending: None }
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
            return Err(format!("la pausa sarà di nuovo disponibile tra {} minuti", (self.pause_ready_in_s() + 59) / 60));
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
        let way: Vec<_> = std::iter::once(from).chain(nodes.iter().copied()).collect();
        let secs = abm::drive::route_time_s(&self.net, u.pos, Some(u.heading), None, &way, abm::suppression::vehicle(u.kind));
        Ok(MovePlan { team: t, hex, path, minutes: secs / 60.0 })
    }

    /// Send team `t` to hex `at`: it drives there and works the fire around
    /// it. Only during a pause.
    pub fn move_team(&mut self, t: usize, at: Axial) -> Result<MovePlan, String> {
        if !self.paused() {
            return Err("gli ordini si danno solo in pausa".into());
        }
        let plan = self.plan_move(t, at)?;
        let unit = self.teams[t].unit;
        if self.crews.units[unit].state == UnitState::Lost {
            return Err(format!("{} è fuori servizio", self.teams[t].name));
        }
        // the engine leaves when its people are aboard; they work from where it stops
        self.teams[t].pending = Some(plan.hex.target);
        self.dispatch(t);
        self.teams[t].hex = Some(at);
        let text = format!("{} verso {} ({} min)", self.teams[t].name, self.place_name(plan.hex.target), plan.minutes.ceil());
        self.log.push(LogEntry { at_s: self.time_s(), text });
        Ok(plan)
    }

    /// Carry out the order waiting for team `t`: the people left on foot walk
    /// back to the engine, and when all are aboard it drives off and they
    /// spread around the hex's road point.
    fn dispatch(&mut self, t: usize) {
        let Some(target) = self.teams[t].pending else { return };
        let alive: Vec<usize> = self.teams[t].crew_units.iter().copied().filter(|&c| self.crews.units[c].state != UnitState::Lost).collect();
        let mut all_aboard = true;
        for &c in &alive {
            let u = &self.crews.units[c];
            if !u.aboard {
                all_aboard = false;
                if u.task != Task::Return {
                    let _ = self.crews.assign(c, Task::Return);
                }
            }
        }
        if !all_aboard {
            return;
        }
        self.teams[t].pending = None;
        let _ = self.crews.assign(self.teams[t].unit, Task::Attack { at: target });
        let n = alive.len();
        for (j, &c) in alive.iter().enumerate() {
            let a = std::f32::consts::TAU * j as f32 / n.max(1) as f32;
            let off = if n > 1 { CREW_SPREAD_M } else { 0.0 };
            let _ = self.crews.assign(c, Task::Attack { at: Pos { x: target.x + off * a.cos(), y: target.y + off * a.sin() } });
        }
    }

    /// The road still ahead of team `t`.
    pub fn team_route(&self, t: usize) -> Vec<Pos> {
        self.crews.route_points(self.teams[t].unit, &self.net)
    }

    /// Minutes of driving left for team `t`, if it is driving.
    pub fn team_eta_min(&self, t: usize) -> Option<f32> {
        let s = self.crews.route_remaining_s(self.teams[t].unit, &self.net);
        (s > 1.0).then(|| s / 60.0)
    }

    /// Metres from team `t`'s engine to the nearest flame; `None` when nothing burns.
    pub fn team_fire_distance_m(&self, t: usize) -> Option<f32> {
        let p = self.crews.units[self.teams[t].unit].pos;
        let w = &self.scn.world;
        let d2 = self.fire.active_cells().iter().map(|c| district::dist(w.centre_of(*c), p).powi(2)).fold(f32::INFINITY, f32::min);
        d2.is_finite().then(|| d2.sqrt())
    }

    /// Team `t` is in danger: withdrawing, or the fire is within `TEAM_DANGER_M`.
    pub fn team_in_danger(&self, t: usize) -> bool {
        let u = &self.crews.units[self.teams[t].unit];
        if matches!(u.state, UnitState::Lost | UnitState::Staged | UnitState::Unavailable | UnitState::Inbound) && self.teams[t].hex.is_none() {
            return false;
        }
        u.state == UnitState::Withdrawing || self.team_fire_distance_m(t).is_some_and(|d| d < TEAM_DANGER_M)
    }

    /// Homes of district `d` the fire has reached, and all its homes.
    pub fn district_damage(&self, d: usize) -> (usize, usize) {
        let ex = self.fire.exposure().fields();
        let hh = &self.districts[d].households;
        (hh.iter().filter(|&&i| ex[i].alight).count(), hh.len())
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
        for t in 0..self.teams.len() {
            self.dispatch(t);
        }
        for a in self.crews.step(STEP_S as f32, &self.net, &self.traffic, &self.fire, &self.scn) {
            self.fire.queue(a);
        }
        self.crews_line_up();
        self.engines_follow_the_fire();
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

    /// The people of a team cut **one line together**: a curve at
    /// [`CREW_LINE_OFFSET_M`] from the fire, wrapping round the front next to
    /// the hex's road point, split into consecutive stretches, one per person
    /// (user, 2026-10-15). It is drawn when the engine has arrived, and again
    /// when everyone has finished, on the fire as it is then.
    fn crews_line_up(&mut self) {
        for t in 0..self.teams.len() {
            let team = &self.teams[t];
            let Some(at) = team.hex else { continue };
            if team.pending.is_some() || self.crews.units[team.unit].state != UnitState::Working {
                continue;
            }
            let alive: Vec<usize> = team.crew_units.iter().copied().filter(|&c| self.crews.units[c].state != UnitState::Lost).collect();
            let unplanned = alive.iter().any(|&c| matches!(self.crews.units[c].task, Task::Attack { .. }));
            let finished = !alive.is_empty() && alive.iter().all(|&c| {
                let u = &self.crews.units[c];
                !u.aboard && u.task == Task::Hold && u.note == "line complete"
            });
            if !unplanned && !finished {
                continue;
            }
            let Some(hex) = self.hexes.get(at) else { continue };
            let Some(stretches) = self.team_line(hex.target, &alive) else { continue };
            for (&c, (from, to)) in alive.iter().zip(stretches) {
                let _ = self.crews.assign(c, Task::Line { from, to });
            }
        }
    }

    /// The stretches of the curve for `crew`, one each, in the order that
    /// keeps the people from crossing. None when the fire is out or too far.
    fn team_line(&self, target: Pos, crew: &[usize]) -> Option<Vec<(Pos, Pos)>> {
        let world = self.scn.world;
        let active: Vec<Pos> = self.fire.active_cells().iter().map(|c| world.centre_of(*c)).collect();
        let nearest = |p: Pos| active.iter().copied().min_by(|a, b| dist(*a, p).total_cmp(&dist(*b, p)));
        let n = crew.len();
        if n == 0 {
            return None;
        }
        // The anchor: on the way from the nearest flame to the road point.
        let f = nearest(target)?;
        let d = dist(f, target);
        if d < 1.0 {
            return None;
        }
        let anchor = Pos { x: f.x + (target.x - f.x) / d * CREW_LINE_OFFSET_M, y: f.y + (target.y - f.y) / d * CREW_LINE_OFFSET_M };
        let total = n as f32 * abm::suppression::LINE_M_PER_H * 1.5;
        // Walk along the level curve of the distance from the fire, both ways.
        let walk = |sign: f32| -> Vec<Pos> {
            let mut pts = vec![anchor];
            let mut p = anchor;
            let mut len = 0.0;
            while len < total {
                let Some(f) = nearest(p) else { break };
                let dd = dist(f, p).max(1.0);
                let (gx, gy) = ((p.x - f.x) / dd, (p.y - f.y) / dd);
                let step = CREW_LINE_STEP_M;
                let mut q = Pos { x: p.x - gy * sign * step, y: p.y + gx * sign * step };
                // Back onto the curve.
                if let Some(fq) = nearest(q) {
                    let dq = dist(fq, q).max(1.0);
                    let k = (CREW_LINE_OFFSET_M - dq) / dq;
                    q = Pos { x: q.x + (q.x - fq.x) * k, y: q.y + (q.y - fq.y) * k };
                }
                if !world.contains(q) || dist(q, target) > CREW_RANGE_M {
                    break;
                }
                len += dist(p, q);
                pts.push(q);
                p = q;
            }
            pts
        };
        let (a, b) = (walk(1.0), walk(-1.0));
        let length = |v: &[Pos]| v.windows(2).map(|w| dist(w[0], w[1])).sum::<f32>();
        let (la, lb) = (length(&a), length(&b));
        let take_a = la.min((total / 2.0).max(total - lb));
        let take_b = lb.min(total - take_a);
        let clip = |v: Vec<Pos>, max: f32| -> Vec<Pos> {
            let mut out = vec![v[0]];
            let mut run = 0.0;
            for w in v.windows(2) {
                let seg = dist(w[0], w[1]);
                if run + seg >= max {
                    let k = if seg > 0.0 { (max - run) / seg } else { 0.0 };
                    out.push(Pos { x: w[0].x + (w[1].x - w[0].x) * k, y: w[0].y + (w[1].y - w[0].y) * k });
                    return out;
                }
                run += seg;
                out.push(w[1]);
            }
            out
        };
        let mut curve: Vec<Pos> = clip(b, take_b).into_iter().rev().collect();
        curve.extend(clip(a, take_a).into_iter().skip(1));
        let len = length(&curve);
        if len < 20.0 {
            return None;
        }
        // The point at arc length `s`.
        let at = |s: f32| -> Pos {
            let mut run = 0.0;
            for w in curve.windows(2) {
                let seg = dist(w[0], w[1]);
                if run + seg >= s {
                    let k = if seg > 0.0 { (s - run) / seg } else { 0.0 };
                    return Pos { x: w[0].x + (w[1].x - w[0].x) * k, y: w[0].y + (w[1].y - w[0].y) * k };
                }
                run += seg;
            }
            *curve.last().unwrap()
        };
        // People in order along the curve, from its start.
        let (c0, c1) = (curve[0], *curve.last().unwrap());
        let (ux, uy) = ((c1.x - c0.x) / dist(c0, c1).max(1.0), (c1.y - c0.y) / dist(c0, c1).max(1.0));
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&i, &j| {
            let key = |k: usize| {
                let p = self.crews.units[crew[k]].pos;
                (p.x - c0.x) * ux + (p.y - c0.y) * uy
            };
            key(i).total_cmp(&key(j))
        });
        let mut out = vec![(c0, c0); n];
        for (rank, &k) in order.iter().enumerate() {
            out[k] = (at(len * rank as f32 / n as f32), at(len * (rank + 1) as f32 / n as f32));
        }
        Some(out)
    }

    /// An engine with nothing to wet within [`ENGINE_RANGE_M`] drives, on the
    /// roads of its hexes of activity (its own and the ring around it), to the
    /// place from which it has the most threatened fuel in reach, or, if the
    /// fire is not yet near, the one nearest to the fire.
    fn engines_follow_the_fire(&mut self) {
        if self.time_s() % ENGINE_LOOK_S != 0 {
            return;
        }
        let world = self.scn.world;
        let work_at = |g: &Game, p: Pos| -> f32 {
            fire::cells_in_radius(&world, p, ENGINE_RANGE_M)
                .into_iter()
                .filter(|c| g.fire.is_suppressible(*c, &g.scn))
                .map(|c| g.fire.threat().at(world.centre_of(c)))
                .sum()
        };
        for t in 0..self.teams.len() {
            let team = &self.teams[t];
            let Some(at) = team.hex else { continue };
            if team.pending.is_some() {
                continue;
            }
            let u = &self.crews.units[team.unit];
            if u.state != UnitState::Working || !matches!(u.task, Task::Attack { .. }) || u.aboard {
                continue;
            }
            let here = u.pos;
            if work_at(self, here) > 0.0 {
                continue;
            }
            let active: Vec<Pos> = self.fire.active_cells().iter().map(|c| world.centre_of(*c)).collect();
            if active.is_empty() {
                continue;
            }
            let Some(hex) = self.hexes.get(at) else { continue };
            let comp = self.net.component(hex.node, true);
            let mut best: Option<(f32, Pos)> = None;
            for (i, &p) in self.net.nodes.iter().enumerate() {
                let n = i as network::NodeId;
                if !self.net.is_drivable_node(n) || Axial::of(p).distance(at) > crate::hex::TEAM_REACH_HEXES || self.net.component(n, true) != comp || !world.contains(p) {
                    continue;
                }
                let nearest = active.iter().map(|f| dist(*f, p)).fold(f32::INFINITY, f32::min);
                if self.fire.threat().at(p) >= ENGINE_PARK_THREAT || nearest < ENGINE_PARK_FLAME_M {
                    continue;
                }
                let w = work_at(self, p);
                // Work in reach beats everything; otherwise the nearest to the fire.
                let score = if w > 0.0 { 1.0e6 + w } else { -nearest };
                if best.map_or(true, |(b, _)| score > b) {
                    best = Some((score, p));
                }
            }
            if let Some((_, p)) = best {
                if dist(p, here) > 30.0 {
                    let unit = self.teams[t].unit;
                    let _ = self.crews.assign(unit, Task::Attack { at: p });
                }
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

    /// An evacuation was ordered and it is finished: every family has left
    /// and reached the refuge (or was cut off). The tutorial ends here.
    pub fn evacuation_finished(&self) -> bool {
        !self.evac.runs.is_empty() && self.evac.active().is_none() && !self.evac.families.iter().any(|f| f.state == State::Road)
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
