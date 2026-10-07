//! The turn game's vocabulary (`docs/demo-spec-gameplay.md` §1): the clock,
//! the tokens in the tray, the targets on the map, what a target says before
//! you commit, the report between turns and the verdict at the end.
//!
//! Everything here is a typed fact with **no text**: `crates/text` turns each
//! kind into Italian, so the `play` binary and the kiosk read the same words.
//! The engine that produces these is [`crate::Session`].

use scenario::Pos;

/// Simulated seconds per turn.
pub const TURN_S: i64 = 8 * 60;
/// Turns the player gives orders in.
pub const TURNS: u8 = 5;
/// The incident runs to here; after the last turn the fire plays out alone.
pub const MISSION_S: i64 = 60 * 60;
/// The turn the reinforcement engine joins the tray.
pub const REINFORCEMENT_TURN: u8 = 3;
/// Police car on blue lights in a small town, m/s (~50 km/h).
pub const PATROL_SPEED: f32 = 14.0;
/// A forecast shift at least this likely "points at" the district it would
/// turn the fire onto (the *possibile* band and above). What a `Prudente`
/// stamp and an engine preview judge by: what the player was told.
pub const POINTS_AT_P: f32 = 0.30;
/// The *probabile* band: a forecast-reading player acts on it.
pub const LIKELY_P: f32 = 0.50;

/// The clock, as the top bar shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Turn {
    /// 1..=[`TURNS`] while orders can be given; above that the finale.
    pub index: u8,
    /// Simulated second this turn opened at.
    pub at_s: i64,
    /// The last turn with orders.
    pub last: bool,
}

impl Turn {
    /// The finale: no more orders, the fire plays out to [`MISSION_S`].
    pub fn finale(&self) -> bool {
        self.index > TURNS
    }
}

/// One card in the tray. Fixed order: the order the tray shows them in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TokenId {
    P,
    E1,
    E2,
    E3,
    S,
    K,
}

impl TokenId {
    pub const ALL: [TokenId; 6] = [TokenId::P, TokenId::E1, TokenId::E2, TokenId::E3, TokenId::S, TokenId::K];

    pub fn kind(self) -> TokenKind {
        match self {
            TokenId::P => TokenKind::Pattuglia,
            TokenId::E1 | TokenId::E2 | TokenId::E3 => TokenKind::Autobotte,
            TokenId::S => TokenKind::Squadra,
            TokenId::K => TokenKind::Canadair,
        }
    }

    /// The model unit this token drives (`abm::suppression` roster order:
    /// three engines, three crews, two aircraft). The patrol is not a unit.
    pub fn unit(self) -> Option<usize> {
        match self {
            TokenId::E1 => Some(0),
            TokenId::E2 => Some(1),
            TokenId::E3 => Some(2),
            TokenId::S => Some(3),
            TokenId::K => Some(6),
            TokenId::P => None,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// Engines are numbered for the tray (1, 2, 3); others are not.
    pub fn number(self) -> Option<u8> {
        match self {
            TokenId::E1 => Some(1),
            TokenId::E2 => Some(2),
            TokenId::E3 => Some(3),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Pattuglia,
    Autobotte,
    Squadra,
    Canadair,
}

/// The badge on a token and on its unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenState {
    Libero,
    InViaggio { eta_s: i64 },
    AlLavoro,
    Rifornimento { eta_s: i64 },
    /// Pulled back from heat it could not work in; sits out the next turn.
    Ritirato,
    Perso,
    /// The Canadair, called and on its way.
    InArrivo { eta_s: i64 },
    NonChiamato,
}

/// A card in the tray, with its unit on the map.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub id: TokenId,
    pub kind: TokenKind,
    pub state: TokenState,
    /// Engines: fraction of the tank left, 0-1.
    pub water: Option<f32>,
    /// Where its unit is (the patrol car, the engine, the plane).
    pub at: Pos,
    /// The order given this turn and not yet played.
    pub order: Option<TargetId>,
    /// What it is working on from an earlier turn, if anything.
    pub doing: Option<TargetKind>,
    /// Whether an order may be given to it now.
    pub orderable: bool,
}

/// A target's number on the map. Stable through a session for districts,
/// head and flanks. Spot fires are shown without target numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetId(pub u8);

/// Left or right of the fire's run, looking downwind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetKind {
    /// Index into the town's districts; the target is its fire-facing edge.
    District(usize),
    Head,
    Flank(Side),
    /// The Canadair call: the tray slot itself.
    Sky,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub id: TargetId,
    pub kind: TargetKind,
    /// Where a unit sent here goes (a district's post, the head cell, ...).
    pub pos: Pos,
    /// Where the marker's label sits (a district's centre).
    pub label_pos: Pos,
    /// Flanks: the compass bearing from the fire's axis out to this flank, so
    /// the label can say *ovest* rather than *sinistro*.
    pub facing_deg: Option<f32>,
}

/// What a target says before you commit (gameplay §4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preview {
    /// Simulated seconds until the unit is there.
    pub eta_s: i64,
    pub effect: Effect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// The patrol warns this many families when it arrives.
    Avvisa { families: u32 },
    /// An engine posted here covers this many homes (the homes within
    /// `DEFEND_REACH_M` of the road node it will work from: what the verdict
    /// credits).
    Difende { homes: u32 },
    /// The crew's firebreak protects this many homes (the homes within
    /// `DEFEND_REACH_M` of its post: what the verdict credits).
    Fascia { homes: u32 },
    /// A drop on a district's edge, over this many homes' ground.
    Bagna { homes: u32 },
    /// The unit works there and saves no home (the head, the flanks, a drop
    /// anywhere but the threatened district: milestone 0, §7.2, §7.6).
    NonSalvaCase,
    /// No road gets an engine within hose reach of it.
    Lontano,
    /// The unit will pull back: the ground it will stand on is already past
    /// its working limit and it gets there within the turn.
    Ritirata,
    /// The Canadair call: overhead in `eta_s`, during turn `turn`.
    Chiamata { eta_s: i64, turn: u8 },
    /// Valid but pointless now (a district nothing points the fire at, a
    /// district already warned). Shown, never refused.
    Inutile,
}

/// Why an order was not taken. Typed; worded in `crates/text`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The incident is over: no more orders.
    Finita,
    /// The token is not in the tray (the reinforcement before turn 3).
    NonDisponibile,
    /// The token cannot take an order in this state.
    Occupato(TokenState),
    /// No such target this turn.
    BersaglioSconosciuto,
    /// That target cannot be given to that token.
    BersaglioNonValido,
}

/// One line of the between-turn report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReportLine {
    pub kind: ReportKind,
    pub district: Option<usize>,
    pub pos: Option<Pos>,
    pub n: Option<u32>,
    pub token: Option<TokenId>,
}

/// Report kinds, in the order they rank (most important first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReportKind {
    /// The fire reached a district's homes.
    QuartiereRaggiunto,
    /// A unit was burnt over.
    MezzoPerso,
    /// The fire came within the threat distance of a district.
    QuartiereMinacciato,
    /// The wind turned.
    VentoGirato,
    /// New spot fire(s): `n` of them.
    NuovoFocolaio,
    /// A unit pulled back from heat.
    Ritirata,
    /// The patrol reached a district and warned it (`n` families).
    PattugliaArrivata,
    /// Families of a warned district on the road (`n`).
    FamiglieInViaggio,
    /// A spot fire went out.
    FocolaioSpento,
    /// The Canadair arrived over the town.
    CanadairInZona,
    /// The Canadair dropped.
    Lancio,
    /// A unit reached its target and started work.
    Arrivato,
    /// Automatic refill finished; standing orders resume.
    Rifornita,
    /// An engine started roadside fire attack, without home defence.
    AttaccoStrada,
    /// A warning mast went down.
    RipetitoreGiu,
    /// The reinforcement engine joins next turn.
    Rinforzo,
    /// A new forecast arrives next turn.
    NuovoBollettino,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TurnReport {
    /// The turn this reports on (0 before the first is played).
    pub turn: u8,
    pub lines: Vec<ReportLine>,
}

/// The people stamp on a district (gameplay §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stamp {
    InTempo,
    Tardi,
    MaiAvvisati,
    GiustoNonAvvisare,
    /// Warned on the forecast's say-so, and the fire did not come. Not a mistake.
    Prudente,
    AllarmeInutile,
}

impl Stamp {
    pub const ALL: [Stamp; 6] = [Stamp::InTempo, Stamp::Tardi, Stamp::MaiAvvisati, Stamp::GiustoNonAvvisare, Stamp::Prudente, Stamp::AllarmeInutile];

    /// Green on the card.
    pub fn good(self) -> bool {
        matches!(self, Stamp::InTempo | Stamp::GiustoNonAvvisare | Stamp::Prudente)
    }
}

/// A firefighting rule the session kept or broke (≤ 3 on the card).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Note {
    UnitaPersa { token: TokenId },
    /// A ground unit sent at the head pulled back.
    TestaRitirata { token: TokenId },
    /// A ground unit was sent at the head and worked there: it saved no homes.
    TestaInutile { token: TokenId },
    /// The patrol stopped at a district nothing threatened and nothing
    /// pointed at, while the districts at risk waited (lesson 3).
    PattugliaSprecata { district: usize },
    /// Engines defended a district the fire never came near.
    DifesaInutile { district: usize },
    /// Called too late to drop before the last turn ended.
    CanadairTardi,
    CanadairMaiChiamato,
    /// Called in time: this many drops.
    CanadairInTempo { drops: u32 },
    /// Engines were posted before the fire arrived and saved homes.
    CaseDifese { district: usize, homes: u32 },
}

impl Note {
    /// A rule broken (shown first).
    pub fn broken(&self) -> bool {
        !matches!(self, Note::CanadairInTempo { .. } | Note::CaseDifese { .. })
    }
}

/// The verdict card's headline, from families caught against no orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Headline {
    TuttiAlSicuro,
    OttimoLavoro,
    HaiFattoLaDifferenza,
    FuocoPiuVeloce,
}

/// The same fire, with no orders (the twin).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Counterfactual {
    pub families_caught: u32,
    pub homes_hit: u32,
    /// Per district: (families caught, homes hit).
    pub districts: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DistrictVerdict {
    pub district: usize,
    pub people: Stamp,
    pub warning: WarningExplanation,
    pub homes_hit: u32,
    pub homes_hit_none: u32,
    pub caught: u32,
    pub caught_none: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub headline: Headline,
    pub districts: Vec<DistrictVerdict>,
    /// Families the fire caught at home or on the road (shown against
    /// `none.families_caught`; never as "safe out of the whole town").
    pub families_caught: u32,
    pub homes_hit: u32,
    pub households: u32,
    pub none: Counterfactual,
    pub notes: Vec<Note>,
    pub aircraft: AircraftHistory,
    pub aircraft_outcome: AircraftOutcome,
}

/// How a district stands now, for its chip / the QUARTIERI rows.
#[derive(Debug, Clone, PartialEq)]
pub struct DistrictView {
    pub district: usize,
    pub name: String,
    pub households: u32,
    /// Distance from the burning edge to the nearest home, metres.
    pub fire_m: f32,
    pub level: crate::district::Level,
    pub warned: bool,
    pub moving: u32,
    pub safe: u32,
    pub caught: u32,
    /// Tokens working here.
    pub units: Vec<TokenId>,
}

/// Observations only: no future weather draw is exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForecastStatus {
    Pending,
    Observed,
    Elapsed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskDirection {
    CurrentPath,
    PossibleShift,
    Outside,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningReason {
    SufficientLead,
    InsufficientLead,
    NoImprovement,
    NeverWarned,
    ForecastPrecaution,
    Unsupported,
    NoThreat,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WarningExplanation {
    pub reason: WarningReason,
    pub warned_at_s: Option<i64>,
    pub threatened_at_s: Option<i64>,
    pub reached_at_s: Option<i64>,
    pub required_lead_s: i64,
    pub caught: u32,
    pub caught_none: u32,
    pub forecast_at_order: Option<crate::Forecast>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AircraftOutcome {
    NotCalled,
    AwaitingArrival,
    NoTarget,
    UnsafeBreakOff,
    TargetNotReached,
    CompletedDrops,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AircraftHistory {
    pub called_at_s: Option<i64>,
    pub arrived_at_s: Option<i64>,
    pub targeted_at_s: Option<i64>,
    pub broke_off_at_s: Option<i64>,
    pub first_drop_at_s: Option<i64>,
    pub drops: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OngoingWork {
    HomeCoverage { homes: u32 },
    RoadsideAttack,
    NoHomeBenefit,
    AircraftDrops { drops: u32 },
}

/// Coverage is geometric, additional homes exclude coverage of other continuing/pending posts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageExplanation {
    pub homes: u32,
    pub additional: u32,
    pub already_covered: u32,
    pub risk: RiskDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewReason {
    AlreadyWarned,
    WarningEnRoute,
    OutsideDirection { level: crate::Level },
    CoveredPost,
    RoadOutOfReach,
    UnsafeHeat,
    FireAttackNoHomeBenefit,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    AircraftArrival {
        eta_s: i64,
        can_target: bool,
    },
    AircraftNeedsTarget,
    Reinforcement {
        turn: u8,
    },
    ForecastUpdate,
    UnwarnedRisk {
        district: usize,
        risk: RiskDirection,
    },
    Reassignable {
        token: TokenId,
        target: TargetKind,
        on_path: bool,
        level: Option<crate::Level>,
    },
}
