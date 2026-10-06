//! Things that happen in a run which a commander should be told about
//! (`docs/demo-spec-gameplay.md` 1, 5.4, 5.6): the model says when something
//! needs a decision, a script does not.
//!
//! Events are derived by [`crate::Run::step`] from the model's own state, one pass
//! per step, and carry **no text**: the presentation side maps [`EventKind`] to
//! Italian. Everything here is a plain fact with a time and, where it has one, a
//! place, so it can feed an advisor ticker, a map marker or a decision pause.

use abm::suppression::UnitKind;
use scenario::Pos;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventKind {
    /// A new fire, not contiguous with the front (`abm::spot`, finding 37).
    SpotFire,
    /// The wind changed: the commander's script or the draw turned it.
    WindShifted { from_deg: f32, to_deg: f32 },
    /// A unit pulled back because where it was working is not survivable.
    UnitWithdrew { unit: usize, kind: UnitKind },
    /// A unit was burnt over.
    UnitLost { unit: usize, kind: UnitKind },
    /// A warning mast went down: some households can no longer be reached.
    MastDown,
    /// The fire has come within `NEAR_TOWN_M` of the nearest home for the first
    /// time (a natural moment to ask for a decision).
    FireNearTown,
    /// Fire came within `district::THREATENED_M` of this district for the first time.
    DistrictThreatened { district: usize },
    /// The fire reached this district's homes.
    DistrictReached { district: usize },
    /// The town judged an earlier warning a false alarm (cry-wolf on only).
    FalseAlarm,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    pub at_s: i64,
    pub kind: EventKind,
    pub pos: Option<Pos>,
}

/// The fire is "near the town" when its head is this close to the nearest home.
pub const NEAR_TOWN_M: f32 = 600.0;
