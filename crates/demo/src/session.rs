//! The turn engine (`docs/demo-spec-gameplay.md` §1, §3, §4, §6): one session
//! of Rocca Ventosa, five turns of eight simulated minutes, played by giving
//! tokens to targets and pressing *Avanti*.
//!
//! **One set of books.** A [`Session`] owns the headless [`Run`] (fire, agents,
//! units and the [`crate::Referee`] that counts them) and the turn state on top
//! of it. The `play` binary, the tests and the kiosk all drive a `Session`
//! through the same methods in the same order, and the counterfactual is
//! another `Session` with no orders -- so "against no orders" is counted by
//! the code that counted the player.
//!
//! **One stepping path** (finding 5): [`Session::end_turn`] applies the turn's
//! orders, then steps the model in [`crate::STEP_S`] chunks to the end of the turn.
//! Anything the turn layer needs to notice (the patrol arriving, the Canadair
//! dropping) is checked after each step, at the step's time.
//!
//! **Inert until used** (finding 34): a session in which nobody gives an
//! order is bit-for-bit the counterfactual run (`tests/lessons.rs` pins it).
//! The patrol, the reinforcement and the single Canadair call only touch the
//! model when a token is given an order.

use std::path::{Path, PathBuf};

use abm::network;
use abm::suppression::{
    Task, UnitKind, UnitState, AIR_RESPONSE_S, CREW_SPEED, CREW_WALK_SPEED, DROP_WIDTH_M, ENGINE_REACH_M, ENGINE_SPEED,
    HYDRANT_LPM, SCOOP_S, TANKER_SPEED, WORK_LIMIT,
};
use anyhow::Result;
use fire::CellFire;
use scenario::Pos;

use crate::district::Level;
use crate::event::EventKind;
use crate::run::{Order, Run, Variant, DEFEND_REACH_M};
use crate::turn::*;
use crate::weather::{draw, Draw, Forecast, ISSUE_2_AT_S};

/// The one scenario the game plays (demo-spec §2).
pub const TOWN: &str = "demo_borgo";

/// A warning landed at least this long before the fire reached a district is
/// in time (the end card's rule since v1: about the median family's preparation).
pub const IN_TIME_S: i64 = crate::district::IN_TIME_MIN * 60;

/// The model options the turn game runs with: home defence (option B) and one
/// fire station. Both are existing, inert-by-default variants.
pub fn variant() -> Variant {
    Variant { defend_homes: true, station: true, ..Variant::default() }
}

/// What "the head" means as an order (gameplay §7.6 sweep).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeadOrder {
    /// The unit works the head as it was when the order was given (the
    /// target's position at the turn opening, like every other target).
    #[default]
    Fixed,
    /// The unit keeps going for the head as it moves: re-tasked to the
    /// current head every [`HEAD_TRACK_S`]. A flag for the lead (milestone 0):
    /// not the default.
    Track,
}

/// How often a unit ordered to the head is re-tasked under [`HeadOrder::Track`].
pub const HEAD_TRACK_S: i64 = 60;

#[derive(Debug, Clone)]
struct Patrol {
    at: Pos,
    path: Vec<Pos>,
    depart_s: i64,
    arrive_s: i64,
    to: Option<usize>,
    delivered: bool,
    /// Whether the forecast pointed at the district when the order was given.
    pointed: bool,
}

#[derive(Debug, Clone, Copy)]
struct Doing {
    kind: TargetKind,
    pos: Pos,
}

#[derive(Debug, Clone)]
struct SpotTrack {
    first: Pos,
    /// Where it is burning now (a cell of its own fire).
    now: Pos,
    /// When it went out (`false`) or joined the main fire (`true`).
    gone: Option<(i64, bool)>,
    /// A token was ordered onto it.
    worked: bool,
}

/// Something that happened during the turn being played, for the report.
#[derive(Debug, Clone, Copy)]
enum Happening {
    Arrived(TokenId, Pos),
    Withdrew(TokenId, Pos),
    Lost(TokenId, Pos),
    PatrolArrived(usize, u32),
    OnStation(Pos),
    Drop(Pos),
    SpotOut(Pos),
}

pub struct Session {
    pub run: Run,
    pub draw: Draw,
    /// See [`HeadOrder`]. `Fixed` unless a sweep sets it.
    pub head_order: HeadOrder,
    /// The hand crew is in the tray. On (the spec's tray); milestone 0
    /// measured it changing nothing (gameplay §7.1), so the lead may cut it.
    pub crew: bool,
    /// Spot fires are targets. On (the spec's map); milestone 0 measured
    /// attacking one changing nothing (gameplay §7.3), so the lead may cut it.
    pub spot_targets: bool,
    /// The cells the opening ignition lights. The core only lights them on
    /// its first advance, so at T+0 nothing is burning yet; until then these
    /// stand for the fire (the map draws them, the head and flanks are on them).
    patch: Vec<scenario::Cell>,
    data_dir: PathBuf,
    turn: u8,
    pending: Vec<(TokenId, TargetId)>,
    doing: [Option<Doing>; 7],
    patrol: Patrol,
    it_alert: Option<u8>,
    called: Option<u8>,
    /// The turn each token last pulled back in.
    withdrew: [Option<u8>; 7],
    /// The token has started work on its current order (for "arrived").
    started: [bool; 7],
    targets: Vec<Target>,
    spots: Vec<SpotTrack>,
    spot_cursor: usize,
    report: TurnReport,
    /// Every order played: (turn, token, target).
    log: Vec<(u8, TokenId, TargetKind)>,
    /// Districts an engine was sent to while nothing pointed the fire at them
    /// (the preview said *Inutile*).
    needless_defence: Vec<usize>,
    /// Per district: whether the forecast pointed at it when the order that
    /// first warned it was given.
    pointed: Vec<Option<bool>>,
    happenings: Vec<Happening>,
    unit_prev: Vec<(UnitState, u32)>,
    ev_cursor: usize,
    drops: u32,
    drops_by_last_turn: u32,
    head_withdrew: Vec<TokenId>,
    lost: Vec<TokenId>,
    none: Option<Counterfactual>,
}

fn dist(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

impl Session {
    /// A new session on Rocca Ventosa with this seed's weather.
    pub fn new(data_dir: &Path, seed: u64) -> Result<Session> {
        let d = draw(TOWN, seed).expect("the demo town has a spec");
        Session::from_draw(data_dir, d)
    }

    pub fn from_draw(data_dir: &Path, draw: Draw) -> Result<Session> {
        Session::with_variant(data_dir, draw, variant())
    }

    /// A session on a model variant (sweeps only; the game plays [`variant`]).
    pub fn with_variant(data_dir: &Path, draw: Draw, v: Variant) -> Result<Session> {
        let run = Run::with_variant(data_dir, draw.spec, draw.seed, v)?;
        let station = run.crews.units[0].base;
        let nd = run.referee.districts.len();
        let patch = {
            let w = &run.scn.world;
            let c = w.cell_of(draw.spec.ignition);
            let r = (draw.spec.radius_m / w.cellsize).ceil() as i64;
            let mut v = vec![];
            for dr in -r..=r {
                for dc in -r..=r {
                    let (row, col) = (c.row as i64 + dr, c.col as i64 + dc);
                    if dr * dr + dc * dc > r * r || row < 0 || col < 0 || row >= w.fire_rows as i64 || col >= w.fire_cols as i64 {
                        continue;
                    }
                    let cell = scenario::Cell { row: row as usize, col: col as usize };
                    if run.scn.is_burnable(cell) {
                        v.push(cell);
                    }
                }
            }
            v
        };
        let mut s = Session {
            head_order: HeadOrder::Fixed,
            crew: true,
            spot_targets: true,
            patch,
            unit_prev: run.crews.units.iter().map(|u| (u.state, u.drops)).collect(),
            run,
            draw,
            data_dir: data_dir.to_path_buf(),
            turn: 1,
            pending: vec![],
            doing: [None; 7],
            patrol: Patrol { at: station, path: vec![], depart_s: 0, arrive_s: 0, to: None, delivered: true, pointed: false },
            it_alert: None,
            called: None,
            withdrew: [None; 7],
            started: [false; 7],
            targets: vec![],
            spots: vec![],
            spot_cursor: 0,
            report: TurnReport::default(),
            log: vec![],
            needless_defence: vec![],
            pointed: vec![None; nd],
            happenings: vec![],
            ev_cursor: 0,
            drops: 0,
            drops_by_last_turn: 0,
            head_withdrew: vec![],
            lost: vec![],
            none: None,
        };
        s.open();
        Ok(s)
    }

    pub fn seed(&self) -> u64 {
        self.draw.seed
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    // --- the clock and the weather ------------------------------------------

    pub fn turn(&self) -> Turn {
        Turn { index: self.turn, at_s: self.run.time_s(), last: self.turn == TURNS }
    }

    pub fn time_s(&self) -> i64 {
        self.run.time_s()
    }

    /// The fire has played out to [`MISSION_S`]: the verdict is final.
    pub fn finished(&self) -> bool {
        self.run.time_s() >= MISSION_S
    }

    /// The forecast in force now: issue 2 from turn 2 (T+8).
    pub fn forecast(&self) -> Forecast {
        self.forecast_at(self.run.time_s())
    }

    fn forecast_at(&self, t: i64) -> Forecast {
        self.draw.forecast(if t >= ISSUE_2_AT_S { 2 } else { 1 })
    }

    /// The wind now: (from, km/h). From the model, so after a shift it says so.
    pub fn wind(&self) -> (f32, f32) {
        let w = self.run.fire.weather();
        (w.wind_dir_deg as f32, w.wind_speed_kmh as f32)
    }

    /// Whether the wind has turned since the start.
    pub fn wind_turned(&self) -> bool {
        self.run.referee.events.iter().any(|e| matches!(e.kind, EventKind::WindShifted { .. }))
    }

    // --- the tray --------------------------------------------------------------

    pub fn tokens(&self) -> Vec<Token> {
        TokenId::ALL.iter().filter(|t| self.in_tray(**t)).map(|&t| self.token(t)).collect()
    }

    /// The reinforcement joins the tray at turn 3.
    pub fn in_tray(&self, t: TokenId) -> bool {
        (t != TokenId::E3 || self.turn >= REINFORCEMENT_TURN) && (t != TokenId::S || self.crew)
    }

    pub fn token(&self, id: TokenId) -> Token {
        let state = self.token_state(id);
        let unit = id.unit().map(|k| &self.run.crews.units[k]);
        let at = match id {
            TokenId::P => self.patrol.at,
            TokenId::I => self.town_centre(),
            _ => unit.map(|u| u.pos).unwrap_or(self.station()),
        };
        Token {
            id,
            kind: id.kind(),
            state,
            water: unit.filter(|u| u.kind == UnitKind::Engine).map(|u| u.water_frac()),
            at,
            order: self.pending.iter().find(|(t, _)| *t == id).map(|(_, g)| *g),
            doing: match id {
                TokenId::P => self.patrol.to.filter(|_| !self.patrol.delivered).map(TargetKind::District),
                _ => self.doing[id.index()].map(|d| d.kind),
            },
            orderable: self.orderable(id, state),
        }
    }

    fn station(&self) -> Pos {
        self.run.crews.units[0].base
    }

    fn town_centre(&self) -> Pos {
        let ds = &self.run.referee.districts;
        let n = ds.len().max(1) as f32;
        Pos { x: ds.iter().map(|d| d.centre.x).sum::<f32>() / n, y: ds.iter().map(|d| d.centre.y).sum::<f32>() / n }
    }

    pub fn token_state(&self, id: TokenId) -> TokenState {
        let now = self.run.time_s();
        match id {
            TokenId::P => {
                if self.patrol.to.is_some() && !self.patrol.delivered {
                    TokenState::InViaggio { eta_s: (self.patrol.arrive_s - now).max(0) }
                } else {
                    TokenState::Libero
                }
            }
            TokenId::I => {
                if self.it_alert.is_some() {
                    TokenState::Usato
                } else {
                    TokenState::Libero
                }
            }
            _ => {
                let k = id.unit().expect("a unit token");
                let u = &self.run.crews.units[k];
                if u.state == UnitState::Lost {
                    return TokenState::Perso;
                }
                if u.state == UnitState::Withdrawing || (self.turn > 1 && self.withdrew[id.index()] == Some(self.turn - 1)) {
                    return TokenState::Ritirato;
                }
                match u.state {
                    UnitState::Unavailable => TokenState::NonChiamato,
                    UnitState::Inbound => TokenState::InArrivo { eta_s: (u.arrives_at_s - self.run.crews.time_s()).ceil().max(0.0) as i64 },
                    UnitState::Staged if id == TokenId::K && self.doing[id.index()].is_some() => TokenState::AlLavoro,
                    UnitState::Staged => TokenState::Libero,
                    UnitState::Working => TokenState::AlLavoro,
                    UnitState::Moving => TokenState::InViaggio { eta_s: self.moving_eta(k) },
                    UnitState::Refilling => TokenState::Rifornimento { eta_s: self.refill_eta(k) },
                    UnitState::Withdrawing => TokenState::Ritirato,
                    UnitState::Lost => TokenState::Perso,
                }
            }
        }
    }

    fn orderable(&self, id: TokenId, state: TokenState) -> bool {
        if self.turn > TURNS || !self.in_tray(id) {
            return false;
        }
        match state {
            TokenState::Libero | TokenState::InViaggio { .. } | TokenState::AlLavoro | TokenState::NonChiamato => true,
            // A plane that will be overhead during the coming turn can be
            // briefed on the way in (`Suppression::assign` allows it); one
            // that will not cannot be given anything yet.
            TokenState::InArrivo { eta_s } => eta_s <= TURN_S,
            TokenState::Rifornimento { .. } | TokenState::Ritirato | TokenState::Perso | TokenState::Usato => false,
        }
    }

    fn moving_eta(&self, k: usize) -> i64 {
        let u = &self.run.crews.units[k];
        let speed = match u.kind {
            UnitKind::Engine => ENGINE_SPEED,
            UnitKind::HandCrew => CREW_SPEED,
            UnitKind::AirTanker => TANKER_SPEED,
        };
        let focus = u.task.focus().unwrap_or(u.pos);
        let road = self.run.crews.route_remaining_m(k, &self.run.agents.network);
        let s = match u.kind {
            UnitKind::AirTanker => dist(u.pos, focus) / TANKER_SPEED + if u.water_l <= 0.0 { SCOOP_S } else { 0.0 },
            UnitKind::HandCrew if road <= 0.0 => dist(u.pos, focus) / CREW_WALK_SPEED,
            _ => road / speed,
        };
        s.ceil() as i64
    }

    fn refill_eta(&self, k: usize) -> i64 {
        let u = &self.run.crews.units[k];
        if u.kind.is_air() {
            return SCOOP_S as i64;
        }
        let drive = self.run.crews.nearest_hydrant(u.pos).map_or(0.0, |h| dist(u.pos, h) / ENGINE_SPEED);
        (drive + (u.tank_l - u.water_l).max(0.0) / (HYDRANT_LPM / 60.0)).ceil() as i64
    }

    // --- targets ---------------------------------------------------------------

    /// Every target on the map this turn.
    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    pub fn target(&self, id: TargetId) -> Option<&Target> {
        self.targets.iter().find(|t| t.id == id)
    }

    pub fn target_of(&self, kind: TargetKind) -> Option<&Target> {
        self.targets.iter().find(|t| t.kind == kind)
    }

    /// The targets this token may be given now (the lit ones).
    pub fn valid_targets(&self, token: TokenId) -> Vec<Target> {
        self.targets.iter().filter(|t| self.valid(token, t.kind)).copied().collect()
    }

    fn valid(&self, token: TokenId, kind: TargetKind) -> bool {
        use TargetKind as K;
        match token.kind() {
            TokenKind::Pattuglia => matches!(kind, K::District(_)),
            TokenKind::ItAlert => kind == K::Town,
            TokenKind::Autobotte | TokenKind::Squadra => matches!(kind, K::District(_) | K::Head | K::Flank(_) | K::SpotFire(_)),
            TokenKind::Canadair => {
                if self.run.crews.units[6].state == UnitState::Unavailable {
                    kind == K::Sky
                } else {
                    matches!(kind, K::District(_) | K::Head | K::Flank(_) | K::SpotFire(_))
                }
            }
        }
    }

    fn target_ids(&self) -> (u8, u8) {
        let nd = self.run.referee.districts.len() as u8;
        (nd, nd + 4)
    }

    /// Target numbering: districts 1..=n, then head, left flank, right flank,
    /// then spot fires; the Canadair call and the IT-alert have their own.
    fn id_of(&self, kind: TargetKind) -> TargetId {
        let (nd, spot0) = self.target_ids();
        TargetId(match kind {
            TargetKind::District(d) => 1 + d as u8,
            TargetKind::Head => nd + 1,
            TargetKind::Flank(Side::Left) => nd + 2,
            TargetKind::Flank(Side::Right) => nd + 3,
            TargetKind::SpotFire(n) => spot0 + n as u8,
            TargetKind::Sky => 200,
            TargetKind::Town => 201,
        })
    }

    /// Compute this turn's targets from the model as it stands.
    fn compute_targets(&mut self) {
        let mut out = vec![];
        let head = self.head();
        for (k, d) in self.run.referee.districts.iter().enumerate() {
            let pos = d.post_facing(&self.run.agents, head, &[], 2.0 * DEFEND_REACH_M);
            out.push(Target { id: TargetId(0), kind: TargetKind::District(k), pos, label_pos: d.centre, facing_deg: None });
        }
        if !self.burning_cells().is_empty() {
            out.push(Target { id: TargetId(0), kind: TargetKind::Head, pos: head, label_pos: head, facing_deg: None });
            if let Some(((l, ld), (r, rd))) = self.flanks() {
                out.push(Target { id: TargetId(0), kind: TargetKind::Flank(Side::Left), pos: l, label_pos: l, facing_deg: Some(ld) });
                out.push(Target { id: TargetId(0), kind: TargetKind::Flank(Side::Right), pos: r, label_pos: r, facing_deg: Some(rd) });
            }
        }
        for (n, s) in self.spots.iter().enumerate() {
            if s.gone.is_none() && self.spot_targets {
                out.push(Target { id: TargetId(0), kind: TargetKind::SpotFire(n), pos: s.now, label_pos: s.now, facing_deg: None });
            }
        }
        if self.run.crews.units[6].state == UnitState::Unavailable {
            let p = self.station();
            out.push(Target { id: TargetId(0), kind: TargetKind::Sky, pos: p, label_pos: p, facing_deg: None });
        }
        if self.it_alert.is_none() {
            let c = self.town_centre();
            out.push(Target { id: TargetId(0), kind: TargetKind::Town, pos: c, label_pos: c, facing_deg: None });
        }
        for t in &mut out {
            t.id = self.id_of(t.kind);
        }
        self.targets = out;
    }

    /// The cells burning now; at T+0, before the core's first advance, the
    /// ignition patch that is about to.
    pub fn burning_cells(&self) -> Vec<scenario::Cell> {
        let a = self.run.fire.active_cells();
        if a.is_empty() && self.run.time_s() == 0 {
            self.patch.clone()
        } else {
            a.to_vec()
        }
    }

    /// Whether the fire has burnt this cell (or, at T+0, is about to).
    pub fn burnt_or_burning(&self, i: usize) -> bool {
        self.run.fire.state()[i] != CellFire::Unburnt
    }

    /// The head of the main fire: its burning cell furthest downwind. Not
    /// `Run::head`, which counts spot fires too and so would call a spot
    /// 400 m ahead "the head".
    pub fn head(&self) -> Pos {
        let (from, _) = self.wind();
        let to = (from + 180.0).to_radians();
        let (ux, uy) = (to.sin(), to.cos());
        let o = self.run.spec.ignition;
        let w = &self.run.scn.world;
        let main = self.main_fire_mask();
        self.burning_cells()
            .iter()
            .filter(|c| main[c.row * w.fire_cols + c.col])
            .map(|c| w.centre_of(*c))
            .max_by(|a, b| {
                let pa = (a.x - o.x) * ux + (a.y - o.y) * uy;
                let pb = (b.x - o.x) * ux + (b.y - o.y) * uy;
                pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or_else(|| self.run.head())
    }

    /// The fire's flanks: perpendicular to the spread direction, a third of
    /// the way back from the head (gameplay §4). Returns (left, right), each
    /// with the compass bearing from the fire's axis out to it.
    fn flanks(&self) -> Option<((Pos, f32), (Pos, f32))> {
        let (from, _) = self.wind();
        let to = (from + 180.0).to_radians();
        let (ux, uy) = (to.sin(), to.cos());
        let (rx, ry) = (uy, -ux);
        let o = self.run.spec.ignition;
        let w = &self.run.scn.world;
        let state = self.run.fire.state();
        let main = self.main_fire_mask();
        let s = |p: Pos| (p.x - o.x) * ux + (p.y - o.y) * uy;
        let t = |p: Pos| (p.x - o.x) * rx + (p.y - o.y) * ry;
        let (mut smin, mut smax) = (f32::INFINITY, f32::NEG_INFINITY);
        for (i, c) in state.iter().enumerate() {
            if (*c != CellFire::Unburnt || self.run.time_s() == 0) && main[i] {
                let p = w.centre_of(scenario::Cell { row: i / w.fire_cols, col: i % w.fire_cols });
                smin = smin.min(s(p));
                smax = smax.max(s(p));
            }
        }
        if !smin.is_finite() {
            return None;
        }
        let at = smax - (smax - smin) / 3.0;
        let active: Vec<Pos> = self
            .burning_cells()
            .iter()
            .filter(|c| main[c.row * w.fire_cols + c.col])
            .map(|c| w.centre_of(*c))
            .collect();
        for band in [60.0, 120.0, 240.0, f32::INFINITY] {
            let near: Vec<Pos> = active.iter().copied().filter(|p| (s(*p) - at).abs() <= band).collect();
            if near.len() >= 2 {
                let cmp = |a: &Pos, b: &Pos| t(*a).partial_cmp(&t(*b)).unwrap_or(std::cmp::Ordering::Equal);
                let l = *near.iter().min_by(|a, b| cmp(a, b))?;
                let r = *near.iter().max_by(|a, b| cmp(a, b))?;
                if dist(l, r) < 1.0 {
                    return None;
                }
                let bearing = |dx: f32, dy: f32| dx.atan2(dy).to_degrees().rem_euclid(360.0);
                return Some(((l, bearing(-rx, -ry)), (r, bearing(rx, ry))));
            }
        }
        None
    }

    /// Cells (burning or burnt) connected to the opening ignition: the main
    /// fire, as opposed to spot fires that have not joined it.
    fn main_fire_mask(&self) -> Vec<bool> {
        let w = &self.run.scn.world;
        if self.run.time_s() == 0 && self.run.fire.active_cells().is_empty() {
            let mut m = vec![false; w.fire_rows * w.fire_cols];
            for c in &self.patch {
                m[c.row * w.fire_cols + c.col] = true;
            }
            return m;
        }
        let (labels, _) = self.components();
        let main = self.main_label(&labels);
        labels.iter().map(|&l| l != 0 && l == main).collect()
    }

    /// The component label of the main fire: the one most of the ignition
    /// patch belongs to. Not the label of the ignition point's own cell, which
    /// may be a road or a field and so never burn (finding 2's shape).
    fn main_label(&self, labels: &[u32]) -> u32 {
        let cols = self.run.scn.world.fire_cols;
        let mut count: Vec<(u32, usize)> = vec![];
        for c in &self.patch {
            let l = labels[c.row * cols + c.col];
            if l == 0 {
                continue;
            }
            match count.iter_mut().find(|(k, _)| *k == l) {
                Some(e) => e.1 += 1,
                None => count.push((l, 1)),
            }
        }
        count.into_iter().max_by_key(|(l, n)| (*n, std::cmp::Reverse(*l))).map_or(0, |(l, _)| l)
    }

    /// 8-connected components of the fire's footprint. Labels from 1; 0 is
    /// unburnt. Also whether each component still has a burning cell.
    fn components(&self) -> (Vec<u32>, Vec<bool>) {
        let w = &self.run.scn.world;
        let (rows, cols) = (w.fire_rows, w.fire_cols);
        let state = self.run.fire.state();
        let mut label = vec![0u32; rows * cols];
        let mut burning = vec![false];
        let mut next = 0u32;
        let mut stack = vec![];
        for start in 0..rows * cols {
            if state[start] == CellFire::Unburnt || label[start] != 0 {
                continue;
            }
            next += 1;
            burning.push(false);
            label[start] = next;
            stack.push(start);
            while let Some(i) = stack.pop() {
                if state[i] == CellFire::Burning {
                    burning[next as usize] = true;
                }
                let (r, c) = ((i / cols) as i64, (i % cols) as i64);
                for dr in -1..=1i64 {
                    for dc in -1..=1i64 {
                        let (rr, cc) = (r + dr, c + dc);
                        if rr < 0 || cc < 0 || rr >= rows as i64 || cc >= cols as i64 {
                            continue;
                        }
                        let j = rr as usize * cols + cc as usize;
                        if state[j] != CellFire::Unburnt && label[j] == 0 {
                            label[j] = next;
                            stack.push(j);
                        }
                    }
                }
            }
        }
        (label, burning)
    }

    /// Fold new spot fires into the list and decide which have gone out or
    /// joined the main fire (finding 37: one per connected component).
    fn update_spots(&mut self) {
        let now = self.run.time_s();
        let events = &self.run.referee.events;
        while self.spot_cursor < events.len() {
            let e = events[self.spot_cursor];
            self.spot_cursor += 1;
            if e.kind == EventKind::SpotFire {
                if let Some(p) = e.pos {
                    self.spots.push(SpotTrack { first: p, now: p, gone: None, worked: false });
                }
            }
        }
        let (labels, burning) = self.components();
        let w = self.run.scn.world;
        let main = self.main_label(&labels);
        let mut seen: Vec<u32> = vec![];
        let active: Vec<(u32, Pos)> = self
            .run
            .fire
            .active_cells()
            .iter()
            .map(|c| (labels[c.row * w.fire_cols + c.col], w.centre_of(*c)))
            .collect();
        for k in 0..self.spots.len() {
            if self.spots[k].gone.is_some() {
                continue;
            }
            let c = w.cell_of(self.spots[k].first);
            let l = labels[c.row * w.fire_cols + c.col];
            let gone = if l == 0 {
                Some(false)
            } else if l == main || seen.contains(&l) {
                Some(true)
            } else if !burning[l as usize] {
                Some(false)
            } else {
                None
            };
            match gone {
                Some(merged) => {
                    self.spots[k].gone = Some((now, merged));
                    if !merged {
                        self.happenings.push(Happening::SpotOut(self.spots[k].first));
                    }
                }
                None => {
                    seen.push(l);
                    let first = self.spots[k].first;
                    if let Some((_, p)) = active.iter().filter(|(al, _)| *al == l).min_by(|a, b| {
                        dist(a.1, first).partial_cmp(&dist(b.1, first)).unwrap_or(std::cmp::Ordering::Equal)
                    }) {
                        self.spots[k].now = *p;
                    }
                }
            }
        }
    }

    /// Spot fires seen so far: (where it started, gone: Some(merged)).
    pub fn spot_fires(&self) -> Vec<(Pos, Option<bool>)> {
        self.spots.iter().map(|s| (s.first, s.gone.map(|g| g.1))).collect()
    }

    // --- previews ----------------------------------------------------------------

    /// What `target` says for `token` before the order is given. `None` if the
    /// target is not valid for it (not lit).
    pub fn preview(&self, token: TokenId, target: TargetId) -> Option<Preview> {
        let t = *self.target(target)?;
        if !self.valid(token, t.kind) {
            return None;
        }
        let ds = &self.run.referee.districts;
        let agents = &self.run.agents;
        let homes_near = |p: Pos, r: f32| agents.households.iter().filter(|h| dist(h.home, p) <= r).count() as u32;
        Some(match token.kind() {
            TokenKind::Pattuglia => {
                let TargetKind::District(d) = t.kind else { return None };
                let (_, len) = self.patrol_route(t.pos);
                let unwarned = ds[d].households.iter().filter(|&&i| !agents.households[i].ordered).count() as u32;
                let en_route = self.patrol.to == Some(d) && !self.patrol.delivered;
                Preview {
                    eta_s: (len / PATROL_SPEED).ceil() as i64,
                    effect: if unwarned == 0 || en_route { Effect::Inutile } else { Effect::Avvisa { families: unwarned } },
                }
            }
            TokenKind::ItAlert => {
                let n = agents.households.iter().filter(|h| !h.ordered).count() as u32;
                Preview { eta_s: 0, effect: Effect::AvvisaTutti { families: n } }
            }
            TokenKind::Autobotte | TokenKind::Squadra => {
                let k = token.unit().expect("unit");
                let at = self.order_point(token, &t);
                let (eta_s, park) = self.ground_eta(k, at);
                let engine = token.kind() == TokenKind::Autobotte;
                // Where the unit will actually stand: an engine at the road
                // node the model drives it to, a crew at the point itself.
                let stand = if engine { park } else { at };
                let gap = dist(park, at);
                let effect = match t.kind {
                    // Withdrawal is promised only where it will follow: the
                    // ground it will stand on is already past the unit's
                    // working limit and it gets there within the turn.
                    TargetKind::Head if eta_s <= TURN_S && self.run.fire.threat().at(stand) >= WORK_LIMIT => Effect::Ritirata,
                    TargetKind::Head if engine && gap > ENGINE_REACH_M => Effect::Lontano,
                    TargetKind::Head => Effect::NienteCase,
                    TargetKind::District(d) if !self.at_risk(d) => Effect::Inutile,
                    // Home defence credits every home within DEFEND_REACH_M of
                    // where the engine works (`run::Tally`): the same count.
                    TargetKind::District(_) if engine => match homes_near(park, DEFEND_REACH_M) {
                        0 => Effect::Lontano,
                        n => Effect::Difende { homes: n },
                    },
                    _ if engine && gap > ENGINE_REACH_M => Effect::Lontano,
                    TargetKind::District(_) | TargetKind::Flank(_) => Effect::Linea,
                    TargetKind::SpotFire(_) => Effect::Spegne,
                    TargetKind::Sky | TargetKind::Town => return None,
                };
                Preview { eta_s, effect }
            }
            TokenKind::Canadair => {
                if t.kind == TargetKind::Sky {
                    let arrive = self.run.time_s() + AIR_RESPONSE_S as i64;
                    let turn = (arrive / TURN_S) as u8 + 1;
                    return Some(Preview { eta_s: AIR_RESPONSE_S as i64, effect: Effect::Chiamata { eta_s: AIR_RESPONSE_S as i64, turn } });
                }
                let u = &self.run.crews.units[6];
                let wait = if u.state == UnitState::Inbound { (u.arrives_at_s - self.run.crews.time_s()).max(0.0) } else { 0.0 };
                let eta = wait + dist(u.pos, t.pos) / TANKER_SPEED + if u.water_l <= 0.0 { SCOOP_S } else { 0.0 };
                let effect = match t.kind {
                    TargetKind::Head => Effect::RallentaPoco,
                    TargetKind::District(d) if !self.at_risk(d) => Effect::Inutile,
                    TargetKind::District(_) => Effect::Bagna { homes: homes_near(t.pos, DROP_WIDTH_M) },
                    TargetKind::Flank(_) => Effect::Bagna { homes: homes_near(t.pos, DROP_WIDTH_M) },
                    TargetKind::SpotFire(_) => Effect::Spegne,
                    TargetKind::Sky | TargetKind::Town => return None,
                };
                Preview { eta_s: eta.ceil() as i64, effect }
            }
        })
    }

    /// Drive time for ground unit `k` to `to`, and where the road ends: the
    /// drivable node `Suppression::drive_toward` routes an engine to
    /// (`nearest_reachable`, finding 17) -- where it will park and work.
    fn ground_eta(&self, k: usize, to: Pos) -> (i64, Pos) {
        let u = &self.run.crews.units[k];
        let engine = u.kind == UnitKind::Engine;
        let net = &self.run.agents.network;
        let from = net.nearest(u.pos, engine);
        let end = from.and_then(|a| net.nearest_reachable(to, engine, a));
        let (Some(a), Some(b)) = (from, end) else {
            return ((dist(u.pos, to) / if engine { ENGINE_SPEED } else { CREW_WALK_SPEED }).ceil() as i64, u.pos);
        };
        let path = network::route(net, a, b, self.run.fire.threat(), engine).unwrap_or_default();
        let mut at = u.pos;
        let mut m = 0.0;
        for n in path.iter().copied().chain(if path.is_empty() { Some(b) } else { None }) {
            m += dist(at, net.pos(n));
            at = net.pos(n);
        }
        let gap = dist(net.pos(b), to);
        let s = if engine { m / ENGINE_SPEED } else { m / CREW_SPEED + gap / CREW_WALK_SPEED };
        (s.ceil() as i64, net.pos(b))
    }

    /// Where an order of `token` to `t` sends the unit: for a district, the
    /// next free post along its fire-facing edge (`District::post_facing`),
    /// skipping posts other units already hold or are being sent to this turn
    /// (in token order, which is the order `end_turn` applies them in). The
    /// preview and the order both use this, so they cannot disagree.
    fn order_point(&self, token: TokenId, t: &Target) -> Pos {
        let TargetKind::District(d) = t.kind else { return t.pos };
        if token.kind() == TokenKind::Canadair || token.unit().is_none() {
            return t.pos;
        }
        let mut taken: Vec<Pos> = TokenId::ALL
            .iter()
            .filter(|o| **o != token && !self.pending.iter().any(|(p, _)| p == *o))
            .filter_map(|o| self.doing[o.index()])
            .filter(|g| g.kind == t.kind)
            .map(|g| g.pos)
            .collect();
        for (o, g) in &self.pending {
            if *o >= token || o.kind() == TokenKind::Canadair || o.unit().is_none() {
                continue;
            }
            if let Some(og) = self.target(*g).filter(|x| x.kind == t.kind) {
                taken.push(self.order_point(*o, og));
            }
        }
        let head = self.target_of(TargetKind::Head).map_or_else(|| self.head(), |h| h.pos);
        self.run.referee.districts[d].post_facing(&self.run.agents, head, &taken, 2.0 * DEFEND_REACH_M)
    }

    /// The patrol's road from where it is to `to`: polyline and length.
    fn patrol_route(&self, to: Pos) -> (Vec<Pos>, f32) {
        let net = &self.run.agents.network;
        let from = net.nearest(self.patrol.at, true);
        let end = from.and_then(|a| net.nearest_reachable(to, true, a));
        let mut pts = vec![self.patrol.at];
        if let (Some(a), Some(b)) = (from, end) {
            pts.push(net.pos(a));
            for n in network::route(net, a, b, self.run.fire.threat(), true).unwrap_or_default() {
                pts.push(net.pos(n));
            }
        }
        pts.push(to);
        let len = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
        (pts, len)
    }

    /// The district lies where the wind blows the fire now, or where the
    /// forecast in force says a likely-enough shift would. What the player
    /// knows; never the future.
    pub fn at_risk(&self, d: usize) -> bool {
        self.points_at(d, self.run.time_s())
    }

    fn points_at(&self, d: usize, t: i64) -> bool {
        let f = self.forecast_at(t);
        let (from, _) = self.wind();
        let downwind = self.run.district_toward((from + 180.0) % 360.0) == Some(d);
        let shift = f.shift_p >= POINTS_AT_P && self.run.district_toward((f.shift_to_deg + 180.0) % 360.0) == Some(d);
        downwind || shift
    }

    // --- orders --------------------------------------------------------------------

    /// Give `token` the order `target` for this turn (replacing any earlier
    /// one this turn). Played on *Avanti*.
    pub fn assign(&mut self, token: TokenId, target: TargetId) -> Result<(), Refusal> {
        if self.turn > TURNS || self.finished() {
            return Err(Refusal::Finita);
        }
        if !self.in_tray(token) {
            return Err(Refusal::NonDisponibile);
        }
        let state = self.token_state(token);
        if !self.orderable(token, state) {
            return Err(Refusal::Occupato(state));
        }
        let t = self.target(target).ok_or(Refusal::BersaglioSconosciuto)?;
        if !self.valid(token, t.kind) {
            return Err(Refusal::BersaglioNonValido);
        }
        self.pending.retain(|(k, _)| *k != token);
        self.pending.push((token, target));
        self.pending.sort();
        Ok(())
    }

    /// Cancel this turn's order for `token`.
    pub fn unassign(&mut self, token: TokenId) -> bool {
        let n = self.pending.len();
        self.pending.retain(|(k, _)| *k != token);
        n != self.pending.len()
    }

    /// Orders given this turn and not yet played.
    pub fn pending(&self) -> &[(TokenId, TargetId)] {
        &self.pending
    }

    /// Every order played so far: (turn, token, target).
    pub fn log(&self) -> &[(u8, TokenId, TargetKind)] {
        &self.log
    }

    /// Play the turn: apply its orders, step the model to the end of the turn,
    /// write the report, open the next turn. After the last turn each call
    /// plays the next [`TURN_S`] of the finale, to [`MISSION_S`].
    pub fn end_turn(&mut self) -> Result<()> {
        self.end_turn_observed(|_| {})
    }

    /// [`Session::end_turn`], calling `observe` after every model step (the
    /// `play` binary prints its two mid-turn frames from here).
    pub fn end_turn_observed(&mut self, mut observe: impl FnMut(&Session)) -> Result<()> {
        if self.finished() {
            return Ok(());
        }
        self.begin();
        let end = ((self.turn as i64) * TURN_S).min(MISSION_S);
        while self.run.time_s() < end {
            self.step()?;
            observe(self);
        }
        self.close();
        Ok(())
    }

    /// Play every remaining turn with no further orders, to the end.
    pub fn finish(&mut self) -> Result<()> {
        while !self.finished() {
            self.end_turn()?;
        }
        Ok(())
    }

    fn begin(&mut self) {
        let now = self.run.time_s();
        let turn = self.turn;
        let points: Vec<Option<Pos>> = self.pending.iter().map(|(k, g)| self.target(*g).map(|t| self.order_point(*k, t))).collect();
        let orders = std::mem::take(&mut self.pending);
        for ((token, tid), point) in orders.into_iter().zip(points) {
            let Some(t) = self.target(tid).copied() else { continue };
            self.log.push((turn, token, t.kind));
            self.started[token.index()] = false;
            if let (TokenKind::Autobotte, TargetKind::District(d)) = (token.kind(), t.kind) {
                if !self.points_at(d, now) && !self.needless_defence.contains(&d) {
                    self.needless_defence.push(d);
                }
            }
            match token {
                TokenId::P => {
                    let TargetKind::District(d) = t.kind else { continue };
                    let (path, len) = self.patrol_route(t.pos);
                    let pointed = self.points_at(d, now);
                    self.patrol = Patrol {
                        at: self.patrol.at,
                        path,
                        depart_s: now,
                        arrive_s: now + (len / PATROL_SPEED).ceil() as i64,
                        to: Some(d),
                        delivered: false,
                        pointed,
                    };
                }
                TokenId::I => {
                    let flags: Vec<bool> = (0..self.pointed.len()).map(|d| self.points_at(d, now)).collect();
                    let before: Vec<bool> = self.run.referee.reports.iter().map(|r| r.warned_at_s.is_some()).collect();
                    self.run.order(Order::EvacuateAll);
                    self.it_alert = Some(turn);
                    for (d, r) in self.run.referee.reports.iter().enumerate() {
                        if !before[d] && r.warned_at_s.is_some() && self.pointed[d].is_none() {
                            self.pointed[d] = Some(flags[d]);
                        }
                    }
                }
                TokenId::K if t.kind == TargetKind::Sky => {
                    self.run.crews.request_air_unit(6);
                    self.called = Some(turn);
                }
                _ => {
                    let k = token.unit().expect("unit token");
                    let pos = point.unwrap_or(t.pos);
                    if let TargetKind::SpotFire(n) = t.kind {
                        if let Some(s) = self.spots.get_mut(n) {
                            s.worked = true;
                        }
                    }
                    let task = if token == TokenId::K { Task::Drop { at: pos } } else { Task::Attack { at: pos } };
                    if self.run.crews.assign(k, task).is_ok() {
                        self.doing[token.index()] = Some(Doing { kind: t.kind, pos });
                    }
                }
            }
        }
        // The Canadair's standing order: one drop a turn on the same target,
        // wherever that target is now.
        if let Some(g) = self.doing[TokenId::K.index()] {
            let u = &self.run.crews.units[6];
            if u.task == Task::Hold && u.state != UnitState::Lost {
                match self.target_of(g.kind).map(|t| t.pos) {
                    Some(p) => {
                        let p = if matches!(g.kind, TargetKind::District(_)) { g.pos } else { p };
                        if self.run.crews.assign(6, Task::Drop { at: p }).is_ok() {
                            self.doing[TokenId::K.index()] = Some(Doing { kind: g.kind, pos: p });
                        }
                    }
                    None => self.doing[TokenId::K.index()] = None,
                }
            }
        }
    }

    fn step(&mut self) -> Result<()> {
        self.run.step()?;
        self.after_step();
        Ok(())
    }

    fn after_step(&mut self) {
        let now = self.run.time_s();
        // The patrol: drive, and warn the district when it gets there.
        if let (Some(d), false) = (self.patrol.to, self.patrol.delivered) {
            if now >= self.patrol.arrive_s {
                let before = self.run.agents.households.iter().filter(|h| h.ordered).count();
                let had = self.run.referee.reports[d].warned_at_s.is_some();
                self.run.order(Order::EvacuateDistrict(d));
                let n = self.run.agents.households.iter().filter(|h| h.ordered).count() - before;
                self.patrol.delivered = true;
                self.patrol.at = *self.patrol.path.last().unwrap_or(&self.patrol.at);
                if !had && self.run.referee.reports[d].warned_at_s.is_some() && self.pointed[d].is_none() {
                    self.pointed[d] = Some(self.patrol.pointed);
                }
                self.happenings.push(Happening::PatrolArrived(d, n as u32));
            } else {
                let travelled = (now - self.patrol.depart_s) as f32 * PATROL_SPEED;
                self.patrol.at = along(&self.patrol.path, travelled);
            }
        }
        // A unit ordered to the head goes on chasing it (HeadOrder::Track only).
        if self.head_order == HeadOrder::Track && now % HEAD_TRACK_S == 0 {
            let head = self.head();
            for token in [TokenId::E1, TokenId::E2, TokenId::E3, TokenId::S] {
                let k = token.unit().expect("unit");
                if let Some(g) = self.doing[token.index()].filter(|g| g.kind == TargetKind::Head) {
                    let u = &self.run.crews.units[k];
                    if matches!(u.state, UnitState::Moving | UnitState::Working) && dist(g.pos, head) > 40.0 && self.run.crews.assign(k, Task::Attack { at: head }).is_ok() {
                        self.doing[token.index()] = Some(Doing { kind: TargetKind::Head, pos: head });
                    }
                }
            }
        }
        // Units: what changed this step.
        for token in [TokenId::E1, TokenId::E2, TokenId::E3, TokenId::S, TokenId::K] {
            let k = token.unit().expect("unit");
            let (state, drops, pos, task) = {
                let u = &self.run.crews.units[k];
                (u.state, u.drops, u.pos, u.task)
            };
            let (prev, prev_drops) = self.unit_prev[k];
            let i = token.index();
            // An aircraft has no retreat to drive: when its policy finds the
            // air it is flying through not survivable it breaks off to Staged
            // with no task (`Suppression::apply_outcome`). That is its
            // withdrawal, and the tray says so rather than "al lavoro".
            let broke_off = token == TokenId::K
                && state == UnitState::Staged
                && matches!(prev, UnitState::Moving | UnitState::Working)
                && task == Task::Hold
                && drops == prev_drops;
            if state != prev {
                match state {
                    _ if broke_off => {
                        self.happenings.push(Happening::Withdrew(token, pos));
                        self.withdrew[i] = Some(self.turn);
                        if self.doing[i].is_some_and(|g| g.kind == TargetKind::Head) && !self.head_withdrew.contains(&token) {
                            self.head_withdrew.push(token);
                        }
                        self.doing[i] = None;
                    }
                    UnitState::Withdrawing => {
                        self.happenings.push(Happening::Withdrew(token, pos));
                        self.withdrew[i] = Some(self.turn);
                        if self.doing[i].is_some_and(|g| g.kind == TargetKind::Head) && !self.head_withdrew.contains(&token) {
                            self.head_withdrew.push(token);
                        }
                        self.doing[i] = None;
                    }
                    UnitState::Lost => {
                        self.happenings.push(Happening::Lost(token, pos));
                        self.lost.push(token);
                        self.doing[i] = None;
                    }
                    UnitState::Working if !self.started[i] && token != TokenId::K => {
                        self.started[i] = true;
                        self.happenings.push(Happening::Arrived(token, pos));
                    }
                    _ => {}
                }
                if token == TokenId::K && prev == UnitState::Inbound {
                    self.happenings.push(Happening::OnStation(pos));
                }
            }
            if drops > prev_drops {
                self.drops += drops - prev_drops;
                if now <= TURNS as i64 * TURN_S {
                    self.drops_by_last_turn += drops - prev_drops;
                }
                self.happenings.push(Happening::Drop(pos));
                // One drop a turn: hold until the next turn re-tasks it.
                let _ = self.run.crews.assign(6, Task::Hold);
            }
            if token != TokenId::K && task == Task::Hold && state != UnitState::Withdrawing {
                self.doing[i] = None;
            }
            let u = &self.run.crews.units[k];
            self.unit_prev[k] = (u.state, u.drops);
        }
    }

    fn close(&mut self) {
        self.update_spots();
        self.report = self.build_report();
        self.happenings.clear();
        self.ev_cursor = self.run.referee.events.len();
        self.turn += 1;
        self.compute_targets();
    }

    fn open(&mut self) {
        self.update_spots();
        self.compute_targets();
    }

    // --- the report ------------------------------------------------------------------

    /// The report on the turn just played (≤ 3 lines, most important first).
    pub fn report(&self) -> &TurnReport {
        &self.report
    }

    fn build_report(&self) -> TurnReport {
        let line = |kind, district: Option<usize>, pos: Option<Pos>, n: Option<u32>, token: Option<TokenId>| ReportLine { kind, district, pos, n, token };
        let mut c: Vec<ReportLine> = vec![];
        let events = &self.run.referee.events[self.ev_cursor..];
        let reached: Vec<usize> = events.iter().filter_map(|e| if let EventKind::DistrictReached { district } = e.kind { Some(district) } else { None }).collect();
        let mut spots = 0u32;
        let mut spot_pos = None;
        for e in events {
            match e.kind {
                EventKind::DistrictReached { district } => c.push(line(ReportKind::QuartiereRaggiunto, Some(district), e.pos, None, None)),
                EventKind::DistrictThreatened { district } if !reached.contains(&district) => {
                    c.push(line(ReportKind::QuartiereMinacciato, Some(district), e.pos, None, None))
                }
                EventKind::WindShifted { to_deg, .. } => c.push(line(ReportKind::VentoGirato, None, None, Some(to_deg.round() as u32), None)),
                EventKind::SpotFire => {
                    spots += 1;
                    spot_pos = e.pos;
                }
                EventKind::MastDown => c.push(line(ReportKind::RipetitoreGiu, None, None, None, None)),
                _ => {}
            }
        }
        if spots > 0 {
            c.push(line(ReportKind::NuovoFocolaio, None, spot_pos, Some(spots), None));
        }
        let district_of = |p: Pos| -> Option<usize> {
            self.run
                .referee
                .districts
                .iter()
                .enumerate()
                .map(|(k, d)| (k, dist(d.centre, p) - d.radius_m))
                .filter(|(_, m)| *m <= 2.0 * DEFEND_REACH_M)
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(k, _)| k)
        };
        for h in &self.happenings {
            match *h {
                Happening::Lost(t, p) => c.push(line(ReportKind::MezzoPerso, None, Some(p), None, Some(t))),
                Happening::Withdrew(t, p) => c.push(line(ReportKind::Ritirata, None, Some(p), None, Some(t))),
                Happening::PatrolArrived(d, n) => {
                    c.push(line(ReportKind::PattugliaArrivata, Some(d), Some(self.run.referee.districts[d].centre), Some(n), Some(TokenId::P)))
                }
                Happening::SpotOut(p) => c.push(line(ReportKind::FocolaioSpento, None, Some(p), None, None)),
                Happening::OnStation(p) => c.push(line(ReportKind::CanadairInZona, None, Some(p), None, Some(TokenId::K))),
                Happening::Drop(p) => c.push(line(ReportKind::Lancio, district_of(p), Some(p), None, Some(TokenId::K))),
                Happening::Arrived(t, p) => c.push(line(ReportKind::Arrivato, district_of(p), Some(p), None, Some(t))),
            }
        }
        for (d, r) in self.run.referee.reports.iter().enumerate() {
            if r.warned_at_s.is_some() && r.moving > 0 {
                c.push(line(ReportKind::FamiglieInViaggio, Some(d), Some(self.run.referee.districts[d].centre), Some(r.moving as u32), None));
            }
        }
        if self.turn + 1 == REINFORCEMENT_TURN {
            c.push(line(ReportKind::Rinforzo, None, Some(self.station()), None, Some(TokenId::E3)));
        }
        if self.turn == 1 {
            c.push(line(ReportKind::NuovoBollettino, None, None, None, None));
        }
        c.sort_by_key(|l| l.kind);
        let mut out: Vec<ReportLine> = vec![];
        for l in c {
            if out.len() == 3 {
                break;
            }
            if out.iter().any(|o| o.kind == l.kind && o.district == l.district && o.token == l.token) {
                continue;
            }
            out.push(l);
        }
        TurnReport { turn: self.turn, lines: out }
    }

    // --- districts on the map ----------------------------------------------------------

    pub fn districts(&self) -> Vec<DistrictView> {
        let r = &self.run.referee;
        r.districts
            .iter()
            .zip(&r.reports)
            .enumerate()
            .map(|(k, (d, rep))| DistrictView {
                district: k,
                name: d.name.clone(),
                households: d.households.len() as u32,
                fire_m: if rep.fire_now_m.is_finite() {
                    rep.fire_now_m
                } else {
                    let w = &self.run.scn.world;
                    d.distance_to(&self.burning_cells().iter().map(|c| w.centre_of(*c)).collect::<Vec<_>>())
                },
                level: if rep.reached_at_s.is_some() {
                    Level::Reached
                } else if rep.fire_now_m.is_finite() {
                    rep.level()
                } else {
                    // T+0: the referee has seen nothing burn yet; judge by the patch.
                    let w = &self.run.scn.world;
                    let m = d.distance_to(&self.burning_cells().iter().map(|c| w.centre_of(*c)).collect::<Vec<_>>());
                    if m <= crate::district::THREATENED_M {
                        Level::Threatened
                    } else if m <= crate::district::WATCH_M {
                        Level::Watch
                    } else {
                        Level::Calm
                    }
                },
                warned: rep.warned_at_s.is_some(),
                moving: rep.moving as u32,
                safe: rep.safe as u32,
                caught: rep.caught as u32,
                units: TokenId::ALL
                    .iter()
                    .filter(|t| self.doing[t.index()].is_some_and(|g| g.kind == TargetKind::District(k)))
                    .copied()
                    .collect(),
            })
            .collect()
    }

    /// Canadair drops so far (all), and those before the last turn ended.
    pub fn drops(&self) -> (u32, u32) {
        (self.drops, self.drops_by_last_turn)
    }

    /// Turn the Canadair was called in, if it was.
    pub fn called(&self) -> Option<u8> {
        self.called
    }

    // --- the verdict ---------------------------------------------------------------------

    /// This session's own facts, as a counterfactual would report them.
    pub fn facts(&self) -> Counterfactual {
        let r = &self.run.referee;
        let tally = &r.tally;
        let n = self.run.agents.households.len();
        let caught = (0..n).filter(|&i| tally.caught_at(i).is_some()).count() as u32;
        let homes = (0..n).filter(|&i| tally.home_lost(i) == Some(true)).count() as u32;
        Counterfactual {
            families_safe: n as u32 - caught,
            families_caught: caught,
            homes_hit: homes,
            districts: r
                .districts
                .iter()
                .map(|d| {
                    let c = d.households.iter().filter(|&&i| tally.caught_at(i).is_some()).count() as u32;
                    let h = d.households.iter().filter(|&&i| tally.home_lost(i) == Some(true)).count() as u32;
                    (c, h)
                })
                .collect(),
        }
    }

    /// The same draw, no orders, to the end: the twin.
    pub fn counterfactual_of(data_dir: &Path, draw: Draw) -> Result<Counterfactual> {
        let mut s = Session::from_draw(data_dir, draw)?;
        s.finish()?;
        Ok(s.facts())
    }

    /// Supply the counterfactual (a sweep shares one per seed).
    pub fn set_counterfactual(&mut self, c: Counterfactual) {
        self.none = Some(c);
    }

    /// The counterfactual, computed once.
    pub fn counterfactual(&mut self) -> Result<Counterfactual> {
        if self.none.is_none() {
            self.none = Some(Session::counterfactual_of(&self.data_dir, self.draw)?);
        }
        Ok(self.none.clone().expect("just set"))
    }

    /// The people stamp on district `d` against the counterfactual: a
    /// warning is *In tempo* only if it landed in time **and** the district
    /// lost fewer families than it would have with no orders (or would have
    /// lost none anyway). A warning that changed nothing is *Tardi*, however
    /// early the clock says it was -- by T+24 the families who would leave
    /// have left on their own, and the stamp must not say otherwise.
    pub fn stamp_against(&self, d: usize, none: &Counterfactual) -> Stamp {
        let s = self.stamp(d);
        if s != Stamp::InTempo {
            return s;
        }
        let caught = self.run.referee.districts[d].households.iter().filter(|&&i| self.run.referee.tally.caught_at(i).is_some()).count() as u32;
        let caught_none = none.districts.get(d).map_or(0, |x| x.0);
        if caught < caught_none || caught_none == 0 {
            Stamp::InTempo
        } else {
            Stamp::Tardi
        }
    }

    /// The people stamp on district `d` by the clock alone, as things stand
    /// (no counterfactual yet): the in-session view.
    pub fn stamp(&self, d: usize) -> Stamp {
        let r = &self.run.referee.reports[d];
        // The danger came when the fire first threatened or reached the
        // district, whichever was first. A warning is in time only if it
        // landed IN_TIME_MIN before that; one landing after the threat is late.
        let danger = match (r.threatened_at_s, r.reached_at_s) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        match (r.warned_at_s, danger) {
            (Some(w), Some(a)) => {
                if a - w >= IN_TIME_S {
                    Stamp::InTempo
                } else {
                    Stamp::Tardi
                }
            }
            (Some(_), None) => {
                if self.pointed[d] == Some(true) {
                    Stamp::Prudente
                } else {
                    Stamp::AllarmeInutile
                }
            }
            // The spec's rule: reached, never warned. A district the fire only
            // came near (within the threat distance) without reaching a home
            // or catching a family is not "mai avvisati": nothing arrived.
            (None, Some(_)) => Stamp::MaiAvvisati,
            (None, None) => Stamp::GiustoNonAvvisare,
        }
    }

    /// The end card. Final once [`Session::finished`]; computes the
    /// counterfactual on first call if none was supplied.
    pub fn verdict(&mut self) -> Result<Verdict> {
        let none = self.counterfactual()?;
        Ok(self.verdict_against(none))
    }

    pub fn verdict_against(&self, none: Counterfactual) -> Verdict {
        let me = self.facts();
        let nd = self.run.referee.districts.len();
        let districts: Vec<DistrictVerdict> = (0..nd)
            .map(|d| DistrictVerdict {
                district: d,
                people: self.stamp_against(d, &none),
                homes_hit: me.districts[d].1,
                homes_hit_none: none.districts.get(d).map_or(0, |x| x.1),
                caught: me.districts[d].0,
                caught_none: none.districts.get(d).map_or(0, |x| x.0),
            })
            .collect();
        let mut notes: Vec<Note> = vec![];
        for t in &self.lost {
            notes.push(Note::UnitaPersa { token: *t });
        }
        for t in &self.head_withdrew {
            notes.push(Note::TestaRitirata { token: *t });
        }
        for (_, t, k) in &self.log {
            if *k == TargetKind::Head && t.kind() != TokenKind::Canadair && !self.head_withdrew.contains(t) && !self.lost.contains(t) {
                notes.push(Note::TestaInutile { token: *t });
            }
        }
        if self.it_alert.is_some() && districts.iter().any(|v| v.people == Stamp::AllarmeInutile) {
            notes.push(Note::ItAlertSprecato);
        }
        let defended: Vec<usize> = (0..nd)
            .filter(|d| self.log.iter().any(|(_, t, k)| t.kind() == TokenKind::Autobotte && *k == TargetKind::District(*d)))
            .collect();
        for &d in &self.needless_defence {
            if self.run.referee.reports[d].threatened_at_s.is_none() {
                notes.push(Note::DifesaInutile { district: d });
            }
        }
        match (self.called, self.drops) {
            (None, _) => notes.push(Note::CanadairMaiChiamato),
            (Some(_), 0) => notes.push(Note::CanadairTardi),
            (Some(_), n) => notes.push(Note::CanadairInTempo { drops: n }),
        }
        if self.spots.iter().any(|s| s.worked && s.gone.is_some_and(|g| !g.1)) {
            notes.push(Note::FocolaioSpento);
        }
        for &d in &defended {
            let v = &districts[d];
            if v.homes_hit < v.homes_hit_none && self.run.referee.reports[d].threatened_at_s.is_some() {
                notes.push(Note::CaseDifese { district: d, homes: v.homes_hit_none - v.homes_hit });
            }
        }
        notes.sort_by_key(|n| !n.broken());
        notes.dedup();
        notes.truncate(3);
        let headline = if me.families_caught == 0 {
            Headline::TuttiAlSicuro
        } else if me.families_caught < none.families_caught && me.families_caught <= 3 {
            Headline::OttimoLavoro
        } else if me.families_caught < none.families_caught {
            Headline::HaiFattoLaDifferenza
        } else {
            Headline::FuocoPiuVeloce
        };
        Verdict {
            headline,
            districts,
            families_safe: me.families_safe,
            families_caught: me.families_caught,
            homes_hit: me.homes_hit,
            households: self.run.agents.households.len() as u32,
            none,
            notes,
        }
    }

    /// The patrol car's position and, while it drives, the road ahead.
    pub fn patrol_path(&self) -> Option<&[Pos]> {
        (!self.patrol.delivered).then_some(self.patrol.path.as_slice())
    }

    /// Where the fire station is.
    pub fn station_pos(&self) -> Pos {
        self.station()
    }
}

/// The point `m` metres along a polyline.
fn along(path: &[Pos], mut m: f32) -> Pos {
    for w in path.windows(2) {
        let d = dist(w[0], w[1]);
        if m <= d && d > 0.0 {
            let f = m / d;
            return Pos { x: w[0].x + (w[1].x - w[0].x) * f, y: w[0].y + (w[1].y - w[0].y) * f };
        }
        m -= d;
    }
    path.last().copied().unwrap_or(Pos { x: 0.0, y: 0.0 })
}
