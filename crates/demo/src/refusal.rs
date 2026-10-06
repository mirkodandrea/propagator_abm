//! Why an order cannot be taken, as a type (`docs/demo-spec-gameplay.md` 5.7).
//!
//! `game::command::target_preview` and `abm::suppression::Suppression::assign` both
//! refuse in English `&'static str`s, which the UI then matches by substring to find
//! its Italian. [`Refusal`] is the same set of reasons with no text: [`check`]
//! answers "can this unit take this order at this point?" with the same logic the
//! preview uses, and [`Refusal::from_assign`] classifies the strings `assign` still
//! returns. The presentation side maps each variant to Italian and has a test that
//! every one of [`Refusal::ALL`] is mapped.

use abm::suppression::{Suppression, UnitKind, UnitState};
use abm::Abm;
use fire::FireSim;
use scenario::{Pos, Scenario};

/// What a commander is trying to do with a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Attack,
    CutLine,
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    NoSuchUnit,
    /// Air support has not been requested yet.
    NotOnIncident,
    /// The unit was burnt over.
    UnitLost,
    AircraftCannotWorkLine,
    EngineCannotCutLine,
    OnlyAircraftDrop,
    OutsideScenario,
    /// No connected road or path from the unit to there.
    NoRoad,
    /// The nearest road is further than the hose reaches.
    OutsideHoseReach,
    /// The approach is closed by fire.
    ApproachBlocked,
    /// Nothing unburnt and burnable within the unit's reach.
    NoFuelHere,
}

impl Refusal {
    pub const ALL: [Refusal; 11] = [
        Refusal::NoSuchUnit,
        Refusal::NotOnIncident,
        Refusal::UnitLost,
        Refusal::AircraftCannotWorkLine,
        Refusal::EngineCannotCutLine,
        Refusal::OnlyAircraftDrop,
        Refusal::OutsideScenario,
        Refusal::NoRoad,
        Refusal::OutsideHoseReach,
        Refusal::ApproachBlocked,
        Refusal::NoFuelHere,
    ];

    /// Classify an error string from `Suppression::assign`.
    pub fn from_assign(msg: &str) -> Option<Refusal> {
        Some(match msg {
            "no such unit" => Refusal::NoSuchUnit,
            "not on the incident: request air support first" => Refusal::NotOnIncident,
            "unit is lost" => Refusal::UnitLost,
            "aircraft drop water; they cannot work a line" => Refusal::AircraftCannotWorkLine,
            "an engine cannot cut line -- send a hand crew" => Refusal::EngineCannotCutLine,
            "only aircraft drop" => Refusal::OnlyAircraftDrop,
            _ => return None,
        })
    }
}

/// Can unit `id` take this order at `p`? The same questions, in the same order,
/// as the kiosk's target preview, but answered with a [`Refusal`].
pub fn check(
    crews: &Suppression,
    agents: &Abm,
    fire: &FireSim,
    scn: &Scenario,
    id: usize,
    intent: Intent,
    p: Pos,
) -> Result<(), Refusal> {
    let u = crews.units.get(id).ok_or(Refusal::NoSuchUnit)?;
    if !u.assignable() {
        return Err(if u.state == UnitState::Unavailable { Refusal::NotOnIncident } else { Refusal::UnitLost });
    }
    match (u.kind, intent) {
        (UnitKind::AirTanker, Intent::Attack | Intent::CutLine) => return Err(Refusal::AircraftCannotWorkLine),
        (UnitKind::Engine, Intent::CutLine) => return Err(Refusal::EngineCannotCutLine),
        (UnitKind::HandCrew | UnitKind::Engine, Intent::Drop) => return Err(Refusal::OnlyAircraftDrop),
        _ => {}
    }
    if !scn.world.contains(p) {
        return Err(Refusal::OutsideScenario);
    }
    if !u.kind.is_air() {
        let net = &agents.network;
        let driving = u.kind == UnitKind::Engine;
        let endpoints = net
            .nearest(u.pos, driving)
            .and_then(|from| net.nearest_reachable(p, driving, from).map(|to| (from, to)));
        let Some((from, to)) = endpoints else { return Err(Refusal::NoRoad) };
        let road = net.pos(to);
        if driving && dist(road, p) > abm::suppression::ENGINE_REACH_M {
            return Err(Refusal::OutsideHoseReach);
        }
        if abm::network::route(net, from, to, fire.threat(), driving).is_none() {
            return Err(Refusal::ApproachBlocked);
        }
    }
    let reach = match u.kind {
        UnitKind::Engine => abm::suppression::ENGINE_REACH_M,
        UnitKind::HandCrew => 120.0,
        UnitKind::AirTanker => abm::suppression::DROP_LENGTH_M * 0.5,
    };
    if fire::cells_in_radius(&scn.world, p, reach).into_iter().any(|c| fire.is_suppressible(c, scn)) {
        Ok(())
    } else {
        Err(Refusal::NoFuelHere)
    }
}

fn dist(a: Pos, b: Pos) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}
