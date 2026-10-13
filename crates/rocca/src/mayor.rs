//! The mayor's authorisation to evacuate (docs/01-GIOCO.md, 2026-10-13).
//!
//! The evacuation order is the mayor's by law: the player may order «Evacua»
//! only for a district she has authorised. She authorises one district when,
//! from what is known now, the front could reach its homes within
//! [`Mayor::lead_s`] (distance over the speed it has been closing, the crisis
//! detector's own estimate, [`crate::crisis::Closing`]), or as soon as the
//! fire is at its doors. An authorisation stays valid to the end.

/// Lead time the authorisation starts from (spec: ~20 simulated minutes).
pub const LEAD_S: i64 = 20 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub struct Mayor {
    /// Authorise a district when the front could be at its homes within this.
    pub lead_s: i64,
}

impl Default for Mayor {
    fn default() -> Mayor {
        Mayor { lead_s: LEAD_S }
    }
}
