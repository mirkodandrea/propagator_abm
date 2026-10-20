//! Road network, driving, suppression units, refuges and havens: the
//! machinery `rocca::Game` builds on.
//!
//! The household agent model (`Abm`: perception, decision, preparation,
//! movement of people) was removed on 2026-10-20: the game no longer
//! simulates individual people.

pub mod behaviour;
pub mod drive;
pub mod haven;
pub mod network;
pub mod refuge;
pub mod spot;
pub mod suppression;
pub mod traffic;

pub use behaviour::{
    BehaviorRuntime, Outcome, PersonOutcome, PersonRuntime, UnitOutcome, UnitRuntime,
};
pub use suppression::{Suppression, SuppressionStats, Task, Unit, UnitKind, UnitState};

/// Deterministic 0-1 noise from an id and a salt.
pub(crate) fn hash01(id: u64, salt: u64) -> f32 {
    let mut h = id.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h >> 40) as f32 / (1u32 << 24) as f32
}
