//! The one sentence under the outcome card: *why* it went the way it did
//! (`docs/demo-spec-gameplay.md` 1, playtest 16 #37). A pure function of facts the
//! live game and the headless twin both have, so the card and the COMPARE twin
//! explain themselves identically. No text here: the presentation side owns Italian.

use crate::run::Outcome;

/// The moments the explanation is built from (simulated seconds).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Facts {
    /// When the (first) evacuation order was given.
    pub order_at_s: Option<i64>,
    /// When the wind turned, if it did.
    pub shift_at_s: Option<i64>,
    /// When the fire first came within `event::NEAR_TOWN_M` of a home.
    pub near_town_s: Option<i64>,
    /// When the fire first reached somebody still at home.
    pub first_caught_s: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WhyKind {
    /// The fire never came near the town. Whatever was ordered was not needed.
    FireNeverCame,
    /// Nobody was ever caught at home: the order went out in time.
    OrderInTime,
    /// The fire came close but never reached anyone at home, and no order was given.
    CloseButNobodyCaught,
    /// Families were caught at home and no order was ever given.
    NoOrder,
    /// The order came after the fire had already reached families at home.
    OrderTooLate,
    /// The order was early enough for most; some households were still caught
    /// (slow to hear it, to prepare, or to believe it).
    SomeSlowToLeave,
    /// The wind turned toward the town and caught families who had been safe.
    WindShiftReachedTown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Why {
    pub kind: WhyKind,
    pub facts: Facts,
    /// Minutes between the order and the fire first reaching someone at home
    /// (negative: the order came after). `None` when either never happened.
    pub lead_min: Option<i64>,
}

pub fn why(o: &Outcome, f: Facts) -> Why {
    let lead_min = match (f.order_at_s, f.first_caught_s) {
        (Some(order), Some(caught)) => Some((caught - order) / 60),
        _ => None,
    };
    let shift_then_caught = matches!((f.shift_at_s, f.first_caught_s), (Some(s), Some(c)) if c >= s);
    let kind = if o.caught == 0 && f.near_town_s.is_none() {
        WhyKind::FireNeverCame
    } else if o.caught == 0 && f.order_at_s.is_none() {
        WhyKind::CloseButNobodyCaught
    } else if o.caught == 0 {
        WhyKind::OrderInTime
    } else if f.order_at_s.is_none() {
        if shift_then_caught { WhyKind::WindShiftReachedTown } else { WhyKind::NoOrder }
    } else if lead_min.map_or(false, |m| m < 0) {
        WhyKind::OrderTooLate
    } else {
        WhyKind::SomeSlowToLeave
    };
    Why { kind, facts: f, lead_min }
}

impl Outcome {
    /// See [`why`].
    pub fn why(&self, facts: Facts) -> Why {
        why(self, facts)
    }
}
